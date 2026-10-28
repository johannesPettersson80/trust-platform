//! Tests of the units that the entry run's channels take (specification 35, B5.3 and E8.3): a channel that a declared signal names keeps
//! the job builder's unit, and a channel that only the entry case observes takes the unit that the loop declares for it. Tests of the
//! start that the case plans on (B5.3): the first sample of the configured plant, and the model's own point when the sample has no
//! measured value. The case itself is judged in `study::validate_entry_tests`, where the Validate composition answers it.

use std::collections::BTreeMap;

use serde_json::json;
use trust_control_math::model::OperatingPoint;

use super::*;
use crate::commissioning::adapters::{controller_adapter, ControllerGains};
use crate::commissioning::fixture_loops::{
    first_order_model, parallel_pid_loop, project_p_loop_with_startup, project_pid_loop,
    requirements,
};
use crate::identification::connected_record::RunEvidence;
use crate::model::decode_general_model;
use crate::signals::Quality;
use crate::tuning::candidate::CandidateSource;
use crate::tuning::project_candidate::{project_candidate, served_seed};
use crate::tuning::structure::ParallelGains;
use crate::validation::isolated_run_protocol::{
    IsolatedRunConditions, IsolatedRunRecord, IsolatedRunSample, IsolatedRunValue,
};

/// A channel's value at a sample: a value of the given quality, or none.
fn sample_value(value: Option<f64>, quality: Quality) -> IsolatedRunValue {
    IsolatedRunValue { value, quality }
}

/// The record of a run whose samples hold the given channels' values, in order. The entry case reads only the samples.
fn run_record(samples: Vec<Vec<(&str, IsolatedRunValue)>>) -> IsolatedRunRecord {
    IsolatedRunRecord {
        id: "run-0".to_owned(),
        evidence: RunEvidence::StExecuted,
        parameters: BTreeMap::new(),
        conditions: IsolatedRunConditions {
            initial_state: BTreeMap::new(),
            events: Vec::new(),
            duration_seconds: 1.0,
            sample_times_seconds: Vec::new(),
            seed: 1,
            model_revision: "model:1".to_owned(),
            program_revision: "sha256:program".to_owned(),
            parameter_schema_revision: "sha256:schema".to_owned(),
            record_every: None,
            model_provenance: None,
            simulation_config_revision: None,
            feedback_relay: None,
        },
        channels: Vec::new(),
        samples: samples
            .into_iter()
            .zip(0_u32..)
            .map(|(values, index)| IsolatedRunSample {
                time_seconds: 0.1 * f64::from(index),
                sequence: u64::from(index),
                values: values
                    .into_iter()
                    .map(|(symbol, value)| (symbol.to_owned(), value))
                    .collect(),
            })
            .collect(),
    }
}

#[test]
fn the_mode_the_manual_command_and_a_state_take_the_units_the_loop_declares() {
    let loop_definition = parallel_pid_loop();
    let mode = loop_definition
        .parameter_binding
        .as_ref()
        .expect("the fixture binds its parameters")
        .mode
        .symbol
        .clone();
    let manual = loop_definition
        .controller
        .manual_output_signal
        .clone()
        .expect("the fixture names its Manual command");
    let states = vec![("Main.Integral".to_owned(), "V".to_owned())];
    assert_eq!(
        undeclared_unit(&loop_definition, &states, &mode),
        Some("1".to_owned()),
        "the mode is dimensionless"
    );
    assert_eq!(
        undeclared_unit(&loop_definition, &states, &manual),
        Some(loop_definition.signals.manipulated_variable.unit.clone()),
        "the Manual command has the unit of the output"
    );
    assert_eq!(
        undeclared_unit(&loop_definition, &states, "Main.Integral"),
        Some("V".to_owned()),
        "a state has the unit its declaration gives"
    );
    assert_eq!(
        undeclared_unit(&loop_definition, &states, "Main.Unknown"),
        None,
        "a channel that the loop declares nowhere has no unit"
    );
}

#[test]
fn a_channel_that_a_signal_names_keeps_the_job_builders_unit_and_the_others_are_extra() {
    let loop_definition = parallel_pid_loop();
    let process = loop_definition.signals.process_variable.symbol.clone();
    let mode = loop_definition
        .parameter_binding
        .as_ref()
        .expect("the fixture binds its parameters")
        .mode
        .symbol
        .clone();
    let (observed, extra) =
        split_observations(&loop_definition, &[process.clone(), mode.clone()], &[])
            .expect("both channels have a unit");
    assert_eq!(
        observed,
        vec![process],
        "the measurement keeps its signal's unit"
    );
    assert_eq!(extra.len(), 1);
    assert_eq!(extra[0].symbol, mode);
    assert_eq!(extra[0].unit, "1");
}

#[test]
fn the_start_model_takes_the_first_sample_of_the_configured_plant_as_its_operating_point() {
    let loop_definition = parallel_pid_loop();
    let model = first_order_model();
    let run = run_record(vec![
        vec![
            ("LevelPV", sample_value(Some(20.0), Quality::Good)),
            ("ValveMV", sample_value(Some(42.5), Quality::Good)),
        ],
        vec![
            ("LevelPV", sample_value(Some(21.0), Quality::Good)),
            ("ValveMV", sample_value(Some(40.0), Quality::Good)),
        ],
    ]);
    let start = entry_start_model(&loop_definition, &model, Some(&run));
    assert_eq!(
        start.identity().operating_point,
        Some(OperatingPoint {
            inputs: vec![42.5],
            outputs: vec![20.0],
        }),
        "the start is the first sample's plant input and measured output, not a later sample's"
    );
    assert_eq!(
        start.identity().revision,
        model.identity().revision,
        "the start keeps the model's revision"
    );
    assert_eq!(
        model.identity().operating_point,
        Some(OperatingPoint {
            inputs: vec![0.0],
            outputs: vec![2.0],
        }),
        "the model keeps its own operating point"
    );
}

#[test]
fn a_start_without_its_first_sample_or_one_of_its_two_values_keeps_the_models_operating_point() {
    let loop_definition = parallel_pid_loop();
    let model = first_order_model();
    let start_point = |start: Option<&IsolatedRunRecord>| {
        entry_start_model(&loop_definition, &model, start)
            .identity()
            .operating_point
            .clone()
    };
    let point = model.identity().operating_point.clone();
    assert_eq!(
        start_point(None),
        point,
        "the model plant gives no start run"
    );
    assert_eq!(
        start_point(Some(&run_record(Vec::new()))),
        point,
        "a run without samples keeps the model's point"
    );
    let output_only = vec![("LevelPV", sample_value(Some(20.0), Quality::Good))];
    let without_input = run_record(vec![output_only]);
    assert_eq!(
        start_point(Some(&without_input)),
        point,
        "a run without the plant input keeps the model's point"
    );
    let bad_input = run_record(vec![vec![
        ("LevelPV", sample_value(Some(20.0), Quality::Good)),
        ("ValveMV", sample_value(Some(42.5), Quality::Bad)),
    ]]);
    assert_eq!(
        start_point(Some(&bad_input)),
        point,
        "a plant input that is no measurement keeps the model's point"
    );
    let missing_output = run_record(vec![vec![
        ("LevelPV", sample_value(None, Quality::Good)),
        ("ValveMV", sample_value(Some(42.5), Quality::Good)),
    ]]);
    assert_eq!(
        start_point(Some(&missing_output)),
        point,
        "a measured output without a value keeps the model's point"
    );
}

/// A first-order model `1/(s + 1)` in percent, operating at `operating_input` and `operating_output`.
fn model_at(operating_input: f64, operating_output: f64) -> GeneralModel {
    decode_general_model(&json!({
        "kind": "transfer-function", "revision": "model:operating", "source": "provided",
        "validation": "validated", "evidence": ["bench"], "delaySeconds": 0,
        "numerator": [1], "denominator": [1, 1],
        "inputs": [{ "id": "u", "unit": "%", "operatingRange": { "minimum": 0, "maximum": 100 } }],
        "outputs": [{ "id": "y", "unit": "%", "operatingRange": { "minimum": 0, "maximum": 100 } }],
        "operatingPoint": { "inputs": [operating_input], "outputs": [operating_output] }
    }))
    .expect("the fixture model decodes")
}

/// The value that the entry case sets `symbol` to at its transfer, when the case sets it.
fn event_value(entry: &EntryCase, symbol: &str) -> Option<f64> {
    entry
        .case
        .events
        .iter()
        .find(|event| event.symbol == symbol)
        .map(|event| event.value)
}

#[test]
fn a_program_pid_candidate_enters_through_its_declared_states_and_runs_with_its_seed_values() {
    // C2 Validate: the entry case is the declared-state plan. The integral term starts at the command the model holds (3 %) and is
    // written with the entry, and the states it holds after the transfer are required (`initialized-state:`). The run takes the seed's
    // values as overrides, with an explicit zero for the derivative variable that a PI candidate omits.
    let loop_definition = project_pid_loop();
    let model = model_at(3.0, 4.0);
    let gains = ControllerGains {
        proportional_gain: Some(2.0),
        integral_gain: Some(0.5),
        derivative_gain: Some(0.0),
        ..ControllerGains::default()
    };
    let values = controller_adapter(&loop_definition)
        .expect("the served law")
        .materialize(&gains, &BTreeMap::new())
        .expect("the seed's values");
    let entry = plan_entry_case(&loop_definition, &model, &requirements(), &values, 0.0)
        .expect("the entry case plans");
    let required: Vec<&str> = entry
        .case
        .requirements
        .iter()
        .map(|requirement| requirement.id.as_str())
        .collect();
    assert!(
        required.contains(&"initialized-state:Main.Program.Integral"),
        "{required:?}"
    );
    assert!(
        required.contains(&"initialized-state:Main.Program.PreviousError"),
        "{required:?}"
    );
    assert_eq!(
        event_value(&entry, "Main.Program.Integral"),
        Some(3.0),
        "the integral term starts at the output at the switch to Auto"
    );
    let candidate = LawCandidate::Project(project_candidate(
        served_seed(&loop_definition).expect("the served seed"),
        CandidateSource::Simc,
        ParallelGains {
            proportional_gain: 2.0,
            integral_gain: 0.5,
            derivative_gain: 0.0,
        },
    ));
    let request = entry
        .request(
            &loop_definition,
            &model,
            &candidate,
            None,
            "entry-program-pid",
            "sha256:project",
        )
        .expect("the entry request of the program's PID candidate");
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
fn a_program_p_law_enters_through_the_connected_p_and_writes_its_flag_with_the_reference() {
    // C13 and C1: a program's P law enters through the connected P. Its reference is r = y + u/Kp (2 % + 3 % / 2 = 3.5 %), and the same
    // transfer writes the startup-done flag TRUE. The run observes the flag with the unit its declaration gives.
    let loop_definition = project_p_loop_with_startup();
    let model = model_at(3.0, 2.0);
    let gains = ControllerGains {
        proportional_gain: Some(2.0),
        ..ControllerGains::default()
    };
    let values = controller_adapter(&loop_definition)
        .expect("the served P law")
        .materialize(&gains, &BTreeMap::new())
        .expect("the connected P's value");
    let entry = plan_entry_case(&loop_definition, &model, &requirements(), &values, 0.0)
        .expect("the entry case plans");
    assert_eq!(
        event_value(&entry, "LevelSP"),
        Some(3.5),
        "the reference is r = y + u/Kp (C13)"
    );
    assert_eq!(
        event_value(&entry, "Main.Program.Started"),
        Some(1.0),
        "the flag is written TRUE with the reference"
    );
    assert!(
        entry.extra_observations.iter().any(|observation| {
            observation.symbol == "Main.Program.Started" && observation.unit == "1"
        }),
        "{:?}",
        entry.extra_observations
    );
    assert!(entry
        .case
        .requirements
        .iter()
        .any(|requirement| requirement.id == "initialized-state:Main.Program.Started"));
}
