//! The commands the injected script calls, which the console's origin alone
//! is granted (see [`crate::console::Console::capability`]), and which each
//! refuse a call that does not carry the launch's token (see
//! [`crate::token`]).

use tauri::ipc::Request;
use tauri::{AppHandle, Runtime, State};
use tauri_plugin_clipboard_manager::ClipboardExt;
use tauri_plugin_opener::OpenerExt;
use url::Url;

use crate::downloads::{self, NAME_HEADER, Saved, TYPE_HEADER};
use crate::token::ShellToken;

/// Opens `url` in the platform's browser.
///
/// # Errors
///
/// Returns an error where the call does not carry the token, `url` is not an
/// `http` or `https` URL, or the platform could not open it.
#[tauri::command]
pub async fn open_external<R: Runtime>(
    app: AppHandle<R>,
    token: State<'_, ShellToken>,
    request: Request<'_>,
    url: String,
) -> Result<(), String> {
    token.check(request.headers())?;
    let url = web_url(&url)?;
    app.opener()
        .open_url(url.as_str(), None::<&str>)
        .map_err(|error| format!("the platform could not open {url}: {error}"))
}

/// Writes `text` to the platform's clipboard.
///
/// # Errors
///
/// Returns an error where the call does not carry the token, or the platform
/// refused the write.
#[tauri::command]
pub async fn copy_text<R: Runtime>(
    app: AppHandle<R>,
    token: State<'_, ShellToken>,
    request: Request<'_>,
    text: String,
) -> Result<(), String> {
    token.check(request.headers())?;
    app.clipboard()
        .write_text(text)
        .map_err(|error| format!("the clipboard refused the text: {error}"))
}

/// Writes the request's body through the platform's download path, under
/// the name its `x-tcab-file-name` header offers.
///
/// # Errors
///
/// Returns an error where the call does not carry the token, the body is not
/// bytes or the file could not be written.
#[tauri::command]
pub async fn save_download<R: Runtime>(
    app: AppHandle<R>,
    token: State<'_, ShellToken>,
    request: Request<'_>,
) -> Result<Saved, String> {
    token.check(request.headers())?;
    let bytes = downloads::body_bytes(request.body())?;
    let header = |name| {
        request
            .headers()
            .get(name)
            .and_then(|value| value.to_str().ok())
    };
    let name = downloads::header_name(header(NAME_HEADER));
    let media_type = header(TYPE_HEADER).unwrap_or_default().to_string();
    save(app, name, media_type, bytes).await
}

/// `text` as a URL the shell opens in the platform's browser: an absolute
/// `http` or `https` URL.
///
/// # Errors
///
/// Returns an error for anything else.
pub fn web_url(text: &str) -> Result<Url, String> {
    let url = Url::parse(text).map_err(|error| format!("{text:?} is not a URL: {error}"))?;
    match url.scheme() {
        "http" | "https" if url.has_host() => Ok(url),
        _ => Err(format!(
            "the shell opens web URLs alone, and {text:?} is not one"
        )),
    }
}

/// Writes a download into the user's download directory.
#[cfg(desktop)]
async fn save<R: Runtime>(
    app: AppHandle<R>,
    name: String,
    _media_type: String,
    bytes: Vec<u8>,
) -> Result<Saved, String> {
    let directory = crate::desktop::download_dir(&app)?;
    let path = tauri::async_runtime::spawn_blocking(move || {
        downloads::write_new(&directory, &name, &bytes)
    })
    .await
    .map_err(|error| format!("the download was not written: {error}"))?
    .map_err(|error| format!("the download was not written: {error}"))?;
    Ok(crate::desktop::saved(&path))
}

/// Hands a download to the Android side, which writes it into the public
/// Downloads collection.
#[cfg(target_os = "android")]
async fn save<R: Runtime>(
    app: AppHandle<R>,
    name: String,
    media_type: String,
    bytes: Vec<u8>,
) -> Result<Saved, String> {
    tauri::async_runtime::spawn_blocking(move || {
        crate::android::save_download(&app, &name, &media_type, &bytes)
    })
    .await
    .map_err(|error| format!("the download was not written: {error}"))?
}

#[cfg(test)]
#[path = "commands.test.rs"]
mod tests;
