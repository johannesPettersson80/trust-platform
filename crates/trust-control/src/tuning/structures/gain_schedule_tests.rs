//! Tests of the value rules of gain scheduling (specification 35, C10), ported from the refusals of
//! `control-studio-gain-schedule.test.ts` (two to four operating points) and the naming and symbol rules of `gainScheduleGenerator.ts`.

#![allow(clippy::float_cmp)] // The magnitude bound is an exact constant.

use super::*;

fn point(name: &str, proportional: &str, integral: &str) -> OperatingPointGains {
    OperatingPointGains {
        name: name.to_owned(),
        proportional_gain: proportional.to_owned(),
        integral_gain: integral.to_owned(),
    }
}

fn low_and_high() -> Vec<OperatingPointGains> {
    vec![
        point("Low", "P1.KpLow", "P1.KiLow"),
        point("High", "P1.KpHigh", "P1.KiHigh"),
    ]
}

#[test]
fn a_schedule_of_two_to_four_points_with_their_own_gains_is_accepted() {
    assert!(check_operating_points(&low_and_high()).is_ok());
    let four: Vec<OperatingPointGains> = (1..=4)
        .map(|index| {
            point(
                &format!("Point {index}"),
                &format!("P1.Kp{index}"),
                &format!("P1.Ki{index}"),
            )
        })
        .collect();
    assert!(check_operating_points(&four).is_ok());
}

#[test]
fn a_schedule_of_fewer_than_two_or_more_than_four_points_is_refused() {
    let both = low_and_high();
    let refusal = check_operating_points(&both[..1]).expect_err("one point is not a schedule");
    assert_eq!(
        refusal.0,
        "A gain schedule needs two to four operating points."
    );
    let five: Vec<OperatingPointGains> = (1..=5)
        .map(|index| {
            point(
                &format!("Point {index}"),
                &format!("P1.Kp{index}"),
                &format!("P1.Ki{index}"),
            )
        })
        .collect();
    assert!(
        check_operating_points(&five).is_err(),
        "five points are too many"
    );
}

#[test]
fn an_operating_point_needs_a_name_that_is_not_empty_or_repeated() {
    let blank = vec![
        point("  ", "P1.KpLow", "P1.KiLow"),
        point("High", "P1.KpHigh", "P1.KiHigh"),
    ];
    let refusal = check_operating_points(&blank).expect_err("a blank name is not a name");
    assert_eq!(
        refusal.0,
        "Give each operating point its own name and its own two gain variables."
    );

    // Names are compared trimmed and in lower case.
    let repeated = vec![
        point("Low", "P1.KpLow", "P1.KiLow"),
        point(" low ", "P1.KpHigh", "P1.KiHigh"),
    ];
    assert!(
        check_operating_points(&repeated).is_err(),
        "‘Low’ and ‘ low ’ are one name"
    );
}

#[test]
fn every_operating_point_has_its_own_two_gain_variables() {
    // The variables are compared in upper case, so a variable repeated in another case is the same one.
    let shared = vec![
        point("Low", "P1.KpLow", "P1.KiLow"),
        point("High", "P1.kplow", "P1.KiHigh"),
    ];
    assert!(
        check_operating_points(&shared).is_err(),
        "P1.KpLow is shared"
    );
    let distinct = vec![
        point("Low", "P1.KpLow", "P1.KiLow"),
        point("High", "P1.KpHigh", "P1.KiLow"),
    ];
    assert!(
        check_operating_points(&distinct).is_err(),
        "P1.KiLow is shared"
    );
}

#[test]
fn a_scheduled_gain_is_bounded_by_a_magnitude_symmetric_about_zero() {
    let bounds = scheduled_gain_bounds();
    assert_eq!(bounds.minimum, -1e6);
    assert_eq!(bounds.maximum, 1e6);
    assert_eq!(SCHEDULED_GAIN_MAGNITUDE, 1e6);
}
