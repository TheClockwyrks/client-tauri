use std::time::Duration;

use super::*;

#[test]
fn a_window_shown_after_being_away_has_returned_once() {
    let mut away = Away::default();
    assert!(!away.observe(false));
    assert!(!away.observe(false));
    assert!(away.observe(true));
    assert!(!away.observe(true));
}

#[test]
fn a_window_never_away_never_returns() {
    let mut away = Away::default();
    for _ in 0..3 {
        assert!(!away.observe(true));
    }
}

#[test]
fn a_gap_far_past_the_interval_is_sleep() {
    assert!(!slept(WATCH_INTERVAL));
    assert!(!slept(WATCH_INTERVAL + Duration::from_secs(5)));
    assert!(!slept(
        WATCH_INTERVAL + SLEEP_SLACK - Duration::from_millis(1)
    ));
    assert!(slept(WATCH_INTERVAL + SLEEP_SLACK));
    assert!(slept(Duration::from_secs(8 * 60 * 60)));
}

#[test]
fn a_clock_set_back_is_not_sleep() {
    // `duration_since` of a clock set backwards is an error, read as zero.
    assert!(!slept(Duration::ZERO));
}
