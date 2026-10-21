use std::collections::HashSet;
use std::path::{Path, PathBuf};

use tauri::ipc::InvokeBody;

use super::*;

#[test]
fn keeps_an_ordinary_name() {
    assert_eq!(sanitize("run-01HZ.tar.gz"), "run-01HZ.tar.gz");
    assert_eq!(sanitize("frame 01 (final).png"), "frame 01 (final).png");
    assert_eq!(sanitize("café.gif"), "café.gif");
}

#[test]
fn keeps_only_the_last_path_segment() {
    assert_eq!(sanitize("../../.ssh/authorized_keys"), "authorized_keys");
    assert_eq!(sanitize("C:\\Windows\\system.ini"), "system.ini");
    assert_eq!(sanitize("runs/abc/"), "download");
}

#[test]
fn replaces_reserved_and_control_characters() {
    assert_eq!(sanitize("a<b>c:d\"e|f?g*h.txt"), "a_b_c_d_e_f_g_h.txt");
    assert_eq!(sanitize("line\nbreak\u{0}.txt"), "line_break_.txt");
}

#[test]
fn trims_dots_and_spaces_at_either_end() {
    assert_eq!(sanitize("  report.pdf. "), "report.pdf");
    assert_eq!(sanitize(".."), "download");
    assert_eq!(sanitize(""), "download");
    assert_eq!(sanitize("   "), "download");
}

#[test]
fn renames_a_device_windows_reserves() {
    assert_eq!(sanitize("CON"), "CON_");
    assert_eq!(sanitize("nul.txt"), "nul_.txt");
    assert_eq!(sanitize("com1.tar.gz"), "com1_.tar.gz");
    assert_eq!(sanitize("LPT9.log"), "LPT9_.log");
    assert_eq!(sanitize("console.log"), "console.log");
    assert_eq!(sanitize("COM10.txt"), "COM10.txt");
}

#[test]
fn shortens_a_long_name_keeping_its_extension() {
    let long = format!("{}.tar.gz", "é".repeat(150));
    let short = sanitize(&long);
    assert!(short.len() <= 200, "{} bytes", short.len());
    assert!(short.ends_with("é.tar.gz"));
    let no_extension = "x".repeat(300);
    assert_eq!(sanitize(&no_extension), "x".repeat(200));
    let long_extension = format!("a.{}", "y".repeat(250));
    assert_eq!(sanitize(&long_extension).len(), 200);
}

#[test]
fn splits_a_compound_extension() {
    assert_eq!(split("run.tar.gz"), ("run", ".tar.gz"));
    assert_eq!(split("run.TAR.ZST"), ("run", ".TAR.ZST"));
    assert_eq!(split("clip.webm"), ("clip", ".webm"));
    assert_eq!(split("archive.gz"), ("archive", ".gz"));
    assert_eq!(split(".env"), (".env", ""));
    assert_eq!(split("README"), ("README", ""));
    assert_eq!(split("v1.2.json"), ("v1.2", ".json"));
}

#[test]
fn numbers_a_name_before_its_extension() {
    assert_eq!(numbered("run.tar.gz", 0), "run.tar.gz");
    assert_eq!(numbered("run.tar.gz", 1), "run (1).tar.gz");
    assert_eq!(numbered("README", 12), "README (12)");
}

#[test]
fn finds_the_first_free_path() {
    let directory = Path::new("/downloads");
    let taken: HashSet<PathBuf> = ["run.tar.gz", "run (1).tar.gz"]
        .iter()
        .map(|name| directory.join(name))
        .collect();
    assert_eq!(
        free_path(directory, "run.tar.gz", |path| taken.contains(path)),
        directory.join("run (2).tar.gz")
    );
    assert_eq!(
        free_path(directory, "clip.webm", |path| taken.contains(path)),
        directory.join("clip.webm")
    );
}

#[test]
fn writes_a_new_file_and_never_overwrites_one() {
    let root = tempfile::tempdir().expect("a temporary directory");
    let directory = root.path().join("Downloads");
    let first = write_new(&directory, "clip.gif", b"one").expect("the first write");
    let second = write_new(&directory, "clip.gif", b"two").expect("the second write");
    assert_eq!(first, directory.join("clip.gif"));
    assert_eq!(second, directory.join("clip (1).gif"));
    assert_eq!(std::fs::read(&first).expect("first"), b"one");
    assert_eq!(std::fs::read(&second).expect("second"), b"two");
}

#[test]
fn reports_a_directory_it_cannot_create() {
    let root = tempfile::tempdir().expect("a temporary directory");
    let file = root.path().join("not-a-directory");
    std::fs::write(&file, b"").expect("a file");
    assert!(write_new(&file.join("Downloads"), "a.txt", b"x").is_err());
}

#[test]
fn reads_the_body_raw_or_as_json_bytes() {
    assert_eq!(
        body_bytes(&InvokeBody::Raw(vec![1, 2, 255])),
        Ok(vec![1, 2, 255])
    );
    assert_eq!(
        body_bytes(&InvokeBody::Json(serde_json::json!([1, 2, 255]))),
        Ok(vec![1, 2, 255])
    );
    assert!(body_bytes(&InvokeBody::Json(serde_json::json!([256]))).is_err());
    assert!(body_bytes(&InvokeBody::Json(serde_json::json!({ "bytes": [] }))).is_err());
}

#[test]
fn decodes_the_name_header() {
    assert_eq!(header_name(Some("run%201.gif")), "run 1.gif");
    assert_eq!(header_name(Some("caf%C3%A9.png")), "café.png");
    assert_eq!(header_name(Some("..%2F..%2Fetc%2Fpasswd")), "passwd");
    assert_eq!(header_name(Some("%FF.bin")), "\u{FFFD}.bin");
    assert_eq!(header_name(None), "download");
}
