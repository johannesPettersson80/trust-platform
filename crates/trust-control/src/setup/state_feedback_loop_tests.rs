//! Tests of the state-feedback generator (`stateFeedbackLoopGenerator.ts`): the loop assertions of
//! `control-studio-loop-setup-state-feedback.test.ts` on its two-tank program, the refusals of the setup form (`loopSetupForm.ts`), the
//! startup-done rule (C1 Setup), and the decode of the written loop by the loop contract and by the state controller's record. The gains
//! themselves are designed by Tune (C4), and are not tested here.

#![allow(clippy::float_cmp)]

use serde_json::{json, Value};

use crate::failure::Failure;
use crate::json::decode_value;
use crate::loop_definition::{AuxiliaryRole, ExperimentKind, LoopDefinition, LoopScope};
use crate::setup::catalog::{decode_catalog_answer, CompiledCatalog};
use crate::setup::state_feedback_loop::{state_feedback_loop_definition, StateFeedbackLoopDraft};
use crate::tuning::state_controller::{decode_state_controller_binding, StateControllerFamily};

fn scope() -> LoopScope {
    LoopScope {
        project_id: "project:setup".to_string(),
        runtime_id: "runtime-1".to_string(),
    }
}

/// The sentence a refusal shows: its whole text, which is the detail when it says more than its first sentence.
fn text_of(failure: &Failure) -> String {
    failure
        .detail
        .clone()
        .unwrap_or_else(|| failure.message.clone())
}

#[allow(clippy::needless_pass_by_value)] // A test fixture: it takes the JSON literals its callers write in place.
fn decode(control: Value) -> CompiledCatalog {
    decode_catalog_answer(&json!({ "ok": true, "processImage": {}, "controlSymbols": control }))
        .expect("the compiled answer decodes")
        .control
}

/// The two tanks of `control-studio-loop-setup-state-feedback.test.ts`: the program measures tank 2 and tank 1 and feeds both back.
fn tanks_json() -> Value {
    json!({
        "symbols": [
            { "symbol": "P1.Tank1M", "type": "LREAL", "address": "%IL0" },
            { "symbol": "P1.Tank2M", "type": "LREAL", "address": "%IL8" },
            { "symbol": "P1.PumpM3PerS", "type": "LREAL", "address": "%QL0" },
            { "symbol": "P1.Tank2SetpointM", "type": "LREAL", "address": "%ML0" },
            { "symbol": "P1.ManualM3PerS", "type": "LREAL", "address": "%ML8" },
            { "symbol": "P1.FeedbackEnabled", "type": "BOOL" },
            { "symbol": "P1.K1", "type": "LREAL" },
            { "symbol": "P1.K2", "type": "LREAL" },
            { "symbol": "P1.N", "type": "LREAL" }
        ],
        "controllers": [
            { "instance": "P1", "type": "Main", "tasks": [{ "name": "MainTask", "intervalMs": 1000 }] }
        ],
        "links": [],
        "writes": ["P1.PumpM3PerS"]
    })
}

fn tanks() -> CompiledCatalog {
    decode(tanks_json())
}

/// The pole-placement or LQR draft of the two tanks: tank 2 is controlled, tank 1 is fed back, and each has its gain variable.
fn lqr_json() -> Value {
    json!({
        "name": "Tank 2 level",
        "action": "direct",
        "family": "LQR",
        "instance": "P1",
        "manipulatedVariable": { "symbol": "P1.PumpM3PerS", "unit": "m3/s", "minimum": 0, "maximum": 0.02 },
        "processVariable": { "symbol": "P1.Tank2M", "unit": "m", "minimum": 0, "maximum": 0.5 },
        "setpoint": { "symbol": "P1.Tank2SetpointM" },
        "gain": "P1.K2",
        "referenceGain": "P1.N",
        "states": [{ "signal": { "symbol": "P1.Tank1M", "unit": "m", "minimum": 0, "maximum": 0.5 }, "gain": "P1.K1" }],
        "mode": { "symbol": "P1.FeedbackEnabled", "whenTrue": "Auto" },
        "manualOutput": "P1.ManualM3PerS",
        "experimentStep": 0.002,
        "experimentDurationSeconds": 300
    })
}

fn draft_of(value: Value) -> StateFeedbackLoopDraft {
    decode_value(value).expect("the draft decodes")
}

fn write(value: Value) -> LoopDefinition {
    state_feedback_loop_definition(&tanks(), &scope(), &draft_of(value), None)
        .expect("the draft is written")
}

/// The refusal a draft gets on the two tanks, as its whole sentence.
fn refusal(value: Value) -> String {
    let failure = state_feedback_loop_definition(&tanks(), &scope(), &draft_of(value), None)
        .expect_err("the draft is refused");
    text_of(&failure)
}

/// The draft with one edit applied to it.
fn edited(edit: impl FnOnce(&mut Value)) -> Value {
    let mut value = lqr_json();
    edit(&mut value);
    value
}

#[test]
fn the_pole_placement_or_lqr_draft_writes_a_loop_on_the_measured_outputs() {
    // control-studio-loop-setup-state-feedback.test.ts: "writes a state-feedback loop whose gains belong to measured outputs".
    let loop_definition = write(lqr_json());
    assert_eq!(loop_definition.id, "loop:tank-2-level");
    loop_definition
        .validate(&scope())
        .expect("the loop passes the checks that need no runtime");
    let channels = loop_definition
        .channels
        .as_ref()
        .expect("a state law carries its channels");
    let outputs: Vec<(&str, &str, Option<&str>)> = channels
        .outputs
        .iter()
        .map(|output| {
            (
                output.model_channel_id.as_str(),
                output.signal_id.as_str(),
                output.setpoint_signal_id.as_deref(),
            )
        })
        .collect();
    assert_eq!(
        outputs,
        [
            ("P1.Tank2M", "processVariable", Some("setpoint")),
            ("P1.Tank1M", "state-2", None)
        ],
        "the controlled output, then the further output as a state"
    );
    let record = loop_definition
        .controller
        .state_feedback
        .as_ref()
        .expect("a state law");
    assert_eq!(record["family"], "LQR");
    assert_eq!(
        record["gainSymbols"],
        json!([["P1.K2", "P1.K1"]]),
        "one gain per measured output, the controlled one first"
    );
    assert_eq!(record["referenceGainSymbols"], json!([["P1.N"]]));
    let policy = loop_definition
        .identification_policy
        .as_ref()
        .expect("an identification policy");
    assert_eq!(
        policy["maximumOrder"],
        json!(1),
        "each measured output is one first-order state"
    );
    let experiment = loop_definition
        .experiment
        .as_ref()
        .expect("both experiment values are set");
    assert_eq!(
        (
            experiment.kind,
            experiment.amplitude,
            experiment.duration_seconds
        ),
        (ExperimentKind::MvStep, 0.002, Some(300.0))
    );
    let additional = loop_definition
        .signals
        .additional
        .as_ref()
        .expect("the further output is a signal");
    assert_eq!(
        (additional[0].id.as_str(), additional[0].role),
        ("state-2", AuxiliaryRole::State)
    );
}

#[test]
fn the_gains_carry_the_input_unit_per_the_output_unit_with_the_limit_of_their_range() {
    // stateFeedbackLoopGenerator.ts: `coefficient` takes the ratio of the input's unit to the output's, and 1000 times the input's span
    // per the output's span as its limit. Here 1000 · 0.02 / 0.5 = 40 for every gain.
    let loop_definition = write(lqr_json());
    let parameters: Vec<(&str, &str, f64, f64)> = loop_definition
        .parameter_binding
        .as_ref()
        .expect("a parameter record")
        .parameters
        .iter()
        .map(|parameter| {
            (
                parameter.symbol.as_str(),
                parameter.unit.as_str(),
                parameter.minimum,
                parameter.maximum,
            )
        })
        .collect();
    assert_eq!(
        parameters,
        [
            ("P1.K2", "m3/s/m", -40.0, 40.0),
            ("P1.K1", "m3/s/m", -40.0, 40.0),
            ("P1.N", "m3/s/m", -40.0, 40.0)
        ]
    );
}

#[test]
fn the_written_loop_decodes_and_the_state_controller_reads_its_record() {
    // The loop contract reads the file back (`LoopDefinition::decode`), the engine serves the law (`rust_engine_support`), and the
    // state controller's record decodes against the loop's parameter record.
    let loop_definition = write(lqr_json());
    let text = serde_json::to_string(&loop_definition).expect("the loop serializes");
    let decoded = LoopDefinition::decode(&text).expect("the written loop decodes");
    assert_eq!(decoded.revision, loop_definition.revision);
    decoded
        .rust_engine_support()
        .expect("the engine serves a state-feedback law with channels");
    let record = decoded
        .controller
        .state_feedback
        .as_ref()
        .expect("a state law");
    let binding = decode_state_controller_binding(record, decoded.parameter_binding.as_ref())
        .expect("the state controller reads the record");
    assert_eq!(binding.family, StateControllerFamily::Lqr);
    assert_eq!(
        binding.gain_symbols,
        vec![vec!["P1.K2".to_string(), "P1.K1".to_string()]]
    );
    assert_eq!(
        binding.reference_gain_symbols,
        vec![vec!["P1.N".to_string()]]
    );
}

#[test]
fn a_pole_placement_draft_writes_the_same_loop_with_its_family() {
    // The family of the form is `state-feedback` for pole placement; the loop file names it so, and the record decodes as that family.
    let loop_definition = write(edited(|value| value["family"] = json!("state-feedback")));
    let record = loop_definition
        .controller
        .state_feedback
        .as_ref()
        .expect("a state law");
    assert_eq!(record["family"], "state-feedback");
    let decoded = serde_json::to_string(&loop_definition).expect("the loop serializes");
    LoopDefinition::decode(&decoded).expect("the written loop decodes");
}

#[test]
fn a_draft_that_misses_a_choice_names_it_in_the_form_s_sentence() {
    // loopSetupForm.ts, the state-feedback draft: each choice the form checks, in the order it checks them.
    assert_eq!(
        refusal(edited(|value| value["name"] = json!(""))),
        "Enter a loop name."
    );
    assert_eq!(
        refusal(edited(|value| value["instance"] = json!(""))),
        "Choose the instance that runs the state-feedback law."
    );
    assert_eq!(
        refusal(edited(|value| value["processVariable"]["unit"] = json!(""))),
        "Enter the controlled output unit."
    );
    assert_eq!(
        refusal(edited(
            |value| value["processVariable"]["maximum"] = json!(0)
        )),
        "The controlled output maximum must be above its minimum."
    );
    assert_eq!(
        refusal(edited(|value| value["setpoint"]["symbol"] = json!(""))),
        "Choose the setpoint."
    );
    assert_eq!(
        refusal(edited(|value| value["gain"] = json!(""))),
        "Choose the variable holding the gain on P1.Tank2M."
    );
    assert_eq!(
        refusal(edited(|value| value["referenceGain"] = json!(""))),
        "Choose the variable holding the reference gain."
    );
    assert_eq!(
        refusal(edited(|value| value["states"] = json!([]))),
        "Choose at least one further measured output: state feedback needs every state it feeds back."
    );
    assert_eq!(
        refusal(edited(|value| value["states"][0]["gain"] = json!(""))),
        "Choose the variable holding the gain on P1.Tank1M."
    );
    assert_eq!(
        refusal(edited(|value| value["mode"]["symbol"] = json!(""))),
        "Choose the Auto/Manual signal."
    );
    assert_eq!(
        refusal(edited(|value| value["manualOutput"] = json!(""))),
        "Choose the manual output."
    );
}

#[test]
fn a_refused_choice_names_the_form_field_it_is_shown_under() {
    // The page shows a refusal under the field it names: the choice's own key, or the signal's unit or bound.
    let refused = |value: Value| {
        state_feedback_loop_definition(&tanks(), &scope(), &draft_of(value), None)
            .expect_err("the draft is refused")
    };
    assert_eq!(
        refused(edited(|value| value["gain"] = json!("")))
            .field
            .as_deref(),
        Some("feedback.gain")
    );
    assert_eq!(
        refused(edited(|value| value["processVariable"]["unit"] = json!("")))
            .field
            .as_deref(),
        Some("feedback.processVariable.unit")
    );
    assert_eq!(
        refused(edited(
            |value| value["processVariable"]["maximum"] = json!(0)
        ))
        .field
        .as_deref(),
        Some("feedback.processVariable.maximum")
    );
}

#[test]
fn a_draft_names_an_instance_the_program_does_not_compile_and_too_many_further_outputs() {
    assert_eq!(
        refusal(edited(|value| value["instance"] = json!("P1.Missing"))),
        "P1.Missing is not a compiled program or function block instance."
    );
    let state = lqr_json()["states"][0].clone();
    let four = json!([state.clone(), state.clone(), state.clone(), state]);
    assert_eq!(
        refusal(edited(|value| value["states"] = four)),
        "State feedback needs one to three measured outputs besides the controlled output."
    );
}

#[test]
fn a_program_that_writes_a_seeded_value_needs_the_startup_flag_the_draft_names() {
    // loopSetupForm.ts's seededWrite and startupFlag, which C1 Setup requires: the program writes the setpoint itself.
    let mut catalog_json = tanks_json();
    catalog_json["writes"] = json!(["P1.PumpM3PerS", "P1.Tank2SetpointM"]);
    let catalog = decode(catalog_json);
    let failure = state_feedback_loop_definition(&catalog, &scope(), &draft_of(lqr_json()), None)
        .expect_err("the startup flag is needed");
    assert_eq!(
        text_of(&failure),
        "The program writes P1.Tank2SetpointM in its own code. Choose the flag that marks its startup as done, so validation keeps the values it starts from."
    );
    let with_flag = edited(|value| value["startupFlag"] = json!("P1.FeedbackEnabled"));
    let loop_definition =
        state_feedback_loop_definition(&catalog, &scope(), &draft_of(with_flag), None)
            .expect("the named flag passes");
    let record = loop_definition
        .controller
        .state_feedback
        .as_ref()
        .expect("a state law");
    assert_eq!(
        record["initialization"]["states"][0]["symbol"], "P1.FeedbackEnabled",
        "the flag is a declared state of the law, starting at TRUE"
    );
    assert_eq!(record["initialization"]["states"][0]["initial"], json!(1.0));
}
