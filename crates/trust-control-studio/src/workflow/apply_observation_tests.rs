//! Tests of the Apply observation: the follow of a recording in process time on scripted pages (`followAppliedResponse` of
//! `applyObservation.ts`), the scans the rules read, and the reattachment of a reopened engine on the scripted plant runtime. The
//! assertions are those of `control-studio-apply-process-time.test.ts` ("apply observation owned by the runtime (B6.3, A8.5)" and
//! "retained recordings and closing the panel") that test the engine's follow; the range, response and coverage decisions are tested in
//! trust-control (`observation_assessment_tests`).

#![allow(clippy::float_cmp)] // The scripted scans hold exact values, compared exactly on purpose.

use std::cell::Cell;
use std::collections::BTreeMap;

use serde_json::json;
use trust_control::commissioning::apply_texts;
use trust_control::commissioning::checkpoint::{CheckpointStage, CommissioningCheckpoint};
use trust_control::commissioning::commissioning_result::{CommissioningStatus, TransferOwner};
use trust_control::commissioning::found_mode::FoundMode;
use trust_control::commissioning::observation::AuxiliaryValue;
use trust_control::commissioning::parameter_application::ParameterOwner;
use trust_control::commissioning::parameter_status::{ParameterStatus, RecordState};
use trust_control::commissioning::transfer::pid_transfer_binding;
use trust_control::commissioning::transfer_tracking::{parallel_pid_tracking, TransferTracking};
use trust_control::experiment::{
    ControlEvidenceSource, RecordingRecoveryBinding, RecordingRecoveryOwner,
};
use trust_control::failure::{Cause, Failure, NextAction};
use trust_control::loop_definition::LoopDefinition;
use trust_control::recording::channels::ChannelQuality;
use trust_control::recording::{
    Acquisition, Completeness, RecordedRun, RecordedSample, RecordingState, RecordingStatus,
    StopReason,
};
use trust_control::signals::{loop_inspection_bindings, Quality, SignalBinding};
use trust_control::study::Study;

use super::*;
use crate::journal::{Journal, JournalEntry};
use crate::workflow::apply::test_support::{
    instant, model, pid_loop, room_loop, scratch, task, Plant, Report, Response,
    PARAMETER_REVISION, TARGET, TRANSFER_REVISION,
};

/// A recording that answers scripted pages, repeating the last one, and a scripted stop (`RecordingFollower` of the `TypeScript`
/// tests). It counts the pages read and the stops.
struct Scripted {
    pages: Vec<RecordingStatus>,
    stopped: RecordedRun,
    bindings: Vec<SignalBinding>,
    follows: usize,
    stops: usize,
}

impl Scripted {
    fn new(
        loop_definition: &LoopDefinition,
        pages: Vec<RecordingStatus>,
        stopped: Vec<RecordedSample>,
    ) -> Self {
        let bindings = loop_inspection_bindings(loop_definition);
        Self {
            stopped: run_of(&bindings, stopped),
            pages,
            bindings,
            follows: 0,
            stops: 0,
        }
    }
}

impl AppliedRecording for Scripted {
    fn page(&mut self) -> Result<RecordingStatus, Failure> {
        let page = self.pages[self.follows.min(self.pages.len() - 1)].clone();
        self.follows += 1;
        Ok(page)
    }

    fn stop(&mut self) -> Result<RecordedRun, Failure> {
        self.stops += 1;
        Ok(self.stopped.clone())
    }

    fn bindings(&self) -> Vec<SignalBinding> {
        self.bindings.clone()
    }
}

/// The values of one scan of the room loop, in the order of its bindings: the setpoint, the measurement, the output, the secondary
/// pressure when the loop records it, and the mode `Main.Manual`, 0 in Auto.
fn values(setpoint: f64, process: f64, secondary: Option<f64>) -> Vec<Option<f64>> {
    let mut values = vec![Some(setpoint), Some(process), Some(0.0)];
    if let Some(pressure) = secondary {
        values.push(Some(pressure));
    }
    values.push(Some(0.0));
    values
}

/// One scan at `milliseconds` of process time, every value good and none forced (E4.4).
fn scan(index: u64, milliseconds: i64, values: Vec<Option<f64>>) -> RecordedSample {
    let count = values.len();
    RecordedSample {
        scan: index,
        runtime_time_ns: Some(milliseconds * 1_000_000),
        captured_at_ms: 1000,
        values,
        forced: vec![false; count],
        quality: vec![Quality::Good; count],
        gap: false,
    }
}

/// One status page (E4.6): its state, the process time it covers, its samples from `from`, and the samples the recording holds.
fn page(
    state: RecordingState,
    elapsed_ms: u64,
    from: u64,
    samples: Vec<RecordedSample>,
    sample_count: u64,
) -> RecordingStatus {
    let read = u64::try_from(samples.len()).unwrap_or(0);
    RecordingStatus {
        recording_id: "recording-1".into(),
        state,
        elapsed_ms,
        sample_count,
        complete: true,
        reason: None,
        samples,
        next_sample: from + read,
    }
}

/// A stopped run of `bindings` holding `samples`, complete (E4.7).
fn run_of(bindings: &[SignalBinding], samples: Vec<RecordedSample>) -> RecordedRun {
    RecordedRun {
        recording_id: "recording-1".into(),
        loop_revision: "sha256:pid-apply-1".into(),
        program_revision: None,
        bindings: bindings.to_vec(),
        stop_reason: StopReason::Stopped,
        acquisition: Acquisition {
            method: "runtime_scan_buffer".into(),
            completeness: Completeness::Complete,
            observed_sample_count: u64::try_from(samples.len()).unwrap_or(0),
            scan_range: None,
            missing_scan_ranges: Vec::new(),
            dropped_observed_samples: 0,
            actual_cycle_timing_available: true,
            freshness_available: false,
            reason: "Captured every scan.".into(),
        },
        samples,
    }
}

/// The follow of `recording` with the reviewed duration of 4 s and the given target, on a wall clock standing still.
fn follow(
    recording: &mut Scripted,
    loop_definition: &LoopDefinition,
    target: Option<f64>,
) -> Result<Followed, Failure> {
    let clock = || 0_u64;
    follow_applied_response(
        recording,
        &FollowPlan {
            loop_definition,
            target,
            duration_ms: 4000.0,
            record_period_ms: 0.0,
        },
        &Report::default(),
        &instant(&clock),
    )
}

/// TS control-studio-apply-process-time, "the observation follows the recording to its end in process time, then returns it whole"
/// (B6.3, A3.2): a page while recording and a page once stopped are followed, and the stopped run is returned as the runtime gave it.
#[test]
fn the_observation_follows_the_recording_to_its_end_and_returns_it_whole() {
    let room = room_loop(false);
    let mut recording = Scripted::new(
        &room,
        vec![
            page(
                RecordingState::Recording,
                1000,
                0,
                vec![scan(1, 0, values(5.0, 5.0, None))],
                1,
            ),
            page(RecordingState::Stopped, 4000, 1, Vec::new(), 1),
        ],
        vec![scan(1, 0, values(5.0, 6.0, None))],
    );
    let followed = follow(&mut recording, &room, None).expect("followed");
    assert_eq!(followed, Followed::Stopped(recording.stopped.clone()));
    assert_eq!(
        recording.follows, 2,
        "the recording is followed page by page until it stops"
    );
}

/// TS control-studio-apply-process-time, "the response is recorded for the reviewed duration from the adopted target, so the target
/// change never shortens it (B6.3)": the target 7 is adopted 2 s into the recording; the reviewed duration is 4 s, so following ends with
/// the scan 4 s after the adopted target, at 6 s of recording, and the recording is stopped once.
#[test]
fn the_response_is_recorded_for_the_reviewed_duration_from_the_adopted_target() {
    let room = room_loop(false);
    let pages = (0_u64..=8)
        .map(|seconds| {
            let setpoint = if seconds >= 2 { 7.0 } else { 5.0 };
            let milliseconds = i64::try_from(seconds * 1000).unwrap_or(0);
            page(
                RecordingState::Recording,
                seconds * 1000,
                seconds,
                vec![scan(seconds + 1, milliseconds, values(setpoint, 5.0, None))],
                seconds + 1,
            )
        })
        .collect();
    let mut recording = Scripted::new(&room, pages, Vec::new());
    follow(&mut recording, &room, Some(7.0)).expect("followed");
    assert_eq!(
        recording.follows, 7,
        "following ends with the scan 4 s after the adopted target"
    );
    assert_eq!(recording.stops, 1);
}

/// TS control-studio-apply-process-time, "a measured output that leaves its permitted range ends the observation at once, naming the
/// signal (B6.3, B6.7)": no further page is read after the departure, and the recording is not stopped by the follow.
#[test]
fn a_measured_output_leaving_its_permitted_range_ends_the_observation_at_once() {
    let room = room_loop(false);
    let mut recording = Scripted::new(
        &room,
        vec![page(
            RecordingState::Recording,
            100,
            0,
            vec![scan(1, 0, values(5.0, 12.5, None))],
            2,
        )],
        Vec::new(),
    );
    let Followed::Departed { failure, stopped } =
        follow(&mut recording, &room, None).expect("followed")
    else {
        panic!("the departure ends the observation");
    };
    assert_eq!(
        failure.message,
        "P1.RoomC left its permitted range (12.5 °C outside 0 °C..10 °C)"
    );
    assert_eq!(failure.next_action, NextAction::None);
    assert!(stopped.is_none());
    assert_eq!((recording.follows, recording.stops), (1, 0));
}

/// TS control-studio-apply-process-time, "a secondary measured output that leaves its own range ends the observation at once, naming that
/// output (B6.3, B6.7)": the process variable is inside its range, so the departure is the secondary output's alone.
#[test]
fn a_secondary_measured_output_leaving_its_own_range_ends_the_observation() {
    let room = room_loop(true);
    let mut recording = Scripted::new(
        &room,
        vec![page(
            RecordingState::Recording,
            100,
            0,
            vec![scan(1, 0, values(5.0, 5.0, Some(3.5)))],
            2,
        )],
        Vec::new(),
    );
    let Followed::Departed { failure, .. } = follow(&mut recording, &room, None).expect("followed")
    else {
        panic!("the departure ends the observation");
    };
    assert_eq!(
        failure.message,
        "P2.Pressure left its permitted range (3.5 bar outside 1 bar..3 bar)"
    );
    assert_eq!((recording.follows, recording.stops), (1, 0));
}

/// TS control-studio-apply-process-time, "a secondary measured output that stays inside its own range does not end the observation
/// (B6.3)": the recording is followed to its end, as a response inside every range is.
#[test]
fn a_secondary_measured_output_inside_its_range_does_not_end_the_observation() {
    let room = room_loop(true);
    let inside = values(5.0, 5.0, Some(2.0));
    let mut recording = Scripted::new(
        &room,
        vec![
            page(
                RecordingState::Recording,
                1000,
                0,
                vec![scan(1, 0, inside.clone())],
                1,
            ),
            page(RecordingState::Stopped, 4000, 1, Vec::new(), 1),
        ],
        vec![scan(1, 0, inside)],
    );
    let followed = follow(&mut recording, &room, None).expect("followed");
    assert!(matches!(followed, Followed::Stopped(_)));
    assert_eq!(recording.follows, 2);
}

/// TS control-studio-apply-process-time, "a secondary measured output that leaves its range after the last page is named when the
/// recording stops (B6.3)": the stopped recording is checked before it is returned, and it is kept with the departure.
#[test]
fn a_departure_in_the_stopped_recording_is_named_with_the_recording() {
    let room = room_loop(true);
    let mut recording = Scripted::new(
        &room,
        vec![page(RecordingState::Stopped, 4000, 0, Vec::new(), 0)],
        vec![scan(1, 0, values(5.0, 5.0, Some(0.5)))],
    );
    let Followed::Departed { failure, stopped } =
        follow(&mut recording, &room, None).expect("followed")
    else {
        panic!("the stopped recording is checked");
    };
    assert_eq!(
        failure.message,
        "P2.Pressure left its permitted range (0.5 bar outside 1 bar..3 bar)"
    );
    assert_eq!(stopped, Some(recording.stopped.clone()));
    assert_eq!(recording.stops, 1);
}

/// TS control-studio-apply-process-time, "the observation stops with the connection-loss message when no newer value arrives for 5 s"
/// (A3.4, A7): the recording's process time stands still while this receiver's clock advances 2 s per look.
#[test]
fn the_observation_stops_when_the_runtime_stops_sending_values() {
    let room = room_loop(false);
    let mut recording = Scripted::new(
        &room,
        vec![page(RecordingState::Recording, 0, 0, Vec::new(), 0)],
        Vec::new(),
    );
    let wall = Cell::new(0_u64);
    let clock = || {
        wall.set(wall.get() + 2000);
        wall.get()
    };
    let refused = follow_applied_response(
        &mut recording,
        &FollowPlan {
            loop_definition: &room,
            target: None,
            duration_ms: 4000.0,
            record_period_ms: 0.0,
        },
        &Report::default(),
        &instant(&clock),
    )
    .expect_err("the runtime stopped sending values");
    assert_eq!(refused, Failure::stopped_sending());
    assert_eq!(refused.next_action, NextAction::ConnectRuntime);
}

/// TS control-studio-apply-process-time, "a recording the runtime ended reports the runtime's reason and asks for the runtime to be
/// connected again (A6.7)".
#[test]
fn a_recording_the_runtime_ended_names_the_reason_and_asks_for_the_runtime() {
    let room = room_loop(false);
    let mut ended = page(RecordingState::Failed, 300, 0, Vec::new(), 3);
    ended.reason = Some("scan_reset".into());
    let mut recording = Scripted::new(&room, vec![ended], Vec::new());
    let refused = follow(&mut recording, &room, None).expect_err("the runtime ended it");
    assert_eq!(refused, apply_texts::recording_ended(Some("scan_reset")));
    assert!(refused
        .message
        .starts_with("The runtime ended the recording: scan_reset."));
    assert_eq!(
        (refused.cause, refused.next_action),
        (Cause::Runtime, NextAction::ConnectRuntime)
    );
}

/// The scans the rules read (`toWorkflowSamples` of `runtimeRecording.ts`; A3.2, B6.3): each scan's process time from the recording's
/// first sample across pages, the mode channel's Auto, the setpoint, the measurement, the applied output, the quality, and the secondary
/// pressure as an additional channel.
#[test]
fn a_page_becomes_the_scans_the_rules_read() {
    let room = room_loop(true);
    let bindings = loop_inspection_bindings(&room);
    let mut origin = None;
    let first = page_samples(
        &room,
        &bindings,
        &[scan(7, 1000, values(7.0, 5.0, Some(2.0)))],
        &mut origin,
    )
    .expect("the first page");
    let second = page_samples(
        &room,
        &bindings,
        &[scan(8, 1020, values(7.0, 5.5, Some(2.1)))],
        &mut origin,
    )
    .expect("the second page");
    assert_eq!(origin, Some(1_000_000_000));
    assert_eq!(first[0].elapsed_seconds, 0.0);
    assert!((second[0].elapsed_seconds - 0.02).abs() < 1e-12);
    let scan = &second[0];
    assert!(
        scan.controller_in_auto,
        "Main.Manual reads 0, which is Auto (modeWhenTrue Manual)"
    );
    assert_eq!(
        (
            scan.scan,
            scan.setpoint,
            scan.process_value,
            scan.applied_output
        ),
        (8, Some(7.0), Some(5.5), Some(0.0))
    );
    assert_eq!(scan.quality, ChannelQuality::Good);
    assert_eq!(
        scan.additional,
        vec![AuxiliaryValue {
            id: "pv-secondary".into(),
            unit: "bar".into(),
            value: Some(2.1),
            quality: ChannelQuality::Good,
        }]
    );
}

/// TS control-studio-apply-process-time, "reopening resumes a journalled observation by following the runtime's recording and assessing
/// it (A8.5)": a journal without an observation has nothing to resume.
#[test]
fn a_reattachment_without_a_journaled_observation_is_refused() {
    let loop_definition = pid_loop();
    let plant = Plant::start(&loop_definition, true, Response::FollowsTarget);
    let folder = scratch("reattach-none");
    let study = Study::new(&loop_definition.id, &loop_definition.revision);
    let connection = plant.connection();
    let mut journal = Journal::open(&folder, &loop_definition.project_id, &loop_definition.id)
        .expect("the journal opens");
    let mut context = ApplyContext {
        loop_definition: &loop_definition,
        study: &study,
        connection: &connection,
        journal: &mut journal,
        physical_approved: false,
        project_root: &folder,
    };
    let clock = || 0_u64;
    let refused = reattach_reporting(&mut context, &Report::default(), &instant(&clock))
        .expect_err("nothing to resume");
    assert_eq!(refused, apply_texts::no_observation_to_resume());
    let _ = std::fs::remove_dir_all(&folder);
}

/// The checkpoint an Apply leaves while its observation runs (A8.5): the loop applied in Auto at the reviewed target, the record `Kp 2,
/// Ki 0.5, Kd 0` written over `Kp 1`, the target change's transfer owned, and the recording's owner journaled.
fn observing_checkpoint(
    loop_definition: &LoopDefinition,
    owner: RecordingRecoveryOwner,
) -> CommissioningCheckpoint {
    let binding = pid_transfer_binding(loop_definition).expect("the PID binding");
    let tracking = parallel_pid_tracking(loop_definition, &binding, false).expect("the tracking");
    let gains = |kp: f64, ki: f64| {
        BTreeMap::from([
            ("Main.Kd".to_owned(), 0.0),
            ("Main.Ki".to_owned(), ki),
            ("Main.Kp".to_owned(), kp),
        ])
    };
    CommissioningCheckpoint {
        id: "commissioning-observing".into(),
        loop_revision: loop_definition.revision.clone(),
        stage: CheckpointStage::Transfer,
        model: model(),
        task: task(),
        origin: ParameterStatus {
            supports_request_id: Some(true),
            transaction_id: None,
            program_revision: "program-1".into(),
            binding_revision: PARAMETER_REVISION.into(),
            active_revision: "original".into(),
            values: gains(1.0, 0.0),
            status: RecordState::Applied,
            previous_revision: None,
            previous_values: None,
            reason: None,
        },
        candidate: gains(2.0, 0.5),
        parameter_operation: ParameterOwner {
            id: "parameters-1".into(),
            program: "program-1".into(),
            binding: PARAMETER_REVISION.into(),
        },
        references: BTreeMap::from([("Main.SP".to_owned(), 2.0)]),
        binding,
        recovery_tracking: TransferTracking::ParallelPid(tracking),
        tolerance: 4e-5,
        replacement_program_revision: None,
        transfer: Some(TransferOwner {
            id: "transaction-target".into(),
            program: "program-1".into(),
            binding: TRANSFER_REVISION.into(),
        }),
        recording: Some(owner),
        recording_result: None,
        found_mode: Some(FoundMode::Auto),
        process_noise: Some(0.0),
        validation_identity: "validation:1".into(),
        transfers: Vec::new(),
    }
}

/// The plant a reopened engine finds (A8.6): in Auto at the reviewed target 3 m with the record `Kp 2, Ki 0.5` in force, the target change
/// as its last transfer transaction, and the observation's recording still held for 6 s, with no sample yet.
fn reopened_plant(loop_definition: &LoopDefinition, bindings: &[SignalBinding]) -> Plant {
    let plant = Plant::start(loop_definition, true, Response::FollowsTarget);
    {
        let mut world = plant.world();
        for (symbol, value) in [
            ("Main.SP", TARGET),
            ("Main.PV", TARGET),
            ("Main.Kp", 2.0),
            ("Main.Ki", 0.5),
        ] {
            world.numbers.insert(symbol.into(), value);
        }
        world.hold_recording(
            "recording-held",
            &loop_definition.revision,
            Some("program-1"),
            bindings
                .iter()
                .map(|binding| serde_json::to_value(binding).expect("a binding"))
                .collect(),
            6000,
        );
        world.transaction = Some(json!({
            "id": "transaction-target", "program_revision": "program-1", "binding_revision": TRANSFER_REVISION,
            "status": "applied", "reason": null, "observed_values": null, "confirmation": "output-image"
        }));
    }
    plant
}

/// The journaled owner of the held recording (E11.6), as the Apply's recording session journaled it before its start.
fn held_owner(
    loop_definition: &LoopDefinition,
    bindings: &[SignalBinding],
) -> RecordingRecoveryOwner {
    RecordingRecoveryOwner {
        schema_version: 1,
        recording_id: "recording-held".into(),
        project_id: loop_definition.project_id.clone(),
        runtime_id: loop_definition.runtime_id.clone(),
        loop_id: loop_definition.id.clone(),
        loop_revision: loop_definition.revision.clone(),
        source: ControlEvidenceSource::Simulation,
        program_revision: Some("program-1".into()),
        start_acknowledgement_unconfirmed: None,
        bindings: bindings
            .iter()
            .map(|binding| RecordingRecoveryBinding {
                id: binding.id.clone(),
                address: binding.address.clone(),
                symbol: binding.symbol.clone(),
                iec_type: binding.iec_type.clone(),
            })
            .collect(),
    }
}

/// A8.6, A10.4, B6.3, E11.5 (TS control-studio-apply-process-time, "reopening resumes a journalled observation by following the
/// runtime's recording and assessing it (A8.5)" and "reopening stores the resumed recording before releasing the runtime's buffer, and
/// replays no write (A8.5)"): a reopened engine reattaches to the recording by its journaled owner, never starts it again, follows it to
/// its own end and assesses the response that holds the target 3 m: it passes after reopening with a recorded error of 0 m. The samples
/// are kept in the checkpoint before the buffer is released, the checkpoint completes, the target change is shown as the runtime still
/// reports it under its journaled identity, and Restore and Keep in project are offered. Nothing is written.
#[test]
fn a_reopened_engine_reattaches_to_the_observation_and_assesses_it() {
    let loop_definition = pid_loop();
    let bindings = loop_inspection_bindings(&loop_definition);
    let plant = reopened_plant(&loop_definition, &bindings);
    let owner = held_owner(&loop_definition, &bindings);
    let folder = scratch("reattach");
    let mut journal = Journal::open(&folder, &loop_definition.project_id, &loop_definition.id)
        .expect("the journal opens");
    journal
        .persist(JournalEntry::Commissioning {
            checkpoint: Box::new(observing_checkpoint(&loop_definition, owner)),
        })
        .expect("journaled");
    let study = Study::new(&loop_definition.id, &loop_definition.revision);
    let connection = plant.connection();
    let mut context = ApplyContext {
        loop_definition: &loop_definition,
        study: &study,
        connection: &connection,
        journal: &mut journal,
        physical_approved: false,
        project_root: &folder,
    };
    let clock = || 0_u64;
    let outcome =
        reattach_reporting(&mut context, &Report::default(), &instant(&clock)).expect("reattached");
    let result = &outcome.result;
    assert_eq!(
        result.status,
        CommissioningStatus::Passed,
        "{}",
        result.reason
    );
    assert_eq!(
        result.reason,
        "Auto commissioning passed after reopening: recorded target error 0 m."
    );
    assert_eq!(result.found_mode, Some(FoundMode::Auto));
    assert_eq!(
        result
            .transfers
            .iter()
            .filter_map(|transfer| transfer
                .transaction
                .as_ref()
                .map(|transaction| transaction.id.clone()))
            .collect::<Vec<_>>(),
        ["transaction-target"]
    );
    assert_eq!(
        (
            outcome.restored,
            outcome.restore_available,
            outcome.keep_available
        ),
        (false, true, true)
    );
    assert_eq!(
        (
            outcome.checkpoint_id.as_str(),
            outcome.validation_identity.as_str()
        ),
        ("commissioning-observing", "validation:1"),
        "the reattached outcome names the journaled checkpoint and its validation (A8.1)"
    );
    {
        let world = plant.world();
        assert!(
            world.recording_released(),
            "the buffer is released once the samples are kept"
        );
        assert_eq!(
            world.log,
            ["recording stop", "recording release"],
            "no start and no write is replayed"
        );
    }
    let checkpoint = journal
        .entries()
        .iter()
        .find_map(|entry| match entry {
            JournalEntry::Commissioning { checkpoint } => Some(checkpoint.as_ref().clone()),
            _ => None,
        })
        .expect("the checkpoint stays journaled");
    assert_eq!(checkpoint.stage, CheckpointStage::Completed);
    assert!(checkpoint.recording.is_none());
    assert_eq!(checkpoint.recording_result, result.recording);
    assert_eq!(
        journal.entries().len(),
        1,
        "the checkpoint is rewritten in place: one per loop (A8.5)"
    );
    let _ = std::fs::remove_dir_all(&folder);
}
