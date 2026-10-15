//! Tests of the schedule block of a `gain-scheduled` project law (specification 35, C10): the block decodes as the setup writes it, and
//! the rules of the table, the family and the seed refuse what C10 refuses.

#![allow(clippy::float_cmp)] // The operating points are the exact values the setup wrote.

use serde_json::{json, Map, Value};

use super::{check_block, schedule_of, GainSchedule, FAMILY};
use crate::loop_definition::ProjectSeedBlock;
use crate::tuning::project_seed::ParallelPidSeed;
use crate::tuning::structure::Structure;

/// The block of the tank's two points, as the setup writes it for the `Low` loop (C10).
fn tank_block() -> Value {
    json!({
        "parameterRecord": "P1.Parameters",
        "scheduling": { "symbol": "P1.LevelM", "unit": "m", "iecType": "LREAL" },
        "point": "Low",
        "points": [
            { "name": "Low", "loopId": "loop:tank-level-low", "value": 0.09, "tolerance": 0.01,
              "proportionalSymbol": "P1.KpLow", "integralSymbol": "P1.KiLow" },
            { "name": "High", "loopId": "loop:tank-level-high", "value": 0.49, "tolerance": 0.01,
              "proportionalSymbol": "P1.KpHigh", "integralSymbol": "P1.KiHigh" }
        ]
    })
}

/// The tank's block with one field changed or one point replaced.
fn changed(change: impl FnOnce(&mut Value)) -> Value {
    let mut block = tank_block();
    change(&mut block);
    block
}

fn pi_seed(proportional: &str, integral: &str) -> ProjectSeedBlock {
    ProjectSeedBlock::ParallelPid(ParallelPidSeed {
        family: Structure::PI,
        proportional_symbol: proportional.to_owned(),
        integral_symbol: Some(integral.to_owned()),
        derivative_symbol: None,
    })
}

#[test]
fn a_schedule_block_decodes_and_writes_back_as_the_setup_writes_it() {
    let schedule = GainSchedule::from_value(&tank_block()).expect("the tank's table is a schedule");
    assert_eq!(schedule.point, "Low");
    assert_eq!(schedule.scheduling.symbol, "P1.LevelM");
    assert_eq!(schedule.own_point().map(|point| point.value), Some(0.09));
    assert_eq!(schedule.to_value(), tank_block());
}

#[test]
fn a_schedule_needs_two_to_four_operating_points() {
    let one = changed(|block| {
        block["points"].as_array_mut().expect("points").truncate(1);
    });
    let text = GainSchedule::from_value(&one).expect_err("one point is not a schedule");
    assert!(text.contains("two to four operating points"), "{text}");
}

#[test]
fn the_values_must_increase_in_the_order_the_points_are_given() {
    let reversed = changed(|block| {
        block["points"][1]["value"] = json!(0.05);
    });
    let text = GainSchedule::from_value(&reversed).expect_err("High is below Low");
    assert!(
        text.contains("High at 0.05 is not above Low at 0.09"),
        "{text}"
    );
}

#[test]
fn a_tolerance_must_be_above_zero() {
    let zero = changed(|block| {
        block["points"][0]["tolerance"] = json!(0.0);
    });
    let text = GainSchedule::from_value(&zero).expect_err("a tolerance of zero is not one");
    assert!(text.contains("tolerance above zero"), "{text}");
}

#[test]
fn the_point_this_loop_tunes_must_be_one_of_the_table() {
    let unknown = changed(|block| {
        block["point"] = json!("Middle");
    });
    let text = GainSchedule::from_value(&unknown).expect_err("Middle is not in the table");
    assert!(
        text.contains("Middle is not one of its operating points"),
        "{text}"
    );
}

#[test]
fn the_record_and_the_scheduling_variable_are_named() {
    let no_record = changed(|block| {
        block["parameterRecord"] = json!("  ");
    });
    assert!(GainSchedule::from_value(&no_record).is_err());
    let unknown_field = changed(|block| {
        block["unknown"] = json!(1);
    });
    assert!(
        GainSchedule::from_value(&unknown_field).is_err(),
        "a field the block does not declare is refused"
    );
}

#[test]
fn the_schedule_is_read_only_from_a_project_block_that_declares_one() {
    let mut object = Map::new();
    assert_eq!(schedule_of(&object), Ok(None));
    object.insert("schedule".to_owned(), tank_block());
    let schedule = schedule_of(&object)
        .expect("the block reads")
        .expect("the block is present");
    assert_eq!(schedule.points.len(), 2);
}

#[test]
fn the_gain_scheduled_family_needs_its_schedule_and_its_own_pi_seed() {
    let schedule = GainSchedule::from_value(&tank_block()).expect("the tank's table");
    let own = pi_seed("P1.KpLow", "P1.KiLow");
    assert_eq!(FAMILY, "gain-scheduled");
    assert!(check_block(FAMILY, Some(&schedule), Some(&own)).is_ok());

    let missing = check_block(FAMILY, None, Some(&own)).expect_err("no schedule");
    assert!(
        missing.contains("needs its controller.project.schedule"),
        "{missing}"
    );

    let other_point = pi_seed("P1.KpHigh", "P1.KiHigh");
    let seed = check_block(FAMILY, Some(&schedule), Some(&other_point))
        .expect_err("another point's gains");
    assert!(seed.contains("own operating point"), "{seed}");

    let proportional_only = ProjectSeedBlock::ParallelPid(ParallelPidSeed {
        family: Structure::P,
        proportional_symbol: "P1.KpLow".to_owned(),
        integral_symbol: None,
        derivative_symbol: None,
    });
    assert!(check_block(FAMILY, Some(&schedule), Some(&proportional_only)).is_err());
    assert!(
        check_block(FAMILY, Some(&schedule), None).is_err(),
        "no seed"
    );
}

#[test]
fn a_schedule_belongs_to_the_gain_scheduled_family_only() {
    let schedule = GainSchedule::from_value(&tank_block()).expect("the tank's table");
    let own = pi_seed("P1.KpLow", "P1.KiLow");
    let text = check_block("PI", Some(&schedule), Some(&own)).expect_err("a schedule of family PI");
    assert!(
        text.contains("belongs to the gain-scheduled family"),
        "{text}"
    );
    assert!(
        check_block("PI", None, Some(&own)).is_ok(),
        "a PI law without a schedule is another law"
    );
}
