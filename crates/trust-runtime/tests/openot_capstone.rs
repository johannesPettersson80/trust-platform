#![cfg(unix)]

use std::env;
use std::error::Error;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Output, Stdio};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use open_ot_carriage::concurrent::ConcurrentRawConsumer;
use open_ot_carriage::consumer::LossAccountingConsumer;
use open_ot_carriage::registry::{
    EVENT_LOGGER_STOPPED, EVENT_MESSAGE, EVENT_SOURCE_HIGH_WATER, KEY_SOURCE_HIGH_WATER,
    SYSTEM_SOURCE_ID, TY_ULINT,
};
use open_ot_carriage::wire::{Record, Slot};
use open_ot_conformance::{
    BatchObserver, ExpectedRecord, ExpectedSource, ObservationMetadata, ObservedReport,
    ReportInputs, SidecarExpectedAbsOracle,
};
use open_ot_shm::{FenceMode, SharedConcurrentStore};
use trust_runtime::config::RuntimeConfig;
use trust_runtime::harness::{CompileSession, SourceFile};
use trust_runtime::Runtime;

const RUN_ID: u64 = 1;
const SOURCE_IDS: [u32; 2] = [10, 30];
const DEFAULT_PER_SOURCE: u64 = 12;
const DEFAULT_CAPACITY: usize = 256;
const DEFAULT_TIMEOUT_SECS: u64 = 60;
const PER_SOURCE_ENV: &str = "OPENOT_CAPSTONE_PER_SOURCE";
const CAPACITY_ENV: &str = "OPENOT_CAPSTONE_CAPACITY";
const TIMEOUT_SECS_ENV: &str = "OPENOT_CAPSTONE_TIMEOUT_SECS";
const PRODUCER_READY_ENV: &str = "OPENOT_CAPSTONE_PRODUCER_READY";
const CONSUMER_READY_ENV: &str = "OPENOT_CAPSTONE_CONSUMER_READY";
const START_ENV: &str = "OPENOT_CAPSTONE_START";
const DONE_ENV: &str = "OPENOT_CAPSTONE_DONE";
const OBSERVED_ENV: &str = "OPENOT_CAPSTONE_OBSERVED";
const SHM_ENV: &str = "OPENOT_CAPSTONE_SHM";
const WORKDIR_ENV: &str = "OPENOT_CAPSTONE_WORKDIR";

#[test]
fn openot_capstone_fenced_cross_process() {
    let paths = CapstonePaths::new();
    paths.cleanup();

    let producer = spawn_role(
        "producer",
        "openot_capstone_producer_process",
        Some(0),
        &paths,
    )
    .expect("spawn OpenOT capstone producer");
    wait_for_file(&paths.producer_ready, Duration::from_secs(30))
        .expect("producer did not become ready");

    let consumer = spawn_role(
        "consumer",
        "openot_capstone_consumer_process",
        Some(1),
        &paths,
    )
    .expect("spawn OpenOT capstone consumer");
    wait_for_file(&paths.consumer_ready, Duration::from_secs(30))
        .expect("consumer did not become ready");

    write_status_atomic(&paths.start, "start\n").expect("release capstone roles");

    let producer_output = wait_for_role("producer", producer).expect("wait for producer role");
    let consumer_output = wait_for_role("consumer", consumer).expect("wait for consumer role");
    assert_role_success("producer", &producer_output);
    assert_role_success("consumer", &consumer_output);

    let report = ObservedReport::read(&paths.observed).expect("read capstone observed report");
    report
        .assert_fenced(None, true)
        .expect("fenced capstone report must reconcile");
    assert_expected_source_rows(&report);

    println!("capstone fenced: {}", report.summary_line());
    for source in &report.sources {
        println!(
            "capstone source run={} source={} expected_total={} delivered={} lost={} reconciled={}",
            source.run_id,
            source.source_id,
            source.expected_total,
            source.delivered,
            source.lost,
            source.delivered + source.lost
        );
    }

    paths.cleanup();
}

#[test]
#[ignore = "spawned by openot_capstone_fenced_cross_process"]
fn openot_capstone_producer_process() {
    if env::var("OPENOT_CAPSTONE_ROLE").as_deref() != Ok("producer") {
        return;
    }
    run_producer_role().expect("OpenOT capstone producer role");
}

#[test]
#[ignore = "spawned by openot_capstone_fenced_cross_process"]
fn openot_capstone_consumer_process() {
    if env::var("OPENOT_CAPSTONE_ROLE").as_deref() != Ok("consumer") {
        return;
    }
    run_consumer_role().expect("OpenOT capstone consumer role");
}

fn run_producer_role() -> Result<(), Box<dyn Error>> {
    let paths = CapstonePaths::from_env()?;
    fs::create_dir_all(&paths.workdir)?;
    let runtime_toml = paths.workdir.join("runtime.toml");
    fs::write(
        &runtime_toml,
        runtime_toml_text(&paths.shm, capstone_capacity(), &paths.workdir),
    )?;
    let config = RuntimeConfig::load(&runtime_toml)?;

    let mut runtime = build_st_runtime(&deterministic_st_program());
    runtime.configure_openot_telemetry(&config.openot, Some(&paths.workdir))?;

    write_status_atomic(&paths.producer_ready, "ready\n")?;
    wait_for_file(&paths.start, Duration::from_secs(30))?;

    for _ in 0..producer_cycle_count() {
        runtime.execute_cycle()?;
    }

    write_status_atomic(&paths.done, "done\n")?;
    Ok(())
}

fn run_consumer_role() -> Result<(), Box<dyn Error>> {
    let paths = CapstonePaths::from_env()?;
    let store = SharedConcurrentStore::open_existing_with_mode(&paths.shm, FenceMode::Fenced)?;
    let mut raw = ConcurrentRawConsumer::with_store(store.clone());
    let mut accounting = LossAccountingConsumer::new();
    let mut observer = BatchObserver::new(SidecarExpectedAbsOracle::new(
        capstone_capacity(),
        expected_records()?,
    )?);

    write_status_atomic(&paths.consumer_ready, "ready\n")?;
    wait_for_file(&paths.start, Duration::from_secs(30))?;

    let deadline = Instant::now() + Duration::from_secs(capstone_timeout_secs());
    loop {
        let batch = raw
            .poll()
            .map_err(|error| format!("capstone consumer poll failed: {error:?}"))?;
        observer.observe_batch(&batch)?;
        accounting.account_batch(&batch);

        if paths.done.exists() && raw.cursor_abs() == raw.head_abs() {
            break;
        }

        if Instant::now() > deadline {
            return Err("consumer timed out before producer completion".into());
        }
        thread::yield_now();
    }

    let report = ObservedReport::from_consumer(ReportInputs {
        metadata: ObservationMetadata::new("capstone", "st-fb", FenceMode::Fenced),
        expected_sources: expected_sources(),
        raw: &raw,
        accounting: &accounting,
        store: &store,
        stale_violations: observer.into_violations(),
    });
    report.write(&paths.observed)?;
    println!("{}", report.summary_line());
    Ok(())
}

fn build_st_runtime(main_source: &str) -> Runtime {
    let session =
        CompileSession::from_sources(openot_st_source_files(main_source)).label_errors(true);
    let mut runtime = session
        .build_runtime()
        .unwrap_or_else(|err| panic!("build OpenOT ST runtime: {err}"));
    let bytecode = session
        .build_bytecode_bytes()
        .unwrap_or_else(|err| panic!("build OpenOT ST bytecode: {err}"));
    runtime
        .apply_bytecode_bytes(&bytecode, None)
        .unwrap_or_else(|err| panic!("apply OpenOT ST bytecode: {err}"));
    runtime
}

fn openot_st_source_files(main_source: &str) -> Vec<SourceFile> {
    let source_dir = openot_iec_dir().join("src");
    let mut paths = fs::read_dir(&source_dir)
        .unwrap_or_else(|err| panic!("read OpenOT ST source dir {}: {err}", source_dir.display()))
        .map(|entry| entry.expect("read OpenOT ST source entry").path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "st"))
        .collect::<Vec<_>>();
    paths.sort();

    let mut sources = paths
        .iter()
        .map(|path| {
            let text = fs::read_to_string(path)
                .unwrap_or_else(|err| panic!("read OpenOT ST source {}: {err}", path.display()));
            let name = path
                .file_name()
                .and_then(|name| name.to_str())
                .expect("OpenOT ST source filename must be UTF-8");
            SourceFile::with_path(name, text)
        })
        .collect::<Vec<_>>();
    sources.push(SourceFile::with_path("main.st", main_source));
    sources
}

fn openot_iec_dir() -> PathBuf {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_dir.join("../../../open-ot-ref/st/iec61131")
}

fn deterministic_st_program() -> String {
    let per_source = capstone_per_source();
    let total_data_records = SOURCE_IDS.len() as u64 * per_source;
    let second_source_start = per_source;
    format!(
        r#"
PROGRAM Main
VAR
    Producer : OPENOT_Producer;
    Phase : UDINT;
    CurrentSourceId : UDINT;
    DefHashOld : OPENOT_HASH8 := [
        BYTE#16#6F, BYTE#16#6C, BYTE#16#64, BYTE#16#68,
        BYTE#16#61, BYTE#16#73, BYTE#16#68, BYTE#16#31
    ];
    DefHashNew : OPENOT_HASH8 := [
        BYTE#16#6E, BYTE#16#65, BYTE#16#77, BYTE#16#68,
        BYTE#16#61, BYTE#16#73, BYTE#16#68, BYTE#16#32
    ];
END_VAR

IF Phase < UDINT#{second_source_start} THEN
    CurrentSourceId := UDINT#10;
    Producer(Execute := TRUE, Op := UINT#0, SourceId := CurrentSourceId, Checkpoint := FALSE);
    Producer(Execute := FALSE, Op := UINT#0, SourceId := CurrentSourceId, Checkpoint := FALSE);
ELSIF Phase < UDINT#{total_data_records} THEN
    CurrentSourceId := UDINT#30;
    Producer(Execute := TRUE, Op := UINT#0, SourceId := CurrentSourceId, Checkpoint := FALSE);
    Producer(Execute := FALSE, Op := UINT#0, SourceId := CurrentSourceId, Checkpoint := FALSE);
ELSIF Phase = UDINT#{total_data_records} THEN
    Producer(
        Execute := TRUE,
        Op := UINT#1,
        SourceId := UDINT#0,
        Checkpoint := FALSE,
        DefHashOld := DefHashOld,
        DefHashNew := DefHashNew,
        TransitionEpochId := ULINT#2);
    Producer(Execute := FALSE, Op := UINT#1, SourceId := UDINT#0, Checkpoint := FALSE);
ELSE
    Producer(Execute := FALSE, Op := UINT#0, SourceId := UDINT#0, Checkpoint := FALSE);
END_IF;

Phase := Phase + UDINT#1;
END_PROGRAM
"#
    )
}

fn expected_records() -> Result<Vec<ExpectedRecord>, Box<dyn Error>> {
    let mut records = Vec::new();
    let per_source = capstone_per_source();
    let message_len = encoded_len(&Record::new(0, RUN_ID, 0, SOURCE_IDS[0], EVENT_MESSAGE))?;
    let high_water_len = encoded_len(&source_high_water_record(SOURCE_IDS[0], per_source))?;
    let logger_stopped_len = encoded_len(&Record::new(
        0,
        RUN_ID,
        0,
        SYSTEM_SOURCE_ID,
        EVENT_LOGGER_STOPPED,
    ))?;

    for source_id in SOURCE_IDS {
        for seq in 0..per_source {
            records.push(ExpectedRecord::new(
                RUN_ID,
                source_id,
                seq,
                EVENT_MESSAGE,
                message_len,
            ));
        }
    }
    records.push(ExpectedRecord::new(
        RUN_ID,
        SYSTEM_SOURCE_ID,
        0,
        EVENT_LOGGER_STOPPED,
        logger_stopped_len,
    ));
    for source_id in SOURCE_IDS {
        records.push(ExpectedRecord::new(
            RUN_ID,
            source_id,
            per_source,
            EVENT_SOURCE_HIGH_WATER,
            high_water_len,
        ));
    }
    Ok(records)
}

fn expected_sources() -> Vec<ExpectedSource> {
    let per_source = capstone_per_source();
    SOURCE_IDS
        .into_iter()
        .map(|source_id| ExpectedSource::new(RUN_ID, source_id, per_source + 1))
        .chain([ExpectedSource::new(RUN_ID, SYSTEM_SOURCE_ID, 1)])
        .collect()
}

fn source_high_water_record(source_id: u32, produced_count: u64) -> Record {
    let mut record = Record::new(
        1_780_000_000_000_000_000 + produced_count,
        RUN_ID,
        produced_count,
        source_id,
        EVENT_SOURCE_HIGH_WATER,
    );
    record.slots.push(Slot::new(
        KEY_SOURCE_HIGH_WATER,
        TY_ULINT,
        produced_count.to_le_bytes(),
    ));
    record
}

fn encoded_len(record: &Record) -> Result<usize, Box<dyn Error>> {
    Ok(record
        .encode(true)
        .map_err(|error| format!("encode expected record: {error:?}"))?
        .len())
}

fn producer_cycle_count() -> u64 {
    SOURCE_IDS.len() as u64 * capstone_per_source() + 2
}

fn capstone_per_source() -> u64 {
    env::var(PER_SOURCE_ENV)
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(DEFAULT_PER_SOURCE)
}

fn capstone_capacity() -> usize {
    env::var(CAPACITY_ENV)
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(DEFAULT_CAPACITY)
}

fn capstone_timeout_secs() -> u64 {
    env::var(TIMEOUT_SECS_ENV)
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(DEFAULT_TIMEOUT_SECS)
}

fn assert_expected_source_rows(report: &ObservedReport) {
    let expected = expected_sources();
    assert_eq!(report.sources.len(), expected.len());
    for expected in expected {
        let source = report
            .sources
            .iter()
            .find(|source| {
                source.run_id == expected.run_id && source.source_id == expected.source_id
            })
            .unwrap_or_else(|| {
                panic!(
                    "missing source row run={} source={}",
                    expected.run_id, expected.source_id
                )
            });
        assert_eq!(source.expected_total, expected.expected_total);
        assert_eq!(source.delivered + source.lost, expected.expected_total);
    }
}

fn runtime_toml_text(shm_path: &Path, capacity: usize, workdir: &Path) -> String {
    format!(
        r#"
[bundle]
version = 1

[resource]
name = "openot-capstone"
cycle_interval_ms = 1

[runtime]
execution_backend = "vm"

[runtime.control]
endpoint = "unix://{control_socket}"
mode = "production"
debug_enabled = false

[runtime.log]
level = "info"

[runtime.retain]
mode = "none"
save_interval_ms = 1000

[runtime.watchdog]
enabled = false
timeout_ms = 5000
action = "halt"

[runtime.fault]
policy = "halt"

[runtime.web]
enabled = true
listen = "127.0.0.1:0"
auth = "local"
tls = false

[runtime.discovery]
enabled = false
service_name = "truST"
advertise = false
interfaces = ["lo"]

[runtime.mesh]
enabled = false
listen = "127.0.0.1:0"
tls = false
publish = []
subscribe = {{}}

[runtime.cloud]
profile = "dev"

[runtime.cloud.wan]
allow_write = []

[runtime.openot]
enabled = true
path = "{shm_path}"
capacity = {capacity}
fence_mode = "fenced"
allow_unfenced_for_proof = false
source = "st-fb"
producer_instance = "Main.Producer"
"#,
        control_socket = workdir.join("control.sock").display(),
        shm_path = shm_path.display()
    )
}

#[derive(Debug, Clone)]
struct CapstonePaths {
    root: PathBuf,
    workdir: PathBuf,
    shm: PathBuf,
    producer_ready: PathBuf,
    consumer_ready: PathBuf,
    start: PathBuf,
    done: PathBuf,
    observed: PathBuf,
}

impl CapstonePaths {
    fn new() -> Self {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let root_base = if Path::new("/dev/shm").is_dir() {
            PathBuf::from("/dev/shm")
        } else {
            env::temp_dir()
        };
        let root = root_base.join(format!(
            "trust-openot-capstone-{}-{stamp}",
            std::process::id()
        ));
        Self::from_root(root)
    }

    fn from_env() -> Result<Self, Box<dyn Error>> {
        Ok(Self {
            root: env_path("OPENOT_CAPSTONE_ROOT")?,
            workdir: env_path(WORKDIR_ENV)?,
            shm: env_path(SHM_ENV)?,
            producer_ready: env_path(PRODUCER_READY_ENV)?,
            consumer_ready: env_path(CONSUMER_READY_ENV)?,
            start: env_path(START_ENV)?,
            done: env_path(DONE_ENV)?,
            observed: env_path(OBSERVED_ENV)?,
        })
    }

    fn from_root(root: PathBuf) -> Self {
        Self {
            workdir: root.join("producer-workdir"),
            shm: root.join("openot-capstone.shm"),
            producer_ready: root.join("producer.ready"),
            consumer_ready: root.join("consumer.ready"),
            start: root.join("start"),
            done: root.join("done"),
            observed: root.join("observed.txt"),
            root,
        }
    }

    fn cleanup(&self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn spawn_role(
    role: &str,
    test_name: &str,
    preferred_core: Option<usize>,
    paths: &CapstonePaths,
) -> io::Result<Child> {
    fs::create_dir_all(&paths.root)?;
    let mut command = command_for_current_exe(preferred_core.filter(|_| can_use_taskset()))?;
    command
        .arg("--ignored")
        .arg("--exact")
        .arg(test_name)
        .arg("--nocapture")
        .env("OPENOT_CAPSTONE_ROLE", role)
        .env("OPENOT_CAPSTONE_ROOT", &paths.root)
        .env(PER_SOURCE_ENV, capstone_per_source().to_string())
        .env(CAPACITY_ENV, capstone_capacity().to_string())
        .env(TIMEOUT_SECS_ENV, capstone_timeout_secs().to_string())
        .env(WORKDIR_ENV, &paths.workdir)
        .env(SHM_ENV, &paths.shm)
        .env(PRODUCER_READY_ENV, &paths.producer_ready)
        .env(CONSUMER_READY_ENV, &paths.consumer_ready)
        .env(START_ENV, &paths.start)
        .env(DONE_ENV, &paths.done)
        .env(OBSERVED_ENV, &paths.observed)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    command.spawn()
}

fn command_for_current_exe(core: Option<usize>) -> io::Result<Command> {
    let exe = env::current_exe()?;
    let command = if let Some(core) = core {
        let mut command = Command::new("taskset");
        command.arg("-c").arg(core.to_string()).arg(exe);
        command
    } else {
        Command::new(exe)
    };
    Ok(command)
}

fn can_use_taskset() -> bool {
    if thread::available_parallelism().map_or(1, usize::from) < 2 {
        return false;
    }
    Command::new("taskset")
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
}

fn wait_for_role(label: &str, child: Child) -> Result<Output, Box<dyn Error>> {
    child
        .wait_with_output()
        .map_err(|err| format!("wait for {label} failed: {err}").into())
}

fn assert_role_success(label: &str, output: &Output) {
    if output.status.success() {
        return;
    }
    panic!(
        "{label} role failed with {}\nstdout:\n{}\nstderr:\n{}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn wait_for_file(path: &Path, timeout: Duration) -> Result<(), Box<dyn Error>> {
    let deadline = Instant::now() + timeout;
    while !path.exists() {
        if Instant::now() > deadline {
            return Err(format!("timed out waiting for {}", path.display()).into());
        }
        thread::sleep(Duration::from_millis(10));
    }
    Ok(())
}

fn write_status_atomic(path: &Path, text: &str) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let tmp = PathBuf::from(format!("{}.{}.tmp", path.display(), std::process::id()));
    let mut file = fs::File::create(&tmp)?;
    file.write_all(text.as_bytes())?;
    file.sync_all()?;
    fs::rename(tmp, path)
}

fn env_path(name: &str) -> Result<PathBuf, Box<dyn Error>> {
    env::var_os(name)
        .map(PathBuf::from)
        .ok_or_else(|| format!("missing env {name}").into())
}
