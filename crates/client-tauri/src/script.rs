//! The JavaScript the shell runs in the console's WebView.
//!
//! The affordances themselves are `packages/shell/src/shell.cjs`, an npm
//! workspace package whose own tests and type check cover it. It is embedded
//! here as it is written, so building the crate never runs npm.

use crate::console::Console;

/// The script's source, `installShell` and the helpers it calls.
const SHELL: &str = include_str!("../../../packages/shell/src/shell.cjs");

/// The `window` event the console reopens its event stream on.
pub const RESUME_EVENT: &str = "tcab:resume";

/// The `window` event the script shows a notice for.
pub const NOTICE_EVENT: &str = "tcab:shell-notice";

/// The initialization script for `console`'s window: the shell's source,
/// wrapped so it declares no global, then installed with the console's
/// origin and the sign-in hosts.
///
/// The WebView runs it before each page's own scripts on every page of the
/// main frame, and the script installs itself on the console's origin alone.
pub fn initialization_script(console: &Console) -> String {
    format!(
        "(function (module) {{\n{SHELL}\ninstallShell(window, {config});\n}})();\n",
        config = console.script_config(),
    )
}

/// The script that tells the console the app has returned from the
/// background.
pub fn resume_script() -> String {
    format!(
        "window.dispatchEvent(new Event({}));",
        json_string(RESUME_EVENT)
    )
}

/// The script that shows `text` as a notice on the console's page.
pub fn notice_script(text: &str) -> String {
    format!(
        "window.dispatchEvent(new CustomEvent({}, {{ detail: {} }}));",
        json_string(NOTICE_EVENT),
        json_string(text),
    )
}

/// `text` as a JavaScript string literal. JSON's string syntax is a subset of
/// JavaScript's, so a quote, a backslash or a line terminator stays inside
/// the literal.
fn json_string(text: &str) -> String {
    serde_json::Value::from(text).to_string()
}

#[cfg(test)]
#[path = "script.test.rs"]
mod tests;
