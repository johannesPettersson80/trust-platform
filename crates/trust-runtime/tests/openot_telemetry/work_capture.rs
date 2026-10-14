use super::*;
use trust_runtime::harness::CompileSession;
use trust_runtime::runtime_core::vm::PreparationLimits;

const DIAGNOSTIC_MAX_WORK: usize = 10_000_000;

pub(super) fn parameter_change_program() -> &'static str {
    r#"
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
"#
}

pub(super) fn assert_parameter_change_round_trip(
    runtime: &mut Runtime,
    path: PathBuf,
    program: &str,
    capture_work: bool,
) {
    runtime
        .configure_openot_telemetry(
            &st_fb_telemetry_config_for(path.clone(), 4096, "Main.OotProducer"),
            None,
        )
        .expect("configure OpenOT ST FB telemetry");

    let store = SharedConcurrentStore::open_existing(&path).expect("open telemetry shm");
    let mut consumer = ConcurrentRawConsumer::with_store(store);

    let seed_scan = if capture_work {
        capture_openot_operation("parameter_change_seed_scan_10m", true, || {
            runtime.execute_cycle()
        })
    } else {
        with_work_charge_log("seed_scan", || runtime.execute_cycle())
    };
    seed_scan.expect("first audited value cycle only seeds baselines");
    let batch = consumer.poll().expect("poll seeded audited values");
    assert_eq!(batch.records.len(), 0);

    let change_scan = if capture_work {
        capture_openot_operation("parameter_change_change_scan_10m", true, || {
            runtime.execute_cycle()
        })
    } else {
        with_work_charge_log("change_scan", || runtime.execute_cycle())
    };
    change_scan.expect("second audited value cycle publishes changes");
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

fn capture_openot_operation<T, E: std::fmt::Debug>(
    operation: &str,
    require_work: bool,
    action: impl FnOnce() -> Result<T, E>,
) -> Result<T, E> {
    use trust_runtime_core::vm::diagnostic_timing::{
        finish_callsite_accounting_log, start_callsite_accounting_log,
    };

    start_callsite_accounting_log();
    let result = action();
    let snapshot = finish_callsite_accounting_log();
    let thread = std::thread::current();
    let test = thread.name().unwrap_or("unnamed");
    let consumed_work: u128 = snapshot.work[..snapshot.work_len]
        .iter()
        .map(|row| u128::from(row.consumed_units))
        .sum();
    let outcome = result
        .as_ref()
        .map(|_| ())
        .map_err(|error| format!("{error:?}"));
    eprintln!(
        "openot-work-capture test={test:?} operation={operation:?} outcome={outcome:?} work_len={} copy_len={} lifecycle_len={} index_lookup_len={} consumed_work={consumed_work} overflow_count={} recorder_busy={} valid={}",
        snapshot.work_len,
        snapshot.copy_len,
        snapshot.lifecycle_len,
        snapshot.index_lookup_len,
        snapshot.overflow_count,
        snapshot.recorder_busy,
        snapshot.is_valid(),
    );
    eprintln!("openot-attribution-header test={test:?} operation={operation:?} owner_query_len={} array_consumer_len={}", snapshot.owner_queries.len(), snapshot.array_consumers.len());
    for row in &snapshot.owner_queries {
        eprintln!("openot-owner-query-row test={test:?} operation={operation:?} {row:?}");
    }
    for row in &snapshot.array_consumers {
        eprintln!("openot-array-consumer-row test={test:?} operation={operation:?} {row:?}");
    }
    for row in &snapshot.work[..snapshot.work_len] {
        eprintln!("openot-work-capture-row test={test:?} operation={operation:?} {row:?}");
    }
    for row in &snapshot.index_lookups[..snapshot.index_lookup_len] {
        eprintln!("openot-index-capture-row test={test:?} operation={operation:?} {row:?}");
    }
    for row in &snapshot.lifecycle[..snapshot.lifecycle_len] {
        eprintln!("openot-lifecycle-capture-row test={test:?} operation={operation:?} {row:?}");
    }
    for row in &snapshot.copies[..snapshot.copy_len] {
        eprintln!(
            "openot-copy-capture-row test={test:?} operation={operation:?} variant={} {row:?}",
            row.value_variant.name(),
        );
    }
    assert!(
        snapshot.is_valid(),
        "OpenOT work capture is incomplete for {operation}: overflow_count={}, recorder_busy={}, outcome={outcome:?}",
        snapshot.overflow_count,
        snapshot.recorder_busy,
    );
    if require_work {
        assert!(
            snapshot.work_len > 0,
            "OpenOT work capture has no work rows for {operation}: outcome={outcome:?}",
        );
    }
    result
}

fn capture_runtime(
    bytes: &[u8],
    operation: &str,
    max_work: usize,
) -> Result<Runtime, trust_runtime::error::RuntimeError> {
    let limits = PreparationLimits {
        max_work,
        ..PreparationLimits::default()
    };
    let result = capture_openot_operation(
        operation,
        max_work > PreparationLimits::default().max_work,
        || Runtime::from_bytes(bytes, limits),
    );
    let outcome = result
        .as_ref()
        .map(|_| ())
        .map_err(|error| format!("{error:?}"));
    let artifact_hash = trust_runtime::ads::diagnostics::sha256_evidence_hash(bytes);
    eprintln!(
        "openot-work-capture operation={operation} artifact_sha256={artifact_hash} bytes={} max_work={max_work} outcome={outcome:?}",
        bytes.len(),
    );
    result
}

#[test]
#[ignore = "opt-in complete OpenOT database-constructor work capture at default and diagnostic limits"]
fn openot_database_constructor_work_charges_default_and_diagnostic_limits() {
    let program = include_str!("../../../../examples/openot_multi_program/src/Main.st");
    let sources = openot_support::openot_st_source_files(program);
    let session = CompileSession::from_sources(sources).label_errors(true);
    let bytes = session
        .build_bytecode_bytes()
        .expect("build the existing OpenOT database artifact once");
    retain_work_capture_artifact("database-constructor", &bytes);
    let artifact_hash = trust_runtime::ads::diagnostics::sha256_evidence_hash(&bytes);
    eprintln!(
        "openot-work-capture operation=database_constructor artifact_sha256={artifact_hash} bytes={} default_max_work={} diagnostic_max_work={DIAGNOSTIC_MAX_WORK}",
        bytes.len(),
        PreparationLimits::default().max_work,
    );

    let default_result = capture_runtime(
        &bytes,
        "database_constructor_default_1m",
        PreparationLimits::default().max_work,
    );
    drop(default_result);

    let diagnostic_result = capture_runtime(
        &bytes,
        "database_constructor_diagnostic_10m",
        DIAGNOSTIC_MAX_WORK,
    );
    diagnostic_result
        .expect("complete the same OpenOT database artifact at the diagnostic work ceiling");
}

#[test]
#[ignore = "opt-in complete OpenOT parameter-change work capture at default and diagnostic limits"]
fn openot_parameter_change_work_charges_default_and_diagnostic_limits() {
    let program = parameter_change_program();
    let session = CompileSession::from_sources(openot_support::openot_st_source_files(program))
        .label_errors(true);
    let bytes = session
        .build_bytecode_bytes()
        .expect("build the existing OpenOT parameter-change artifact once");
    retain_work_capture_artifact("parameter-change", &bytes);
    let artifact_hash = trust_runtime::ads::diagnostics::sha256_evidence_hash(&bytes);
    eprintln!(
        "openot-work-capture operation=parameter_change artifact_sha256={artifact_hash} bytes={} default_max_work={} diagnostic_max_work={DIAGNOSTIC_MAX_WORK}",
        bytes.len(),
        PreparationLimits::default().max_work,
    );

    let default_result = capture_runtime(
        &bytes,
        "parameter_change_default_1m",
        PreparationLimits::default().max_work,
    );
    drop(default_result);

    let mut runtime = capture_runtime(
        &bytes,
        "parameter_change_diagnostic_10m",
        DIAGNOSTIC_MAX_WORK,
    )
    .expect("complete the same parameter-change artifact at the diagnostic work ceiling");
    capture_openot_operation("parameter_change_install_diagnostic_10m", true, || {
        runtime.apply_bytecode_bytes(&bytes, None)
    })
    .expect("install the same saved bytecode on the diagnostic runtime");
    assert_parameter_change_round_trip(
        &mut runtime,
        temp_shm_path("authoring-parameter-change-diagnostic"),
        program,
        true,
    );
}

fn retain_work_capture_artifact(name: &str, bytes: &[u8]) {
    let Some(directory) = std::env::var_os("TRUST_WORK_CAPTURE_ARTIFACT_DIR") else {
        return;
    };
    let directory = PathBuf::from(directory);
    std::fs::create_dir_all(&directory).expect("create diagnostic artifact directory");
    let path = directory.join(format!("{name}.stbc"));
    std::fs::write(&path, bytes).expect("retain the exact diagnostic artifact");
    let module = trust_runtime::bytecode::BytecodeModule::decode(bytes)
        .expect("decode the same retained diagnostic artifact");
    let tables = format!(
        "POU_INDEX {:#?}\nSTRING_TABLE {:#?}\n",
        module.section(trust_runtime::bytecode::SectionId::PouIndex),
        module.section(trust_runtime::bytecode::SectionId::StringTable)
    );
    std::fs::write(directory.join(format!("{name}.tables.txt")), tables)
        .expect("retain numeric identity tables");
    let hash = trust_runtime::ads::diagnostics::sha256_evidence_hash(bytes);
    eprintln!(
        "work-capture-artifact path={path:?} artifact_sha256={hash} bytes={}",
        bytes.len()
    );
}
