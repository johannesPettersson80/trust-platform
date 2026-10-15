//! The run requests of a gain schedule (specification 35, C10 Validate): a candidate's run writes the gains of every point of its table with their
//! units and bounds, starts the scheduling variable at the loop's own operating point, and observes the active gains with the units of its own
//! point. A loop that is not a schedule keeps the overrides of its record. The tank is the one of `gain_schedule_case_tests.rs`.

#![allow(clippy::float_cmp)] // The gains and operating points are the exact literals the candidate and the tank carry.

use std::collections::BTreeMap;

use serde_json::{json, Value};

use super::*;
use crate::loop_definition::{Action, LoopDefinition, LoopScope, ModeWhenTrue};
use crate::setup::catalog::{decode_catalog_answer, CompiledCatalog};
use crate::setup::loop_bindings::FormSignal;
use crate::setup::pid_loop::{ModeChoice, SignalChoice};
use crate::setup::schedule_loop::{
    schedule_loop_definitions, ScheduleLoopDraft, SchedulePointDraft, SchedulingVariableDraft,
};
use crate::tuning::candidate::{CandidateSource, LawCandidate};
use crate::tuning::project_candidate::ProjectCandidate;
use crate::tuning::state_controller::fixtures::two_tank_model;
use crate::tuning::structure::{ParallelGains, Structure};
use crate::validation::engineer_verification_campaign::CampaignCase;
use crate::validation::engineer_verification_plan::VerificationPlan;
use crate::validation::gain_schedule_case;
use crate::validation::isolated_run_protocol::IsolatedParameterType;
use crate::validation::smith_case;

/// Lesson 84 as the shipped file declares it: the heater with its draw-off fed forward through a lead and a lag.
const LESSON_84: &str = include_str!(
    "../../../../editors/vscode/media/examples/control/14-integrated-capstones/84-independent/control-loop.json"
);

fn scope() -> LoopScope {
    LoopScope {
        project_id: "project:validate".to_string(),
        runtime_id: "runtime-1".to_string(),
    }
}

#[allow(clippy::needless_pass_by_value)] // A test fixture: it takes the JSON literals its callers write in place.
fn decode(control: Value) -> CompiledCatalog {
    decode_catalog_answer(&json!({ "ok": true, "processImage": {}, "controlSymbols": control }))
        .expect("the compiled answer decodes")
        .control
}

/// The tank's program symbols: the level, the setpoint, the valve, the manual output, the mode, the startup flag and the four table gains.
fn tank() -> CompiledCatalog {
    decode(json!({
        "symbols": [
            { "symbol": "P1.LevelM", "type": "LREAL", "address": "%IL0" },
            { "symbol": "P1.SetpointM", "type": "LREAL", "address": "%ML0" },
            { "symbol": "P1.ValvePct", "type": "LREAL", "address": "%QL0" },
            { "symbol": "P1.ManualPct", "type": "LREAL", "address": "%ML8" },
            { "symbol": "P1.Auto", "type": "BOOL" },
            { "symbol": "P1.Started", "type": "BOOL" },
            { "symbol": "P1.KpLow", "type": "LREAL" },
            { "symbol": "P1.KiLow", "type": "LREAL" },
            { "symbol": "P1.KpHigh", "type": "LREAL" },
            { "symbol": "P1.KiHigh", "type": "LREAL" }
        ],
        "controllers": [
            { "instance": "P1", "type": "Main", "tasks": [{ "name": "MainTask", "intervalMs": 100 }] },
            { "instance": "P1.Loop", "type": "Control.PID", "tasks": [{ "name": "MainTask", "intervalMs": 100 }] }
        ],
        "links": [],
        "writes": ["P1.ValvePct"]
    }))
}

fn signal(symbol: &str, unit: &str, minimum: f64, maximum: f64) -> FormSignal {
    FormSignal {
        symbol: symbol.to_string(),
        unit: unit.to_string(),
        minimum: Some(minimum),
        maximum: Some(maximum),
    }
}

/// The tank's schedule: Low at 0.09 m and High at 0.49 m, each with a tolerance of 0.01 m.
fn draft() -> ScheduleLoopDraft {
    ScheduleLoopDraft {
        name: "Tank level".to_string(),
        action: Action::Direct,
        instance: "P1.Loop".to_string(),
        parameter_record: "P1.Parameters".to_string(),
        scheduling: SchedulingVariableDraft {
            symbol: "P1.LevelM".to_string(),
            unit: "m".to_string(),
        },
        process_variable: signal("P1.LevelM", "m", 0.0, 2.0),
        manipulated_variable: signal("P1.ValvePct", "%", 0.0, 100.0),
        setpoint: SignalChoice {
            symbol: "P1.SetpointM".to_string(),
            unit: None,
            minimum: None,
            maximum: None,
        },
        mode: ModeChoice {
            symbol: "P1.Auto".to_string(),
            when_true: ModeWhenTrue::Auto,
        },
        manual_output: "P1.ManualPct".to_string(),
        points: vec![
            SchedulePointDraft {
                name: "Low".to_string(),
                value: Some(0.09),
                tolerance: Some(0.01),
                proportional_gain: "P1.KpLow".to_string(),
                integral_gain: "P1.KiLow".to_string(),
            },
            SchedulePointDraft {
                name: "High".to_string(),
                value: Some(0.49),
                tolerance: Some(0.01),
                proportional_gain: "P1.KpHigh".to_string(),
                integral_gain: "P1.KiHigh".to_string(),
            },
        ],
        experiment_step: None,
        experiment_duration_seconds: None,
        startup_flag: Some("P1.Started".to_string()),
    }
}

fn tank_loops() -> Vec<LoopDefinition> {
    schedule_loop_definitions(&tank(), &scope(), &draft()).expect("the tank's schedule")
}

fn gains(proportional: f64, integral: f64) -> ParallelGains {
    ParallelGains {
        proportional_gain: proportional,
        integral_gain: integral,
        derivative_gain: 0.0,
    }
}

/// The tank's candidate as Tune writes it: the Low point's gains, and the values of the whole table.
fn table_candidate() -> LawCandidate {
    LawCandidate::Project(ProjectCandidate {
        applied: false,
        source: CandidateSource::Manual,
        structure: Structure::PI,
        gains: gains(2.0, 0.2),
        values: BTreeMap::from([
            ("P1.KpLow".to_owned(), 2.0),
            ("P1.KiLow".to_owned(), 0.2),
            ("P1.KpHigh".to_owned(), 3.0),
            ("P1.KiHigh".to_owned(), 0.3),
        ]),
    })
}

/// The candidate of lesson 84's PI with its feedforward values, as the measured-disturbance tests build it.
fn lesson_84_candidate() -> LawCandidate {
    LawCandidate::Project(ProjectCandidate {
        applied: false,
        source: CandidateSource::ConstrainedComparison,
        structure: Structure::PI,
        gains: gains(2.0, 0.5),
        values: BTreeMap::from([
            ("P1.Parameters.ProportionalGain".to_owned(), 2.0),
            ("P1.Parameters.IntegralGain".to_owned(), 0.5),
            ("P1.FeedforwardGain".to_owned(), 0.5),
            ("P1.LeadS".to_owned(), 8.0),
            ("P1.LagS".to_owned(), 3.0),
        ]),
    })
}

/// A plan on the tank's 100 ms scan over 10 seconds, with the level and another value as its initial values, and the given observed symbols.
fn tank_plan(observed: &[&str]) -> VerificationPlan {
    VerificationPlan {
        step_ms: 100,
        duration_ms: 10_000,
        max_total_cycles: 1_000,
        record_every: 1,
        initial_values: BTreeMap::from([
            ("P1.LevelM".to_owned(), 0.0),
            ("P1.Other".to_owned(), 1.0),
        ]),
        observed_symbols: observed.iter().map(|symbol| (*symbol).to_owned()).collect(),
        requirements: Vec::new(),
    }
}

/// The target-response case of a plan: the plan's grid, with no events, overrides or effects.
fn case_of(plan: &VerificationPlan) -> CampaignCase {
    CampaignCase {
        id: "target-response".to_owned(),
        plan: plan.clone(),
        initial_overrides: BTreeMap::new(),
        events: Vec::new(),
        effects: Vec::new(),
        requirements: Vec::new(),
    }
}

#[test]
fn a_schedule_candidate_writes_the_gains_of_every_point_with_their_units_and_bounds() {
    let loops = tank_loops();
    let candidate = table_candidate();
    let overrides = parameter_overrides(&loops[0], RunSettings::Candidate(&candidate))
        .expect("the schedule's overrides");
    let written: Vec<(&str, f64, &str)> = overrides
        .iter()
        .map(|item| (item.symbol.as_str(), item.value, item.unit.as_str()))
        .collect();
    assert_eq!(
        written,
        vec![
            ("P1.KpLow", 2.0, "%/m"),
            ("P1.KiLow", 0.2, "%/(m*s)"),
            ("P1.KpHigh", 3.0, "%/m"),
            ("P1.KiHigh", 0.3, "%/(m*s)"),
        ]
    );
    for item in &overrides {
        assert_eq!(item.iec_type, IsolatedParameterType::Lreal);
        assert_eq!((item.minimum, item.maximum), (-1e6, 1e6));
    }
}

#[test]
fn a_run_starts_the_scheduling_variable_at_its_own_points_value_after_the_plan_and_the_case() {
    let loops = tank_loops();
    let plan = tank_plan(&[]);
    let mut case = case_of(&plan);
    // The case's own start of the level is overwritten by the operating point, which the run starts from.
    case.initial_overrides.insert("P1.LevelM".to_owned(), 0.3);
    let high = initial_values(&loops[1], &plan, &case);
    assert_eq!(high.get("P1.LevelM"), Some(&0.49));
    assert_eq!(high.get("P1.Other"), Some(&1.0));
    let low = initial_values(&loops[0], &plan, &case);
    assert_eq!(low.get("P1.LevelM"), Some(&0.09));
}

#[test]
fn a_schedule_run_observes_its_active_gains_with_the_units_of_its_own_point() {
    let loops = tank_loops();
    let plan = tank_plan(&[
        "P1.LevelM",
        "P1.Parameters.ProportionalGain",
        "P1.Parameters.IntegralGain",
    ]);
    let observed = observations(&loops[1], &plan).expect("the observations");
    let units: Vec<(&str, &str)> = observed
        .iter()
        .map(|item| (item.symbol.as_str(), item.unit.as_str()))
        .collect();
    assert_eq!(
        units,
        vec![
            ("P1.LevelM", "m"),
            ("P1.Parameters.ProportionalGain", "%/m"),
            ("P1.Parameters.IntegralGain", "%/(m*s)"),
        ]
    );
}

#[test]
fn a_schedule_candidate_request_passes_the_protocol_checks_with_its_gains_and_its_start() {
    let loops = tank_loops();
    let plan = tank_plan(&[
        "P1.LevelM",
        "P1.Parameters.ProportionalGain",
        "P1.Parameters.IntegralGain",
    ]);
    let case = case_of(&plan);
    let model = two_tank_model();
    let candidate = table_candidate();
    let request = isolated_run_request(&RunRequestInput {
        loop_definition: &loops[1],
        model: &model,
        plan: &plan,
        case: &case,
        settings: RunSettings::Candidate(&candidate),
        simulation: None,
        request_id: "schedule-run-1",
        expected_project_revision: "sha256:project",
    })
    .expect("the schedule's request passes the protocol");
    assert_eq!(request.parameter_overrides.len(), 4);
    assert_eq!(request.initial_values.get("P1.LevelM"), Some(&0.49));
    assert_eq!(request.observations.len(), 3);
}

#[test]
fn a_candidate_of_a_loop_that_is_not_a_schedule_keeps_its_record_overrides() {
    let loop_definition = LoopDefinition::decode(LESSON_84).expect("lesson 84 decodes");
    let candidate = lesson_84_candidate();
    let mut expected = record_overrides(&loop_definition, |mapped| {
        candidate_value(&candidate, mapped)
    })
    .expect("the record's overrides");
    expected.extend(
        feedforward_overrides(&loop_definition, &candidate).expect("the feedforward overrides"),
    );
    expected.extend(
        smith_case::overrides(&loop_definition, &candidate).expect("the predictor's overrides"),
    );
    assert_eq!(
        parameter_overrides(&loop_definition, RunSettings::Candidate(&candidate))
            .expect("the candidate's overrides"),
        expected
    );
    assert_eq!(
        gain_schedule_case::observed_active_gains(&loop_definition),
        [] as [String; 0]
    );
}
