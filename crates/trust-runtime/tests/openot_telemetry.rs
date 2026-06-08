#![cfg(unix)]

use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use open_ot_carriage::concurrent::{ConcurrentRawConsumer, ConcurrentStore};
use open_ot_carriage::consumer::LossAccountingConsumer;
use open_ot_carriage::control::ControlBlockSnapshot;
use open_ot_carriage::loss::LossEvent;
use open_ot_carriage::registry::{
    EVENT_CONDITION_ACKNOWLEDGED, EVENT_CONDITION_ACTIVE, EVENT_CONDITION_CLEARED,
    EVENT_CONDITION_CONFIRMED, EVENT_CONDITION_IN_SERVICE, EVENT_CONDITION_OUT_OF_SERVICE,
    EVENT_CONDITION_RESET, EVENT_CONDITION_SHELVED, EVENT_CONDITION_SUPPRESSED,
    EVENT_CONDITION_UNSHELVED, EVENT_CONDITION_UNSUPPRESSED, EVENT_HEARTBEAT, EVENT_LOGGER_STOPPED,
    EVENT_MESSAGE, EVENT_SOURCE_HIGH_WATER, EVENT_STATE_TRANSITION, EVENT_VALUE_CHANGED,
    KEY_ACK_BY, KEY_ARG, KEY_CATEGORY, KEY_CAUSE_OPERAND, KEY_CONDITION_CLASS, KEY_CONDITION_ID,
    KEY_CORRELATION_ID, KEY_MESSAGE_TEMPLATE_ID, KEY_NEW_STATE, KEY_NEW_VALUE, KEY_PREVIOUS_STATE,
    KEY_PREVIOUS_VALUE, KEY_REASON, KEY_SEVERITY, KEY_SHELVE_SECS, KEY_SOURCE_HIGH_WATER,
    KEY_STATE_MACHINE_ID, KEY_VALUE_ID, SYSTEM_SOURCE_ID, TY_BOOL, TY_DINT, TY_INT, TY_LINT,
    TY_LREAL, TY_REAL, TY_SINT, TY_STRING, TY_UDINT, TY_UINT, TY_ULINT, TY_USINT,
};
use open_ot_carriage::ring::{ReadRecord, DEFAULT_BUFFER_ID};
use open_ot_carriage::wire::{Record, Slot};
use open_ot_definition::{compute_content_hash, resolve_record, DefinitionFile, DefinitionSet};
use open_ot_document::{
    document_from_loss, document_from_resolution, to_json, EpochRelation, LossDocumentContext,
    RecordDocumentContext,
};
use open_ot_shm::SharedConcurrentStore;
use serde_json::json;
use time::OffsetDateTime;
use trust_runtime::config::{
    OpenOtTelemetryConfig, OpenOtTelemetryFenceMode, OpenOtTelemetrySource,
};
use trust_runtime::harness::{CompileSession, SourceFile, TestHarness};
use trust_runtime::memory::InstanceId;
use trust_runtime::openot_authoring;
use trust_runtime::value::Value;
use trust_runtime::Runtime;

const DETERMINISTIC_SOURCE_TIME_BASE: u64 = 1_780_000_000_000_000_000;

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
    st_fb_telemetry_config_for(path, capacity, "Main.Producer")
}

fn st_fb_telemetry_config_for(
    path: PathBuf,
    capacity: usize,
    producer_instance: &str,
) -> OpenOtTelemetryConfig {
    OpenOtTelemetryConfig {
        source: OpenOtTelemetrySource::StFb,
        producer_instance: Some(producer_instance.into()),
        ..telemetry_config(path, capacity)
    }
}

fn openot_iec_dir() -> PathBuf {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_dir.join("../../../open-ot-ref/st/iec61131")
}

fn openot_ref_dir() -> PathBuf {
    openot_iec_dir()
        .parent()
        .and_then(|path| path.parent())
        .expect("IEC dir has open-ot-ref root")
        .to_path_buf()
}

fn reactor_program() -> String {
    let path = openot_ref_dir().join("examples/reactor/Reactor.st");
    std::fs::read_to_string(&path)
        .unwrap_or_else(|err| panic!("read reactor example {}: {err}", path.display()))
}

fn openot_st_source_paths() -> Vec<PathBuf> {
    let source_dir = openot_iec_dir().join("src");
    let mut paths = std::fs::read_dir(&source_dir)
        .unwrap_or_else(|err| panic!("read OpenOT ST source dir {}: {err}", source_dir.display()))
        .map(|entry| entry.expect("read OpenOT ST source entry").path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "st"))
        .collect::<Vec<_>>();
    paths.sort();
    paths
}

fn openot_st_source_files(main_source: &str) -> Vec<SourceFile> {
    let mut sources = openot_st_source_paths()
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

fn openot_st_test_sources(test_name: &str, main_source: &str) -> Vec<String> {
    let mut sources = openot_st_source_paths()
        .iter()
        .map(|path| {
            std::fs::read_to_string(path)
                .unwrap_or_else(|err| panic!("read OpenOT ST source {}: {err}", path.display()))
        })
        .collect::<Vec<_>>();
    let test_path = openot_iec_dir().join("tests").join(test_name);
    sources.push(
        std::fs::read_to_string(&test_path)
            .unwrap_or_else(|err| panic!("read OpenOT ST test {}: {err}", test_path.display())),
    );
    sources.push(main_source.to_string());
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

Producer(Execute := TRUE, Op := UINT#0, SourceId := UDINT#77, Checkpoint := FALSE, MessageTemplateId := UDINT#10001);
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

Producer(Execute := TRUE, Op := UINT#0, SourceId := UDINT#77, Checkpoint := FALSE, MessageTemplateId := UDINT#10001);
Producer(Execute := FALSE, Op := UINT#0, SourceId := UDINT#77, Checkpoint := FALSE);
Producer(Execute := TRUE, Op := UINT#0, SourceId := UDINT#77, Checkpoint := FALSE, MessageTemplateId := UDINT#10001);
Producer(Execute := FALSE, Op := UINT#0, SourceId := UDINT#77, Checkpoint := FALSE);
END_PROGRAM
"#
}

fn transition_burst_program() -> &'static str {
    r#"
PROGRAM Main
VAR
    Producer : OPENOT_Producer;
    Phase : UINT;
    DefHashOld : OPENOT_HASH8 := [
        BYTE#16#6F, BYTE#16#6C, BYTE#16#64, BYTE#16#68,
        BYTE#16#61, BYTE#16#73, BYTE#16#68, BYTE#16#31
    ];
    DefHashNew : OPENOT_HASH8 := [
        BYTE#16#6E, BYTE#16#65, BYTE#16#77, BYTE#16#68,
        BYTE#16#61, BYTE#16#73, BYTE#16#68, BYTE#16#32
    ];
END_VAR

CASE Phase OF
    UINT#0:
        Producer(Execute := TRUE, Op := UINT#0, SourceId := UDINT#30, Checkpoint := FALSE, MessageTemplateId := UDINT#10001);
        Producer(Execute := FALSE, Op := UINT#0, SourceId := UDINT#30, Checkpoint := FALSE);
    UINT#1:
        Producer(Execute := TRUE, Op := UINT#0, SourceId := UDINT#10, Checkpoint := FALSE, MessageTemplateId := UDINT#10001);
        Producer(Execute := FALSE, Op := UINT#0, SourceId := UDINT#10, Checkpoint := FALSE);
    UINT#2:
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
END_CASE;

Phase := Phase + UINT#1;
END_PROGRAM
"#
}

fn run_openot_st_pou_test(test_name: &str, fb_name: &str) {
    let sources = openot_st_test_sources(
        test_name,
        &format!(
            r#"
PROGRAM Main
VAR
    Test : {fb_name};
    Passed : BOOL;
END_VAR

Test();
Passed := Test.Passed;
END_PROGRAM
"#
        ),
    );
    let source_refs = sources.iter().map(String::as_str).collect::<Vec<_>>();
    let mut harness = TestHarness::from_sources(&source_refs)
        .unwrap_or_else(|err| panic!("build OpenOT ST POU harness {fb_name}: {err}"));
    harness.cycle();

    if harness.get_output("Passed") != Some(Value::Bool(true)) {
        for name in [
            "FirstBatchCount",
            "FirstBatchDelta",
            "NoChangeCount",
            "NoChangeDelta",
            "HeadersOk",
            "EmissionsOk",
            "ChangeSuppressionOk",
            "PeriodicInitialDelta",
            "PeriodicEarlyDelta",
            "PeriodicIntervalDelta",
            "PeriodicChangeDelta",
            "HysteresisInitialDelta",
            "HysteresisInsideDelta",
            "HysteresisUpCrossDelta",
            "HysteresisInsideAfterCenterDelta",
            "HysteresisDownCrossDelta",
            "DeadbandInitialDelta",
            "DeadbandInsideDelta",
            "DeadbandCrossDelta",
            "OnChangeInitialDelta",
            "OnChangeSameDelta",
            "OnChangeChangeDelta",
            "AckPassed",
            "ShelvedPassed",
            "SuppressedPassed",
            "OutOfServicePassed",
            "MismatchIndex",
        ] {
            if let Some(value) = harness.get_output(name) {
                eprintln!("{name}: {value:?}");
            }
        }
    }

    assert_eq!(harness.get_output("Passed"), Some(Value::Bool(true)));
}

#[test]
fn openot_st_scan_records_burst_pou_passes() {
    let sources = openot_st_test_sources(
        "test_scan_records_burst.st",
        r#"
PROGRAM Main
VAR
    Test : OPENOT_TestScanRecordsBurst;
    Passed : BOOL;
    BurstNoEdgeScanRecordCount : UINT;
    SingleNoEdgeScanRecordCount : UINT;
END_VAR

Test();
Passed := Test.Passed;
BurstNoEdgeScanRecordCount := Test.BurstNoEdgeScanRecordCount;
SingleNoEdgeScanRecordCount := Test.SingleNoEdgeScanRecordCount;
END_PROGRAM
"#,
    );
    let source_refs = sources.iter().map(String::as_str).collect::<Vec<_>>();
    let mut harness = TestHarness::from_sources(&source_refs)
        .unwrap_or_else(|err| panic!("build scan-record burst POU harness: {err}"));
    harness.cycle();

    assert_eq!(harness.get_output("Passed"), Some(Value::Bool(true)));
    assert_eq!(
        harness.get_output("BurstNoEdgeScanRecordCount"),
        Some(Value::UInt(3))
    );
    assert_eq!(
        harness.get_output("SingleNoEdgeScanRecordCount"),
        Some(Value::UInt(1))
    );
}

#[test]
fn openot_st_value_changed_vectors_are_byte_exact() {
    run_openot_st_pou_test(
        "test_conformant_value_changed_real.st",
        "OPENOT_TestConformantValueChangedReal",
    );
    run_openot_st_pou_test(
        "test_conformant_value_changed_int.st",
        "OPENOT_TestConformantValueChangedInt",
    );
}

#[test]
fn openot_st_condition_lifecycle_vectors_are_byte_exact() {
    run_openot_st_pou_test(
        "test_conformant_condition_lifecycle.st",
        "OPENOT_TestConformantConditionLifecycle",
    );
}

#[test]
fn openot_st_authoring_api_pou_passes() {
    let sources = openot_st_test_sources(
        "test_authoring_api.st",
        r#"
PROGRAM Main
VAR
    Test : OPENOT_TestAuthoringApi;
    Passed : BOOL;
    FirstBatchCount : UINT;
    FirstBatchDelta : ULINT;
    NoChangeCount : UINT;
    NoChangeDelta : ULINT;
    HeadersOk : BOOL;
    EmissionsOk : BOOL;
    ChangeSuppressionOk : BOOL;
END_VAR

Test();
Passed := Test.Passed;
FirstBatchCount := Test.FirstBatchCount;
FirstBatchDelta := Test.FirstBatchDelta;
NoChangeCount := Test.NoChangeCount;
NoChangeDelta := Test.NoChangeDelta;
HeadersOk := Test.HeadersOk;
EmissionsOk := Test.EmissionsOk;
ChangeSuppressionOk := Test.ChangeSuppressionOk;
END_PROGRAM
"#,
    );
    let source_refs = sources.iter().map(String::as_str).collect::<Vec<_>>();
    let mut harness = TestHarness::from_sources(&source_refs)
        .unwrap_or_else(|err| panic!("build authoring API POU harness: {err}"));
    harness.cycle();

    assert_eq!(harness.get_output("Passed"), Some(Value::Bool(true)));
}

#[test]
fn openot_st_value_sampling_pou_passes() {
    let sources = openot_st_test_sources(
        "test_value_sampling.st",
        r#"
PROGRAM Main
VAR
    Test : OPENOT_TestValueSampling;
    Passed : BOOL;
    PeriodicInitialDelta : ULINT;
    PeriodicEarlyDelta : ULINT;
    PeriodicIntervalDelta : ULINT;
    PeriodicChangeDelta : ULINT;
    HysteresisInitialDelta : ULINT;
    HysteresisInsideDelta : ULINT;
    HysteresisUpCrossDelta : ULINT;
    HysteresisInsideAfterCenterDelta : ULINT;
    HysteresisDownCrossDelta : ULINT;
    DeadbandInitialDelta : ULINT;
    DeadbandInsideDelta : ULINT;
    DeadbandCrossDelta : ULINT;
    OnChangeInitialDelta : ULINT;
    OnChangeSameDelta : ULINT;
    OnChangeChangeDelta : ULINT;
END_VAR

Test();
Passed := Test.Passed;
PeriodicInitialDelta := Test.PeriodicInitialDelta;
PeriodicEarlyDelta := Test.PeriodicEarlyDelta;
PeriodicIntervalDelta := Test.PeriodicIntervalDelta;
PeriodicChangeDelta := Test.PeriodicChangeDelta;
HysteresisInitialDelta := Test.HysteresisInitialDelta;
HysteresisInsideDelta := Test.HysteresisInsideDelta;
HysteresisUpCrossDelta := Test.HysteresisUpCrossDelta;
HysteresisInsideAfterCenterDelta := Test.HysteresisInsideAfterCenterDelta;
HysteresisDownCrossDelta := Test.HysteresisDownCrossDelta;
DeadbandInitialDelta := Test.DeadbandInitialDelta;
DeadbandInsideDelta := Test.DeadbandInsideDelta;
DeadbandCrossDelta := Test.DeadbandCrossDelta;
OnChangeInitialDelta := Test.OnChangeInitialDelta;
OnChangeSameDelta := Test.OnChangeSameDelta;
OnChangeChangeDelta := Test.OnChangeChangeDelta;
END_PROGRAM
"#,
    );
    let source_refs = sources.iter().map(String::as_str).collect::<Vec<_>>();
    let mut harness = TestHarness::from_sources(&source_refs)
        .unwrap_or_else(|err| panic!("build value sampling POU harness: {err}"));
    harness.cycle();

    assert_eq!(harness.get_output("Passed"), Some(Value::Bool(true)));
    assert_eq!(
        harness.get_output("PeriodicInitialDelta"),
        Some(Value::ULInt(1))
    );
    assert_eq!(
        harness.get_output("PeriodicEarlyDelta"),
        Some(Value::ULInt(0))
    );
    assert_eq!(
        harness.get_output("PeriodicIntervalDelta"),
        Some(Value::ULInt(1))
    );
    assert_eq!(
        harness.get_output("PeriodicChangeDelta"),
        Some(Value::ULInt(1))
    );
    assert_eq!(
        harness.get_output("HysteresisInsideDelta"),
        Some(Value::ULInt(0))
    );
    assert_eq!(
        harness.get_output("HysteresisUpCrossDelta"),
        Some(Value::ULInt(1))
    );
    assert_eq!(
        harness.get_output("HysteresisInsideAfterCenterDelta"),
        Some(Value::ULInt(0))
    );
    assert_eq!(
        harness.get_output("HysteresisDownCrossDelta"),
        Some(Value::ULInt(1))
    );
    assert_eq!(
        harness.get_output("DeadbandInsideDelta"),
        Some(Value::ULInt(0))
    );
    assert_eq!(
        harness.get_output("DeadbandCrossDelta"),
        Some(Value::ULInt(1))
    );
    assert_eq!(
        harness.get_output("OnChangeSameDelta"),
        Some(Value::ULInt(0))
    );
    assert_eq!(
        harness.get_output("OnChangeChangeDelta"),
        Some(Value::ULInt(1))
    );
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
        assert_eq!(required_udint(record, KEY_MESSAGE_TEMPLATE_ID), 10001);
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
    assert!(err.to_string().contains("delta 2 != ScanRecordCount 1"));

    drop(std::fs::remove_file(path));
}

#[test]
fn openot_telemetry_publishes_st_transition_burst() {
    let path = temp_shm_path("st-fb-burst");
    let mut runtime = build_st_runtime(transition_burst_program());
    runtime
        .configure_openot_telemetry(&st_fb_telemetry_config(path.clone(), 4096), None)
        .expect("configure OpenOT ST FB telemetry");

    let store = SharedConcurrentStore::open_existing(&path).expect("open telemetry shm");
    let mut consumer = ConcurrentRawConsumer::with_store(store);

    runtime
        .execute_cycle()
        .expect("cycle registers first source");
    let batch = consumer.poll().expect("poll first source registration");
    assert_eq!(batch.records.len(), 1);
    let record = &batch.records[0].record;
    assert_eq!(record.event_type_id, EVENT_MESSAGE);
    assert_eq!(record.source_id, 30);
    assert_eq!(record.run_id, 1);
    assert_eq!(record.seq, 0);

    runtime
        .execute_cycle()
        .expect("cycle registers second source");
    let batch = consumer.poll().expect("poll second source registration");
    assert_eq!(batch.records.len(), 1);
    let record = &batch.records[0].record;
    assert_eq!(record.event_type_id, EVENT_MESSAGE);
    assert_eq!(record.source_id, 10);
    assert_eq!(record.run_id, 1);
    assert_eq!(record.seq, 0);

    runtime
        .execute_cycle()
        .expect("cycle publishes transition burst");
    let batch = consumer.poll().expect("poll transition burst");
    assert_eq!(batch.records.len(), 3);

    let stopped = &batch.records[0].record;
    assert_eq!(stopped.event_type_id, EVENT_LOGGER_STOPPED);
    assert_eq!(stopped.source_id, SYSTEM_SOURCE_ID);
    assert_eq!(stopped.run_id, 1);
    assert_eq!(stopped.seq, 0);
    assert!(stopped.slots.is_empty());

    let first_high_water = &batch.records[1].record;
    assert_eq!(first_high_water.event_type_id, EVENT_SOURCE_HIGH_WATER);
    assert_eq!(first_high_water.source_id, 10);
    assert_eq!(first_high_water.run_id, 1);
    assert_eq!(first_high_water.seq, 1);

    let second_high_water = &batch.records[2].record;
    assert_eq!(second_high_water.event_type_id, EVENT_SOURCE_HIGH_WATER);
    assert_eq!(second_high_water.source_id, 30);
    assert_eq!(second_high_water.run_id, 1);
    assert_eq!(second_high_water.seq, 1);

    runtime
        .execute_cycle()
        .expect("idle cycle ignores stale scan descriptors");
    let batch = consumer.poll().expect("poll idle cycle");
    assert_eq!(batch.records.len(), 0);

    drop(std::fs::remove_file(path));
}

#[test]
fn openot_telemetry_authoring_showcase_renders_typed_audit_log() {
    let path = temp_shm_path("authoring-showcase");
    let program = reactor_program();
    let mut runtime = build_st_runtime(&program);
    runtime
        .configure_openot_telemetry(
            &st_fb_telemetry_config_for(path.clone(), 4096, "Main.OotProducer"),
            None,
        )
        .expect("configure OpenOT ST FB telemetry");

    let store = SharedConcurrentStore::open_existing(&path).expect("open telemetry shm");
    let mut consumer = ConcurrentRawConsumer::with_store(store);
    let mut audit_log = Vec::new();
    let mut retained_records = Vec::new();
    let source_time_window_start = unix_epoch_ns();
    enable_host_source_time(&mut runtime);

    for _ in 0..8 {
        set_host_source_time(&mut runtime, unix_epoch_ns());
        runtime
            .execute_cycle()
            .expect("showcase cycle publishes typed audit record");
        let batch = consumer.poll().expect("poll showcase telemetry");
        assert!(!batch.lapped);
        audit_log.extend(render_audit_log(&batch.records));
        retained_records.extend(batch.records.iter().cloned());
    }

    assert!(retained_records
        .iter()
        .all(|read| read.record.source_time >= source_time_window_start
            && read.record.source_time <= unix_epoch_ns()));
    assert!(retained_records
        .iter()
        .all(|read| read.record.source_time != DETERMINISTIC_SOURCE_TIME_BASE + read.record.seq));

    let rendered_events = audit_log
        .iter()
        .map(|line| {
            let (timestamp, event) = line
                .split_once("  ")
                .unwrap_or_else(|| panic!("timestamp column missing from {line}"));
            assert_readable_utc_timestamp(timestamp);
            event
        })
        .collect::<Vec<_>>();
    assert_eq!(
        rendered_events,
        [
            "Message source=1 seq=0 templateId=10001 severity=100 args=[DINT(1)]",
            "StateTransition source=1 seq=1 machine=7001 category=0 previous=0 new=1",
            "ValueChanged source=1 seq=2 valueId=2001 new=REAL(0)",
            "ValueChanged source=1 seq=3 valueId=2002 new=DINT(1)",
            "ValueChanged source=1 seq=4 valueId=2001 previous=REAL(0) new=REAL(6)",
            "StateTransition source=1 seq=5 machine=7001 category=0 previous=1 new=2",
            "ValueChanged source=1 seq=6 valueId=2001 previous=REAL(6) new=REAL(12)",
            "ValueChanged source=1 seq=7 valueId=2001 previous=REAL(12) new=REAL(13.5)",
            "StateTransition source=1 seq=8 machine=7001 category=0 previous=2 new=3",
            "ValueChanged source=1 seq=9 valueId=2001 previous=REAL(13.5) new=REAL(15)",
            "ConditionActive source=1 seq=10 conditionId=9001 class=0 severity=900 correlation=1 causes=[1]",
            "ValueChanged source=1 seq=11 valueId=2001 previous=REAL(15) new=REAL(7.5)",
            "StateTransition source=1 seq=12 machine=7001 category=0 previous=3 new=4",
            "ValueChanged source=1 seq=13 valueId=2001 previous=REAL(7.5) new=REAL(0)",
            "ConditionCleared source=1 seq=14 conditionId=9001 class=0 correlation=1",
        ]
    );
    assert_eq!(consumer.rejected_records(), 0);

    let overflow = run_authoring_showcase_overflow();
    write_authoring_showcase_artifacts(&retained_records, &audit_log, &overflow, &program)
        .expect("write OpenOT authoring showcase artifact");

    drop(std::fs::remove_file(path));
}

#[test]
fn openot_telemetry_authoring_fixed_width_values_round_trip() {
    let path = temp_shm_path("authoring-fixed-width-values");
    let program = r#"
PROGRAM Main
VAR
    Enabled : BOOL {attribute 'oot' := 'value'};
    Total : ULINT {attribute 'oot' := 'value'};
    Ratio : LREAL {attribute 'oot' := 'value'};
    Label : STRING[16] {attribute 'oot' := 'value'};
END_VAR

Enabled := TRUE;
Total := ULINT#42;
Ratio := LREAL#1.25;
Label := 'ready';
END_PROGRAM
"#;
    let mut runtime = build_st_runtime(program);
    runtime
        .configure_openot_telemetry(
            &st_fb_telemetry_config_for(path.clone(), 4096, "Main.OotProducer"),
            None,
        )
        .expect("configure OpenOT ST FB telemetry");

    runtime
        .execute_cycle()
        .expect("fixed-width value cycle publishes");
    let store = SharedConcurrentStore::open_existing(&path).expect("open telemetry shm");
    let mut consumer = ConcurrentRawConsumer::with_store(store);
    let batch = consumer.poll().expect("poll fixed-width values");
    assert_eq!(batch.records.len(), 4);
    assert_eq!(
        slot(&batch.records[0].record, KEY_NEW_VALUE).unwrap().ty,
        TY_BOOL
    );
    assert_eq!(
        slot(&batch.records[1].record, KEY_NEW_VALUE).unwrap().ty,
        TY_ULINT
    );
    assert_eq!(
        slot(&batch.records[2].record, KEY_NEW_VALUE).unwrap().ty,
        TY_LREAL
    );
    assert_eq!(
        slot(&batch.records[3].record, KEY_NEW_VALUE).unwrap().ty,
        TY_STRING
    );
    assert_eq!(
        render_record(&batch.records[0].record),
        "ValueChanged source=1 seq=0 valueId=2001 new=BOOL(true)"
    );
    assert_eq!(
        render_record(&batch.records[1].record),
        "ValueChanged source=1 seq=1 valueId=2002 new=ULINT(42)"
    );
    assert_eq!(
        render_record(&batch.records[2].record),
        "ValueChanged source=1 seq=2 valueId=2003 new=LREAL(1.25)"
    );
    assert_eq!(
        render_record(&batch.records[3].record),
        "ValueChanged source=1 seq=3 valueId=2004 new=STRING(ready)"
    );

    drop(std::fs::remove_file(path));
}

#[test]
fn openot_telemetry_authoring_message_args_and_condition_correlation_round_trip() {
    let path = temp_shm_path("authoring-message-condition-rich");
    let program = r#"
PROGRAM Main
VAR
    Level : REAL {attribute 'oot' := 'value'};
    BatchCount : DINT {attribute 'oot' := 'value'};
    HighLevel : BOOL {attribute 'oot' := 'alarm', 'class' := 'alarm', 'severity' := '900', 'cause' := 'Level'};
    Started : BOOL {attribute 'oot' := 'message', 'template' := 'level count', 'severity' := '500', 'arg1' := 'Level', 'arg2' := 'BatchCount'};
END_VAR

Level := REAL#12.5;
BatchCount := DINT#7;
HighLevel := TRUE;
Started := TRUE;
END_PROGRAM
"#;
    let mut runtime = build_st_runtime(program);
    runtime
        .configure_openot_telemetry(
            &st_fb_telemetry_config_for(path.clone(), 4096, "Main.OotProducer"),
            None,
        )
        .expect("configure OpenOT ST FB telemetry");

    runtime
        .execute_cycle()
        .expect("rich authoring cycle publishes");
    let store = SharedConcurrentStore::open_existing(&path).expect("open telemetry shm");
    let mut consumer = ConcurrentRawConsumer::with_store(store);
    let batch = consumer.poll().expect("poll rich authoring records");
    assert_eq!(batch.records.len(), 4);
    assert_eq!(
        render_record(&batch.records[0].record),
        "ValueChanged source=1 seq=0 valueId=2001 new=REAL(12.5)"
    );
    assert_eq!(
        render_record(&batch.records[1].record),
        "ValueChanged source=1 seq=1 valueId=2002 new=DINT(7)"
    );
    assert_eq!(
        render_record(&batch.records[2].record),
        "ConditionActive source=1 seq=2 conditionId=9001 class=0 severity=900 correlation=1 causes=[1]"
    );
    assert_eq!(
        render_record(&batch.records[3].record),
        "Message source=1 seq=3 templateId=10001 severity=500 args=[REAL(12.5),DINT(7)]"
    );

    let definition = openot_authoring::definition_json_from_sources(&[SourceFile::with_path(
        "main.st", program,
    )])
    .expect("definition");
    assert_eq!(definition["messageTemplates"][0]["argTypes"], json!([9, 6]));
    assert_eq!(
        definition["conditions"][0]["causeOperands"],
        json!([{ "operandId": 1, "name": "Level" }])
    );

    drop(std::fs::remove_file(path));
}

#[test]
fn openot_telemetry_authoring_condition_lifecycle_round_trip() {
    let path = temp_shm_path("authoring-condition-lifecycle");
    let program = r#"
PROGRAM Main
    VAR
        Phase : UINT;
        OperatorName : STRING[32] := 'operator-a';
        ReasonText : STRING[32] := 'maintenance';
        ShelveSecs : UDINT := UDINT#300;
        AckHighPh : BOOL {attribute 'oot' := 'condition', 'of' := HighPhAlarm, 'event' := 'acknowledge', 'by' := OperatorName};
        HighPhAlarm : BOOL {attribute 'oot' := 'alarm', 'sourceid' := '77', 'conditionid' := '9101', 'class' := 'alarm', 'severity' := '900'};
        ConfirmHighPh : BOOL {attribute 'oot' := 'condition', 'of' := HighPhAlarm, 'event' := 'confirm', 'by' := OperatorName};
        ShelveHighPh : BOOL {attribute 'oot' := 'condition', 'of' := HighPhAlarm, 'event' := 'shelve', 'by' := OperatorName, 'seconds' := ShelveSecs};
        UnshelveHighPh : BOOL {attribute 'oot' := 'condition', 'of' := HighPhAlarm, 'event' := 'unshelve'};
        SuppressHighPh : BOOL {attribute 'oot' := 'condition', 'of' := HighPhAlarm, 'event' := 'suppress', 'reason' := ReasonText};
        UnsuppressHighPh : BOOL {attribute 'oot' := 'condition', 'of' := HighPhAlarm, 'event' := 'unsuppress'};
        OosHighPh : BOOL {attribute 'oot' := 'condition', 'of' := HighPhAlarm, 'event' := 'out-of-service', 'by' := OperatorName};
        InServiceHighPh : BOOL {attribute 'oot' := 'condition', 'of' := HighPhAlarm, 'event' := 'in-service'};
        ResetHighPh : BOOL {attribute 'oot' := 'condition', 'of' := HighPhAlarm, 'event' := 'reset', 'by' := OperatorName};
    END_VAR

    CASE Phase OF
        UINT#0:
            HighPhAlarm := TRUE;
            AckHighPh := TRUE;
        UINT#1:
            ConfirmHighPh := TRUE;
        UINT#2:
            ShelveHighPh := TRUE;
        UINT#3:
            UnshelveHighPh := TRUE;
        UINT#4:
            SuppressHighPh := TRUE;
        UINT#5:
            UnsuppressHighPh := TRUE;
        UINT#6:
            OosHighPh := TRUE;
        UINT#7:
            InServiceHighPh := TRUE;
        UINT#8:
            ResetHighPh := TRUE;
    END_CASE;
Phase := Phase + UINT#1;
END_PROGRAM
"#;
    let mut runtime = build_st_runtime(program);
    runtime
        .configure_openot_telemetry(
            &st_fb_telemetry_config_for(path.clone(), 4096, "Main.OotProducer"),
            None,
        )
        .expect("configure OpenOT ST FB telemetry");

    runtime
        .execute_cycle()
        .expect("active plus ack cycle publishes");
    let store = SharedConcurrentStore::open_existing(&path).expect("open telemetry shm");
    let mut consumer = ConcurrentRawConsumer::with_store(store);
    let batch = consumer.poll().expect("poll active plus ack records");
    assert_eq!(batch.records.len(), 2);
    assert_eq!(
        render_record(&batch.records[0].record),
        "ConditionActive source=77 seq=0 conditionId=9101 class=0 severity=900 correlation=1"
    );
    assert_eq!(
        render_record(&batch.records[1].record),
        "ConditionAcknowledged source=77 seq=1 conditionId=9101 correlation=1 ackBy=operator-a"
    );

    runtime.execute_cycle().expect("confirm cycle publishes");
    let batch = consumer.poll().expect("poll confirm record");
    assert_eq!(batch.records.len(), 1);
    assert_eq!(
        render_record(&batch.records[0].record),
        "ConditionConfirmed source=77 seq=2 conditionId=9101 correlation=1 ackBy=operator-a"
    );

    runtime.execute_cycle().expect("shelve cycle publishes");
    let batch = consumer.poll().expect("poll shelve record");
    assert_eq!(batch.records.len(), 1);
    assert_eq!(
        render_record(&batch.records[0].record),
        "ConditionShelved source=77 seq=3 conditionId=9101 correlation=1 ackBy=operator-a shelveSecs=300"
    );

    runtime.execute_cycle().expect("unshelve cycle publishes");
    let batch = consumer.poll().expect("poll unshelve record");
    assert_eq!(batch.records.len(), 1);
    assert_eq!(
        render_record(&batch.records[0].record),
        "ConditionUnshelved source=77 seq=4 conditionId=9101 correlation=1"
    );

    runtime.execute_cycle().expect("suppress cycle publishes");
    let batch = consumer.poll().expect("poll suppress record");
    assert_eq!(batch.records.len(), 1);
    assert_eq!(
        render_record(&batch.records[0].record),
        "ConditionSuppressed source=77 seq=5 conditionId=9101 reason=maintenance"
    );
    assert!(slot(&batch.records[0].record, KEY_CORRELATION_ID).is_none());

    runtime.execute_cycle().expect("unsuppress cycle publishes");
    let batch = consumer.poll().expect("poll unsuppress record");
    assert_eq!(batch.records.len(), 1);
    assert_eq!(
        render_record(&batch.records[0].record),
        "ConditionUnsuppressed source=77 seq=6 conditionId=9101"
    );
    assert!(slot(&batch.records[0].record, KEY_CORRELATION_ID).is_none());

    runtime.execute_cycle().expect("OOS cycle publishes");
    let batch = consumer.poll().expect("poll OOS record");
    assert_eq!(batch.records.len(), 1);
    assert_eq!(
        render_record(&batch.records[0].record),
        "ConditionOutOfService source=77 seq=7 conditionId=9101 ackBy=operator-a"
    );
    assert!(slot(&batch.records[0].record, KEY_CORRELATION_ID).is_none());

    runtime.execute_cycle().expect("in-service cycle publishes");
    let batch = consumer.poll().expect("poll in-service record");
    assert_eq!(batch.records.len(), 1);
    assert_eq!(
        render_record(&batch.records[0].record),
        "ConditionInService source=77 seq=8 conditionId=9101"
    );
    assert!(slot(&batch.records[0].record, KEY_CORRELATION_ID).is_none());

    runtime.execute_cycle().expect("reset cycle publishes");
    let batch = consumer.poll().expect("poll reset record");
    assert_eq!(batch.records.len(), 1);
    assert_eq!(
        render_record(&batch.records[0].record),
        "ConditionReset source=77 seq=9 conditionId=9101 correlation=1 ackBy=operator-a"
    );

    let definition = openot_authoring::definition_json_from_sources(&[SourceFile::with_path(
        "main.st", program,
    )])
    .expect("definition");
    assert_eq!(definition["conditions"][0]["conditionId"], 9101);
    assert_eq!(definition["sources"][0]["sourceId"], 77);

    drop(std::fs::remove_file(path));
}

#[test]
fn openot_telemetry_authoring_condition_suppress_while_inactive_emits() {
    let path = temp_shm_path("authoring-condition-suppress-inactive");
    let program = r#"
PROGRAM Main
VAR
    ReasonText : STRING[32] := 'maintenance';
    HighPhAlarm : BOOL {attribute 'oot' := 'alarm', 'sourceid' := '77', 'conditionid' := '9101'};
    SuppressHighPh : BOOL {attribute 'oot' := 'condition', 'of' := HighPhAlarm, 'event' := 'suppress', 'reason' := ReasonText};
END_VAR

SuppressHighPh := TRUE;
END_PROGRAM
"#;
    let mut runtime = build_st_runtime(program);
    runtime
        .configure_openot_telemetry(
            &st_fb_telemetry_config_for(path.clone(), 4096, "Main.OotProducer"),
            None,
        )
        .expect("configure OpenOT ST FB telemetry");

    runtime
        .execute_cycle()
        .expect("inactive suppress cycle publishes");
    let store = SharedConcurrentStore::open_existing(&path).expect("open telemetry shm");
    let mut consumer = ConcurrentRawConsumer::with_store(store);
    let batch = consumer.poll().expect("poll inactive suppress record");
    assert_eq!(batch.records.len(), 1);
    assert_eq!(
        render_record(&batch.records[0].record),
        "ConditionSuppressed source=77 seq=0 conditionId=9101 reason=maintenance"
    );
    assert!(slot(&batch.records[0].record, KEY_CORRELATION_ID).is_none());

    drop(std::fs::remove_file(path));
}

#[test]
fn openot_telemetry_authoring_condition_oos_while_inactive_emits() {
    let path = temp_shm_path("authoring-condition-oos-inactive");
    let program = r#"
PROGRAM Main
VAR
    OperatorName : STRING[32] := 'operator-a';
    HighPhAlarm : BOOL {attribute 'oot' := 'alarm', 'sourceid' := '77', 'conditionid' := '9101'};
    OosHighPh : BOOL {attribute 'oot' := 'condition', 'of' := HighPhAlarm, 'event' := 'out-of-service', 'by' := OperatorName};
END_VAR

OosHighPh := TRUE;
END_PROGRAM
"#;
    let mut runtime = build_st_runtime(program);
    runtime
        .configure_openot_telemetry(
            &st_fb_telemetry_config_for(path.clone(), 4096, "Main.OotProducer"),
            None,
        )
        .expect("configure OpenOT ST FB telemetry");

    runtime
        .execute_cycle()
        .expect("inactive OOS cycle publishes");
    let store = SharedConcurrentStore::open_existing(&path).expect("open telemetry shm");
    let mut consumer = ConcurrentRawConsumer::with_store(store);
    let batch = consumer.poll().expect("poll inactive OOS record");
    assert_eq!(batch.records.len(), 1);
    assert_eq!(
        render_record(&batch.records[0].record),
        "ConditionOutOfService source=77 seq=0 conditionId=9101 ackBy=operator-a"
    );
    assert!(slot(&batch.records[0].record, KEY_CORRELATION_ID).is_none());

    drop(std::fs::remove_file(path));
}

#[test]
fn openot_telemetry_authoring_condition_unsuppress_and_in_service_while_inactive_emit() {
    let path = temp_shm_path("authoring-condition-unsuppress-inservice-inactive");
    let program = r#"
PROGRAM Main
VAR
    Phase : UINT;
    HighPhAlarm : BOOL {attribute 'oot' := 'alarm', 'sourceid' := '77', 'conditionid' := '9101'};
    UnsuppressHighPh : BOOL {attribute 'oot' := 'condition', 'of' := HighPhAlarm, 'event' := 'unsuppress'};
    InServiceHighPh : BOOL {attribute 'oot' := 'condition', 'of' := HighPhAlarm, 'event' := 'in-service'};
END_VAR

CASE Phase OF
    UINT#0:
        UnsuppressHighPh := TRUE;
    UINT#1:
        InServiceHighPh := TRUE;
END_CASE;
Phase := Phase + UINT#1;
END_PROGRAM
"#;
    let mut runtime = build_st_runtime(program);
    runtime
        .configure_openot_telemetry(
            &st_fb_telemetry_config_for(path.clone(), 4096, "Main.OotProducer"),
            None,
        )
        .expect("configure OpenOT ST FB telemetry");

    runtime
        .execute_cycle()
        .expect("inactive unsuppress cycle publishes");
    let store = SharedConcurrentStore::open_existing(&path).expect("open telemetry shm");
    let mut consumer = ConcurrentRawConsumer::with_store(store);
    let batch = consumer.poll().expect("poll inactive unsuppress record");
    assert_eq!(batch.records.len(), 1);
    assert_eq!(
        render_record(&batch.records[0].record),
        "ConditionUnsuppressed source=77 seq=0 conditionId=9101"
    );
    assert!(slot(&batch.records[0].record, KEY_CORRELATION_ID).is_none());

    runtime
        .execute_cycle()
        .expect("inactive in-service cycle publishes");
    let batch = consumer.poll().expect("poll inactive in-service record");
    assert_eq!(batch.records.len(), 1);
    assert_eq!(
        render_record(&batch.records[0].record),
        "ConditionInService source=77 seq=1 conditionId=9101"
    );
    assert!(slot(&batch.records[0].record, KEY_CORRELATION_ID).is_none());

    drop(std::fs::remove_file(path));
}

#[test]
fn openot_telemetry_authoring_condition_ack_without_activation_fails_closed() {
    let path = temp_shm_path("authoring-condition-ack-inactive");
    let program = r#"
PROGRAM Main
VAR
    OperatorName : STRING[32] := 'operator-a';
    HighPhAlarm : BOOL {attribute 'oot' := 'alarm', 'sourceid' := '77', 'conditionid' := '9101'};
    AckHighPh : BOOL {attribute 'oot' := 'condition', 'of' := HighPhAlarm, 'event' := 'acknowledge', 'by' := OperatorName};
END_VAR

AckHighPh := TRUE;
END_PROGRAM
"#;
    let mut runtime = build_st_runtime(program);
    runtime
        .configure_openot_telemetry(
            &st_fb_telemetry_config_for(path.clone(), 4096, "Main.OotProducer"),
            None,
        )
        .expect("configure OpenOT ST FB telemetry");

    let err = runtime
        .execute_cycle()
        .expect_err("inactive ack must fail closed");
    let err = format!("{err:?}");
    assert!(err.contains("dropped 1 OpenOT lifecycle command"), "{err}");
    assert!(err.contains("LastLifecycleError 1"), "{err}");

    let store = SharedConcurrentStore::open_existing(&path).expect("open telemetry shm");
    let mut consumer = ConcurrentRawConsumer::with_store(store);
    let batch = consumer.poll().expect("poll after inactive ack");
    assert!(batch.records.is_empty(), "{batch:#?}");

    drop(std::fs::remove_file(path));
}

#[test]
fn openot_telemetry_authoring_condition_shelve_without_activation_fails_closed() {
    let path = temp_shm_path("authoring-condition-shelve-inactive");
    let program = r#"
PROGRAM Main
VAR
    OperatorName : STRING[32] := 'operator-a';
    ShelveSecs : UDINT := UDINT#300;
    HighPhAlarm : BOOL {attribute 'oot' := 'alarm', 'sourceid' := '77', 'conditionid' := '9101'};
    ShelveHighPh : BOOL {attribute 'oot' := 'condition', 'of' := HighPhAlarm, 'event' := 'shelve', 'by' := OperatorName, 'seconds' := ShelveSecs};
END_VAR

ShelveHighPh := TRUE;
END_PROGRAM
"#;
    let mut runtime = build_st_runtime(program);
    runtime
        .configure_openot_telemetry(
            &st_fb_telemetry_config_for(path.clone(), 4096, "Main.OotProducer"),
            None,
        )
        .expect("configure OpenOT ST FB telemetry");

    let err = runtime
        .execute_cycle()
        .expect_err("inactive shelve must fail closed");
    let err = format!("{err:?}");
    assert!(err.contains("dropped 1 OpenOT lifecycle command"), "{err}");
    assert!(err.contains("LastLifecycleError 1"), "{err}");

    let store = SharedConcurrentStore::open_existing(&path).expect("open telemetry shm");
    let mut consumer = ConcurrentRawConsumer::with_store(store);
    let batch = consumer.poll().expect("poll after inactive shelve");
    assert!(batch.records.is_empty(), "{batch:#?}");

    drop(std::fs::remove_file(path));
}

#[test]
fn openot_telemetry_authoring_condition_confirm_without_activation_fails_closed() {
    assert_inactive_activation_lifecycle_fails_closed("confirm", true);
}

#[test]
fn openot_telemetry_authoring_condition_unshelve_without_activation_fails_closed() {
    assert_inactive_activation_lifecycle_fails_closed("unshelve", false);
}

#[test]
fn openot_telemetry_authoring_condition_reset_without_activation_fails_closed() {
    assert_inactive_activation_lifecycle_fails_closed("reset", true);
}

fn assert_inactive_activation_lifecycle_fails_closed(event: &str, has_by: bool) {
    let path = temp_shm_path(&format!("authoring-condition-{event}-inactive"));
    let operator_decl = if has_by {
        "    OperatorName : STRING[32] := 'operator-a';\n"
    } else {
        ""
    };
    let by_attr = if has_by { ", 'by' := OperatorName" } else { "" };
    let program = format!(
        "PROGRAM Main\nVAR\n{operator_decl}    HighPhAlarm : BOOL {{attribute 'oot' := 'alarm', 'sourceid' := '77', 'conditionid' := '9101'}};\n    CommandHighPh : BOOL {{attribute 'oot' := 'condition', 'of' := HighPhAlarm, 'event' := '{event}'{by_attr}}};\nEND_VAR\n\nCommandHighPh := TRUE;\nEND_PROGRAM\n"
    );
    let mut runtime = build_st_runtime(&program);
    runtime
        .configure_openot_telemetry(
            &st_fb_telemetry_config_for(path.clone(), 4096, "Main.OotProducer"),
            None,
        )
        .expect("configure OpenOT ST FB telemetry");

    let err = runtime
        .execute_cycle()
        .expect_err("inactive activation-scoped lifecycle event must fail closed");
    let err = format!("{err:?}");
    assert!(err.contains("dropped 1 OpenOT lifecycle command"), "{err}");
    assert!(err.contains("LastLifecycleError 1"), "{err}");

    let store = SharedConcurrentStore::open_existing(&path).expect("open telemetry shm");
    let mut consumer = ConcurrentRawConsumer::with_store(store);
    let batch = consumer.poll().expect("poll after inactive lifecycle");
    assert!(batch.records.is_empty(), "{batch:#?}");

    drop(std::fs::remove_file(path));
}

#[test]
fn openot_telemetry_authoring_sampling_policy_round_trips_definition() {
    let program = r#"
PROGRAM Main
VAR
    Pressure : REAL {attribute 'oot' := 'value', 'sampling' := 'periodic', 'interval' := '250'};
    Flow : REAL {attribute 'oot' := 'value', 'sampling' := 'hysteresis', 'deadband' := '1.5'};
END_VAR
END_PROGRAM
"#;
    let definition = openot_authoring::definition_json_from_sources(&[SourceFile::with_path(
        "sampling.st",
        program,
    )])
    .expect("sampling definition");
    let parsed: DefinitionFile =
        serde_json::from_value(definition).expect("definition parses through open-ot-definition");
    assert_eq!(
        parsed.values[0].sampling_policy.as_deref(),
        Some("periodic:250")
    );
    assert_eq!(
        parsed.values[1].sampling_policy.as_deref(),
        Some("hysteresis")
    );
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct OverflowSummary {
    retained_records: usize,
    lapped_batches: u64,
    lost_count: u64,
    source_delivered: u64,
    source_lost: u64,
    loss_events: Vec<LossEvent>,
}

fn run_authoring_showcase_overflow() -> OverflowSummary {
    let path = temp_shm_path("authoring-showcase-overflow");
    let program = reactor_program();
    let mut runtime = build_st_runtime(&program);
    runtime
        .configure_openot_telemetry(
            &st_fb_telemetry_config_for(path.clone(), 180, "Main.OotProducer"),
            None,
        )
        .expect("configure small OpenOT ST FB telemetry");
    enable_host_source_time(&mut runtime);

    for _ in 0..8 {
        set_host_source_time(&mut runtime, unix_epoch_ns());
        runtime
            .execute_cycle()
            .expect("showcase overflow cycle publishes");
    }

    let store = SharedConcurrentStore::open_existing(&path).expect("open overflow telemetry shm");
    let lost_count = store.load_lost_acquire();
    let mut consumer = ConcurrentRawConsumer::with_store(store);
    let batch = consumer.poll().expect("poll overflow telemetry");
    let mut accounting = LossAccountingConsumer::new();
    accounting.account_batch(&batch);
    let source_delivered = accounting.delivered(1);
    let source_lost = accounting.lost(1);
    let loss_events = accounting.loss_events();

    assert!(batch.lapped, "small showcase ring must force a lapped read");
    assert!(lost_count > 0, "small showcase ring must evict records");
    assert!(
        !loss_events.is_empty(),
        "small showcase ring should produce reconciled loss intervals"
    );
    assert_eq!(
        source_delivered + source_lost,
        15,
        "sequence gaps should reconcile the source stream"
    );

    drop(std::fs::remove_file(path));

    OverflowSummary {
        retained_records: batch.records.len(),
        lapped_batches: consumer.lapped_batches(),
        lost_count,
        source_delivered,
        source_lost,
        loss_events,
    }
}

fn render_audit_log(records: &[ReadRecord]) -> Vec<String> {
    records
        .iter()
        .map(|read| render_record_line(&read.record))
        .collect()
}

fn render_record_line(record: &Record) -> String {
    format!(
        "{}  {}",
        format_source_time_utc(record.source_time),
        render_record(record)
    )
}

fn render_record(record: &Record) -> String {
    match record.event_type_id {
        EVENT_MESSAGE => {
            let args = record
                .slots
                .iter()
                .filter(|slot| slot.key == KEY_ARG)
                .map(render_value_slot)
                .collect::<Vec<_>>();
            let severity = slot(record, KEY_SEVERITY)
                .map(|_| format!(" severity={}", required_uint(record, KEY_SEVERITY)))
                .unwrap_or_default();
            let args = if args.is_empty() {
                String::new()
            } else {
                format!(" args=[{}]", args.join(","))
            };
            format!(
                "Message source={} seq={} templateId={}{}{}",
                record.source_id,
                record.seq,
                required_udint(record, KEY_MESSAGE_TEMPLATE_ID),
                severity,
                args
            )
        }
        EVENT_LOGGER_STOPPED => {
            format!(
                "LoggerStopped source={} seq={}",
                record.source_id, record.seq
            )
        }
        EVENT_SOURCE_HIGH_WATER => format!(
            "SourceHighWater source={} seq={} produced={}",
            record.source_id,
            record.seq,
            required_ulint(record, KEY_SOURCE_HIGH_WATER)
        ),
        EVENT_STATE_TRANSITION => format!(
            "StateTransition source={} seq={} machine={} category={} previous={} new={}",
            record.source_id,
            record.seq,
            required_udint(record, KEY_STATE_MACHINE_ID),
            required_uint(record, KEY_CATEGORY),
            required_uint(record, KEY_PREVIOUS_STATE),
            required_uint(record, KEY_NEW_STATE)
        ),
        EVENT_CONDITION_ACTIVE => render_condition(record, "ConditionActive"),
        EVENT_CONDITION_CLEARED => render_condition(record, "ConditionCleared"),
        EVENT_CONDITION_ACKNOWLEDGED => {
            render_condition_lifecycle_ack(record, "ConditionAcknowledged")
        }
        EVENT_CONDITION_CONFIRMED => render_condition_lifecycle_ack(record, "ConditionConfirmed"),
        EVENT_CONDITION_SHELVED => render_condition_lifecycle_shelve(record),
        EVENT_CONDITION_UNSHELVED => {
            render_condition_lifecycle_activation(record, "ConditionUnshelved")
        }
        EVENT_CONDITION_SUPPRESSED => render_condition_lifecycle_suppress(record),
        EVENT_CONDITION_UNSUPPRESSED => {
            render_condition_lifecycle_condition(record, "ConditionUnsuppressed")
        }
        EVENT_CONDITION_OUT_OF_SERVICE => render_condition_lifecycle_oos(record),
        EVENT_CONDITION_IN_SERVICE => {
            render_condition_lifecycle_condition(record, "ConditionInService")
        }
        EVENT_CONDITION_RESET => render_condition_lifecycle_ack(record, "ConditionReset"),
        EVENT_VALUE_CHANGED => render_value_changed(record),
        other => format!(
            "Event 0x{other:08X} source={} seq={}",
            record.source_id, record.seq
        ),
    }
}

fn render_condition(record: &Record, name: &str) -> String {
    let severity = slot(record, KEY_SEVERITY)
        .map(|_| format!(" severity={}", required_uint(record, KEY_SEVERITY)))
        .unwrap_or_default();
    let correlation = slot(record, KEY_CORRELATION_ID)
        .map(|_| {
            format!(
                " correlation={}",
                required_udint(record, KEY_CORRELATION_ID)
            )
        })
        .unwrap_or_default();
    let causes = record
        .slots
        .iter()
        .filter(|slot| slot.key == KEY_CAUSE_OPERAND)
        .map(|slot| {
            let payload: [u8; 4] = slot
                .payload
                .as_slice()
                .try_into()
                .expect("cause operand width");
            u32::from_le_bytes(payload).to_string()
        })
        .collect::<Vec<_>>();
    let causes = if causes.is_empty() {
        String::new()
    } else {
        format!(" causes=[{}]", causes.join(","))
    };
    format!(
        "{name} source={} seq={} conditionId={} class={}{}{}{}",
        record.source_id,
        record.seq,
        required_udint(record, KEY_CONDITION_ID),
        required_uint(record, KEY_CONDITION_CLASS),
        severity,
        correlation,
        causes
    )
}

fn render_condition_lifecycle_ack(record: &Record, name: &str) -> String {
    let ack_by = slot(record, KEY_ACK_BY)
        .map(|_| format!(" ackBy={}", required_string(record, KEY_ACK_BY)))
        .unwrap_or_default();
    format!(
        "{name} source={} seq={} conditionId={} correlation={}{}",
        record.source_id,
        record.seq,
        required_udint(record, KEY_CONDITION_ID),
        required_udint(record, KEY_CORRELATION_ID),
        ack_by
    )
}

fn render_condition_lifecycle_activation(record: &Record, name: &str) -> String {
    format!(
        "{name} source={} seq={} conditionId={} correlation={}",
        record.source_id,
        record.seq,
        required_udint(record, KEY_CONDITION_ID),
        required_udint(record, KEY_CORRELATION_ID)
    )
}

fn render_condition_lifecycle_shelve(record: &Record) -> String {
    let ack_by = slot(record, KEY_ACK_BY)
        .map(|_| format!(" ackBy={}", required_string(record, KEY_ACK_BY)))
        .unwrap_or_default();
    let shelve_secs = slot(record, KEY_SHELVE_SECS)
        .map(|_| format!(" shelveSecs={}", required_udint(record, KEY_SHELVE_SECS)))
        .unwrap_or_default();
    format!(
        "ConditionShelved source={} seq={} conditionId={} correlation={}{}{}",
        record.source_id,
        record.seq,
        required_udint(record, KEY_CONDITION_ID),
        required_udint(record, KEY_CORRELATION_ID),
        ack_by,
        shelve_secs
    )
}

fn render_condition_lifecycle_suppress(record: &Record) -> String {
    let reason = slot(record, KEY_REASON)
        .map(|_| format!(" reason={}", required_string(record, KEY_REASON)))
        .unwrap_or_default();
    format!(
        "ConditionSuppressed source={} seq={} conditionId={}{}",
        record.source_id,
        record.seq,
        required_udint(record, KEY_CONDITION_ID),
        reason
    )
}

fn render_condition_lifecycle_oos(record: &Record) -> String {
    let ack_by = slot(record, KEY_ACK_BY)
        .map(|_| format!(" ackBy={}", required_string(record, KEY_ACK_BY)))
        .unwrap_or_default();
    format!(
        "ConditionOutOfService source={} seq={} conditionId={}{}",
        record.source_id,
        record.seq,
        required_udint(record, KEY_CONDITION_ID),
        ack_by
    )
}

fn render_condition_lifecycle_condition(record: &Record, name: &str) -> String {
    format!(
        "{name} source={} seq={} conditionId={}",
        record.source_id,
        record.seq,
        required_udint(record, KEY_CONDITION_ID)
    )
}

fn format_source_time_utc(source_time_ns: u64) -> String {
    let datetime = OffsetDateTime::from_unix_timestamp_nanos(i128::from(source_time_ns))
        .expect("sourceTimeNs must be a Unix timestamp");
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}.{:03}Z",
        datetime.year(),
        u8::from(datetime.month()),
        datetime.day(),
        datetime.hour(),
        datetime.minute(),
        datetime.second(),
        datetime.nanosecond() / 1_000_000
    )
}

fn assert_readable_utc_timestamp(timestamp: &str) {
    assert_eq!(timestamp.len(), "2026-06-06T18:00:00.000Z".len());
    assert_eq!(&timestamp[4..5], "-");
    assert_eq!(&timestamp[7..8], "-");
    assert_eq!(&timestamp[10..11], "T");
    assert_eq!(&timestamp[13..14], ":");
    assert_eq!(&timestamp[16..17], ":");
    assert_eq!(&timestamp[19..20], ".");
    assert_eq!(&timestamp[23..24], "Z");
}

fn unix_epoch_ns() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock must be after Unix epoch")
        .as_nanos()
        .try_into()
        .expect("Unix nanoseconds fit u64")
}

fn enable_host_source_time(runtime: &mut Runtime) {
    write_main_program_var(
        runtime,
        openot_authoring::GENERATED_USE_SOURCE_TIME_NAME,
        Value::Bool(true),
    );
}

fn set_host_source_time(runtime: &mut Runtime, source_time_ns: u64) {
    write_main_program_var(
        runtime,
        openot_authoring::GENERATED_SOURCE_TIME_NAME,
        Value::ULInt(source_time_ns),
    );
}

fn write_main_program_var(runtime: &mut Runtime, name: &str, value: Value) {
    let main_id = main_instance_id(runtime);
    let reference = runtime
        .storage()
        .ref_for_instance_recursive(main_id, name)
        .unwrap_or_else(|| panic!("missing generated Main.{name}"));
    assert!(
        runtime.storage_mut().write_by_ref(reference, value),
        "write generated Main.{name}"
    );
}

fn main_instance_id(runtime: &Runtime) -> InstanceId {
    match runtime.storage().get_global("Main") {
        Some(Value::Instance(id)) => *id,
        other => panic!("expected Main program instance, got {other:?}"),
    }
}

fn render_value_changed(record: &Record) -> String {
    let value_id = required_udint(record, KEY_VALUE_ID);
    let new_value = required_value(record, KEY_NEW_VALUE);
    let previous = slot(record, KEY_PREVIOUS_VALUE)
        .map(render_value_slot)
        .map(|value| format!(" previous={value}"))
        .unwrap_or_default();
    format!(
        "ValueChanged source={} seq={} valueId={}{} new={}",
        record.source_id, record.seq, value_id, previous, new_value
    )
}

fn required_value(record: &Record, key: u16) -> String {
    render_value_slot(slot(record, key).unwrap_or_else(|| panic!("missing slot 0x{key:04X}")))
}

fn render_value_slot(slot: &Slot) -> String {
    match slot.ty {
        TY_BOOL => format!("BOOL({})", slot.payload.first().copied().unwrap_or(0) != 0),
        TY_SINT => format!("SINT({})", slot.payload[0] as i8),
        TY_USINT => format!("USINT({})", slot.payload[0]),
        TY_UINT => format!(
            "UINT({})",
            u16::from_le_bytes(
                slot.payload
                    .as_slice()
                    .try_into()
                    .expect("UINT payload width")
            )
        ),
        TY_INT => format!(
            "INT({})",
            i16::from_le_bytes(
                slot.payload
                    .as_slice()
                    .try_into()
                    .expect("INT payload width")
            )
        ),
        TY_UDINT => format!(
            "UDINT({})",
            u32::from_le_bytes(
                slot.payload
                    .as_slice()
                    .try_into()
                    .expect("UDINT payload width")
            )
        ),
        TY_REAL => format!(
            "REAL({})",
            f32::from_le_bytes(
                slot.payload
                    .as_slice()
                    .try_into()
                    .expect("REAL payload width")
            )
        ),
        TY_DINT => format!(
            "DINT({})",
            i32::from_le_bytes(
                slot.payload
                    .as_slice()
                    .try_into()
                    .expect("DINT payload width")
            )
        ),
        TY_ULINT => format!(
            "ULINT({})",
            u64::from_le_bytes(
                slot.payload
                    .as_slice()
                    .try_into()
                    .expect("ULINT payload width")
            )
        ),
        TY_LINT => format!(
            "LINT({})",
            i64::from_le_bytes(
                slot.payload
                    .as_slice()
                    .try_into()
                    .expect("LINT payload width")
            )
        ),
        TY_LREAL => format!(
            "LREAL({})",
            f64::from_le_bytes(
                slot.payload
                    .as_slice()
                    .try_into()
                    .expect("LREAL payload width")
            )
        ),
        TY_STRING => format!(
            "STRING({})",
            std::str::from_utf8(&slot.payload).expect("STRING payload utf-8")
        ),
        other => panic!("unsupported value payload type {other}"),
    }
}

fn required_udint(record: &Record, key: u16) -> u32 {
    let slot = slot(record, key).unwrap_or_else(|| panic!("missing slot 0x{key:04X}"));
    assert_eq!(slot.ty, TY_UDINT);
    u32::from_le_bytes(
        slot.payload
            .as_slice()
            .try_into()
            .expect("UDINT payload width"),
    )
}

fn required_uint(record: &Record, key: u16) -> u16 {
    let slot = slot(record, key).unwrap_or_else(|| panic!("missing slot 0x{key:04X}"));
    assert_eq!(slot.ty, TY_UINT);
    u16::from_le_bytes(
        slot.payload
            .as_slice()
            .try_into()
            .expect("UINT payload width"),
    )
}

fn required_ulint(record: &Record, key: u16) -> u64 {
    let slot = slot(record, key).unwrap_or_else(|| panic!("missing slot 0x{key:04X}"));
    assert_eq!(slot.ty, TY_ULINT);
    u64::from_le_bytes(
        slot.payload
            .as_slice()
            .try_into()
            .expect("ULINT payload width"),
    )
}

fn required_string(record: &Record, key: u16) -> String {
    let slot = slot(record, key).unwrap_or_else(|| panic!("missing slot 0x{key:04X}"));
    assert_eq!(slot.ty, TY_STRING);
    std::str::from_utf8(&slot.payload)
        .expect("STRING payload utf-8")
        .to_string()
}

fn slot(record: &Record, key: u16) -> Option<&Slot> {
    record.slots.iter().find(|slot| slot.key == key)
}

fn write_authoring_showcase_artifacts(
    records: &[ReadRecord],
    audit_log: &[String],
    overflow: &OverflowSummary,
    reactor_source: &str,
) -> std::io::Result<()> {
    let reactor_dir = openot_ref_dir().join("examples/reactor");
    std::fs::create_dir_all(&reactor_dir)?;

    let mut text = String::new();
    text.push_str("OpenOT Reactor Batch Log\n\n");
    for line in audit_log {
        text.push_str(line);
        text.push('\n');
    }
    text.push_str("\nForced overflow\n");
    text.push_str(&format!(
        "retained_records: {}\nlapped_batches: {}\nshm_lost_count: {}\nsource_1_delivered: {}\nsource_1_lost: {}\nsource_1_reconciled: {}\n",
        overflow.retained_records,
        overflow.lapped_batches,
        overflow.lost_count,
        overflow.source_delivered,
        overflow.source_lost,
        overflow.source_delivered + overflow.source_lost
    ));
    std::fs::write(reactor_dir.join("batch-log.txt"), &text)?;

    let definition = reactor_definition(reactor_source);
    let docs = batch_log_documents(records, overflow, &definition);
    assert_resolved_reactor_documents(&docs);
    let json = serde_json::to_string_pretty(&docs).expect("serialize reactor batch log");
    std::fs::write(reactor_dir.join("batch-log.json"), format!("{json}\n"))?;

    let definition_json =
        serde_json::to_string_pretty(&definition).expect("serialize reactor definition");
    std::fs::write(
        reactor_dir.join("openot-definition.json"),
        format!("{definition_json}\n"),
    )?;

    write_reactor_readme(&reactor_dir, &text)?;

    Ok(())
}

fn reactor_definition(reactor_source: &str) -> DefinitionFile {
    let definition = openot_authoring::definition_json_from_sources(&[SourceFile::with_path(
        "examples/reactor/Reactor.st",
        reactor_source,
    )])
    .expect("generate OpenOT reactor definition");
    serde_json::from_value(definition).expect("parse generated OpenOT reactor definition")
}

fn write_reactor_readme(reactor_dir: &std::path::Path, batch_log: &str) -> std::io::Result<()> {
    let readme = format!(
        r#"# OpenOT Reactor Example

This example is the attribute-authored OpenOT path:

- `Reactor.st` contains normal Structured Text and `{{attribute 'oot' := ...}}` declarations.
- The process state is declared as `E_ReactorStep`; the generated definition file carries that enum set.
- The truST compiler lowers those declarations to a hidden `OotProducer : OPENOT_Producer`.
- The existing ST-FB telemetry path publishes `Main.OotProducer` records to shared memory.
- The runtime supplies the source clock: hosted build -> host Unix clock; real hardware -> RTC/NTP.
- `batch-log.json`, `batch-log.txt`, and `openot-definition.json` are generated from a truST -> shm -> consumer run.

Run the proof from the sibling `trust-platform` checkout:

```sh
cargo test -p trust-runtime --test openot_telemetry openot_telemetry_authoring_showcase_renders_typed_audit_log -- --nocapture
```

The authoring surface in `Reactor.st` is declaration-only. It must not contain `Op :=`, `Execute :=`, or `OOT_Log`.

Rendered batch log:

```text
{batch_log}```
"#
    );
    std::fs::write(reactor_dir.join("README.md"), readme)
}

fn batch_log_documents(
    records: &[ReadRecord],
    overflow: &OverflowSummary,
    definition: &DefinitionFile,
) -> serde_json::Value {
    let hash = compute_content_hash(definition).expect("hash reactor definition");
    let snapshot = ControlBlockSnapshot {
        version: 2,
        caps: 0,
        buffer_id: DEFAULT_BUFFER_ID,
        buffer_bytes: 4096,
        head_abs: records.last().map_or(0, |record| record.end_abs),
        oldest_abs: records.first().map_or(0, |record| record.start_abs),
        lost_count: overflow.lost_count,
        run_id: 1,
        epoch_id: 1,
        epoch_first_abs: 0,
        definition_hash: hash.carriage_hash,
        prev_definition_hash: [0; 8],
    };
    let definition_set = DefinitionSet::current(definition);
    let mut docs = records
        .iter()
        .map(|read| {
            let resolution =
                resolve_record(&read.record, read.start_abs, &snapshot, &definition_set);
            let context = RecordDocumentContext::new(
                DEFAULT_BUFFER_ID,
                read.record.source_time,
                hash.carriage_hash,
                read.record.flags,
            )
            .with_semantic_version(definition.header.semantic_version.clone());
            let document = document_from_resolution(&resolution, &context);
            serde_json::from_str(&to_json(&document).expect("serialize event document"))
                .expect("parse event document")
        })
        .collect::<Vec<serde_json::Value>>();

    for event in &overflow.loss_events {
        let context = LossDocumentContext::new(
            unix_epoch_ns(),
            1,
            EpochRelation::Current,
            hash.carriage_hash,
        )
        .with_semantic_version(definition.header.semantic_version.clone());
        let document = document_from_loss(event, &context);
        docs.push(
            serde_json::from_str(&to_json(&document).expect("serialize loss document"))
                .expect("parse loss document"),
        );
    }
    docs.push(serde_json::json!({
        "kind": "lossReconciliation",
        "retainedRecords": overflow.retained_records,
        "lappedBatches": overflow.lapped_batches,
        "shmLostCount": overflow.lost_count,
        "sourceId": 1,
        "sourceDelivered": overflow.source_delivered,
        "sourceLost": overflow.source_lost,
        "sourceReconciled": overflow.source_delivered + overflow.source_lost
    }));
    serde_json::Value::Array(docs)
}

fn assert_resolved_reactor_documents(docs: &serde_json::Value) {
    let text = serde_json::to_string(docs).expect("serialize resolved docs");
    assert!(
        text.contains(r#""name":"value","type":"ValueRef","value":"Level""#),
        "{text}"
    );
    assert!(text.contains(r#""unit":"L""#), "{text}");
    assert!(
        text.contains(r#""name":"stateMachine","type":"StateMachineRef","value":"Step""#),
        "{text}"
    );
    assert!(
        text.contains(r#""name":"newState","type":"StateRef","value":"Filling""#),
        "{text}"
    );
    assert!(
        text.contains(r#""name":"condition","type":"ConditionRef","value":"HighPhAlarm""#),
        "{text}"
    );
    assert!(
        text.contains(
            r#""name":"messageTemplate","type":"MessageTemplateRef","value":"batch started count""#
        ),
        "{text}"
    );
    assert!(
        text.contains(r#""name":"causeOperand","type":"CauseOperandRef","value":"Level""#),
        "{text}"
    );
    assert!(
        text.contains(r#""name":"arg","type":"DInt","value":1"#),
        "{text}"
    );
    assert!(
        text.contains(r#""name":"severity","type":"UInt","value":100"#)
            && text.contains(r#""enumLabel":"Low","key":8,"name":"severity""#),
        "{text}"
    );
    assert!(!text.contains(r#""valueId""#), "{text}");
    assert!(!text.contains(r#""stateMachineId""#), "{text}");
    assert!(!text.contains(r#""conditionId""#), "{text}");
    assert!(!text.contains(r#""messageTemplateId""#), "{text}");
}
