//! Telling the console when the app returns from the background.
//!
//! A WebView suspended in the background, or on a machine that slept, holds a
//! half-open socket when it wakes, so the console's event stream would wait
//! out its stale-stream watchdog before it updated again. The shell instead
//! dispatches `tcab:resume` on the console's `window` on return, and the
//! console reopens its stream at once.
//!
//! What counts as a return:
//!
//! - on Android, the activity resuming, which the runtime reports as
//!   [`tauri::RunEvent::Resumed`];
//! - on the desktop, the window shown again after it was minimized or
//!   hidden, and the machine waking from sleep, which a watcher thread sees
//!   as a gap in the wall clock far longer than the interval it slept for.
//!
//! A window merely losing and regaining focus is not a return: the WebView
//! kept running behind the other window.

use std::time::Duration;

use tauri::{AppHandle, Manager, Runtime};

use crate::console::WINDOW;
use crate::script;

/// Tells the console's page the app has returned from the background.
pub fn dispatch<R: Runtime>(app: &AppHandle<R>) {
    if let Some(window) = app.get_webview_window(WINDOW)
        && let Err(error) = window.eval(script::resume_script())
    {
        eprintln!("the shell could not tell the console it resumed: {error}");
    }
}

/// Whether the window is away, minimized or hidden, and so whether showing it
/// again is a return.
#[derive(Debug, Default)]
pub struct Away {
    away: bool,
}

impl Away {
    /// Records whether the window is `shown` now, and answers whether this
    /// is the window's return from being away.
    pub fn observe(&mut self, shown: bool) -> bool {
        if shown {
            std::mem::take(&mut self.away)
        } else {
            self.away = true;
            false
        }
    }
}

/// How long the sleep watcher sleeps between two looks at the wall clock.
pub const WATCH_INTERVAL: Duration = Duration::from_secs(10);

/// How much longer than [`WATCH_INTERVAL`] a look may come before the gap is
/// read as the machine having slept rather than the thread having been
/// scheduled late.
pub const SLEEP_SLACK: Duration = Duration::from_secs(20);

/// Whether `elapsed` of wall-clock time across one [`WATCH_INTERVAL`] means
/// the machine slept in between.
pub fn slept(elapsed: Duration) -> bool {
    elapsed >= WATCH_INTERVAL + SLEEP_SLACK
}

#[cfg(desktop)]
pub use desktop::{observe_window, watch_sleep};

#[cfg(desktop)]
mod desktop {
    use std::sync::Mutex;
    use std::time::SystemTime;

    use tauri::{AppHandle, Manager, Runtime, WebviewWindow};

    use super::{Away, WATCH_INTERVAL, dispatch, slept};

    /// Looks at the console's window after one of its events and dispatches
    /// the resume event where the window is back from being away.
    pub fn observe_window<R: Runtime>(window: &WebviewWindow<R>) {
        let minimized = window.is_minimized().unwrap_or(false);
        let visible = window.is_visible().unwrap_or(true);
        let away = window.state::<Mutex<Away>>();
        let returned = away
            .lock()
            .map(|mut away| away.observe(visible && !minimized))
            .unwrap_or(false);
        if returned {
            dispatch(window.app_handle());
        }
    }

    /// Starts the thread that watches for the machine sleeping. A machine
    /// asleep runs no thread, so the wall clock across one interval of this
    /// thread's sleep spans the whole time the machine was asleep.
    pub fn watch_sleep<R: Runtime>(app: AppHandle<R>) {
        let spawned = std::thread::Builder::new()
            .name("tcab-sleep-watch".into())
            .spawn(move || {
                loop {
                    let before = SystemTime::now();
                    std::thread::sleep(WATCH_INTERVAL);
                    let elapsed = SystemTime::now().duration_since(before).unwrap_or_default();
                    if slept(elapsed) {
                        dispatch(&app);
                    }
                }
            });
        if let Err(error) = spawned {
            eprintln!("the shell could not watch for the machine sleeping: {error}");
        }
    }
}

/// Answers the runtime's events that mean the app returned.
pub fn on_run_event<R: Runtime>(app: &AppHandle<R>, event: &tauri::RunEvent) {
    match event {
        #[cfg(mobile)]
        tauri::RunEvent::Resumed => dispatch(app),
        #[cfg(desktop)]
        tauri::RunEvent::WindowEvent {
            label,
            event: tauri::WindowEvent::Focused(_) | tauri::WindowEvent::Resized(_),
            ..
        } if label == WINDOW => {
            if let Some(window) = app.get_webview_window(WINDOW) {
                observe_window(&window);
            }
        }
        _ => {}
    }
}

#[cfg(test)]
#[path = "resume.test.rs"]
mod tests;
