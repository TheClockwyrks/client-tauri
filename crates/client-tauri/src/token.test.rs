use tauri::http::{HeaderMap, HeaderValue};

use super::*;

#[test]
fn a_token_is_64_hex_digits_and_fresh_each_launch() {
    let first = ShellToken::generate().expect("randomness");
    let second = ShellToken::generate().expect("randomness");
    assert_eq!(first.as_str().len(), 64);
    assert!(first.as_str().bytes().all(|b| b.is_ascii_hexdigit()));
    assert_ne!(first.as_str(), second.as_str());
}

#[test]
fn admits_the_token_alone() {
    let token = ShellToken::from_value("abc123");
    assert!(token.admits(Some("abc123")));
    assert!(!token.admits(Some("abc124")));
    assert!(!token.admits(Some("abc12")));
    assert!(!token.admits(Some("abc1234")));
    assert!(!token.admits(Some("")));
    assert!(!token.admits(None));
}

#[test]
fn checks_the_header() {
    let token = ShellToken::from_value("abc123");
    let mut headers = HeaderMap::new();
    assert!(token.check(&headers).is_err());
    headers.insert(HEADER, HeaderValue::from_static("nope"));
    assert!(token.check(&headers).is_err());
    headers.insert(HEADER, HeaderValue::from_static("abc123"));
    assert!(token.check(&headers).is_ok());
}

#[test]
fn compares_bytes_exactly() {
    assert!(constant_time_eq(b"", b""));
    assert!(constant_time_eq(b"same", b"same"));
    assert!(!constant_time_eq(b"same", b"Same"));
    assert!(!constant_time_eq(b"same", b"sam"));
}

#[test]
fn never_prints_the_token() {
    let token = ShellToken::from_value("secret");
    assert!(!format!("{token:?}").contains("secret"));
}
