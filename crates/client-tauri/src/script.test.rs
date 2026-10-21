use super::*;
use crate::console::Console;

#[test]
fn the_initialization_script_installs_the_shell_for_the_flavors_origin() {
    let console = Console::parse("https://console.staging.testcabinet.ai").expect("staging");
    let script = initialization_script(&console);
    assert!(script.starts_with("(function (module) {\n"));
    assert!(script.contains("function installShell(win, config)"));
    assert!(script.ends_with(
        "\ninstallShell(window, {\"origin\":\"https://console.staging.testcabinet.ai\",\
         \"signInHosts\":[\"cloudflareaccess.com\",\"login.microsoftonline.com\"]});\n})();\n"
    ));
}

#[test]
fn the_script_and_the_crate_name_the_same_events() {
    assert!(SHELL.contains(&format!("const NOTICE_EVENT = \"{NOTICE_EVENT}\";")));
}

#[test]
fn the_resume_script_dispatches_the_consoles_event() {
    assert_eq!(
        resume_script(),
        "window.dispatchEvent(new Event(\"tcab:resume\"));"
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
