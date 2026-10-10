//! Port of `control-studio-experiment-recording.test.ts` (all six tests), of the window and start assertions of
//! `control-studio-next-actions.test.ts`, and of the Test-journal gate of `control-studio-study-store.test.ts`. The
//! sessions are fakes that log their calls in order, as the `TypeScript` fakes do. The owner's start, journal and keep tests
//! of `control-studio-lifecycle.test.ts` and `control-studio-final-wiring.test.ts` are the last ones in the file.

use std::cell::Cell;
use std::sync::Arc;

use parking_lot::Mutex;
use trust_control::experiment::{ControlEvidenceSource, RecordingRecoveryOwner};
use trust_control::failure::{Cause, Failure, NextAction};
use trust_control::loop_definition::LoopDefinition;

use crate::runtime_client::fake_runtime::tank_loop;

use super::*;
use crate::runtime_client::runtime_recording::{
    RecordingOwnerPersistence, RecordingStartError, RecordingStartUnconfirmed,
};

type Events = Arc<Mutex<Vec<String>>>;

/// A recording session that logs its stop and its release, and stops with the run `run`.
struct Session {
    loop_definition: LoopDefinition,
    stop_event: String,
    run: String,
    owner: Option<RecordingRecoveryOwner>,
    events: Events,
}

impl Session {
    fn new(loop_definition: &LoopDefinition, stop_event: &str, events: &Events) -> Self {
        Self {
            loop_definition: loop_definition.clone(),
            stop_event: stop_event.to_string(),
            run: "complete".to_string(),
            owner: None,
            events: Arc::clone(events),
        }
    }

    fn with_owner(mut self, owner: RecordingRecoveryOwner) -> Self {
        self.owner = Some(owner);
        self
    }
}

impl RecordingSession for Session {
    type Recording = String;

    fn loop_definition(&self) -> &LoopDefinition {
        &self.loop_definition
    }

    fn stop(&mut self) -> Result<String, Failure> {
        self.events.lock().push(self.stop_event.clone());
        Ok(self.run.clone())
    }

    fn release(&mut self) -> Result<(), Failure> {
        self.events.lock().push("release".to_string());
        Ok(())
    }

    fn recovery_owner(&self) -> Option<RecordingRecoveryOwner> {
        self.owner.clone()
    }
}

/// A journal that accepts every write, for the tests that do not read it.
struct Journal;

impl RecordingOwnerPersistence for Journal {
    fn persist(&self, _owner: &RecordingRecoveryOwner) -> Result<(), Failure> {
        Ok(())
    }

    fn clear(&self, _owner: &RecordingRecoveryOwner) -> Result<(), Failure> {
        Ok(())
    }
}

/// A journal that keeps the recording id of every owner it writes, so that a test can see what the start wrote through it.
struct WritingJournal {
    writes: Events,
}

impl RecordingOwnerPersistence for WritingJournal {
    fn persist(&self, owner: &RecordingRecoveryOwner) -> Result<(), Failure> {
        self.writes.lock().push(owner.recording_id.clone());
        Ok(())
    }

    fn clear(&self, _owner: &RecordingRecoveryOwner) -> Result<(), Failure> {
        Ok(())
    }
}

/// The loop of the Test tests (`projectId` `project:test`, runtime `simulation`, id `loop:test`, revision `sha256:test`).
fn test_loop() -> LoopDefinition {
    let mut loop_definition = tank_loop();
    loop_definition.project_id = "project:test".into();
    loop_definition.runtime_id = "simulation".into();
    loop_definition.id = "loop:test".into();
    loop_definition.revision = "sha256:test".into();
    loop_definition
}

fn owner_of(recording_id: &str) -> RecordingRecoveryOwner {
    RecordingRecoveryOwner {
        schema_version: 1,
        recording_id: recording_id.into(),
        project_id: "project:test".into(),
        runtime_id: "simulation".into(),
        loop_id: "loop:test".into(),
        loop_revision: "sha256:test".into(),
        source: ControlEvidenceSource::Simulation,
        program_revision: None,
        start_acknowledgement_unconfirmed: None,
        bindings: Vec::new(),
    }
}

#[test]
fn starts_and_stops_one_runtime_owned_test_recording_without_using_the_manual_owner() {
    let events: Events = Arc::default();
    let start_events = Arc::clone(&events);
    let owner = ExperimentRecordingOwner::new(move |requested, _persistence, _duration| {
        start_events.lock().push(format!("start:{}", requested.id));
        Ok(Session::new(
            requested,
            &format!("stop:{}", requested.id),
            &start_events,
        ))
    });
    let loop_definition = test_loop();
    let started = owner.start(&loop_definition, None, None).expect("started");
    assert!(matches!(started, RecordingTransition::Started));
    assert!(owner.recording());
    assert_eq!(owner.loop_definition(), Some(loop_definition.clone()));
    match owner.stop(&loop_definition, None).expect("stopped") {
        RecordingTransition::Stopped(run) => assert_eq!(run, "complete"),
        _ => panic!("the recording stops"),
    }
    assert!(!owner.recording());
    assert_eq!(*events.lock(), ["start:loop:test", "stop:loop:test"]);
}

#[test]
fn does_not_turn_an_uncertain_start_acknowledgement_into_test_authority() {
    let owner: ExperimentRecordingOwner<Session> = ExperimentRecordingOwner::new(|_, _, _| {
        Err(RecordingStartError::Failed(Failure::unreachable(
            "socket closed before the Start acknowledgement",
        )))
    });
    let failure = owner
        .start(&test_loop(), None, None)
        .err()
        .expect("the start fails");
    assert!(
        failure
            .detail
            .as_deref()
            .unwrap_or_default()
            .contains("socket closed"),
        "{failure:?}"
    );
    assert!(!owner.recording());
}

#[test]
fn starts_runtime_acquisition_before_excitation_and_stops_it_when_the_excitation_start_fails() {
    let events: Events = Arc::default();
    let start_events = Arc::clone(&events);
    let owner = ExperimentRecordingOwner::new(move |requested, _persistence, _duration| {
        start_events.lock().push("recording-start".to_string());
        Ok(Session::new(requested, "recording-stop", &start_events))
    });
    let loop_definition = test_loop();
    let excitation_events = Arc::clone(&events);
    let refused = start_experiment_with_recording(
        &owner,
        &loop_definition,
        || {
            excitation_events
                .lock()
                .push("experiment-start".to_string());
            Err::<&str, _>(Failure::new(
                "runtime rejected the reviewed test",
                Cause::Runtime,
                NextAction::None,
            ))
        },
        Some(&Journal),
        None,
        None,
    )
    .expect_err("the excitation is rejected");
    assert!(refused.message.contains("runtime rejected"), "{refused:?}");
    assert_eq!(
        *events.lock(),
        ["recording-start", "experiment-start", "recording-stop"]
    );
    assert!(!owner.recording());
}

#[test]
fn persists_the_acknowledged_test_recording_owner_before_excitation_dispatch() {
    let events: Events = Arc::default();
    let start_events = Arc::clone(&events);
    let owner = ExperimentRecordingOwner::new(move |requested, _persistence, _duration| {
        start_events.lock().push("recording-start".to_string());
        Ok(Session::new(requested, "recording-stop", &start_events)
            .with_owner(owner_of("test-recording")))
    });
    let loop_definition = test_loop();
    let persist_events = Arc::clone(&events);
    let persist = move |persisted: &RecordingRecoveryOwner| -> Result<(), Failure> {
        persist_events
            .lock()
            .push(format!("persist:{}", persisted.recording_id));
        Ok(())
    };
    let excitation_events = Arc::clone(&events);
    let started = start_experiment_with_recording(
        &owner,
        &loop_definition,
        move || {
            excitation_events
                .lock()
                .push("experiment-start".to_string());
            Ok("started")
        },
        Some(&Journal),
        Some(&persist),
        None,
    );
    assert_eq!(started.expect("started"), "started");
    assert_eq!(
        *events.lock(),
        [
            "recording-start",
            "persist:test-recording",
            "experiment-start"
        ]
    );
}

#[test]
fn adopts_a_recovered_test_owner_for_original_buffer_stop_only() {
    let events: Events = Arc::default();
    let owner: ExperimentRecordingOwner<Session> = ExperimentRecordingOwner::new(|_, _, _| {
        Err(RecordingStartError::Failed(Failure::new(
            "replacement Start must not run",
            Cause::Implementation,
            NextAction::None,
        )))
    });
    let loop_definition = test_loop();
    owner
        .adopt_recovered_owner(
            Session::new(&loop_definition, "stop:original", &events),
            "Test recording recovery unconfirmed — Stop the original buffer.",
        )
        .expect("adopted");
    assert!(owner.recording());
    let refused = owner
        .start(&loop_definition, None, None)
        .err()
        .expect("a replacement start is refused");
    assert!(refused.message.contains("already active"), "{refused:?}");
    match owner.stop(&loop_definition, None).expect("stopped") {
        RecordingTransition::Stopped(run) => assert_eq!(run, "complete"),
        _ => panic!("the original buffer stops"),
    }
    assert_eq!(*events.lock(), ["stop:original"]);
}

#[test]
fn reconstructs_the_persisted_test_recording_through_a_stop_only_recovery_callback() {
    let events: Events = Arc::default();
    let owner: ExperimentRecordingOwner<Session> = ExperimentRecordingOwner::new(|_, _, _| {
        Err(RecordingStartError::Failed(Failure::new(
            "replacement Start must not run",
            Cause::Implementation,
            NextAction::None,
        )))
    });
    let loop_definition = test_loop();
    let persisted = owner_of("persisted-test-recording");
    let recover_events = Arc::clone(&events);
    restore_experiment_recording_owner(
        &owner,
        &loop_definition,
        &persisted,
        |exact_loop, exact_owner| {
            recover_events
                .lock()
                .push(format!("recover:{}", exact_owner.recording_id));
            assert_eq!(
                exact_loop, &loop_definition,
                "the recovery is for this loop"
            );
            Ok(Session::new(exact_loop, "stop:original", &recover_events))
        },
    )
    .expect("restored");
    assert!(owner.recording());
    owner.stop(&loop_definition, None).expect("stopped");
    assert_eq!(
        *events.lock(),
        ["recover:persisted-test-recording", "stop:original"]
    );
}

#[test]
fn the_test_recording_window_is_the_planned_duration_with_a_margin_capped_at_the_limit() {
    // The TypeScript numbers of `control-studio-next-actions.test.ts`: at least 10 s of margin, 10 % of a longer hold,
    // and the recording limit.
    assert_eq!(experiment_recording_window_ms(60_000), 70_000);
    assert_eq!(experiment_recording_window_ms(3_600_000), 3_960_000);
    assert_eq!(experiment_recording_window_ms(86_400_000), 86_400_000);
}

#[test]
fn a_test_recording_starts_with_its_window_and_the_experiment_runs_through_the_journal() {
    let events: Events = Arc::default();
    let start_events = Arc::clone(&events);
    let owner = ExperimentRecordingOwner::new(move |requested, _persistence, _duration| {
        Ok(Session::new(requested, "stop", &start_events))
    });
    let loop_definition = test_loop();
    let started = start_experiment_with_recording(
        &owner,
        &loop_definition,
        || Ok("started"),
        Some(&Journal),
        None,
        Some(70_000),
    );
    assert_eq!(started.expect("started"), "started");
}

#[test]
fn a_test_recording_is_refused_without_its_journal_and_starts_once_a_journal_is_given() {
    let events: Events = Arc::default();
    let start_events = Arc::clone(&events);
    let owner = ExperimentRecordingOwner::new(move |requested, _persistence, _duration| {
        Ok(Session::new(requested, "stop", &start_events))
    });
    let loop_definition = test_loop();
    let sent = Cell::new(0_u32);
    let refused = start_experiment_with_recording(
        &owner,
        &loop_definition,
        || {
            sent.set(sent.get() + 1);
            Ok("started")
        },
        None,
        None,
        None,
    )
    .expect_err("the recording gate refuses to dispatch without a journal");
    assert!(refused.message.contains("journal"), "{refused:?}");
    assert_eq!(
        sent.get(),
        0,
        "no request reaches the experiment without a journal"
    );

    let started = start_experiment_with_recording(
        &owner,
        &loop_definition,
        || {
            sent.set(sent.get() + 1);
            Ok("started")
        },
        Some(&Journal),
        None,
        None,
    );
    assert_eq!(started.expect("started"), "started");
    assert_eq!(
        sent.get(),
        1,
        "the experiment is started once, through the journal"
    );
}

#[test]
fn an_unconfirmed_test_start_stops_its_original_recording_and_is_reported() {
    let events: Events = Arc::default();
    let start_events = Arc::clone(&events);
    let owner = ExperimentRecordingOwner::new(move |requested, _persistence, _duration| {
        Err(RecordingStartError::Unconfirmed(Box::new(
            RecordingStartUnconfirmed {
                session: Session::new(requested, "stop:lost-start", &start_events),
                recording: None,
                start_error: Failure::unreachable("first Start reply lost"),
                stop_error: None,
            },
        )))
    });
    let loop_definition = test_loop();
    let excitation_sent = Cell::new(false);
    let failure = start_experiment_with_recording(
        &owner,
        &loop_definition,
        || {
            excitation_sent.set(true);
            Ok("started")
        },
        Some(&Journal),
        None,
        None,
    )
    .expect_err("the Test start is not confirmed");
    assert!(
        failure
            .detail
            .as_deref()
            .unwrap_or_default()
            .contains("Test recording Start acknowledgement was not confirmed"),
        "{failure:?}"
    );
    assert!(!excitation_sent.get(), "the experiment is never dispatched");
    assert_eq!(
        *events.lock(),
        ["stop:lost-start"],
        "the original owner is stopped"
    );
    assert!(!owner.recording());
}

#[test]
fn the_recording_start_receives_the_journal_and_writes_its_owner_through_it() {
    let events: Events = Arc::default();
    let start_events = Arc::clone(&events);
    let owner = ExperimentRecordingOwner::new(move |requested, persistence, _duration| {
        let Some(journal) = persistence else {
            return Err(RecordingStartError::Failed(Failure::new(
                "the recording start received no journal",
                Cause::Implementation,
                NextAction::None,
            )));
        };
        if let Err(error) = journal.persist(&owner_of("test-recording")) {
            return Err(RecordingStartError::Failed(error));
        }
        start_events.lock().push("recording-start".to_string());
        Ok(Session::new(requested, "recording-stop", &start_events)
            .with_owner(owner_of("test-recording")))
    });
    let loop_definition = test_loop();
    let journal = WritingJournal {
        writes: Arc::default(),
    };
    let persist_events = Arc::clone(&events);
    let persist = move |persisted: &RecordingRecoveryOwner| -> Result<(), Failure> {
        persist_events
            .lock()
            .push(format!("persist:{}", persisted.recording_id));
        Ok(())
    };
    let excitation_events = Arc::clone(&events);
    let started = start_experiment_with_recording(
        &owner,
        &loop_definition,
        move || {
            excitation_events
                .lock()
                .push("experiment-start".to_string());
            Ok("started")
        },
        Some(&journal),
        Some(&persist),
        None,
    );
    assert_eq!(started.expect("started"), "started");
    assert_eq!(
        *journal.writes.lock(),
        ["test-recording"],
        "the owner's start wrote its owner through the journal it received"
    );
    assert_eq!(
        *events.lock(),
        [
            "recording-start",
            "persist:test-recording",
            "experiment-start"
        ]
    );
}

#[test]
fn a_test_recording_without_a_journal_refuses_with_the_exact_sentence_before_any_start() {
    let events: Events = Arc::default();
    let start_events = Arc::clone(&events);
    let owner = ExperimentRecordingOwner::new(move |requested, _persistence, _duration| {
        start_events.lock().push("recording-start".to_string());
        Ok(Session::new(requested, "stop", &start_events))
    });
    let loop_definition = test_loop();
    let excitation_events = Arc::clone(&events);
    let refused = start_experiment_with_recording(
        &owner,
        &loop_definition,
        move || {
            excitation_events
                .lock()
                .push("experiment-start".to_string());
            Ok("started")
        },
        None,
        None,
        None,
    )
    .expect_err("the Test recording refuses without its journal");
    assert_eq!(
        refused.detail.as_deref(),
        Some(JOURNAL_REQUIRED_MESSAGE),
        "{refused:?}"
    );
    assert!(
        events.lock().is_empty(),
        "no Start of either kind is dispatched"
    );
    assert!(!owner.recording());
}

#[test]
fn a_test_recording_is_kept_in_the_study_before_its_runtime_buffer_is_released() {
    let events: Events = Arc::default();
    let start_events = Arc::clone(&events);
    let owner = ExperimentRecordingOwner::new(move |requested, _persistence, _duration| {
        Ok(Session::new(requested, "stop", &start_events))
    });
    let loop_definition = test_loop();
    owner.start(&loop_definition, None, None).expect("started");
    let keep_events = Arc::clone(&events);
    let keeper = move |recording: &String| -> Result<(), Failure> {
        let label = if recording.as_str() == "complete" {
            "retain"
        } else {
            "retain-other"
        };
        keep_events.lock().push(label.to_string());
        Ok(())
    };
    let kept = stop_experiment_recording(&owner, &loop_definition, Some(&keeper))
        .expect("stopped and kept");
    assert_eq!(kept, "complete", "the study keeps the run that was stopped");
    assert_eq!(
        *events.lock(),
        ["stop", "retain", "release"],
        "stop, then keep in the study, then release"
    );
    assert!(!owner.recording());
}

#[test]
fn a_failed_keep_of_a_test_recording_leaves_its_buffer_for_the_next_stop() {
    let events: Events = Arc::default();
    let start_events = Arc::clone(&events);
    let owner = ExperimentRecordingOwner::new(move |requested, _persistence, _duration| {
        Ok(Session::new(requested, "stop", &start_events))
    });
    let loop_definition = test_loop();
    owner.start(&loop_definition, None, None).expect("started");
    let keep_events = Arc::clone(&events);
    let keeper = move |_recording: &String| -> Result<(), Failure> {
        keep_events.lock().push("keep".to_string());
        Err(Failure::new(
            "study is full",
            Cause::Settings,
            NextAction::None,
        ))
    };
    let refused = stop_experiment_recording(&owner, &loop_definition, Some(&keeper))
        .expect_err("the keep fails");
    assert!(refused.message.contains("study is full"), "{refused:?}");
    assert_eq!(
        *events.lock(),
        ["stop", "keep"],
        "nothing is released before the study keeps the samples"
    );
    assert!(
        owner.recording(),
        "the owner stays, so the next Stop retries the keep"
    );
}
