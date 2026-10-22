//! The JavaScript the shell runs in the console's WebView.
//!
//! The affordances themselves are `packages/shell/src/shell.cjs`, an npm
//! workspace package whose own tests and type check cover it. It is embedded
//! here as it is written, so building the crate never runs npm.

use crate::console::Console;
use crate::token::ShellToken;

/// The script's source, `installShell` and the helpers it calls.
const SHELL: &str = include_str!("../../../packages/shell/src/shell.cjs");

/// The `window` event the shell raises when the app returns from the
/// background. The script turns it into the console's `tcab:resume`, once per
/// return, together with the page's own return to visibility.
pub const SHELL_RESUME_EVENT: &str = "tcab:shell-resume";

/// The `window` event the script shows a notice for.
pub const NOTICE_EVENT: &str = "tcab:shell-notice";

/// The initialization script for `console`'s window: the shell's source,
/// wrapped so it declares no global, then installed with the console's
/// origin, the sign-in hosts and this launch's token.
///
/// The WebView runs it before each page's own scripts, on every frame on
/// Android, and the script installs itself, keeping the token, on the
/// console's origin alone.
pub fn initialization_script(console: &Console, token: &ShellToken) -> String {
    format!(
        "(function (module) {{\n{SHELL}\ninstallShell(window, {config});\n}})();\n",
        config = console.script_config(token),
    )
}

/// The script that tells the console the app has returned from the
/// background.
pub fn resume_script() -> String {
    format!(
        "window.dispatchEvent(new Event({}));",
        json_string(SHELL_RESUME_EVENT)
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
