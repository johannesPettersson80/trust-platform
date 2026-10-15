//! Tests of the value rules of gain scheduling (specification 35, C10), ported from the refusals of
//! `control-studio-gain-schedule.test.ts` (two to four operating points) and the naming and symbol rules of `gainScheduleGenerator.ts`,
//! and the rules C10 adds: the order of the values, the tolerance, the choice between the fixed and the scheduled design (D-19), and the
//! Identify precondition (D-21).

#![allow(clippy::float_cmp)] // The magnitude bound is an exact constant.

use super::*;
use crate::loop_definition::SchedulePoint;
use crate::tuning::structure::ParallelGains;

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

/// A schedule point with the given value and tolerance, for the Identify precondition and the table rules.
fn table_point(name: &str, value: f64, tolerance: f64) -> SchedulePoint {
    SchedulePoint {
        name: name.to_owned(),
        loop_id: format!("loop:{}", name.to_lowercase()),
        value,
        tolerance,
        proportional_symbol: format!("P1.Kp{name}"),
        integral_symbol: format!("P1.Ki{name}"),
    }
}

fn gains(proportional: f64, integral: f64) -> ParallelGains {
    ParallelGains {
        proportional_gain: proportional,
        integral_gain: integral,
        derivative_gain: 0.0,
    }
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

#[test]
fn the_fixed_design_is_recommended_when_it_meets_the_requirements_at_every_point() {
    assert_eq!(choose_schedule(&[true, true]), ScheduleChoice::Fixed);
    assert_eq!(choose_schedule(&[true, true, true]), ScheduleChoice::Fixed);
}

#[test]
fn a_scheduled_design_is_recommended_only_when_the_fixed_design_fails_a_point() {
    // One failing point is enough, wherever it is (D-19).
    assert_eq!(choose_schedule(&[true, false]), ScheduleChoice::Scheduled);
    assert_eq!(choose_schedule(&[false, true]), ScheduleChoice::Scheduled);
    assert_eq!(choose_schedule(&[false, false]), ScheduleChoice::Scheduled);
}

#[test]
fn the_fixed_design_gives_the_first_points_gains_at_every_point() {
    let designs = [gains(2.0, 0.2), gains(3.0, 0.3)];
    assert_eq!(
        gains_per_point(ScheduleChoice::Fixed, &designs),
        vec![gains(2.0, 0.2), gains(2.0, 0.2)]
    );
    assert_eq!(
        gains_per_point(ScheduleChoice::Scheduled, &designs),
        vec![gains(2.0, 0.2), gains(3.0, 0.3)]
    );
}

#[test]
fn the_values_of_the_points_must_increase_in_the_order_given() {
    let increasing = [("Low", 0.09), ("High", 0.49)];
    assert!(check_point_order(&increasing).is_ok());

    let decreasing = [("Low", 0.49), ("High", 0.09)];
    let refusal = check_point_order(&decreasing).expect_err("High is below Low");
    assert_eq!(
        refusal.0,
        "The operating points must increase in the order they are given: High at 0.09 is not above Low at 0.49."
    );

    // Equal values are not increasing either: the strict order names the point that repeats the value.
    let equal = [("Low", 0.3), ("Middle", 0.6), ("High", 0.6)];
    let refusal = check_point_order(&equal).expect_err("High repeats Middle");
    assert!(refusal.0.contains("High at 0.6 is not above Middle at 0.6"));
}

#[test]
fn the_order_names_the_first_point_out_of_order() {
    let points = [("A", 1.0), ("B", 3.0), ("C", 2.0), ("D", 1.0)];
    let refusal = check_point_order(&points).expect_err("C is below B");
    assert!(
        refusal.0.contains("C at 2 is not above B at 3"),
        "{}",
        refusal.0
    );
    assert!(!refusal.0.contains("D at"), "{}", refusal.0);
}

#[test]
fn a_value_of_the_scheduling_variable_must_be_finite() {
    let refusal =
        check_point_order(&[("Low", f64::NAN), ("High", 0.5)]).expect_err("NaN is not a value");
    assert_eq!(
        refusal.0,
        "The operating point Low needs a finite value of the scheduling variable."
    );
}

#[test]
fn a_tolerance_must_be_a_finite_number_above_zero() {
    assert!(check_tolerance(0.01).is_ok());
    for tolerance in [0.0, -1.0, f64::NAN, f64::INFINITY] {
        assert_eq!(
            check_tolerance(tolerance).expect_err("not above zero").0,
            "Give each operating point a tolerance above zero, in the scheduling variable's unit."
        );
    }
}

#[test]
fn the_span_of_a_gain_is_its_largest_minus_its_smallest_value() {
    assert_eq!(span_of(&[0.5, 0.25, 0.75]), 0.5);
    assert_eq!(span_of(&[2.0]), 0.0);
    assert_eq!(
        span_of(&[3.0, 3.0]),
        0.0,
        "a gain the same at every point has no span"
    );
    assert_eq!(span_of(&[]), 0.0);
}

#[test]
fn the_spans_of_the_two_gains_are_read_over_the_points() {
    assert_eq!(
        gain_spans(&[gains(2.0, 0.2), gains(3.0, 0.2)]),
        GainSpans {
            proportional: 1.0,
            integral: 0.0,
        }
    );
}

#[test]
fn the_table_pairs_each_point_with_its_own_gains() {
    let points = [
        table_point("Low", 0.09, 0.01),
        table_point("High", 0.49, 0.01),
    ];
    let values = table_values(&points, &[gains(2.0, 0.2), gains(3.0, 0.3)]).expect("the table");
    assert_eq!(values.len(), 4);
    assert_eq!(values["P1.KpLow"], 2.0);
    assert_eq!(values["P1.KiLow"], 0.2);
    assert_eq!(values["P1.KpHigh"], 3.0);
    assert_eq!(values["P1.KiHigh"], 0.3);
}

#[test]
fn the_table_refuses_a_gain_count_other_than_one_pair_per_point_and_a_shared_variable() {
    let points = [
        table_point("Low", 0.09, 0.01),
        table_point("High", 0.49, 0.01),
    ];
    assert!(table_values(&points, &[gains(2.0, 0.2)]).is_err());
    let mut shared = points.clone();
    shared[1].proportional_symbol = "P1.KpLow".to_owned();
    assert!(table_values(&shared, &[gains(2.0, 0.2), gains(3.0, 0.3)]).is_err());
}

#[test]
fn a_candidate_holds_the_whole_table_and_its_own_points_gains() {
    let points = [
        table_point("Low", 0.09, 0.01),
        table_point("High", 0.49, 0.01),
    ];
    let designs = [gains(2.0, 0.2), gains(3.0, 0.3)];
    let candidate =
        schedule_candidate(&points, &designs, 1, CandidateSource::Manual).expect("the candidate");
    assert!(!candidate.applied, "a candidate is never applied by Tune");
    assert_eq!(candidate.structure, Structure::PI);
    assert_eq!(candidate.gains, designs[1]);
    assert_eq!(candidate.values.len(), 4);
    assert_eq!(candidate.values["P1.KpLow"], 2.0);
    assert_eq!(candidate.values["P1.KiHigh"], 0.3);
    assert!(
        schedule_candidate(&points, &designs, 2, CandidateSource::Manual).is_err(),
        "an index past the table is refused"
    );
}

#[test]
fn identify_starts_only_when_the_scheduling_variable_is_within_the_tolerance() {
    // Binary-exact numbers, so that a difference of exactly the tolerance is exactly the tolerance.
    let low = table_point("Low", 0.5, 0.25);
    assert!(check_scheduling_value(0.5, &low, "m").is_ok());
    assert!(
        check_scheduling_value(0.75, &low, "m").is_ok(),
        "the tolerance itself is accepted"
    );
    assert!(check_scheduling_value(0.25, &low, "m").is_ok());
    // Decimal values: 0.10 − 0.09 is not exactly 0.01 in binary, and a reading at the tolerance is still accepted.
    let decimal = table_point("Low", 0.09, 0.01);
    assert!(check_scheduling_value(0.10, &decimal, "m").is_ok());

    let refusal = check_scheduling_value(0.8, &low, "m").expect_err("0.3 is above 0.25");
    assert!(
        refusal
            .0
            .starts_with("The scheduling variable reads 0.8 m, outside the tolerance of 0.25 m"),
        "{}",
        refusal.0
    );
    assert!(
        refusal.0.contains("operating point Low (0.5 m)"),
        "{}",
        refusal.0
    );
    assert!(
        check_scheduling_value(f64::NAN, &low, "m").is_err(),
        "a value that is not read is not within"
    );
}
