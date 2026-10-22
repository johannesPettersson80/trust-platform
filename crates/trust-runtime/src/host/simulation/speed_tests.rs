//! Specification 35, A2.4 and B9.2, and specification 33, §6.1: the speed words and the target kind the runtime reports.

use super::*;

#[test]
fn a_target_is_simulated_only_when_simulation_is_enabled_and_all_io_is_simulated() {
    assert!(speed_facts(true, true, 1, None).simulated);
    assert!(
        !speed_facts(true, false, 1, None).simulated,
        "an attached I/O that is not simulated makes the target physical"
    );
    assert!(!speed_facts(false, true, 1, None).simulated);
}

#[test]
fn without_simulation_the_speed_is_one_and_names_nothing() {
    let facts = speed_facts(false, true, 10, Some(4.0));
    assert_eq!(facts.speed_text(), "", "no simulation runs at a speed");
    assert_eq!(facts.configured, None);
    assert!(!facts.speed_behind());
}

#[test]
fn a_zero_configured_speed_runs_at_real_time() {
    let facts = speed_facts(true, true, 0, Some(0.5));
    assert_eq!(facts.speed_text(), "");
    assert_eq!(facts.configured, None);
}

#[test]
fn a_simulation_at_real_time_names_no_speed_and_shows_no_achieved_speed() {
    assert_eq!(speed_facts(true, true, 1, None).speed_text(), "");
    let slow = speed_facts(true, true, 1, Some(0.9));
    assert_eq!(slow.speed_text(), "", "no achieved speed at 1x");
    assert!(!slow.speed_behind());
}

#[test]
fn a_simulation_names_its_configured_speed_with_the_multiplication_sign() {
    let facts = speed_facts(true, true, 10, Some(10.0));
    assert_eq!(facts.speed_text(), "\u{00D7}10");
    assert_eq!(facts.configured, Some(10));
    assert!(
        !facts.speed_text().contains('x'),
        "the speed word is the sign, not the letter: {}",
        facts.speed_text()
    );
    assert!(!facts.speed_behind());
}

#[test]
fn an_achieved_speed_more_than_five_percent_below_the_configured_one_is_shown_and_behind() {
    let seven = speed_facts(true, true, 10, Some(7.0));
    assert_eq!(seven.speed_text(), "\u{00D7}7 of \u{00D7}10");
    assert!(seven.speed_behind());
    assert_eq!(seven.achieved_behind.as_deref(), Some("7"));
    assert_eq!(
        speed_facts(true, true, 100, Some(94.9)).speed_text(),
        "\u{00D7}94.9 of \u{00D7}100"
    );
    assert_eq!(
        speed_facts(true, true, 2, Some(1.8)).speed_text(),
        "\u{00D7}1.8 of \u{00D7}2"
    );
}

#[test]
fn an_achieved_speed_at_or_above_95_percent_is_not_behind() {
    let just_below_the_share = speed_facts(true, true, 10, Some(9.6));
    assert_eq!(
        just_below_the_share.speed_text(),
        "\u{00D7}10",
        "9.6 is not below 9.5, which is 0.95 times 10"
    );
    assert!(!just_below_the_share.speed_behind());
    let exactly_the_share = speed_facts(true, true, 10, Some(9.5));
    assert_eq!(
        exactly_the_share.speed_text(),
        "\u{00D7}10",
        "exactly 0.95 times the configured speed is not behind"
    );
    assert!(!exactly_the_share.speed_behind());
    assert_eq!(
        speed_facts(true, true, 2, Some(1.9)).speed_text(),
        "\u{00D7}2"
    );
}

#[test]
fn a_missing_or_non_positive_achieved_speed_gives_no_of() {
    assert_eq!(speed_facts(true, true, 10, None).speed_text(), "\u{00D7}10");
    assert_eq!(
        speed_facts(true, true, 10, Some(0.0)).speed_text(),
        "\u{00D7}10",
        "a speed that is not positive is no achieved speed"
    );
    assert_eq!(
        speed_facts(true, true, 10, Some(f64::NAN)).speed_text(),
        "\u{00D7}10",
        "a speed that is not a number is no achieved speed"
    );
}

#[test]
fn the_achieved_speed_is_shown_rounded_down_to_one_decimal() {
    let rounded = speed_facts(true, true, 10, Some(9.45));
    assert_eq!(
        rounded.speed_text(),
        "\u{00D7}9.4 of \u{00D7}10",
        "a shown value is never at or above the threshold it is behind"
    );
    assert!(rounded.speed_behind());
    assert_eq!(rounded.achieved_behind.as_deref(), Some("9.4"));
    assert_eq!(
        speed_facts(true, true, 10, Some(5.0)).speed_text(),
        "\u{00D7}5 of \u{00D7}10"
    );
}

#[test]
fn the_speed_words_follow_the_clock_whatever_the_io_is() {
    assert_eq!(
        speed_facts(true, false, 10, Some(10.0)).speed_text(),
        "\u{00D7}10",
        "the words describe the clock; the target kind is what decides physical"
    );
}
