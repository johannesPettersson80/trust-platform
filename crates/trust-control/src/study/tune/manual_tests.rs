//! Tests of the manual method (specification 35, B4.2, B4.5 and C0): the bounds of the entered settings, the candidate they
//! give, and the structure and the filter time that the record takes from them.

use std::collections::BTreeMap;

use super::*;
use crate::identification::identify_result::tests::plant_loop;
use crate::loop_definition::{ControllerLaw, ModeBinding, ParameterBinding, PidLaw, RealType};
use crate::messages::TuneRequest;
use crate::tuning::candidate::{CandidateSource, ControlPidCandidate, LawCandidate};
use crate::tuning::project_seed::{ParallelPidSeed, TrajectoryFeedforwardSeed};
use crate::tuning::settings_pins::{family_setting_symbols, ControllerSettingsPins};

/// The `Control.PID` record of a manual candidate: these tests hold a `Control.PID` loop, so the candidate is a record.
fn pid(candidate: &LawCandidate) -> ControlPidCandidate {
    match candidate {
        LawCandidate::Pid(candidate) => *candidate,
        LawCandidate::Project(_)
        | LawCandidate::State(_)
        | LawCandidate::Search(_)
        | LawCandidate::OnOff(_)
        | LawCandidate::Group(_) => panic!("a Control.PID loop gives a record"),
    }
}

/// A loop with a PI-capable law over the parameters `Main.Kp`, `Main.Ki` and `Main.Kd` (each LREAL, 0 to 10 in unit 1), and
/// over `Main.Tf` (REAL, 0 to 5) when `with_filter` is set, which the law then declares as its measurement filter.
fn bound_pi_loop(with_filter: bool) -> LoopDefinition {
    let mut parameters = vec![
        bound("Main.Kp", RealType::Lreal, 10.0),
        bound("Main.Ki", RealType::Lreal, 10.0),
        bound("Main.Kd", RealType::Lreal, 10.0),
    ];
    if with_filter {
        parameters.push(bound("Main.Tf", RealType::Real, 5.0));
    }
    let mut loop_definition = plant_loop();
    loop_definition.controller.law = Some(ControllerLaw::Pid(Box::new(PidLaw {
        parameter_record_symbol: "Main.Parameters".to_owned(),
        parameter_revision_symbol: None,
        proportional_gain_symbol: "Main.Kp".to_owned(),
        integral_gain_symbol: "Main.Ki".to_owned(),
        derivative_gain_symbol: "Main.Kd".to_owned(),
        proportional_setpoint_weight_symbol: None,
        derivative_setpoint_weight_symbol: None,
        measurement_filter_time_symbol: with_filter.then(|| "Main.Tf".to_owned()),
        anti_windup: None,
    })));
    loop_definition.parameter_binding = Some(ParameterBinding {
        loop_id: "tank".to_owned(),
        task_name: "MainTask".to_owned(),
        parameters,
        mode: ModeBinding {
            symbol: "Main.Mode".to_owned(),
            manual_value: false,
        },
    });
    loop_definition
}

/// A parameter of the binding in unit 1, from zero to `maximum`.
fn bound(symbol: &str, iec_type: RealType, maximum: f64) -> Parameter {
    Parameter {
        symbol: symbol.to_owned(),
        iec_type,
        unit: "1".to_owned(),
        convention: "gain".to_owned(),
        minimum: 0.0,
        maximum,
    }
}

fn entry(pairs: &[(&str, f64)]) -> BTreeMap<String, f64> {
    pairs
        .iter()
        .map(|(symbol, value)| ((*symbol).to_owned(), *value))
        .collect()
}

/// The trajectory-following law's seed over `Main.Kp`, `Main.Ki`, `Main.Kd`, `Main.Kv` and `Main.Ka` (C11).
fn trajectory_seed() -> TrajectoryFeedforwardSeed {
    TrajectoryFeedforwardSeed {
        pid: ParallelPidSeed {
            family: Structure::PID,
            proportional_symbol: "Main.Kp".to_owned(),
            integral_symbol: Some("Main.Ki".to_owned()),
            derivative_symbol: Some("Main.Kd".to_owned()),
        },
        rate_symbol: "Main.Kv".to_owned(),
        acceleration_symbol: "Main.Ka".to_owned(),
    }
}

#[test]
fn a_trajectory_manual_entry_keeps_its_rate_and_acceleration_gains_and_refuses_without_them() {
    let seed = trajectory_seed();
    let gains = ParallelGains {
        proportional_gain: 2.0,
        integral_gain: 1.0,
        derivative_gain: 0.0,
    };
    let entered = entry(&[
        ("Main.Kp", 2.0),
        ("Main.Ki", 1.0),
        ("Main.Kd", 0.0),
        ("Main.Kv", 0.3),
        ("Main.Ka", 0.1),
    ]);
    let (rate, acceleration) =
        trajectory_feedforward(&seed, &entered).expect("both feedforward gains are entered");
    let candidate =
        trajectory_candidate(&seed, CandidateSource::Manual, gains, (rate, acceleration));
    assert!(candidate
        .values
        .get("Main.Kv")
        .is_some_and(|value| (value - 0.3).abs() < f64::EPSILON));
    assert!(candidate
        .values
        .get("Main.Ka")
        .is_some_and(|value| (value - 0.1).abs() < f64::EPSILON));
    let without = entry(&[
        ("Main.Kp", 2.0),
        ("Main.Ki", 1.0),
        ("Main.Kd", 0.0),
        ("Main.Kv", 0.3),
    ]);
    let failure =
        trajectory_feedforward(&seed, &without).expect_err("the acceleration gain is required");
    assert_eq!(failure.message, texts::REQUIRED_SETTING_MISSING);
}

#[test]
fn a_pi_entry_on_a_bound_loop_is_an_unapplied_candidate_from_the_manual_source() {
    let loop_definition = bound_pi_loop(false);
    let candidate = manual_candidate(
        &loop_definition,
        Structure::PI,
        &entry(&[("Main.Kp", 1.2), ("Main.Ki", 0.4)]),
        None,
    )
    .map(|candidate| pid(&candidate))
    .expect("a complete PI entry is a candidate");
    assert_eq!(candidate.source, CandidateSource::Manual);
    assert!(!candidate.applied);
    assert_eq!(candidate.structure, Structure::PI);
    assert!((candidate.parameters.proportional_gain - 1.2).abs() <= 1e-12);
    assert!((candidate.parameters.integral_gain - 0.4).abs() <= 1e-12);
    assert!(candidate.parameters.derivative_gain.abs() <= 1e-12);
    assert_eq!(candidate.measurement_filter_seconds, None);
}

#[test]
fn an_entry_outside_its_bounds_or_for_an_unbound_symbol_gives_its_sentence() {
    let loop_definition = bound_pi_loop(false);
    assert_eq!(
        outside_bounds(&loop_definition, &entry(&[("Main.Kp", 11.0)])).as_deref(),
        Some("Main.Kp must be between 0 and 10 1; 11 is outside that range.")
    );
    assert_eq!(
        outside_bounds(&loop_definition, &entry(&[("Main.Other", 1.0)])).as_deref(),
        Some("Main.Other is not a setting of this controller.")
    );
    assert_eq!(
        outside_bounds(
            &loop_definition,
            &entry(&[("Main.Kp", 1.0), ("Main.Ki", 0.5)])
        ),
        None
    );
}

#[test]
fn a_refused_setting_names_its_own_field_and_an_entry_no_number_reads_is_refused_so() {
    let loop_definition = bound_pi_loop(false);
    assert_eq!(
        first_outside_bounds(&loop_definition, &entry(&[("Main.Kp", 11.0)])),
        Some((
            "Main.Kp".to_owned(),
            "Main.Kp must be between 0 and 10 1; 11 is outside that range.".to_owned()
        ))
    );
    let unreadable = first_outside_bounds(&loop_definition, &entry(&[("Main.Kp", f64::NAN)]))
        .expect("an entry that no number reads is refused");
    assert_eq!(unreadable.0, "Main.Kp");
    assert_eq!(
        unreadable.1,
        "Main.Kp must be between 0 and 10 1; the entry is not a number."
    );
}

#[test]
fn a_missing_required_setting_is_refused_as_a_settings_failure_with_no_next_action() {
    let loop_definition = bound_pi_loop(false);
    let failure = manual_candidate(
        &loop_definition,
        Structure::PI,
        &entry(&[("Main.Kp", 1.2)]),
        None,
    )
    .expect_err("PI needs its integral gain");
    assert_eq!(failure.message, texts::REQUIRED_SETTING_MISSING);
    assert_eq!(failure.cause, Cause::Settings);
    assert_eq!(failure.next_action, NextAction::None);
}

#[test]
fn an_entry_that_changes_a_pinned_value_is_refused_and_one_that_keeps_it_is_kept() {
    let loop_definition = bound_pi_loop(false);
    let pins = ControllerSettingsPins {
        loop_revision: loop_definition.revision.clone(),
        family: Structure::PI,
        values: BTreeMap::from([("Main.Kp".to_owned(), 2.0)]),
    };
    let changed = manual_candidate(
        &loop_definition,
        Structure::PI,
        &entry(&[("Main.Kp", 1.2), ("Main.Ki", 0.4)]),
        Some(&pins),
    )
    .expect_err("the entry changes the pinned Kp");
    assert_eq!(
        changed.message,
        "Unpin Main.Kp before changing its setting."
    );
    let kept = manual_candidate(
        &loop_definition,
        Structure::PI,
        &entry(&[("Main.Kp", 2.0), ("Main.Ki", 0.4)]),
        Some(&pins),
    )
    .map(|candidate| pid(&candidate))
    .expect("the entry keeps the pinned Kp");
    assert!((kept.parameters.proportional_gain - 2.0).abs() <= 1e-12);
}

#[test]
fn a_law_that_declares_a_filter_needs_its_entered_filter_time_and_records_it() {
    let loop_definition = bound_pi_loop(true);
    let without = manual_candidate(
        &loop_definition,
        Structure::PI,
        &entry(&[("Main.Kp", 1.2), ("Main.Ki", 0.4)]),
        None,
    )
    .expect_err("the filter time is required");
    assert_eq!(without.message, texts::REQUIRED_SETTING_MISSING);
    let candidate = manual_candidate(
        &loop_definition,
        Structure::PI,
        &entry(&[("Main.Kp", 1.2), ("Main.Ki", 0.4), ("Main.Tf", 0.25)]),
        None,
    )
    .map(|candidate| pid(&candidate))
    .expect("the entered filter time completes the entry");
    assert!(matches!(
        candidate.measurement_filter_seconds,
        Some(filter) if (filter - 0.25).abs() <= 1e-12
    ));
}

#[test]
fn without_a_fixed_structure_the_entered_gains_give_it_and_the_method_must_be_manual() {
    let loop_definition = bound_pi_loop(false);
    let request = |settings: BTreeMap<String, f64>, method: Option<TuneMethod>| TuneRequest {
        project_root: "/project".to_owned(),
        runtime_id: "runtime".to_owned(),
        loop_id: "tank".to_owned(),
        storage_folder: "/storage".to_owned(),
        method,
        choice: None,
        structure: None,
        pins: None,
        settings: Some(settings),
        connection: None,
    };
    let proportional_only = candidate_of(
        &loop_definition,
        &request(entry(&[("Main.Kp", 1.2)]), Some(TuneMethod::Manual)),
        None,
    )
    .expect("a proportional entry is a candidate")
    .expect("the manual method gives a candidate");
    assert_eq!(proportional_only.structure(), Some(Structure::P));
    let with_integral = candidate_of(
        &loop_definition,
        &request(
            entry(&[("Main.Kp", 1.2), ("Main.Ki", 0.4)]),
            Some(TuneMethod::Manual),
        ),
        None,
    )
    .expect("a PI entry is a candidate")
    .expect("the manual method gives a candidate");
    assert_eq!(with_integral.structure(), Some(Structure::PI));
    let formula = request(entry(&[("Main.Kp", 1.2)]), Some(TuneMethod::Simc));
    assert_eq!(
        candidate_of(&loop_definition, &formula, None).expect("a formula is not a candidate"),
        None
    );
}

#[test]
fn a_control_pid_family_takes_the_record_fields_it_uses_and_its_filter_when_one_is_bound() {
    // C0 "Families and zero terms": the page asks for the settings of the family alone, and Tune refuses any other.
    fn names(symbols: &[&str]) -> Vec<String> {
        symbols.iter().map(|symbol| (*symbol).to_owned()).collect()
    }
    let plain = bound_pi_loop(false);
    assert_eq!(
        family_setting_symbols(&plain, Structure::P),
        names(&["Main.Kp"])
    );
    assert_eq!(
        family_setting_symbols(&plain, Structure::PI),
        names(&["Main.Kp", "Main.Ki"])
    );
    assert_eq!(
        family_setting_symbols(&plain, Structure::PD),
        names(&["Main.Kp", "Main.Kd"])
    );
    assert_eq!(
        family_setting_symbols(&plain, Structure::PID),
        names(&["Main.Kp", "Main.Ki", "Main.Kd"])
    );
    let filtered = bound_pi_loop(true);
    assert_eq!(
        family_setting_symbols(&filtered, Structure::P),
        names(&["Main.Kp", "Main.Tf"]),
        "the filter time is entered for every family once the law declares one (C0)"
    );
}

#[test]
fn a_proportional_law_names_its_gain_for_every_family_the_page_shows() {
    // C13: a proportional law's one setting is its gain, whichever family the page shows (manualTuneSettings.ts).
    let mut loop_definition = bound_pi_loop(false);
    loop_definition.controller.law = Some(ControllerLaw::Proportional {
        gain_symbol: "Main.Kp".to_owned(),
    });
    for family in [Structure::P, Structure::PI] {
        assert_eq!(
            family_setting_symbols(&loop_definition, family),
            vec!["Main.Kp".to_owned()]
        );
    }
}

/// The loop of [`bound_pi_loop`] whose law also declares the derivative setpoint weight γ as the record field `Main.Gamma`, from 0 to 1.
fn gamma_loop() -> LoopDefinition {
    let mut loop_definition = bound_pi_loop(false);
    let Some(ControllerLaw::Pid(law)) = loop_definition.controller.law.as_mut() else {
        panic!("the loop has a PID law");
    };
    law.derivative_setpoint_weight_symbol = Some("Main.Gamma".to_owned());
    loop_definition
        .parameter_binding
        .as_mut()
        .expect("the loop binds its parameters")
        .parameters
        .push(bound("Main.Gamma", RealType::Lreal, 1.0));
    loop_definition
}

#[test]
fn the_entered_derivative_weight_reaches_the_record_and_a_calculated_candidate_states_gamma_zero() {
    // control-studio-derivative-weight: "writes the entered derivative weight and a calculated candidate differentiates the measurement"
    // (specification 35, C0 "Setpoint weight": manual settings require the weight beside the gains, and a calculated candidate writes the
    // default weight, γ = 0, so that the derivative acts on the measurement).
    use crate::study::tune::record::shaping_of_candidate;
    let loop_definition = gamma_loop();
    let entered = entry(&[("Main.Kp", 6.0), ("Main.Kd", 3.0), ("Main.Gamma", 1.0)]);
    let candidate = manual_candidate(&loop_definition, Structure::PD, &entered, None)
        .expect("a complete PD entry");
    assert!(
        (pid(&candidate).parameters.derivative_setpoint_weight - 1.0).abs() <= 1e-12,
        "the record receives the engineer's γ"
    );
    assert!(
        (shaping_of_candidate(&candidate).derivative_setpoint_weight - 1.0).abs() <= 1e-12,
        "and the prediction uses the same γ"
    );

    let missing = manual_candidate(
        &loop_definition,
        Structure::PD,
        &entry(&[("Main.Kp", 6.0), ("Main.Kd", 3.0)]),
        None,
    )
    .expect_err("a declared derivative weight is part of the manual PD settings");
    assert_eq!(missing.message, texts::REQUIRED_SETTING_MISSING);

    let calculated = candidate_record(
        ParallelGains {
            proportional_gain: 6.0,
            integral_gain: 0.0,
            derivative_gain: 3.0,
        },
        CandidateSource::ConstrainedComparison,
        &LoopLimits::of(&loop_definition),
        record_shaping(&loop_definition, &BTreeMap::new()),
        None,
    )
    .expect("a valid record");
    assert!(
        calculated.parameters.derivative_setpoint_weight.abs() <= 1e-12,
        "a calculated candidate differentiates the measurement, so its record states γ = 0"
    );
}

#[test]
fn an_entered_gain_is_taken_inside_its_bounds_and_refused_below_them_or_when_no_number_reads() {
    // control-studio-chart: "keeps exact graphical gain parsing as a pure controller input boundary" (specification 35, B4.5 and C0): with
    // the bounds 0.01 to 20 the gain 4.25 is taken, 0 is below the minimum, and an entry that no number reads is refused, each with the
    // bounds in its sentence.
    let mut loop_definition = bound_pi_loop(false);
    let proportional = &mut loop_definition
        .parameter_binding
        .as_mut()
        .expect("the loop binds its parameters")
        .parameters[0];
    assert_eq!(proportional.symbol, "Main.Kp");
    proportional.minimum = 0.01;
    proportional.maximum = 20.0;
    assert_eq!(
        first_outside_bounds(&loop_definition, &entry(&[("Main.Kp", 4.25)])),
        None
    );
    assert_eq!(
        first_outside_bounds(&loop_definition, &entry(&[("Main.Kp", 0.0)])),
        Some((
            "Main.Kp".to_owned(),
            "Main.Kp must be between 0.01 and 20 1; 0 is outside that range.".to_owned()
        ))
    );
    let unreadable = first_outside_bounds(&loop_definition, &entry(&[("Main.Kp", f64::NAN)]))
        .expect("an entry that no number reads is refused");
    assert_eq!(
        unreadable.1,
        "Main.Kp must be between 0.01 and 20 1; the entry is not a number."
    );
}
