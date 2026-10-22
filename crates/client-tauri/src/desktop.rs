//! What the console's window does on the desktop beyond loading the console:
//! a new window goes to the platform's browser, and the WebView's own
//! downloads go to the user's download directory.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use tauri::webview::{DownloadEvent, NewWindowResponse};
use tauri::{AppHandle, Manager, Runtime, WebviewWindowBuilder};
use tauri_plugin_opener::OpenerExt;

use crate::commands::web_url;
use crate::downloads::{self, Reservations, Saved};
use crate::script;

/// The window's size when it first opens, and the smallest it may be made.
const SIZE: (f64, f64) = (1280.0, 800.0);
const MIN_SIZE: (f64, f64) = (480.0, 360.0);

/// The user's download directory, or `Downloads` in the home directory
/// where the platform names none, as a Linux desktop without XDG user
/// directories does.
///
/// # Errors
///
/// Returns an error where the platform names neither.
pub fn download_dir<R: Runtime>(app: &AppHandle<R>) -> Result<PathBuf, String> {
    let paths = app.path();
    paths
        .download_dir()
        .or_else(|_| paths.home_dir().map(|home| home.join("Downloads")))
        .map_err(|error| format!("the platform names no download directory: {error}"))
}

/// What the page is told about a file saved at `path`.
pub fn saved(path: &Path) -> Saved {
    Saved {
        name: path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default(),
        location: path
            .parent()
            .map(|parent| parent.display().to_string())
            .unwrap_or_default(),
    }
}

/// Gives the console's window its desktop size and handlers.
///
/// The runtime's own handling of files dropped on the window is turned off,
/// so a drop reaches the page as the HTML drag-and-drop events the console
/// is written against.
pub fn configure<'a, R: Runtime, M: Manager<R>>(
    builder: WebviewWindowBuilder<'a, R, M>,
    app: &AppHandle<R>,
    title: &str,
) -> WebviewWindowBuilder<'a, R, M> {
    let opener = app.clone();
    let pending: Arc<Mutex<Reservations>> = Arc::default();
    builder
        .title(title)
        .inner_size(SIZE.0, SIZE.1)
        .min_inner_size(MIN_SIZE.0, MIN_SIZE.1)
        .disable_drag_drop_handler()
        // A new window the script did not catch, from a page it does not run
        // on or a form with a target, goes to the platform's browser too.
        .on_new_window(move |url, _features| {
            if let Ok(url) = web_url(url.as_str())
                && let Err(error) = opener.opener().open_url(url.as_str(), None::<&str>)
            {
                eprintln!("the platform could not open {url}: {error}");
            }
            NewWindowResponse::Deny
        })
        .on_download(move |webview, event| on_download(&webview, event, &pending))
}

/// Points a download the WebView started at the user's download directory,
/// under the name the server offered made safe and numbered past both the
/// files there and the downloads still under way, and tells the operator
/// where it went.
fn on_download<R: Runtime>(
    webview: &tauri::Webview<R>,
    event: DownloadEvent<'_>,
    pending: &Mutex<Reservations>,
) -> bool {
    match event {
        DownloadEvent::Requested { url, destination } => {
            let directory = match download_dir(webview.app_handle()) {
                Ok(directory) => directory,
                Err(error) => {
                    notify(webview, &format!("Could not download: {error}"));
                    return false;
                }
            };
            if let Err(error) = std::fs::create_dir_all(&directory) {
                notify(
                    webview,
                    &format!("Could not download into {}: {error}", directory.display()),
                );
                return false;
            }
            let offered = destination
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_default();
            let name = downloads::sanitize(&offered);
            let Ok(mut reservations) = pending.lock() else {
                notify(
                    webview,
                    "Could not download: the shell's download list failed",
                );
                return false;
            };
            *destination = reservations.reserve(url.as_str(), &directory, &name, Path::exists);
            drop(reservations);
            let saved = saved(destination);
            notify(
                webview,
                &format!("Downloading {} to {}", saved.name, saved.location),
            );
            true
        }
        DownloadEvent::Finished { url, path, success } => {
            let recorded = pending
                .lock()
                .ok()
                .and_then(|mut pending| pending.release(url.as_str(), path.as_deref()));
            let path = path.or(recorded);
            let text = match (success, path) {
                (true, Some(path)) => {
                    let saved = saved(&path);
                    format!("Saved {} to {}", saved.name, saved.location)
                }
                (true, None) => "Download finished".to_string(),
                (false, Some(path)) => format!("Could not download {}", saved(&path).name),
                (false, None) => "Could not download the file".to_string(),
            };
            notify(webview, &text);
            true
        }
        _ => true,
    }
}

/// Shows `text` as a notice on the page.
fn notify<R: Runtime>(webview: &tauri::Webview<R>, text: &str) {
    if let Err(error) = webview.eval(script::notice_script(text)) {
        eprintln!("the shell could not show a notice: {error}");
    }
}
