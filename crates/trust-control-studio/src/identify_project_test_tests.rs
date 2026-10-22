//! The review and the start of a project test (specification 35, §0.1a, A2.8 and C12), against the fake runtime: the test needs no
//! acquisition settings from the page, a runtime that is not running is answered with the next action that applies, and the review
//! binds the experiment settings that the runtime holds, so a setting changed before the start refuses it.

#![allow(clippy::float_cmp)] // The review's duration is compared exactly on purpose.

use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

use parking_lot::Mutex;
use serde_json::{json, Value};
use trust_control::experiment::{ControllerMode, ExperimentSample, PlanSignal};
use trust_control::failure::{Cause, Failure, NextAction};
use trust_control::identification::acquisition_settings::{declared_settings, AcquisitionMethod};
use trust_control::identification::campaign_review::{campaign_review, campaign_settings};
use trust_control::identification::connected_run::{
    review_changed_refusal, runtime_required, ACQUISITION_SETTINGS_TEXT,
};
use trust_control::loop_definition::LoopDefinition;
use trust_control::messages::{IdentifyAction, IdentifyRequest, OperationUpdate};
use trust_control::recording::EvidenceSource;
use trust_control::runtime::RuntimeConnection;
use trust_control::signals::Quality;

use super::identify_review::start_refusal;
use super::{answer, confirm_start};
use crate::busy_loops::LoopClaims;
use crate::loops;
use crate::operation::{Operations, UpdateSink};
use crate::runtime_client::fake_runtime::{answer as reply, refusal, FakeRuntime};
use crate::runtime_client::runtime_recording::PendingReleases;

/// Lesson 21 (a safe, informative experiment): a project-scripted open-loop test with the activation `P1.ExperimentEnabled`, a 5 s
/// baseline, a declared maximum of 45.1 s, and the experiment settings `P1.SelectedStepPoints` (5 to 15) and `P1.HoldSeconds` (20 to
/// 40 s, its hold parameter), in a 100 ms loop.
const LESSON_21: &str = include_str!(
    "../../../examples/control/05-experiments-identification-and-validation/21-safe-informative-experiment/control-loop.json"
);
/// A closed loop that Control Studio runs itself: it has no project test.
const EXAMPLE: &str = include_str!("../../../examples/control-pid-commissioning/control-loop.json");

struct Ignore;

impl UpdateSink for Ignore {
    fn send(&self, _update: OperationUpdate) {}
}

/// The experiment settings of lesson 21 with the step `step` and the hold `hold`.
fn settings_of(step: f64, hold: f64) -> BTreeMap<String, f64> {
    BTreeMap::from([
        ("P1.SelectedStepPoints".to_owned(), step),
        ("P1.HoldSeconds".to_owned(), hold),
    ])
}

/// A simulated runtime (E1.3) whose parameter record holds the settings in `record` (E7.2), which a test may change while it runs. It
/// answers the loop's scan as pending (E3), so a review has no live sample.
fn runtime(record: &Arc<Mutex<BTreeMap<String, f64>>>) -> FakeRuntime {
    let record = Arc::clone(record);
    FakeRuntime::start(move |request| match request["type"].as_str()? {
        "status" => Some(reply(
            request,
            json!({
                "session_epoch": 7, "simulation_enabled": true, "simulation_only_io": true,
                "simulation_time_scale": 1, "simulation_achieved_time_scale": null,
                "program_revision": "program:1", "tasks": [{ "name": "MainTask", "interval_ms": 100 }],
                "cycle_interval_ms": 100
            }),
        )),
        "control.signals.inspect" => {
            let params = &request["params"];
            Some(reply(
                request,
                json!({
                    "available": false, "loop_revision": params["loop_revision"],
                    "program_revision": "program:1", "bindings": params["bindings"],
                    "reason": "Waiting for the next coherent runtime scan."
                }),
            ))
        }
        "control.parameters.status" => Some(reply(
            request,
            json!({
                "supports_request_id": true, "transaction_id": null, "program_revision": "program:1",
                "binding_revision": "binding:1", "active_revision": "active:1", "values": *record.lock(),
                "status": "idle", "previous_revision": null, "previous_values": null, "reason": null
            }),
        )),
        _ => Some(refusal(request, "invalid_request", "Not in this test.")),
    })
}

/// A project folder holding one loop file, removed when the test ends.
struct Project {
    root: PathBuf,
    loop_definition: LoopDefinition,
}

impl Project {
    fn new(name: &str, document: &str) -> Self {
        let root = std::env::temp_dir().join(format!(
            "trust-control-studio-{name}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).expect("the project folder");
        fs::write(root.join("control-loop.json"), document).expect("the loop file");
        let decoded = LoopDefinition::decode(document).expect("the loop");
        let loop_definition =
            loops::open(&root, &decoded.runtime_id, &decoded.id).expect("the loop opens");
        Self {
            root,
            loop_definition,
        }
    }

    /// The answer of `controlStudio/identify` to `action` on this project's loop, with `connection` and the lifecycle's `runtime_mode`.
    fn answer(
        &self,
        connection: Option<RuntimeConnection>,
        runtime_mode: Option<&str>,
        action: IdentifyAction,
    ) -> Result<Value, Failure> {
        answer(
            &Operations::new(Arc::new(Ignore)),
            &Arc::new(PendingReleases::default()),
            &LoopClaims::default(),
            IdentifyRequest {
                project_root: self.root.display().to_string(),
                runtime_id: self.loop_definition.runtime_id.clone(),
                loop_id: self.loop_definition.id.clone(),
                storage_folder: self.root.join("storage").display().to_string(),
                connection,
                runtime_mode: runtime_mode.map(str::to_owned),
                action,
            },
        )
    }

    /// Whether anything was journaled or written for the project's operations.
    fn wrote_nothing(&self) -> bool {
        !self.root.join("storage").exists()
    }
}

impl Drop for Project {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

/// The review of the project test, which carries no settings (C12).
fn review() -> IdentifyAction {
    IdentifyAction::Review { settings: None }
}

/// The start of the project test that names `review`, with no settings and no approval.
fn start(review: Option<&str>) -> IdentifyAction {
    IdentifyAction::Identify {
        settings: None,
        review: review.map(str::to_owned),
        physical_approved: false,
    }
}

// C12, §0.1a: the project owns its experiment, so a project test is reviewed and started with no settings from the page; the engine
// runs the loop's declared experiment with the Automatic method. A page that invented the settings (the Automatic method, the declared
// duration, no amplitude) would have given the same campaign, so the identity is the same; a loop that has no project test still needs
// the engineer's method and limits.
#[test]
fn a_project_test_review_needs_no_acquisition_settings() {
    let project = Project::new("project-test-settings", LESSON_21);
    let record = Arc::new(Mutex::new(settings_of(10.0, 30.0)));
    let runtime = runtime(&record);
    let reviewed = project
        .answer(Some(runtime.connection(None)), None, review())
        .expect("a project test is reviewed without settings");
    // The applied 30 s hold records the 5 s baseline, every scan of the hold and one 0.1 s restoration sample (C12, Hold parameter).
    assert_eq!(
        reviewed["maximumDurationSeconds"].as_f64(),
        Some(35.1),
        "{reviewed}"
    );
    let declared = declared_settings(&project.loop_definition, AcquisitionMethod::Automatic);
    let invented = project
        .answer(
            Some(runtime.connection(None)),
            None,
            IdentifyAction::Review {
                settings: Some(declared),
            },
        )
        .expect("the same campaign with the settings spelled out");
    assert_eq!(
        reviewed["identity"], invented["identity"],
        "the engine's settings of the project test are the declared experiment with the Automatic method"
    );
    // The start without settings passes every check of the settings and reaches the runtime's: here none runs.
    let identity = reviewed["identity"]
        .as_str()
        .expect("the review's identity");
    assert_eq!(
        project.answer(None, None, start(Some(identity))),
        Err(runtime_required())
    );

    let closed = Project::new("closed-loop-settings", EXAMPLE);
    for action in [review(), start(None)] {
        let refused = closed
            .answer(None, None, action)
            .expect_err("a loop without a project test needs the engineer's settings");
        assert_eq!(refused.message, ACQUISITION_SETTINGS_TEXT);
        assert_eq!(
            (refused.cause, refused.next_action),
            (Cause::Settings, NextAction::OpenLimits)
        );
    }
    assert!(closed.wrote_nothing());
}

// A2.8, E11.1: the page sends the project test's review whatever the runtime's state. When no runtime runs and the window's runtime is
// the local simulator, the engine answers with the next action Start simulator, whose button the page shows; the start that finds the
// simulator stopped answers the same. Nothing is written.
#[test]
fn a_project_test_review_on_a_stopped_simulator_answers_start_simulator() {
    let project = Project::new("project-test-stopped-simulator", LESSON_21);
    for action in [review(), start(None)] {
        let refused = project
            .answer(None, Some("simulate"), action)
            .expect_err("no runtime runs");
        assert_eq!(
            refused.message,
            "Start the simulator before running a live action."
        );
        assert_eq!(
            (refused.cause, refused.next_action),
            (Cause::Runtime, NextAction::StartSimulator)
        );
        let words = serde_json::to_value(&refused).expect("the failure's JSON");
        assert_eq!(words["nextAction"], json!("start-simulator"), "{words}");
        assert_eq!(
            words["nextActionLabel"],
            json!("Start simulator"),
            "{words}"
        );
    }
    assert!(project.wrote_nothing());
}

// A2.4, A2.5, A2.8, C12: on a runtime attached through its endpoint that is not running, the engine answers Connect runtime, which the
// engineer connects or starts; it never offers to start the simulator for a physical target. A running physical target reviews its
// project test with the approval block and the engine's sentence, and the approved start of that identity runs.
#[test]
fn a_project_test_review_on_a_physical_target_never_starts_the_simulator() {
    let project = Project::new("project-test-physical", LESSON_21);
    for action in [review(), start(None)] {
        let refused = project
            .answer(None, Some("online"), action)
            .expect_err("no runtime runs");
        assert_eq!(refused, runtime_required());
        assert_eq!(refused.next_action, NextAction::ConnectRuntime);
    }
    assert!(project.wrote_nothing());

    let loop_definition = &project.loop_definition;
    let settings = campaign_settings(loop_definition, None).expect("the project test's settings");
    let signal = |symbol: &str, value: f64, unit: &str| PlanSignal {
        symbol: symbol.to_owned(),
        address: None,
        value,
        unit: unit.to_owned(),
        quality: Quality::Good,
    };
    let physical_sample = ExperimentSample {
        scan: 1,
        source: EvidenceSource::PhysicalPlant,
        controller_mode: ControllerMode::Manual,
        setpoint: None,
        process_variable: signal("P1.ProcessValue", 40.0, "degC"),
        manipulated_variable: signal("P1.HeaterPower", 75.0, "%"),
        additional: Vec::new(),
    };
    let physical = campaign_review(
        loop_definition,
        &settings,
        Some(&physical_sample),
        Some(30.0),
    )
    .expect("the project test is reviewed on the physical target");
    let approval = physical.approval.as_ref().expect("the approval block");
    assert_eq!(
        approval.message.as_deref(),
        Some("Physical target. Approve the reviewed test before it starts; nothing is written before your approval.")
    );
    assert_eq!(
        start_refusal(&physical, Some(physical.identity.as_str()), true),
        None,
        "the approved identity starts on the physical target"
    );
    assert_eq!(
        start_refusal(&physical, Some(physical.identity.as_str()), false)
            .map(|refused| refused.next_action),
        Some(NextAction::Approve)
    );
}

// C12, Parameter purpose: the experiment settings are frozen through review. The review's identity binds the settings the runtime
// holds when it is made, as it binds the hold (Hold parameter), so a step changed between the review and the start gives another
// identity, and the start is refused with the review-changed sentence; the reviewed values start again.
#[test]
fn a_setting_changed_after_review_refuses_the_start() {
    let project = Project::new("project-test-frozen", LESSON_21);
    let record = Arc::new(Mutex::new(settings_of(10.0, 30.0)));
    let runtime = runtime(&record);
    let connection = runtime.connection(None);
    let reviewed = project
        .answer(Some(connection.clone()), None, review())
        .expect("reviewed");
    let identity = reviewed["identity"]
        .as_str()
        .expect("the review's identity")
        .to_owned();
    let settings =
        campaign_settings(&project.loop_definition, None).expect("the project test's settings");
    let confirm = || {
        confirm_start(
            &project.loop_definition,
            &connection,
            &settings,
            Some(identity.as_str()),
            false,
        )
    };
    assert_eq!(confirm(), Ok(None), "the settings are the reviewed ones");
    // The step changes and the hold does not, so the recording and its duration are the same: only the bound settings differ.
    *record.lock() = settings_of(12.0, 30.0);
    assert_eq!(confirm(), Err(review_changed_refusal()));
    *record.lock() = settings_of(10.0, 30.0);
    assert_eq!(confirm(), Ok(None), "the reviewed settings again");
}
