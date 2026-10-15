//! Tests of the isolated run requests of Validate (`engineerVerificationPlan` and `configuredPlantRequest`), and of the record that
//! a completed run converts to (`verificationClient`). They run on the engineer suite's loop, model and task, which the campaign
//! tests build the same way.

#![allow(clippy::float_cmp)] // The values are the literals the fixture and the candidates carry, compared exactly on purpose.

use std::collections::BTreeMap;

use serde_json::{json, Value};

use super::*;
use crate::commissioning::fixture_loops::{first_order_model, project_pid_loop};
use crate::identification::connected_record::{RunEvent, RunEvidence, ValidationChannel};
use crate::model::decode_general_model;
use crate::tuning::candidate::{
    AntiWindup, CandidateSource, ControlPidParameters, LawCandidate, DEFAULT_RATE_LIMIT_PER_S,
};
use crate::tuning::project_candidate::{project_candidate, served_seed};
use crate::tuning::structure::{ParallelGains, Structure};
use crate::validation::engineer_verification_plan::{
    engineer_requirements, engineer_verification_plan, EngineerRequirements,
};
use crate::validation::isolated_run_protocol::{
    IsolatedRunConditions, IsolatedRunSample, IsolatedRunValue,
};
use crate::validation::scenario_contract::ScenarioEvent;

const FIXTURE: &str = include_str!(
    "../../../../editors/vscode/src/test/fixtures/control-studio-engineer/control-loop.json"
);

/// The engineer fixture as the campaign tests build it: a first-order proportional loop with the PID law and the parameters
/// `Main.Kp`, `Main.Ki` and `Main.Kd`, each bounded from 0 to 100.
fn fixture_loop() -> LoopDefinition {
    let mut value: Value = crate::json::parse_text(FIXTURE).expect("the fixture is JSON");
    if let Some(fields) = value.as_object_mut() {
        fields.remove("analysis");
        fields.remove("analysisParameterSymbols");
    }
    value["controller"]["type"] = json!("Control.PID");
    value["controller"]["design"] = json!("pid");
    value["controller"]["law"] = json!({
        "kind": "pid",
        "parameterRecordSymbol": "Main.Parameters",
        "parameterRevisionSymbol": "Main.ParameterRevision",
        "proportionalGainSymbol": "Main.Kp",
        "integralGainSymbol": "Main.Ki",
        "derivativeGainSymbol": "Main.Kd"
    });
    value["parameterBinding"]["parameters"] = json!([
        { "symbol": "Main.Kp", "iec_type": "LREAL", "unit": "V/m", "convention": "proportional gain", "minimum": 0, "maximum": 100 },
        { "symbol": "Main.Ki", "iec_type": "LREAL", "unit": "V/(m*s)", "convention": "integral gain", "minimum": 0, "maximum": 100 },
        { "symbol": "Main.Kd", "iec_type": "LREAL", "unit": "V*s/m", "convention": "derivative gain", "minimum": 0, "maximum": 100 }
    ]);
    LoopDefinition::decode(&value.to_string()).expect("the fixture decodes")
}

/// The suite's campaign model: a first-order process with a 0.5 s delay, validated as `model-4`.
fn fixture_model() -> GeneralModel {
    decode_general_model(&json!({
        "kind": "transfer-function",
        "numerator": [1.5],
        "denominator": [4.0, 1.0],
        "delaySeconds": 0.5,
        "revision": "model-4",
        "source": "provided",
        "validation": "validated",
        "evidence": ["declared-fixture"],
        "inputs": [{ "id": "u", "unit": "V", "operatingRange": { "minimum": -20.0, "maximum": 20.0 } }],
        "outputs": [{ "id": "y", "unit": "m", "operatingRange": { "minimum": -20.0, "maximum": 20.0 } }]
    }))
    .expect("the fixture model decodes")
}

/// The suite's campaign task: a unit reference change, a 0.2 m error limit, a 0.1 m settling band, 15 s to settle, and a 30 s
/// validation.
fn fixture_task() -> EngineerRequirements {
    engineer_requirements(&json!({
        "referenceChange": 1.0,
        "maximumError": 0.2,
        "settlingBand": 0.1,
        "settlingTimeSeconds": 15.0,
        "durationSeconds": 30.0
    }))
    .expect("the fixture requirements decode")
}

/// The gains the parameter binding names, as the program holds them.
fn fixture_gains() -> BTreeMap<String, f64> {
    BTreeMap::from([
        ("Main.Kp".to_owned(), 1.0),
        ("Main.Ki".to_owned(), 0.2),
        ("Main.Kd".to_owned(), 0.0),
    ])
}

/// The verification plan of the fixture: its grid, its initial values, its observed symbols and its requirements.
fn plan_of(loop_definition: &LoopDefinition) -> VerificationPlan {
    engineer_verification_plan(
        loop_definition,
        &fixture_model(),
        &fixture_task(),
        &fixture_gains(),
        &BTreeMap::new(),
    )
    .expect("the fixture plan")
}

/// The target-response case of a plan: the plan's own requirements, with no events, overrides or effects.
fn nominal_case(plan: &VerificationPlan) -> CampaignCase {
    CampaignCase {
        id: "target-response".to_owned(),
        plan: plan.clone(),
        initial_overrides: BTreeMap::new(),
        events: Vec::new(),
        effects: Vec::new(),
        requirements: plan.requirements.clone(),
    }
}

/// A candidate record with the given proportional and integral gains, valid under `validate_parameters`.
fn sample_candidate(proportional: f64, integral: f64) -> ControlPidCandidate {
    ControlPidCandidate {
        applied: false,
        source: CandidateSource::Simc,
        structure: Structure::PI,
        parameters: ControlPidParameters {
            proportional_gain: proportional,
            integral_gain: integral,
            derivative_gain: 0.0,
            proportional_setpoint_weight: 1.0,
            derivative_setpoint_weight: 0.0,
            bias: 0.0,
            output_minimum: -10.0,
            output_maximum: 10.0,
            rate_minimum: -DEFAULT_RATE_LIMIT_PER_S,
            rate_maximum: DEFAULT_RATE_LIMIT_PER_S,
            tracking_time: 1.0,
            anti_windup: AntiWindup::BackCalculation,
        },
        measurement_filter_seconds: None,
    }
}

/// The request of one case under the given settings and plant, on the fixture's project revision.
fn request_for(
    loop_definition: &LoopDefinition,
    plan: &VerificationPlan,
    case: &CampaignCase,
    settings: RunSettings<'_>,
    simulation: Option<&SimulationOverride>,
    request_id: &str,
) -> Result<IsolatedRunRequest, Failure> {
    isolated_run_request(&RunRequestInput {
        loop_definition,
        model: &fixture_model(),
        plan,
        case,
        settings,
        simulation,
        request_id,
        expected_project_revision: "sha256:project",
    })
}

/// A completed run of two samples, one good and one missing, as the runtime answers it.
fn completed_record() -> IsolatedRunRecord {
    IsolatedRunRecord {
        id: "run-3".to_owned(),
        evidence: RunEvidence::StExecuted,
        parameters: BTreeMap::new(),
        conditions: IsolatedRunConditions {
            initial_state: BTreeMap::from([("Main.Kp".to_owned(), 1.0)]),
            events: vec![RunEvent {
                at_seconds: 0.5,
                channel: "Main.Kp".to_owned(),
                value: 2.0,
            }],
            duration_seconds: 0.02,
            sample_times_seconds: vec![0.0, 0.02],
            seed: 7,
            model_revision: "connected-plant:rev".to_owned(),
            program_revision: "sha256:program".to_owned(),
            parameter_schema_revision: "sha256:schema".to_owned(),
            record_every: None,
            model_provenance: None,
            simulation_config_revision: None,
            feedback_relay: None,
        },
        channels: vec![ValidationChannel {
            id: "Main.PV".to_owned(),
            unit: "m".to_owned(),
        }],
        samples: vec![
            IsolatedRunSample {
                time_seconds: 0.0,
                sequence: 0,
                values: BTreeMap::from([(
                    "Main.PV".to_owned(),
                    IsolatedRunValue {
                        value: Some(1.0),
                        quality: Quality::Good,
                    },
                )]),
            },
            IsolatedRunSample {
                time_seconds: 0.02,
                sequence: 1,
                values: BTreeMap::from([(
                    "Main.PV".to_owned(),
                    IsolatedRunValue {
                        value: None,
                        quality: Quality::Missing,
                    },
                )]),
            },
        ],
    }
}

/// The values the running controller holds for the fixture's mapped record fields (B5.3).
fn observed_settings() -> BTreeMap<String, f64> {
    BTreeMap::from([
        ("Main.Kp".to_owned(), 1.0),
        ("Main.Ki".to_owned(), 0.2),
        ("Main.Kd".to_owned(), 0.0),
    ])
}

#[test]
fn the_current_settings_and_the_candidate_are_both_written_as_their_records() {
    let loop_definition = fixture_loop();
    let plan = plan_of(&loop_definition);
    let case = nominal_case(&plan);
    // B5.3: the current settings are the values the running controller holds, written like the candidate's.
    let observed = BTreeMap::from([
        ("Main.Kp".to_owned(), 1.5),
        ("Main.Ki".to_owned(), 0.25),
        ("Main.Kd".to_owned(), 0.0),
    ]);
    let current = request_for(
        &loop_definition,
        &plan,
        &case,
        RunSettings::Current(&observed),
        None,
        "run-current",
    )
    .expect("the current request");
    let current_values: BTreeMap<String, f64> = current
        .parameter_overrides
        .iter()
        .map(|parameter| (parameter.symbol.clone(), parameter.value))
        .collect();
    assert_eq!(current_values, observed);
    let unread = BTreeMap::from([("Main.Kp".to_owned(), 1.5)]);
    let refused = request_for(
        &loop_definition,
        &plan,
        &case,
        RunSettings::Current(&unread),
        None,
        "run-current",
    )
    .expect_err("a current setting the runtime did not give");
    assert_eq!(refused.next_action, NextAction::ConnectRuntime);
    let candidate = LawCandidate::Pid(sample_candidate(2.0, 0.5));
    let tuned = request_for(
        &loop_definition,
        &plan,
        &case,
        RunSettings::Candidate(&candidate),
        None,
        "run-candidate",
    )
    .expect("the candidate request");
    assert_eq!(
        tuned.parameter_overrides.len(),
        3,
        "Kp, Ki and Kd are mapped"
    );
    let kp = tuned
        .parameter_overrides
        .iter()
        .find(|parameter| parameter.symbol == "Main.Kp")
        .expect("Main.Kp is overridden");
    assert_eq!(kp.value, 2.0);
    assert_eq!(kp.iec_type, IsolatedParameterType::Lreal);
    assert_eq!(kp.unit, "V/m", "the unit the binding declares");
    assert_eq!((kp.minimum, kp.maximum), (0.0, 100.0));
    let ki = tuned
        .parameter_overrides
        .iter()
        .find(|parameter| parameter.symbol == "Main.Ki")
        .expect("Main.Ki is overridden");
    assert_eq!(ki.value, 0.5);
}

#[test]
fn a_program_candidate_writes_its_seed_variables_and_an_explicit_zero_for_the_term_it_omits() {
    // C2 and E8.4: the nominal runs write the candidate as parameter overrides. A PI candidate of a PID seed writes the proportional
    // and integral variables and an explicit zero to the derivative variable, which the seed names but the structure does not use.
    let loop_definition = project_pid_loop();
    let model = first_order_model();
    let gains = BTreeMap::from([
        ("Main.Program.Kp".to_owned(), 2.0),
        ("Main.Program.Ki".to_owned(), 0.5),
        ("Main.Program.Kd".to_owned(), 0.0),
    ]);
    let plan = engineer_verification_plan(
        &loop_definition,
        &model,
        &fixture_task(),
        &gains,
        &BTreeMap::new(),
    )
    .expect("the plan of the program's law");
    let seed = served_seed(&loop_definition).expect("the served seed");
    let candidate = LawCandidate::Project(project_candidate(
        seed,
        CandidateSource::Simc,
        ParallelGains {
            proportional_gain: 2.0,
            integral_gain: 0.5,
            derivative_gain: 0.0,
        },
    ));
    let request = isolated_run_request(&RunRequestInput {
        loop_definition: &loop_definition,
        model: &model,
        plan: &plan,
        case: &nominal_case(&plan),
        settings: RunSettings::Candidate(&candidate),
        simulation: None,
        request_id: "run-program-candidate",
        expected_project_revision: "sha256:project",
    })
    .expect("the candidate request of the program's law");
    let written: BTreeMap<String, f64> = request
        .parameter_overrides
        .iter()
        .map(|parameter| (parameter.symbol.clone(), parameter.value))
        .collect();
    assert_eq!(
        written,
        BTreeMap::from([
            ("Main.Program.Kd".to_owned(), 0.0),
            ("Main.Program.Ki".to_owned(), 0.5),
            ("Main.Program.Kp".to_owned(), 2.0)
        ])
    );
}

#[test]
fn the_request_carries_the_plan_grid_and_passes_the_limits_of_the_runtime() {
    let loop_definition = fixture_loop();
    let plan = plan_of(&loop_definition);
    let case = nominal_case(&plan);
    let request = request_for(
        &loop_definition,
        &plan,
        &case,
        RunSettings::Current(&observed_settings()),
        None,
        "run-grid",
    )
    .expect("the request");
    assert_eq!(request.request_id, "run-grid");
    assert_eq!(request.expected_project_revision, "sha256:project");
    assert_eq!(request.step_ms, plan.step_ms);
    assert_eq!(request.duration_ms, plan.duration_ms);
    assert_eq!(request.max_total_cycles, plan.max_total_cycles);
    assert_eq!(request.record_every, plan.record_every);
    assert_eq!(
        request.wall_budget_ms,
        ISOLATED_SIMULATION_WALL_TIME_LIMIT_MS
    );
    assert_eq!(request.seed, run_seed(&loop_definition));
    assert_eq!(request.observations.len(), plan.observed_symbols.len());
    assert!(
        request
            .observations
            .iter()
            .all(|observation| !observation.unit.is_empty()),
        "every observed channel names its unit (E8.3)"
    );
    assert!(
        request.check().is_ok(),
        "the request holds the limits of E8.4"
    );
}

#[test]
fn the_inline_plant_is_the_callers_choice() {
    let loop_definition = fixture_loop();
    let plan = plan_of(&loop_definition);
    let case = nominal_case(&plan);
    let selected = SimulationOverride {
        model_revision: "model-4".to_owned(),
        model_provenance: "provided".to_owned(),
        simulation_toml: "[plant]\n".to_owned(),
    };
    let modelled = request_for(
        &loop_definition,
        &plan,
        &case,
        RunSettings::Current(&observed_settings()),
        Some(&selected),
        "run-model",
    )
    .expect("the model request");
    assert_eq!(modelled.simulation_override, Some(selected));
    let configured = request_for(
        &loop_definition,
        &plan,
        &case,
        RunSettings::Current(&observed_settings()),
        None,
        "run-plant",
    )
    .expect("the configured request");
    assert!(
        configured.simulation_override.is_none(),
        "the configured simulated plant takes no inline plant (B5.3)"
    );
}

#[test]
fn the_case_initial_overrides_replace_the_plan_values() {
    let loop_definition = fixture_loop();
    let plan = plan_of(&loop_definition);
    let (symbol, _) = plan
        .initial_values
        .iter()
        .next()
        .expect("the plan starts from values");
    let symbol = symbol.clone();
    let case = CampaignCase {
        id: "overridden".to_owned(),
        plan: plan.clone(),
        initial_overrides: BTreeMap::from([(symbol.clone(), 7.0)]),
        events: Vec::new(),
        effects: Vec::new(),
        requirements: Vec::new(),
    };
    let request = request_for(
        &loop_definition,
        &plan,
        &case,
        RunSettings::Current(&observed_settings()),
        None,
        "run-initial",
    )
    .expect("the request");
    assert_eq!(request.initial_values.get(&symbol), Some(&7.0));
}

#[test]
fn an_event_that_is_not_on_a_whole_millisecond_is_refused() {
    let loop_definition = fixture_loop();
    let plan = plan_of(&loop_definition);
    let case = CampaignCase {
        id: "off-grid".to_owned(),
        plan: plan.clone(),
        initial_overrides: BTreeMap::new(),
        events: vec![ScenarioEvent {
            at_ms: 1.5,
            symbol: "Main.Kp".to_owned(),
            value: 1.0,
        }],
        effects: Vec::new(),
        requirements: Vec::new(),
    };
    let refused = request_for(
        &loop_definition,
        &plan,
        &case,
        RunSettings::Current(&observed_settings()),
        None,
        "run-off",
    )
    .expect_err("an event at 1.5 ms is refused");
    assert!(
        refused.message.contains("whole millisecond"),
        "{}",
        refused.message
    );
}

#[test]
fn the_observed_units_come_from_the_loop_declarations() {
    let loop_definition = fixture_loop();
    assert_eq!(
        observation_unit(&loop_definition, "Main.Kp").as_deref(),
        Some("V/m"),
        "a bound parameter names its unit"
    );
    let process = &loop_definition.signals.process_variable;
    assert_eq!(
        observation_unit(&loop_definition, &process.symbol),
        Some(process.unit.clone()),
        "a loop signal names its unit"
    );
    assert_eq!(observation_unit(&loop_definition, "No.Such.Symbol"), None);
}

#[test]
fn a_loop_without_an_experiment_runs_with_seed_one() {
    let mut loop_definition = fixture_loop();
    loop_definition.experiment = None;
    assert_eq!(run_seed(&loop_definition), 1);
}

#[test]
fn the_record_fields_take_the_values_of_their_gain_keys() {
    let mut candidate = sample_candidate(2.0, 0.5);
    candidate.parameters.tracking_time = 4.0;
    assert_eq!(
        pid_value(&candidate, GainKey::BackCalculationGain).expect("1/Tt"),
        0.25,
        "the back-calculation gain is 1/Tt (C0)"
    );
    assert!(
        pid_value(&candidate, GainKey::MeasurementFilterSeconds).is_err(),
        "a filter the candidate does not set is refused"
    );
    candidate.measurement_filter_seconds = Some(0.1);
    assert_eq!(
        pid_value(&candidate, GainKey::MeasurementFilterSeconds).expect("Tf"),
        0.1
    );
}

#[test]
fn a_completed_run_converts_to_the_record_the_evaluator_reads() {
    let record = completed_record();
    let run = validation_run_of(&record).expect("the record converts");
    assert_eq!(run.id, "run-3");
    assert_eq!(run.evidence, RunEvidence::StExecuted);
    assert_eq!(run.conditions.seed, 7);
    assert_eq!(run.conditions.sample_times_seconds, vec![0.0, 0.02]);
    assert_eq!(
        run.samples[0].values["Main.PV"],
        ValidationValue {
            value: Some(1.0),
            quality: ChannelQuality::Good,
        }
    );
    assert_eq!(
        run.samples[1].values["Main.PV"],
        ValidationValue {
            value: None,
            quality: ChannelQuality::Bad,
        },
        "a missing value is bad quality (A4.2)"
    );
}

#[test]
fn a_seed_that_the_validation_record_cannot_hold_is_refused() {
    let mut record = completed_record();
    record.conditions.seed = 4_294_967_296;
    let refused = validation_run_of(&record).expect_err("a 33-bit seed is refused");
    assert!(refused.message.contains("32-bit"), "{}", refused.message);
}
