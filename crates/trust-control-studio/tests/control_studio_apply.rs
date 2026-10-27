//! Apply, Restore previous settings and Keep in project through the command line of `trust-control-studio` on a real runtime (checklist
//! V4.6; specification 35, §0.1b, A6.2, A8.1, A8.5, B6.1 to B6.5, B7, C1; runtime protocol annex E6, E7 and E11).
//!
//! The loop is lesson 30's heater, `examples/control/06-pid-tuning-methods/30-model-based-pi-tuning`, a `Control.PID` whose program
//! starts in Manual and designs its own gains in its first scan. Its runtime runs at ten times real time (`time_scale = 10` in the copy's
//! `simulation.toml`), as the validate test does. The journey goes in the engineer's order:
//! - `identify`, `tune --method simc` and `validate` give the candidate, as `control_studio_validate.rs` does;
//! - the loop is moved to Auto through the runtime's transfer, with the declared profile of the controller (A6.4), so the mode that Apply
//!   finds is Auto and Restore must return to it (the V4.6 regression);
//! - a review written in Auto, then a review made stale by moving the loop to Manual: `apply --review` with the stale identity exits 1,
//!   says the runtime changed, and writes nothing;
//! - the text and the JSON review of Apply, which write nothing, then `apply --review <identity>`: exit 0, the record holds the candidate,
//!   the loop is in Auto, the study's Apply section holds the receipt and the journal holds a completed checkpoint;
//! - with that checkpoint completed, `identify`, `tune` and `validate` are not held back;
//! - `restore` (review, then confirmation): the record returns to the values from before Apply and the loop to Auto; the journal's
//!   checkpoint is cleared; `keep` is then refused and writes nothing;
//! - `identify`, `tune` and `validate` again at the operating point Restore returned to, then a second Apply, and `keep` (review, then
//!   confirmation): the project's `src/Main.st` takes the running values in the form the review showed.
//!
//! A check that does not block the next step is recorded and the journey goes on. A step that the next ones need ends the run at once,
//! with the findings so far. Run time at ten times real time: about a minute, nearly all of it the verbs' own work (each one starts the
//! engine and reads the runtime and the study); the Apply observations last the test duration of the lesson in process time.
//!
//! Run it without concurrent heavy load: the validation batches run the integration tests of this crate in their own Rust phase,
//! before the VS Code shards start. At ten times real time, a machine loaded beside it (about 12 on 16 cores, 15 October 2026) delays
//! the engine's reads and writes by whole task periods of process time, and the runtime then refuses Validate's return to Auto as a
//! first output beyond the reviewed transfer tolerance (annex E6.9). That refusal is the runtime's safety check doing its work, so the
//! tolerance is never widened for the test; alone, the test passes (focused checks `cs-w2af-2` reruns and `cs-w5b-h`).
//!
//! The file is Unix only, as the validate test is: the runtime's endpoint is a Unix socket. Keep needs the `trust-lsp` executable beside
//! `trust-control-studio` (`cargo build -p trust-lsp`), as `control_studio_keep.rs` does.
#![cfg(unix)]

mod common;

use std::collections::BTreeMap;
use std::time::Duration;

use serde_json::Value;
use trust_control::commissioning::adapters::controller_adapter;
use trust_control::commissioning::application::ApplySource;
use trust_control::commissioning::checkpoint::CheckpointStage;
use trust_control::commissioning::commissioning_result::{
    TransferOwner, TransferState, TransferStatus,
};
use trust_control::commissioning::transfer::{TransferBinding, TransferValue};
use trust_control::failure::Failure;
use trust_control::model::general::{decode_general_model, GeneralModel};
use trust_control_studio::journal::{Journal, JournalEntry};
use trust_control_studio::runtime_client::transfers::{
    TransferOwnerSink, TransferSession, TransferWrite,
};

use common::engine::{journal_of, read_text, same_values, snapshot, study_of, Findings, Lab};

/// Lesson 30's heater project, a copy of which the test runs (see the module documentation).
const LESSON_30: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../examples/control/06-pid-tuning-methods/30-model-based-pi-tuning"
);
/// The loop's file in the copy.
const LOOP_FILE: &str = "control-loop.json";
/// The speed of the simulation, ten times real time.
const TIME_SCALE: u32 = 10;
/// A control endpoint that nothing listens on: Tune must finish without reaching it (B4.1).
const UNREACHABLE: &str = "tcp://127.0.0.1:1";
/// The longest that one verb may run; at ten times real time the verbs take seconds, so this only ends a hung run.
const LIMIT: Duration = Duration::from_secs(300);
/// The output span of the heater, 0 to 100 %: the change that a transfer may make at its first execution (annex E6.9).
const OUTPUT_SPAN: f64 = 100.0;
/// The message of a review that the runtime no longer matches (§0.1b Confirmation).
const REVIEW_AGAIN: &str = "The runtime changed since the review; review again.";

/// The heater's model of the lesson (`Main.st`: a gain of 0.2 °C per percent, a time constant of 5 s and a delay of 1 s), at its
/// operating point of 75 % and 35 °C. The declared profile is built from a model of this kind (A6.4).
fn lesson_model() -> GeneralModel {
    decode_general_model(&serde_json::json!({
        "kind": "transfer-function",
        "revision": "model:lesson-30",
        "source": "provided",
        "validation": "validated",
        "evidence": ["lesson-30"],
        "delaySeconds": 1.0,
        "numerator": [0.2],
        "denominator": [5.0, 1.0],
        "inputs": [{ "id": "u", "unit": "%", "operatingRange": { "minimum": 0, "maximum": 100 } }],
        "outputs": [{ "id": "y", "unit": "degC", "operatingRange": { "minimum": 0, "maximum": 80 } }],
        "operatingPoint": { "inputs": [75], "outputs": [35] }
    }))
    .expect("the lesson's model decodes")
}

/// The values of the binding's states in a transfer status: each one is a number.
fn state_values(status: &TransferStatus, binding: &TransferBinding) -> BTreeMap<String, f64> {
    binding
        .states
        .iter()
        .map(|member| {
            let value = status
                .values
                .get(&member.symbol)
                .and_then(|value| value.as_number())
                .expect("the runtime reports each state as a number");
            (member.symbol.clone(), value)
        })
        .collect()
}

/// The owner sink of a transfer of this journey: each owner is kept in memory, where the engine's journal keeps it before the dispatch
/// (E11.2).
#[derive(Default)]
struct OwnerSink(Vec<TransferOwner>);

impl TransferOwnerSink for OwnerSink {
    fn retain(&mut self, owner: &TransferOwner) -> Result<(), Failure> {
        self.0.push(owner.clone());
        Ok(())
    }
}

/// Moves the loop to Manual (`manual`) or to Auto through the runtime's transfer, as the engine's own transfers do: a move to Manual
/// tracks the controller's recovery tracking, a move to Auto enters with the declared profile of the controller (A6.4). The runtime
/// confirms the transfer, and the test checks that it did.
fn transfer(lab: &Lab, manual: bool) {
    let adapter = controller_adapter(&lab.loop_definition).expect("the controller has an adapter");
    let binding = adapter.transfer_binding().expect("the transfer binding");
    let model = lesson_model();
    let original = lab.parameter_values();
    let mut client = lab.client();
    let mut session =
        TransferSession::new(&mut client, binding.clone(), lab.program_revision.clone());
    let observed = session.status().expect("the transfer status");
    let (values, tracking) = if manual {
        let recovery = adapter
            .recovery_tracking(&model)
            .expect("the recovery tracking");
        (state_values(&observed, &binding), recovery)
    } else {
        let profile = adapter
            .declared_profile(&model, &original)
            .expect("the declared profile fits the loop")
            .expect("Control.PID declares its profile");
        let states = profile
            .states(&binding, &observed.values)
            .expect("the declared states");
        let tracking = profile
            .tracking(&binding, true)
            .expect("the declared tracking");
        (states, tracking)
    };
    let mut sink = OwnerSink::default();
    let outcome = session
        .apply(
            &TransferWrite {
                values,
                manual,
                tolerance: OUTPUT_SPAN,
                entry_error_gain: None,
                tracking: Some(tracking),
                reviewed: None,
                source: ApplySource::Simulation,
                physical_approved: false,
            },
            &mut sink,
        )
        .expect("the runtime answers the transfer");
    assert_eq!(
        outcome.status,
        TransferState::Applied,
        "the runtime confirms the transfer (E6.7): {:?}",
        outcome.reason
    );
    assert_eq!(sink.0.len(), 1, "the owner is retained before the dispatch");
}

/// The loop's reference as the runtime's transfer status reports it (annex E6.3), or `None` when it cannot be read.
fn reference(lab: &Lab) -> Option<f64> {
    let adapter = controller_adapter(&lab.loop_definition).ok()?;
    let binding = adapter.transfer_binding().ok()?;
    let symbol = lab
        .loop_definition
        .signals
        .setpoint
        .as_ref()?
        .symbol
        .clone();
    let mut client = lab.client();
    let mut session = TransferSession::new(&mut client, binding, lab.program_revision.clone());
    session
        .status()
        .ok()?
        .values
        .get(&symbol)
        .copied()
        .and_then(TransferValue::as_number)
}

/// The names in the array `field` of `value`, in order; a missing or non-text entry is left out.
fn names(value: &Value, field: &str) -> Vec<String> {
    value[field]
        .as_array()
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default()
}

/// The values of a review's parameters, each symbol with the value its confirmation writes (`next`).
fn next_values(review: &Value) -> BTreeMap<String, f64> {
    review["parameters"]
        .as_array()
        .map(|items| {
            items
                .iter()
                .filter_map(|item| {
                    Some((item["symbol"].as_str()?.to_string(), item["next"].as_f64()?))
                })
                .collect()
        })
        .unwrap_or_default()
}

/// The text of a string field, or an empty text when the answer lacks it.
fn text_of(value: &Value, field: &str) -> String {
    value[field].as_str().unwrap_or_default().to_string()
}

/// Whether the journal holds the commissioning checkpoint `id` in the stage `stage` (A8.5).
fn journal_holds(journal: &Journal, id: &str, stage: CheckpointStage) -> bool {
    journal.entries().iter().any(|entry| {
        matches!(entry, JournalEntry::Commissioning { checkpoint } if checkpoint.id == id && checkpoint.stage == stage)
    })
}

/// Whether the journal holds any commissioning checkpoint (A8.5).
fn journal_holds_a_checkpoint(journal: &Journal) -> bool {
    journal
        .entries()
        .iter()
        .any(|entry| matches!(entry, JournalEntry::Commissioning { .. }))
}

/// The receipts of the study's Apply section that `key` lists (`applications`, `restorations` or `keeps`).
fn receipts(study: &Value, key: &str) -> Vec<Value> {
    study["sections"]["apply"][key]
        .as_array()
        .cloned()
        .unwrap_or_default()
}

/// The whole journey of Apply, Restore and Keep on lesson 30 (V4.6), in the order described in the module documentation.
///
/// Run time: ×10 (ten times real time), about a minute.
#[test]
#[allow(clippy::too_many_lines)] // One journey through the command line, in the engineer's order; each step depends on the one before.
fn apply_restore_and_keep_run_through_the_command_line_on_lesson_30() {
    let lab = Lab::start("apply-30", LESSON_30, LOOP_FILE, TIME_SCALE);
    let project = lab.project.clone();
    let loop_id = lab.loop_definition.id.clone();
    let mut findings = Findings::default();

    // The candidate: identify, tune and validate. Each one is needed by the next, so a failure ends the run.
    let (program, status) = lab.run("identify", "identify", &["--json"], true, LIMIT);
    findings.require(
        status.code() == Some(0) && program.answer().get("result").is_some(),
        format!(
            "identify finishes with its result (B3):\n{}",
            program.report(status)
        ),
    );
    let (program, status) = lab.run(
        "tune",
        "tune",
        &["--method", "simc", "--endpoint", UNREACHABLE, "--json"],
        false,
        LIMIT,
    );
    findings.require(
        status.code() == Some(0) && program.answer()["result"]["candidate"]["applied"] == false,
        format!(
            "tune designs an unapplied candidate (B4.1):\n{}",
            program.report(status)
        ),
    );
    let tuned_candidate = program.answer()["result"]["candidate"].clone();
    let (program, status) = lab.run("validate", "validate", &["--json"], true, LIMIT);
    findings.require(
        program.answer()["result"]["applyAvailable"] == true,
        format!(
            "validate leaves Apply available (B5.4):\n{}",
            program.report(status)
        ),
    );

    // The mode: the program starts the loop in Manual; Apply must find it in Auto, and Restore must return it there (A6.2).
    findings.check(
        lab.is_auto() == Some(false),
        "the program starts the loop in Manual (P1.AutoMode is FALSE)",
    );
    transfer(&lab, false);
    findings.require(
        lab.is_auto() == Some(true),
        "the loop is in Auto before Apply (the transfer confirmed it)",
    );

    // A review made in Auto, and then a review that is stale: the loop moves to Manual, so the identity no longer matches (E11.8).
    let (program, status) = lab.run("apply-review-stale", "apply", &["--json"], true, LIMIT);
    let stale_review = program.answer()["result"].clone();
    findings.require(
        status.code() == Some(0)
            && stale_review["action"] == "apply"
            && stale_review["foundMode"] == "auto"
            && text_of(&stale_review, "identity").starts_with("sha256:"),
        format!(
            "apply without --review answers the review in Auto (§0.1b):\n{}",
            program.report(status)
        ),
    );
    let stale_identity = text_of(&stale_review, "identity");
    findings.check(
        stale_review["physical"] == false && stale_review["confirmation"] == "press",
        "the review of a simulated target says the press of Apply confirms it (A2.4, A2.5)",
    );

    transfer(&lab, true);
    findings.check(
        lab.is_auto() == Some(false),
        "the loop is in Manual, so the review is stale",
    );
    let values_before_stale = lab.parameter_values();
    let stale = stale_identity.as_str();
    let (program, status) = lab.run(
        "apply-stale",
        "apply",
        &["--review", stale, "--json"],
        true,
        LIMIT,
    );
    findings.check(
        status.code() == Some(1) && program.answer()["failure"]["message"] == REVIEW_AGAIN,
        format!(
            "a stale identity is refused with status 1 and the review again (§0.1b):\n{}",
            program.report(status)
        ),
    );
    findings.check(
        same_values(&lab.parameter_values(), &values_before_stale),
        "the refused Apply writes no parameter (§0.1b)",
    );
    findings.check(
        study_of(&project, &loop_id)["sections"]["apply"].is_null(),
        "the refused Apply keeps no receipt (B6.7)",
    );
    findings.check(
        !journal_holds_a_checkpoint(&journal_of(&lab.journal, &lab.loop_definition)),
        "the refused Apply journals no checkpoint (E11.2)",
    );
    findings.check(
        lab.is_auto() == Some(false),
        "the refused Apply leaves the loop in Manual",
    );
    transfer(&lab, false);
    findings.require(lab.is_auto() == Some(true), "the loop is back in Auto");
    // The reference Apply finds: this Auto entry aligned it to the measurement (annex E6.4), so it is read here, not assumed.
    let found_reference = reference(&lab);
    findings.require(
        found_reference.is_some(),
        "the loop's reference is read before Apply (annex E6.3)",
    );

    // The review in Auto, in JSON and in text: they write nothing.
    let (program, status) = lab.run("apply-review", "apply", &["--json"], true, LIMIT);
    let review = program.answer()["result"].clone();
    let identity = text_of(&review, "identity");
    findings.require(
        status.code() == Some(0)
            && review["foundMode"] == "auto"
            && identity.starts_with("sha256:"),
        format!(
            "the review in Auto names its identity (§0.1b):\n{}",
            program.report(status)
        ),
    );
    let (text_program, text_status) = lab.run("apply-review-text", "apply", &[], true, LIMIT);
    let text = text_program.stdout_text();
    findings.check(
        text_status.code() == Some(0)
            && text.contains("The loop was found in auto mode.")
            && text.contains(&format!("Review identity: {identity}."))
            && text.contains("Nothing was written."),
        format!("the text review names the identity and says nothing was written:\n{text}"),
    );
    let values_before_apply = lab.parameter_values();
    findings.check(
        !next_values(&review).is_empty(),
        "the review lists each bound parameter with the value its confirmation writes (§0.1b)",
    );

    // Apply: exit 0, the record holds the candidate, the loop is in Auto, the study keeps the receipt, the journal the checkpoint.
    let (program, status) = lab.run(
        "apply",
        "apply",
        &["--review", identity.as_str(), "--json"],
        true,
        LIMIT,
    );
    let applied = program.answer();
    findings.require(
        status.code() == Some(0) && applied["result"]["result"]["status"] == "passed",
        format!(
            "the Apply passes and exits with status 0 (B6.3, §0.1b):\n{}",
            program.report(status)
        ),
    );
    findings.check(
        applied["result"]["restored"] == false
            && applied["result"]["restoreAvailable"] == true
            && applied["result"]["keepAvailable"] == true,
        "a passed Apply offers Restore and Keep, and restores nothing (B6.3)",
    );
    findings.check(
        applied["result"]["applyAvailable"] == false,
        "a passed Apply offers no second Apply: its settings stand, with Restore in the Apply position (B7)",
    );
    let checkpoint = text_of(&applied["result"], "checkpointId");
    findings.check(
        !checkpoint.is_empty() && !text_of(&applied["result"], "validationIdentity").is_empty(),
        "the Apply names its checkpoint and the validation it was reviewed on (A8.5)",
    );
    findings.check(
        same_values(&lab.parameter_values(), &next_values(&review)),
        "the runtime's record holds the candidate that the review named (B6.1, E7.3)",
    );
    findings.check(
        lab.is_auto() == Some(true),
        "Apply leaves the loop in Auto (A6.2)",
    );
    let applications = receipts(&study_of(&project, &loop_id), "applications");
    findings.check(
        applications.len() == 1
            && applications[0]["checkpoint"] == checkpoint.as_str()
            && applications[0]["settingsApplied"] == true,
        format!(
            "the study's Apply section holds the receipt of the Apply (A8.1): {applications:?}"
        ),
    );
    findings.check(
        journal_holds(
            &journal_of(&lab.journal, &lab.loop_definition),
            &checkpoint,
            CheckpointStage::Completed,
        ),
        "the journal holds the completed checkpoint of the Apply (A8.5)",
    );

    // Reopening after the Apply holds the study's candidate with its settings (A8.6): the Tune page shows it, and the manual method
    // starts from those settings.
    let (program, status) = lab.run("open-after-apply", "open", &["--json"], false, LIMIT);
    let opened = program.answer();
    findings.check(
        status.code() == Some(0)
            && opened["result"]["stageEvidence"]["hasCandidate"] == true
            && opened["result"]["tune"]["candidate"] == tuned_candidate
            && opened["result"]["tune"]["settings"]
                .as_object()
                .is_some_and(|settings| !settings.is_empty()),
        format!(
            "open after the Apply holds the study's candidate and its settings (A8.6):\n{}",
            program.report(status)
        ),
    );
    findings.check(
        opened["result"]["applyAvailable"] == false
            && opened["result"]["restoreAvailable"] == true
            && opened["result"]["keepAvailable"] == true,
        format!(
            "open after the Apply offers Restore and Keep in project and not Apply, though the validation passed (B7, A8.6):\n{}",
            program.report(status)
        ),
    );

    // With the checkpoint completed, Identify, Tune and Validate are not held back (B7).
    let (program, status) = lab.run("identify-after-apply", "identify", &["--json"], true, LIMIT);
    findings.check(
        status.code() == Some(0) && program.answer().get("result").is_some(),
        format!(
            "Identify is not held back by a completed checkpoint (B7):\n{}",
            program.report(status)
        ),
    );
    let (program, status) = lab.run(
        "tune-after-apply",
        "tune",
        &["--method", "simc", "--endpoint", UNREACHABLE, "--json"],
        false,
        LIMIT,
    );
    findings.check(
        status.code() == Some(0) && program.answer().get("result").is_some(),
        format!(
            "Tune is not held back by a completed checkpoint (B7):\n{}",
            program.report(status)
        ),
    );
    // Validate passes after Apply, on the selected model. Apply's target change stays in force (B6.3), so this Identify models the loop
    // at the changed reference, while the configured plant starts from its configured state (B5.3). Its target-response run starts
    // outside the settling band around the model's operating point, so every case runs on the model, and the result says why (B5.3).
    // Status 0 is a validation that passed (§0.1b).
    let (program, status) = lab.run("validate-after-apply", "validate", &["--json"], true, LIMIT);
    let validated = program.answer()["result"].clone();
    findings.check(
        status.code() == Some(0)
            && validated["status"] == "Pass"
            && validated["applyAvailable"] == true,
        format!(
            "Validate is not held back by a completed checkpoint and passes, leaving Apply available (B7):\n{}",
            program.report(status)
        ),
    );
    findings.check(
        validated["scenarios"].as_array().is_some_and(|scenarios| {
            !scenarios.is_empty()
                && scenarios.iter().all(|scenario| {
                    scenario["plant"]
                        .as_str()
                        .is_some_and(|plant| plant.starts_with("model "))
                })
        }),
        "every scenario ran on the selected model (B5.3)",
    );
    findings.check(
        validated["plantReason"]
            .as_str()
            .is_some_and(|reason| !reason.is_empty()),
        "the result says why the model replaced the configured plant (B5.3)",
    );

    // Restore previous settings: review, then confirmation. The record returns to the values from before Apply, the loop to Auto.
    let (program, status) = lab.run("restore-review", "restore", &["--json"], true, LIMIT);
    let restore_review = program.answer()["result"].clone();
    findings.require(
        status.code() == Some(0)
            && restore_review["action"] == "restore"
            && restore_review["foundMode"] == "auto",
        format!(
            "restore without --review answers its review (§0.1b):\n{}",
            program.report(status)
        ),
    );
    findings.check(
        same_values(&next_values(&restore_review), &values_before_apply),
        "the Restore's review names the values from before Apply (B6.4, E7.5)",
    );
    findings.check(
        restore_review["physical"] == false && restore_review["confirmation"] == "press",
        "the Restore's review of a simulated target says the press confirms it (A2.4, A2.5)",
    );
    let restore_identity = text_of(&restore_review, "identity");
    let (program, status) = lab.run(
        "restore",
        "restore",
        &["--review", restore_identity.as_str(), "--json"],
        true,
        LIMIT,
    );
    let restored = program.answer();
    findings.check(
        status.code() == Some(0)
            && restored["result"]["restored"] == true
            && restored["result"]["mode"] == "auto",
        format!(
            "Restore is confirmed by the runtime and names Auto (B6.4):\n{}",
            program.report(status)
        ),
    );
    findings.check(
        restored["result"]["restoreAvailable"] == false
            && restored["result"]["keepAvailable"] == false
            && restored["result"]["applyAvailable"] == false,
        "a confirmed Restore offers neither Restore nor Keep in project, and no Apply until a validation passes again (B6.4, B6.5, B7)",
    );
    findings.check(
        same_values(&lab.parameter_values(), &values_before_apply),
        "Restore returns the parameter record to the values from before Apply (B6.4)",
    );
    findings.check(
        lab.is_auto() == Some(true),
        "Restore returns the loop to Auto, the mode it was found in before Apply (A6.2, B6.4)",
    );
    let restored_reference = reference(&lab);
    findings.check(
        restored_reference.is_some() && restored_reference == found_reference,
        format!(
            "Restore returns the reference Apply found, {found_reference:?} °C, and keeps it in Auto (B6.4, E6.4): {restored_reference:?}"
        ),
    );
    let restorations = receipts(&study_of(&project, &loop_id), "restorations");
    findings.check(
        restorations.len() == 1
            && restorations[0]["restored"] == true
            && restorations[0]["checkpoint"] == checkpoint.as_str(),
        format!("the study keeps the Restore's receipt (A8.1): {restorations:?}"),
    );
    findings.check(
        !journal_holds_a_checkpoint(&journal_of(&lab.journal, &lab.loop_definition)),
        "a confirmed Restore clears the checkpoint it returned (B6.4)",
    );

    // Keep in project is refused after a Restore, and writes nothing (B6.5, B7).
    let before_refused_keep = snapshot(&project);
    let (program, status) = lab.run("keep-refused", "keep", &["--json"], true, LIMIT);
    findings.check(
        status.code() == Some(1) && program.answer().get("failure").is_some(),
        format!(
            "Keep is refused after a Restore (B6.5, B7):\n{}",
            program.report(status)
        ),
    );
    findings.check(
        snapshot(&project) == before_refused_keep,
        "the refused Keep changes no file of the project (B6.5)",
    );

    // A new candidate at the operating point Restore returned to (the reference Apply found, B6.4): Identify, Tune and Validate again,
    // since the study's model is the one identified at the changed reference.
    let (program, status) = lab.run("identify-restored", "identify", &["--json"], true, LIMIT);
    findings.require(
        status.code() == Some(0) && program.answer().get("result").is_some(),
        format!(
            "Identify finishes at the restored operating point (B3):\n{}",
            program.report(status)
        ),
    );
    let (program, status) = lab.run(
        "tune-restored",
        "tune",
        &["--method", "simc", "--endpoint", UNREACHABLE, "--json"],
        false,
        LIMIT,
    );
    findings.require(
        status.code() == Some(0) && program.answer()["result"]["candidate"]["applied"] == false,
        format!(
            "Tune designs an unapplied candidate at the restored operating point (B4.1):\n{}",
            program.report(status)
        ),
    );
    let (program, status) = lab.run("validate-restored", "validate", &["--json"], true, LIMIT);
    findings.require(
        status.code() == Some(0) && program.answer()["result"]["applyAvailable"] == true,
        format!(
            "Validate leaves Apply available at the restored operating point (B5.4):\n{}",
            program.report(status)
        ),
    );

    // A second Apply, then Keep in project: review, then confirmation.
    let (program, status) = lab.run("apply-review-2", "apply", &["--json"], true, LIMIT);
    let review_two = program.answer()["result"].clone();
    let identity_two = text_of(&review_two, "identity");
    findings.require(
        status.code() == Some(0) && identity_two.starts_with("sha256:"),
        format!(
            "the second review answers its identity (§0.1b):\n{}",
            program.report(status)
        ),
    );
    let (program, status) = lab.run(
        "apply-2",
        "apply",
        &["--review", identity_two.as_str(), "--json"],
        true,
        LIMIT,
    );
    let applied_two = program.answer();
    findings.require(
        status.code() == Some(0) && applied_two["result"]["result"]["status"] == "passed",
        format!(
            "the second Apply passes (B6.3):\n{}",
            program.report(status)
        ),
    );
    findings.check(
        same_values(&lab.parameter_values(), &next_values(&review_two)),
        "the second Apply writes the record its review named (B6.1)",
    );
    let checkpoint_two = text_of(&applied_two["result"], "checkpointId");
    findings.check(
        journal_holds(
            &journal_of(&lab.journal, &lab.loop_definition),
            &checkpoint_two,
            CheckpointStage::Completed,
        ),
        "the journal holds the second Apply's completed checkpoint (A8.5)",
    );

    let main_before = read_text(&project.join("src/Main.st"));
    let (program, status) = lab.run("keep-review", "keep", &["--json"], true, LIMIT);
    let keep_review = program.answer()["result"].clone();
    findings.require(
        status.code() == Some(0)
            && keep_review["record"] == "P1.Parameters"
            && names(&keep_review, "kept")
                .iter()
                .any(|name| name == "ProportionalGain")
            && keep_review["files"]
                .as_array()
                .is_some_and(|files| files.len() == 1),
        format!(
            "Keep reviews the record P1.Parameters and the file it changes (B6.5):\n{}",
            program.report(status)
        ),
    );
    let keep_identity = text_of(&keep_review, "identity");
    let after = text_of(&keep_review["files"][0], "after");
    findings.check(
        keep_review["files"][0]["before"].as_str() == Some(main_before.as_str())
            && after != main_before,
        "the review shows the file as it is and the text the Keep writes",
    );
    let (program, status) = lab.run(
        "keep",
        "keep",
        &["--review", keep_identity.as_str(), "--json"],
        true,
        LIMIT,
    );
    let kept = program.answer();
    findings.check(
        status.code() == Some(0)
            && kept["result"]["record"] == "P1.Parameters"
            && names(&kept["result"], "kept")
                .iter()
                .any(|name| name == "ProportionalGain"),
        format!(
            "Keep in project is confirmed and names the fields it kept (B6.5):\n{}",
            program.report(status)
        ),
    );
    findings.check(
        kept["result"]["restoreAvailable"] == true && kept["result"]["keepAvailable"] == false,
        "a confirmed Keep still offers Restore previous settings and no Keep in project for the values it kept (B6.5, B7)",
    );
    findings.check(
        keep_review["confirmation"] == "explicit",
        "a Keep review is confirmed with its own button on any target (B6.5)",
    );
    findings.check(
        read_text(&project.join("src/Main.st")) == after,
        "Keep writes the running values into the record's initializer, as the review showed (B6.5)",
    );
    let study = study_of(&project, &loop_id);
    findings.check(
        receipts(&study, "keeps").len() == 1 && receipts(&study, "applications").len() == 2,
        "the study keeps the Keep's receipt beside the two Apply receipts (A8.1)",
    );
    findings.conclude();
}
