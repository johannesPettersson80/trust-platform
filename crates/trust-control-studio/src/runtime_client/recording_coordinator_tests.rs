//! Port of `control-studio-recording-coordinator.test.ts` (all eight tests). The sessions are fakes that log what is
//! called, in order, so each test checks the order of stop, keep and release as the `TypeScript` test does.

use std::sync::mpsc;
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use parking_lot::Mutex;
use trust_control::failure::{Cause, Failure, NextAction};
use trust_control::loop_definition::LoopDefinition;

use crate::runtime_client::fake_runtime::tank_loop;

use super::*;

/// A stopped recording, reduced to what the tests read: its label and its sample count.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Run {
    label: &'static str,
    samples: u64,
}

/// The calls made on the fake sessions and the keepers, in order.
type Events = Arc<Mutex<Vec<&'static str>>>;

fn events_of(events: &Events) -> Vec<&'static str> {
    events.lock().clone()
}

fn count(events: &Events, call: &str) -> usize {
    events.lock().iter().filter(|event| **event == call).count()
}

struct FakeSession {
    loop_definition: LoopDefinition,
    run: Run,
    events: Events,
}

impl FakeSession {
    fn new(loop_definition: &LoopDefinition, run: Run, events: &Events) -> Self {
        Self {
            loop_definition: loop_definition.clone(),
            run,
            events: Arc::clone(events),
        }
    }
}

impl RecordingSession for FakeSession {
    type Recording = Run;

    fn loop_definition(&self) -> &LoopDefinition {
        &self.loop_definition
    }

    fn stop(&mut self) -> Result<Run, Failure> {
        self.events.lock().push("stop");
        Ok(self.run.clone())
    }

    fn release(&mut self) -> Result<(), Failure> {
        self.events.lock().push("release");
        Ok(())
    }
}

fn incomplete_run() -> Run {
    Run {
        label: "stopped before data arrived",
        samples: 0,
    }
}

fn retained_run() -> Run {
    Run {
        label: "start acknowledgement unconfirmed; stopped data retained",
        samples: 3,
    }
}

fn loop_fixture() -> LoopDefinition {
    tank_loop()
}

/// A loop of another runtime with the same revision string (the scope check must still refuse it).
fn other_runtime_loop() -> LoopDefinition {
    let mut other = tank_loop();
    other.runtime_id = "another-device".to_string();
    other
}

/// Waits until `condition` holds, for at most five seconds.
fn wait_until(condition: impl Fn() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while !condition() {
        assert!(
            Instant::now() < deadline,
            "the condition did not hold in time"
        );
        thread::sleep(Duration::from_millis(5));
    }
}

#[test]
fn honors_stop_while_a_recording_start_acknowledgement_is_pending() {
    let events: Events = Arc::default();
    let (acknowledge, acknowledgements) = mpsc::channel::<FakeSession>();
    let acknowledgements = Mutex::new(acknowledgements);
    let coordinator = Arc::new(RecordingCoordinator::new(
        move |_loop, _persistence, _duration| {
            let session = acknowledgements.lock().recv().map_err(|_| {
                RecordingStartError::Failed(Failure::unreachable("no acknowledgement"))
            })?;
            Ok(session)
        },
    ));
    let starting = {
        let coordinator = Arc::clone(&coordinator);
        let loop_definition = loop_fixture();
        thread::spawn(move || coordinator.start(&loop_definition, None, None))
    };
    wait_until(|| {
        coordinator
            .current_update()
            .is_some_and(|notice| notice.state == RecordingNoticeState::Starting)
    });
    let stopping = {
        let coordinator = Arc::clone(&coordinator);
        let loop_definition = loop_fixture();
        thread::spawn(move || coordinator.stop(&loop_definition, None))
    };
    acknowledge
        .send(FakeSession::new(&loop_fixture(), retained_run(), &events))
        .map_err(|_| ())
        .expect("acknowledged");

    let started = starting.join().expect("the start thread").expect("started");
    assert!(matches!(started, Some(RecordingTransition::Started)));
    let stopped = stopping.join().expect("the stop thread").expect("stopped");
    assert!(
        matches!(stopped, Some(RecordingTransition::Stopped(_))),
        "the user's Stop must not be lost during the start handshake"
    );
    assert_eq!(count(&events, "stop"), 1);
    assert!(!coordinator.recording());
}

#[test]
fn reports_the_retained_owner_state_when_its_panel_is_reopened() {
    let events: Events = Arc::default();
    let (acknowledge, acknowledgements) = mpsc::channel::<FakeSession>();
    let acknowledgements = Mutex::new(acknowledgements);
    let coordinator = Arc::new(RecordingCoordinator::new(
        move |_loop, _persistence, _duration| {
            let session = acknowledgements.lock().recv().map_err(|_| {
                RecordingStartError::Failed(Failure::unreachable("no acknowledgement"))
            })?;
            Ok(session)
        },
    ));
    let starting = {
        let coordinator = Arc::clone(&coordinator);
        let loop_definition = loop_fixture();
        thread::spawn(move || coordinator.start(&loop_definition, None, None))
    };
    wait_until(|| coordinator.current_update().is_some());
    assert_eq!(
        coordinator.current_update(),
        Some(RecordingStateNotice {
            state: RecordingNoticeState::Starting,
            reason: "Starting recording…".to_string(),
        })
    );
    acknowledge
        .send(FakeSession::new(&loop_fixture(), incomplete_run(), &events))
        .map_err(|_| ())
        .expect("acknowledged");
    starting.join().expect("the start thread").expect("started");
    assert_eq!(
        coordinator.current_update(),
        Some(RecordingStateNotice {
            state: RecordingNoticeState::Recording,
            reason: "Recording at runtime — Start acknowledged; waiting for Stop to retain runtime scans."
                .to_string(),
        })
    );
}

#[test]
fn rejects_a_stop_for_another_runtime_even_if_revision_strings_match() {
    let events: Events = Arc::default();
    let start_events = Arc::clone(&events);
    let coordinator = RecordingCoordinator::new(move |loop_definition, _persistence, _duration| {
        Ok(FakeSession::new(
            loop_definition,
            incomplete_run(),
            &start_events,
        ))
    });
    coordinator
        .start(&loop_fixture(), None, None)
        .expect("started")
        .expect("a start");
    let Err(refused) = coordinator.stop(&other_runtime_loop(), None) else {
        panic!("another runtime");
    };
    assert!(
        refused.message.contains("matching") || refused.message.contains("scope"),
        "{refused:?}"
    );
    assert_eq!(count(&events, "stop"), 0);
    assert!(coordinator.recording());
}

#[test]
fn does_not_let_a_completed_unconfirmed_record_replace_a_later_recording_stop() {
    let events: Events = Arc::default();
    let attempts = Arc::new(Mutex::new(0_u32));
    let start_events = Arc::clone(&events);
    let start_attempts = Arc::clone(&attempts);
    let coordinator = RecordingCoordinator::new(move |loop_definition, _persistence, _duration| {
        let mut attempts = start_attempts.lock();
        *attempts += 1;
        if *attempts == 1 {
            return Err(RecordingStartError::Unconfirmed(Box::new(
                RecordingStartUnconfirmed {
                    session: FakeSession::new(loop_definition, retained_run(), &start_events),
                    recording: Some(retained_run()),
                    start_error: Failure::unreachable("first Start reply lost"),
                    stop_error: None,
                },
            )));
        }
        Ok(FakeSession::new(
            loop_definition,
            Run {
                label: "later recording",
                samples: 4,
            },
            &start_events,
        ))
    });

    let first = coordinator
        .start(&loop_fixture(), None, None)
        .expect("the start settles");
    let second = coordinator
        .start(&loop_fixture(), None, None)
        .expect("the start settles");
    let stopped = coordinator
        .stop(&loop_fixture(), None)
        .expect("stopped")
        .expect("a stop");

    assert!(matches!(
        first,
        Some(RecordingTransition::StartUnconfirmed(_))
    ));
    assert!(matches!(second, Some(RecordingTransition::Started)));
    match stopped {
        RecordingTransition::Stopped(run) => assert_eq!(
            run,
            Run {
                label: "later recording",
                samples: 4,
            },
            "The next Stop must belong to the newly acknowledged session"
        ),
        _ => panic!("the later recording stops"),
    }
    assert_eq!(count(&events, "stop"), 1);
}

#[test]
fn adopts_a_recovered_owner_as_unconfirmed_and_stops_only_that_owner() {
    let events: Events = Arc::default();
    let fresh_starts = Arc::new(Mutex::new(0_u32));
    let start_fresh = Arc::clone(&fresh_starts);
    let start_events = Arc::clone(&events);
    let coordinator = RecordingCoordinator::new(move |loop_definition, _persistence, _duration| {
        *start_fresh.lock() += 1;
        Ok(FakeSession::new(
            loop_definition,
            incomplete_run(),
            &start_events,
        ))
    });
    let reason = "Recording recovery unconfirmed — Stop the original runtime buffer to obtain terminal evidence.";
    coordinator
        .adopt_recovered_owner(
            FakeSession::new(&loop_fixture(), incomplete_run(), &events),
            reason,
        )
        .expect("adopted");
    assert_eq!(
        coordinator.current_update(),
        Some(RecordingStateNotice {
            state: RecordingNoticeState::Unconfirmed,
            reason: reason.to_string(),
        })
    );
    assert!(
        coordinator
            .start(&loop_fixture(), None, None)
            .expect("no error")
            .is_none(),
        "Recovery blocks a replacement Start."
    );
    assert_eq!(*fresh_starts.lock(), 0);
    assert!(matches!(
        coordinator.stop(&loop_fixture(), None).expect("stopped"),
        Some(RecordingTransition::Stopped(_))
    ));
    assert_eq!(count(&events, "stop"), 1, "only the recovered owner stops");
}

#[test]
fn reconstructs_the_same_unavailable_owner_when_its_runtime_becomes_available() {
    let events: Events = Arc::default();
    let coordinator: RecordingCoordinator<FakeSession> = RecordingCoordinator::new(|_, _, _| {
        Err(RecordingStartError::Failed(Failure::new(
            "Recovery must not create a replacement Start.",
            Cause::Implementation,
            NextAction::None,
        )))
    });
    coordinator
        .adopt_unavailable_recovery(
            &loop_fixture(),
            "Recording recovery unconfirmed — Start the matching runtime before retrying Stop.",
        )
        .expect("adopted as unavailable");
    coordinator
        .adopt_recovered_owner(
            FakeSession::new(&loop_fixture(), incomplete_run(), &events),
            "Recording recovery unconfirmed — Stop the original runtime buffer to obtain terminal evidence.",
        )
        .expect("the same owner is reconstructed");
    assert!(matches!(
        coordinator.stop(&loop_fixture(), None).expect("stopped"),
        Some(RecordingTransition::Stopped(_))
    ));
    assert_eq!(count(&events, "stop"), 1);
    assert!(!coordinator.recording());
}

#[test]
fn keeps_a_stopped_recording_before_its_runtime_buffer_is_released() {
    let events: Events = Arc::default();
    let start_events = Arc::clone(&events);
    let coordinator = RecordingCoordinator::new(move |loop_definition, _persistence, _duration| {
        Ok(FakeSession::new(
            loop_definition,
            incomplete_run(),
            &start_events,
        ))
    });
    coordinator
        .start(&loop_fixture(), None, None)
        .expect("started");
    let retain_events = Arc::clone(&events);
    let retain = move |_run: &Run| -> Result<(), Failure> {
        retain_events.lock().push("retain");
        Ok(())
    };
    let stopped = coordinator
        .stop(&loop_fixture(), Some(&retain))
        .expect("stopped");
    assert!(matches!(stopped, Some(RecordingTransition::Stopped(_))));
    assert_eq!(events_of(&events), ["stop", "retain", "release"]);
    assert!(!coordinator.recording());
}

#[test]
fn a_failed_keep_leaves_the_owner_and_its_buffer_in_place_and_the_next_stop_keeps_then_releases() {
    let events: Events = Arc::default();
    let start_events = Arc::clone(&events);
    let coordinator = RecordingCoordinator::new(move |loop_definition, _persistence, _duration| {
        Ok(FakeSession::new(
            loop_definition,
            incomplete_run(),
            &start_events,
        ))
    });
    coordinator
        .start(&loop_fixture(), None, None)
        .expect("started");
    let failing = |_run: &Run| -> Result<(), Failure> {
        Err(Failure::new(
            "study store unavailable",
            Cause::Implementation,
            NextAction::None,
        ))
    };
    let Err(refused) = coordinator.stop(&loop_fixture(), Some(&failing)) else {
        panic!("the keep fails");
    };
    assert!(
        refused.message.contains("study store unavailable"),
        "{refused:?}"
    );
    assert!(coordinator.recording(), "the owner is kept for a retry");
    assert_eq!(
        events_of(&events),
        ["stop"],
        "nothing is released before the samples are kept"
    );

    let retain_events = Arc::clone(&events);
    let retain = move |_run: &Run| -> Result<(), Failure> {
        retain_events.lock().push("retain");
        Ok(())
    };
    let retried = coordinator
        .stop(&loop_fixture(), Some(&retain))
        .expect("retried")
        .expect("a stop");
    assert!(matches!(retried, RecordingTransition::Stopped(_)));
    assert_eq!(events_of(&events), ["stop", "stop", "retain", "release"]);
    assert!(!coordinator.recording());
}
