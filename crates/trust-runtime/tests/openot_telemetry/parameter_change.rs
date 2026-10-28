//! The audited parameter-change round trip: four audited values change in one scan and publish
//! one `ParameterChange` record each, which the definition resolves into documents.

use super::*;

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
    runtime: &mut CompiledRuntime,
    path: PathBuf,
    program: &str,
) {
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
