//! Where a download the console starts is written, and under what name.
//!
//! A download reaches the shell two ways. The injected script fetches the
//! target of an `<a download>` in the page and hands the bytes to the
//! `save_download` command; and on the desktop, a navigation the server
//! answers with an attachment, such as a run's archive, is the WebView's own
//! download, which the shell points at the same directory. Either way the
//! file lands in the platform's download directory under the name the page
//! offered, made safe for every file system and numbered rather than
//! overwriting a file already there.

use std::fs::OpenOptions;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tauri::ipc::InvokeBody;

/// The request header carrying the file name, percent-encoded.
pub const NAME_HEADER: &str = "x-tcab-file-name";

/// The request header carrying the media type, which may be empty.
pub const TYPE_HEADER: &str = "x-tcab-mime-type";

/// The name a download takes when the page offered none the shell can use.
const FALLBACK: &str = "download";

/// The longest file name the shell writes, in bytes, under the 255 most file
/// systems allow, leaving room for a ` (n)` suffix.
const MAX_NAME_BYTES: usize = 200;

/// The extensions read as one with the extension before them, so numbering
/// `run.tar.gz` gives `run (1).tar.gz`.
const COMPOUND: &[&str] = &["gz", "bz2", "xz", "zst"];

/// What the script tells the operator about a saved download.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Saved {
    /// The file name the download was saved under.
    pub name: String,
    /// Where the file went, as the operator would look for it.
    pub location: String,
}

/// `name` as a file name every platform can write: no directory, no control
/// character, none of the characters Windows reserves, no trailing dot or
/// space, not a device name Windows reserves, and no longer than 200 bytes,
/// keeping its extension. A name with nothing left is `download`.
pub fn sanitize(name: &str) -> String {
    // Only the last path segment, whichever separator the page used.
    let base = name.rsplit(['/', '\\']).next().unwrap_or_default();
    let mut cleaned: String = base
        .chars()
        .map(|c| match c {
            '<' | '>' | ':' | '"' | '|' | '?' | '*' => '_',
            c if c.is_control() => '_',
            c => c,
        })
        .collect();
    let trimmed = cleaned.trim_matches(|c: char| c == ' ' || c == '.');
    cleaned = trimmed.to_string();
    if cleaned.is_empty() {
        return FALLBACK.into();
    }
    let (stem, extension) = split(&cleaned);
    if is_reserved_device(stem) {
        cleaned = format!("{stem}_{extension}");
    }
    truncate(&cleaned)
}

/// `name` split into its stem and its extension, the extension carrying its
/// dot: `run.tar.gz` is `run` and `.tar.gz`, `.env` is all stem.
pub fn split(name: &str) -> (&str, &str) {
    let Some(dot) = name.rfind('.').filter(|&dot| dot > 0) else {
        return (name, "");
    };
    let last = &name[dot + 1..];
    if COMPOUND
        .iter()
        .any(|compound| last.eq_ignore_ascii_case(compound))
        && let Some(inner) = name[..dot].rfind('.').filter(|&inner| inner > 0)
    {
        return (&name[..inner], &name[inner..]);
    }
    (&name[..dot], &name[dot..])
}

/// The `attempt`th name to try for `name`: the name itself first, then
/// `stem (1).ext`, `stem (2).ext` and on.
pub fn numbered(name: &str, attempt: u32) -> String {
    if attempt == 0 {
        return name.to_string();
    }
    let (stem, extension) = split(name);
    format!("{stem} ({attempt}){extension}")
}

/// The first path for `name` in `directory` that `taken` says is free.
pub fn free_path(directory: &Path, name: &str, taken: impl Fn(&Path) -> bool) -> PathBuf {
    (0..)
        .map(|attempt| directory.join(numbered(name, attempt)))
        .find(|path| !taken(path))
        .expect("an unbounded run of names holds a free one")
}

/// Writes `bytes` to a new file for `name` in `directory`, creating the
/// directory where it is missing, and returns the path written. The file is
/// created exclusively, so a file already there, even one another program
/// wrote a moment before, is never overwritten.
///
/// # Errors
///
/// Returns the error creating the directory or writing the file raised.
pub fn write_new(directory: &Path, name: &str, bytes: &[u8]) -> io::Result<PathBuf> {
    std::fs::create_dir_all(directory)?;
    for attempt in 0.. {
        let path = directory.join(numbered(name, attempt));
        match OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(mut file) => {
                if let Err(error) = file.write_all(bytes).and_then(|()| file.sync_all()) {
                    drop(file);
                    let _ = std::fs::remove_file(&path);
                    return Err(error);
                }
                return Ok(path);
            }
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(error),
        }
    }
    unreachable!("an unbounded run of names holds a free one")
}

/// The bytes of a `save_download` call. The script sends them as the raw
/// body; where the WebView's IPC carries JSON alone, as Android's does, the
/// runtime hands them on as an array of numbers.
///
/// # Errors
///
/// Returns an error where the body is JSON other than an array of bytes.
pub fn body_bytes(body: &InvokeBody) -> Result<Vec<u8>, String> {
    match body {
        InvokeBody::Raw(bytes) => Ok(bytes.clone()),
        InvokeBody::Json(value) => Vec::<u8>::deserialize(value)
            .map_err(|error| format!("the download's body is not bytes: {error}")),
    }
}

/// The file name the `x-tcab-file-name` header offers, decoded and made safe.
pub fn header_name(header: Option<&str>) -> String {
    let decoded = header
        .map(|value| {
            percent_encoding::percent_decode_str(value)
                .decode_utf8_lossy()
                .into_owned()
        })
        .unwrap_or_default();
    sanitize(&decoded)
}

fn is_reserved_device(stem: &str) -> bool {
    let upper = stem.to_ascii_uppercase();
    matches!(upper.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || ["COM", "LPT"].iter().any(|prefix| {
            upper
                .strip_prefix(prefix)
                .is_some_and(|digit| digit.len() == 1 && digit.as_bytes()[0].is_ascii_digit())
        })
}

/// `name` cut to [`MAX_NAME_BYTES`] on a character boundary, keeping its
/// extension where the extension fits.
fn truncate(name: &str) -> String {
    if name.len() <= MAX_NAME_BYTES {
        return name.to_string();
    }
    let (stem, extension) = split(name);
    let (stem, extension) = if extension.len() < MAX_NAME_BYTES / 2 {
        (stem, extension)
    } else {
        (name, "")
    };
    let mut end = MAX_NAME_BYTES - extension.len();
    while !stem.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}{extension}", &stem[..end])
}

#[cfg(test)]
#[path = "downloads.test.rs"]
mod tests;
