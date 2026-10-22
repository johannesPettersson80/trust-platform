//! Tests of the PID group (specification 35, C7), ported from `control-studio-decentralized-pid.test.ts` and `control-studio-decoupler.test.ts`:
//! each member's SIMC PI on its own path with the gain it sees when the other loops are closed, the refusal of a negative relative gain
//! with the pairing to use, and the decoupler `D = K⁻¹·diag(K)` through which the members' relative gains are one.
//!
//! The members' PIs are SIMC closed forms, so these tests need no prediction and make none (see `fixtures::NoSupport`).

#![allow(clippy::float_cmp)] // The members' delays are exact zeros by the reduction's clamp, and the derivative gains are exact zeros.

use trust_control_math::identify::gain::ControllerAction;
use trust_control_math::identify::ParameterInterval;

use super::*;
use crate::tuning::objective::TuningObjective;
use crate::tuning::prediction::RecordShaping;
use crate::tuning::requirements::TuningRequirements;
use crate::tuning::shortfall::{GainLimits, OutputLimits};
use crate::tuning::structures::fixtures::{
    crossed_zones_model, decoupler_zones_model, requirements, zones_model, NoSupport,
};
use crate::tuning::structures::path::LoopFacts;

fn pairs() -> Vec<DecentralizedPair> {
    vec![
        DecentralizedPair {
            input_id: "u1".to_owned(),
            output_id: "y1".to_owned(),
        },
        DecentralizedPair {
            input_id: "u2".to_owned(),
            output_id: "y2".to_owned(),
        },
    ]
}

/// The loop's facts of the zones: a 100 ms task, the manipulated variable's range, the units, and no bounds or rate limits declared.
fn loop_facts(action: ControllerAction) -> LoopFacts {
    LoopFacts {
        action,
        task_period_ms: 100.0,
        gain_limits: GainLimits::default(),
        output_limits: OutputLimits {
            minimum: 0.0,
            maximum: 100.0,
        },
        rate_limits: ParameterInterval {
            minimum: -1_000_000.0,
            maximum: 1_000_000.0,
        },
        shaping: RecordShaping::default(),
        process_unit: "degC".to_owned(),
        manipulated_unit: "%".to_owned(),
    }
}

/// The requirements of the zones' test with the declared response `objective`, or with none (Automatic, More robust).
fn record_with(objective: Option<TuningObjective>) -> TuningRequirements {
    TuningRequirements {
        tuning_objective: objective,
        ..requirements(1.0, 0.3, 0.1, 40.0, 80.0)
    }
}

/// The group's tuning of a test: the facts and the record, with the support that predicts nothing, so each member takes the declared
/// response's formula (C7).
fn tuning<'a>(facts: &'a LoopFacts, record: &'a TuningRequirements) -> GroupTuning<'a> {
    GroupTuning {
        facts,
        support: &NoSupport,
        requirements: record,
    }
}

/// Whether two values agree to a relative tolerance.
fn close(actual: f64, expected: f64, relative: f64) -> bool {
    (actual - expected).abs() <= relative * expected.abs()
}

#[test]
fn each_member_is_designed_by_simc_on_its_own_path_with_the_gain_it_sees_with_the_others_closed() {
    let facts = loop_facts(ControllerAction::Direct);
    let record = record_with(Some(TuningObjective::Balanced));
    let design = design_decentralized_pi(&zones_model(), &pairs(), tuning(&facts, &record))
        .expect("the zones' gains are nonsingular");
    assert_eq!(design.members.len(), 2);
    for member in &design.members {
        // The member's own path has the gain 0.225 °C/% and the relative gain 1.125.
        assert!(
            (member.path_gain - 0.225).abs() < 1e-3,
            "own-path gain {}",
            member.path_gain
        );
        assert!(
            (member.relative_gain - 1.125).abs() < 1e-3,
            "relative gain {}",
            member.relative_gain
        );
        assert!(
            (member.effective_gain - member.path_gain / member.relative_gain).abs() < 1e-12,
            "Kᵢᵢ/λᵢᵢ"
        );
        // y(t) = 0.225 − 0.15·e^(−0.1t) − 0.075·e^(−0.2t): the two-point reduction gives a negative delay, so θ = 0 and τ = t63.
        assert_eq!(member.delay_seconds, 0.0);
        assert!(
            (member.time_constant_seconds - 7.97).abs() < 0.1,
            "time constant {}",
            member.time_constant_seconds
        );
        // Balanced SIMC: λ = 2·max(θ, τ/3), Kp = τ/(K(λ + θ)), Ti = min(τ, 4(λ + θ)).
        let closed_loop = 2.0 * member.delay_seconds.max(member.time_constant_seconds / 3.0);
        let horizon = closed_loop + member.delay_seconds;
        let kp = member.time_constant_seconds / (member.effective_gain * horizon);
        let ti = member.time_constant_seconds.min(4.0 * horizon);
        assert!(
            close(member.gains.proportional_gain, kp, 1e-7),
            "SIMC Kp {} ≠ {kp}",
            member.gains.proportional_gain
        );
        assert!(close(member.gains.integral_gain, kp / ti, 1e-7), "SIMC Ti");
        assert_eq!(member.gains.derivative_gain, 0.0);
    }
}

#[test]
fn a_pairing_with_a_negative_relative_gain_is_refused_and_the_pairing_to_use_is_named() {
    // K = [[0.2, 0.5], [0.5, 0.2]]: λ₁₁ = 1/(1 − 0.25/0.04) ≈ −0.19, and the crossed pairing has λ ≈ 1.19.
    let facts = loop_facts(ControllerAction::Direct);
    let record = record_with(Some(TuningObjective::Balanced));
    let refusal =
        design_decentralized_pi(&crossed_zones_model(), &pairs(), tuning(&facts, &record))
            .expect_err("the direct pairing has a negative relative gain");
    assert!(
        refusal
            .sentence()
            .contains("The relative gain of u1 → y1 is -0.19"),
        "{}",
        refusal.sentence()
    );
    assert!(
        refusal
            .sentence()
            .contains("Pair u1 → y2 and u2 → y1 instead."),
        "{}",
        refusal.sentence()
    );
    assert!(
        matches!(refusal, GroupRefusal::Model(_)),
        "a pairing refusal is the model's, not a requirement's"
    );
}

#[test]
fn the_decoupler_is_k_inverse_times_diag_k_and_the_members_are_designed_through_it() {
    let members = vec![
        DecouplerMember {
            instance: "P1.Zone1".to_owned(),
            input_channel_id: "P1.Heater1Pct".to_owned(),
            output_channel_id: "P1.Zone1C".to_owned(),
        },
        DecouplerMember {
            instance: "P1.Zone2".to_owned(),
            input_channel_id: "P1.Heater2Pct".to_owned(),
            output_channel_id: "P1.Zone2C".to_owned(),
        },
    ];
    let facts = loop_facts(ControllerAction::Direct);
    let record = record_with(None);
    let design =
        design_decoupled_group(&decoupler_zones_model(), &members, tuning(&facts, &record))
            .expect("the zones' gains are nonsingular");

    // With k = 0.01/(0.13² − 0.04²), K = [[0.13k, 0.04k], [0.04k, 0.13k]] and its inverse is K⁻¹ = 100·[[0.13, −0.04], [−0.04, 0.13]],
    // so D = K⁻¹·diag(K) = [[13·K₁₁, −4·K₁₁], [−4·K₁₁, 13·K₁₁]], since K₂₂ = K₁₁.
    let k = 0.01 / (0.13 * 0.13 - 0.04 * 0.04);
    let k11 = 0.13 * k;
    let expected = [[13.0 * k11, -4.0 * k11], [-4.0 * k11, 13.0 * k11]];
    for (row, values) in expected.iter().enumerate() {
        for (column, value) in values.iter().enumerate() {
            assert!(
                (design.matrix[row][column] - value).abs() < 1e-4,
                "D[{row}][{column}] = {} vs {value}",
                design.matrix[row][column]
            );
        }
    }

    for member in &design.members {
        assert!(
            member.gains.proportional_gain > 0.0 && member.gains.integral_gain > 0.0,
            "{:?}",
            member.gains
        );
        // Through the decoupler, each member's relative gain is one (C7).
        assert!(
            (member.relative_gain - 1.0).abs() < 1e-3,
            "relative gain {}",
            member.relative_gain
        );
    }

    // K·D is diagonal: each member moves only its own measurement in steady state.
    let k01 = 0.04 * k;
    let off_diagonal_upper = k11 * design.matrix[0][1] + k01 * design.matrix[1][1];
    let off_diagonal_lower = k01 * design.matrix[0][0] + k11 * design.matrix[1][0];
    assert!(
        off_diagonal_upper.abs() < 1e-9 && off_diagonal_lower.abs() < 1e-9,
        "K·D is not diagonal"
    );

    assert_eq!(
        design.member_ids,
        vec!["P1.Zone1".to_owned(), "P1.Zone2".to_owned()]
    );
    assert_eq!(
        design.input_ids,
        vec!["P1.Heater1Pct".to_owned(), "P1.Heater2Pct".to_owned()]
    );
}

#[test]
fn a_decoupler_needs_members() {
    let facts = loop_facts(ControllerAction::Direct);
    let record = record_with(None);
    let refusal = design_decoupled_group(&decoupler_zones_model(), &[], tuning(&facts, &record))
        .expect_err("a decoupler has one row and one column per member");
    assert_eq!(refusal.sentence(), texts::DECOUPLER_MEMBERS);
    assert_eq!(
        refusal,
        GroupRefusal::Model(Refusal::new(texts::DECOUPLER_MEMBERS))
    );
}

#[test]
fn a_two_member_decoupler_is_configured_as_static_decoupler_2x2_and_its_inverse_is_d() {
    // G = diag(K)⁻¹·K = [[1, K₁₂/K₁₁], [K₂₁/K₂₂, 1]] = [[1, 0.04/0.13], [0.04/0.13, 1]]; G⁻¹ = D (spec 37 §4.2; C7).
    let members = decoupler_members();
    let facts = loop_facts(ControllerAction::Direct);
    let record = record_with(None);
    let design =
        design_decoupled_group(&decoupler_zones_model(), &members, tuning(&facts, &record))
            .expect("the zones' gains are nonsingular");
    let block = design
        .block
        .expect("a two-member group has the block's configuration");
    let ratio = 0.04 / 0.13;
    assert!((block.gain[0][1] - ratio).abs() < 1e-12 && (block.gain[1][0] - ratio).abs() < 1e-12);
    assert!((block.gain[0][0] - 1.0).abs() < 1e-12 && (block.gain[1][1] - 1.0).abs() < 1e-12);
    for (row, values) in block.inverse.iter().enumerate() {
        for (column, value) in values.iter().enumerate() {
            assert!(
                (value - design.matrix[row][column]).abs() < 1e-9,
                "G⁻¹[{row}][{column}] = {value}"
            );
        }
    }
    // The relative determinant is (1 − ratio²) over (1 + ratio)², which the block compares with its tolerance.
    let expected = (1.0 - ratio * ratio) / ((1.0 + ratio) * (1.0 + ratio));
    assert!(
        (block.relative_determinant - expected).abs() < 1e-12,
        "{}",
        block.relative_determinant
    );
}

#[test]
fn the_block_refuses_a_gain_matrix_it_cannot_configure_with_its_reason() {
    // K = [[1, 1], [1, 1]] is singular: G = [[1, 1], [1, 1]] has no determinant (the block's rule, spec 37 §4.2).
    let refusal =
        static_decoupler_2x2(&vec![vec![1.0, 1.0], vec![1.0, 1.0]]).expect_err("G is singular");
    assert!(
        refusal.0.contains("Control.StaticDecoupler2x2 refuses"),
        "{}",
        refusal.0
    );
}

#[test]
fn an_unspecified_objective_designs_for_more_robust_as_automatic_does() {
    // C7 and D-13: a group with no declared response takes Automatic's objective, More robust.
    let members = decoupler_members();
    let facts = loop_facts(ControllerAction::Direct);
    let unspecified_record = record_with(None);
    let robust_record = record_with(Some(TuningObjective::Robust));
    let unspecified = design_decoupled_group(
        &decoupler_zones_model(),
        &members,
        tuning(&facts, &unspecified_record),
    )
    .expect("the zones' gains are nonsingular");
    let robust = design_decoupled_group(
        &decoupler_zones_model(),
        &members,
        tuning(&facts, &robust_record),
    )
    .expect("the zones' gains are nonsingular");
    assert_eq!(unspecified.members, robust.members);
}

/// The two members of the zones' decoupler: each heater drives its own zone.
fn decoupler_members() -> Vec<DecouplerMember> {
    vec![
        DecouplerMember {
            instance: "P1.Zone1".to_owned(),
            input_channel_id: "P1.Heater1Pct".to_owned(),
            output_channel_id: "P1.Zone1C".to_owned(),
        },
        DecouplerMember {
            instance: "P1.Zone2".to_owned(),
            input_channel_id: "P1.Heater2Pct".to_owned(),
            output_channel_id: "P1.Zone2C".to_owned(),
        },
    ]
}
