//! The secret that proves a command comes from the shell's own script on the
//! console's page.
//!
//! The capability that grants the shell's commands names the console's
//! origin, but a WebView does not always report the origin of the frame that
//! called: Android's injects every initialization script, the runtime's IPC
//! among them, into every frame, and reports the main frame's URL for a call
//! from any of them. A cross-origin frame on the console's page, such as a
//! run's playable build, could otherwise call the shell's commands.
//!
//! So each launch draws a random token, the initialization script carries it
//! into `installShell`, which keeps it in a closure once the page's origin
//! is the console's and never puts it where the page can read it, and every
//! command refuses a call whose `x-tcab-shell-token` header does not carry it.

use std::fmt::Write;

/// The request header that carries the token.
pub const HEADER: &str = "x-tcab-shell-token";

/// The token of this launch.
#[derive(Clone)]
pub struct ShellToken(String);

impl ShellToken {
    /// A fresh token: 32 random bytes from the operating system, as hex.
    ///
    /// # Errors
    ///
    /// Returns an error where the operating system offers no randomness.
    pub fn generate() -> Result<Self, getrandom::Error> {
        let mut bytes = [0u8; 32];
        getrandom::fill(&mut bytes)?;
        Ok(Self(bytes.iter().fold(
            String::with_capacity(64),
            |mut hex, byte| {
                let _ = write!(hex, "{byte:02x}");
                hex
            },
        )))
    }

    /// The token as the script is given it.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Whether `presented` is this token, compared in constant time.
    pub fn admits(&self, presented: Option<&str>) -> bool {
        presented.is_some_and(|presented| constant_time_eq(self.0.as_bytes(), presented.as_bytes()))
    }

    /// Refuses a call that does not present this token.
    ///
    /// # Errors
    ///
    /// Returns the refusal the script reports.
    pub fn check(&self, headers: &tauri::http::HeaderMap) -> Result<(), String> {
        let presented = headers.get(HEADER).and_then(|value| value.to_str().ok());
        if self.admits(presented) {
            Ok(())
        } else {
            Err("the call did not come from the shell's script on the console".into())
        }
    }
}

impl std::fmt::Debug for ShellToken {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("ShellToken(..)")
    }
}

/// Whether `a` and `b` are equal, taking a time that depends on their lengths
/// alone.
pub fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.iter()
        .zip(b)
        .fold(0u8, |difference, (x, y)| difference | (x ^ y))
        == 0
}

#[cfg(test)]
impl ShellToken {
    /// A token with a known value, for tests.
    pub fn from_value(value: &str) -> Self {
        Self(value.into())
    }
}

#[cfg(test)]
#[path = "token.test.rs"]
mod tests;
