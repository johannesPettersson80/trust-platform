#![cfg(unix)]

use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use open_ot_carriage::concurrent::ConcurrentRawConsumer;
use open_ot_carriage::registry::{EVENT_HEARTBEAT, EVENT_MESSAGE, SYSTEM_SOURCE_ID};
use open_ot_shm::SharedConcurrentStore;
use trust_runtime::config::{
    OpenOtTelemetryConfig, OpenOtTelemetryFenceMode, OpenOtTelemetrySource,
};
use trust_runtime::harness::{CompileSession, SourceFile};
use trust_runtime::Runtime;

fn temp_shm_path(name: &str) -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    std::env::temp_dir().join(format!("trust-runtime-openot-{name}-{stamp}.shm"))
}

fn telemetry_config(path: PathBuf, capacity: usize) -> OpenOtTelemetryConfig {
    OpenOtTelemetryConfig {
        enabled: true,
        path,
        capacity,
        fence_mode: OpenOtTelemetryFenceMode::Fenced,
        allow_unfenced_for_proof: false,
        source: OpenOtTelemetrySource::Heartbeat,
        producer_instance: None,
    }
}

fn st_fb_telemetry_config(path: PathBuf, capacity: usize) -> OpenOtTelemetryConfig {
    OpenOtTelemetryConfig {
        source: OpenOtTelemetrySource::StFb,
        producer_instance: Some("Main.Producer".into()),
        ..telemetry_config(path, capacity)
    }
}

fn openot_st_source_files(main_source: &str) -> Vec<SourceFile> {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let source_dir = manifest_dir.join("../../../open-ot-ref/st/iec61131/src");
    let mut paths = std::fs::read_dir(&source_dir)
        .unwrap_or_else(|err| panic!("read OpenOT ST source dir {}: {err}", source_dir.display()))
        .map(|entry| entry.expect("read OpenOT ST source entry").path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "st"))
        .collect::<Vec<_>>();
    paths.sort();

    let mut sources = paths
        .iter()
        .map(|path| {
            let text = std::fs::read_to_string(path)
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

fn one_message_per_scan_program() -> &'static str {
    r#"
PROGRAM Main
VAR
    Producer : OPENOT_Producer;
END_VAR

Producer(Execute := TRUE, Op := UINT#0, SourceId := UDINT#77, Checkpoint := FALSE);
Producer(Execute := FALSE, Op := UINT#0, SourceId := UDINT#77, Checkpoint := FALSE);
END_PROGRAM
"#
}

fn two_messages_per_scan_program() -> &'static str {
    r#"
PROGRAM Main
VAR
    Producer : OPENOT_Producer;
END_VAR

Producer(Execute := TRUE, Op := UINT#0, SourceId := UDINT#77, Checkpoint := FALSE);
Producer(Execute := FALSE, Op := UINT#0, SourceId := UDINT#77, Checkpoint := FALSE);
Producer(Execute := TRUE, Op := UINT#0, SourceId := UDINT#77, Checkpoint := FALSE);
Producer(Execute := FALSE, Op := UINT#0, SourceId := UDINT#77, Checkpoint := FALSE);
END_PROGRAM
"#
}

#[test]
fn openot_telemetry_publishes_crc_valid_heartbeats() {
    let path = temp_shm_path("heartbeat");
    let mut runtime = Runtime::new();
    runtime
        .configure_openot_telemetry(&telemetry_config(path.clone(), 4096), None)
        .expect("configure OpenOT telemetry");

    for _ in 0..3 {
        runtime.execute_cycle().expect("cycle publishes heartbeat");
    }

    let store = SharedConcurrentStore::open_existing(&path).expect("open telemetry shm");
    let mut consumer = ConcurrentRawConsumer::with_store(store);
    let batch = consumer.poll().expect("poll telemetry records");
    assert_eq!(batch.records.len(), 3);

    for (expected_seq, read) in batch.records.iter().enumerate() {
        let expected_seq = u64::try_from(expected_seq).expect("small test sequence");
        let record = &read.record;
        assert_eq!(record.event_type_id, EVENT_HEARTBEAT);
        assert_eq!(record.source_id, SYSTEM_SOURCE_ID);
        assert_eq!(record.run_id, 1);
        assert_eq!(record.seq, expected_seq);
        assert_eq!(record.source_time, expected_seq);
        assert!(record.slots.is_empty());
    }

    drop(std::fs::remove_file(path));
}

#[test]
fn openot_telemetry_publish_failure_is_fail_closed() {
    let path = temp_shm_path("too-small");
    let mut runtime = Runtime::new();
    runtime
        .configure_openot_telemetry(&telemetry_config(path.clone(), 1), None)
        .expect("configure tiny OpenOT telemetry ring");

    let err = runtime
        .execute_cycle()
        .expect_err("telemetry append failure must fail the cycle");
    assert!(err
        .to_string()
        .contains("OpenOT telemetry publish heartbeat"));

    drop(std::fs::remove_file(path));
}

#[test]
fn openot_telemetry_publishes_real_st_producer_records() {
    let path = temp_shm_path("st-fb");
    let mut runtime = build_st_runtime(one_message_per_scan_program());
    runtime
        .configure_openot_telemetry(&st_fb_telemetry_config(path.clone(), 4096), None)
        .expect("configure OpenOT ST FB telemetry");

    const CYCLES: usize = 5;
    for _ in 0..CYCLES {
        runtime
            .execute_cycle()
            .expect("cycle publishes one ST producer record");
    }

    let store = SharedConcurrentStore::open_existing(&path).expect("open telemetry shm");
    let mut consumer = ConcurrentRawConsumer::with_store(store);
    let batch = consumer.poll().expect("poll ST producer telemetry records");
    assert_eq!(batch.records.len(), CYCLES);

    for (expected_seq, read) in batch.records.iter().enumerate() {
        let expected_seq = u64::try_from(expected_seq).expect("small test sequence");
        let record = &read.record;
        assert_eq!(record.event_type_id, EVENT_MESSAGE);
        assert_eq!(record.source_id, 77);
        assert_eq!(record.run_id, 1);
        assert_eq!(record.seq, expected_seq);
        assert_eq!(record.source_time, 1_780_000_000_000_000_000 + expected_seq);
        assert!(record.slots.is_empty());
    }

    drop(std::fs::remove_file(path));
}

#[test]
fn openot_telemetry_st_fb_multi_record_scan_is_fail_closed() {
    let path = temp_shm_path("st-fb-multi");
    let mut runtime = build_st_runtime(two_messages_per_scan_program());
    runtime
        .configure_openot_telemetry(&st_fb_telemetry_config(path.clone(), 4096), None)
        .expect("configure OpenOT ST FB telemetry");

    let err = runtime
        .execute_cycle()
        .expect_err("multiple ST producer records in one scan must fail closed");
    assert!(err.to_string().contains("published 2 records in one scan"));

    drop(std::fs::remove_file(path));
}
