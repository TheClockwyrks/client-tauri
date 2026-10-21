use super::*;

#[test]
fn opens_web_urls_alone() {
    for web in [
        "https://run.pages.dev/",
        "http://localhost:1430/runs",
        "https://tcab.ai/r/1",
    ] {
        assert_eq!(web_url(web).expect(web).as_str(), web);
    }
    for refused in [
        "javascript:alert(1)",
        "file:///etc/passwd",
        "mailto:a@example.com",
        "data:text/html,hi",
        "/runs/1",
        "",
        "https://",
    ] {
        assert!(web_url(refused).is_err(), "{refused}");
    }
}
