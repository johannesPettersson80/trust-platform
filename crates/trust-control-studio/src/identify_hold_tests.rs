//! The review and the start of a project step test whose loop declares a hold parameter (specification 35, §0.1a and C12 Hold
//! parameter), against the fake runtime. The review reads the hold that the runtime applies and shows the recording of that hold as its
//! duration; it refuses a hold that cannot be read, lies outside its declared bounds, or exceeds the declared maximum. The start reads the
//! hold again, so a hold whose recording changed since the review is another campaign and is refused with the review-changed sentence.

#![allow(clippy::float_cmp)] // The review's durations are compared exactly on purpose.

use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

use parking_lot::Mutex;
use serde_json::{json, Value};
use trust_control::failure::{Cause, Failure, NextAction};
use trust_control::identification::acquisition_settings::{AcquisitionMethod, AcquisitionSettings};
use trust_control::identification::connected_run::{review_changed_refusal, runtime_required};
use trust_control::messages::{IdentifyAction, IdentifyRequest, OperationUpdate};
use trust_control::runtime::RuntimeConnection;

use super::{answer, confirm_start};
use crate::busy_loops::LoopClaims;
use crate::loops;
use crate::operation::{Operations, UpdateSink};
use crate::runtime_client::fake_runtime::{answer as reply, refusal, FakeRuntime};
use crate::runtime_client::runtime_recording::PendingReleases;

/// Lesson 21 (a safe, informative experiment): a project-scripted open-loop test with a 5 s baseline, a declared maximum of 45.1 s and the
/// hold parameter `P1.HoldSeconds`, 20 to 40 s, in a 100 ms loop.
const LESSON_21: &str = include_str!(
    "../../../examples/control/05-experiments-identification-and-validation/21-safe-informative-experiment/control-loop.json"
);
const HOLD: &str = "P1.HoldSeconds";
const NOT_REPORTED: &str =
    "The runtime did not report P1.HoldSeconds; inspect and apply the experiment settings first.";
const OUTSIDE_BOUNDS: &str =
    "Apply a P1.HoldSeconds value within 20–40 s before reviewing the project test.";

struct Ignore;

impl UpdateSink for Ignore {
    fn send(&self, _update: OperationUpdate) {}
}

/// A simulated runtime (E1.3) whose parameter record holds the step 10 and the hold `hold` (E7.2), or no hold while `hold` is `None`. It
/// answers the loop's scan as pending (E3), so the review has no live sample, and a test may change the hold while the runtime runs.
fn runtime(hold: &Arc<Mutex<Option<f64>>>) -> FakeRuntime {
    let hold = Arc::clone(hold);
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
        "control.parameters.status" => {
            let mut values = json!({ "P1.SelectedStepPoints": 10.0 });
            if let Some(applied) = *hold.lock() {
                values[HOLD] = json!(applied);
            }
            Some(reply(
                request,
                json!({
                    "supports_request_id": true, "transaction_id": null, "program_revision": "program:1",
                    "binding_revision": "binding:1", "active_revision": "active:1", "values": values,
                    "status": "idle", "previous_revision": null, "previous_values": null, "reason": null
                }),
            ))
        }
        _ => Some(refusal(request, "invalid_request", "Not in this test.")),
    })
}

/// A project folder holding lesson 21's loop, removed when the test ends.
struct Project {
    root: PathBuf,
}

impl Project {
    fn new(name: &str) -> Self {
        let root = std::env::temp_dir().join(format!(
            "trust-control-studio-{name}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).expect("the project folder");
        fs::write(root.join("control-loop.json"), LESSON_21).expect("the loop file");
        Self { root }
    }

    /// The review of the Automatic method for `seconds`, as the page asks for the project test, on `connection`.
    fn review(
        &self,
        connection: Option<RuntimeConnection>,
        seconds: f64,
    ) -> Result<Value, Failure> {
        answer(
            &Operations::new(Arc::new(Ignore)),
            &Arc::new(PendingReleases::default()),
            &LoopClaims::default(),
            IdentifyRequest {
                project_root: self.root.display().to_string(),
                runtime_id: "simulator".to_owned(),
                loop_id: "control-lesson-21".to_owned(),
                storage_folder: self.root.join("storage").display().to_string(),
                connection,
                runtime_mode: None,
                action: IdentifyAction::Review {
                    settings: Some(settings(seconds)),
                },
            },
        )
    }
}

impl Drop for Project {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

/// The settings of the project test: Automatic, no amplitude of its own, and the maximum duration `seconds`.
fn settings(seconds: f64) -> AcquisitionSettings {
    AcquisitionSettings {
        method: AcquisitionMethod::Automatic,
        duration_seconds: seconds,
        amplitude: 0.0,
        setpoint_signal_id: None,
        relay_direction: None,
        reuse_recordings: false,
        input_range: None,
        output_range: None,
        input_ranges: None,
        output_ranges: None,
    }
}

#[test]
fn lesson_21_is_reviewed_from_the_hold_the_runtime_applies() {
    let project = Project::new("hold-review");
    let hold = Arc::new(Mutex::new(Some(30.0)));
    let runtime = runtime(&hold);
    let reviewed = project
        .review(Some(runtime.connection(None)), 45.1)
        .expect("lesson 21 is reviewed from its applied hold");
    assert_eq!(
        reviewed["maximumDurationSeconds"].as_f64(),
        Some(35.1),
        "5 s + 30 s + 0.1 s, not the declared 45.1 s: {reviewed}"
    );
    assert_eq!(
        reviewed["projectScripted"]["baselineSeconds"].as_f64(),
        Some(5.0),
        "{reviewed}"
    );
    assert_eq!(
        reviewed["projectScripted"]["stopConditions"][1],
        json!("35.1 s timeout"),
        "{reviewed}"
    );
    assert_eq!(
        runtime.requests_of("control.parameters.status").len(),
        1,
        "the review reads the hold once"
    );
    assert!(
        runtime.requests_of("control.parameters.apply").is_empty(),
        "a review writes nothing"
    );
    // The longest permitted hold is another campaign, with its own identity.
    *hold.lock() = Some(40.0);
    let longer = project
        .review(Some(runtime.connection(None)), 45.1)
        .expect("a 40 s hold fills the declared maximum");
    assert_eq!(
        longer["maximumDurationSeconds"].as_f64(),
        Some(45.1),
        "{longer}"
    );
    assert_ne!(longer["identity"], reviewed["identity"]);
}

#[test]
fn a_hold_that_cannot_be_read_is_refused_by_the_review() {
    let project = Project::new("hold-unread");
    let hold = Arc::new(Mutex::new(None));
    let runtime = runtime(&hold);
    let refused = project
        .review(Some(runtime.connection(None)), 45.1)
        .expect_err("the runtime reports no hold");
    assert_eq!(refused.message, NOT_REPORTED);
    assert_eq!(
        (refused.cause, refused.next_action),
        (Cause::Runtime, NextAction::None),
        "the engineer inspects and applies the experiment settings first"
    );
    let refused = project
        .review(None, 45.1)
        .expect_err("a review without a connection cannot read the hold");
    assert_eq!(refused, runtime_required());
}

#[test]
fn a_hold_outside_its_bounds_or_above_the_maximum_is_refused_by_the_review() {
    let project = Project::new("hold-bounds");
    let hold = Arc::new(Mutex::new(Some(41.0)));
    let runtime = runtime(&hold);
    let refused = project
        .review(Some(runtime.connection(None)), 45.1)
        .expect_err("41 s is above the hold's maximum");
    assert_eq!(refused.message, OUTSIDE_BOUNDS);
    assert_eq!(
        (refused.cause, refused.next_action),
        (Cause::Settings, NextAction::None)
    );
    // A 30 s hold needs 35.1 s, which a maximum of 30 s cannot record.
    *hold.lock() = Some(30.0);
    let refused = project
        .review(Some(runtime.connection(None)), 30.0)
        .expect_err("35.1 s exceeds 30 s");
    assert_eq!(
        refused.message,
        "A 30 s hold needs 35.1 s, more than the declared 30 s maximum."
    );
    assert_eq!(
        (refused.cause, refused.next_action),
        (Cause::Settings, NextAction::OpenLimits)
    );
}

#[test]
fn the_start_reads_the_hold_again_and_a_changed_recording_is_another_campaign() {
    let project = Project::new("hold-start");
    let hold = Arc::new(Mutex::new(Some(30.0)));
    let runtime = runtime(&hold);
    let connection = runtime.connection(None);
    let reviewed = project
        .review(Some(runtime.connection(None)), 45.1)
        .expect("reviewed");
    let identity = reviewed["identity"]
        .as_str()
        .expect("the review's identity")
        .to_owned();
    let loop_definition =
        loops::open(&project.root, "simulator", "control-lesson-21").expect("lesson 21 opens");
    let start = |review: Option<&str>| {
        confirm_start(
            &loop_definition,
            &connection,
            &settings(45.1),
            review,
            false,
        )
    };
    assert_eq!(
        start(Some(identity.as_str())),
        Ok(None),
        "the hold is unchanged, so the start names the campaign it reviewed"
    );
    // The engineer applies another hold elsewhere, and the start reads it.
    *hold.lock() = Some(40.0);
    assert_eq!(
        start(Some(identity.as_str())),
        Err(review_changed_refusal()),
        "a 40 s hold records 45.1 s, not the reviewed 35.1 s"
    );
    assert_eq!(
        start(None),
        Ok(None),
        "a start that names no review runs the campaign as it is now, on a simulated target"
    );
    // The runtime no longer reports the hold: the start is refused as the review is.
    *hold.lock() = None;
    let refused = start(None).expect_err("the hold cannot be read");
    assert_eq!(refused.message, NOT_REPORTED);
}
