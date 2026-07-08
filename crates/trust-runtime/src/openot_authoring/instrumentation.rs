use std::collections::BTreeMap;

use trust_hir::openot_authoring as hir_openot;
use trust_hir::openot_authoring::OotKind;

use super::types::*;
use super::{PRODUCER_NAME, SOURCE_TIME_NAME, USE_SOURCE_TIME_NAME};

pub(super) fn instrument_source_text(text: &str, programs: &[ProgramInstrumentation]) -> String {
    if programs.is_empty() {
        return text.to_string();
    }

    let mut output = String::with_capacity(text.len() + 2048);
    let mut cursor = 0usize;
    let mut changed = false;

    for program in programs {
        output.push_str(&text[cursor..program.start]);
        let block = &text[program.start..program.end];
        let instrumented = instrument_program_block(block, &program.annotations);
        if instrumented != block {
            changed = true;
        }
        output.push_str(&instrumented);
        cursor = program.end;
    }
    output.push_str(&text[cursor..]);

    if changed {
        output
    } else {
        text.to_string()
    }
}

fn instrument_program_block(block: &str, annotations: &[Annotation]) -> String {
    if annotations.is_empty() {
        return block.to_string();
    }

    let lines = block.lines().collect::<Vec<_>>();
    let Some(var_end) = lines
        .iter()
        .position(|line| line.trim().eq_ignore_ascii_case("END_VAR"))
    else {
        return block.to_string();
    };
    let Some(end_program) = lines
        .iter()
        .rposition(|line| line.trim().eq_ignore_ascii_case("END_PROGRAM"))
    else {
        return block.to_string();
    };

    let mut out = String::new();
    for (idx, line) in lines.iter().enumerate() {
        if idx == var_end {
            for declaration in hidden_declarations(annotations) {
                out.push_str(&declaration);
                out.push('\n');
            }
        }
        if idx == end_program {
            for statement in hidden_statements(annotations) {
                out.push_str(&statement);
                out.push('\n');
            }
        }
        out.push_str(line);
        out.push('\n');
    }
    out
}

fn hidden_declarations(annotations: &[Annotation]) -> Vec<String> {
    let mut declarations = vec![
        format!("    {PRODUCER_NAME} : OPENOT_Producer;"),
        format!("    {USE_SOURCE_TIME_NAME} : BOOL := FALSE;"),
        format!("    {SOURCE_TIME_NAME} : ULINT := ULINT#0;"),
    ];
    for annotation in annotations {
        let safe = safe_identifier(&annotation.var_name);
        match annotation.kind {
            OotKind::State => {
                if annotation.enum_state.is_some() {
                    declarations.push(format!(
                        "    OotPrev_{safe} : {} := {};",
                        annotation.st_type,
                        state_initializer(annotation)
                    ));
                    declarations.push(format!("    OotStatePrev_{safe} : UINT := UINT#0;"));
                    declarations.push(format!("    OotStateNew_{safe} : UINT := UINT#0;"));
                } else {
                    declarations.push(format!(
                        "    OotPrev_{safe} : UINT := {};",
                        state_initializer(annotation)
                    ));
                }
            }
            OotKind::Batch => {
                declarations.push(format!(
                    "    OotPrev_{safe} : {} := {};",
                    annotation.st_type,
                    state_initializer(annotation)
                ));
                declarations.push(format!("    OotBatchStateNew_{safe} : UINT := UINT#0;"));
            }
            OotKind::Alarm
            | OotKind::Message
            | OotKind::Condition
            | OotKind::RecipeLoaded
            | OotKind::RecipeApproved
            | OotKind::MaterialAddition
            | OotKind::OperatorAction
            | OotKind::OperatorLogin
            | OotKind::OperatorLogout
            | OotKind::SecurityFailure
            | OotKind::ESignature => {
                declarations.push(format!("    OotPrev_{safe} : BOOL := FALSE;"))
            }
            OotKind::Value => {}
        }
    }
    declarations
}

fn hidden_statements(annotations: &[Annotation]) -> Vec<String> {
    let mut statements = vec![
        "(* Generated OpenOT attribute instrumentation. *)".to_string(),
        format!("{PRODUCER_NAME}(Execute := FALSE, ResetScanRecords := TRUE);"),
        format!("{PRODUCER_NAME}(Execute := FALSE, ResetScanRecords := FALSE);"),
    ];

    for annotation in annotations {
        if matches!(annotation.kind, OotKind::Condition | OotKind::ESignature) {
            continue;
        }
        match annotation.kind {
            OotKind::Value => statements.extend(value_statements(annotation)),
            OotKind::State => statements.extend(state_statements(annotation)),
            OotKind::Alarm => statements.extend(alarm_statements(annotation)),
            OotKind::Message => statements.extend(message_statements(annotation)),
            OotKind::Batch => statements.extend(batch_statements(annotation)),
            OotKind::RecipeLoaded => statements.extend(recipe_loaded_statements(annotation)),
            OotKind::RecipeApproved => statements.extend(recipe_approved_statements(annotation)),
            OotKind::MaterialAddition => {
                statements.extend(material_addition_statements(annotation))
            }
            OotKind::OperatorAction => statements.extend(operator_action_statements(annotation)),
            OotKind::OperatorLogin => statements.extend(operator_login_statements(annotation)),
            OotKind::OperatorLogout => statements.extend(operator_logout_statements(annotation)),
            OotKind::SecurityFailure => statements.extend(security_failure_statements(annotation)),
            OotKind::Condition | OotKind::ESignature => {}
        }
    }
    for annotation in annotations
        .iter()
        .filter(|annotation| annotation.kind == OotKind::Condition)
    {
        statements.extend(condition_lifecycle_statements(annotation));
    }
    for annotation in annotations
        .iter()
        .filter(|annotation| annotation.kind == OotKind::ESignature)
    {
        statements.extend(e_signature_statements(annotation));
    }
    statements
}

fn value_statements(annotation: &Annotation) -> Vec<String> {
    if annotation.is_audited_value() {
        return audited_value_statements(annotation);
    }

    if annotation.is_real() {
        return vec![
            format!(
                "{PRODUCER_NAME}(Execute := TRUE, Op := UINT#6, SourceId := UDINT#{}, {}, ValueId := UDINT#{}, ValueReal := {}, DeadbandReal := {}, {}, SuppressPrevious := {}, HasQuality := {}, Quality := UINT#{}, {});",
                annotation.source_id,
                source_time_args(),
                annotation.id,
                annotation.var_name,
                real_literal(annotation.deadband.as_deref().unwrap_or("0.0")),
                annotation.sampling_args(),
                bool_literal(annotation.suppress_previous),
                bool_literal(annotation.quality.is_some()),
                annotation.quality.unwrap_or(0),
                attestable_arg(annotation)
            ),
            format!("{PRODUCER_NAME}(Execute := FALSE);"),
        ];
    }

    if annotation.is_dint() {
        return vec![
            format!(
                "{PRODUCER_NAME}(Execute := TRUE, Op := UINT#7, SourceId := UDINT#{}, {}, ValueId := UDINT#{}, ValueInt := {}, DeadbandReal := REAL#0.0, {}, SuppressPrevious := {}, HasQuality := {}, Quality := UINT#{}, {});",
                annotation.source_id,
                source_time_args(),
                annotation.id,
                annotation.var_name,
                annotation.sampling_args(),
                bool_literal(annotation.suppress_previous),
                bool_literal(annotation.quality.is_some()),
                annotation.quality.unwrap_or(0),
                attestable_arg(annotation)
            ),
            format!("{PRODUCER_NAME}(Execute := FALSE);"),
        ];
    }

    if annotation.is_string() {
        return vec![
            format!(
                "{PRODUCER_NAME}(Execute := TRUE, Op := UINT#11, SourceId := UDINT#{}, {}, ValueId := UDINT#{}, ValueString := {}, {}, SuppressPrevious := {}, HasQuality := {}, Quality := UINT#{}, {});",
                annotation.source_id,
                source_time_args(),
                annotation.id,
                annotation.var_name,
                annotation.sampling_args(),
                bool_literal(annotation.suppress_previous),
                bool_literal(annotation.quality.is_some()),
                annotation.quality.unwrap_or(0),
                attestable_arg(annotation)
            ),
            format!("{PRODUCER_NAME}(Execute := FALSE);"),
        ];
    }

    vec![
        format!(
            "{PRODUCER_NAME}(Execute := TRUE, Op := UINT#10, SourceId := UDINT#{}, {}, ValueId := UDINT#{}, ValueTypeTag := BYTE#16#{:02X}, ValuePayloadLength := UINT#{}, ValueBits := {}, {}, SuppressPrevious := {}, HasQuality := {}, Quality := UINT#{}, {});",
            annotation.source_id,
            source_time_args(),
            annotation.id,
            annotation.tlv_type(),
            annotation.payload_len(),
            annotation.value_bits_expr(),
            annotation.sampling_args(),
            bool_literal(annotation.suppress_previous),
            bool_literal(annotation.quality.is_some()),
            annotation.quality.unwrap_or(0),
            attestable_arg(annotation)
        ),
        format!("{PRODUCER_NAME}(Execute := FALSE);"),
    ]
}

fn audited_value_statements(annotation: &Annotation) -> Vec<String> {
    let mut call_args = vec![
        "Execute := TRUE".to_string(),
        "Op := UINT#15".to_string(),
        format!("SourceId := UDINT#{}", annotation.source_id),
        source_time_args(),
        attestable_arg(annotation),
        format!("ValueId := UDINT#{}", annotation.id),
        format!("ValueTypeTag := BYTE#16#{:02X}", annotation.tlv_type()),
        format!("ValuePayloadLength := UINT#{}", annotation.payload_len()),
    ];

    if annotation.is_real() {
        call_args.push(format!("ValueReal := {}", annotation.var_name));
    } else if annotation.is_dint() {
        call_args.push(format!("ValueInt := {}", annotation.var_name));
    } else if annotation.is_string() {
        call_args.push(format!("ValueString := {}", annotation.var_name));
    } else {
        call_args.push(format!("ValueBits := {}", annotation.value_bits_expr()));
    }

    call_args.push(format!(
        "AuditActor := {}",
        annotation
            .regulated_actor
            .as_deref()
            .expect("audit actor should be validated")
    ));
    call_args.push(format!(
        "AuditReason := {}",
        annotation
            .regulated_reason
            .as_deref()
            .expect("audit reason should be validated")
    ));
    call_args.push(format!(
        "AuditHasAuthResult := {}",
        bool_literal(annotation.auth_result.is_some())
    ));
    call_args.push(format!(
        "AuditAuthResult := {}",
        annotation
            .auth_result
            .as_deref()
            .map(auth_result_expr)
            .unwrap_or_else(|| "UINT#0".to_string())
    ));

    vec![
        format!("{PRODUCER_NAME}({});", call_args.join(", ")),
        format!("{PRODUCER_NAME}(Execute := FALSE);"),
    ]
}

fn state_statements(annotation: &Annotation) -> Vec<String> {
    let safe = safe_identifier(&annotation.var_name);
    if let Some(enum_state) = &annotation.enum_state {
        let mut statements = vec![format!("IF {} <> OotPrev_{safe} THEN", annotation.var_name)];
        statements.extend(enum_state_value_statements(
            &format!("OotPrev_{safe}"),
            &format!("OotStatePrev_{safe}"),
            enum_state,
            "    ",
        ));
        statements.extend(enum_state_value_statements(
            &annotation.var_name,
            &format!("OotStateNew_{safe}"),
            enum_state,
            "    ",
        ));
        statements.push(format!(
            "    {PRODUCER_NAME}(Execute := TRUE, Op := UINT#8, SourceId := UDINT#{}, {}, {}, StateMachineId := UDINT#{}, Category := UINT#{}, PreviousState := OotStatePrev_{safe}, NewState := OotStateNew_{safe});",
            annotation.source_id, source_time_args(), attestable_arg(annotation), annotation.id, annotation.category
        ));
        statements.push(format!("    {PRODUCER_NAME}(Execute := FALSE);"));
        statements.push(format!("    OotPrev_{safe} := {};", annotation.var_name));
        statements.push("END_IF;".to_string());
        return statements;
    }
    vec![
        format!("IF {} <> OotPrev_{safe} THEN", annotation.var_name),
        format!(
            "    {PRODUCER_NAME}(Execute := TRUE, Op := UINT#8, SourceId := UDINT#{}, {}, {}, StateMachineId := UDINT#{}, Category := UINT#{}, PreviousState := OotPrev_{safe}, NewState := {});",
            annotation.source_id, source_time_args(), attestable_arg(annotation), annotation.id, annotation.category, annotation.var_name
        ),
        format!("    {PRODUCER_NAME}(Execute := FALSE);"),
        format!("    OotPrev_{safe} := {};", annotation.var_name),
        "END_IF;".to_string(),
    ]
}

fn batch_statements(annotation: &Annotation) -> Vec<String> {
    let safe = safe_identifier(&annotation.var_name);
    let enum_state = annotation
        .enum_state
        .as_ref()
        .expect("batch annotations should resolve an enum state");
    let mut call_args = vec![
        "Execute := TRUE".to_string(),
        "Op := UINT#13".to_string(),
        format!("SourceId := UDINT#{}", annotation.source_id),
        source_time_args(),
        attestable_arg(annotation),
        "ProcedureEventTypeId := UDINT#16#0303".to_string(),
        format!(
            "ProcedureBatchId := {}",
            annotation
                .batch_id
                .as_deref()
                .expect("batch id should be validated")
        ),
        format!("ProcedureNewState := OotBatchStateNew_{safe}"),
    ];
    if let Some(recipe_id) = &annotation.recipe_id {
        call_args.push("ProcedureHasRecipeId := TRUE".to_string());
        call_args.push(format!("ProcedureRecipeId := {recipe_id}"));
    }

    let mut statements = vec![format!("IF {} <> OotPrev_{safe} THEN", annotation.var_name)];
    statements.extend(enum_state_value_statements(
        &annotation.var_name,
        &format!("OotBatchStateNew_{safe}"),
        enum_state,
        "    ",
    ));
    statements.push(format!("    {PRODUCER_NAME}({});", call_args.join(", ")));
    statements.push(format!("    {PRODUCER_NAME}(Execute := FALSE);"));
    statements.push(format!("    OotPrev_{safe} := {};", annotation.var_name));
    statements.push("END_IF;".to_string());
    statements
}

fn recipe_loaded_statements(annotation: &Annotation) -> Vec<String> {
    let mut call_args = vec![
        "Execute := TRUE".to_string(),
        "Op := UINT#13".to_string(),
        format!("SourceId := UDINT#{}", annotation.source_id),
        source_time_args(),
        "ProcedureEventTypeId := UDINT#16#0301".to_string(),
        format!(
            "ProcedureRecipeId := {}",
            annotation
                .recipe_id
                .as_deref()
                .expect("recipe id should be validated")
        ),
        format!(
            "ProcedureRecipeVersion := {}",
            annotation
                .recipe_version
                .as_deref()
                .expect("recipe version should be validated")
        ),
    ];
    if let Some(batch_id) = &annotation.procedure_batch_id {
        call_args.push("ProcedureHasBatchId := TRUE".to_string());
        call_args.push(format!("ProcedureBatchId := {batch_id}"));
    }
    edge_trigger_statements(annotation, call_args)
}

fn recipe_approved_statements(annotation: &Annotation) -> Vec<String> {
    let mut call_args = vec![
        "Execute := TRUE".to_string(),
        "Op := UINT#13".to_string(),
        format!("SourceId := UDINT#{}", annotation.source_id),
        source_time_args(),
        "ProcedureEventTypeId := UDINT#16#0302".to_string(),
        format!(
            "ProcedureRecipeId := {}",
            annotation
                .recipe_id
                .as_deref()
                .expect("recipe id should be validated")
        ),
        format!(
            "ProcedureRecipeVersion := {}",
            annotation
                .recipe_version
                .as_deref()
                .expect("recipe version should be validated")
        ),
    ];
    if let Some(auth_result) = &annotation.auth_result {
        call_args.push("ProcedureHasAuthResult := TRUE".to_string());
        call_args.push(format!(
            "ProcedureAuthResult := {}",
            auth_result_expr(auth_result)
        ));
    }
    if let Some(ack_by) = &annotation.procedure_ack_by {
        call_args.push("ProcedureHasAckBy := TRUE".to_string());
        call_args.push(format!("ProcedureAckBy := {ack_by}"));
    }
    edge_trigger_statements(annotation, call_args)
}

fn material_addition_statements(annotation: &Annotation) -> Vec<String> {
    let mut call_args = vec![
        "Execute := TRUE".to_string(),
        "Op := UINT#13".to_string(),
        format!("SourceId := UDINT#{}", annotation.source_id),
        source_time_args(),
        "ProcedureEventTypeId := UDINT#16#0304".to_string(),
        format!(
            "ProcedureBatchId := {}",
            annotation
                .procedure_batch_id
                .as_deref()
                .expect("batch id should be validated")
        ),
        format!(
            "ProcedureMaterialId := {}",
            annotation
                .material_id
                .as_deref()
                .expect("material id should be validated")
        ),
        format!(
            "ProcedureQuantity := {}",
            annotation
                .quantity
                .as_deref()
                .expect("quantity should be validated")
        ),
    ];
    if let Some(unit_id) = annotation.material_unit_id {
        call_args.push("ProcedureHasUnit := TRUE".to_string());
        call_args.push(format!("ProcedureUnitId := UINT#{unit_id}"));
    }
    edge_trigger_statements(annotation, call_args)
}

fn operator_action_statements(annotation: &Annotation) -> Vec<String> {
    let mut call_args = vec![
        "Execute := TRUE".to_string(),
        "Op := UINT#14".to_string(),
        format!("SourceId := UDINT#{}", annotation.source_id),
        source_time_args(),
        "RegulatedEventTypeId := UDINT#16#0400".to_string(),
        format!(
            "RegulatedActionId := {}",
            annotation
                .regulated_action_id
                .as_deref()
                .expect("action id should be validated")
        ),
        format!(
            "RegulatedActor := {}",
            annotation
                .regulated_actor
                .as_deref()
                .expect("actor should be validated")
        ),
    ];
    for (idx, context_ref) in annotation.regulated_context_refs.iter().enumerate() {
        let number = idx + 1;
        call_args.push(format!("RegulatedHasContext{number} := TRUE"));
        call_args.push(format!("RegulatedContext{number} := {context_ref}"));
    }
    if let Some(auth_result) = &annotation.auth_result {
        call_args.push("RegulatedHasAuthResult := TRUE".to_string());
        call_args.push(format!(
            "RegulatedAuthResult := {}",
            auth_result_expr(auth_result)
        ));
    }
    if let Some(workstation) = &annotation.regulated_workstation {
        call_args.push("RegulatedHasWorkstation := TRUE".to_string());
        call_args.push(format!("RegulatedWorkstation := {workstation}"));
    }
    edge_trigger_statements(annotation, call_args)
}

fn operator_login_statements(annotation: &Annotation) -> Vec<String> {
    let mut call_args = vec![
        "Execute := TRUE".to_string(),
        "Op := UINT#14".to_string(),
        format!("SourceId := UDINT#{}", annotation.source_id),
        source_time_args(),
        "RegulatedEventTypeId := UDINT#16#0401".to_string(),
        format!(
            "RegulatedActor := {}",
            annotation
                .regulated_actor
                .as_deref()
                .expect("actor should be validated")
        ),
        "RegulatedHasAuthResult := TRUE".to_string(),
        format!(
            "RegulatedAuthResult := {}",
            auth_result_expr(
                annotation
                    .auth_result
                    .as_deref()
                    .expect("authResult should be validated")
            )
        ),
    ];
    if let Some(workstation) = &annotation.regulated_workstation {
        call_args.push("RegulatedHasWorkstation := TRUE".to_string());
        call_args.push(format!("RegulatedWorkstation := {workstation}"));
    }
    if let Some(role) = &annotation.regulated_role {
        call_args.push("RegulatedHasRole := TRUE".to_string());
        call_args.push(format!("RegulatedRole := {role}"));
    }
    edge_trigger_statements(annotation, call_args)
}

fn operator_logout_statements(annotation: &Annotation) -> Vec<String> {
    let mut call_args = vec![
        "Execute := TRUE".to_string(),
        "Op := UINT#14".to_string(),
        format!("SourceId := UDINT#{}", annotation.source_id),
        source_time_args(),
        "RegulatedEventTypeId := UDINT#16#0402".to_string(),
        format!(
            "RegulatedActor := {}",
            annotation
                .regulated_actor
                .as_deref()
                .expect("actor should be validated")
        ),
    ];
    if let Some(workstation) = &annotation.regulated_workstation {
        call_args.push("RegulatedHasWorkstation := TRUE".to_string());
        call_args.push(format!("RegulatedWorkstation := {workstation}"));
    }
    edge_trigger_statements(annotation, call_args)
}

fn security_failure_statements(annotation: &Annotation) -> Vec<String> {
    let mut call_args = vec![
        "Execute := TRUE".to_string(),
        "Op := UINT#14".to_string(),
        format!("SourceId := UDINT#{}", annotation.source_id),
        source_time_args(),
        "RegulatedEventTypeId := UDINT#16#0405".to_string(),
        format!(
            "RegulatedActor := {}",
            annotation
                .regulated_actor
                .as_deref()
                .expect("actor should be validated")
        ),
    ];
    if let Some(workstation) = &annotation.regulated_workstation {
        call_args.push("RegulatedHasWorkstation := TRUE".to_string());
        call_args.push(format!("RegulatedWorkstation := {workstation}"));
    }
    if let Some(reason) = &annotation.regulated_reason {
        call_args.push("RegulatedHasReason := TRUE".to_string());
        call_args.push(format!("RegulatedReason := {reason}"));
    }
    edge_trigger_statements(annotation, call_args)
}

fn e_signature_statements(annotation: &Annotation) -> Vec<String> {
    let mut call_args = vec![
        "Execute := TRUE".to_string(),
        "Op := UINT#16".to_string(),
        format!("SourceId := UDINT#{}", annotation.source_id),
        source_time_args(),
        format!(
            "SignatureActionId := {}",
            annotation
                .signature_action_id
                .as_deref()
                .expect("signature action should be validated")
        ),
        format!(
            "SignatureActor := {}",
            annotation
                .signature_actor
                .as_deref()
                .expect("signature actor should be validated")
        ),
        format!(
            "SignatureMeaning := UINT#{}",
            annotation
                .signature_meaning
                .expect("signature meaning should be validated")
        ),
        format!(
            "SignatureAttestsId := UDINT#{}",
            annotation.signature_attests_id
        ),
    ];
    if let Some(auth_result) = &annotation.auth_result {
        call_args.push("SignatureHasAuthResult := TRUE".to_string());
        call_args.push(format!(
            "SignatureAuthResult := {}",
            auth_result_expr(auth_result)
        ));
    }
    edge_trigger_statements(annotation, call_args)
}

fn auth_result_expr(value: &str) -> String {
    hir_openot::auth_result_code(value)
        .map(|code| format!("UINT#{code}"))
        .unwrap_or_else(|| value.to_string())
}

fn edge_trigger_statements(annotation: &Annotation, mut call_args: Vec<String>) -> Vec<String> {
    let safe = safe_identifier(&annotation.var_name);
    call_args.push(attestable_arg(annotation));
    vec![
        format!("IF {} AND (NOT OotPrev_{safe}) THEN", annotation.var_name),
        format!("    {PRODUCER_NAME}({});", call_args.join(", ")),
        format!("    {PRODUCER_NAME}(Execute := FALSE);"),
        "END_IF;".to_string(),
        format!("OotPrev_{safe} := {};", annotation.var_name),
    ]
}

fn enum_state_value_statements(
    expression: &str,
    target: &str,
    enum_state: &EnumStateInfo,
    indent: &str,
) -> Vec<String> {
    let mut statements = Vec::new();
    for (idx, variant) in enum_state.variants.iter().enumerate() {
        let keyword = if idx == 0 { "IF" } else { "ELSIF" };
        statements.push(format!(
            "{indent}{keyword} {expression} = {} THEN",
            enum_state.literal(&variant.name)
        ));
        statements.push(format!("{indent}    {target} := UINT#{};", variant.value));
    }
    statements.push(format!("{indent}END_IF;"));
    statements
}

fn alarm_statements(annotation: &Annotation) -> Vec<String> {
    let safe = safe_identifier(&annotation.var_name);
    let mut call_args = vec![
        "Execute := TRUE".to_string(),
        "Op := UINT#9".to_string(),
        format!("SourceId := UDINT#{}", annotation.source_id),
        source_time_args(),
        attestable_arg(annotation),
        format!("ConditionId := UDINT#{}", annotation.id),
        format!("ConditionClass := UINT#{}", annotation.condition_class),
        format!("Severity := UINT#{}", annotation.severity),
        format!("ConditionActive := {}", annotation.var_name),
    ];
    if let Some(cause_operand) = &annotation.cause_operand {
        call_args.push("HasCauseOperand := TRUE".to_string());
        call_args.push(format!(
            "CauseOperand := UDINT#{}",
            cause_operand.operand_id
        ));
    }
    vec![
        format!("IF {} <> OotPrev_{safe} THEN", annotation.var_name),
        format!("    {PRODUCER_NAME}({});", call_args.join(", ")),
        format!("    {PRODUCER_NAME}(Execute := FALSE);"),
        format!("    OotPrev_{safe} := {};", annotation.var_name),
        "END_IF;".to_string(),
    ]
}

fn message_statements(annotation: &Annotation) -> Vec<String> {
    let safe = safe_identifier(&annotation.var_name);
    let mut call_args = vec![
        "Execute := TRUE".to_string(),
        "Op := UINT#0".to_string(),
        format!("SourceId := UDINT#{}", annotation.source_id),
        source_time_args(),
        attestable_arg(annotation),
        "Checkpoint := FALSE".to_string(),
        "AccumulateScanRecords := TRUE".to_string(),
        format!("MessageTemplateId := UDINT#{}", annotation.id),
    ];
    if let Some(severity) = annotation.message_severity {
        call_args.push("MessageHasSeverity := TRUE".to_string());
        call_args.push(format!("MessageSeverity := UINT#{severity}"));
    }
    call_args.push(format!(
        "MessageArgCount := UINT#{}",
        annotation.message_args.len()
    ));
    for (idx, arg) in annotation.message_args.iter().enumerate() {
        let arg_number = idx + 1;
        call_args.push(format!(
            "MessageArg{arg_number}TypeTag := BYTE#16#{:02X}",
            arg.tlv_type()
        ));
        if arg.is_string() {
            call_args.push(format!("MessageArg{arg_number}String := {}", arg.name));
        } else {
            call_args.push(format!(
                "MessageArg{arg_number}PayloadLength := UINT#{}",
                arg.payload_len()
            ));
            call_args.push(format!(
                "MessageArg{arg_number}Bits := {}",
                arg.value_bits_expr()
            ));
        }
    }
    vec![
        format!("IF {} AND (NOT OotPrev_{safe}) THEN", annotation.var_name),
        format!("    {PRODUCER_NAME}({});", call_args.join(", ")),
        format!("    {PRODUCER_NAME}(Execute := FALSE);"),
        "END_IF;".to_string(),
        format!("OotPrev_{safe} := {};", annotation.var_name),
    ]
}

fn condition_lifecycle_statements(annotation: &Annotation) -> Vec<String> {
    let safe = safe_identifier(&annotation.var_name);
    let mut call_args = vec![
        "Execute := TRUE".to_string(),
        "Op := UINT#12".to_string(),
        format!("SourceId := UDINT#{}", annotation.source_id),
        source_time_args(),
        attestable_arg(annotation),
        format!("ConditionId := UDINT#{}", annotation.id),
        format!(
            "ConditionLifecycleEventTypeId := UDINT#16#{:04X}",
            annotation
                .condition_event_id
                .expect("condition event id should be resolved")
        ),
    ];
    call_args.push(format!(
        "LifecycleHasAckBy := {}",
        bool_literal(annotation.condition_ack_by.is_some())
    ));
    call_args.push(format!(
        "LifecycleAckBy := {}",
        annotation.condition_ack_by.as_deref().unwrap_or("''")
    ));
    call_args.push(format!(
        "LifecycleHasShelveSecs := {}",
        bool_literal(annotation.condition_shelve_secs.is_some())
    ));
    call_args.push(format!(
        "LifecycleShelveSecs := {}",
        annotation
            .condition_shelve_secs
            .as_deref()
            .unwrap_or("UDINT#0")
    ));
    call_args.push(format!(
        "LifecycleHasReason := {}",
        bool_literal(annotation.condition_reason.is_some())
    ));
    call_args.push(format!(
        "LifecycleReason := {}",
        annotation.condition_reason.as_deref().unwrap_or("''")
    ));
    call_args.push(format!(
        "LifecycleHasComment := {}",
        bool_literal(annotation.condition_comment.is_some())
    ));
    call_args.push(format!(
        "LifecycleComment := {}",
        annotation.condition_comment.as_deref().unwrap_or("''")
    ));
    call_args.push(format!(
        "LifecycleHasPreviousPriority := {}",
        bool_literal(annotation.condition_previous_priority.is_some())
    ));
    call_args.push(format!(
        "LifecyclePreviousPriority := {}",
        annotation
            .condition_previous_priority
            .as_deref()
            .unwrap_or("UINT#0")
    ));
    call_args.push(format!(
        "LifecycleHasNewPriority := {}",
        bool_literal(annotation.condition_new_priority.is_some())
    ));
    call_args.push(format!(
        "LifecycleNewPriority := {}",
        annotation
            .condition_new_priority
            .as_deref()
            .unwrap_or("UINT#0")
    ));
    vec![
        format!("IF {} AND (NOT OotPrev_{safe}) THEN", annotation.var_name),
        format!("    {PRODUCER_NAME}({});", call_args.join(", ")),
        format!("    {PRODUCER_NAME}(Execute := FALSE);"),
        "END_IF;".to_string(),
        format!("OotPrev_{safe} := {};", annotation.var_name),
    ]
}

fn source_time_args() -> String {
    format!("UseSourceTimeInput := {USE_SOURCE_TIME_NAME}, SourceTimeInput := {SOURCE_TIME_NAME}")
}

fn attestable_arg(annotation: &Annotation) -> String {
    format!("AttestableId := UDINT#{}", annotation.attestable_id)
}

pub(super) fn id_or_default(attrs: &BTreeMap<String, String>, default: u32) -> u32 {
    attrs
        .get("id")
        .or_else(|| attrs.get("valueid"))
        .or_else(|| attrs.get("machineid"))
        .or_else(|| attrs.get("statemachineid"))
        .or_else(|| attrs.get("conditionid"))
        .and_then(|value| value.parse::<u32>().ok())
        .unwrap_or(default)
}

fn state_initializer(annotation: &Annotation) -> String {
    if let Some(enum_state) = &annotation.enum_state {
        return annotation
            .initializer
            .clone()
            .unwrap_or_else(|| enum_state.default_literal());
    }
    annotation
        .initializer
        .as_deref()
        .filter(|init| init.to_ascii_uppercase().starts_with("UINT#"))
        .unwrap_or("UINT#0")
        .to_string()
}

fn real_literal(value: &str) -> String {
    if value.to_ascii_uppercase().starts_with("REAL#") {
        value.to_string()
    } else {
        format!("REAL#{value}")
    }
}

fn bool_literal(value: bool) -> &'static str {
    if value {
        "TRUE"
    } else {
        "FALSE"
    }
}

fn safe_identifier(name: &str) -> String {
    name.chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '_' {
                ch
            } else {
                '_'
            }
        })
        .collect()
}
