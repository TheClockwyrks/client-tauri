//! The operator client for Windows, Linux, macOS and Android: the console of
//! The Test Cabinet in the platform's WebView.
//!
//! What the shell is required to do is stated by the operator client's page
//! of the superrepo's documentation site,
//! `apps/docs/src/content/docs/components/client/overview.md`. In short, the
//! shell loads the one console URL its build flavor names ([`console`]) and
//! carries the affordances a WebView does not supply: new windows open in
//! the platform's browser, downloads go through the platform's download path
//! ([`downloads`]), the clipboard is reached through the shell, and a return
//! from the background tells the console to reopen its event stream
//! ([`resume`]). The page side of those is a script injected into the
//! console's pages ([`script`]), which calls the [`commands`] over the IPC.
//!
//! Sign-in is the console's own: its origin sits behind Cloudflare Access, so
//! the WebView follows the Access and Microsoft login pages and keeps the
//! cookie the edge sets in its persistent profile. The shell holds no
//! credential and adds nothing to a request.

#[cfg(target_os = "android")]
mod android;
pub mod commands;
pub mod console;
#[cfg(desktop)]
mod desktop;
pub mod downloads;
pub mod resume;
pub mod script;

use tauri::webview::Color;
use tauri::{App, AppHandle, Manager, Runtime, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

use crate::console::{Console, WINDOW};

/// The console's background color, painted before its first page arrives so
/// the window opens dark like the page rather than flashing white.
const BACKGROUND: Color = Color(5, 1, 8, 255);

/// Runs the shell.
///
/// Where the configuration names no console, as a build without a flavor's
/// overlay does, or where the shell cannot start, it says so on standard
/// error and exits with status 1.
///
/// # Panics
///
/// Panics where the event loop fails once it runs, which leaves nothing to
/// use.
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let context = tauri::generate_context!();
    let console =
        Console::from_plugins(&context.config().plugins).unwrap_or_else(|error| exit(&error));
    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_clipboard_manager::init());
    #[cfg(target_os = "android")]
    let builder = builder.plugin(android::init());
    let app = builder
        .invoke_handler(tauri::generate_handler![
            commands::open_external,
            commands::copy_text,
            commands::save_download,
        ])
        .setup(move |app| setup(app, console))
        .build(context)
        .unwrap_or_else(|error| exit(&error));
    app.run(|app, event| resume::on_run_event(app, &event));
}

/// Reports why the shell cannot start and exits.
fn exit(error: &dyn std::fmt::Display) -> ! {
    eprintln!("The Test Cabinet could not start: {error}");
    std::process::exit(1);
}

/// Grants the console's origin the shell's commands and opens its window.
fn setup<R: Runtime>(app: &mut App<R>, console: Console) -> Result<(), Box<dyn std::error::Error>> {
    app.add_capability(console.capability())?;
    #[cfg(desktop)]
    {
        app.manage(std::sync::Mutex::new(resume::Away::default()));
        resume::watch_sleep(app.handle().clone());
    }
    open_console(app.handle(), &console)?;
    app.manage(console);
    Ok(())
}

/// Opens the window that loads `console`, with the shell's script injected
/// into every page of its main frame.
fn open_console<R: Runtime>(
    app: &AppHandle<R>,
    console: &Console,
) -> tauri::Result<WebviewWindow<R>> {
    let builder =
        WebviewWindowBuilder::new(app, WINDOW, WebviewUrl::External(console.url().clone()))
            .initialization_script(script::initialization_script(console))
            .background_color(BACKGROUND);
    #[cfg(desktop)]
    let builder = {
        let title = app
            .config()
            .product_name
            .clone()
            .unwrap_or_else(|| "The Test Cabinet".into());
        desktop::configure(builder, app, &title)
    };
    builder.build()
}
