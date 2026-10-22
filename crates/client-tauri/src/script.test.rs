use super::*;
use crate::console::Console;
use crate::token::ShellToken;

#[test]
fn the_initialization_script_installs_the_shell_with_the_origin_and_the_token() {
    let console = Console::parse("https://console.staging.testcabinet.ai").expect("staging");
    let token = ShellToken::from_value("0123abcd");
    let script = initialization_script(&console, &token);
    assert!(script.starts_with("(function (module) {\n"));
    assert!(script.contains("function installShell(win, config)"));
    let tail = script
        .rsplit_once("\ninstallShell(window, ")
        .map(|(_, tail)| tail)
        .expect("the script installs the shell");
    let config = tail
        .strip_suffix(");\n})();\n")
        .expect("the wrapper closes");
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(config).expect("the config is JSON"),
        console.script_config(&token)
    );
}

#[test]
fn the_script_and_the_crate_name_the_same_events_and_header() {
    assert!(SHELL.contains(&format!("const NOTICE_EVENT = \"{NOTICE_EVENT}\";")));
    assert!(SHELL.contains(&format!(
        "const SHELL_RESUME_EVENT = \"{SHELL_RESUME_EVENT}\";"
    )));
    assert!(SHELL.contains(&format!(
        "const TOKEN_HEADER = \"{}\";",
        crate::token::HEADER
    )));
}

#[test]
fn the_resume_script_raises_the_shells_event() {
    assert_eq!(
        resume_script(),
        "window.dispatchEvent(new Event(\"tcab:shell-resume\"));"
    );
}

#[test]
fn a_notice_stays_inside_its_string_literal() {
    assert_eq!(
        notice_script("Saved \"a\\b\".zip\n</script>"),
        "window.dispatchEvent(new CustomEvent(\"tcab:shell-notice\", \
         { detail: \"Saved \\\"a\\\\b\\\".zip\\n</script>\" }));"
    );
}
