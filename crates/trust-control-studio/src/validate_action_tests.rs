//! Tests of the Validate method (`validate_action`): what the method refuses at once, before any operation starts and before the
//! runtime is asked anything, and its place among the engine's methods. The run itself is tested with the runtime
//! (`workflow::validate_tests`) and on the real runtime (`tests/control_studio_validate.rs`).

#![allow(clippy::float_cmp)] // The values are the literals the plans and the requests carry, compared exactly on purpose.

use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

use serde_json::{json, Value};
use trust_control::loop_definition::LoopDefinition;
use trust_control::messages::OperationUpdate;
use trust_control::model::{decode_general_model, GeneralModel};
use trust_control::study::current_model::{CurrentModel, CurrentModelSource};
use trust_control::study::STUDY_FORMAT;
use trust_control::tuning::candidate::{CandidateSource, LawCandidate};
use trust_control::tuning::project_candidate::ProjectCandidate;
use trust_control::tuning::requirements::TuningRequirements;
use trust_control::tuning::structure::{ParallelGains, Structure};
use trust_control::validation::model_route::model_route;

use super::{campaign_of, entry_noise, supplied_plants, RunShape, ValidateWork};
use crate::commands::{ControlStudio, METHODS, VALIDATE};
use crate::operation::UpdateSink;
use crate::runtime_client::fake_runtime::FakeRuntime;
use crate::runtime_client::runtime_recording::PendingReleases;
use crate::storage;
use crate::workflow::apply::test_support::model;
use crate::workflow::validate::Settings;

const EXAMPLE: &str = include_str!("../../../examples/control-pid-commissioning/control-loop.json");

struct Ignore;

impl UpdateSink for Ignore {
    fn send(&self, _update: OperationUpdate) {}
}

/// A project folder with the example loop, and the arguments of `validate` for it, with `connection` when one is given.
fn example_project(name: &str, connection: Option<Value>) -> (PathBuf, LoopDefinition, Value) {
    let root = std::env::temp_dir().join(format!(
        "trust-control-validate-action-{name}-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).expect("project");
    fs::write(root.join("control-loop.json"), EXAMPLE).expect("loop");
    let example = LoopDefinition::decode(EXAMPLE).expect("example");
    let mut request = json!({
        "projectRoot": root.display().to_string(),
        "runtimeId": example.runtime_id,
        "loopId": example.id,
        "storageFolder": root.join("storage").display().to_string(),
    });
    if let Some(connection) = connection {
        request["connection"] = connection;
    }
    (root, example, request)
}

#[test]
fn validate_is_one_of_the_engine_methods_and_a_malformed_request_is_a_failure() {
    assert!(METHODS.contains(&VALIDATE), "the engine answers {VALIDATE}");
    let studio = ControlStudio::new(Arc::new(Ignore));
    let malformed = studio
        .execute(VALIDATE, &[json!({ "projectRoot": "/x" })])
        .expect("answered");
    assert!(
        malformed["failure"]["message"]
            .as_str()
            .is_some_and(|message| message.contains("invalid arguments")),
        "{malformed}"
    );
    assert!(malformed.get("result").is_none(), "{malformed}");
}

#[test]
fn a_validation_without_a_model_or_a_connection_is_refused_for_the_model_first_and_starts_no_operation(
) {
    // B5.5's order: the missing model is named before the missing runtime connection, so the next action is Identify.
    let (root, _example, request) = example_project("no-connection", None);
    let studio = ControlStudio::new(Arc::new(Ignore));
    let refused = studio.execute(VALIDATE, &[request]).expect("answered");
    assert_eq!(
        refused["failure"]["nextAction"],
        json!("identify"),
        "{refused}"
    );
    assert!(
        studio.operations().status("operation-1").is_err(),
        "a refused validation starts no operation"
    );
    fs::remove_dir_all(&root).expect("clean up");
}

#[test]
fn a_validation_without_a_model_is_refused_before_any_isolated_run_starts() {
    let fake = FakeRuntime::start(|_| None);
    let connection = serde_json::to_value(fake.connection(None)).expect("a connection");
    let (root, _example, request) = example_project("no-model", Some(connection));
    let studio = ControlStudio::new(Arc::new(Ignore));
    let refused = studio.execute(VALIDATE, &[request]).expect("answered");
    assert_eq!(refused["failure"]["nextAction"], "identify", "{refused}");
    assert!(
        fake.requests_of("control.simulation.run.start").is_empty(),
        "a refused validation starts no isolated run"
    );
    assert!(studio.operations().status("operation-1").is_err());
    fs::remove_dir_all(&root).expect("clean up");
}

#[test]
fn a_study_that_cannot_be_read_ends_the_validation_instead_of_making_the_entry_case_inconclusive() {
    // The study and the noise of the Manual-to-Auto entry case are reads of the validation itself (B5.4): a study that cannot be
    // read fails `entry_noise`, which the validation returns with `?`. Only a planning refusal makes the case Inconclusive.
    let (root, example, _request) = example_project("unreadable-study", None);
    let directory = storage::study_directory(&root, &example.id);
    fs::create_dir_all(&directory).expect("study folder");
    fs::write(
        directory.join("study.json"),
        format!("{{\"format\": {STUDY_FORMAT}}}"),
    )
    .expect("a study that does not decode");
    let work = ValidateWork {
        project_root: root.clone(),
        storage_folder: root.join("storage"),
        pending: Arc::new(PendingReleases::default()),
        loop_definition: example.clone(),
    };
    assert!(
        entry_noise(
            &work,
            &example,
            &CurrentModel {
                source: CurrentModelSource::Supplied,
                model: model(),
                identify: None,
            }
        )
        .is_err(),
        "an unreadable study is a failure of the validation"
    );
    fs::remove_dir_all(&root).expect("clean up");
}

/// Lesson 84 as its file declares it, and the heater model its Identify step imports (`provided-model.json`).
const LESSON_84: &str = include_str!(
    "../../../examples/control/14-integrated-capstones/84-independent/control-loop.json"
);
const LESSON_84_MODEL: &str = include_str!(
    "../../../examples/control/14-integrated-capstones/84-independent/provided-model.json"
);

/// Lesson 84's loop, its study model, the task of its campaign, and the settings its controller runs with.
fn lesson_84_inputs() -> (
    LoopDefinition,
    GeneralModel,
    TuningRequirements,
    BTreeMap<String, f64>,
) {
    let loop_definition = LoopDefinition::decode(LESSON_84).expect("lesson 84 decodes");
    let model_value: Value =
        serde_json::from_str(LESSON_84_MODEL).expect("the supplied model is JSON");
    let study_model = decode_general_model(&model_value).expect("the supplied model decodes");
    let requirements: TuningRequirements = serde_json::from_value(json!({
        "referenceChange": 1.0,
        "maximumError": 0.3,
        "settlingBand": 0.1,
        "settlingTimeSeconds": 40.0,
        "durationSeconds": 80.0,
    }))
    .expect("the requirements decode");
    let observed = BTreeMap::from([
        ("P1.Parameters.ProportionalGain".to_owned(), 2.0),
        ("P1.Parameters.IntegralGain".to_owned(), 0.5),
        ("P1.Parameters.DerivativeGain".to_owned(), 0.0),
        ("P1.FeedforwardGain".to_owned(), 0.5),
        ("P1.LeadS".to_owned(), 8.0),
        ("P1.LagS".to_owned(), 3.0),
    ]);
    (loop_definition, study_model, requirements, observed)
}

/// Lesson 84's candidate: the PI's gains 2 and 0.5 with no derivative, and the feedforward's gain 0.5 with its lead 8 s and lag 3 s.
fn lesson_84_candidate() -> LawCandidate {
    LawCandidate::Project(ProjectCandidate {
        applied: false,
        source: CandidateSource::ConstrainedComparison,
        structure: Structure::PI,
        gains: ParallelGains {
            proportional_gain: 2.0,
            integral_gain: 0.5,
            derivative_gain: 0.0,
        },
        values: BTreeMap::from([
            ("P1.Parameters.ProportionalGain".to_owned(), 2.0),
            ("P1.Parameters.IntegralGain".to_owned(), 0.5),
            ("P1.FeedforwardGain".to_owned(), 0.5),
            ("P1.LeadS".to_owned(), 8.0),
            ("P1.LagS".to_owned(), 3.0),
        ]),
    })
}

/// Lesson 84's campaign, built by `campaign_of` (the call both plants of Validate make): the target-response case comes first, then the
/// five declared scenarios in their order. Each case is planned on its own model (C8): the study's model for the target-response and the
/// load case, and each supplied model from its own operating point, so the 8 kW/°C and 12 kW/°C cases start at 60 % and 90 %.
#[test]
fn lesson_84_campaign_is_built_through_validate_with_its_declared_scenarios() {
    let (loop_definition, study_model, requirements, observed) = lesson_84_inputs();
    let cases = campaign_of(
        &loop_definition,
        &study_model,
        &requirements,
        &observed,
        &BTreeMap::new(),
    )
    .expect("lesson 84's campaign is built");
    let ids: Vec<&str> = cases.iter().map(|case| case.id.as_str()).collect();
    assert_eq!(ids.first(), Some(&"target-response"), "{ids:?}");
    assert!(
        ids.ends_with(&[
            "step-h8",
            "step-h12",
            "draw-off-h10",
            "draw-off-h8",
            "draw-off-h12"
        ]),
        "{ids:?}"
    );
    let heater = |id: &str| {
        cases
            .iter()
            .find(|case| case.id == id)
            .expect("the case is planned")
            .plan
            .initial_values["P1.HeaterPct"]
    };
    assert_eq!(heater("target-response"), 75.0);
    assert_eq!(heater("draw-off-h10"), 75.0);
    assert_eq!(heater("step-h8"), 60.0);
    assert_eq!(heater("draw-off-h8"), 60.0);
    assert_eq!(heater("step-h12"), 90.0);
    assert_eq!(heater("draw-off-h12"), 90.0);
    let step_h8 = cases
        .iter()
        .find(|case| case.id == "step-h8")
        .expect("step-h8 is a case");
    assert!(
        step_h8
            .requirements
            .iter()
            .any(|requirement| requirement.id == "step-h8-late-error"),
        "the declared case keeps its own requirements"
    );
    let supplied =
        supplied_plants(&loop_definition, &study_model, &observed).expect("the supplied plants");
    let candidate = lesson_84_candidate();
    let shape = RunShape {
        loop_definition: &loop_definition,
        model: &study_model,
        simulation: None,
        observed: &observed,
        candidate: &candidate,
        project_revision: "rev-1",
        supplied: &supplied,
    };
    let request = shape
        .request(&cases[0], Settings::Candidate, "job-candidate")
        .expect("the target-response candidate run is requested");
    assert_eq!(
        request.parameter_overrides.len(),
        6,
        "the PI's three and the feedforward's three: {:?}",
        request.parameter_overrides
    );
}

/// A draw-off case writes its load to the disturbance input of the plant it runs on (C8, B5.3): every model input is bound, the heater's
/// command to `P1.HeaterPct` and the load to `P1.LoadKw`, on the study's plant for the draw-off at the study's operating point and on the
/// draw-off's own plant for a supplied model; and the case's event writes the load at the start of the run.
#[test]
fn a_draw_off_case_writes_the_load_to_the_disturbance_input_of_its_plant() {
    let (loop_definition, study_model, requirements, observed) = lesson_84_inputs();
    let cases = campaign_of(
        &loop_definition,
        &study_model,
        &requirements,
        &observed,
        &BTreeMap::new(),
    )
    .expect("lesson 84's campaign is built");
    let supplied =
        supplied_plants(&loop_definition, &study_model, &observed).expect("the supplied plants");
    let study_route =
        model_route(&loop_definition, &study_model, &observed).expect("the study's route is built");
    let candidate = lesson_84_candidate();
    let shape = RunShape {
        loop_definition: &loop_definition,
        model: &study_model,
        simulation: Some(&study_route.simulation),
        observed: &observed,
        candidate: &candidate,
        project_revision: "rev-1",
        supplied: &supplied,
    };
    let case_named = |id: &str| {
        cases
            .iter()
            .find(|case| case.id == id)
            .expect("the case is planned")
    };
    for id in ["draw-off-h10", "draw-off-h8"] {
        let request = shape
            .request(case_named(id), Settings::Current, "job-draw-off")
            .expect("the draw-off run is requested");
        let [event] = request.events.as_slice() else {
            panic!("one event writes the load: {:?}", request.events);
        };
        assert_eq!(event.symbol, "P1.LoadKw", "{id}");
        assert_eq!(event.value, 10.0, "{id}");
        assert_eq!(event.at_ms, 0, "{id}");
        let plant = request
            .simulation_override
            .as_ref()
            .expect("the run takes a plant");
        assert!(
            plant
                .simulation_toml
                .contains("command_sources = [\"%QL0\", \"%ML8\"]"),
            "{id}: {}",
            plant.simulation_toml
        );
    }
    let own = shape
        .request(case_named("draw-off-h8"), Settings::Current, "job-own")
        .expect("the draw-off run is requested");
    assert_eq!(
        own.simulation_override.map(|plant| plant.model_revision),
        Some("line-heater-8-kw-per-c".to_owned()),
        "the 8 kW/°C draw-off runs on its own model"
    );
}

/// The campaign's plant of each declared scenario (C8): the scenarios that supply a model get their own plant, and the others run on
/// the study's.
#[test]
fn only_the_scenarios_that_supply_a_model_have_their_own_plant() {
    let (loop_definition, study_model, _requirements, observed) = lesson_84_inputs();
    let supplied =
        supplied_plants(&loop_definition, &study_model, &observed).expect("the supplied plants");
    let ids: Vec<&str> = supplied.keys().map(String::as_str).collect();
    assert_eq!(ids, ["draw-off-h12", "draw-off-h8", "step-h12", "step-h8"]);
    assert_eq!(
        supplied["step-h12"].model.identity().revision,
        "line-heater-12-kw-per-c"
    );
}
