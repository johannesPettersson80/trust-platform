use serde_json::{json, Value};
use text_size::TextRange;
use trust_hir::openot_authoring::OotKind;

use super::{SAMPLING_MODE_DEFAULT, SAMPLING_MODE_HYSTERESIS, SAMPLING_MODE_PERIODIC};

#[derive(Debug, Clone)]
pub(super) struct Annotation {
    pub(super) kind: OotKind,
    pub(super) var_name: String,
    pub(super) st_type: String,
    pub(super) initializer: Option<String>,
    pub(super) id: u32,
    pub(super) source_id: u32,
    pub(super) source: SourceDescriptor,
    pub(super) deadband: Option<String>,
    pub(super) sampling: Option<String>,
    pub(super) interval_ms: Option<u64>,
    pub(super) unit: Option<String>,
    pub(super) quality: Option<u16>,
    pub(super) semantic_role: u16,
    pub(super) suppress_previous: bool,
    pub(super) category: u16,
    pub(super) model: Option<String>,
    pub(super) condition_class: u16,
    pub(super) severity: u16,
    pub(super) message: Option<String>,
    pub(super) message_severity: Option<u16>,
    pub(super) message_args: Vec<MessageArgInfo>,
    pub(super) cause_operand: Option<CauseOperandInfo>,
    pub(super) enum_state: Option<EnumStateInfo>,
    pub(super) condition_event_id: Option<u32>,
    pub(super) condition_ack_by: Option<String>,
    pub(super) condition_shelve_secs: Option<String>,
    pub(super) condition_reason: Option<String>,
    pub(super) condition_comment: Option<String>,
    pub(super) condition_previous_priority: Option<String>,
    pub(super) condition_new_priority: Option<String>,
    pub(super) batch_id: Option<String>,
    pub(super) recipe_id: Option<String>,
    pub(super) recipe_version: Option<String>,
    pub(super) procedure_batch_id: Option<String>,
    pub(super) auth_result: Option<String>,
    pub(super) procedure_ack_by: Option<String>,
    pub(super) material_id: Option<String>,
    pub(super) quantity: Option<String>,
    pub(super) material_unit_id: Option<u16>,
    pub(super) regulated_action_id: Option<String>,
    pub(super) regulated_actor: Option<String>,
    pub(super) regulated_context_refs: Vec<String>,
    pub(super) regulated_workstation: Option<String>,
    pub(super) regulated_role: Option<String>,
    pub(super) regulated_reason: Option<String>,
    pub(super) signature_action_id: Option<String>,
    pub(super) signature_actor: Option<String>,
    pub(super) signature_meaning: Option<u16>,
    pub(super) signature_attests_id: u32,
    pub(super) attestable_id: u32,
}

impl Annotation {
    pub(super) fn is_real(&self) -> bool {
        self.st_type.eq_ignore_ascii_case("REAL")
    }

    pub(super) fn is_dint(&self) -> bool {
        self.st_type.eq_ignore_ascii_case("DINT")
    }

    pub(super) fn is_string(&self) -> bool {
        let ty = self.normalized_type();
        ty == "STRING" || ty.starts_with("STRING[")
    }

    pub(super) fn is_audited_value(&self) -> bool {
        self.kind == OotKind::Value
            && self.regulated_actor.is_some()
            && self.regulated_reason.is_some()
    }

    pub(super) fn normalized_type(&self) -> String {
        self.st_type.trim().to_ascii_uppercase()
    }

    pub(super) fn tlv_type(&self) -> u8 {
        tlv_type_for_type(&self.st_type)
    }

    pub(super) fn payload_len(&self) -> u8 {
        payload_len_for_type(&self.st_type)
    }

    pub(super) fn value_bits_expr(&self) -> String {
        value_bits_expr_for_type(&self.var_name, &self.st_type)
    }

    pub(super) fn sampling_policy_json(&self) -> Value {
        match self.sampling.as_deref() {
            Some("on-change") => json!("on-change"),
            Some("deadband") => json!("deadband"),
            Some("periodic") => json!(format!(
                "periodic:{}",
                self.interval_ms
                    .expect("periodic sampling interval should be validated")
            )),
            Some("hysteresis") => json!("hysteresis"),
            _ if self.deadband.is_some() => Value::Null,
            _ => json!("on-change"),
        }
    }

    pub(super) fn sampling_mode(&self) -> u16 {
        match self.sampling.as_deref() {
            Some("periodic") => SAMPLING_MODE_PERIODIC,
            Some("hysteresis") => SAMPLING_MODE_HYSTERESIS,
            _ => SAMPLING_MODE_DEFAULT,
        }
    }

    pub(super) fn sampling_interval_ms(&self) -> u64 {
        self.interval_ms.unwrap_or(0)
    }

    pub(super) fn sampling_args(&self) -> String {
        format!(
            "SamplingMode := UINT#{}, SamplingIntervalMs := ULINT#{}",
            self.sampling_mode(),
            self.sampling_interval_ms()
        )
    }
}

fn normalized_type_name(st_type: &str) -> String {
    st_type.trim().to_ascii_uppercase()
}

fn is_string_type(st_type: &str) -> bool {
    let ty = normalized_type_name(st_type);
    ty == "STRING" || ty.starts_with("STRING[")
}

fn tlv_type_for_type(st_type: &str) -> u8 {
    if is_string_type(st_type) {
        return 0x0C;
    }
    match normalized_type_name(st_type).as_str() {
        "BOOL" => 0x00,
        "SINT" => 0x01,
        "USINT" => 0x02,
        "UINT" => 0x03,
        "INT" => 0x04,
        "UDINT" => 0x05,
        "DINT" => 0x06,
        "ULINT" => 0x07,
        "LINT" => 0x08,
        "REAL" => 0x09,
        "LREAL" => 0x0A,
        _ => 0x06,
    }
}

fn payload_len_for_type(st_type: &str) -> u8 {
    if is_string_type(st_type) {
        return 96;
    }
    match normalized_type_name(st_type).as_str() {
        "BOOL" | "SINT" | "USINT" => 1,
        "INT" | "UINT" => 2,
        "DINT" | "UDINT" | "REAL" => 4,
        "LINT" | "ULINT" | "LREAL" => 8,
        _ => 4,
    }
}

fn value_bits_expr_for_type(var_name: &str, st_type: &str) -> String {
    match normalized_type_name(st_type).as_str() {
        "BOOL" => format!("BOOL_TO_ULINT({var_name})"),
        "SINT" => format!("BYTE_TO_ULINT(SINT_TO_BYTE({var_name}))"),
        "USINT" => format!("USINT_TO_ULINT({var_name})"),
        "INT" => format!("WORD_TO_ULINT(INT_TO_WORD({var_name}))"),
        "UINT" => format!("UINT_TO_ULINT({var_name})"),
        "UDINT" => format!("UDINT_TO_ULINT({var_name})"),
        "DINT" => format!("DWORD_TO_ULINT(DINT_TO_DWORD({var_name}))"),
        "REAL" => format!("DWORD_TO_ULINT(REAL_TO_DWORD({var_name}))"),
        "LINT" => format!("LWORD_TO_ULINT(LINT_TO_LWORD({var_name}))"),
        "ULINT" => var_name.to_string(),
        "LREAL" => format!("LWORD_TO_ULINT(LREAL_TO_LWORD({var_name}))"),
        other => panic!("unsupported OpenOT value type {other}"),
    }
}

#[derive(Debug, Clone, Default)]
pub(super) struct AuthoringModel {
    pub(super) files: Vec<FileAuthoringModel>,
    pub(super) annotations: Vec<Annotation>,
}

#[derive(Debug, Clone, Default)]
pub(super) struct FileAuthoringModel {
    pub(super) programs: Vec<ProgramInstrumentation>,
}

#[derive(Debug, Clone)]
pub(super) struct ProgramInstrumentation {
    pub(super) start: usize,
    pub(super) end: usize,
    pub(super) annotations: Vec<Annotation>,
}

#[derive(Debug, Default)]
pub(super) struct AnnotationCounters {
    pub(super) values: u32,
    pub(super) states: u32,
    pub(super) alarms: u32,
    pub(super) messages: u32,
}

#[derive(Debug, Clone)]
pub(super) struct AnnotationDraft {
    pub(super) kind: OotKind,
    pub(super) var_name: String,
    pub(super) st_type: String,
    pub(super) initializer: Option<String>,
    pub(super) source_id: u32,
    pub(super) source: SourceDescriptor,
    pub(super) enum_state: Option<EnumStateInfo>,
    pub(super) message_args: Vec<MessageArgInfo>,
    pub(super) cause_operand: Option<CauseOperandInfo>,
    pub(super) condition_parent: Option<String>,
    pub(super) condition_event_id: Option<u32>,
    pub(super) condition_ack_by: Option<String>,
    pub(super) condition_shelve_secs: Option<String>,
    pub(super) condition_reason: Option<String>,
    pub(super) condition_comment: Option<String>,
    pub(super) condition_previous_priority: Option<String>,
    pub(super) condition_new_priority: Option<String>,
    pub(super) batch_id: Option<String>,
    pub(super) recipe_id: Option<String>,
    pub(super) recipe_version: Option<String>,
    pub(super) procedure_batch_id: Option<String>,
    pub(super) auth_result: Option<String>,
    pub(super) procedure_ack_by: Option<String>,
    pub(super) material_id: Option<String>,
    pub(super) quantity: Option<String>,
    pub(super) material_unit_id: Option<u16>,
    pub(super) regulated_action_id: Option<String>,
    pub(super) regulated_actor: Option<String>,
    pub(super) regulated_context_refs: Vec<String>,
    pub(super) regulated_workstation: Option<String>,
    pub(super) regulated_role: Option<String>,
    pub(super) regulated_reason: Option<String>,
    pub(super) signature_action_id: Option<String>,
    pub(super) signature_actor: Option<String>,
    pub(super) signature_meaning: Option<u16>,
    pub(super) signature_attests: Option<String>,
}

#[derive(Debug, Clone)]
pub(super) struct AnnotationIndexEntry {
    pub(super) kind: OotKind,
    pub(super) id: u32,
    pub(super) source_id: u32,
    pub(super) attestable_id: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct SourceDescriptor {
    pub(super) name: String,
    pub(super) path: Vec<String>,
    pub(super) hierarchy: Vec<String>,
}

#[derive(Debug, Clone)]
pub(super) struct MessageArgInfo {
    pub(super) name: String,
    pub(super) st_type: String,
}

impl MessageArgInfo {
    pub(super) fn normalized_type(&self) -> String {
        self.st_type.trim().to_ascii_uppercase()
    }

    pub(super) fn is_string(&self) -> bool {
        let ty = self.normalized_type();
        ty == "STRING" || ty.starts_with("STRING[")
    }

    pub(super) fn tlv_type(&self) -> u8 {
        tlv_type_for_type(&self.st_type)
    }

    pub(super) fn payload_len(&self) -> u8 {
        payload_len_for_type(&self.st_type)
    }

    pub(super) fn value_bits_expr(&self) -> String {
        value_bits_expr_for_type(&self.name, &self.st_type)
    }
}

#[derive(Debug, Clone)]
pub(super) struct CauseOperandInfo {
    pub(super) operand_id: u32,
    pub(super) name: String,
}

#[derive(Debug, Clone)]
pub(super) struct NameInfo {
    pub(super) text: String,
    pub(super) range: TextRange,
}

#[derive(Debug, Clone)]
pub(super) struct EnumStateInfo {
    pub(super) type_name: String,
    pub(super) enum_set_name: String,
    pub(super) variants: Vec<EnumVariantInfo>,
}

impl EnumStateInfo {
    pub(super) fn literal(&self, variant: &str) -> String {
        format!("{}#{variant}", self.type_name)
    }

    pub(super) fn default_literal(&self) -> String {
        self.variants.first().map_or_else(
            || format!("{}#0", self.type_name),
            |variant| self.literal(&variant.name),
        )
    }

    pub(super) fn definition_json(&self) -> Value {
        json!({
            "name": self.enum_set_name,
            "members": self
                .variants
                .iter()
                .map(|variant| json!({
                    "value": variant.value,
                    "label": variant.name
                }))
                .collect::<Vec<_>>()
        })
    }
}

#[derive(Debug, Clone)]
pub(super) struct EnumVariantInfo {
    pub(super) name: String,
    pub(super) value: u16,
}
