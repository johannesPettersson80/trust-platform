//! Tests of the words of a Tune answer (the Tune page specification, "Result and plot"): the setting rows of a PID law, the labels that
//! a seed and a group member give, the margin words and the outcome sentence.

use std::collections::BTreeMap;

use trust_control_math::margins::MarginValue;

use super::{
    group_setting_symbols, horizon_policy_line, horizon_stages_heading, manual_setting_label,
    margin_word, owned_label, readable, seeded_labels, setting_labels, SettingLabel,
};
use crate::loop_definition::{
    ControllerLaw, LoopDefinition, Parameter, PidControllerGroup, PidGroupMember, ProjectSeedBlock,
};
use crate::messages::CandidateOutcome;
use crate::tuning::project_seed::{
    CascadeGains, CascadePidSeed, FeedforwardPidSeed, LeadLagSymbols, ParallelPidSeed,
    SmithPidSeed, TrajectoryFeedforwardSeed,
};
use crate::tuning::state_controller::design::{HorizonPolicy, StateHorizon};
use crate::tuning::structure::Structure;

/// The gains of the commissioning example, which its parameter binding names in the order Kp, Ki and Kd.
const KP: &str = "P1.Parameters.ProportionalGain";
const KI: &str = "P1.Parameters.IntegralGain";
const KD: &str = "P1.Parameters.DerivativeGain";

/// The shipped `Control.PID` loop of the commissioning example.
fn pid_loop() -> LoopDefinition {
    LoopDefinition::decode(include_str!(
        "../../../../examples/control-pid-commissioning/control-loop.json"
    ))
    .expect("the commissioning example's loop")
}

/// The three gains of the example, all finite.
fn all_gains() -> BTreeMap<String, f64> {
    BTreeMap::from([
        (KP.to_owned(), 2.0),
        (KI.to_owned(), 0.5),
        (KD.to_owned(), 0.1),
    ])
}

/// The unit the example's parameter binding declares for each of its three gains (`control-loop.json`, `parameterBinding`): the row of a
/// setting carries it, so the page reads no unit from the loop.
fn unit_of(symbol: &str) -> &'static str {
    match symbol {
        KP => "%/\u{b0}C",
        KI => "%/(\u{b0}C*s)",
        KD => "%*s/\u{b0}C",
        other => panic!("the example binds no gain {other}"),
    }
}

/// The rows that the pairs of a symbol and a label name, in the order given, each with the unit of its symbol.
fn rows(pairs: &[(&str, &str)]) -> Vec<SettingLabel> {
    pairs
        .iter()
        .map(|(symbol, label)| SettingLabel {
            symbol: (*symbol).to_owned(),
            label: (*label).to_owned(),
            unit: Some(unit_of(symbol).to_owned()),
        })
        .collect()
}

/// The label map that the pairs of a symbol and a label give.
fn labels(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
    pairs
        .iter()
        .map(|(symbol, label)| ((*symbol).to_owned(), (*label).to_owned()))
        .collect()
}

fn cascade_gains(
    instance: &str,
    proportional: &str,
    integral: &str,
    derivative: &str,
) -> CascadeGains {
    CascadeGains {
        instance: instance.to_owned(),
        proportional_symbol: proportional.to_owned(),
        integral_symbol: integral.to_owned(),
        derivative_symbol: derivative.to_owned(),
    }
}

fn parallel(
    family: Structure,
    proportional: &str,
    integral: Option<&str>,
    derivative: Option<&str>,
) -> ParallelPidSeed {
    ParallelPidSeed {
        family,
        proportional_symbol: proportional.to_owned(),
        integral_symbol: integral.map(str::to_owned),
        derivative_symbol: derivative.map(str::to_owned),
    }
}

#[test]
fn a_p_family_names_only_its_proportional_gain() {
    assert_eq!(
        setting_labels(&pid_loop(), Some(Structure::P), &all_gains()),
        rows(&[(KP, "Proportional gain (Kp)")])
    );
}

#[test]
fn a_pi_family_names_the_proportional_and_the_integral_gain() {
    assert_eq!(
        setting_labels(&pid_loop(), Some(Structure::PI), &all_gains()),
        rows(&[(KP, "Proportional gain (Kp)"), (KI, "Integral gain (Ki)")])
    );
}

#[test]
fn a_pd_family_names_the_proportional_and_the_derivative_gain() {
    assert_eq!(
        setting_labels(&pid_loop(), Some(Structure::PD), &all_gains()),
        rows(&[(KP, "Proportional gain (Kp)"), (KD, "Derivative gain (Kd)")])
    );
}

#[test]
fn a_pid_family_names_all_three_gains_in_the_order_of_the_binding() {
    assert_eq!(
        setting_labels(&pid_loop(), Some(Structure::PID), &all_gains()),
        rows(&[
            (KP, "Proportional gain (Kp)"),
            (KI, "Integral gain (Ki)"),
            (KD, "Derivative gain (Kd)"),
        ])
    );
}

#[test]
fn a_setting_without_a_finite_value_has_no_row() {
    // A value that is not a number and a missing value both leave the row out: the page filters on finite values.
    let gains = BTreeMap::from([(KP.to_owned(), 2.0), (KI.to_owned(), f64::NAN)]);
    assert_eq!(
        setting_labels(&pid_loop(), Some(Structure::PID), &gains),
        rows(&[(KP, "Proportional gain (Kp)")])
    );
}

#[test]
fn a_cascade_seed_names_each_controllers_gains_after_its_instance() {
    let seed = ProjectSeedBlock::Cascade(CascadePidSeed {
        inner: cascade_gains("Inner", "C.InnerKp", "C.InnerKi", "C.InnerKd"),
        outer: cascade_gains("Outer", "C.OuterKp", "C.OuterKi", "C.OuterKd"),
    });
    // The derivative variables of a cascade are written as zero and have no row.
    assert_eq!(
        seeded_labels(&seed),
        Some(labels(&[
            ("C.InnerKp", "Inner · Proportional gain (Kp)"),
            ("C.InnerKi", "Inner · Integral gain (Ki)"),
            ("C.OuterKp", "Outer · Proportional gain (Kp)"),
            ("C.OuterKi", "Outer · Integral gain (Ki)"),
        ]))
    );
}

#[test]
fn a_feedforward_seed_names_its_gain_and_its_lead_and_lag_times() {
    let seed = ProjectSeedBlock::Feedforward(FeedforwardPidSeed {
        pid: parallel(Structure::PI, "F.Kp", Some("F.Ki"), Some("F.Kd")),
        gain_symbol: "F.Kff".to_owned(),
        lead_lag: Some(LeadLagSymbols {
            lead_symbol: "F.Lead".to_owned(),
            lag_symbol: "F.Lag".to_owned(),
        }),
    });
    // The page names no derivative gain for this seed, though the seed declares one.
    assert_eq!(
        seeded_labels(&seed),
        Some(labels(&[
            ("F.Kp", "Proportional gain (Kp)"),
            ("F.Ki", "Integral gain (Ki)"),
            ("F.Kff", "Feedforward gain"),
            ("F.Lead", "Lead time"),
            ("F.Lag", "Lag time"),
        ]))
    );
}

#[test]
fn a_smith_seed_names_the_model_gain_time_constant_and_delay() {
    let seed = ProjectSeedBlock::Smith(SmithPidSeed {
        pid: parallel(Structure::PI, "S.Kp", Some("S.Ki"), None),
        gain_symbol: "S.K".to_owned(),
        time_constant_symbol: "S.T".to_owned(),
        delay_symbol: "S.Theta".to_owned(),
        library_block: None,
    });
    assert_eq!(
        seeded_labels(&seed),
        Some(labels(&[
            ("S.Kp", "Proportional gain (Kp)"),
            ("S.Ki", "Integral gain (Ki)"),
            ("S.K", "Model gain K"),
            ("S.T", "Model time constant T"),
            ("S.Theta", "Model delay θ"),
        ]))
    );
}

#[test]
fn a_trajectory_seed_names_its_three_gains_and_its_rate_and_acceleration_feedforward() {
    let seed = ProjectSeedBlock::TrajectoryFeedforward(TrajectoryFeedforwardSeed {
        pid: parallel(Structure::PID, "T.Kp", Some("T.Ki"), Some("T.Kd")),
        rate_symbol: "T.Kv".to_owned(),
        acceleration_symbol: "T.Ka".to_owned(),
    });
    assert_eq!(
        seeded_labels(&seed),
        Some(labels(&[
            ("T.Kp", "Proportional gain (Kp)"),
            ("T.Ki", "Integral gain (Ki)"),
            ("T.Kd", "Derivative gain (Kd)"),
            ("T.Kv", "Rate feedforward (Kv)"),
            ("T.Ka", "Acceleration feedforward (Ka)"),
        ]))
    );
}

#[test]
fn a_parallel_seed_names_only_the_gains_it_declares() {
    let pi = ProjectSeedBlock::ParallelPid(parallel(Structure::PI, "P.Kp", Some("P.Ki"), None));
    assert_eq!(
        seeded_labels(&pi),
        Some(labels(&[
            ("P.Kp", "Proportional gain (Kp)"),
            ("P.Ki", "Integral gain (Ki)")
        ]))
    );
    let proportional = ProjectSeedBlock::ParallelPid(parallel(Structure::P, "Q.Kp", None, None));
    assert_eq!(
        seeded_labels(&proportional),
        Some(labels(&[("Q.Kp", "Proportional gain (Kp)")]))
    );
}

#[test]
fn a_group_seed_and_a_parked_seed_name_no_setting_of_the_page() {
    assert_eq!(seeded_labels(&ProjectSeedBlock::Decentralized), None);
    let parked = ProjectSeedBlock::Parked {
        kind: "unknown-seed".to_owned(),
    };
    assert_eq!(seeded_labels(&parked), None);
}

#[test]
fn a_group_member_row_names_its_instance_and_its_gain() {
    let symbol = "P1.Zone1.Parameters.ProportionalGain";
    let group = PidControllerGroup {
        members: vec![PidGroupMember {
            instance: "P1.Zone1".to_owned(),
            type_name: "Control.PID".to_owned(),
            input_channel_id: "input-1".to_owned(),
            output_channel_id: "output-1".to_owned(),
            mode_symbol: "P1.Auto".to_owned(),
            parameter_symbols: vec![symbol.to_owned()],
            application_state_symbols: None,
        }],
    };
    assert_eq!(
        owned_label(Some(&group), symbol, "Control.PID ProportionalGain"),
        "P1.Zone1 · Proportional gain (Kp)"
    );
    // A field that is no gain is its words, split at each capital letter.
    assert_eq!(
        owned_label(Some(&group), symbol, "Control.PID TrackingTime"),
        "P1.Zone1 · Tracking Time"
    );
    // A symbol that no member owns keeps its label, and so does a loop that is not a group.
    assert_eq!(
        owned_label(Some(&group), "P1.Other", "Control.PID ProportionalGain"),
        "Control.PID ProportionalGain"
    );
    assert_eq!(
        owned_label(None, symbol, "Proportional gain (Kp)"),
        "Proportional gain (Kp)"
    );
}

#[test]
fn a_label_read_from_a_convention_keeps_its_dotted_names_and_splits_its_words() {
    assert_eq!(readable("proportional gain"), "proportional gain");
    assert_eq!(
        readable("Control.PID ProportionalGain"),
        "Control.PID Proportional Gain"
    );
    assert_eq!(readable("max-output_limit"), "max output limit");
    assert_eq!(readable("Main.DeadTime"), "Main.DeadTime");
}

#[test]
fn the_manual_editor_names_each_pid_term_by_its_label_with_the_weights_and_the_filter_time() {
    let mut loop_definition = pid_loop();
    let Some(ControllerLaw::Pid(pid)) = loop_definition.controller.law.as_mut() else {
        panic!("the example is a Control.PID law");
    };
    pid.proportional_setpoint_weight_symbol = Some("P1.Parameters.SetpointWeight".to_owned());
    pid.derivative_setpoint_weight_symbol = Some("P1.Parameters.DerivativeWeight".to_owned());
    pid.measurement_filter_time_symbol = Some("P1.Parameters.FilterTime".to_owned());
    let template = loop_definition
        .parameter_binding
        .as_ref()
        .expect("the example binds its parameters")
        .parameters[0]
        .clone();
    let named = |symbol: &str| Parameter {
        symbol: symbol.to_owned(),
        ..template.clone()
    };
    let expected = [
        (KP, "Proportional gain (Kp)"),
        (KI, "Integral gain (Ki)"),
        (KD, "Derivative gain (Kd)"),
        ("P1.Parameters.SetpointWeight", "Setpoint weight (β)"),
        (
            "P1.Parameters.DerivativeWeight",
            "Derivative setpoint weight (γ)",
        ),
        ("P1.Parameters.FilterTime", "Measurement filter time (Tf)"),
    ];
    for (symbol, label) in expected {
        assert_eq!(
            manual_setting_label(&loop_definition, &named(symbol)),
            label
        );
    }
    assert!(group_setting_symbols(&loop_definition).is_none());
}

#[test]
fn a_margin_that_is_not_a_number_has_its_word_and_a_finite_margin_has_none() {
    assert_eq!(margin_word(MarginValue::Finite(12.5)), None);
    assert_eq!(
        margin_word(MarginValue::Unbounded).as_deref(),
        Some("unbounded")
    );
    assert_eq!(
        margin_word(MarginValue::NotObservedInRange).as_deref(),
        Some("not observed in range")
    );
}

#[test]
fn the_outcome_sentence_is_the_page_s_for_each_status() {
    assert_eq!(CandidateOutcome::Meets.sentence(), "Meets the requirements");
    assert_eq!(
        CandidateOutcome::Misses {
            shortfall: "settling time is 3 s over the limit".to_owned(),
        }
        .sentence(),
        "Misses: settling time is 3 s over the limit"
    );
    assert_eq!(
        CandidateOutcome::Unpredicted {
            reason: "the model has no delay".to_owned(),
        }
        .sentence(),
        "No prediction: the model has no delay"
    );
}

#[test]
fn the_engine_names_the_three_gains_once_for_the_structure_tables() {
    let labels = super::GainLabels::engine();
    assert_eq!(labels.proportional, "Proportional gain (Kp)");
    assert_eq!(labels.integral, "Integral gain (Ki)");
    assert_eq!(labels.derivative, "Derivative gain (Kd)");
}

#[test]
fn every_offered_choice_carries_its_words() {
    // C4, N5.14, D-B9: a state law's finite horizon runs under one of two policies, and the answer words both: the line that names the
    // steps and the policy, and the heading of the stages, which a pin makes the nominal ones. The page lists no table of its own, so a
    // policy without words would show a stage table under no name, and an unknown policy would be worded as the other one.
    let horizon = |steps: usize, policy: HorizonPolicy| StateHorizon {
        steps,
        policy,
        gain_sequence: Vec::new(),
        riccati_sequence: Vec::new(),
    };
    let repeated = horizon(3, HorizonPolicy::RepeatFirstFiniteGain);
    assert_eq!(
        horizon_policy_line(&repeated),
        "Finite horizon: 3 steps \u{b7} Repeated first gain"
    );
    assert_eq!(horizon_stages_heading(&repeated), "Riccati stages");
    let pinned = horizon(1, HorizonPolicy::PinnedFixedGain);
    assert_eq!(
        horizon_policy_line(&pinned),
        "Finite horizon: 1 step \u{b7} Pinned fixed gain",
        "one step is written in the singular"
    );
    assert_eq!(horizon_stages_heading(&pinned), "Nominal Riccati stages");
}
