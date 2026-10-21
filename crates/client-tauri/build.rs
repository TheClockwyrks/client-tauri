//! The crate's build script.
//!
//! `tauri-build` reads `tauri.conf.json` and generates what
//! `tauri::generate_context!` embeds. The app manifest names the commands the
//! console's pages call, so each takes a permission of its own,
//! `allow-<command>`, and reaches the page only through the capability
//! `src/console.rs` grants the console's origin at startup.

fn main() {
    let manifest =
        tauri_build::AppManifest::new().commands(&["open_external", "copy_text", "save_download"]);
    tauri_build::try_build(tauri_build::Attributes::new().app_manifest(manifest))
        .expect("tauri-build reads tauri.conf.json and the app manifest");
}
