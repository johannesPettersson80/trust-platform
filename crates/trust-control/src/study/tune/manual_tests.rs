//! Tests of the manual method (specification 35, B4.2, B4.5 and C0): the bounds of the entered settings, the candidate they
//! give, and the structure and the filter time that the record takes from them.

use std::collections::BTreeMap;

use super::*;
use crate::identification::identify_result::tests::plant_loop;
use crate::loop_definition::{ControllerLaw, ModeBinding, ParameterBinding, PidLaw, RealType};
use crate::messages::TuneRequest;
use crate::tuning::candidate::{CandidateSource, ControlPidCandidate, LawCandidate};
use crate::tuning::settings_pins::{family_setting_symbols, ControllerSettingsPins};

/// The `Control.PID` record of a manual candidate: these tests hold a `Control.PID` loop, so the candidate is a record.
fn pid(candidate: &LawCandidate) -> ControlPidCandidate {
    match candidate {
        LawCandidate::Pid(candidate) => *candidate,
        LawCandidate::Project(_) => panic!("a Control.PID loop gives a record"),
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
    )
    .expect("a proportional entry is a candidate")
    .expect("the manual method gives a candidate");
    assert_eq!(proportional_only.structure(), Structure::P);
    let with_integral = candidate_of(
        &loop_definition,
        &request(
            entry(&[("Main.Kp", 1.2), ("Main.Ki", 0.4)]),
            Some(TuneMethod::Manual),
        ),
    )
    .expect("a PI entry is a candidate")
    .expect("the manual method gives a candidate");
    assert_eq!(with_integral.structure(), Structure::PI);
    let formula = request(entry(&[("Main.Kp", 1.2)]), Some(TuneMethod::Simc));
    assert_eq!(
        candidate_of(&loop_definition, &formula).expect("a formula is not a candidate"),
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
