//! The console a build of the shell loads, read from the build's configuration.
//!
//! A build flavor is `tauri.<flavor>.conf.json` beside the crate's
//! `tauri.conf.json`, which the Tauri CLI merges over it as a JSON Merge Patch.
//! The two flavors differ in the console's URL alone, so the URL sits where an
//! overlay can set it as one key: `plugins.console.url`. The window list is an
//! array, which a merge patch replaces whole, so the window is created here
//! rather than declared there, and an overlay restates nothing else.

use std::fmt;

use serde::Deserialize;
use tauri::ipc::CapabilityBuilder;
use tauri::utils::config::PluginConfig;
use url::{Host, Url};

use crate::token::ShellToken;

/// The key under `plugins` in the configuration that holds the console.
pub const CONFIG_KEY: &str = "console";

/// The hosts of the sign-in flow, each standing for itself and its
/// subdomains: the Cloudflare Access team domain in front of the console and
/// the Microsoft login pages it signs in at. A link from the console to one of
/// them, and a navigation of the main frame to one, stays inside the WebView.
///
/// Only the hosts a top-level navigation of the sign-in flow reaches are
/// listed: Microsoft Entra's sign-in and its legacy and seamless single
/// sign-on endpoints, the Microsoft account pages a personal account passes
/// through, and the security-info pages a first multi-factor registration
/// leads to. The flow's scripts and images, from `aadcdn.msftauth.net` and
/// the like, are subresources no navigation policy judges.
pub const SIGN_IN_HOSTS: &[&str] = &[
    "cloudflareaccess.com",
    "login.microsoftonline.com",
    "login.microsoft.com",
    "login.windows.net",
    "autologon.microsoftazuread-sso.com",
    "login.live.com",
    "account.live.com",
    "mysignins.microsoft.com",
    "account.activedirectory.windowsazure.com",
];

/// The label of the one window, the console's.
pub const WINDOW: &str = "main";

/// The commands the console's pages call, each granted to the console's
/// origin alone. `build.rs` names the same commands in the app manifest.
pub const COMMANDS: &[&str] = &["open_external", "copy_text", "save_download"];

/// `plugins.console` as the configuration states it.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ConsoleConfig {
    url: String,
}

/// The console the shell loads.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Console {
    url: Url,
}

/// Why the configuration names no console the shell can load.
#[derive(Debug)]
pub enum ConsoleError {
    /// The configuration holds no `plugins.console`, which a flavor's overlay
    /// supplies.
    Missing,
    /// `plugins.console` is not an object holding the one key `url`.
    Malformed(serde_json::Error),
    /// The URL does not parse.
    Unparsable(String, url::ParseError),
    /// The URL is not one the shell loads: `https`, or `http` on the loopback
    /// interface, at the root of a host.
    Unsupported(String, &'static str),
}

impl fmt::Display for ConsoleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Missing => write!(
                f,
                "the configuration names no console: build with a flavor's overlay, such as \
                 `tauri dev --config tauri.staging.conf.json`, which sets plugins.{CONFIG_KEY}.url"
            ),
            Self::Malformed(error) => {
                write!(
                    f,
                    "plugins.{CONFIG_KEY} is not {{ \"url\": <string> }}: {error}"
                )
            }
            Self::Unparsable(url, error) => {
                write!(f, "plugins.{CONFIG_KEY}.url {url:?} is not a URL: {error}")
            }
            Self::Unsupported(url, why) => {
                write!(f, "plugins.{CONFIG_KEY}.url {url:?} {why}")
            }
        }
    }
}

impl std::error::Error for ConsoleError {}

impl Console {
    /// The console the configuration's `plugins` section names.
    ///
    /// # Errors
    ///
    /// Returns an error where the section names none, or names one the shell
    /// does not load (see [`Console::parse`]).
    pub fn from_plugins(plugins: &PluginConfig) -> Result<Self, ConsoleError> {
        let value = plugins.0.get(CONFIG_KEY).ok_or(ConsoleError::Missing)?;
        let config = ConsoleConfig::deserialize(value).map_err(ConsoleError::Malformed)?;
        Self::parse(&config.url)
    }

    /// The console at `url`.
    ///
    /// The URL is the root of an `https` origin, or of an `http` one on the
    /// loopback interface, which is where a console served on the developer's
    /// own machine is.
    ///
    /// # Errors
    ///
    /// Returns an error where `url` does not parse, has another scheme, names
    /// no host, carries credentials, or names a path, query or fragment.
    pub fn parse(url: &str) -> Result<Self, ConsoleError> {
        let parsed =
            Url::parse(url).map_err(|error| ConsoleError::Unparsable(url.into(), error))?;
        let unsupported = |why| Err(ConsoleError::Unsupported(url.into(), why));
        let Some(host) = parsed.host() else {
            return unsupported("names no host");
        };
        match parsed.scheme() {
            "https" => {}
            "http" if is_loopback(&host) => {}
            "http" => return unsupported("is http on a host other than the loopback interface"),
            _ => return unsupported("is neither https nor http"),
        }
        if !parsed.username().is_empty() || parsed.password().is_some() {
            return unsupported("carries credentials");
        }
        if parsed.path() != "/" || parsed.query().is_some() || parsed.fragment().is_some() {
            return unsupported("is not the root of its origin");
        }
        Ok(Self { url: parsed })
    }

    /// The URL the shell loads.
    pub fn url(&self) -> &Url {
        &self.url
    }

    /// The console's origin, as a page's `location.origin` states it.
    pub fn origin(&self) -> String {
        self.url.origin().ascii_serialization()
    }

    /// Whether `url` is on the console's origin.
    pub fn owns(&self, url: &Url) -> bool {
        url.origin() == self.url.origin()
    }

    /// The capability that lets the console's pages, and no other page the
    /// WebView loads, call the shell's commands. The shell bundles no page
    /// of its own, so nothing local is granted either.
    pub fn capability(&self) -> CapabilityBuilder {
        COMMANDS.iter().fold(
            CapabilityBuilder::new("console")
                .local(false)
                .remote(self.remote_pattern())
                .window(WINDOW),
            |capability, command| capability.permission(permission(command)),
        )
    }

    /// The remote URL pattern every page of the console matches: the origin,
    /// whose path, query and fragment Tauri reads as wildcards.
    pub fn remote_pattern(&self) -> String {
        self.origin()
    }

    /// What the injected script is told: the console's origin, the sign-in
    /// flow's hosts, and this launch's token (see [`crate::token`]).
    pub fn script_config(&self, token: &ShellToken) -> serde_json::Value {
        serde_json::json!({
            "origin": self.origin(),
            "signInHosts": SIGN_IN_HOSTS,
            "token": token.as_str(),
        })
    }

    /// What the main frame does with a navigation to `url`: the console's
    /// origin and the sign-in flow's hosts load in the WebView, any other
    /// web URL opens in the platform's browser, a document the page made
    /// itself (`about:`, `blob:`, `data:`) loads, and anything else is
    /// refused.
    pub fn navigation(&self, url: &Url) -> Navigation {
        match url.scheme() {
            "http" | "https" => {
                if self.owns(url) || url.host_str().is_some_and(is_sign_in_host) {
                    Navigation::Load
                } else {
                    Navigation::Browser
                }
            }
            "about" | "blob" | "data" => Navigation::Load,
            _ => Navigation::Refuse,
        }
    }
}

/// What the main frame does with a navigation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Navigation {
    /// The WebView loads the URL.
    Load,
    /// The URL opens in the platform's browser and the WebView stays.
    Browser,
    /// The WebView stays and nothing opens.
    Refuse,
}

/// Whether `host` is one of [`SIGN_IN_HOSTS`] or a subdomain of one.
pub fn is_sign_in_host(host: &str) -> bool {
    let host = host.trim_end_matches('.').to_ascii_lowercase();
    SIGN_IN_HOSTS.iter().any(|entry| {
        host == *entry
            || host
                .strip_suffix(entry)
                .is_some_and(|prefix| prefix.ends_with('.'))
    })
}

/// The permission the app manifest generates for `command`.
pub fn permission(command: &str) -> String {
    format!("allow-{}", command.replace('_', "-"))
}

fn is_loopback(host: &Host<&str>) -> bool {
    match host {
        Host::Domain(domain) => domain.eq_ignore_ascii_case("localhost"),
        Host::Ipv4(address) => address.is_loopback(),
        Host::Ipv6(address) => address.is_loopback(),
    }
}

#[cfg(test)]
#[path = "console.test.rs"]
mod tests;
