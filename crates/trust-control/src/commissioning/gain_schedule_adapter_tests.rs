//! Tests of the Apply and Restore of a gain schedule (specification 35, C10 Apply and Restore): the variables and values one transaction
//! writes for the fixed and the scheduled design, the refusals of a table that does not pair its gains, the read-back of a candidate's
//! values, and that Restore returns every point's gains. They also pin the transfer members that a schedule adds to a binding, their type,
//! unit and bounds from the record, the tracking's values at an entry and between entries, and the values Restore returns to.

#![allow(clippy::float_cmp)] // The gains are the exact values the test designs.

use std::collections::BTreeMap;

use super::{
    check_restore_mode, gains_of_table, is_table_variable, restore_values, schedule_members,
    schedule_origin, schedule_tracking_values, schedule_values, schedule_write, schedule_write_of,
    with_schedule_members,
};
use crate::commissioning::found_mode::FoundMode;
use crate::commissioning::transfer::{
    TransferBinding, TransferMember, TransferModeBinding, TransferValue, TransferValueType,
};
use crate::loop_definition::{ModeBinding, Parameter, ParameterBinding, RealType, SchedulePoint};
use crate::tuning::candidate::{CandidateSource, LawCandidate};
use crate::tuning::project_candidate::project_candidate;
use crate::tuning::project_seed::ParallelPidSeed;
use crate::tuning::structure::{ParallelGains, Structure};
use crate::tuning::structures::gain_schedule::{gains_per_point, ScheduleChoice};

fn point(name: &str, value: f64, proportional: &str, integral: &str) -> SchedulePoint {
    SchedulePoint {
        name: name.to_owned(),
        loop_id: format!("loop:{}", name.to_lowercase()),
        value,
        tolerance: 0.01,
        proportional_symbol: proportional.to_owned(),
        integral_symbol: integral.to_owned(),
    }
}

/// The tank's table: Low at 0.09 m and High at 0.49 m.
fn table() -> Vec<SchedulePoint> {
    vec![
        point("Low", 0.09, "P1.KpLow", "P1.KiLow"),
        point("High", 0.49, "P1.KpHigh", "P1.KiHigh"),
    ]
}

fn gains(proportional: f64, integral: f64) -> ParallelGains {
    ParallelGains {
        proportional_gain: proportional,
        integral_gain: integral,
        derivative_gain: 0.0,
    }
}

/// The values of the table's variables, in the order the test lists them.
fn written(values: &BTreeMap<String, f64>) -> Vec<(&str, f64)> {
    values
        .iter()
        .map(|(symbol, value)| (symbol.as_str(), *value))
        .collect()
}

/// The record of the tank's Low loop: its own gains, named as the setup names them, in the units of the table.
fn record(iec_type: RealType) -> ParameterBinding {
    ParameterBinding {
        loop_id: "loop:low".to_owned(),
        task_name: "MainTask".to_owned(),
        parameters: vec![
            parameter("P1.KpLow", "%/m", iec_type),
            parameter("P1.KiLow", "%/(m*s)", iec_type),
        ],
        mode: ModeBinding {
            symbol: "P1.AutoMode".to_owned(),
            manual_value: false,
        },
    }
}

/// A record field of the tank's gains, with the bounds that Tune may choose.
fn parameter(symbol: &str, unit: &str, iec_type: RealType) -> Parameter {
    Parameter {
        symbol: symbol.to_owned(),
        iec_type,
        unit: unit.to_owned(),
        convention: "gain".to_owned(),
        minimum: -1_000_000.0,
        maximum: 1_000_000.0,
    }
}

/// A transfer member with the bounds of the tank's gains.
fn member(symbol: &str, unit: &str, iec_type: TransferValueType) -> TransferMember {
    TransferMember {
        symbol: symbol.to_owned(),
        iec_type,
        unit: unit.to_owned(),
        minimum: -1_000_000.0,
        maximum: 1_000_000.0,
    }
}

/// The transfer binding of the Low loop before its schedule widens it: the setpoint and the Manual command are its states, its own gains
/// and the level are its guards, and the valve is its output.
fn own_binding() -> TransferBinding {
    TransferBinding {
        loop_id: "loop:low".to_owned(),
        task_name: "MainTask".to_owned(),
        mode: TransferModeBinding {
            symbol: "P1.AutoMode".to_owned(),
            manual_value: false,
        },
        additional_modes: Vec::new(),
        tracking_outputs: BTreeMap::new(),
        states: vec![
            member("P1.Setpoint", "m", TransferValueType::Lreal),
            member("P1.Manual", "%", TransferValueType::Lreal),
        ],
        guards: vec![
            member("P1.KpLow", "%/m", TransferValueType::Lreal),
            member("P1.KiLow", "%/(m*s)", TransferValueType::Lreal),
            member("P1.Level", "m", TransferValueType::Lreal),
        ],
        output: member("P1.Valve", "%", TransferValueType::Lreal),
        additional_outputs: Vec::new(),
    }
}

/// The table's four variables as a schedule's binding states them: each in the range 0 to 10, so that a midpoint is 5.
fn table_states() -> TransferBinding {
    let mut binding = own_binding();
    binding.states = ["P1.KpLow", "P1.KiLow", "P1.KpHigh", "P1.KiHigh"]
        .into_iter()
        .map(|symbol| TransferMember {
            minimum: 0.0,
            maximum: 10.0,
            ..member(symbol, "%/m", TransferValueType::Lreal)
        })
        .collect();
    binding
}

/// The entry values of the table's four variables, each distinct and inside the range 0 to 10 that `table_states` gives them.
fn entry_values() -> BTreeMap<String, f64> {
    BTreeMap::from([
        ("P1.KpLow".to_owned(), 2.0),
        ("P1.KiLow".to_owned(), 0.2),
        ("P1.KpHigh".to_owned(), 3.0),
        ("P1.KiHigh".to_owned(), 0.3),
    ])
}

/// The same value for each of the table's four variables.
fn uniform_table(value: f64) -> BTreeMap<String, f64> {
    ["P1.KpLow", "P1.KiLow", "P1.KpHigh", "P1.KiHigh"]
        .into_iter()
        .map(|symbol| (symbol.to_owned(), value))
        .collect()
}

fn names(members: &[TransferMember]) -> Vec<&str> {
    members
        .iter()
        .map(|member| member.symbol.as_str())
        .collect()
}

#[test]
fn the_fixed_design_applies_the_first_points_gains_at_every_point() {
    let designs = [gains(2.0, 0.2), gains(3.0, 0.3)];
    let per_point = gains_per_point(ScheduleChoice::Fixed, &designs);
    let values = schedule_values(&table(), &per_point).expect("the fixed design's write set");
    assert_eq!(
        written(&values),
        vec![
            ("P1.KiHigh", 0.2),
            ("P1.KiLow", 0.2),
            ("P1.KpHigh", 2.0),
            ("P1.KpLow", 2.0),
        ]
    );
}

#[test]
fn the_scheduled_design_applies_each_points_own_gains() {
    let designs = [gains(2.0, 0.2), gains(3.0, 0.3)];
    let per_point = gains_per_point(ScheduleChoice::Scheduled, &designs);
    let values = schedule_values(&table(), &per_point).expect("the scheduled write set");
    assert_eq!(
        written(&values),
        vec![
            ("P1.KiHigh", 0.3),
            ("P1.KiLow", 0.2),
            ("P1.KpHigh", 3.0),
            ("P1.KpLow", 2.0),
        ]
    );
}

#[test]
fn an_apply_needs_one_pair_of_gains_for_each_point() {
    let failure =
        schedule_values(&table(), &[gains(2.0, 0.2)]).expect_err("one pair is not one per point");
    assert_eq!(
        failure.message,
        "A schedule needs one pair of gains for each operating point."
    );
}

#[test]
fn an_apply_refuses_a_variable_that_two_gains_name() {
    let shared = vec![
        point("Low", 0.09, "P1.KpLow", "P1.KiLow"),
        point("High", 0.49, "P1.KpLow", "P1.KiHigh"),
    ];
    let failure = schedule_values(&shared, &[gains(2.0, 0.2), gains(3.0, 0.3)])
        .expect_err("P1.KpLow is written twice");
    assert_eq!(
        failure.message,
        "Two gains of the schedule name the same variable, so one transaction cannot write both."
    );
}

#[test]
fn the_gains_of_each_point_read_back_from_the_values_of_the_table() {
    let per_point = gains_per_point(
        ScheduleChoice::Scheduled,
        &[gains(2.0, 0.2), gains(3.0, 0.3)],
    );
    let values = schedule_values(&table(), &per_point).expect("the write set");
    let read = gains_of_table(&table(), &values).expect("every value is there");
    assert_eq!(read, per_point);
}

#[test]
fn a_candidate_without_one_of_the_table_variables_is_refused_by_name() {
    let values = BTreeMap::from([
        ("P1.KpLow".to_owned(), 2.0),
        ("P1.KiLow".to_owned(), 0.2),
        ("P1.KpHigh".to_owned(), 3.0),
    ]);
    let failure = gains_of_table(&table(), &values).expect_err("KiHigh is missing");
    assert_eq!(
        failure.message,
        "The schedule's candidate holds no value for P1.KiHigh."
    );
}

#[test]
fn restore_returns_every_points_gains_and_nothing_else() {
    let previous = BTreeMap::from([
        ("P1.KpLow".to_owned(), 0.5),
        ("P1.KiLow".to_owned(), 0.05),
        ("P1.KpHigh".to_owned(), 0.6),
        ("P1.KiHigh".to_owned(), 0.06),
        ("P1.Other".to_owned(), 9.0),
    ]);
    let restored = restore_values(&table(), &previous).expect("every gain was known");
    assert_eq!(
        written(&restored),
        vec![
            ("P1.KiHigh", 0.06),
            ("P1.KiLow", 0.05),
            ("P1.KpHigh", 0.6),
            ("P1.KpLow", 0.5),
        ]
    );
}

#[test]
fn an_apply_and_its_restore_return_the_gains_the_controller_held() {
    let before = BTreeMap::from([
        ("P1.KpLow".to_owned(), 0.5),
        ("P1.KiLow".to_owned(), 0.05),
        ("P1.KpHigh".to_owned(), 0.6),
        ("P1.KiHigh".to_owned(), 0.06),
    ]);
    let applied = schedule_values(
        &table(),
        &gains_per_point(
            ScheduleChoice::Scheduled,
            &[gains(2.0, 0.2), gains(3.0, 0.3)],
        ),
    )
    .expect("the write set");
    assert_ne!(applied, before, "the transaction changes the gains");
    assert_eq!(restore_values(&table(), &before).expect("restore"), before);
}

#[test]
fn restore_refuses_a_point_whose_previous_gain_is_not_known() {
    let previous = BTreeMap::from([
        ("P1.KpLow".to_owned(), 0.5),
        ("P1.KiLow".to_owned(), 0.05),
        ("P1.KpHigh".to_owned(), 0.6),
    ]);
    let failure = restore_values(&table(), &previous).expect_err("KiHigh is not known");
    assert_eq!(
        failure.message,
        "The value of P1.KiHigh before Apply is not known, so Restore cannot return it."
    );
}

#[test]
fn the_members_of_the_table_follow_the_record_in_table_order_with_its_units_and_bounds() {
    let members =
        schedule_members(&table(), &record(RealType::Lreal)).expect("the record names the gains");
    assert_eq!(
        members,
        vec![
            member("P1.KpLow", "%/m", TransferValueType::Lreal),
            member("P1.KiLow", "%/(m*s)", TransferValueType::Lreal),
            member("P1.KpHigh", "%/m", TransferValueType::Lreal),
            member("P1.KiHigh", "%/(m*s)", TransferValueType::Lreal),
        ]
    );
}

#[test]
fn the_members_of_a_table_keep_the_real_type_of_a_real_record() {
    let members =
        schedule_members(&table(), &record(RealType::Real)).expect("the record names the gains");
    assert!(members
        .iter()
        .all(|member| member.iec_type == TransferValueType::Real));
}

#[test]
fn a_record_that_names_none_of_the_table_s_gains_is_refused() {
    let mut other = record(RealType::Lreal);
    other.parameters.clear();
    let failure = schedule_members(&table(), &other).expect_err("no gain of the table");
    assert_eq!(
        failure.message,
        "The parameter record of the loop holds no gain of the schedule's table."
    );
}

#[test]
fn the_schedule_states_the_table_after_the_own_states_and_drops_the_own_gains_from_the_guards() {
    let widened = with_schedule_members(&table(), &record(RealType::Lreal), own_binding())
        .expect("the table fits the record");
    assert_eq!(
        names(&widened.states),
        vec![
            "P1.Setpoint",
            "P1.Manual",
            "P1.KpLow",
            "P1.KiLow",
            "P1.KpHigh",
            "P1.KiHigh",
        ]
    );
    assert_eq!(names(&widened.guards), vec!["P1.Level"]);
}

#[test]
fn a_binding_that_already_states_a_table_variable_is_refused() {
    let mut binding = own_binding();
    binding
        .states
        .push(member("P1.KpHigh", "%/m", TransferValueType::Lreal));
    let failure = with_schedule_members(&table(), &record(RealType::Lreal), binding)
        .expect_err("KpHigh is already a state");
    assert_eq!(
        failure.message,
        "The transfer binding already states P1.KpHigh, a variable of the schedule's table."
    );
}

#[test]
fn the_apply_writes_the_table_values_of_the_candidate_and_nothing_else() {
    let per_point = gains_per_point(
        ScheduleChoice::Scheduled,
        &[gains(2.0, 0.2), gains(3.0, 0.3)],
    );
    let candidate = schedule_values(&table(), &per_point).expect("the candidate's table");
    let mut with_other_value = candidate.clone();
    with_other_value.insert("P1.Other".to_owned(), 9.0);
    assert_eq!(
        schedule_write(&table(), &with_other_value).expect("the table's values"),
        candidate
    );
}

#[test]
fn the_tracking_gives_each_table_variable_its_entry_value_or_the_middle_of_its_range() {
    let binding = table_states();
    assert_eq!(
        schedule_tracking_values(&table(), &binding, Some(&entry_values()))
            .expect("every entry value is given"),
        entry_values()
    );
    assert_eq!(
        schedule_tracking_values(&table(), &binding, None).expect("the middle of each range"),
        uniform_table(5.0)
    );
}

#[test]
fn a_real_variable_takes_its_entry_value_rounded_to_single_precision() {
    let mut binding = table_states();
    for state in &mut binding.states {
        state.iec_type = TransferValueType::Real;
    }
    let entry = uniform_table(0.1);
    assert_eq!(
        schedule_tracking_values(&table(), &binding, Some(&entry)).expect("the entry values"),
        uniform_table(f64::from(0.1_f32))
    );
}

#[test]
fn restore_returns_each_table_variable_to_the_value_it_held_before_the_apply() {
    let observed = BTreeMap::from([
        ("P1.KpLow".to_owned(), TransferValue::Number(0.5)),
        ("P1.KiLow".to_owned(), TransferValue::Number(0.05)),
        ("P1.KpHigh".to_owned(), TransferValue::Number(0.6)),
        ("P1.KiHigh".to_owned(), TransferValue::Number(0.06)),
        ("P1.Level".to_owned(), TransferValue::Number(0.3)),
    ]);
    assert_eq!(
        schedule_origin(&table(), &observed).expect("every table variable was reported"),
        BTreeMap::from([
            ("P1.KpLow".to_owned(), 0.5),
            ("P1.KiLow".to_owned(), 0.05),
            ("P1.KpHigh".to_owned(), 0.6),
            ("P1.KiHigh".to_owned(), 0.06),
        ])
    );
}

#[test]
fn restore_refuses_a_table_variable_that_the_review_did_not_report_as_a_number() {
    let observed = BTreeMap::from([
        ("P1.KpLow".to_owned(), TransferValue::Number(0.5)),
        ("P1.KiLow".to_owned(), TransferValue::Number(0.05)),
        ("P1.KpHigh".to_owned(), TransferValue::Number(0.6)),
        ("P1.KiHigh".to_owned(), TransferValue::Boolean(true)),
    ]);
    let failure = schedule_origin(&table(), &observed).expect_err("KiHigh is a BOOL");
    assert_eq!(
        failure.message,
        "The value of P1.KiHigh before Apply is not known, so Restore cannot return it."
    );
}

/// The candidate of the tank's Low point as a program's own PI law holds it, with the given values of the other points in its table.
fn law_candidate(other_values: &[(&str, f64)]) -> LawCandidate {
    let seed = ParallelPidSeed {
        family: Structure::PI,
        proportional_symbol: "P1.KpLow".to_owned(),
        integral_symbol: Some("P1.KiLow".to_owned()),
        derivative_symbol: None,
    };
    let mut candidate = project_candidate(&seed, CandidateSource::Simc, gains(2.0, 0.2));
    candidate.values.extend(
        other_values
            .iter()
            .map(|(symbol, value)| ((*symbol).to_owned(), *value)),
    );
    LawCandidate::Project(candidate)
}

#[test]
fn the_table_is_known_by_the_proportional_and_integral_variables_of_its_points() {
    assert!(is_table_variable(&table(), "P1.KiHigh"));
    assert!(!is_table_variable(&table(), "P1.Level"));
}

#[test]
fn a_schedule_candidate_writes_the_table_and_no_other_value() {
    let candidate = law_candidate(&[("P1.KpHigh", 3.0), ("P1.KiHigh", 0.3), ("P1.Other", 9.0)]);
    assert_eq!(
        schedule_write_of(Some(&table()), &candidate).expect("the candidate holds the table"),
        Some(BTreeMap::from([
            ("P1.KiHigh".to_owned(), 0.3),
            ("P1.KiLow".to_owned(), 0.2),
            ("P1.KpHigh".to_owned(), 3.0),
            ("P1.KpLow".to_owned(), 2.0),
        ]))
    );
}

#[test]
fn a_loop_without_a_schedule_writes_no_table() {
    let candidate = law_candidate(&[]);
    assert_eq!(
        schedule_write_of(None, &candidate).expect("no schedule, no table"),
        None
    );
}

#[test]
fn a_schedule_candidate_without_one_table_value_is_refused_by_name() {
    let candidate = law_candidate(&[("P1.KpHigh", 3.0)]);
    let failure = schedule_write_of(Some(&table()), &candidate).expect_err("KiHigh is missing");
    assert_eq!(
        failure.message,
        "The schedule's candidate holds no value for P1.KiHigh."
    );
}

#[test]
fn a_schedule_restores_only_into_auto() {
    let origin = BTreeMap::from([("P1.KpLow".to_owned(), 0.5)]);
    for found in [Some(FoundMode::Manual), None] {
        let failure = check_restore_mode(Some(&origin), found)
            .expect_err("a schedule is not restored into Manual");
        assert_eq!(
            failure.message,
            "The gains of a gain schedule can be restored only into Auto."
        );
    }
    check_restore_mode(Some(&origin), Some(FoundMode::Auto)).expect("Auto is restored");
    check_restore_mode(None, Some(FoundMode::Manual)).expect("a loop without a schedule");
}
