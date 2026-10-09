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
//! executable in `TRUST_RUNTIME_BIN`.
#![cfg(unix)]

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::thread::sleep;
use std::time::{Duration, Instant};

use trust_control::loop_definition::LoopDefinition;
use trust_control::recording::{
    Completeness, EvidenceSource, RecordingFile, RecordingRequest, RecordingState,
};
use trust_control::runtime::{RuntimeConnection, RuntimeStatus};
use trust_control::signals::{
    loop_inspection_bindings, Inspection, InspectionRequest, InspectionSample, SignalBinding,
};
use trust_lsp::control_studio::journal::{Journal, JournalEntry};
use trust_lsp::control_studio::runtime_client::RuntimeClient;
use trust_lsp::control_studio::{loops, storage};

const EXAMPLE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../examples/control-pid-commissioning"
);
/// The example's task (src/Main.st: `TASK MainTask(INTERVAL := T#100ms)`).
const TASK: &str = "MainTask";
/// A known control token for the test runtime. It is not a credential.
const TOKEN: &str = "control-studio-runtime-test-token";
/// The executions recorded: 2 s of process time at the 100 ms task.
const EXECUTIONS: u64 = 20;
const DEADLINE: Duration = Duration::from_secs(60);

/// A `trust-runtime run` child; dropping it kills and reaps the process.
struct Runtime(Child);

impl Drop for Runtime {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

/// Removes the test's folder when the test ends.
struct RemoveDir(PathBuf);

impl Drop for RemoveDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
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
    journal
        .record(JournalEntry::Recording {
            project_id: loop_definition.project_id.clone(),
            runtime_id: loop_definition.runtime_id.clone(),
            session_epoch: status.session_epoch,
            loop_id: loop_definition.id.clone(),
            source: status.source(),
            request: request.clone(),
        })
        .expect("the recording is journaled before it starts (E11.2)");
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
    let file = RecordingFile::new(&loop_definition.id, status.source(), run);
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
    assert!(Journal::open(
        &storage_folder,
        &loop_definition.project_id,
        &loop_definition.id
    )
    .expect("journal")
    .entries()
    .is_empty());
}

/// Inspects until the runtime has a complete scan (E3); the first answer may be pending.
fn inspect_until_available(
    client: &mut RuntimeClient,
    loop_definition: &LoopDefinition,
    bindings: &[SignalBinding],
    program_revision: &str,
) -> InspectionSample {
    let request = InspectionRequest::new(
        &loop_definition.revision,
        Some(program_revision),
        bindings.to_vec(),
    )
    .expect("a valid inspection");
    let deadline = Instant::now() + DEADLINE;
    loop {
        match client.inspect(&request).expect("inspection") {
            Inspection::Available(sample) => return sample,
            Inspection::Pending { reason, .. } => assert!(
                Instant::now() < deadline,
                "no scan within {} s: {reason}",
                DEADLINE.as_secs()
            ),
        }
        sleep(Duration::from_millis(100));
    }
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
                refusals.push(failure)
            }
            Err(failure) => panic!("the recording did not start: {failure:?}"),
        }
    }
    panic!("the recording did not start after five attempts: {refusals:?}");
}

fn wait_for_status(connection: &RuntimeConnection, log: &Path) -> RuntimeStatus {
    let deadline = Instant::now() + DEADLINE;
    loop {
        if let Ok(status) =
            RuntimeClient::connect(connection).and_then(|mut client| client.status())
        {
            return status;
        }
        assert!(
            Instant::now() < deadline,
            "the runtime did not answer status; its log:\n{}",
            fs::read_to_string(log).unwrap_or_default()
        );
        sleep(Duration::from_millis(200));
    }
}

/// The `trust-runtime` executable of this target folder, or `TRUST_RUNTIME_BIN`.
fn trust_runtime() -> PathBuf {
    if let Some(path) = std::env::var_os("TRUST_RUNTIME_BIN") {
        return PathBuf::from(path);
    }
    let test = std::env::current_exe().expect("the test executable");
    let profile = test
        .parent()
        .and_then(Path::parent)
        .expect("the target profile folder");
    let executable = profile.join("trust-runtime");
    assert!(
        executable.is_file(),
        "{} is missing: run `cargo build -p trust-runtime --bin trust-runtime` first, or set TRUST_RUNTIME_BIN",
        executable.display()
    );
    executable
}

fn run_trust_runtime(arguments: &[&str]) {
    let output = Command::new(trust_runtime())
        .args(arguments)
        .output()
        .expect("run trust-runtime");
    assert!(
        output.status.success(),
        "trust-runtime {arguments:?} failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn start_runtime(project: &Path, log: &Path) -> Runtime {
    let stdout = fs::File::create(log).expect("runtime log");
    let stderr = stdout.try_clone().expect("share the log");
    let child = Command::new(trust_runtime())
        .args([
            "run",
            "--project",
            &project.display().to_string(),
            "--simulation",
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::from(stdout))
        .stderr(Stdio::from(stderr))
        .spawn()
        .expect("spawn trust-runtime run");
    Runtime(child)
}

/// Points the control endpoint at a socket in the copy, sets the test token and turns the web
/// server off, so parallel runs never share an endpoint or a port.
fn prepare_runtime_toml(project: &Path, socket: &Path) {
    let path = project.join("runtime.toml");
    let text = fs::read_to_string(&path).expect("runtime.toml");
    let lines: Vec<String> = text
        .lines()
        .map(|line| match line.split('=').next().map(str::trim) {
            Some("endpoint") => format!("endpoint = \"unix://{}\"", socket.display()),
            Some("auth_token") => format!("auth_token = \"{TOKEN}\""),
            _ => line.to_string(),
        })
        .collect();
    fs::write(
        &path,
        format!("{}\n\n[runtime.web]\nenabled = false\n", lines.join("\n")),
    )
    .expect("write runtime.toml");
}

fn copy_dir(from: &Path, to: &Path) {
    fs::create_dir_all(to).expect("create the copy");
    for entry in fs::read_dir(from).expect("read the example") {
        let entry = entry.expect("an entry");
        let target = to.join(entry.file_name());
        if entry.file_type().expect("its type").is_dir() {
            copy_dir(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), &target).expect("copy a file");
        }
    }
}
