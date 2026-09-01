#![cfg(unix)]

use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

mod openot_support;

use open_ot_carriage::concurrent::{ConcurrentRawConsumer, ConcurrentStore};
use open_ot_carriage::consumer::LossAccountingConsumer;
use open_ot_carriage::control::ControlBlockSnapshot;
use open_ot_carriage::loss::LossEvent;
use open_ot_carriage::registry::{
    EVENT_BATCH_EVENT, EVENT_CONDITION_ACKNOWLEDGED, EVENT_CONDITION_ACTIVE,
    EVENT_CONDITION_CLEARED, EVENT_CONDITION_COMMENTED, EVENT_CONDITION_CONFIRMED,
    EVENT_CONDITION_IN_SERVICE, EVENT_CONDITION_OUT_OF_SERVICE, EVENT_CONDITION_PRIORITY_CHANGED,
    EVENT_CONDITION_RESET, EVENT_CONDITION_SHELVED, EVENT_CONDITION_SUPPRESSED,
    EVENT_CONDITION_UNSHELVED, EVENT_CONDITION_UNSUPPRESSED, EVENT_ESIGNATURE, EVENT_HEARTBEAT,
    EVENT_LOGGER_STOPPED, EVENT_MATERIAL_ADDITION, EVENT_MESSAGE, EVENT_OPERATOR_ACTION,
    EVENT_OPERATOR_LOGIN, EVENT_OPERATOR_LOGOUT, EVENT_PARAMETER_CHANGE, EVENT_RECIPE_APPROVED,
    EVENT_RECIPE_LOADED, EVENT_SECURITY_ACCESS_FAILURE, EVENT_SOURCE_HIGH_WATER,
    EVENT_STATE_TRANSITION, EVENT_VALUE_CHANGED, KEY_ACK_BY, KEY_ACTION_ID, KEY_ACTOR, KEY_ARG,
    KEY_AUTH_RESULT, KEY_BATCH_ID, KEY_CATEGORY, KEY_CAUSE_OPERAND, KEY_COMMENT,
    KEY_CONDITION_CLASS, KEY_CONDITION_ID, KEY_CONTEXT_REF, KEY_CORRELATION_ID, KEY_MATERIAL_ID,
    KEY_MESSAGE_TEMPLATE_ID, KEY_NEW_PRIORITY, KEY_NEW_STATE, KEY_NEW_VALUE, KEY_PREVIOUS_PRIORITY,
    KEY_PREVIOUS_STATE, KEY_PREVIOUS_VALUE, KEY_QUANTITY, KEY_REASON, KEY_RECIPE_ID,
    KEY_RECIPE_VERSION, KEY_ROLE, KEY_SEVERITY, KEY_SHELVE_SECS, KEY_SIGNATURE_MEANING,
    KEY_SIGNED_EVENT_SEQ, KEY_SOURCE_HIGH_WATER, KEY_STATE_MACHINE_ID, KEY_UNIT, KEY_VALUE_ID,
    KEY_WORKSTATION, SYSTEM_SOURCE_ID, TY_BOOL, TY_DINT, TY_INT, TY_LINT, TY_LREAL, TY_REAL,
    TY_SINT, TY_STRING, TY_UDINT, TY_UINT, TY_ULINT, TY_USINT,
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
    OpenOtPersistenceBackend, OpenOtPersistenceConfig, OpenOtSqlitePersistenceConfig,
    OpenOtTelemetryConfig, OpenOtTelemetryFenceMode, OpenOtTelemetrySource,
};
use trust_runtime::harness::{CompileSession, SourceFile, TestHarness};
use trust_runtime::memory::InstanceId;
use trust_runtime::openot_authoring;
#[cfg(feature = "openot-real-database-tests")]
use trust_runtime::openot_persistence::{
    DocumentSink, InfluxDb3DocumentSink, MySqlDocumentSink, PostgreSqlDocumentSink,
    SqlServerDocumentSink, TimescaleDbDocumentSink,
};
use trust_runtime::openot_persistence::{
    OpenOtDocumentSource, OpenOtPersistenceConsumer, OpenOtPersistenceService,
    OpenOtPersistenceWorker, PersistenceBatch, PersistenceCheckpoint, PersistenceError,
    SharedMemoryOpenOtSource, SqliteDocumentSink,
};
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
        producer_instances: Vec::new(),
        persistence: Default::default(),
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
        producer_instances: vec![producer_instance.into()],
        ..telemetry_config(path, capacity)
    }
}

fn st_fb_telemetry_config_for_many(
    path: PathBuf,
    capacity: usize,
    producer_instances: &[&str],
) -> OpenOtTelemetryConfig {
    OpenOtTelemetryConfig {
        source: OpenOtTelemetrySource::StFb,
        producer_instance: None,
        producer_instances: producer_instances
            .iter()
            .map(|path| (*path).into())
            .collect(),
        ..telemetry_config(path, capacity)
    }
}

fn reactor_program() -> String {
    openot_support::REACTOR_PROGRAM.to_owned()
}

fn openot_st_source_files(main_source: &str) -> Vec<SourceFile> {
    let mut sources = openot_support::ST_LIBRARY_SOURCES
        .iter()
        .map(|(name, source)| SourceFile::with_path(*name, *source))
        .collect::<Vec<_>>();
    sources.push(SourceFile::with_path("main.st", main_source));
    sources
}

fn openot_st_test_sources(test_name: &str, main_source: &str) -> Vec<String> {
    let mut sources = openot_support::ST_LIBRARY_SOURCES
        .iter()
        .map(|(_, source)| (*source).to_owned())
        .collect::<Vec<_>>();
    sources.push(openot_support::st_test_source(test_name).to_owned());
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

fn multi_program_authoring_messages_program() -> &'static str {
    r#"
PROGRAM First
VAR
    Scan : UINT;
    Start : BOOL {attribute 'oot' := 'message', 'template' := 'first started'};
END_VAR

CASE Scan OF
    UINT#0:
        Start := TRUE;
    UINT#1:
        Start := FALSE;
    UINT#2:
        Start := TRUE;
END_CASE;
Scan := Scan + UINT#1;
END_PROGRAM

PROGRAM Second
VAR
    Scan : UINT;
    Start : BOOL {attribute 'oot' := 'message', 'template' := 'second started'};
END_VAR

CASE Scan OF
    UINT#0:
        Start := TRUE;
    UINT#1:
        Start := FALSE;
    UINT#2:
        Start := TRUE;
END_CASE;
Scan := Scan + UINT#1;
END_PROGRAM
"#
}

fn multi_program_checkpoint_program() -> &'static str {
    r#"
PROGRAM First
VAR
    Producer : OPENOT_Producer;
    Phase : UINT;
END_VAR

CASE Phase OF
    UINT#0:
        Producer(Execute := TRUE, Op := UINT#0, SourceId := UDINT#10, Checkpoint := FALSE, MessageTemplateId := UDINT#10001);
        Producer(Execute := FALSE, Op := UINT#0, SourceId := UDINT#10, Checkpoint := FALSE);
    UINT#1:
        Producer(Execute := TRUE, Op := UINT#0, SourceId := UDINT#10, Checkpoint := TRUE);
        Producer(Execute := FALSE, Op := UINT#0, SourceId := UDINT#10, Checkpoint := FALSE);
END_CASE;
Phase := Phase + UINT#1;
END_PROGRAM

PROGRAM Second
VAR
    Producer : OPENOT_Producer;
    Phase : UINT;
END_VAR

CASE Phase OF
    UINT#0:
        Producer(Execute := TRUE, Op := UINT#0, SourceId := UDINT#20, Checkpoint := FALSE, MessageTemplateId := UDINT#10001);
        Producer(Execute := FALSE, Op := UINT#0, SourceId := UDINT#20, Checkpoint := FALSE);
    UINT#1:
        Producer(Execute := TRUE, Op := UINT#0, SourceId := UDINT#20, Checkpoint := TRUE);
        Producer(Execute := FALSE, Op := UINT#0, SourceId := UDINT#20, Checkpoint := FALSE);
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
            "OperatorActionPassed",
            "OperatorLoginPassed",
            "OperatorLogoutPassed",
            "SecurityAccessFailurePassed",
            "ESignaturePassed",
            "MismatchIndex",
        ] {
            if let Some(value) = harness
                .get_output(name)
                .or_else(|| harness.get_output(&format!("Test.{name}")))
            {
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
fn openot_st_batch_recipe_vectors_are_byte_exact() {
    run_openot_st_pou_test(
        "test_conformant_batch_recipe.st",
        "OPENOT_TestConformantBatchRecipe",
    );
}

#[test]
fn openot_st_regulated_vectors_are_byte_exact() {
    run_openot_st_pou_test(
        "test_conformant_regulated.st",
        "OPENOT_TestConformantRegulated",
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
fn openot_telemetry_publishes_multi_program_authoring_sources_to_one_ring() {
    let path = temp_shm_path("st-fb-multi-program-authoring");
    let mut runtime = build_st_runtime(multi_program_authoring_messages_program());
    runtime
        .configure_openot_telemetry(
            &st_fb_telemetry_config_for_many(
                path.clone(),
                4096,
                &["First.OotProducer", "Second.OotProducer"],
            ),
            None,
        )
        .expect("configure multi-program OpenOT ST FB telemetry");

    let store = SharedConcurrentStore::open_existing(&path).expect("open telemetry shm");
    let mut consumer = ConcurrentRawConsumer::with_store(store);
    let mut accounting = LossAccountingConsumer::new();

    runtime
        .execute_cycle()
        .expect("first scan publishes one message per program");
    let batch = consumer.poll().expect("poll first multi-program scan");
    accounting.account_batch(&batch);
    assert_eq!(batch.records.len(), 2);
    assert_eq!(consumer.rejected_records(), 0);
    assert_eq!(batch.records[0].record.event_type_id, EVENT_MESSAGE);
    assert_eq!(batch.records[0].record.source_id, 1);
    assert_eq!(batch.records[0].record.seq, 0);
    assert_eq!(
        required_udint(&batch.records[0].record, KEY_MESSAGE_TEMPLATE_ID),
        10001
    );
    assert_eq!(batch.records[1].record.event_type_id, EVENT_MESSAGE);
    assert_eq!(batch.records[1].record.source_id, 2);
    assert_eq!(batch.records[1].record.seq, 0);
    assert_eq!(
        required_udint(&batch.records[1].record, KEY_MESSAGE_TEMPLATE_ID),
        10002
    );

    runtime
        .execute_cycle()
        .expect("falling edges publish no records");
    let batch = consumer.poll().expect("poll idle multi-program scan");
    assert_eq!(batch.records.len(), 0);
    assert_eq!(consumer.rejected_records(), 0);

    runtime
        .execute_cycle()
        .expect("third scan publishes the next per-source seq");
    let batch = consumer.poll().expect("poll second multi-program emission");
    accounting.account_batch(&batch);
    assert_eq!(batch.records.len(), 2);
    assert_eq!(batch.records[0].record.source_id, 1);
    assert_eq!(batch.records[0].record.seq, 1);
    assert_eq!(batch.records[1].record.source_id, 2);
    assert_eq!(batch.records[1].record.seq, 1);

    assert_eq!(accounting.delivered(1), 2);
    assert_eq!(accounting.delivered(2), 2);
    assert_eq!(accounting.lost(1), 0);
    assert_eq!(accounting.lost(2), 0);

    drop(std::fs::remove_file(path));
}

#[test]
fn openot_database_example_emits_every_documented_authored_event_family() {
    let path = temp_shm_path("database-example-coverage");
    let program = include_str!("../../../examples/openot_multi_program/src/Main.st");
    let mut runtime = build_st_runtime(program);
    runtime
        .configure_openot_telemetry(
            &st_fb_telemetry_config_for_many(
                path.clone(),
                65_536,
                &[
                    "Filler.OotProducer",
                    "BatchControl.OotProducer",
                    "OperatorAudit.OotProducer",
                    "SignatureAudit.OotProducer",
                    "TypedValues.OotProducer",
                    "ConditionLifecycle.OotProducer",
                ],
            ),
            None,
        )
        .expect("configure database example OpenOT producers");
    let store = SharedConcurrentStore::open_existing(&path).expect("open telemetry shm");
    let mut consumer = ConcurrentRawConsumer::with_store(store);
    let mut event_types = std::collections::BTreeSet::new();
    let mut value_types = std::collections::BTreeSet::new();
    let mut condition_classes = std::collections::BTreeSet::new();

    for _ in 0..12 {
        runtime.execute_cycle().expect("database example cycle");
        let batch = consumer.poll().expect("poll database example");
        event_types.extend(batch.records.iter().map(|read| read.record.event_type_id));
        value_types.extend(
            batch
                .records
                .iter()
                .filter(|read| read.record.event_type_id == EVENT_VALUE_CHANGED)
                .filter_map(|read| slot(&read.record, KEY_NEW_VALUE).map(|value| value.ty)),
        );
        condition_classes.extend(
            batch
                .records
                .iter()
                .filter(|read| {
                    matches!(
                        read.record.event_type_id,
                        EVENT_CONDITION_ACTIVE | EVENT_CONDITION_CLEARED
                    )
                })
                .map(|read| required_uint(&read.record, KEY_CONDITION_CLASS)),
        );
    }

    for required in [
        EVENT_MESSAGE,
        EVENT_STATE_TRANSITION,
        EVENT_VALUE_CHANGED,
        EVENT_PARAMETER_CHANGE,
        EVENT_CONDITION_ACTIVE,
        EVENT_CONDITION_CLEARED,
        EVENT_CONDITION_ACKNOWLEDGED,
        EVENT_CONDITION_CONFIRMED,
        EVENT_CONDITION_SHELVED,
        EVENT_CONDITION_UNSHELVED,
        EVENT_CONDITION_SUPPRESSED,
        EVENT_CONDITION_UNSUPPRESSED,
        EVENT_CONDITION_OUT_OF_SERVICE,
        EVENT_CONDITION_IN_SERVICE,
        EVENT_CONDITION_RESET,
        EVENT_CONDITION_COMMENTED,
        EVENT_CONDITION_PRIORITY_CHANGED,
        EVENT_RECIPE_LOADED,
        EVENT_RECIPE_APPROVED,
        EVENT_MATERIAL_ADDITION,
        EVENT_BATCH_EVENT,
        EVENT_OPERATOR_ACTION,
        EVENT_OPERATOR_LOGIN,
        EVENT_OPERATOR_LOGOUT,
        EVENT_SECURITY_ACCESS_FAILURE,
        EVENT_ESIGNATURE,
    ] {
        assert!(
            event_types.contains(&required),
            "database example omitted event type 0x{required:04X}; emitted={event_types:?}"
        );
    }
    assert_eq!(
        value_types,
        [
            TY_BOOL, TY_SINT, TY_USINT, TY_INT, TY_UINT, TY_DINT, TY_UDINT, TY_LINT, TY_ULINT,
            TY_REAL, TY_LREAL, TY_STRING,
        ]
        .into_iter()
        .collect(),
        "database example must preserve every supported authored value type"
    );
    assert_eq!(
        condition_classes,
        [0, 1].into_iter().collect(),
        "database example must exercise alarm and interlock conditions"
    );
    let definition = openot_authoring::definition_json_from_sources(&[SourceFile::with_path(
        "Main.st", program,
    )])
    .expect("generate example coverage definition");
    let state_categories: std::collections::BTreeSet<_> = definition["stateMachines"]
        .as_array()
        .expect("state machines")
        .iter()
        .filter_map(|state| state["category"].as_u64())
        .collect();
    assert_eq!(state_categories, [0, 1, 2].into_iter().collect());
    let procedural_models: std::collections::BTreeSet<_> = definition["stateMachines"]
        .as_array()
        .expect("state machines")
        .iter()
        .filter_map(|state| state["proceduralModel"].as_str())
        .collect();
    assert_eq!(
        procedural_models,
        ["ISA-88", "PackML"].into_iter().collect()
    );
    let sampling_policies: std::collections::BTreeSet<_> = definition["values"]
        .as_array()
        .expect("values")
        .iter()
        .filter_map(|value| value["samplingPolicy"].as_str())
        .collect();
    assert!(sampling_policies.contains("on-change"));
    assert!(sampling_policies.contains("deadband"));
    assert!(sampling_policies.contains("periodic:250"));
    assert!(sampling_policies.contains("hysteresis"));
    assert!(definition["messageTemplates"]
        .as_array()
        .expect("message templates")
        .iter()
        .any(|message| message["argTypes"]
            .as_array()
            .is_some_and(|args| args.len() == 4)));
    std::fs::remove_file(path).ok();
}

#[test]
fn openot_database_example_coverage_manifest_names_the_reviewed_contract() {
    let manifest: serde_json::Value = serde_json::from_str(include_str!(
        "../../../examples/openot_multi_program/openot-coverage-manifest.json"
    ))
    .expect("parse canonical OpenOT coverage manifest");

    assert_eq!(
        manifest["supportedOpenOtRevision"],
        "137f0e765f085c262651f479be35298b836ac891"
    );
    assert_eq!(
        manifest["producerInstances"].as_array().map(Vec::len),
        Some(6)
    );
    assert_eq!(manifest["valueTypes"].as_array().map(Vec::len), Some(12));
    assert_eq!(
        manifest["authoredEvents"]
            .as_object()
            .map(serde_json::Map::len),
        Some(26)
    );
    assert_eq!(
        manifest["databaseProducts"].as_array().map(Vec::len),
        Some(7)
    );
    for policy in ["on-change", "deadband", "periodic:250", "hysteresis"] {
        assert!(
            manifest["samplingPolicies"].get(policy).is_some(),
            "{policy}"
        );
    }
    assert_eq!(
        manifest["runtimeSystemEvents"],
        serde_json::json!([
            "Heartbeat",
            "LoggerStarted",
            "LoggerStopped",
            "BufferCleared",
            "RecordsDropped",
            "SourceRegistered",
            "DefinitionChanged",
            "TimeSyncChanged",
            "SourceHighWater"
        ]),
        "the manifest must distinguish runtime-authored system records"
    );
    assert_eq!(
        manifest["consumerDocuments"]["Loss"]["bases"],
        serde_json::json!(["authoritative", "inferred"])
    );
    for event in manifest["runtimeSystemEvents"]
        .as_array()
        .expect("runtime system events")
    {
        let event = event.as_str().expect("system event name");
        assert!(
            manifest["runtimeSystemTriggers"][event]
                .as_str()
                .is_some_and(|trigger| !trigger.trim().is_empty()),
            "system event {event} needs a reviewed runtime or contract-fixture trigger"
        );
    }
    assert_eq!(
        manifest["consumerDocuments"]["Placeholder"]["preserves"],
        serde_json::json!(["reason", "rawSlots"])
    );
    assert_eq!(
        manifest["requiredProvenance"],
        serde_json::json!([
            "bufferId",
            "source.id",
            "source.name",
            "source.path",
            "source.hierarchy",
            "runId",
            "epoch.id",
            "epoch.relation",
            "epoch.definitionHash",
            "sourceTimeNs",
            "receiveTimeNs",
            "flags"
        ])
    );
    assert_eq!(
        manifest["coverageGate"]["retrieval"],
        "native documented query returns canonical JSON"
    );
    assert_eq!(
        manifest["coverageGate"]["cardinality"],
        "every expected identity exactly once unless explicitly idempotent"
    );
}

#[test]
fn openot_database_example_persists_real_st_documents_to_sqlite() {
    let ring_path = temp_shm_path("database-example-sqlite-ring");
    let database_root = temp_shm_path("database-example-sqlite");
    let database_path = database_root.join("trust-logging.sqlite3");
    let program = include_str!("../../../examples/openot_multi_program/src/Main.st");
    let definition_json = openot_authoring::definition_json_from_sources(&[SourceFile::with_path(
        "main.st", program,
    )])
    .expect("generate canonical example definition");
    let definition: DefinitionFile = serde_json::from_value(definition_json.clone())
        .expect("parse canonical example definition");
    let mut runtime = build_st_runtime(program);
    runtime
        .configure_openot_telemetry(
            &st_fb_telemetry_config_for_many(
                ring_path.clone(),
                65_536,
                &[
                    "Filler.OotProducer",
                    "BatchControl.OotProducer",
                    "OperatorAudit.OotProducer",
                    "SignatureAudit.OotProducer",
                    "TypedValues.OotProducer",
                    "ConditionLifecycle.OotProducer",
                ],
            ),
            None,
        )
        .expect("configure canonical example ring");
    for _ in 0..12 {
        runtime.execute_cycle().expect("canonical example cycle");
    }

    let mut expected_source =
        SharedMemoryOpenOtSource::open(&ring_path).expect("open expected-document source");
    let expected_poll = expected_source.poll().expect("poll expected documents");
    let mut expected_consumer = OpenOtPersistenceConsumer::new(definition.clone(), None)
        .expect("create expected-document resolver");
    let expected_batch = expected_consumer
        .prepare_batch(
            &expected_poll.batch,
            &expected_poll.snapshot,
            DETERMINISTIC_SOURCE_TIME_BASE,
            0,
        )
        .expect("resolve expected SQLite documents");
    let mut expected_json = expected_batch
        .documents
        .iter()
        .map(|document| to_json(document).expect("serialize expected SQLite document"))
        .collect::<Vec<_>>();
    expected_json.sort();
    let source = SharedMemoryOpenOtSource::open(&ring_path).expect("open real shared-memory ring");
    let consumer = OpenOtPersistenceConsumer::new(definition.clone(), None)
        .expect("create canonical resolver");
    let sink = SqliteDocumentSink::open_with_definitions(&database_path, vec![definition])
        .expect("open real SQLite database");
    let mut worker = OpenOtPersistenceWorker::new(source, consumer, sink);
    let committed = worker
        .run_once(DETERMINISTIC_SOURCE_TIME_BASE)
        .expect("run complete persistence path")
        .expect("canonical example produced a durable batch");

    assert!(
        committed.inserted >= 30,
        "expected broad canonical workload"
    );
    assert_eq!(
        worker.status().unresolved,
        0,
        "definition must resolve exactly"
    );
    assert_eq!(worker.status().loss_range_count, 0);
    assert_eq!(worker.status().cursor_abs, worker.status().head_abs);
    let status_cursor_abs = worker.status().cursor_abs;
    let status_head_abs = worker.status().head_abs;
    let status_unresolved = worker.status().unresolved;
    let status_loss_range_count = worker.status().loss_range_count;
    let connection = rusqlite::Connection::open(&database_path).expect("inspect persisted SQLite");
    let persisted: i64 = connection
        .query_row("SELECT COUNT(*) FROM logging_records", [], |row| row.get(0))
        .expect("count persisted documents");
    let event_families: i64 = connection
        .query_row(
            "SELECT COUNT(DISTINCT event_name) FROM event_log",
            [],
            |row| row.get(0),
        )
        .expect("count persisted event families");
    assert_eq!(persisted, committed.inserted as i64);
    assert!(
        event_families >= 26,
        "all authored event families must persist"
    );
    for table in [
        "logged_values",
        "alarm_history",
        "message_log",
        "state_history",
        "batch_history",
        "recipe_history",
        "material_additions",
        "operator_activity",
        "audit_log",
        "electronic_signatures",
    ] {
        let count: i64 = connection
            .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
                row.get(0)
            })
            .unwrap_or_else(|error| panic!("query descriptive {table}: {error}"));
        assert!(count > 0, "real ST workload must populate {table}");
    }
    let mut actual_json = connection
        .prepare("SELECT canonical_json FROM logging_records ORDER BY identity_key")
        .expect("prepare exact SQLite JSON query")
        .query_map([], |row| row.get::<_, String>(0))
        .expect("query exact SQLite JSON")
        .collect::<Result<Vec<_>, _>>()
        .expect("collect exact SQLite JSON");
    actual_json.sort();
    assert_eq!(actual_json, expected_json, "SQLite canonical JSON drift");

    if let Some(evidence_root) = std::env::var_os("TRUST_OPENOT_EVIDENCE_DIR") {
        let evidence_root = std::path::PathBuf::from(evidence_root);
        std::fs::create_dir_all(&evidence_root).expect("create SQLite evidence directory");
        drop(worker);
        let retained_path = evidence_root.join("trust-logging.sqlite3");
        std::fs::copy(&database_path, &retained_path).expect("retain validated SQLite database");
        let mut source_wal = database_path.as_os_str().to_os_string();
        source_wal.push("-wal");
        let source_wal = std::path::PathBuf::from(source_wal);
        let retained_wal = evidence_root.join("trust-logging.sqlite3-wal");
        std::fs::copy(&source_wal, &retained_wal).expect("retain validated SQLite WAL snapshot");
        let retained = rusqlite::Connection::open(&retained_path)
            .expect("open retained SQLite database and WAL");
        let retained_schema: i64 = retained
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .expect("inspect retained SQLite schema version");
        let retained_documents: i64 = retained
            .query_row("SELECT COUNT(*) FROM logging_records", [], |row| row.get(0))
            .expect("inspect retained SQLite document count");
        assert_eq!(retained_schema, 1);
        assert_eq!(retained_documents, persisted);
        retained
            .execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")
            .expect("checkpoint retained SQLite WAL snapshot");
        drop(retained);
        let retained = rusqlite::Connection::open_with_flags(
            &retained_path,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
        )
        .expect("reopen standalone retained SQLite database read-only");
        let retained_schema: i64 = retained
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .expect("inspect retained SQLite schema version");
        let retained_documents: i64 = retained
            .query_row("SELECT COUNT(*) FROM logging_records", [], |row| row.get(0))
            .expect("inspect retained SQLite document count");
        assert_eq!(retained_schema, 1);
        assert_eq!(retained_documents, persisted);
        std::fs::write(
            evidence_root.join("openot-definition.json"),
            format!(
                "{}\n",
                serde_json::to_string_pretty(&definition_json)
                    .expect("serialize retained OpenOT definition")
            ),
        )
        .expect("retain generated OpenOT definition");
        let reconciliation = serde_json::json!({
            "schemaGeneration": 1,
            "integrityCheck": "ok",
            "persistedDocuments": persisted,
            "insertedDocuments": committed.inserted,
            "duplicateDocuments": committed.duplicated,
            "eventFamilies": event_families,
            "cursorAbs": status_cursor_abs,
            "headAbs": status_head_abs,
            "unresolved": status_unresolved,
            "lossRangeCount": status_loss_range_count,
        });
        std::fs::write(
            evidence_root.join("reconciliation.json"),
            format!(
                "{}\n",
                serde_json::to_string_pretty(&reconciliation)
                    .expect("serialize SQLite reconciliation evidence")
            ),
        )
        .expect("retain SQLite reconciliation evidence");
        drop(connection);
    } else {
        drop(connection);
    }
    std::fs::remove_file(ring_path).ok();
    std::fs::remove_dir_all(database_root).ok();
}

#[test]
fn openot_persistence_forced_ring_overflow_persists_both_loss_bases() {
    use open_ot_carriage::loss::records_dropped_record;
    use open_ot_carriage::ring::LossRange;
    use open_ot_shm::SharedRecordPublisher;

    let ring_path = temp_shm_path("database-overflow-ring");
    let database_root = temp_shm_path("database-overflow");
    std::fs::create_dir_all(&database_root).expect("database root");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&database_root, std::fs::Permissions::from_mode(0o700))
            .expect("secure database root");
    }
    let database_path = database_root.join("trust-logging.sqlite3");
    let mut publisher = SharedRecordPublisher::create(&ring_path, 4096).expect("publisher");
    for seq in 0..200u64 {
        publisher
            .append_record(&Record::new(
                DETERMINISTIC_SOURCE_TIME_BASE + seq,
                1,
                seq,
                11,
                EVENT_HEARTBEAT,
            ))
            .expect("publish overflowing heartbeat");
    }
    publisher
        .append_record(&records_dropped_record(
            0,
            &LossRange {
                run_id: 1,
                source_id: 12,
                first_seq: 5,
                last_seq: 7,
            },
        ))
        .expect("publish authoritative loss marker");
    let source = SharedMemoryOpenOtSource::open_with_limits(&ring_path, 256, 256)
        .expect("open lapped source");
    let consumer = OpenOtPersistenceConsumer::new(open_ot_definition::sample_definition(), None)
        .expect("create loss-accounting consumer");
    let sink = SqliteDocumentSink::open(&database_path).expect("open SQLite sink");
    let mut worker = OpenOtPersistenceWorker::new(source, consumer, sink);

    worker.run_once(99).expect("persist lapped ring batch");

    assert_eq!(worker.status().cursor_abs, worker.status().head_abs);
    assert!(worker.status().loss_range_count >= 2);
    assert!(worker.status().lost_record_count > 0);
    let status_lost_record_count = worker.status().lost_record_count;
    drop(worker);
    let connection = rusqlite::Connection::open(&database_path).expect("inspect loss rows");
    let mut statement = connection
        .prepare(
            "SELECT source_id,loss_basis,first_seq,last_seq,canonical_json \
             FROM logging_records WHERE document_kind='loss' ORDER BY loss_basis,first_seq",
        )
        .expect("prepare loss query");
    let rows = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, u32>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, Vec<u8>>(2)?,
                row.get::<_, Vec<u8>>(3)?,
                row.get::<_, String>(4)?,
            ))
        })
        .expect("query loss rows")
        .collect::<Result<Vec<_>, _>>()
        .expect("collect loss rows");
    let bases = rows
        .iter()
        .map(|(_, basis, _, _, _)| basis.as_str())
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(bases, ["authoritative", "inferred"].into_iter().collect());
    assert!(rows.iter().all(|(_, _, first, last, json)| {
        first.len() == 8
            && last.len() == 8
            && serde_json::from_str::<serde_json::Value>(json).is_ok()
    }));
    let decode =
        |bytes: &[u8]| u64::from_be_bytes(bytes.try_into().expect("8-byte sequence identity"));
    let inferred_source_11: u64 = rows
        .iter()
        .filter(|(source, basis, _, _, _)| *source == 11 && basis == "inferred")
        .map(|(_, _, first, last, _)| decode(last) - decode(first) + 1)
        .sum();
    let authoritative_source_12 = rows
        .iter()
        .find(|(source, basis, _, _, _)| *source == 12 && basis == "authoritative")
        .expect("authoritative source-12 loss row");
    assert_eq!(decode(&authoritative_source_12.2), 5);
    assert_eq!(decode(&authoritative_source_12.3), 7);
    let delivered_source_11 = connection
        .query_row(
            "SELECT COUNT(*) FROM logging_records \
             WHERE document_kind='event' AND source_id=11",
            [],
            |row| row.get::<_, i64>(0),
        )
        .expect("delivered source-11 count");
    let delivered_source_11 = u64::try_from(delivered_source_11).expect("nonnegative count");
    assert_eq!(delivered_source_11 + inferred_source_11, 200);
    assert_eq!(status_lost_record_count, inferred_source_11 + 3);
    std::fs::remove_file(ring_path).ok();
    std::fs::remove_dir_all(database_root).ok();
}

#[test]
fn openot_slow_real_sqlite_consumer_remains_bounded_and_reports_ring_loss() {
    use open_ot_shm::SharedRecordPublisher;
    use trust_runtime::openot_persistence::{CommitOutcome, DocumentSink};

    #[derive(Debug)]
    struct DelayedSqliteSink {
        inner: SqliteDocumentSink,
        delay: std::time::Duration,
    }

    impl DocumentSink for DelayedSqliteSink {
        fn load_checkpoint(
            &mut self,
            buffer_id: u32,
            run_id: u64,
        ) -> Result<Option<PersistenceCheckpoint>, PersistenceError> {
            self.inner.load_checkpoint(buffer_id, run_id)
        }

        fn commit(&mut self, batch: &PersistenceBatch) -> Result<CommitOutcome, PersistenceError> {
            std::thread::sleep(self.delay);
            self.inner.commit(batch)
        }
    }

    let ring_path = temp_shm_path("database-slow-consumer-ring");
    let database_root = temp_shm_path("database-slow-consumer");
    std::fs::create_dir_all(&database_root).expect("slow-consumer database root");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&database_root, std::fs::Permissions::from_mode(0o700))
            .expect("secure slow-consumer root");
    }
    let database_path = database_root.join("trust-logging.sqlite3");
    let mut publisher = SharedRecordPublisher::create(&ring_path, 4096).expect("publisher");
    publisher
        .append_record(&Record::new(
            DETERMINISTIC_SOURCE_TIME_BASE,
            1,
            0,
            11,
            EVENT_HEARTBEAT,
        ))
        .expect("initial record");
    let source = SharedMemoryOpenOtSource::open_with_limits(&ring_path, 256, 256)
        .expect("open bounded slow source");
    let consumer = OpenOtPersistenceConsumer::new(open_ot_definition::sample_definition(), None)
        .expect("slow-consumer resolver");
    let sink = DelayedSqliteSink {
        inner: SqliteDocumentSink::open(&database_path).expect("slow real SQLite sink"),
        delay: std::time::Duration::from_millis(150),
    };
    let worker = OpenOtPersistenceWorker::new(source, consumer, sink);
    let handle = std::thread::spawn(move || {
        let mut worker = worker;
        worker.run_once(1).expect("delayed first commit");
        worker
    });
    std::thread::sleep(std::time::Duration::from_millis(20));
    for seq in 1..=200u64 {
        publisher
            .append_record(&Record::new(
                DETERMINISTIC_SOURCE_TIME_BASE + seq,
                1,
                seq,
                11,
                EVENT_HEARTBEAT,
            ))
            .expect("producer remains independent of slow database");
    }
    let mut worker = handle.join().expect("join delayed worker");
    worker.run_once(2).expect("account overwritten ring");

    assert!(worker.status().loss_range_count > 0);
    assert!(worker.status().lost_record_count > 0);
    assert!(worker.status().cursor_abs <= worker.status().head_abs);
    std::fs::remove_file(ring_path).ok();
    std::fs::remove_dir_all(database_root).ok();
}

#[test]
fn openot_persistence_service_does_not_materially_regress_plc_scan_timing() {
    const PROGRAM: &str = r#"
PROGRAM Main
VAR
    Trigger : BOOL {attribute 'oot' := 'message', 'template' := 'scan timing event'};
END_VAR
Trigger := NOT Trigger;
END_PROGRAM
"#;

    fn runtime_with_ring(path: PathBuf) -> Runtime {
        let mut runtime = build_st_runtime(PROGRAM);
        runtime
            .configure_openot_telemetry(
                &st_fb_telemetry_config_for(path, 1_048_576, "Main.OotProducer"),
                None,
            )
            .expect("configure scan timing telemetry");
        runtime
    }

    fn median_scan_ns(runtime: &mut Runtime) -> u128 {
        let mut samples = Vec::new();
        for _ in 0..3 {
            let started = std::time::Instant::now();
            for _ in 0..10 {
                runtime.execute_cycle().expect("scan timing cycle");
            }
            samples.push(started.elapsed().as_nanos() / 10);
        }
        samples.sort_unstable();
        samples[samples.len() / 2]
    }

    let root = temp_shm_path("persistence-scan-timing");
    std::fs::create_dir_all(&root).expect("scan timing root");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o700))
            .expect("secure scan timing root");
    }
    let disabled_ring = root.join("disabled.shm");
    let enabled_ring = root.join("enabled.shm");
    let mut disabled_runtime = runtime_with_ring(disabled_ring);
    let mut enabled_runtime = runtime_with_ring(enabled_ring.clone());
    let definition = openot_authoring::definition_json_from_sources(&[SourceFile::with_path(
        "Main.st", PROGRAM,
    )])
    .expect("scan timing definition");
    std::fs::write(
        root.join("openot-definition.json"),
        serde_json::to_vec_pretty(&definition).expect("serialize scan timing definition"),
    )
    .expect("write scan timing definition");
    let config = OpenOtTelemetryConfig {
        enabled: true,
        path: enabled_ring,
        persistence: OpenOtPersistenceConfig {
            enabled: true,
            backend: Some(OpenOtPersistenceBackend::Sqlite),
            batch_size: 256,
            flush_interval_ms: 10,
            sqlite: Some(OpenOtSqlitePersistenceConfig {
                path: root.join("history/trust-logging.sqlite3"),
            }),
            ..OpenOtPersistenceConfig::default()
        },
        ..OpenOtTelemetryConfig::default()
    };
    let mut service = OpenOtPersistenceService::start(&config, &root)
        .expect("start scan timing persistence")
        .expect("enabled scan timing persistence");
    for _ in 0..4 {
        disabled_runtime
            .execute_cycle()
            .expect("warm disabled scan");
        enabled_runtime.execute_cycle().expect("warm enabled scan");
    }

    let disabled_ns = median_scan_ns(&mut disabled_runtime);
    let enabled_ns = median_scan_ns(&mut enabled_runtime);
    eprintln!(
        "OpenOT scan timing: disabled={disabled_ns} ns/scan enabled={enabled_ns} ns/scan ratio={:.3}",
        enabled_ns as f64 / disabled_ns.max(1) as f64
    );
    assert!(
        enabled_ns <= disabled_ns.saturating_mul(3) / 2 + 1_000_000,
        "persistence worker materially regressed scan timing: disabled={disabled_ns}ns enabled={enabled_ns}ns"
    );
    service.shutdown();
    std::fs::remove_dir_all(root).ok();
}

#[cfg(feature = "openot-real-database-tests")]
#[test]
fn openot_database_example_persists_same_real_st_workload_to_every_network_backend() {
    fn persist<D: DocumentSink>(
        sink: &mut D,
        batch: &trust_runtime::openot_persistence::PersistenceBatch,
        backend: &str,
    ) {
        let committed = sink
            .commit(batch)
            .unwrap_or_else(|error| panic!("persist canonical workload to {backend}: {error:?}"));
        assert_eq!(
            committed.inserted,
            batch.documents.len(),
            "{backend} insert"
        );
        assert_eq!(
            committed.checkpoint, batch.checkpoint,
            "{backend} checkpoint"
        );
        while sink
            .maintenance()
            .unwrap_or_else(|error| panic!("maintain {backend}: {error:?}"))
            > 0
        {}
    }

    fn canonical_jsons(documents: &[open_ot_document::Document]) -> Vec<String> {
        let mut jsons = documents
            .iter()
            .map(|document| to_json(document).expect("serialize canonical workload document"))
            .collect::<Vec<_>>();
        jsons.sort();
        jsons
    }

    fn assert_exact(mut actual: Vec<String>, expected: &[String], backend: &str) {
        actual.sort();
        assert_eq!(
            actual.len(),
            expected.len(),
            "{backend} stored document count drift"
        );
        assert_eq!(actual, expected, "{backend} canonical JSON drift");
    }

    let ring_path = temp_shm_path("database-example-network-ring");
    let program = include_str!("../../../examples/openot_multi_program/src/Main.st");
    let definition_json = openot_authoring::definition_json_from_sources(&[SourceFile::with_path(
        "main.st", program,
    )])
    .expect("generate canonical network definition");
    let definition: DefinitionFile =
        serde_json::from_value(definition_json).expect("parse canonical network definition");
    let mut runtime = build_st_runtime(program);
    runtime
        .configure_openot_telemetry(
            &st_fb_telemetry_config_for_many(
                ring_path.clone(),
                65_536,
                &[
                    "Filler.OotProducer",
                    "BatchControl.OotProducer",
                    "OperatorAudit.OotProducer",
                    "SignatureAudit.OotProducer",
                    "TypedValues.OotProducer",
                    "ConditionLifecycle.OotProducer",
                ],
            ),
            None,
        )
        .expect("configure canonical network ring");
    for _ in 0..12 {
        runtime.execute_cycle().expect("canonical network cycle");
    }

    let mut source = SharedMemoryOpenOtSource::open(&ring_path).expect("open canonical source");
    let poll = source.poll().expect("poll canonical authored workload");
    let mut consumer = OpenOtPersistenceConsumer::new(definition.clone(), None)
        .expect("create canonical resolver");
    let batch = consumer
        .prepare_batch(
            &poll.batch,
            &poll.snapshot,
            DETERMINISTIC_SOURCE_TIME_BASE,
            0,
        )
        .expect("resolve canonical authored workload once");
    assert!(
        batch.documents.len() >= 30,
        "canonical workload is too small"
    );
    assert!(batch
        .documents
        .iter()
        .all(|document| matches!(document, open_ot_document::Document::Event(_))));
    let expected = canonical_jsons(&batch.documents);

    let pid = std::process::id();
    let postgres_url = std::env::var("TRUST_TEST_OPENOT_POSTGRES_URL").expect("PostgreSQL URL");
    let postgres_ca = std::env::var("TRUST_TEST_OPENOT_POSTGRES_CA").expect("PostgreSQL CA");
    let mut postgres = PostgreSqlDocumentSink::open_with_definitions(
        &postgres_url,
        &format!("logging_e2e_{pid}"),
        std::path::Path::new(&postgres_ca),
        vec![definition.clone()],
    )
    .expect("open PostgreSQL e2e sink");
    persist(&mut postgres, &batch, "PostgreSQL");
    assert_exact(
        postgres.canonical_jsons().expect("query PostgreSQL JSON"),
        &expected,
        "PostgreSQL",
    );

    let timescale_url = std::env::var("TRUST_TEST_OPENOT_TIMESCALE_URL").expect("Timescale URL");
    let timescale_ca = std::env::var("TRUST_TEST_OPENOT_TIMESCALE_CA").expect("Timescale CA");
    let mut timescale = TimescaleDbDocumentSink::open_with_definitions(
        &timescale_url,
        &format!("logging_e2e_{pid}"),
        std::path::Path::new(&timescale_ca),
        vec![definition.clone()],
    )
    .expect("open TimescaleDB e2e sink");
    persist(&mut timescale, &batch, "TimescaleDB");
    assert_exact(
        timescale.canonical_jsons().expect("query TimescaleDB JSON"),
        &expected,
        "TimescaleDB",
    );

    for (name, url_env, ca_env) in [
        (
            "MySQL",
            "TRUST_TEST_OPENOT_MYSQL_URL",
            "TRUST_TEST_OPENOT_MYSQL_CA",
        ),
        (
            "MariaDB",
            "TRUST_TEST_OPENOT_MARIADB_URL",
            "TRUST_TEST_OPENOT_MARIADB_CA",
        ),
    ] {
        let url = std::env::var(url_env).unwrap_or_else(|_| panic!("{name} URL"));
        let ca = std::env::var(ca_env).unwrap_or_else(|_| panic!("{name} CA"));
        let mut sink = MySqlDocumentSink::open_with_definitions(
            &url,
            "trust_logging",
            std::path::Path::new(&ca),
            vec![definition.clone()],
        )
        .unwrap_or_else(|error| panic!("open {name} e2e sink: {error:?}"));
        sink.reset_test_state()
            .unwrap_or_else(|error| panic!("reset {name} fixture: {error:?}"));
        persist(&mut sink, &batch, name);
        assert_exact(
            sink.canonical_jsons()
                .unwrap_or_else(|error| panic!("query {name} JSON: {error:?}")),
            &expected,
            name,
        );
    }

    let sqlserver_url = std::env::var("TRUST_TEST_OPENOT_SQLSERVER_URL").expect("SQL Server URL");
    let sqlserver_ca = std::env::var("TRUST_TEST_OPENOT_SQLSERVER_CA").expect("SQL Server CA");
    let mut sqlserver = SqlServerDocumentSink::open_with_definitions(
        &sqlserver_url,
        &format!("logging_e2e_{pid}"),
        std::path::Path::new(&sqlserver_ca),
        vec![definition.clone()],
    )
    .expect("open SQL Server e2e sink");
    persist(&mut sqlserver, &batch, "SQL Server");
    assert_exact(
        sqlserver.canonical_jsons().expect("query SQL Server JSON"),
        &expected,
        "SQL Server",
    );

    let influx_host = std::env::var("TRUST_TEST_OPENOT_INFLUX_HOST").expect("InfluxDB host");
    let influx_token = std::env::var("TRUST_TEST_OPENOT_INFLUX_TOKEN").expect("InfluxDB token");
    let influx_ca = std::env::var("TRUST_TEST_OPENOT_INFLUX_CA").expect("InfluxDB CA");
    let spool_root = temp_shm_path("database-example-influx-spool");
    let spool = spool_root.join("spool.sqlite3");
    let mut influx = InfluxDb3DocumentSink::open_bounded_with_definitions(
        &influx_host,
        &influx_token,
        "trust_logging",
        &spool,
        std::path::Path::new(&influx_ca),
        u64::MAX,
        vec![definition],
    )
    .expect("open InfluxDB 3 e2e sink");
    persist(&mut influx, &batch, "InfluxDB 3");
    // Influx acknowledges a WAL-synchronous write before the query engine's
    // read snapshot necessarily exposes the batch. Verification waits for
    // bounded query visibility; durability status still follows the write ack.
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
    let influx_json = loop {
        let documents = influx
            .remote_canonical_jsons_at(DETERMINISTIC_SOURCE_TIME_BASE)
            .expect("query InfluxDB 3 JSON");
        if documents.len() == expected.len() || std::time::Instant::now() >= deadline {
            break documents;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    };
    assert_exact(influx_json, &expected, "InfluxDB 3");

    std::fs::remove_file(ring_path).ok();
    std::fs::remove_dir_all(spool_root).ok();
}

#[test]
fn openot_telemetry_drains_multi_program_source_high_water_to_one_ring() {
    let path = temp_shm_path("st-fb-multi-program-high-water");
    let mut runtime = build_st_runtime(multi_program_checkpoint_program());
    runtime
        .configure_openot_telemetry(
            &st_fb_telemetry_config_for_many(
                path.clone(),
                4096,
                &["First.Producer", "Second.Producer"],
            ),
            None,
        )
        .expect("configure multi-program OpenOT checkpoint telemetry");

    let store = SharedConcurrentStore::open_existing(&path).expect("open telemetry shm");
    let mut consumer = ConcurrentRawConsumer::with_store(store);
    let mut accounting = LossAccountingConsumer::new();

    runtime
        .execute_cycle()
        .expect("first scan registers both program sources");
    let batch = consumer.poll().expect("poll multi-program messages");
    accounting.account_batch(&batch);
    assert_eq!(batch.records.len(), 2);
    assert_eq!(batch.records[0].record.event_type_id, EVENT_MESSAGE);
    assert_eq!(batch.records[0].record.source_id, 10);
    assert_eq!(batch.records[0].record.seq, 0);
    assert_eq!(batch.records[1].record.event_type_id, EVENT_MESSAGE);
    assert_eq!(batch.records[1].record.source_id, 20);
    assert_eq!(batch.records[1].record.seq, 0);

    runtime
        .execute_cycle()
        .expect("second scan publishes one high-water checkpoint per program source");
    let batch = consumer.poll().expect("poll multi-program checkpoints");
    accounting.account_batch(&batch);
    assert_eq!(batch.records.len(), 2);
    assert_eq!(consumer.rejected_records(), 0);

    for (record, expected_source) in batch.records.iter().map(|read| &read.record).zip([10, 20]) {
        assert_eq!(record.event_type_id, EVENT_SOURCE_HIGH_WATER);
        assert_eq!(record.source_id, expected_source);
        assert_eq!(record.seq, 1);
        assert_eq!(required_ulint(record, KEY_SOURCE_HIGH_WATER), 1);
    }

    assert_eq!(accounting.delivered(10), 2);
    assert_eq!(accounting.delivered(20), 2);
    assert_eq!(accounting.lost(10), 0);
    assert_eq!(accounting.lost(20), 0);

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
    let artifact_dir = temp_shm_path("authoring-showcase-artifacts");
    write_authoring_showcase_artifacts(
        &artifact_dir,
        &retained_records,
        &audit_log,
        &overflow,
        &program,
    )
    .expect("write OpenOT authoring showcase artifact");
    for name in [
        "batch-log.txt",
        "batch-log.json",
        "openot-definition.json",
        "README.md",
    ] {
        assert!(
            artifact_dir.join(name).is_file(),
            "showcase artifact {name} was not written"
        );
    }

    drop(std::fs::remove_file(path));
    drop(std::fs::remove_dir_all(artifact_dir));
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
fn openot_telemetry_authoring_parameter_change_round_trip() {
    let path = temp_shm_path("authoring-parameter-change");
    let program = r#"
PROGRAM Main
VAR
    Phase : UINT;
    OperatorName : STRING[32] := 'operator-a';
    ReasonText : STRING[32] := 'audit change';
    SetPoint : REAL {attribute 'oot' := 'value', 'id' := '2101', 'audit' := 'true', 'actor' := OperatorName, 'reason' := ReasonText, 'auth' := 'Granted', 'unit' := 'bar', 'semanticRole' := 'setpoint'};
    BatchCount : DINT {attribute 'oot' := 'value', 'id' := '2102', 'audit' := 'true', 'actor' := OperatorName, 'reason' := ReasonText};
    Enabled : BOOL {attribute 'oot' := 'value', 'id' := '2103', 'audit' := 'true', 'actor' := OperatorName, 'reason' := ReasonText};
    ModeText : STRING[16] {attribute 'oot' := 'value', 'id' := '2104', 'audit' := 'true', 'actor' := OperatorName, 'reason' := ReasonText};
END_VAR

CASE Phase OF
    UINT#0:
        SetPoint := REAL#10.0;
        BatchCount := DINT#1;
        Enabled := FALSE;
        ModeText := 'manual';
    UINT#1:
        SetPoint := REAL#12.5;
        BatchCount := DINT#2;
        Enabled := TRUE;
        ModeText := 'auto';
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

    let store = SharedConcurrentStore::open_existing(&path).expect("open telemetry shm");
    let mut consumer = ConcurrentRawConsumer::with_store(store);

    runtime
        .execute_cycle()
        .expect("first audited value cycle only seeds baselines");
    let batch = consumer.poll().expect("poll seeded audited values");
    assert_eq!(batch.records.len(), 0);

    runtime
        .execute_cycle()
        .expect("second audited value cycle publishes changes");
    let batch = consumer.poll().expect("poll audited value changes");
    assert_eq!(batch.records.len(), 4);
    assert!(batch
        .records
        .iter()
        .all(|read| read.record.event_type_id == EVENT_PARAMETER_CHANGE));
    assert!(batch
        .records
        .iter()
        .all(|read| read.record.event_type_id != EVENT_VALUE_CHANGED));

    let rendered = batch
        .records
        .iter()
        .map(|read| render_record(&read.record))
        .collect::<Vec<_>>();
    assert_eq!(
        rendered,
        [
            "ParameterChange source=1 seq=0 valueId=2101 previous=REAL(10) new=REAL(12.5) actor=operator-a reason=audit change authResult=0",
            "ParameterChange source=1 seq=1 valueId=2102 previous=DINT(1) new=DINT(2) actor=operator-a reason=audit change",
            "ParameterChange source=1 seq=2 valueId=2103 previous=BOOL(false) new=BOOL(true) actor=operator-a reason=audit change",
            "ParameterChange source=1 seq=3 valueId=2104 previous=STRING(manual) new=STRING(auto) actor=operator-a reason=audit change",
        ]
    );

    let definition_json = openot_authoring::definition_json_from_sources(&[SourceFile::with_path(
        "parameter_change.st",
        program,
    )])
    .expect("parameter-change definition");
    assert_eq!(definition_json["values"][0]["valueId"], 2101);
    assert_eq!(definition_json["values"][0]["name"], "SetPoint");
    assert_eq!(definition_json["values"][0]["semanticRole"], 1);
    assert_eq!(definition_json["values"][0]["unit"], 4);
    assert_eq!(definition_json["operatorDefinitions"], json!([]));
    let definition: DefinitionFile =
        serde_json::from_value(definition_json).expect("definition parses");
    let hash = compute_content_hash(&definition).expect("hash definition");
    let snapshot = ControlBlockSnapshot {
        version: 2,
        caps: 0,
        buffer_id: DEFAULT_BUFFER_ID,
        buffer_bytes: 4096,
        head_abs: batch.records.last().map_or(0, |record| record.end_abs),
        oldest_abs: batch.records.first().map_or(0, |record| record.start_abs),
        lost_count: 0,
        run_id: 1,
        epoch_id: 1,
        epoch_first_abs: 0,
        definition_hash: hash.carriage_hash,
        prev_definition_hash: [0; 8],
    };
    let definition_set = DefinitionSet::current(&definition);
    let resolution = resolve_record(
        &batch.records[0].record,
        batch.records[0].start_abs,
        &snapshot,
        &definition_set,
    );
    let context = RecordDocumentContext::new(
        DEFAULT_BUFFER_ID,
        batch.records[0].record.source_time,
        hash.carriage_hash,
        batch.records[0].record.flags,
    )
    .with_semantic_version(definition.header.semantic_version.clone());
    let document = document_from_resolution(&resolution, &context);
    let document_text = to_json(&document).expect("serialize parameter-change document");
    assert!(
        document_text.contains(r#""eventName":"ParameterChange""#),
        "{document_text}"
    );
    assert!(
        document_text.contains(r#""name":"value","type":"ValueRef","value":"SetPoint""#),
        "{document_text}"
    );
    assert!(
        document_text.contains(r#""name":"actor","type":"String","value":"operator-a""#),
        "{document_text}"
    );
    assert!(
        document_text.contains(r#""name":"reason","type":"String","value":"audit change""#),
        "{document_text}"
    );

    drop(std::fs::remove_file(path));
}

#[test]
fn openot_telemetry_parameter_change_oversize_drop_keeps_baseline() {
    let path = temp_shm_path("parameter-change-oversize");
    let program = r#"
PROGRAM Main
VAR
    Producer : OPENOT_Producer;
    Phase : UINT;
    ActorLong : STRING[96] := 'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa';
    ReasonLong : STRING[96] := 'bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb';
    ActorShort : STRING[8] := 'operator';
    ReasonShort : STRING[8] := 'reason';
    OldText : STRING[96] := 'old-state';
    NewText : STRING[96] := 'new-state';
END_VAR

CASE Phase OF
    UINT#0:
        Producer(Execute := TRUE, Op := UINT#15, SourceId := UDINT#88, ValueId := UDINT#2301, ValueTypeTag := BYTE#16#0C, ValuePayloadLength := UINT#96, ValueString := OldText, AuditActor := ActorLong, AuditReason := ReasonLong);
        Producer(Execute := FALSE);
    UINT#1:
        Producer(Execute := TRUE, Op := UINT#15, SourceId := UDINT#88, ValueId := UDINT#2301, ValueTypeTag := BYTE#16#0C, ValuePayloadLength := UINT#96, ValueString := NewText, AuditActor := ActorLong, AuditReason := ReasonLong);
        Producer(Execute := FALSE);
    UINT#2:
        Producer(Execute := TRUE, Op := UINT#15, SourceId := UDINT#88, ValueId := UDINT#2301, ValueTypeTag := BYTE#16#0C, ValuePayloadLength := UINT#96, ValueString := NewText, AuditActor := ActorShort, AuditReason := ReasonShort);
        Producer(Execute := FALSE);
END_CASE;
Phase := Phase + UINT#1;
END_PROGRAM
"#;
    let mut runtime = build_st_runtime(program);
    runtime
        .configure_openot_telemetry(&st_fb_telemetry_config(path.clone(), 4096), None)
        .expect("configure OpenOT ST FB telemetry");

    let store = SharedConcurrentStore::open_existing(&path).expect("open telemetry shm");
    let mut consumer = ConcurrentRawConsumer::with_store(store);

    runtime
        .execute_cycle()
        .expect("first audited string observation only seeds");
    let batch = consumer.poll().expect("poll seeded audited string");
    assert!(batch.records.is_empty(), "{batch:#?}");

    let err = runtime
        .execute_cycle()
        .expect_err("oversize audited string record must fail closed");
    let err = format!("{err:?}");
    assert!(err.contains("dropped 1 OpenOT command(s)"), "{err}");
    assert!(err.contains("LastCommandError 3"), "{err}");
    let batch = consumer.poll().expect("poll after oversize audited string");
    assert!(batch.records.is_empty(), "{batch:#?}");
    runtime.clear_fault();

    runtime
        .execute_cycle()
        .expect("short audit strings retry publishes original previous value");
    let batch = consumer.poll().expect("poll retried audited string");
    assert_eq!(batch.records.len(), 1);
    assert_eq!(
        render_record(&batch.records[0].record),
        "ParameterChange source=88 seq=0 valueId=2301 previous=STRING(old-state) new=STRING(new-state) actor=operator reason=reason"
    );

    drop(std::fs::remove_file(path));
}

#[test]
fn openot_telemetry_authoring_batch_recipe_round_trip() {
    let path = temp_shm_path("authoring-batch-recipe");
    let program = r#"
TYPE E_BatchState : (Started := 0, Completed := 1, Held := 2, Resumed := 3, Aborted := 4, Paused := 5) END_TYPE

PROGRAM Main
VAR
    Phase : UINT;
    RecipeId : UDINT := UDINT#3001;
    RecipeVersion : STRING[16] := 'v1.2.3';
    BatchId : UDINT := UDINT#4001;
    MaterialId : UDINT := UDINT#5001;
    Quantity : LREAL := LREAL#12.25;
    AuthResult : UINT := UINT#1;
    Approver : STRING[32] := 'approver-a';
    BatchState : E_BatchState := Started {attribute 'oot' := 'batch', 'batchId' := BatchId, 'recipe' := RecipeId};
    RecipeLoadedTrigger : BOOL {attribute 'oot' := 'recipe-loaded', 'recipe' := RecipeId, 'version' := RecipeVersion, 'batch' := BatchId};
    RecipeApprovedTrigger : BOOL {attribute 'oot' := 'recipe-approved', 'recipe' := RecipeId, 'version' := RecipeVersion, 'auth' := AuthResult, 'by' := Approver};
    MaterialAdditionTrigger : BOOL {attribute 'oot' := 'material-addition', 'batch' := BatchId, 'material' := MaterialId, 'quantity' := Quantity, 'unit' := 'kg'};
END_VAR

CASE Phase OF
    UINT#0:
        RecipeLoadedTrigger := TRUE;
    UINT#1:
        RecipeApprovedTrigger := TRUE;
    UINT#2:
        MaterialAdditionTrigger := TRUE;
    UINT#3:
        BatchState := Held;
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

    let store = SharedConcurrentStore::open_existing(&path).expect("open telemetry shm");
    let mut consumer = ConcurrentRawConsumer::with_store(store);
    let mut rendered = Vec::new();
    for _ in 0..4 {
        runtime
            .execute_cycle()
            .expect("batch recipe cycle publishes");
        let batch = consumer.poll().expect("poll batch recipe record");
        assert_eq!(batch.records.len(), 1);
        rendered.push(render_record(&batch.records[0].record));
    }

    assert_eq!(
        rendered,
        [
            "RecipeLoaded source=1 seq=0 recipeId=3001 version=v1.2.3 batchId=4001",
            "RecipeApproved source=1 seq=1 recipeId=3001 version=v1.2.3 authResult=1 ackBy=approver-a",
            "MaterialAddition source=1 seq=2 batchId=4001 materialId=5001 quantity=12.25 unit=8",
            "BatchEvent source=1 seq=3 batchId=4001 newState=2 recipeId=3001",
        ]
    );

    let definition = openot_authoring::definition_json_from_sources(&[SourceFile::with_path(
        "batch_recipe.st",
        program,
    )])
    .expect("definition");
    assert_eq!(definition["recipeDefinitions"], json!([]));
    assert_eq!(definition["batchDefinitions"], json!([]));
    assert_eq!(definition["materialDefinitions"], json!([]));

    drop(std::fs::remove_file(path));
}

#[test]
fn openot_telemetry_authoring_operator_regulated_round_trip() {
    let path = temp_shm_path("authoring-operator-regulated");
    let program = r#"
PROGRAM Main
VAR
    Phase : UINT;
    ActionId : UDINT := UDINT#6001;
    ContextA : UDINT := UDINT#7001;
    ContextB : UDINT := UDINT#7002;
    OperatorName : STRING[32] := 'operator-a';
    FailedOperator : STRING[32] := 'operator-x';
    Workstation : STRING[32] := 'station-1';
    FailedWorkstation : STRING[32] := 'station-2';
    Role : UINT := UINT#3;
    ReasonText : STRING[32] := 'denied';
    ActionTrigger : BOOL {attribute 'oot' := 'operator-action', 'action' := ActionId, 'actor' := OperatorName, 'context1' := ContextA, 'context2' := ContextB, 'auth' := 'Granted', 'workstation' := Workstation};
    LoginTrigger : BOOL {attribute 'oot' := 'operator-login', 'actor' := OperatorName, 'auth' := 'Granted', 'workstation' := Workstation, 'role' := Role};
    LogoutTrigger : BOOL {attribute 'oot' := 'operator-logout', 'actor' := OperatorName, 'workstation' := Workstation};
    SecurityFailureTrigger : BOOL {attribute 'oot' := 'security-failure', 'actor' := FailedOperator, 'workstation' := FailedWorkstation, 'reason' := ReasonText};
END_VAR

CASE Phase OF
    UINT#0:
        ActionTrigger := TRUE;
    UINT#1:
        LoginTrigger := TRUE;
    UINT#2:
        LogoutTrigger := TRUE;
    UINT#3:
        SecurityFailureTrigger := TRUE;
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

    let store = SharedConcurrentStore::open_existing(&path).expect("open telemetry shm");
    let mut consumer = ConcurrentRawConsumer::with_store(store);
    let mut rendered = Vec::new();
    for _ in 0..4 {
        runtime
            .execute_cycle()
            .expect("operator regulated cycle publishes");
        let batch = consumer.poll().expect("poll operator regulated record");
        assert_eq!(batch.records.len(), 1);
        rendered.push(render_record(&batch.records[0].record));
    }

    assert_eq!(
        rendered,
        [
            "OperatorAction source=1 seq=0 actionId=6001 actor=operator-a contextRef=[7001,7002] authResult=0 workstation=station-1",
            "OperatorLogin source=1 seq=1 actor=operator-a authResult=0 workstation=station-1 role=3",
            "OperatorLogout source=1 seq=2 actor=operator-a workstation=station-1",
            "SecurityAccessFailure source=1 seq=3 actor=operator-x workstation=station-2 reason=denied",
        ]
    );

    let definition = openot_authoring::definition_json_from_sources(&[SourceFile::with_path(
        "operator_regulated.st",
        program,
    )])
    .expect("definition");
    assert_eq!(definition["operatorDefinitions"], json!([]));

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
fn openot_telemetry_authoring_esignature_cross_scan_attests_prior_event() {
    let path = temp_shm_path("authoring-esignature-cross-scan");
    let program = r#"
PROGRAM Main
VAR
    Phase : UINT;
    ActionId : UDINT := UDINT#6002;
    OperatorName : STRING[32] := 'operator-a';
    TargetAction : BOOL {attribute 'oot' := 'operator-action', 'action' := ActionId, 'actor' := OperatorName};
    SignTarget : BOOL {attribute 'oot' := 'e-signature', 'action' := ActionId, 'actor' := OperatorName, 'meaning' := 'Approved', 'attests' := TargetAction, 'auth' := 'Granted'};
END_VAR

CASE Phase OF
    UINT#0:
        TargetAction := TRUE;
    UINT#1:
        TargetAction := FALSE;
        SignTarget := TRUE;
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

    let store = SharedConcurrentStore::open_existing(&path).expect("open telemetry shm");
    let mut consumer = ConcurrentRawConsumer::with_store(store);

    runtime.execute_cycle().expect("target action publishes");
    let target_batch = consumer.poll().expect("poll target action");
    assert_eq!(target_batch.records.len(), 1);
    assert_eq!(
        target_batch.records[0].record.event_type_id,
        EVENT_OPERATOR_ACTION
    );
    let target_seq = target_batch.records[0].record.seq;

    runtime.execute_cycle().expect("e-signature publishes");
    let signature_batch = consumer.poll().expect("poll e-signature");
    assert_eq!(signature_batch.records.len(), 1);
    let signature = &signature_batch.records[0].record;
    assert_eq!(signature.event_type_id, EVENT_ESIGNATURE);
    assert_eq!(
        render_record(signature),
        "ESignature source=1 seq=1 actionId=6002 actor=operator-a signatureMeaning=2 signedEventSeq=0 authResult=0"
    );
    assert_eq!(required_ulint(signature, KEY_SIGNED_EVENT_SEQ), target_seq);

    drop(std::fs::remove_file(path));
}

#[test]
fn openot_telemetry_authoring_esignature_same_scan_is_phased_last() {
    let path = temp_shm_path("authoring-esignature-same-scan");
    let program = r#"
PROGRAM Main
VAR
    ActionId : UDINT := UDINT#6002;
    OperatorName : STRING[32] := 'operator-a';
    SignTarget : BOOL {attribute 'oot' := 'e-signature', 'action' := ActionId, 'actor' := OperatorName, 'meaning' := 'Approved', 'attests' := TargetAction};
    TargetAction : BOOL {attribute 'oot' := 'operator-action', 'action' := ActionId, 'actor' := OperatorName};
END_VAR

SignTarget := TRUE;
TargetAction := TRUE;
END_PROGRAM
"#;
    let mut runtime = build_st_runtime(program);
    runtime
        .configure_openot_telemetry(
            &st_fb_telemetry_config_for(path.clone(), 4096, "Main.OotProducer"),
            None,
        )
        .expect("configure OpenOT ST FB telemetry");

    let store = SharedConcurrentStore::open_existing(&path).expect("open telemetry shm");
    let mut consumer = ConcurrentRawConsumer::with_store(store);

    runtime
        .execute_cycle()
        .expect("same-scan target and e-signature publish");
    let batch = consumer.poll().expect("poll same-scan e-signature");
    assert_eq!(batch.records.len(), 2);
    let target = &batch.records[0].record;
    let signature = &batch.records[1].record;
    assert_eq!(target.event_type_id, EVENT_OPERATOR_ACTION);
    assert_eq!(signature.event_type_id, EVENT_ESIGNATURE);
    assert_eq!(required_ulint(signature, KEY_SIGNED_EVENT_SEQ), target.seq);
    assert_eq!(
        render_record(signature),
        "ESignature source=1 seq=1 actionId=6002 actor=operator-a signatureMeaning=2 signedEventSeq=0"
    );

    drop(std::fs::remove_file(path));
}

#[test]
fn openot_telemetry_authoring_esignature_never_emitted_target_fails_closed() {
    let path = temp_shm_path("authoring-esignature-never-emitted");
    let program = r#"
PROGRAM Main
VAR
    ActionId : UDINT := UDINT#6002;
    OperatorName : STRING[32] := 'operator-a';
    TargetAction : BOOL {attribute 'oot' := 'operator-action', 'action' := ActionId, 'actor' := OperatorName};
    SignTarget : BOOL {attribute 'oot' := 'e-signature', 'action' := ActionId, 'actor' := OperatorName, 'meaning' := 'Approved', 'attests' := TargetAction};
END_VAR

SignTarget := TRUE;
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
        .expect_err("e-signature without attested event must fail closed");
    let err = format!("{err:?}");
    assert!(err.contains("dropped 1 OpenOT command(s)"), "{err}");
    assert!(err.contains("LastCommandError 6"), "{err}");

    let store = SharedConcurrentStore::open_existing(&path).expect("open telemetry shm");
    let mut consumer = ConcurrentRawConsumer::with_store(store);
    let batch = consumer.poll().expect("poll failed e-signature");
    assert!(batch.records.is_empty(), "{batch:#?}");

    drop(std::fs::remove_file(path));
}

#[test]
fn openot_telemetry_esignature_attestation_state_resets_on_epoch_transition() {
    let path = temp_shm_path("esignature-epoch-reset");
    let program = r#"
PROGRAM Main
VAR
    Producer : OPENOT_Producer;
    Phase : UINT;
    Signer : STRING[32] := 'operator-a';
    DefHashOld : OPENOT_HASH8 := [
        BYTE#16#6F, BYTE#16#6C, BYTE#16#64, BYTE#16#68,
        BYTE#16#61, BYTE#16#73, BYTE#16#68, BYTE#16#31
    ];
    DefHashNew : OPENOT_HASH8 := [
        BYTE#16#6E, BYTE#16#65, BYTE#16#77, BYTE#16#68,
        BYTE#16#61, BYTE#16#73, BYTE#16#68, BYTE#16#32
    ];
END_VAR

Producer(Execute := FALSE, ResetScanRecords := TRUE);
Producer(Execute := FALSE, ResetScanRecords := FALSE);

CASE Phase OF
    UINT#0:
        Producer(Execute := TRUE, Op := UINT#0, SourceId := UDINT#88, Checkpoint := FALSE, MessageTemplateId := UDINT#10001, AttestableId := UDINT#1);
        Producer(Execute := FALSE);
    UINT#1:
        Producer(Execute := TRUE, Op := UINT#16, SourceId := UDINT#88, SignatureActionId := UDINT#6002, SignatureActor := Signer, SignatureMeaning := UINT#2, SignatureAttestsId := UDINT#1);
        Producer(Execute := FALSE);
    UINT#2:
        Producer(Execute := TRUE, Op := UINT#1, SourceId := UDINT#0, Checkpoint := FALSE, DefHashOld := DefHashOld, DefHashNew := DefHashNew, TransitionEpochId := ULINT#2);
        Producer(Execute := FALSE);
    UINT#3:
        Producer(Execute := TRUE, Op := UINT#2, SourceId := UDINT#0, Checkpoint := FALSE);
        Producer(Execute := FALSE);
    UINT#4:
        Producer(Execute := TRUE, Op := UINT#3, SourceId := UDINT#0, Checkpoint := FALSE);
        Producer(Execute := FALSE);
    UINT#5:
        Producer(Execute := TRUE, Op := UINT#16, SourceId := UDINT#88, SignatureActionId := UDINT#6002, SignatureActor := Signer, SignatureMeaning := UINT#2, SignatureAttestsId := UDINT#1);
        Producer(Execute := FALSE);
END_CASE;

Phase := Phase + UINT#1;
END_PROGRAM
"#;
    let mut runtime = build_st_runtime(program);
    runtime
        .configure_openot_telemetry(&st_fb_telemetry_config(path.clone(), 4096), None)
        .expect("configure OpenOT ST FB telemetry");

    let store = SharedConcurrentStore::open_existing(&path).expect("open telemetry shm");
    let mut consumer = ConcurrentRawConsumer::with_store(store);

    runtime
        .execute_cycle()
        .expect("attestable target publishes");
    let batch = consumer.poll().expect("poll target");
    assert_eq!(batch.records.len(), 1);
    assert_eq!(batch.records[0].record.event_type_id, EVENT_MESSAGE);
    runtime
        .execute_cycle()
        .expect("pre-transition signature publishes");
    let batch = consumer.poll().expect("poll pre-transition signature");
    assert_eq!(batch.records.len(), 1);
    assert_eq!(batch.records[0].record.event_type_id, EVENT_ESIGNATURE);
    assert_eq!(
        required_ulint(&batch.records[0].record, KEY_SIGNED_EVENT_SEQ),
        0
    );

    for _ in 0..3 {
        runtime
            .execute_cycle()
            .expect("transition step publishes and clears attestable state");
        let _ = consumer.poll().expect("poll transition record(s)");
    }

    let err = runtime
        .execute_cycle()
        .expect_err("post-transition e-signature must not attest prior epoch");
    let err = format!("{err:?}");
    assert!(err.contains("dropped 1 OpenOT command(s)"), "{err}");
    assert!(err.contains("LastCommandError 6"), "{err}");

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
        CommentText : STRING[32] := 'operator comment';
        ShelveSecs : UDINT := UDINT#300;
        PreviousPriority : UINT := UINT#600;
        NewPriority : UINT := UINT#900;
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
        CommentHighPh : BOOL {attribute 'oot' := 'condition', 'of' := HighPhAlarm, 'event' := 'comment', 'comment' := CommentText, 'by' := OperatorName};
        PriorityHighPh : BOOL {attribute 'oot' := 'condition', 'of' := HighPhAlarm, 'event' := 'priority-changed', 'previous-priority' := PreviousPriority, 'new-priority' := NewPriority, 'by' := OperatorName};
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
        UINT#9:
            CommentHighPh := TRUE;
        UINT#10:
            PriorityHighPh := TRUE;
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

    runtime.execute_cycle().expect("comment cycle publishes");
    let batch = consumer.poll().expect("poll comment record");
    assert_eq!(batch.records.len(), 1);
    assert_eq!(
        render_record(&batch.records[0].record),
        "ConditionCommented source=77 seq=10 conditionId=9101 correlation=1 comment=operator comment ackBy=operator-a"
    );

    runtime
        .execute_cycle()
        .expect("priority-changed cycle publishes");
    let batch = consumer.poll().expect("poll priority-changed record");
    assert_eq!(batch.records.len(), 1);
    assert_eq!(
        render_record(&batch.records[0].record),
        "ConditionPriorityChanged source=77 seq=11 conditionId=9101 previousPriority=600 newPriority=900 ackBy=operator-a"
    );
    assert!(slot(&batch.records[0].record, KEY_CORRELATION_ID).is_none());

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
fn openot_telemetry_authoring_condition_priority_while_inactive_emits() {
    let path = temp_shm_path("authoring-condition-priority-inactive");
    let program = r#"
PROGRAM Main
VAR
    OperatorName : STRING[32] := 'operator-a';
    PreviousPriority : UINT := UINT#600;
    NewPriority : UINT := UINT#900;
    HighPhAlarm : BOOL {attribute 'oot' := 'alarm', 'sourceid' := '77', 'conditionid' := '9101'};
    PriorityHighPh : BOOL {attribute 'oot' := 'condition', 'of' := HighPhAlarm, 'event' := 'priority-changed', 'previous-priority' := PreviousPriority, 'new-priority' := NewPriority, 'by' := OperatorName};
END_VAR

PriorityHighPh := TRUE;
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
        .expect("inactive priority-changed cycle publishes");
    let store = SharedConcurrentStore::open_existing(&path).expect("open telemetry shm");
    let mut consumer = ConcurrentRawConsumer::with_store(store);
    let batch = consumer.poll().expect("poll inactive priority record");
    assert_eq!(batch.records.len(), 1);
    assert_eq!(
        render_record(&batch.records[0].record),
        "ConditionPriorityChanged source=77 seq=0 conditionId=9101 previousPriority=600 newPriority=900 ackBy=operator-a"
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

#[test]
fn openot_telemetry_authoring_condition_comment_oversize_fails_closed() {
    let path = temp_shm_path("authoring-condition-comment-oversize");
    let program = r#"
PROGRAM Main
VAR
    OperatorName : STRING[96] := 'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa';
    CommentText : STRING[96] := 'bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb';
    HighPhAlarm : BOOL {attribute 'oot' := 'alarm', 'sourceid' := '77', 'conditionid' := '9101'};
    CommentHighPh : BOOL {attribute 'oot' := 'condition', 'of' := HighPhAlarm, 'event' := 'comment', 'comment' := CommentText, 'by' := OperatorName};
END_VAR

HighPhAlarm := TRUE;
CommentHighPh := TRUE;
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
        .expect_err("oversize comment lifecycle record must fail closed");
    let err = format!("{err:?}");
    assert!(err.contains("dropped 1 OpenOT lifecycle command"), "{err}");
    assert!(err.contains("LastLifecycleError 6"), "{err}");

    let store = SharedConcurrentStore::open_existing(&path).expect("open telemetry shm");
    let mut consumer = ConcurrentRawConsumer::with_store(store);
    let batch = consumer.poll().expect("poll after oversize comment");
    assert!(batch.records.is_empty(), "{batch:#?}");

    drop(std::fs::remove_file(path));
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
        EVENT_CONDITION_COMMENTED => render_condition_lifecycle_comment(record),
        EVENT_CONDITION_RESET => render_condition_lifecycle_ack(record, "ConditionReset"),
        EVENT_CONDITION_PRIORITY_CHANGED => render_condition_lifecycle_priority(record),
        EVENT_RECIPE_LOADED => render_recipe_loaded(record),
        EVENT_RECIPE_APPROVED => render_recipe_approved(record),
        EVENT_BATCH_EVENT => render_batch_event(record),
        EVENT_MATERIAL_ADDITION => render_material_addition(record),
        EVENT_OPERATOR_ACTION => render_operator_action(record),
        EVENT_OPERATOR_LOGIN => render_operator_login(record),
        EVENT_OPERATOR_LOGOUT => render_operator_logout(record),
        EVENT_SECURITY_ACCESS_FAILURE => render_security_access_failure(record),
        EVENT_PARAMETER_CHANGE => render_parameter_change(record),
        EVENT_ESIGNATURE => render_e_signature(record),
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

fn render_condition_lifecycle_comment(record: &Record) -> String {
    let ack_by = slot(record, KEY_ACK_BY)
        .map(|_| format!(" ackBy={}", required_string(record, KEY_ACK_BY)))
        .unwrap_or_default();
    format!(
        "ConditionCommented source={} seq={} conditionId={} correlation={} comment={}{}",
        record.source_id,
        record.seq,
        required_udint(record, KEY_CONDITION_ID),
        required_udint(record, KEY_CORRELATION_ID),
        required_string(record, KEY_COMMENT),
        ack_by
    )
}

fn render_condition_lifecycle_priority(record: &Record) -> String {
    let ack_by = slot(record, KEY_ACK_BY)
        .map(|_| format!(" ackBy={}", required_string(record, KEY_ACK_BY)))
        .unwrap_or_default();
    format!(
        "ConditionPriorityChanged source={} seq={} conditionId={} previousPriority={} newPriority={}{}",
        record.source_id,
        record.seq,
        required_udint(record, KEY_CONDITION_ID),
        required_uint(record, KEY_PREVIOUS_PRIORITY),
        required_uint(record, KEY_NEW_PRIORITY),
        ack_by
    )
}

fn render_recipe_loaded(record: &Record) -> String {
    let batch_id = slot(record, KEY_BATCH_ID)
        .map(|_| format!(" batchId={}", required_udint(record, KEY_BATCH_ID)))
        .unwrap_or_default();
    format!(
        "RecipeLoaded source={} seq={} recipeId={} version={}{}",
        record.source_id,
        record.seq,
        required_udint(record, KEY_RECIPE_ID),
        required_string(record, KEY_RECIPE_VERSION),
        batch_id
    )
}

fn render_recipe_approved(record: &Record) -> String {
    let auth_result = slot(record, KEY_AUTH_RESULT)
        .map(|_| format!(" authResult={}", required_uint(record, KEY_AUTH_RESULT)))
        .unwrap_or_default();
    let ack_by = slot(record, KEY_ACK_BY)
        .map(|_| format!(" ackBy={}", required_string(record, KEY_ACK_BY)))
        .unwrap_or_default();
    format!(
        "RecipeApproved source={} seq={} recipeId={} version={}{}{}",
        record.source_id,
        record.seq,
        required_udint(record, KEY_RECIPE_ID),
        required_string(record, KEY_RECIPE_VERSION),
        auth_result,
        ack_by
    )
}

fn render_batch_event(record: &Record) -> String {
    let recipe_id = slot(record, KEY_RECIPE_ID)
        .map(|_| format!(" recipeId={}", required_udint(record, KEY_RECIPE_ID)))
        .unwrap_or_default();
    format!(
        "BatchEvent source={} seq={} batchId={} newState={}{}",
        record.source_id,
        record.seq,
        required_udint(record, KEY_BATCH_ID),
        required_uint(record, KEY_NEW_STATE),
        recipe_id
    )
}

fn render_material_addition(record: &Record) -> String {
    let unit = slot(record, KEY_UNIT)
        .map(|_| format!(" unit={}", required_uint(record, KEY_UNIT)))
        .unwrap_or_default();
    format!(
        "MaterialAddition source={} seq={} batchId={} materialId={} quantity={}{}",
        record.source_id,
        record.seq,
        required_udint(record, KEY_BATCH_ID),
        required_udint(record, KEY_MATERIAL_ID),
        required_lreal(record, KEY_QUANTITY),
        unit
    )
}

fn render_operator_action(record: &Record) -> String {
    let context_refs = record
        .slots
        .iter()
        .filter(|slot| slot.key == KEY_CONTEXT_REF)
        .map(|slot| {
            assert_eq!(slot.ty, TY_UDINT);
            u32::from_le_bytes(
                slot.payload
                    .as_slice()
                    .try_into()
                    .expect("contextRef payload width"),
            )
            .to_string()
        })
        .collect::<Vec<_>>();
    let context_refs = if context_refs.is_empty() {
        String::new()
    } else {
        format!(" contextRef=[{}]", context_refs.join(","))
    };
    let auth_result = slot(record, KEY_AUTH_RESULT)
        .map(|_| format!(" authResult={}", required_uint(record, KEY_AUTH_RESULT)))
        .unwrap_or_default();
    let workstation = slot(record, KEY_WORKSTATION)
        .map(|_| format!(" workstation={}", required_string(record, KEY_WORKSTATION)))
        .unwrap_or_default();
    format!(
        "OperatorAction source={} seq={} actionId={} actor={}{}{}{}",
        record.source_id,
        record.seq,
        required_udint(record, KEY_ACTION_ID),
        required_string(record, KEY_ACTOR),
        context_refs,
        auth_result,
        workstation
    )
}

fn render_operator_login(record: &Record) -> String {
    let workstation = slot(record, KEY_WORKSTATION)
        .map(|_| format!(" workstation={}", required_string(record, KEY_WORKSTATION)))
        .unwrap_or_default();
    let role = slot(record, KEY_ROLE)
        .map(|_| format!(" role={}", required_uint(record, KEY_ROLE)))
        .unwrap_or_default();
    format!(
        "OperatorLogin source={} seq={} actor={} authResult={}{}{}",
        record.source_id,
        record.seq,
        required_string(record, KEY_ACTOR),
        required_uint(record, KEY_AUTH_RESULT),
        workstation,
        role
    )
}

fn render_operator_logout(record: &Record) -> String {
    let workstation = slot(record, KEY_WORKSTATION)
        .map(|_| format!(" workstation={}", required_string(record, KEY_WORKSTATION)))
        .unwrap_or_default();
    format!(
        "OperatorLogout source={} seq={} actor={}{}",
        record.source_id,
        record.seq,
        required_string(record, KEY_ACTOR),
        workstation
    )
}

fn render_security_access_failure(record: &Record) -> String {
    let workstation = slot(record, KEY_WORKSTATION)
        .map(|_| format!(" workstation={}", required_string(record, KEY_WORKSTATION)))
        .unwrap_or_default();
    let reason = slot(record, KEY_REASON)
        .map(|_| format!(" reason={}", required_string(record, KEY_REASON)))
        .unwrap_or_default();
    format!(
        "SecurityAccessFailure source={} seq={} actor={}{}{}",
        record.source_id,
        record.seq,
        required_string(record, KEY_ACTOR),
        workstation,
        reason
    )
}

fn render_parameter_change(record: &Record) -> String {
    let auth_result = slot(record, KEY_AUTH_RESULT)
        .map(|_| format!(" authResult={}", required_uint(record, KEY_AUTH_RESULT)))
        .unwrap_or_default();
    format!(
        "ParameterChange source={} seq={} valueId={} previous={} new={} actor={} reason={}{}",
        record.source_id,
        record.seq,
        required_udint(record, KEY_VALUE_ID),
        required_value(record, KEY_PREVIOUS_VALUE),
        required_value(record, KEY_NEW_VALUE),
        required_string(record, KEY_ACTOR),
        required_string(record, KEY_REASON),
        auth_result
    )
}

fn render_e_signature(record: &Record) -> String {
    let auth_result = slot(record, KEY_AUTH_RESULT)
        .map(|_| format!(" authResult={}", required_uint(record, KEY_AUTH_RESULT)))
        .unwrap_or_default();
    format!(
        "ESignature source={} seq={} actionId={} actor={} signatureMeaning={} signedEventSeq={}{}",
        record.source_id,
        record.seq,
        required_udint(record, KEY_ACTION_ID),
        required_string(record, KEY_ACTOR),
        required_uint(record, KEY_SIGNATURE_MEANING),
        required_ulint(record, KEY_SIGNED_EVENT_SEQ),
        auth_result
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

fn required_lreal(record: &Record, key: u16) -> f64 {
    let slot = slot(record, key).unwrap_or_else(|| panic!("missing slot 0x{key:04X}"));
    assert_eq!(slot.ty, TY_LREAL);
    f64::from_le_bytes(
        slot.payload
            .as_slice()
            .try_into()
            .expect("LREAL payload width"),
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
    reactor_dir: &std::path::Path,
    records: &[ReadRecord],
    audit_log: &[String],
    overflow: &OverflowSummary,
    reactor_source: &str,
) -> std::io::Result<()> {
    std::fs::create_dir_all(reactor_dir)?;

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

    write_reactor_readme(reactor_dir, &text)?;

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
