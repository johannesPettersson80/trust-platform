//! Tests of `controlStudio/inputs` and of what `controlStudio/open` answers for the recorded inputs (specification 35, A8.1, A8.4,
//! A8.6 and §0.1a). They use the example loop, a closed loop with a setpoint, and no runtime.

use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

use serde_json::{json, Value};
use trust_control::failure::NextAction;
use trust_control::identification::acquisition_settings::RangeSetting;
use trust_control::loop_definition::LoopDefinition;
use trust_control::messages::{InputsRequest, OperationUpdate, TuneMethod};
use trust_control::study::inputs::InputsSection;
use trust_control::study::tune::state::StateDesignInput;
use trust_control::tuning::objective::TuningMethod;
use trust_control::tuning::requirements::TuningRequirements;
use trust_control::tuning::state_controller::preferences::StateDesignDraft;

use super::answer;
use crate::busy_loops::LoopClaims;
use crate::commands::{ControlStudio, OPEN};
use crate::operation::UpdateSink;
use crate::storage;

const EXAMPLE: &str = include_str!("../../../examples/control-pid-commissioning/control-loop.json");
/// Lesson 69's state-feedback loop, the LQR law of the course.
const LESSON_69_LOOP: &str = include_str!(
    "../../../examples/control/11-state-space-estimation-and-predictive-control/69-lqr/control-loop.json"
);

struct Ignore;

impl UpdateSink for Ignore {
    fn send(&self, _update: OperationUpdate) {}
}

/// A project folder holding the example loop, removed when the test ends.
struct Project {
    root: PathBuf,
    loop_definition: LoopDefinition,
}

impl Project {
    fn new(name: &str) -> Self {
        Self::with_loop(name, EXAMPLE)
    }

    /// A project folder holding the loop file `text`.
    fn with_loop(name: &str, text: &str) -> Self {
        let root = std::env::temp_dir().join(format!(
            "trust-control-studio-{name}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).expect("project");
        fs::write(root.join("control-loop.json"), text).expect("loop");
        let loop_definition = LoopDefinition::decode(text).expect("loop");
        Self {
            root,
            loop_definition,
        }
    }

    fn inputs_request(&self, inputs: InputsSection) -> InputsRequest {
        InputsRequest {
            project_root: self.root.display().to_string(),
            runtime_id: self.loop_definition.runtime_id.clone(),
            loop_id: self.loop_definition.id.clone(),
            inputs: Some(inputs),
            change: None,
            channel_selection: None,
            storage_folder: Some(self.storage_folder()),
        }
    }

    /// The request of one change of the page, as the page sends it (the engine merges it into the recorded inputs).
    fn change_request(&self, change: &Value) -> InputsRequest {
        InputsRequest {
            project_root: self.root.display().to_string(),
            runtime_id: self.loop_definition.runtime_id.clone(),
            loop_id: self.loop_definition.id.clone(),
            inputs: None,
            change: change.as_object().cloned(),
            channel_selection: None,
            storage_folder: Some(self.storage_folder()),
        }
    }

    /// The folder of the project's operation journals (A8.3).
    fn storage_folder(&self) -> String {
        self.root.join("storage").display().to_string()
    }

    fn open_request(&self) -> Value {
        json!({
            "projectRoot": self.root.display().to_string(),
            "runtimeId": self.loop_definition.runtime_id,
            "loopId": self.loop_definition.id,
            "storageFolder": self.storage_folder(),
        })
    }
}

impl Drop for Project {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

/// Requirements that are valid and that differ by their change: `change` is the setpoint change of the test.
fn requirements(change: f64) -> TuningRequirements {
    TuningRequirements {
        channel_requirements: None,
        reference_change: change,
        maximum_error: 0.4,
        settling_band: 0.1,
        settling_time_seconds: 45.0,
        duration_seconds: 90.0,
        constant_input_disturbance: None,
        measurement_noise_band: None,
        gain_uncertainty_fraction: None,
        transfer_tolerance: None,
        maximum_overshoot_percent: None,
        tuning_method: None,
        tuning_objective: None,
        automatic_criteria: None,
    }
}

fn with_requirements(change: f64) -> InputsSection {
    InputsSection {
        requirements: Some(requirements(change)),
        ..InputsSection::default()
    }
}

#[test]
fn a_state_law_answers_its_form_fields_before_its_first_tune_on_inputs_and_open() {
    let project = Project::with_loop("state-fields", LESSON_69_LOOP);
    // The finite horizon is the stored solver policy: the horizon steps and the terminal weights join the fields.
    let inputs = InputsSection {
        state_design: Some(StateDesignInput {
            draft: Some(StateDesignDraft {
                solver_mode: Some("finite".to_owned()),
                ..StateDesignDraft::default()
            }),
            ..StateDesignInput::default()
        }),
        ..InputsSection::default()
    };
    let answered =
        answer(&LoopClaims::default(), project.inputs_request(inputs)).expect("recorded");
    // B4.2, C4: the solver policy is a choice, so its field lists the two options with their words and names the one that stands, which
    // is the stored finite horizon here. A field without them leaves the page to list the options and to start at the infinite horizon
    // itself, which is the default the page invented before the engine said which option stands. The other fields are text and name none.
    let expected = json!([
        {
            "field": "solverMode", "label": "Solver policy", "selected": "finite",
            "choices": [
                {"value": "infinite", "label": "Infinite-horizon / steady-state"},
                {"value": "finite", "label": "Finite-horizon (fixed policy)"}
            ]
        },
        {"field": "stateWeights", "label": "State weights (Q)"},
        {"field": "inputWeights", "label": "Input weights (R)"},
        {"field": "horizonSteps", "label": "Finite horizon steps"},
        {"field": "terminalWeights", "label": "Terminal weights"}
    ]);
    assert_eq!(answered["stateFields"], expected, "{answered}");
    // Open answers the same fields for the inputs the study holds.
    let studio = ControlStudio::new(Arc::new(Ignore));
    let opened = studio
        .execute(OPEN, &[project.open_request()])
        .expect("answered");
    assert_eq!(opened["result"]["stateFields"], expected, "{opened}");
}

#[test]
fn a_loop_without_a_state_law_answers_no_form_fields() {
    let project = Project::new("no-state-fields");
    let answered = answer(
        &LoopClaims::default(),
        project.inputs_request(InputsSection::default()),
    )
    .expect("recorded");
    assert!(answered.get("stateFields").is_none(), "{answered}");
    let studio = ControlStudio::new(Arc::new(Ignore));
    let opened = studio
        .execute(OPEN, &[project.open_request()])
        .expect("answered");
    assert!(opened["result"].get("stateFields").is_none(), "{opened}");
}

#[test]
fn recorded_inputs_are_answered_by_open_and_the_stages_are_answered_at_once() {
    let project = Project::new("inputs-open");
    let inputs = InputsSection {
        method: Some(TuneMethod::Simc),
        ..InputsSection::default()
    };
    let answered =
        answer(&LoopClaims::default(), project.inputs_request(inputs)).expect("recorded");
    assert_eq!(
        answered["invalidated"]["validation"],
        json!(false),
        "no requirements were held before or after: nothing is invalidated"
    );
    assert!(answered["stages"].is_array(), "{answered}");
    // The page hides a Validate table by this field, so the answer carries it under its camelCase name (A8.4).
    assert!(
        answered["stageEvidence"]["validationHasResult"].is_boolean(),
        "{answered}"
    );
    let studio = ControlStudio::new(Arc::new(Ignore));
    let opened = studio
        .execute(OPEN, &[project.open_request()])
        .expect("answered");
    assert_eq!(
        opened["result"]["inputs"]["method"],
        json!("SIMC"),
        "{opened}"
    );
    assert_eq!(opened["result"]["page"], json!("Identify"), "{opened}");
}

#[test]
fn a_change_of_the_requirements_is_reported_as_invalidating_the_validations() {
    let project = Project::new("inputs-invalidate");
    let claims = LoopClaims::default();
    let first = answer(&claims, project.inputs_request(with_requirements(2.0))).expect("recorded");
    assert_eq!(
        first["invalidated"]["validation"],
        json!(true),
        "requirements recorded where none were held are a change"
    );
    let same = answer(&claims, project.inputs_request(with_requirements(2.0))).expect("recorded");
    assert_eq!(
        same["invalidated"]["validation"],
        json!(false),
        "the same requirements invalidate nothing"
    );
    let changed =
        answer(&claims, project.inputs_request(with_requirements(3.0))).expect("recorded");
    assert_eq!(changed["invalidated"]["validation"], json!(true));
    // The Validate page says why in the engine's words (A8.4, B9.9), and an answer that invalidates nothing carries none.
    assert_eq!(
        changed["invalidated"]["reason"],
        json!("Inputs changed. Tune and verify before applying.")
    );
    assert_eq!(
        first["invalidated"]["reason"],
        changed["invalidated"]["reason"]
    );
    assert!(
        same["invalidated"].get("reason").is_none(),
        "{}",
        same["invalidated"]
    );
}

#[test]
fn the_inputs_cannot_change_while_an_operation_of_the_loop_runs() {
    let project = Project::new("inputs-busy");
    let claims = LoopClaims::default();
    let busy = claims
        .claim(
            &project.loop_definition.project_id,
            &project.loop_definition.id,
            "Identify plant",
        )
        .expect("claimed");
    assert!(
        answer(&claims, project.inputs_request(InputsSection::default())).is_err(),
        "the page's inputs are read-only while the operation runs"
    );
    drop(busy);
    assert!(
        answer(&claims, project.inputs_request(InputsSection::default())).is_ok(),
        "once the operation ends, the inputs change again"
    );
}

#[test]
fn an_invalid_requirement_is_refused_with_its_next_action_and_nothing_is_recorded() {
    let project = Project::new("inputs-invalid");
    let mut bad = requirements(2.0);
    bad.settling_band = 0.4;
    let refused = answer(
        &LoopClaims::default(),
        project.inputs_request(InputsSection {
            requirements: Some(bad),
            ..InputsSection::default()
        }),
    )
    .expect_err("a settling band not below the error limit");
    assert_eq!(refused.next_action, NextAction::OpenRequirements);
    let study = storage::open_study(
        &project.root,
        &project.loop_definition.id,
        &project.loop_definition.revision,
    )
    .expect("the study");
    assert_eq!(
        study.section::<InputsSection>().expect("read"),
        None,
        "a refused section is not kept"
    );
}

#[test]
fn a_change_answers_the_merged_inputs_and_whether_they_changed_and_the_study_keeps_them() {
    let project = Project::new("change-answer");
    let claims = LoopClaims::default();
    let first = answer(&claims, project.change_request(&json!({ "family": "PI" })))
        .expect("the change is merged");
    assert_eq!(first["inputs"]["structure"], "PI", "{first}");
    assert_eq!(first["changed"], true, "{first}");
    // The same change again merges into the same inputs: nothing changed, so nothing is written.
    let again = answer(&claims, project.change_request(&json!({ "family": "PI" })))
        .expect("the same change is merged");
    assert_eq!(again["changed"], false, "{again}");
    assert_eq!(again["inputs"]["structure"], "PI", "{again}");
    // The study keeps the merged inputs, which the next change merges into (A8.1).
    let study = storage::open_study(
        &project.root,
        &project.loop_definition.id,
        &project.loop_definition.revision,
    )
    .expect("the study");
    assert_eq!(
        study
            .section::<InputsSection>()
            .expect("read")
            .and_then(|inputs| inputs.structure),
        Some(trust_control::tuning::structure::Structure::PI)
    );
    // A key the study does not take is refused by name, and nothing is merged.
    let refused = answer(
        &claims,
        project.change_request(&json!({ "gainSchedule": 1 })),
    )
    .expect_err("the study takes no gainSchedule input");
    assert_eq!(
        refused.message,
        "The change names an input that the study does not take: gainSchedule."
    );
}

#[test]
fn a_request_names_the_whole_inputs_or_one_change_and_neither_or_both_is_refused() {
    let project = Project::new("both-or-neither");
    let change = project.change_request(&json!({ "family": "PI" }));
    let both = InputsRequest {
        inputs: Some(InputsSection::default()),
        ..change.clone()
    };
    let neither = InputsRequest {
        change: None,
        ..change
    };
    for request in [both, neither] {
        let refused = answer(&LoopClaims::default(), request)
            .expect_err("the request names one of its two forms");
        assert_eq!(
            refused.message,
            "The request names either the whole inputs or one change of them, not both and not neither."
        );
    }
    let recorded = storage::open_study(
        &project.root,
        &project.loop_definition.id,
        &project.loop_definition.revision,
    )
    .ok()
    .and_then(|study| study.section::<InputsSection>().ok().flatten());
    assert_eq!(recorded, None, "a refused request records nothing");
}

#[test]
fn the_request_decodes_as_the_page_writes_it_and_refuses_an_unknown_field() {
    let request = json!({
        "projectRoot": "/project",
        "runtimeId": "simulator",
        "loopId": "control-pid-commissioning",
        "inputs": {
            "method": "SIMC",
            "acquisition": {
                "method": "automatic",
                "durationSeconds": 30.0,
                "amplitude": 0.0,
                "reuseRecordings": false,
            },
        },
    });
    let decoded: InputsRequest =
        trust_control::json::decode_value(request.clone()).expect("the page's request");
    let section = decoded.inputs.as_ref().expect("the whole inputs");
    assert_eq!(section.method, Some(TuneMethod::Simc));
    assert!(section.acquisition.is_some());
    let mut unknown = request;
    unknown["bogus"] = json!(1);
    assert!(
        trust_control::json::decode_value::<InputsRequest>(unknown).is_err(),
        "a field the request does not know is refused (deny_unknown_fields)"
    );
}

#[test]
fn a_requirement_keeps_no_tuning_method_of_its_own_and_the_method_is_the_inputs_one() {
    let project = Project::new("inputs-one-method");
    let mut with_method = requirements(2.0);
    with_method.tuning_method = Some(TuningMethod::Simc);
    answer(
        &LoopClaims::default(),
        project.inputs_request(InputsSection {
            method: Some(TuneMethod::Simc),
            requirements: Some(with_method),
            ..InputsSection::default()
        }),
    )
    .expect("recorded");
    let studio = ControlStudio::new(Arc::new(Ignore));
    let opened = studio
        .execute(OPEN, &[project.open_request()])
        .expect("answered");
    assert_eq!(
        opened["result"]["inputs"]["method"],
        json!("SIMC"),
        "{opened}"
    );
    assert!(
        opened["result"]["inputs"]["requirements"]["tuningMethod"].is_null(),
        "the requirement's copy of the method is not recorded (A8.1): {opened}"
    );
    assert_eq!(
        opened["result"]["inputs"]["requirements"]["referenceChange"],
        json!(2.0),
        "{opened}"
    );
}

#[test]
fn an_acquisition_range_beyond_the_loops_limits_is_refused_under_the_field_that_holds_it() {
    // Lesson 69's loop has one input and two measured outputs, so Limits names each range field after its signal (B3.2).
    let project = Project::with_loop("inputs-acquisition-range", LESSON_69_LOOP);
    let input = project
        .loop_definition
        .signals
        .manipulated_variable
        .symbol
        .clone();
    let output = project
        .loop_definition
        .signals
        .process_variable
        .symbol
        .clone();
    let refused = |settings: Value| {
        answer(
            &LoopClaims::default(),
            project.change_request(&json!({ "acquisitionSettings": settings })),
        )
        .expect_err("the settings are refused")
    };
    let settings = |input_range: Value, output_range: Value| {
        json!({
            "method": "automatic", "durationSeconds": 60, "amplitude": 0, "reuseRecordings": false,
            "inputRange": input_range, "outputRange": output_range
        })
    };
    // The manipulated variable is declared 0 to 0.02: a maximum above it, or a reversed range, names its maximum field.
    let above = refused(settings(
        json!({ "minimum": 0.0, "maximum": 0.05 }),
        Value::Null,
    ));
    assert_eq!(above.field, Some(format!("inputMaximum:{input}")));
    assert_eq!(above.next_action, NextAction::OpenLimits);
    let reversed = refused(settings(
        json!({ "minimum": 0.02, "maximum": 0.01 }),
        Value::Null,
    ));
    assert_eq!(reversed.field, Some(format!("inputMaximum:{input}")));
    // The measured output is declared 0 to 0.5.
    let output_above = refused(settings(
        Value::Null,
        json!({ "minimum": 0.0, "maximum": 0.9 }),
    ));
    assert_eq!(output_above.field, Some(format!("outputMaximum:{output}")));
    // A duration that no check accepts is refused under the duration field, and nothing is recorded by any refused change.
    let duration = refused(json!({
        "method": "automatic", "durationSeconds": 0.5, "amplitude": 0, "reuseRecordings": false
    }));
    assert_eq!(duration.field.as_deref(), Some("durationSeconds"));
    let study = storage::open_study(
        &project.root,
        &project.loop_definition.id,
        &project.loop_definition.revision,
    )
    .expect("the study");
    assert_eq!(
        study.section::<InputsSection>().expect("read"),
        None,
        "a refused change is not kept"
    );
}

#[test]
fn a_page_change_with_each_controlled_output_is_recorded_and_a_malformed_one_is_refused_under_its_field(
) {
    // The change the requirements page sends for two controlled outputs: the record's own values, the shared values, the scenarios
    // and `channelRequirements`, which holds the second output's own record (B4.3, C7).
    let project = Project::new("inputs-channel-requirements");
    let change = json!({
        "referenceChange": 1.0,
        "maximumError": 0.3,
        "settlingBand": 0.1,
        "settlingTimeSeconds": 40.0,
        "durationSeconds": 80,
        "scenarioDefinitions": [],
        "channelRequirements": {
            "P1.Zone2C": {
                "referenceChange": 2.0,
                "maximumError": 0.5,
                "settlingBand": 0.2,
                "settlingTimeSeconds": 30.0
            }
        }
    });
    let recorded = answer(&LoopClaims::default(), project.change_request(&change))
        .expect("the page's change is recorded");
    assert_eq!(
        recorded["inputs"]["requirements"]["channelRequirements"]["P1.Zone2C"]["maximumError"],
        json!(0.5),
        "the second output's record is kept as the page sent it"
    );
    let mut malformed = change;
    malformed["channelRequirements"]["P1.Zone2C"]["settlingBand"] = json!(0.6);
    let refused = answer(&LoopClaims::default(), project.change_request(&malformed))
        .expect_err("the second output's band is above its error limit");
    assert_eq!(refused.field.as_deref(), Some("maximumError:P1.Zone2C"));
    assert_eq!(refused.next_action, NextAction::OpenRequirements);
}

#[test]
fn rows_typed_for_each_controlled_output_are_assembled_into_the_record_and_a_refused_value_names_its_field(
) {
    // The change the requirements editor sends for two controlled outputs (B4.3, C7): the rows of each output as the engineer typed them,
    // in the editor's order, with the shared values and the scenarios beside them. The engine assembles the record: the first output's
    // rows are the record's own values, and each output's rows are kept under its channel.
    let project = Project::new("inputs-typed-outputs");
    let change = json!({
        "outputRequirements": [
            { "channel": "P1.Zone1C", "referenceChange": "1", "maximumError": "0.3", "settlingBand": "0.1", "settlingTimeSeconds": "40 s" },
            { "channel": "P1.Zone2C", "referenceChange": 2, "maximumError": 0.5, "settlingBand": 0.2, "settlingTimeSeconds": "30",
              "maximumOvershootPercent": "" }
        ],
        "durationSeconds": "80 s",
        "scenarioDefinitions": []
    });
    let recorded = answer(&LoopClaims::default(), project.change_request(&change))
        .expect("the typed rows are assembled and recorded");
    let requirements = &recorded["inputs"]["requirements"];
    assert_eq!(
        requirements["referenceChange"],
        json!(1.0),
        "the record's own values are the first output's copy"
    );
    assert_eq!(requirements["settlingTimeSeconds"], json!(40.0));
    assert_eq!(requirements["durationSeconds"], json!(80.0));
    assert_eq!(
        requirements["channelRequirements"]["P1.Zone2C"]["maximumError"],
        json!(0.5),
        "the second output's rows are kept under its channel"
    );
    assert_eq!(
        requirements["channelRequirements"]["P1.Zone2C"]["settlingTimeSeconds"],
        json!(30.0)
    );
    assert!(
        requirements["channelRequirements"]["P1.Zone2C"]
            .get("maximumOvershootPercent")
            .is_none(),
        "a blank overshoot row declares no limit: {recorded}"
    );
    let mut malformed = change;
    malformed["outputRequirements"][1]["settlingBand"] = json!("0.6");
    let refused = answer(&LoopClaims::default(), project.change_request(&malformed))
        .expect_err("the second output's band is above its error limit");
    assert_eq!(refused.field.as_deref(), Some("maximumError:P1.Zone2C"));
    assert_eq!(refused.next_action, NextAction::OpenRequirements);
}

#[test]
fn open_answers_the_controlled_outputs_of_the_requirements_editor_and_what_their_rows_start_from() {
    // Lesson 69's loop measures the upper tank and controls the lower one (B4.3, C7): only the controlled output is listed.
    let project = Project::with_loop("requirement-outputs", LESSON_69_LOOP);
    let studio = ControlStudio::new(Arc::new(Ignore));
    let opened = studio
        .execute(OPEN, &[project.open_request()])
        .expect("answered");
    assert_eq!(
        opened["result"]["requirementOutputs"],
        json!([{ "channelId": "tank-2-level", "signalId": "processVariable" }]),
        "the measured upper tank has no requirements, and the study holds none yet: {opened}"
    );
    answer(
        &LoopClaims::default(),
        project.inputs_request(with_requirements(2.0)),
    )
    .expect("recorded");
    let reopened = studio
        .execute(OPEN, &[project.open_request()])
        .expect("answered");
    assert_eq!(
        reopened["result"]["requirementOutputs"][0]["requirements"],
        json!({
            "referenceChange": 2.0, "maximumError": 0.4, "settlingBand": 0.1, "settlingTimeSeconds": 45.0
        }),
        "the process variable's rows start from the record's own values: {reopened}"
    );
}

/// The minimum and the maximum an acquisition record keeps for the signal `symbol`.
fn bounds_of(ranges: Option<&BTreeMap<String, RangeSetting>>, symbol: &str) -> Option<(f64, f64)> {
    ranges
        .and_then(|ranges| ranges.get(symbol))
        .map(|range| (range.minimum, range.maximum))
}

/// The ranges of one signal as the page sends them: the signal's symbol and its minimum and maximum, each a number or the text typed.
fn ranges_of(symbol: &str, minimum: &Value, maximum: &Value) -> Value {
    let mut ranges = serde_json::Map::new();
    ranges.insert(
        symbol.to_owned(),
        json!({ "minimum": minimum, "maximum": maximum }),
    );
    Value::Object(ranges)
}

/// B3.2, A8.1: the page sends a chosen live method with the limits as they stand. A loop that declares no duration, whose engineer has
/// given none, sends the duration as the blank text of its field, and the engine refuses the change under the duration's field with
/// Limits as the next action: the page reopens the Limits editor with that sentence, and its Done sends the method again. Nothing is
/// recorded. Catches an engine that records a method without a duration, a refusal without the field, and one that does not open Limits.
#[test]
fn a_method_change_without_a_duration_is_the_limits_editors_refusal() {
    let project = Project::new("method-without-duration");
    let input = project
        .loop_definition
        .signals
        .manipulated_variable
        .symbol
        .clone();
    let output = project
        .loop_definition
        .signals
        .process_variable
        .symbol
        .clone();
    let change = json!({ "acquisitionSettings": {
        "method": "mv_step", "durationSeconds": "", "amplitude": 0,
        "inputRanges": ranges_of(&input, &json!(0), &json!(100)),
        "outputRanges": ranges_of(&output, &json!(0), &json!(100))
    } });
    let refused = answer(&LoopClaims::default(), project.change_request(&change))
        .expect_err("a method without a duration is refused");
    assert_eq!(refused.field.as_deref(), Some("durationSeconds"));
    assert_eq!(refused.next_action, NextAction::OpenLimits);
    assert_eq!(
        refused.message,
        "The experiment duration must be a time such as 90 s, 15 min or 1 h 30 min."
    );
    let study = storage::open_study(
        &project.root,
        &project.loop_definition.id,
        &project.loop_definition.revision,
    )
    .expect("the study");
    assert_eq!(
        study.section::<InputsSection>().expect("read"),
        None,
        "a refused change is not kept"
    );
}

/// B3.2, A8.1, A8.6: the study stores the ranges of an acquisition record by signal, one entry for each plant input and measured
/// output, and the open answer's limits list them: the page sends every signal's range, as the text typed or a number, and the record
/// holds no single range. Catches a record that drops the range of the loop's primary signal because it expects it in the earlier
/// single form, a typed bound that is not read, and limits that do not show what the study recorded.
#[test]
fn an_acquisition_record_keeps_every_signal_range() {
    let project = Project::new("acquisition-every-range");
    let input = project
        .loop_definition
        .signals
        .manipulated_variable
        .symbol
        .clone();
    let output = project
        .loop_definition
        .signals
        .process_variable
        .symbol
        .clone();
    let change = json!({ "acquisitionSettings": {
        "method": "mv_step", "durationSeconds": "2 min", "amplitude": 0,
        "inputRanges": ranges_of(&input, &json!(10), &json!(90)),
        "outputRanges": ranges_of(&output, &json!("5"), &json!(95))
    } });
    let recorded = answer(&LoopClaims::default(), project.change_request(&change))
        .expect("the record is kept");
    assert_eq!(recorded["changed"], json!(true), "{recorded}");
    let study = storage::open_study(
        &project.root,
        &project.loop_definition.id,
        &project.loop_definition.revision,
    )
    .expect("the study");
    let record = study
        .section::<InputsSection>()
        .expect("read")
        .and_then(|inputs| inputs.acquisition)
        .expect("the acquisition record");
    assert_eq!(
        bounds_of(record.input_ranges.as_ref(), &input),
        Some((10.0, 90.0))
    );
    assert_eq!(
        bounds_of(record.output_ranges.as_ref(), &output),
        Some((5.0, 95.0)),
        "a bound typed as text is read as the number it names"
    );
    assert_eq!(
        (record.input_range, record.output_range),
        (None, None),
        "the record holds no single range"
    );
    // The open answer's limits are the record's: the duration, and the range of each signal.
    let studio = ControlStudio::new(Arc::new(Ignore));
    let opened = studio
        .execute(OPEN, &[project.open_request()])
        .expect("answered");
    let limits = &opened["result"]["experimentLimits"];
    assert_eq!(limits["durationSeconds"], json!(120.0), "{opened}");
    assert_eq!(limits["durationWords"], json!("2 min"), "{opened}");
    assert_eq!(limits["inputs"][0]["symbol"], json!(input), "{opened}");
    assert_eq!(limits["inputs"][0]["minimum"], json!(10.0), "{opened}");
    assert_eq!(limits["inputs"][0]["maximum"], json!(90.0), "{opened}");
    assert_eq!(limits["outputs"][0]["minimum"], json!(5.0), "{opened}");
    assert_eq!(limits["outputs"][0]["maximum"], json!(95.0), "{opened}");
    assert_eq!(
        opened["result"]["heldInputs"]["identifyMethod"],
        json!("mv_step"),
        "the Method list stands on the method the study holds: {opened}"
    );
}

/// A8.6, B4.2, B4.4: the open answer names what the recorded inputs mean for the editors, so the page picks nothing out of the
/// record: the controller family, and the pins of the tuning method the study holds. Catches an open answer without them.
#[test]
fn open_answers_the_held_inputs_the_editors_take() {
    let project = Project::new("open-held-inputs");
    let studio = ControlStudio::new(Arc::new(Ignore));
    let before = studio
        .execute(OPEN, &[project.open_request()])
        .expect("answered");
    assert_eq!(
        before["result"]["heldInputs"],
        json!({}),
        "a study that records no choice holds none: {before}"
    );
    let change = json!({
        "family": "PI",
        "tuningPreferences": { "method": "SIMC" },
        "pinnedValues": { "P1.Parameters.ProportionalGain": 3.0 }
    });
    answer(&LoopClaims::default(), project.change_request(&change)).expect("the change is kept");
    let opened = studio
        .execute(OPEN, &[project.open_request()])
        .expect("answered");
    let held = &opened["result"]["heldInputs"];
    assert_eq!(held["family"], json!("PI"), "{opened}");
    assert_eq!(
        held["pins"],
        json!({ "P1.Parameters.ProportionalGain": 3.0 }),
        "{opened}"
    );
    assert!(held.get("identifyMethod").is_none(), "{opened}");
}

#[path = "inputs_action_scenario_tests.rs"]
mod scenario_tests;
