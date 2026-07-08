use std::collections::BTreeSet;
use std::path::Path;

use serde_json::Value;
use trust_hir::db::SemanticDatabase;
use trust_hir::{Project, SourceKey};
use trust_syntax::parser;

use super::*;

fn explicit_sourceid_collision_source() -> SourceFile {
    SourceFile::with_path(
            "main.st",
            "PROGRAM First\nVAR\n    Start : BOOL {attribute 'oot' := 'message', 'sourceid' := '77', 'template' := 'first'};\nEND_VAR\nEND_PROGRAM\nPROGRAM Second\nVAR\n    Start : BOOL {attribute 'oot' := 'message', 'sourceid' := '77', 'template' := 'second'};\nEND_VAR\nEND_PROGRAM\n",
        )
}

fn hir_openot_error_messages(source: &SourceFile) -> Vec<String> {
    let parse = parser::parse(&source.text);
    let mut diagnostics = hir_openot::collect_openot_attribute_diagnostics(&parse.syntax());
    let mut project = Project::new();
    let key = match source.path.as_deref() {
        Some(path) => SourceKey::from_path(Path::new(path)),
        None => SourceKey::from_virtual("openot_authoring_test"),
    };
    let file_id = project.set_source_text(key, source.text.clone());
    let analysis = project.database().analyze(file_id);
    diagnostics.extend(hir_openot::collect_openot_semantic_diagnostics(
        &parse.syntax(),
        analysis.symbols.as_ref(),
        analysis.declaration_catalog.as_ref(),
        file_id,
    ));
    diagnostics
        .into_iter()
        .filter(|diagnostic| diagnostic.is_error())
        .map(|diagnostic| diagnostic.message)
        .collect()
}

#[test]
fn instruments_simple_program_attributes() {
    let source = SourceFile::with_path(
            "main.st",
            "PROGRAM Main\nVAR\n    Level : REAL {attribute 'oot' := 'value', 'deadband' := '0.5'};\nEND_VAR\nLevel := REAL#1.0;\nEND_PROGRAM\n",
        );

    let instrumented = instrument_source_files(&[source]);
    assert!(instrumented[0]
        .text
        .contains("OotProducer : OPENOT_Producer"));
    assert!(instrumented[0]
        .text
        .contains("OotUseSourceTimeInput : BOOL := FALSE;"));
    assert!(instrumented[0]
        .text
        .contains("OotSourceTime : ULINT := ULINT#0;"));
    assert!(instrumented[0].text.contains("Op := UINT#6"));
    assert!(instrumented[0]
        .text
        .contains("UseSourceTimeInput := OotUseSourceTimeInput"));
    assert!(instrumented[0]
        .text
        .contains("SourceTimeInput := OotSourceTime"));
    assert!(instrumented[0].text.contains("ValueReal := Level"));
}

#[test]
fn generated_definition_contains_hash() {
    let source = SourceFile::with_path(
            "main.st",
            "PROGRAM Main\nVAR\n    Level : REAL {attribute 'oot' := 'value', 'unit' := 'L'};\nEND_VAR\nEND_PROGRAM\n",
        );
    let definition = definition_json_from_sources(&[source]).expect("definition");
    let hash = definition["header"]["contentHash"].as_str().unwrap();
    assert_eq!(hash.len(), 64);
    assert_eq!(
        definition["header"]["constraints"]["maxRecordSize"],
        ST_PRODUCER_MAX_RECORD_SIZE
    );
    assert_eq!(definition["values"][0]["name"], "Level");
    assert_eq!(definition["sources"][0]["name"], "main.Main");
    assert_eq!(
        definition["sources"][0]["path"],
        serde_json::json!(["main", "Main"])
    );
    assert_eq!(
        definition["sources"][0]["hierarchy"],
        serde_json::json!(["file", "program"])
    );
    assert_eq!(definition["sources"][0]["sourceId"], DEFAULT_SOURCE_ID);
}

#[test]
fn multi_program_omitted_sourceids_are_assigned_distinctly() {
    let source = SourceFile::with_path(
            "main.st",
            "PROGRAM First\nVAR\n    Start : BOOL {attribute 'oot' := 'message', 'template' := 'first'};\nEND_VAR\nEND_PROGRAM\nPROGRAM Second\nVAR\n    Start : BOOL {attribute 'oot' := 'message', 'template' := 'second'};\nEND_VAR\nEND_PROGRAM\n",
        );
    let definition = definition_json_from_sources(&[source]).expect("definition");
    let source_ids = definition["sources"]
        .as_array()
        .expect("sources")
        .iter()
        .map(|source| {
            (
                source["sourceId"].as_u64().expect("sourceId"),
                source["name"].as_str().expect("source name").to_string(),
            )
        })
        .collect::<Vec<_>>();

    assert_eq!(
        source_ids,
        [
            (u64::from(DEFAULT_SOURCE_ID), "main.First".to_string()),
            (u64::from(DEFAULT_SOURCE_ID + 1), "main.Second".to_string()),
        ]
    );
}

#[test]
fn explicit_sourceid_collision_between_programs_is_rejected() {
    let source = explicit_sourceid_collision_source();
    let err = definition_json_from_sources(&[source]).expect_err("source collision");

    assert!(err.contains("OpenOT sourceid 77 is used by both 'main.First' and 'main.Second'"));
}

#[test]
#[ignore = "red test for runtime-safety Phase 11 SEAM-TEST-014"]
fn hir_and_runtime_authoring_report_explicit_sourceid_collisions_consistently() {
    let source = explicit_sourceid_collision_source();
    let hir_errors = hir_openot_error_messages(&source);
    let runtime_errors = validate_authoring_sources(std::slice::from_ref(&source));

    assert!(
        runtime_errors
            .iter()
            .any(|error| error.contains("OpenOT sourceid 77 is used by both")),
        "runtime authoring must report sourceid ownership collision, got {runtime_errors:?}"
    );
    assert!(
            hir_errors
                .iter()
                .any(|error| error.contains("OpenOT sourceid 77 is used by both")),
            "HIR OpenOT diagnostics must report the same sourceid ownership collision as runtime authoring; \
             hir_errors={hir_errors:?}; runtime_errors={runtime_errors:?}"
        );
}

#[test]
fn generated_definition_uses_canonical_unit_ids() {
    let source = SourceFile::with_path(
            "main.st",
            "PROGRAM Main\nVAR\n    Level : REAL {attribute 'oot' := 'value', 'unit' := 'L'};\n    Temp : REAL {attribute 'oot' := 'value', 'unit' := 'degC'};\nEND_VAR\nEND_PROGRAM\n",
        );
    let definition = definition_json_from_sources(&[source]).expect("definition");
    assert_eq!(definition["values"][0]["unit"], 2);
    assert_eq!(definition["values"][1]["unit"], 3);
    let units = definition["units"].as_array().expect("units");
    assert!(units
        .iter()
        .any(|unit| unit["unitId"] == 2 && unit["symbol"] == "L"));
    assert!(units
        .iter()
        .any(|unit| unit["unitId"] == 3 && unit["symbol"] == "degC"));
}

#[test]
fn hir_and_runtime_authoring_accept_omitted_sourceids_units_and_payload_widths() {
    let source = SourceFile::with_path(
            "main.st",
            "PROGRAM First\nVAR\n    Level : REAL {attribute 'oot' := 'value', 'unit' := 'degC'};\nEND_VAR\nEND_PROGRAM\nPROGRAM Second\nVAR\n    Count : DINT {attribute 'oot' := 'value'};\n    Total : ULINT {attribute 'oot' := 'value'};\n    Label : STRING[16] {attribute 'oot' := 'value'};\nEND_VAR\nEND_PROGRAM\n",
        );

    let hir_errors = hir_openot_error_messages(&source);
    let runtime_errors = validate_authoring_sources(std::slice::from_ref(&source));
    assert!(
            hir_errors.is_empty(),
            "HIR OpenOT diagnostics must accept valid omitted sourceid/unit/payload fixture, got {hir_errors:?}"
        );
    assert!(
            runtime_errors.is_empty(),
            "runtime OpenOT authoring must accept valid omitted sourceid/unit/payload fixture, got {runtime_errors:?}"
        );

    let definition =
        definition_json_from_sources(std::slice::from_ref(&source)).expect("definition");
    assert_eq!(definition["sources"][0]["sourceId"], 1);
    assert_eq!(definition["sources"][1]["sourceId"], 2);
    assert_eq!(definition["values"][0]["unit"], 3);
    assert!(definition["units"]
        .as_array()
        .expect("units")
        .iter()
        .any(|unit| unit["unitId"] == 3 && unit["symbol"] == "degC"));

    let instrumented = instrument_source_files(&[source]);
    let text = &instrumented[0].text;
    assert!(text.contains("SourceId := UDINT#1"), "{text}");
    assert!(text.contains("SourceId := UDINT#2"), "{text}");
    assert!(text.contains("ValueInt := Count"), "{text}");
    assert!(text.contains("ValueTypeTag := BYTE#16#07"), "{text}");
    assert!(text.contains("ValuePayloadLength := UINT#8"), "{text}");
    assert!(text.contains("ValueBits := Total"), "{text}");
    assert!(text.contains("ValueString := Label"), "{text}");
}

#[test]
#[ignore = "red test for runtime-safety Phase 11 SEAM-TEST-015"]
fn compile_session_surfaces_openot_validation_failure_instead_of_building_uninstrumented_bytecode()
{
    let source = explicit_sourceid_collision_source();
    let instrumented = instrument_source_files(std::slice::from_ref(&source));
    let bytecode =
        crate::harness::CompileSession::from_sources(vec![source.clone()]).build_bytecode_bytes();
    let bytecode_len = bytecode.as_ref().map(Vec::len);

    assert!(
            bytecode.is_err(),
            "OpenOT validation failure must surface through CompileSession instead of building uninstrumented bytecode; \
             bytecode_len={bytecode_len:?}; unchanged_source={}; contains_producer={}",
            instrumented[0].text == source.text,
            instrumented[0].text.contains(PRODUCER_NAME)
        );
}

#[test]
fn value_quality_semantic_role_and_previous_lowering_are_explicit() {
    let source = SourceFile::with_path(
            "main.st",
            "PROGRAM Main\nVAR\n    Setpoint : REAL {attribute 'oot' := 'value', 'quality' := 'uncertain', 'semanticRole' := 'setpoint', 'previous' := 'false'};\nEND_VAR\nSetpoint := REAL#10.0;\nEND_PROGRAM\n",
        );
    let definition =
        definition_json_from_sources(std::slice::from_ref(&source)).expect("definition");
    assert_eq!(definition["values"][0]["semanticRole"], 1);

    let instrumented = instrument_source_files(&[source]);
    let text = &instrumented[0].text;
    assert!(text.contains("SuppressPrevious := TRUE"), "{text}");
    assert!(text.contains("HasQuality := TRUE"), "{text}");
    assert!(text.contains("Quality := UINT#1"), "{text}");
}

#[test]
fn value_sampling_policy_lowering_uses_existing_definition_field() {
    let source = SourceFile::with_path(
            "main.st",
            "PROGRAM Main\nVAR\n    Pressure : REAL {attribute 'oot' := 'value', 'sampling' := 'periodic', 'interval' := '250'};\n    Flow : REAL {attribute 'oot' := 'value', 'sampling' := 'hysteresis', 'deadband' := '1.5'};\n    Level : REAL {attribute 'oot' := 'value', 'deadband' := '0.5'};\nEND_VAR\nEND_PROGRAM\n",
        );
    let definition =
        definition_json_from_sources(std::slice::from_ref(&source)).expect("definition");
    assert_eq!(definition["values"][0]["samplingPolicy"], "periodic:250");
    assert_eq!(definition["values"][1]["samplingPolicy"], "hysteresis");
    assert_eq!(definition["values"][2]["samplingPolicy"], Value::Null);

    let instrumented = instrument_source_files(&[source]);
    let text = &instrumented[0].text;
    assert!(text.contains("SamplingMode := UINT#1"), "{text}");
    assert!(text.contains("SamplingIntervalMs := ULINT#250"), "{text}");
    assert!(text.contains("SamplingMode := UINT#2"), "{text}");
}

#[test]
fn condition_lifecycle_lowering_inherits_parent_and_emits_after_alarm_phase() {
    let source = SourceFile::with_path(
            "main.st",
            "PROGRAM Main\nVAR\n    OperatorName : STRING[32] := 'operator-a';\n    ReasonText : STRING[32] := 'maintenance';\n    CommentText : STRING[32] := 'operator comment';\n    ShelveSecs : UDINT := UDINT#300;\n    PreviousPriority : UINT := UINT#600;\n    NewPriority : UINT := UINT#900;\n    AckHighPh : BOOL {attribute 'oot' := 'condition', 'of' := HighPhAlarm, 'event' := 'acknowledge', 'by' := OperatorName};\n    HighPhAlarm : BOOL {attribute 'oot' := 'alarm', 'sourceid' := '77', 'conditionid' := '9101'};\n    ConfirmHighPh : BOOL {attribute 'oot' := 'condition', 'of' := HighPhAlarm, 'event' := 'confirm', 'by' := OperatorName};\n    ShelveHighPh : BOOL {attribute 'oot' := 'condition', 'of' := HighPhAlarm, 'event' := 'shelve', 'by' := OperatorName, 'seconds' := ShelveSecs};\n    UnshelveHighPh : BOOL {attribute 'oot' := 'condition', 'of' := HighPhAlarm, 'event' := 'unshelve'};\n    SuppressHighPh : BOOL {attribute 'oot' := 'condition', 'of' := HighPhAlarm, 'event' := 'suppress', 'reason' := ReasonText};\n    UnsuppressHighPh : BOOL {attribute 'oot' := 'condition', 'of' := HighPhAlarm, 'event' := 'unsuppress'};\n    OosHighPh : BOOL {attribute 'oot' := 'condition', 'of' := HighPhAlarm, 'event' := 'out-of-service', 'by' := OperatorName};\n    InServiceHighPh : BOOL {attribute 'oot' := 'condition', 'of' := HighPhAlarm, 'event' := 'in-service'};\n    ResetHighPh : BOOL {attribute 'oot' := 'condition', 'of' := HighPhAlarm, 'event' := 'reset', 'by' := OperatorName};\n    CommentHighPh : BOOL {attribute 'oot' := 'condition', 'of' := HighPhAlarm, 'event' := 'comment', 'comment' := CommentText, 'by' := OperatorName};\n    PriorityHighPh : BOOL {attribute 'oot' := 'condition', 'of' := HighPhAlarm, 'event' := 'priority-changed', 'previous-priority' := PreviousPriority, 'new-priority' := NewPriority, 'by' := OperatorName};\nEND_VAR\nHighPhAlarm := TRUE;\nAckHighPh := TRUE;\nConfirmHighPh := TRUE;\nShelveHighPh := TRUE;\nUnshelveHighPh := TRUE;\nSuppressHighPh := TRUE;\nUnsuppressHighPh := TRUE;\nOosHighPh := TRUE;\nInServiceHighPh := TRUE;\nResetHighPh := TRUE;\nCommentHighPh := TRUE;\nPriorityHighPh := TRUE;\nEND_PROGRAM\n",
        );
    let definition =
        definition_json_from_sources(std::slice::from_ref(&source)).expect("definition");
    assert_eq!(definition["conditions"].as_array().unwrap().len(), 1);
    assert_eq!(definition["conditions"][0]["conditionId"], 9101);

    let instrumented = instrument_source_files(&[source]);
    let text = &instrumented[0].text;
    let alarm_pos = text.find("Op := UINT#9").expect("alarm op");
    let lifecycle_pos = text.find("Op := UINT#12").expect("lifecycle op");
    assert!(
        alarm_pos < lifecycle_pos,
        "alarm updates must emit before lifecycle commands:\n{text}"
    );
    assert!(text.contains("SourceId := UDINT#77"), "{text}");
    assert!(text.contains("ConditionId := UDINT#9101"), "{text}");
    assert!(
        text.contains("ConditionLifecycleEventTypeId := UDINT#16#0202"),
        "{text}"
    );
    assert!(
        text.contains("ConditionLifecycleEventTypeId := UDINT#16#0203"),
        "{text}"
    );
    assert!(
        text.contains("ConditionLifecycleEventTypeId := UDINT#16#0204"),
        "{text}"
    );
    assert!(
        text.contains("ConditionLifecycleEventTypeId := UDINT#16#0205"),
        "{text}"
    );
    assert!(
        text.contains("ConditionLifecycleEventTypeId := UDINT#16#0206"),
        "{text}"
    );
    assert!(
        text.contains("ConditionLifecycleEventTypeId := UDINT#16#0207"),
        "{text}"
    );
    assert!(
        text.contains("ConditionLifecycleEventTypeId := UDINT#16#0208"),
        "{text}"
    );
    assert!(
        text.contains("ConditionLifecycleEventTypeId := UDINT#16#0209"),
        "{text}"
    );
    assert!(
        text.contains("ConditionLifecycleEventTypeId := UDINT#16#020A"),
        "{text}"
    );
    assert!(
        text.contains("ConditionLifecycleEventTypeId := UDINT#16#020B"),
        "{text}"
    );
    assert!(
        text.contains("ConditionLifecycleEventTypeId := UDINT#16#020C"),
        "{text}"
    );
    assert!(text.contains("LifecycleAckBy := OperatorName"), "{text}");
    assert!(text.contains("LifecycleShelveSecs := ShelveSecs"), "{text}");
    assert!(text.contains("LifecycleReason := ReasonText"), "{text}");
    assert!(text.contains("LifecycleComment := CommentText"), "{text}");
    assert!(
        text.contains("LifecyclePreviousPriority := PreviousPriority"),
        "{text}"
    );
    assert!(
        text.contains("LifecycleNewPriority := NewPriority"),
        "{text}"
    );
}

#[test]
fn batch_recipe_lowering_uses_procedure_op_and_raw_definition_tables() {
    let source = SourceFile::with_path(
            "main.st",
            "TYPE E_BatchState : (Started := 0, Completed := 1, Held := 2, Resumed := 3, Aborted := 4, Paused := 5) END_TYPE\nPROGRAM Main\nVAR\n    RecipeId : UDINT := UDINT#3001;\n    RecipeVersion : STRING[16] := 'v1.2.3';\n    BatchId : UDINT := UDINT#4001;\n    MaterialId : UDINT := UDINT#5001;\n    Quantity : LREAL := LREAL#12.25;\n    AuthResult : UINT := UINT#1;\n    Approver : STRING[32] := 'approver-a';\n    State : E_BatchState := Started {attribute 'oot' := 'batch', 'batchId' := BatchId, 'recipe' := RecipeId};\n    Loaded : BOOL {attribute 'oot' := 'recipe-loaded', 'recipe' := RecipeId, 'version' := RecipeVersion, 'batch' := BatchId};\n    Approved : BOOL {attribute 'oot' := 'recipe-approved', 'recipe' := RecipeId, 'version' := RecipeVersion, 'auth' := AuthResult, 'by' := Approver};\n    Addition : BOOL {attribute 'oot' := 'material-addition', 'batch' := BatchId, 'material' := MaterialId, 'quantity' := Quantity, 'unit' := 'kg'};\nEND_VAR\nLoaded := TRUE;\nApproved := TRUE;\nAddition := TRUE;\nState := Held;\nEND_PROGRAM\n",
        );

    let definition =
        definition_json_from_sources(std::slice::from_ref(&source)).expect("definition");
    assert_eq!(definition["recipeDefinitions"], Value::Array(vec![]));
    assert_eq!(definition["batchDefinitions"], Value::Array(vec![]));
    assert_eq!(definition["materialDefinitions"], Value::Array(vec![]));

    let instrumented = instrument_source_files(&[source]);
    let text = &instrumented[0].text;
    assert!(text.contains("Op := UINT#13"), "{text}");
    assert!(
        text.contains("ProcedureEventTypeId := UDINT#16#0303"),
        "{text}"
    );
    assert!(text.contains("ProcedureHasRecipeId := TRUE"), "{text}");
    assert!(
        text.contains("ProcedureNewState := OotBatchStateNew_State"),
        "{text}"
    );
    assert!(text.contains("ProcedureQuantity := Quantity"), "{text}");
    assert!(text.contains("ProcedureUnitId := UINT#8"), "{text}");
    assert!(!text.contains("StateMachineId"), "{text}");
}

#[test]
fn operator_regulated_lowering_uses_regulated_op_and_auth_symbols() {
    let source = SourceFile::with_path(
            "main.st",
            "PROGRAM Main\nVAR\n    ActionId : UDINT := UDINT#6001;\n    ContextA : UDINT := UDINT#7001;\n    ContextB : UDINT := UDINT#7002;\n    Actor : STRING[32] := 'operator-a';\n    Workstation : STRING[32] := 'station-1';\n    Role : UINT := UINT#3;\n    Reason : STRING[32] := 'denied';\n    ActionTrigger : BOOL {attribute 'oot' := 'operator-action', 'action' := ActionId, 'actor' := Actor, 'context1' := ContextA, 'context2' := ContextB, 'auth' := 'Granted', 'workstation' := Workstation};\n    Login : BOOL {attribute 'oot' := 'operator-login', 'actor' := Actor, 'auth' := 'Denied', 'workstation' := Workstation, 'role' := Role};\n    Logout : BOOL {attribute 'oot' := 'operator-logout', 'actor' := Actor, 'workstation' := Workstation};\n    Failure : BOOL {attribute 'oot' := 'security-failure', 'actor' := Actor, 'workstation' := Workstation, 'reason' := Reason};\nEND_VAR\nActionTrigger := TRUE;\nLogin := TRUE;\nLogout := TRUE;\nFailure := TRUE;\nEND_PROGRAM\n",
        );

    let definition =
        definition_json_from_sources(std::slice::from_ref(&source)).expect("definition");
    assert_eq!(definition["operatorDefinitions"], Value::Array(vec![]));

    let instrumented = instrument_source_files(&[source]);
    let text = &instrumented[0].text;
    assert!(text.contains("Op := UINT#14"), "{text}");
    assert!(
        text.contains("RegulatedEventTypeId := UDINT#16#0400"),
        "{text}"
    );
    assert!(
        text.contains("RegulatedEventTypeId := UDINT#16#0401"),
        "{text}"
    );
    assert!(
        text.contains("RegulatedEventTypeId := UDINT#16#0402"),
        "{text}"
    );
    assert!(
        text.contains("RegulatedEventTypeId := UDINT#16#0405"),
        "{text}"
    );
    assert!(text.contains("RegulatedActionId := ActionId"), "{text}");
    assert!(text.contains("RegulatedActor := Actor"), "{text}");
    assert!(text.contains("RegulatedHasContext1 := TRUE"), "{text}");
    assert!(text.contains("RegulatedContext1 := ContextA"), "{text}");
    assert!(text.contains("RegulatedHasContext2 := TRUE"), "{text}");
    assert!(text.contains("RegulatedContext2 := ContextB"), "{text}");
    assert!(text.contains("RegulatedAuthResult := UINT#0"), "{text}");
    assert!(text.contains("RegulatedAuthResult := UINT#1"), "{text}");
    assert!(
        text.contains("RegulatedWorkstation := Workstation"),
        "{text}"
    );
    assert!(text.contains("RegulatedRole := Role"), "{text}");
    assert!(text.contains("RegulatedHasReason := TRUE"), "{text}");
    assert!(text.contains("RegulatedReason := Reason"), "{text}");
}

#[test]
fn instruments_fixed_width_value_types_with_generic_value_op() {
    let source = SourceFile::with_path(
            "main.st",
            "PROGRAM Main\nVAR\n    Enabled : BOOL {attribute 'oot' := 'value'};\n    Total : ULINT {attribute 'oot' := 'value'};\n    Ratio : LREAL {attribute 'oot' := 'value'};\n    Label : STRING[16] {attribute 'oot' := 'value'};\nEND_VAR\nEnabled := TRUE;\nTotal := ULINT#10;\nRatio := LREAL#1.25;\nLabel := 'ready';\nEND_PROGRAM\n",
        );

    let instrumented = instrument_source_files(&[source]);
    let text = &instrumented[0].text;
    assert!(text.contains("Op := UINT#10"), "{text}");
    assert!(text.contains("ValueTypeTag := BYTE#16#00"), "{text}");
    assert!(text.contains("ValueTypeTag := BYTE#16#07"), "{text}");
    assert!(text.contains("ValueTypeTag := BYTE#16#0A"), "{text}");
    assert!(text.contains("ValuePayloadLength := UINT#8"), "{text}");
    assert!(text.contains("LREAL_TO_LWORD(Ratio)"), "{text}");
    assert!(text.contains("Op := UINT#11"), "{text}");
    assert!(text.contains("ValueString := Label"), "{text}");
}

#[test]
fn generated_definition_rejects_unsupported_value_types() {
    let source = SourceFile::with_path(
            "main.st",
            "PROGRAM Main\nVAR\n    WideText : WSTRING {attribute 'oot' := 'value'};\nEND_VAR\nEND_PROGRAM\n",
        );
    let err = definition_json_from_sources(&[source]).expect_err("unsupported values reject");
    assert!(err.contains("OpenOT value logging supports BOOL"));
}

#[test]
fn enum_state_lowering_uses_hir_enum_values() {
    let source = SourceFile::with_path(
            "main.st",
            "TYPE Phase : (Idle := 0, Fill := 10, Done := 20) END_TYPE\nPROGRAM Main\nVAR\n    ActiveStep : Phase {attribute 'oot' := 'state', 'category' := 'process'};\nEND_VAR\nActiveStep := Fill;\nEND_PROGRAM\n",
        );

    let instrumented = instrument_source_files(&[source]);
    let text = &instrumented[0].text;
    assert!(
        text.contains("OotPrev_ActiveStep : Phase := Phase#Idle;"),
        "{text}"
    );
    assert!(text.contains("OotStatePrev_ActiveStep : UINT := UINT#0;"));
    assert!(text.contains("IF ActiveStep = Phase#Fill THEN"));
    assert!(text.contains("OotStateNew_ActiveStep := UINT#10;"));
    assert!(text.contains("PreviousState := OotStatePrev_ActiveStep"));
    assert!(text.contains("NewState := OotStateNew_ActiveStep"));
}

#[test]
fn enum_state_definition_includes_enum_set() {
    let source = SourceFile::with_path(
            "main.st",
            "TYPE Phase : (Idle := 0, Running := 1, Complete := 2) END_TYPE\nPROGRAM Main\nVAR\n    ActiveStep : Phase {attribute 'oot' := 'state', 'category' := 'procedural', 'model' := 'ISA-88'};\nEND_VAR\nEND_PROGRAM\n",
        );

    let definition = definition_json_from_sources(&[source]).expect("definition");
    assert_eq!(definition["stateMachines"][0]["enumSet"], "Phase");
    assert_eq!(definition["enumSets"][0]["name"], "Phase");
    assert_eq!(definition["enumSets"][0]["members"][1]["label"], "Running");
    assert_eq!(definition["enumSets"][0]["members"][1]["value"], 1);
}

#[test]
fn state_category_defaults_to_process() {
    let source = SourceFile::with_path(
            "main.st",
            "TYPE Phase : (Idle := 0, Fill := 10, Done := 20) END_TYPE\nPROGRAM Main\nVAR\n    ActiveStep : Phase {attribute 'oot' := 'state'};\nEND_VAR\nEND_PROGRAM\n",
        );

    let definition = definition_json_from_sources(&[source]).expect("definition");
    assert_eq!(
        definition["stateMachines"][0]["category"],
        hir_openot::STATE_CATEGORY_PROCESS
    );
    assert_eq!(
        definition["stateMachines"][0]["proceduralModel"],
        Value::Null
    );
}

#[test]
fn generated_event_types_match_openot_reference_canonical_schema() {
    let source = SourceFile::with_path(
            "main.st",
            "TYPE Phase : (Idle := 0, Filling := 1) END_TYPE\nPROGRAM Main\nVAR\n    Step : Phase {attribute 'oot' := 'state', 'category' := 'process'};\n    Level : REAL {attribute 'oot' := 'value'};\n    Alarm : BOOL {attribute 'oot' := 'alarm'};\n    Started : BOOL {attribute 'oot' := 'message', 'template' := 'started'};\nEND_VAR\nEND_PROGRAM\n",
        );

    let definition = definition_json_from_sources(&[source]).expect("definition");
    for event in definition["eventTypes"].as_array().expect("eventTypes") {
        let parsed: open_ot_definition::model::EventTypeDefinition =
            serde_json::from_value(event.clone()).expect("generated event type shape");
        assert_eq!(
            parsed,
            open_ot_definition::model::canonical_event_type(parsed.id)
                .expect("canonical schema for emitted event")
        );
    }
}

#[test]
fn pinned_ids_are_stable_when_declarations_are_reordered() {
    let first = SourceFile::with_path(
            "main.st",
            "PROGRAM Main\nVAR\n    Level : REAL {attribute 'oot' := 'value', 'id' := '2201'};\n    BatchCount : DINT {attribute 'oot' := 'value', 'id' := '2202'};\nEND_VAR\nEND_PROGRAM\n",
        );
    let second = SourceFile::with_path(
            "main.st",
            "PROGRAM Main\nVAR\n    BatchCount : DINT {attribute 'oot' := 'value', 'id' := '2202'};\n    Level : REAL {attribute 'oot' := 'value', 'id' := '2201'};\nEND_VAR\nEND_PROGRAM\n",
        );

    let first = definition_json_from_sources(&[first]).expect("first definition");
    let second = definition_json_from_sources(&[second]).expect("second definition");
    let first_ids = first["values"]
        .as_array()
        .unwrap()
        .iter()
        .map(|value| value["valueId"].as_u64().unwrap())
        .collect::<BTreeSet<_>>();
    let second_ids = second["values"]
        .as_array()
        .unwrap()
        .iter()
        .map(|value| value["valueId"].as_u64().unwrap())
        .collect::<BTreeSet<_>>();

    assert_eq!(first_ids, second_ids);
    assert_eq!(first_ids, BTreeSet::from([2201, 2202]));
}

#[test]
fn unpinned_ids_follow_declaration_order() {
    let first = SourceFile::with_path(
            "main.st",
            "PROGRAM Main\nVAR\n    Level : REAL {attribute 'oot' := 'value'};\n    BatchCount : DINT {attribute 'oot' := 'value'};\nEND_VAR\nEND_PROGRAM\n",
        );
    let second = SourceFile::with_path(
            "main.st",
            "PROGRAM Main\nVAR\n    BatchCount : DINT {attribute 'oot' := 'value'};\n    Level : REAL {attribute 'oot' := 'value'};\nEND_VAR\nEND_PROGRAM\n",
        );

    let first = definition_json_from_sources(&[first]).expect("first definition");
    let second = definition_json_from_sources(&[second]).expect("second definition");

    assert_eq!(first["values"][0]["name"], "Level");
    assert_eq!(first["values"][0]["valueId"], 2001);
    assert_eq!(second["values"][0]["name"], "BatchCount");
    assert_eq!(second["values"][0]["valueId"], 2001);
}
