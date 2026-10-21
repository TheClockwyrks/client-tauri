use std::collections::HashMap;
use std::str::FromStr;

use tauri::utils::acl::RemoteUrlPattern;
use tauri::utils::config::PluginConfig;
use url::Url;

use super::*;

const PROD: &str = "https://console.testcabinet.ai";
const STAGING: &str = "https://console.staging.testcabinet.ai";

fn plugins(value: serde_json::Value) -> PluginConfig {
    PluginConfig(HashMap::from([(CONFIG_KEY.to_string(), value)]))
}

fn url(text: &str) -> Url {
    Url::parse(text).expect("the test's URL parses")
}

#[test]
fn reads_the_url_each_flavor_sets() {
    for flavor in [PROD, STAGING] {
        let console = Console::from_plugins(&plugins(serde_json::json!({ "url": flavor })))
            .expect("the flavor's URL is a console");
        assert_eq!(console.origin(), flavor);
        assert_eq!(console.url().as_str(), format!("{flavor}/"));
    }
}

#[test]
fn reads_the_flavor_overlays_in_this_crate() {
    // The overlays bundle.sh builds, read the way the CLI merges them.
    let base: serde_json::Value =
        serde_json::from_str(include_str!("../tauri.conf.json")).expect("tauri.conf.json is JSON");
    assert!(
        base.pointer("/plugins/console").is_none(),
        "the base configuration names no console, so every build chooses a flavor"
    );
    for (overlay, expected) in [
        (include_str!("../tauri.prod.conf.json"), PROD),
        (include_str!("../tauri.staging.conf.json"), STAGING),
    ] {
        let overlay: serde_json::Value = serde_json::from_str(overlay).expect("an overlay is JSON");
        let mut keys = overlay.as_object().expect("an overlay is an object").keys();
        assert_eq!(keys.next().map(String::as_str), Some("plugins"));
        assert_eq!(
            keys.next(),
            None,
            "an overlay sets the console's URL and nothing else"
        );
        let section = overlay
            .get("plugins")
            .expect("the overlay has plugins")
            .clone();
        let section: HashMap<String, serde_json::Value> =
            serde_json::from_value(section).expect("plugins is an object");
        assert_eq!(section.len(), 1);
        let console =
            Console::from_plugins(&PluginConfig(section)).expect("the overlay names a console");
        assert_eq!(console.origin(), expected);
    }
}

#[test]
fn refuses_a_configuration_without_a_console() {
    let error = Console::from_plugins(&PluginConfig::default()).expect_err("no console");
    assert!(matches!(error, ConsoleError::Missing));
    assert!(error.to_string().contains("tauri.staging.conf.json"));
}

#[test]
fn refuses_a_malformed_section() {
    for value in [
        serde_json::json!("https://console.testcabinet.ai"),
        serde_json::json!({}),
        serde_json::json!({ "url": 3 }),
        serde_json::json!({ "url": PROD, "title": "x" }),
    ] {
        let error = Console::from_plugins(&plugins(value.clone())).expect_err("malformed");
        assert!(
            matches!(error, ConsoleError::Malformed(_)),
            "{value}: {error}"
        );
    }
}

#[test]
fn takes_the_root_of_an_https_origin() {
    assert_eq!(
        Console::parse(&format!("{PROD}/")).expect("root").origin(),
        PROD
    );
    assert_eq!(
        Console::parse("https://console.testcabinet.ai:8443")
            .expect("port")
            .origin(),
        "https://console.testcabinet.ai:8443"
    );
}

#[test]
fn takes_http_on_the_loopback_interface_alone() {
    for local in [
        "http://localhost:1430",
        "http://127.0.0.1:1430",
        "http://[::1]:1430",
    ] {
        assert!(Console::parse(local).is_ok(), "{local}");
    }
    let error = Console::parse("http://console.testcabinet.ai").expect_err("plain http");
    assert!(matches!(error, ConsoleError::Unsupported(..)));
}

#[test]
fn refuses_what_is_not_the_root_of_a_web_origin() {
    for refused in [
        "ftp://console.testcabinet.ai",
        "file:///tmp/index.html",
        "https://console.testcabinet.ai/runs",
        "https://console.testcabinet.ai/?flavor=prod",
        "https://console.testcabinet.ai/#runs",
        "https://operator:secret@console.testcabinet.ai",
        "data:text/html,hi",
    ] {
        let error = Console::parse(refused).expect_err(refused);
        assert!(
            matches!(error, ConsoleError::Unsupported(..)),
            "{refused}: {error}"
        );
    }
    assert!(matches!(
        Console::parse("console.testcabinet.ai"),
        Err(ConsoleError::Unparsable(..))
    ));
}

#[test]
fn owns_its_own_origin_alone() {
    let console = Console::parse(STAGING).expect("staging");
    assert!(console.owns(&url("https://console.staging.testcabinet.ai/runs/1?x=1#y")));
    assert!(!console.owns(&url("https://console.testcabinet.ai/")));
    assert!(!console.owns(&url("http://console.staging.testcabinet.ai/")));
    assert!(!console.owns(&url("https://console.staging.testcabinet.ai:444/")));
}

#[test]
fn the_remote_pattern_matches_the_consoles_pages_alone() {
    let console = Console::parse(PROD).expect("prod");
    let pattern = RemoteUrlPattern::from_str(&console.remote_pattern()).expect("a URL pattern");
    for page in [
        "https://console.testcabinet.ai/",
        "https://console.testcabinet.ai/runs/abc?tab=log#top",
        "https://console.testcabinet.ai/cdn-cgi/access/authorized",
    ] {
        assert!(pattern.test(&url(page)), "{page}");
    }
    for page in [
        "https://console.staging.testcabinet.ai/",
        "https://shrill-term-5c16.cloudflareaccess.com/cdn-cgi/access/login",
        "https://login.microsoftonline.com/common/oauth2/authorize",
        "https://console.testcabinet.ai.example.com/",
        "http://console.testcabinet.ai/",
        "https://console.testcabinet.ai:8443/",
    ] {
        assert!(!pattern.test(&url(page)), "{page}");
    }
}

#[test]
fn grants_each_command_by_its_generated_permission() {
    assert_eq!(permission("save_download"), "allow-save-download");
    assert_eq!(permission("open_external"), "allow-open-external");
    assert_eq!(permission("copy_text"), "allow-copy-text");
}

#[test]
fn tells_the_script_the_origin_and_the_sign_in_hosts() {
    let console = Console::parse(STAGING).expect("staging");
    assert_eq!(
        console.script_config(),
        serde_json::json!({
            "origin": STAGING,
            "signInHosts": ["cloudflareaccess.com", "login.microsoftonline.com"],
        })
    );
}
