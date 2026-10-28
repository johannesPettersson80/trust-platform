//! The PID group's candidate (specification 35, C7): the values that Apply writes are each member's gains under the member's own symbols
//! and each decoupler entry under the program's symbol; a member without a gain symbol is refused by name; and the candidate is stored and
//! read back unchanged.

#![allow(clippy::float_cmp)] // The values are the numbers written, compared exactly as the candidate holds them.

use std::collections::BTreeMap;

use serde_json::json;

use super::*;
use crate::loop_definition::PidControllerGroup;
use crate::tuning::structures::pid_group::{DecentralizedMember, DecentralizedPair};

/// A two-member group whose members name their three gains in their own records.
fn group() -> PidControllerGroup {
    serde_json::from_value(json!({ "members": [
        { "instance": "P1.Zone1", "type": "Control.PID", "inputChannelId": "u1", "outputChannelId": "y1",
          "modeSymbol": "P1.Auto", "parameterSymbols": ["P1.Zone1Parameters.ProportionalGain", "P1.Zone1Parameters.IntegralGain", "P1.Zone1Parameters.DerivativeGain"] },
        { "instance": "P1.Zone2", "type": "Control.PID", "inputChannelId": "u2", "outputChannelId": "y2",
          "modeSymbol": "P1.Auto", "parameterSymbols": ["P1.Zone2Parameters.ProportionalGain", "P1.Zone2Parameters.IntegralGain", "P1.Zone2Parameters.DerivativeGain"] }
    ] }))
    .expect("the group")
}

/// A member's design with the given proportional and integral gains.
fn design(kp: f64, ki: f64) -> DecentralizedMember {
    DecentralizedMember {
        pair: DecentralizedPair {
            input_id: "u".to_owned(),
            output_id: "y".to_owned(),
        },
        path_gain: 0.225,
        relative_gain: 1.125,
        effective_gain: 0.2,
        time_constant_seconds: 8.0,
        delay_seconds: 0.0,
        gains: ParallelGains {
            proportional_gain: kp,
            integral_gain: ki,
            derivative_gain: 0.0,
        },
    }
}

#[test]
fn a_member_design_names_its_gain_symbols_for_the_page() {
    let member = member_design(&group().members[1], &design(3.0, 0.2)).expect("the member design");
    assert_eq!(
        member.proportional_symbol,
        "P1.Zone2Parameters.ProportionalGain"
    );
    assert_eq!(member.integral_symbol, "P1.Zone2Parameters.IntegralGain");
}

#[test]
fn each_member_writes_its_own_gains_and_a_derivative_of_zero() {
    let designs = [design(7.5, 0.94), design(3.0, 0.2)];
    let values = member_values(&group(), &designs).expect("the gains are named");
    let expected = BTreeMap::from([
        ("P1.Zone1Parameters.ProportionalGain".to_owned(), 7.5),
        ("P1.Zone1Parameters.IntegralGain".to_owned(), 0.94),
        ("P1.Zone1Parameters.DerivativeGain".to_owned(), 0.0),
        ("P1.Zone2Parameters.ProportionalGain".to_owned(), 3.0),
        ("P1.Zone2Parameters.IntegralGain".to_owned(), 0.2),
        ("P1.Zone2Parameters.DerivativeGain".to_owned(), 0.0),
    ]);
    assert_eq!(values, expected);
}

#[test]
fn a_member_without_one_of_its_gain_symbols_is_refused_by_its_instance() {
    let mut without_integral = group();
    without_integral.members[1]
        .parameter_symbols
        .retain(|symbol| !symbol.ends_with(".IntegralGain"));
    let refusal = member_values(&without_integral, &[design(1.0, 0.1), design(1.0, 0.1)])
        .expect_err("no integral gain");
    assert_eq!(refusal.message, "P1.Zone2 binds no IntegralGain.");
}

#[test]
fn each_decoupler_entry_is_written_under_the_symbol_the_program_names() {
    let symbols = vec![
        vec!["P1.D11".to_owned(), "P1.D12".to_owned()],
        vec!["P1.D21".to_owned(), "P1.D22".to_owned()],
    ];
    let matrix = vec![vec![1.1, -0.4], vec![-0.4, 1.1]];
    let values = decoupler_values(&symbols, &matrix);
    assert_eq!(values.len(), 4);
    assert_eq!(values["P1.D12"], -0.4);
    assert_eq!(values["P1.D21"], -0.4);
}

#[test]
fn a_candidate_with_a_value_that_is_not_finite_is_not_stored() {
    let mut candidate = GroupCandidate {
        applied: false,
        source: CandidateSource::Simc,
        members: Vec::new(),
        decoupler: None,
        values: BTreeMap::from([("P1.Zone1Parameters.ProportionalGain".to_owned(), 1.0)]),
    };
    assert!(candidate.is_finite());
    candidate
        .values
        .insert("P1.Zone2Parameters.IntegralGain".to_owned(), f64::NAN);
    assert!(!candidate.is_finite());
}

#[test]
fn a_candidate_is_stored_and_read_back_unchanged_and_names_no_unknown_field() {
    let candidate = GroupCandidate {
        applied: false,
        source: CandidateSource::Simc,
        members: vec![
            member_design(&group().members[0], &design(7.5, 0.94)).expect("the member design")
        ],
        decoupler: Some(GroupDecoupler {
            matrix: vec![vec![1.1, -0.4], vec![-0.4, 1.1]],
            input_channel_ids: vec!["u1".to_owned(), "u2".to_owned()],
            input_names: None,
            member_instances: vec!["P1.Zone1".to_owned(), "P1.Zone2".to_owned()],
            block: None,
        }),
        values: BTreeMap::from([("P1.D11".to_owned(), 1.1)]),
    };
    let text = serde_json::to_string(&candidate).expect("the candidate writes");
    let read: GroupCandidate = serde_json::from_str(&text).expect("the candidate reads back");
    assert_eq!(read, candidate);
    let value: serde_json::Value = serde_json::from_str(&text).expect("json");
    assert_eq!(
        value["members"][0]["proportionalSymbol"],
        "P1.Zone1Parameters.ProportionalGain"
    );
    assert_eq!(
        value["members"][0]["integralSymbol"],
        "P1.Zone1Parameters.IntegralGain"
    );
    let mut extra: serde_json::Value = serde_json::from_str(&text).expect("json");
    extra["unknown"] = json!(true);
    assert!(
        serde_json::from_value::<GroupCandidate>(extra).is_err(),
        "an unknown field is refused"
    );
}
