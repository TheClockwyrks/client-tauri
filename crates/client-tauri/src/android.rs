//! The shell's Android side, `ShellPlugin` in the Gradle project's
//! `ai.testcabinet.client` package.
//!
//! The Kotlin plugin does what only the Android framework can: it writes a
//! download into the public Downloads collection through `MediaStore`, and it
//! hands a download the WebView starts itself, such as a run's archive, to
//! the system's `DownloadManager` with the WebView's cookies. The path Tauri
//! resolves as the download directory on Android is the application's own
//! external files directory, which the operator's file manager does not
//! show as Downloads, so the shell does not write there.

use serde::Serialize;
use tauri::plugin::{Builder, PluginHandle, TauriPlugin};
use tauri::{AppHandle, Manager, Runtime};

use crate::downloads::Saved;

/// The package the Kotlin plugin's class is in, the application's own.
const PACKAGE: &str = "ai.testcabinet.client";

/// The Kotlin plugin's class.
const CLASS: &str = "ShellPlugin";

/// The handle the Kotlin plugin's commands are called through.
struct Shell<R: Runtime>(PluginHandle<R>);

/// The arguments of the Kotlin plugin's `saveDownload` command.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SaveDownload<'a> {
    /// A file in the application's cache holding the bytes, which the
    /// command moves into the Downloads collection.
    path: String,
    name: &'a str,
    mime_type: &'a str,
}

/// The plugin that registers the Kotlin side when the application starts.
pub fn init<R: Runtime>() -> TauriPlugin<R> {
    Builder::new("tcab-android")
        .setup(|app, api| {
            let handle = api.register_android_plugin(PACKAGE, CLASS)?;
            app.manage(Shell(handle));
            Ok(())
        })
        .build()
}

/// Writes `bytes` into the public Downloads collection as `name`.
///
/// # Errors
///
/// Returns an error where the bytes could not be staged in the cache or the
/// Kotlin side could not write them.
pub fn save_download<R: Runtime>(
    app: &AppHandle<R>,
    name: &str,
    mime_type: &str,
    bytes: &[u8],
) -> Result<Saved, String> {
    let shell = app
        .try_state::<Shell<R>>()
        .ok_or("the shell's Android side is not registered")?;
    let staging = app
        .path()
        .app_cache_dir()
        .map_err(|error| format!("the application names no cache directory: {error}"))?
        .join("downloads");
    let staged = crate::downloads::write_new(&staging, "download", bytes)
        .map_err(|error| format!("the download was not staged: {error}"))?;
    let result = shell.0.run_mobile_plugin::<Saved>(
        "saveDownload",
        SaveDownload {
            path: staged.to_string_lossy().into_owned(),
            name,
            mime_type,
        },
    );
    // The Kotlin side moves the file; one it left behind is removed here.
    let _ = std::fs::remove_file(&staged);
    result.map_err(|error| format!("the download was not written: {error}"))
}
