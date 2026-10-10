//! The lifecycle of a Test experiment (`refreshTestExperiment` in `testExperimentLifecycle.ts`; specification 35, A8.5 and
//! A8.6; annex E11.4 and E11.5). The decisions are in `trust-control` (`identification::test_lifecycle`). This module makes
//! the calls that each decision needs, in the order of `refreshTestExperiment`: the recovery of the recording owner, the
//! status read, the conclusion of an ended experiment, and the phase the Test posts. The host of the Test implements
//! [`TestExperimentLifecycle`] and holds the journal, the runtime and the page.

// The trait keeps the name of the lifecycle it models.
#![allow(clippy::module_name_repetitions)]

use serde_json::{json, Value};

use trust_control::experiment::{ControlExperimentPlan, RecordingRecoveryOwner};
use trust_control::failure::Failure;
use trust_control::identification::reconcile::{experiment_identity, StatusRead};
use trust_control::identification::test_lifecycle::{
    failure_text, identity_step, recovery_step, recovery_unconfirmed_text, status_step,
    IdentityStep, PostedPhase, RecoveryStep, StatusStep, INVALID_STATE_TEXT, PLAN_MISSING_TEXT,
};
use trust_control::loop_definition::LoopDefinition;

/// A Test's journalled recovery (`TestExperimentRecovery` in `testExperimentLifecycle.ts`): its reviewed plan and its
/// recording owner, each present when it is journalled.
#[derive(Debug, Clone, Default)]
pub struct TestExperimentRecovery {
    /// The reviewed plan of the experiment.
    pub plan: Option<ControlExperimentPlan>,
    /// The recording owner that the experiment runs with.
    pub recording_owner: Option<RecordingRecoveryOwner>,
}

/// The host of a Test's lifecycle (`TestExperimentLifecycle` in `testExperimentLifecycle.ts`). The host holds the Test's
/// identifier, its loop, its journal, the runtime and the page. Each method is one call of the lifecycle, and a method
/// that can fail returns the failure that the call throws.
pub trait TestExperimentLifecycle {
    /// The identifier of the Test's experiment, when one is held (`experimentId()`).
    fn experiment_id(&self) -> Option<String>;

    /// The loop of the Test, when one is active (`loop()`).
    fn loop_definition(&self) -> Option<LoopDefinition>;

    /// The journalled recovery of the Test for `loop_definition`, when there is one (`recovery(loop)`).
    fn recovery(&self, loop_definition: &LoopDefinition) -> Option<TestExperimentRecovery>;

    /// Whether a recording is active (`recordingActive()`).
    fn recording_active(&self) -> bool;

    /// Recovers the recording owner for `loop_definition` (`recoverOwner(loop, owner)`).
    ///
    /// # Errors
    /// The owner cannot be recovered. The flow posts `failed` with the text that the recovery remains unconfirmed.
    fn recover_owner(
        &mut self,
        loop_definition: &LoopDefinition,
        owner: &RecordingRecoveryOwner,
    ) -> Result<(), Failure>;

    /// Records that the plan is recovered with its recording (`recovered(plan)`).
    fn recovered(&mut self, plan: &ControlExperimentPlan);

    /// Reads the status of the experiment by its identifier (`status(experimentId)`).
    fn status(&mut self, experiment_id: &str) -> StatusRead;

    /// Builds the conclusion of an ended experiment from its status record (`terminal(value)`).
    ///
    /// # Errors
    /// The conclusion cannot be built from the record.
    fn terminal(&mut self, state: &Value) -> Result<Value, Failure>;

    /// Observes the record of an ended experiment before it is concluded (`terminalObserved(value)`).
    fn terminal_observed(&mut self, state: &Value);

    /// Clears the Test's journal (`clear()`).
    ///
    /// # Errors
    /// The journal cannot be cleared.
    fn clear(&mut self) -> Result<(), Failure>;

    /// Posts a phase with its payload to the page (`post(phase, payload)`).
    fn post(&mut self, phase: PostedPhase, payload: Value);

    /// Schedules the next status read of the experiment (`schedule()`).
    fn schedule(&mut self);

    /// Whether the start of `experiment_id` is still landing: journalled, and not yet answered or failed (A8.5). The
    /// optional `startLanding` of `testExperimentLifecycle.ts` defaults to `false`; here the method is required.
    fn start_landing(&self, experiment_id: &str) -> bool;
}

/// Keeps a bounded Test alive through runtime reconnect and terminal restoration (`refreshTestExperiment`). Without an
/// identifier or a loop it does nothing. Without an active recording it first recovers its journal: a Test journalled with
/// its identity only is reconciled by its status, a recording owner without its plan fails, and a recording owner is
/// recovered. Then the experiment's status is read, and the Test posts the phase that the status gives.
///
/// # Errors
/// A Test journalled with its identity only returns the failure of concluding an ended experiment or of clearing its
/// journal, and posts nothing, as `testExperimentLifecycle.ts` rejects. Every other failure is posted as `failed`.
pub fn refresh_test_experiment(
    lifecycle: &mut impl TestExperimentLifecycle,
) -> Result<(), Failure> {
    let Some(experiment_id) = lifecycle.experiment_id().filter(|id| !id.is_empty()) else {
        return Ok(());
    };
    let Some(loop_definition) = lifecycle.loop_definition() else {
        return Ok(());
    };
    if !lifecycle.recording_active() {
        let recovery = lifecycle.recovery(&loop_definition).unwrap_or_default();
        match recovery_step(recovery.recording_owner.as_ref(), recovery.plan.as_ref()) {
            RecoveryStep::ByIdentity => return reconcile_by_identity(lifecycle, &experiment_id),
            RecoveryStep::PlanMissing => {
                post_failed(lifecycle, PLAN_MISSING_TEXT);
                return Ok(());
            }
            RecoveryStep::Recover { owner, plan } => {
                if let Err(failure) = lifecycle.recover_owner(&loop_definition, owner) {
                    post_failed(lifecycle, &recovery_unconfirmed_text(&failure));
                    return Ok(());
                }
                lifecycle.recovered(plan);
            }
        }
    }
    if let Err(reason) = read_and_follow(lifecycle, &experiment_id) {
        post_failed(lifecycle, &reason);
    }
    Ok(())
}

/// Reconciles a Test journalled with its identity only, by the experiment's status. Nothing is started or sent again: a
/// running experiment is followed, an ended one is concluded and cleared, and an identity that the runtime does not hold
/// is cleared, unless its start is still landing (annex E11.4 and E11.5).
fn reconcile_by_identity(
    lifecycle: &mut impl TestExperimentLifecycle,
    experiment_id: &str,
) -> Result<(), Failure> {
    let identity = experiment_identity(experiment_id, lifecycle.status(experiment_id));
    let start_landing = lifecycle.start_landing(experiment_id);
    match identity_step(identity, start_landing) {
        IdentityStep::Attach { phase, state } => {
            lifecycle.schedule();
            lifecycle.post(phase, state);
        }
        IdentityStep::Conclude { state } => {
            lifecycle.terminal_observed(&state);
            let payload = lifecycle.terminal(&state)?;
            lifecycle.clear()?;
            lifecycle.post(PostedPhase::Terminal, payload);
        }
        IdentityStep::KeepJournal => {}
        IdentityStep::ClearAndFail { reason } => {
            lifecycle.clear()?;
            post_failed(lifecycle, &reason);
        }
        IdentityStep::Fail { reason } => post_failed(lifecycle, &reason),
    }
    Ok(())
}

/// Reads the experiment's status, and then follows it, concludes it, or fails. A failure is returned as the reason that the
/// Test posts.
fn read_and_follow(
    lifecycle: &mut impl TestExperimentLifecycle,
    experiment_id: &str,
) -> Result<(), String> {
    let value = status_answer(lifecycle, experiment_id)?;
    match status_step(&value, lifecycle.recording_active()) {
        StatusStep::Invalid => Err(INVALID_STATE_TEXT.to_string()),
        StatusStep::Follow { phase } => {
            lifecycle.schedule();
            lifecycle.post(phase, value);
            Ok(())
        }
        StatusStep::Conclude { clears_journal } => conclude(lifecycle, &value, clears_journal),
    }
}

/// The answer of a status read, or the text that the Test posts when the runtime refused the read or it could not be made.
fn status_answer(
    lifecycle: &mut impl TestExperimentLifecycle,
    experiment_id: &str,
) -> Result<Value, String> {
    match lifecycle.status(experiment_id) {
        StatusRead::Answer(value) => Ok(value),
        StatusRead::Refused { text, .. } | StatusRead::Unreadable(text) => Err(text),
    }
}

/// Concludes an ended experiment. Its record is observed and concluded. Its journal is then cleared when no recording is
/// active, and otherwise the follow is scheduled. The Test posts `terminal` with the conclusion.
fn conclude(
    lifecycle: &mut impl TestExperimentLifecycle,
    value: &Value,
    clears_journal: bool,
) -> Result<(), String> {
    lifecycle.terminal_observed(value);
    let payload = lifecycle
        .terminal(value)
        .map_err(|failure| failure_text(&failure))?;
    if clears_journal {
        lifecycle
            .clear()
            .map_err(|failure| failure_text(&failure))?;
    } else {
        lifecycle.schedule();
    }
    lifecycle.post(PostedPhase::Terminal, payload);
    Ok(())
}

/// Posts the `failed` phase with its reason.
fn post_failed(lifecycle: &mut impl TestExperimentLifecycle, reason: &str) {
    lifecycle.post(PostedPhase::Failed, json!({ "reason": reason }));
}

#[cfg(test)]
mod tests {
    use serde_json::{json, Value};

    use trust_control::experiment::{ControlExperimentPlan, RecordingRecoveryOwner};
    use trust_control::failure::{Cause, Failure, NextAction};
    use trust_control::identification::reconcile::StatusRead;
    use trust_control::identification::test_lifecycle::{PostedPhase, INVALID_STATE_TEXT};
    use trust_control::loop_definition::LoopDefinition;

    use super::{refresh_test_experiment, TestExperimentLifecycle, TestExperimentRecovery};

    /// A Test host that records its calls in order, as the test doubles of the `control-studio` suites do. Reads of the host
    /// are not recorded.
    struct FakeHost {
        experiment: Option<String>,
        loop_def: Option<LoopDefinition>,
        journal: Option<TestExperimentRecovery>,
        recording: bool,
        landing: bool,
        scripted: StatusRead,
        terminal_failure: Option<Failure>,
        calls: Vec<String>,
        posted: Vec<(PostedPhase, Value)>,
    }

    impl TestExperimentLifecycle for FakeHost {
        fn experiment_id(&self) -> Option<String> {
            self.experiment.clone()
        }

        fn loop_definition(&self) -> Option<LoopDefinition> {
            self.loop_def.clone()
        }

        fn recovery(&self, _loop_definition: &LoopDefinition) -> Option<TestExperimentRecovery> {
            self.journal.clone()
        }

        fn recording_active(&self) -> bool {
            self.recording
        }

        fn recover_owner(
            &mut self,
            _loop_definition: &LoopDefinition,
            _owner: &RecordingRecoveryOwner,
        ) -> Result<(), Failure> {
            self.calls.push("recover".to_string());
            Ok(())
        }

        fn recovered(&mut self, _plan: &ControlExperimentPlan) {
            self.calls.push("recovered".to_string());
        }

        fn status(&mut self, _experiment_id: &str) -> StatusRead {
            self.scripted.clone()
        }

        fn terminal(&mut self, state: &Value) -> Result<Value, Failure> {
            self.calls.push("terminal".to_string());
            match &self.terminal_failure {
                Some(failure) => Err(failure.clone()),
                None => Ok(json!({ "concluded": state })),
            }
        }

        fn terminal_observed(&mut self, _state: &Value) {
            self.calls.push("terminal_observed".to_string());
        }

        fn clear(&mut self) -> Result<(), Failure> {
            self.calls.push("clear".to_string());
            Ok(())
        }

        fn post(&mut self, phase: PostedPhase, payload: Value) {
            self.calls.push(format!("post:{}", word(phase)));
            self.posted.push((phase, payload));
        }

        fn schedule(&mut self) {
            self.calls.push("schedule".to_string());
        }

        fn start_landing(&self, _experiment_id: &str) -> bool {
            self.landing
        }
    }

    fn word(phase: PostedPhase) -> &'static str {
        match phase {
            PostedPhase::Running => "running",
            PostedPhase::Restoring => "restoring",
            PostedPhase::Terminal => "terminal",
            PostedPhase::Failed => "failed",
        }
    }

    /// The tank loop of the test suites, with the fields that the loop contract requires.
    fn tank_loop() -> LoopDefinition {
        serde_json::from_value(json!({
            "schemaVersion": 2, "id": "tank", "name": "Tank", "revision": "rev-1",
            "projectId": "project", "runtimeId": "runtime",
            "controller": { "instance": "Main.TIC101", "type": "Control.PID", "revision": "sha256:test",
                "action": "direct", "taskPeriodMs": 100 },
            "signals": {
                "processVariable": { "symbol": "Main.TT101.Value", "iecType": "LREAL", "direction": "read",
                    "unit": "degC", "range": { "minimum": 0, "maximum": 100 } },
                "manipulatedVariable": { "symbol": "Main.TIC101_Output", "iecType": "LREAL", "direction": "write",
                    "unit": "%", "range": { "minimum": 0, "maximum": 100 } }
            },
            "limits": { "outputMinimum": 0, "outputMaximum": 100 },
            "observedSampleTiming": { "nominalPeriodMs": 100 }
        }))
        .expect("the tank loop is a loop definition")
    }

    /// A Test of `experiment-1` with its loop, no journal, and the runtime answer `scripted`.
    fn test_host(scripted: StatusRead) -> FakeHost {
        FakeHost {
            experiment: Some("experiment-1".to_string()),
            loop_def: Some(tank_loop()),
            journal: None,
            recording: false,
            landing: false,
            scripted,
            terminal_failure: None,
            calls: Vec::new(),
            posted: Vec::new(),
        }
    }

    /// A Test journalled with its identity only: its journal exists, but holds no recording owner (A8.5, E11.4).
    fn identity_only(scripted: StatusRead) -> FakeHost {
        FakeHost {
            journal: Some(TestExperimentRecovery::default()),
            ..test_host(scripted)
        }
    }

    fn refused(text: &str) -> StatusRead {
        StatusRead::Refused {
            code: Some("experiment_not_found".to_string()),
            text: text.to_string(),
        }
    }

    fn count(host: &FakeHost, call: &str) -> usize {
        host.calls
            .iter()
            .filter(|recorded| recorded.as_str() == call)
            .count()
    }

    fn phases(host: &FakeHost) -> Vec<PostedPhase> {
        host.posted.iter().map(|(phase, _)| *phase).collect()
    }

    fn reason(host: &FakeHost) -> &str {
        host.posted[0].1["reason"]
            .as_str()
            .expect("a failure carries its reason")
    }

    #[test]
    fn a_running_identity_is_attached_and_followed_without_recovering_a_recording_owner() {
        let mut host = identity_only(StatusRead::Answer(json!({
            "experiment_id": "experiment-1", "status": "running", "restoration": "pending", "stop_reason": null
        })));
        refresh_test_experiment(&mut host).expect("a running identity is reconciled");
        assert_eq!(phases(&host), [PostedPhase::Running]);
        assert_eq!(
            count(&host, "schedule"),
            1,
            "the experiment keeps being followed"
        );
        assert_eq!(
            count(&host, "recover") + count(&host, "recovered"),
            0,
            "no recording owner is recovered from an identity alone"
        );
        assert_eq!(count(&host, "clear"), 0);
    }

    #[test]
    fn a_terminal_identity_is_concluded_and_its_journal_cleared() {
        let mut host = identity_only(StatusRead::Answer(json!({
            "experiment_id": "experiment-1", "status": "completed", "restoration": "restored",
            "stop_reason": "duration_complete"
        })));
        refresh_test_experiment(&mut host).expect("an ended identity is concluded");
        assert_eq!(phases(&host), [PostedPhase::Terminal]);
        assert_eq!(
            count(&host, "terminal_observed"),
            1,
            "the record is observed once"
        );
        assert_eq!(count(&host, "clear"), 1, "a concluded identity is cleared");
    }

    #[test]
    fn an_identity_the_runtime_does_not_hold_is_cleared_since_nothing_was_applied() {
        let mut host = identity_only(refused("No experiment holds this identifier."));
        refresh_test_experiment(&mut host).expect("an absent identity is reconciled");
        assert_eq!(phases(&host), [PostedPhase::Failed]);
        let text = reason(&host);
        assert!(text.contains("holds no experiment"), "{text}");
        assert_eq!(
            count(&host, "clear"),
            1,
            "nothing was applied, so the journal clears"
        );
    }

    #[test]
    fn an_identity_whose_status_cannot_be_read_stays_journalled_for_the_next_reconciliation() {
        let mut host = identity_only(StatusRead::Unreadable("transport lost".to_string()));
        refresh_test_experiment(&mut host).expect("an unreadable identity is posted, not returned");
        assert_eq!(phases(&host), [PostedPhase::Failed]);
        let text = reason(&host);
        assert!(text.contains("could not be read"), "{text}");
        assert_eq!(count(&host, "clear"), 0, "an unreadable identity is kept");
    }

    #[test]
    fn an_absent_identity_whose_start_is_still_landing_keeps_its_journal() {
        let mut landing = FakeHost {
            landing: true,
            journal: None,
            ..identity_only(refused("no such experiment"))
        };
        refresh_test_experiment(&mut landing).expect("a landing start is kept, not returned");
        assert!(
            landing.calls.is_empty(),
            "an unanswered start is not treated as never started: nothing is cleared or posted"
        );

        let mut settled = FakeHost {
            journal: None,
            ..identity_only(refused("no such experiment"))
        };
        refresh_test_experiment(&mut settled).expect("an answered absence is posted");
        assert_eq!(
            settled.calls,
            ["clear", "post:failed"],
            "once the start has failed or been answered, absent means it never started"
        );
    }

    #[test]
    fn while_a_recording_is_active_a_running_status_is_followed_and_posted_as_running() {
        let record = json!({ "status": "running", "restoration": "pending" });
        let mut host = FakeHost {
            recording: true,
            ..test_host(StatusRead::Answer(record.clone()))
        };
        refresh_test_experiment(&mut host).expect("a running status is followed");
        assert_eq!(host.calls, ["schedule", "post:running"]);
        assert_eq!(host.posted[0].1, record, "the page receives the record");
    }

    #[test]
    fn a_status_outside_the_vocabulary_fails_with_the_runtime_state_sentence() {
        let paused = json!({ "status": "paused", "restoration": "pending" });
        let mut host = FakeHost {
            recording: true,
            ..test_host(StatusRead::Answer(paused))
        };
        refresh_test_experiment(&mut host).expect("an invalid status is posted, not returned");
        assert_eq!(phases(&host), [PostedPhase::Failed]);
        assert_eq!(reason(&host), INVALID_STATE_TEXT);
        assert_eq!(host.calls, ["post:failed"]);
    }

    #[test]
    fn a_conclusion_that_fails_on_an_identity_is_returned_and_neither_cleared_nor_posted() {
        let ended = json!({
            "status": "completed", "restoration": "restored", "stop_reason": "duration_complete"
        });
        let mut host = FakeHost {
            terminal_failure: Some(Failure::new(
                "The record has no outcome.",
                Cause::ModelOrData,
                NextAction::None,
            )),
            ..identity_only(StatusRead::Answer(ended))
        };
        let Err(failure) = refresh_test_experiment(&mut host) else {
            panic!("the conclusion failure is returned to the caller");
        };
        assert_eq!(failure.message, "The record has no outcome.");
        assert!(host.posted.is_empty(), "nothing is posted");
        assert_eq!(
            count(&host, "clear"),
            0,
            "the journal stays when the conclusion fails"
        );
    }
}
