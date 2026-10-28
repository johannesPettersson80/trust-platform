//! The start of `controlStudio/identify` refuses a method the loop cannot run (specification 35, §0.1a, B3.2 and B3.3). The refusal is the
//! sentence the open answer's Method list gives that method, with the next action Choose another method, and it comes before the runtime
//! is asked for anything, so the page checks no method itself. The Identify stage of a run makes the same check with the same function
//! (`require_applicable_method`), which these tests call.

use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

use serde_json::{json, Value};
use trust_control::failure::{Cause, Failure, NextAction};
use trust_control::identification::acquisition_settings::{AcquisitionMethod, AcquisitionSettings};
use trust_control::identification::method_offers::identify_method_offers;
use trust_control::loop_definition::{ExperimentKind, LoopDefinition};
use trust_control::messages::{IdentifyAction, IdentifyRequest, OperationUpdate};

use super::{answer, require_applicable_method};
use crate::busy_loops::LoopClaims;
use crate::operation::{Operations, UpdateSink};
use crate::runtime_client::runtime_recording::PendingReleases;

const EXAMPLE: &str = include_str!("../../../examples/control-pid-commissioning/control-loop.json");

/// The methods of the Method list that start a campaign, with the word the list gives each.
const LIVE_METHODS: [(AcquisitionMethod, &str); 6] = [
    (AcquisitionMethod::MvStep, "mv_step"),
    (AcquisitionMethod::MvPulse, "mv_pulse"),
    (AcquisitionMethod::SetpointStep, "setpoint_step"),
    (AcquisitionMethod::Relay, "relay"),
    (AcquisitionMethod::Prbs, "prbs"),
    (AcquisitionMethod::Multisine, "multisine"),
];

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
        let root = std::env::temp_dir().join(format!(
            "trust-control-studio-{name}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).expect("project");
        fs::write(root.join("control-loop.json"), EXAMPLE).expect("loop");
        let loop_definition = LoopDefinition::decode(EXAMPLE).expect("the example loop");
        Self {
            root,
            loop_definition,
        }
    }

    /// The `identify` start of `settings` on this project's loop, with no runtime connection.
    fn identify(&self, settings: AcquisitionSettings) -> Result<Value, Failure> {
        answer(
            &Operations::new(Arc::new(Ignore)),
            &Arc::new(PendingReleases::default()),
            &LoopClaims::default(),
            IdentifyRequest {
                project_root: self.root.display().to_string(),
                runtime_id: self.loop_definition.runtime_id.clone(),
                loop_id: self.loop_definition.id.clone(),
                storage_folder: self.root.join("storage").display().to_string(),
                connection: None,
                runtime_mode: None,
                action: IdentifyAction::Identify {
                    settings: Some(settings),
                    review: None,
                    physical_approved: false,
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

fn settings_of(method: AcquisitionMethod) -> AcquisitionSettings {
    AcquisitionSettings {
        method,
        duration_seconds: 16.0,
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

/// The reason the Method list gives `word` for `loop_definition`, when it says the loop cannot run it.
fn offered_reason(loop_definition: &LoopDefinition, word: &str) -> Option<String> {
    identify_method_offers(loop_definition)
        .into_iter()
        .find(|offer| offer.value == word)
        .and_then(|offer| offer.reason)
}

/// The example loop with no setpoint and no parameter binding: it cannot run a setpoint step or an open-loop method.
fn loop_without_ownership() -> LoopDefinition {
    let mut value: Value = serde_json::from_str(EXAMPLE).expect("the example document");
    value
        .as_object_mut()
        .expect("a loop is an object")
        .remove("parameterBinding");
    value["signals"]
        .as_object_mut()
        .expect("the signals")
        .remove("setpoint");
    trust_control::json::decode_value(value).expect("the loop decodes")
}

#[test]
fn a_start_whose_method_the_loop_cannot_run_is_refused_with_the_method_lists_sentence_before_the_runtime_is_asked(
) {
    let project = Project::new("method-refused");
    let reason = offered_reason(&project.loop_definition, "relay")
        .expect("the example declares no relay direction, so its Method list says why");
    assert_eq!(
        reason,
        "Relay identification needs an explicitly declared process direction."
    );
    let refused = project
        .identify(settings_of(AcquisitionMethod::Relay))
        .expect_err("a relay needs the direction the loop declares");
    assert_eq!(
        refused
            .detail
            .as_deref()
            .unwrap_or(refused.message.as_str()),
        reason,
        "the sentence is the Method list's, with no runtime connection asked for first"
    );
    assert_eq!(refused.cause, Cause::Settings);
    assert_eq!(refused.next_action, NextAction::ChooseMethod);
    assert!(
        !project.root.join("storage").exists(),
        "nothing was started or journaled"
    );
}

#[test]
fn the_start_refuses_exactly_what_the_method_list_says_the_loop_cannot_run() {
    let ownerless = loop_without_ownership();
    let example = LoopDefinition::decode(EXAMPLE).expect("the example loop");
    let mut refusals = 0;
    for loop_definition in [&example, &ownerless] {
        for (method, word) in LIVE_METHODS {
            let checked = require_applicable_method(loop_definition, &settings_of(method));
            match offered_reason(loop_definition, word) {
                Some(reason) => {
                    refusals += 1;
                    let failure = checked.expect_err(word);
                    assert_eq!(
                        failure
                            .detail
                            .as_deref()
                            .unwrap_or(failure.message.as_str()),
                        reason,
                        "{word}"
                    );
                    assert_eq!(failure.next_action, NextAction::ChooseMethod, "{word}");
                }
                None => assert!(checked.is_ok(), "{word} is offered, so it starts"),
            }
        }
    }
    assert!(
        refusals >= 4,
        "the loop without ownership cannot run a setpoint step or an open-loop method, and the example no relay: {refusals}"
    );
}

#[test]
fn automatic_always_starts_and_a_configured_method_is_the_one_the_loop_declares() {
    let example = LoopDefinition::decode(EXAMPLE).expect("the example loop");
    for method in [AcquisitionMethod::Automatic, AcquisitionMethod::Configured] {
        assert!(
            require_applicable_method(&example, &settings_of(method)).is_ok(),
            "{method:?}: the example declares an observation"
        );
    }
    let mut declared = example.clone();
    declared
        .experiment
        .as_mut()
        .expect("the example declares an experiment")
        .kind = ExperimentKind::Relay;
    let reason = offered_reason(&declared, "relay").expect("the loop declares no relay direction");
    let refused = require_applicable_method(&declared, &settings_of(AcquisitionMethod::Configured))
        .expect_err("the declared relay cannot run");
    assert_eq!(
        refused
            .detail
            .as_deref()
            .unwrap_or(refused.message.as_str()),
        reason
    );
    assert!(
        require_applicable_method(&declared, &settings_of(AcquisitionMethod::Automatic)).is_ok(),
        "Automatic does not use the declared kind"
    );
    let words = json!(["automatic", "configured"]);
    assert_eq!(
        serde_json::to_value([AcquisitionMethod::Automatic, AcquisitionMethod::Configured])
            .expect("the methods"),
        words,
        "the words the settings carry"
    );
}
