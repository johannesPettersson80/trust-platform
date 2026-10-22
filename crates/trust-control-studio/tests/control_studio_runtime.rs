//! Control Studio's runtime client against a real runtime (checklist V0.7; specification 35, A8.3
//! and A8.5; runtime protocol annex, E1.3, E3, E4 and E11.2).
//!
//! The PID commissioning example runs in `trust-runtime run --simulation`. Control Studio opens its
//! loop, reads the runtime facts, inspects the loop's signals, journals a recording before it
//! starts it, records twenty executions of the loop's task, stops the recording, keeps it in the
//! project's study, and releases it only after it is kept.
//!
//! The test runs the `trust-runtime` executable of the same target folder. `cargo test --all`
//! builds it; on its own, run `cargo build -p trust-runtime --bin trust-runtime` first, or name the
//! executable in `TRUST_RUNTIME_BIN`. The helpers that start and guard the runtime are in
//! `tests/common/mod.rs`, which `control_studio_identify.rs` uses too.
#![cfg(unix)]

mod common;

use std::fs;
use std::path::Path;
use std::thread::sleep;
use std::time::{Duration, Instant};

use trust_control::experiment::{
    ControlEvidenceSource, RecordingRecoveryBinding, RecordingRecoveryOwner,
};
use trust_control::loop_definition::LoopDefinition;
use trust_control::recording::evidence::recording_evidence;
use trust_control::recording::{
    Completeness, EvidenceSource, RecordingFile, RecordingRequest, RecordingState,
};
use trust_control::runtime::RuntimeConnection;
use trust_control::signals::loop_inspection_bindings;
use trust_control_studio::journal::{Journal, JournalEntry};
use trust_control_studio::runtime_client::RuntimeClient;
use trust_control_studio::{loops, storage};

use common::{
    copy_dir, inspect_until_available, prepare_runtime_toml, run_trust_runtime, start_runtime,
    wait_for_status, RemoveDir, DEADLINE, TOKEN,
};

const EXAMPLE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../examples/control-pid-commissioning"
);
/// The example's task (src/Main.st: `TASK MainTask(INTERVAL := T#100ms)`).
const TASK: &str = "MainTask";
/// The executions recorded: 2 s of process time at the 100 ms task.
const EXECUTIONS: u64 = 20;

#[test]
#[allow(clippy::too_many_lines)] // One journey against the runtime, step by step; split, it would hide the order (E11.2).
fn opens_inspects_records_keeps_and_releases_on_a_real_runtime() {
    let folder = std::env::temp_dir().join(format!("trust-cs-v0-{}", std::process::id()));
    let _ = fs::remove_dir_all(&folder);
    let _remove = RemoveDir(folder.clone());
    let project = folder.join("project");
    copy_dir(Path::new(EXAMPLE), &project);
    let socket = project.join(".control.sock");
    assert!(
        socket.as_os_str().len() <= 100,
        "the socket path {} is too long; use a shorter TMPDIR",
        socket.display()
    );
    prepare_runtime_toml(&project, &socket);
    run_trust_runtime(&["build", "--project", &project.display().to_string()]);

    let text = fs::read_to_string(project.join("control-loop.json")).expect("the example's loop");
    let declared = LoopDefinition::decode(&text).expect("the example's loop decodes");
    let loop_definition = loops::open(&project, &declared.runtime_id, &declared.id)
        .expect("Control Studio opens the loop");

    let log = project.join("runtime.log");
    let _runtime = start_runtime(&project, &log);
    let connection = RuntimeConnection {
        endpoint: format!("unix://{}", socket.display()),
        token: Some(TOKEN.into()),
        runtime_id: None,
    };
    let first = wait_for_status(&connection, &log);
    assert!(first.task(TASK).is_some(), "the runtime schedules {TASK}");

    let mut client = RuntimeClient::connect(&connection).expect("connected");
    let bindings = loop_inspection_bindings(&loop_definition);
    let sample = inspect_until_available(
        &mut client,
        &loop_definition,
        &bindings,
        &first.program_revision,
    );
    for binding in &bindings {
        assert!(
            sample.usable(&binding.id).is_some(),
            "{} is read with a usable value: {sample:?}",
            binding.id
        );
    }
    // The I/O snapshot is published with the first scan, so the target's kind is read after one (E1.3).
    let status = client.status().expect("status");
    assert_eq!(
        status.source(),
        EvidenceSource::SimulatedPlant,
        "a simulated runtime with simulated I/O is a simulated plant: {status:?}"
    );
    assert_eq!(
        status.program_revision, first.program_revision,
        "the same session"
    );

    let request = RecordingRequest {
        recording_id: "control-studio-v0".into(),
        loop_revision: loop_definition.revision.clone(),
        program_revision: Some(status.program_revision.clone()),
        bindings: bindings.clone(),
        maximum_samples: 1_000,
        maximum_duration_ms: 60_000,
        task: Some(TASK.into()),
        record_every: 1,
    };
    let storage_folder = folder.join("storage");
    let mut journal = Journal::open(
        &storage_folder,
        &loop_definition.project_id,
        &loop_definition.id,
    )
    .expect("journal");
    let source = match status.source() {
        EvidenceSource::SimulatedPlant => ControlEvidenceSource::Simulation,
        EvidenceSource::PhysicalPlant => ControlEvidenceSource::Physical,
    };
    let owner = RecordingRecoveryOwner {
        schema_version: 1,
        recording_id: request.recording_id.clone(),
        project_id: loop_definition.project_id.clone(),
        runtime_id: loop_definition.runtime_id.clone(),
        loop_id: loop_definition.id.clone(),
        loop_revision: loop_definition.revision.clone(),
        source,
        program_revision: request.program_revision.clone(),
        start_acknowledgement_unconfirmed: None,
        bindings: request
            .bindings
            .iter()
            .map(|binding| RecordingRecoveryBinding {
                id: binding.id.clone(),
                address: binding.address.clone(),
                symbol: binding.symbol.clone(),
                iec_type: binding.iec_type.clone(),
            })
            .collect(),
    };
    journal
        .record(JournalEntry::RecordingOwner { owner })
        .expect("the recording owner is journaled before it starts (E11.2)");
    start_recording(&mut client, &loop_definition, &request);

    let deadline = Instant::now() + DEADLINE;
    loop {
        let recording = client
            .recording_status(&request, None)
            .expect("recording status");
        assert_eq!(recording.state, RecordingState::Recording, "{recording:?}");
        if recording.sample_count >= EXECUTIONS {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "{EXECUTIONS} executions were not recorded within {} s: {recording:?}",
            DEADLINE.as_secs()
        );
        sleep(Duration::from_millis(200));
    }
    let run = client
        .stop_recording(&request)
        .expect("the recording stops with its run");
    assert!(run.samples.len() as u64 >= EXECUTIONS);
    assert_eq!(
        run.acquisition.completeness,
        Completeness::Complete,
        "{}",
        run.acquisition.reason
    );
    assert_eq!(
        client
            .stop_recording(&request)
            .expect("a stop is idempotent"),
        run
    );

    let mut study = storage::open_study(&project, &loop_definition.id, &loop_definition.revision)
        .expect("a new study");
    let evidence = recording_evidence(&run, None);
    let file = RecordingFile::new(&loop_definition.id, status.source(), run, evidence);
    let id = file.id().expect("identified");
    study.recordings.push(id.clone());
    storage::save_study(&project, &study, std::slice::from_ref(&file))
        .expect("the run is kept in the project");
    assert_eq!(
        storage::read_recording(&project, &loop_definition.id, &id).expect("read back"),
        file
    );

    client
        .release_recording(&request)
        .expect("released only after it is kept (E11.6)");
    journal
        .clear(&request.recording_id)
        .expect("the owner is cleared");
    let journal_after = Journal::open(
        &storage_folder,
        &loop_definition.project_id,
        &loop_definition.id,
    )
    .expect("journal");
    assert_eq!(
        journal_after.entries(),
        &[] as &[JournalEntry],
        "the journal is empty"
    );
}

/// Starts the recording. A symbolic start needs a current inspection of the same bindings; it is
/// attempted at most five times, each after a fresh inspection (E11.4).
fn start_recording(
    client: &mut RuntimeClient,
    loop_definition: &LoopDefinition,
    request: &RecordingRequest,
) {
    let program_revision = request
        .program_revision
        .as_deref()
        .expect("a symbolic recording names its program");
    let mut refusals = Vec::new();
    for _ in 0..5 {
        inspect_until_available(client, loop_definition, &request.bindings, program_revision);
        match client.start_recording(request) {
            Ok(()) => return,
            Err(failure)
                if failure
                    .detail
                    .as_deref()
                    .unwrap_or(&failure.message)
                    .contains("a current coherent inspection") =>
            {
                refusals.push(failure);
            }
            Err(failure) => panic!("the recording did not start: {failure:?}"),
        }
    }
    panic!("the recording did not start after five attempts: {refusals:?}");
}
