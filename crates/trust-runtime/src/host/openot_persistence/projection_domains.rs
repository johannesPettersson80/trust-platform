use open_ot_document::{Document, DocumentField, EventDocument};

use super::projection::{event_log_row, EventLogRow};
use super::PersistenceError;

pub(super) enum DomainRow {
    Alarm(AlarmRow),
    Message(MessageRow),
    State(StateRow),
    Batch(BatchRow),
    Recipe(RecipeRow),
    Material(MaterialRow),
    Operator(OperatorRow),
    Audit(AuditRow),
    Signature(SignatureRow),
    System(SystemRow),
    Loss(LossRow),
    Unresolved(UnresolvedRow),
}

pub(super) struct AlarmRow {
    pub common: EventLogRow,
    pub condition: String,
    pub condition_class: Option<String>,
    pub lifecycle_action: String,
    pub correlation_id: Option<String>,
    pub severity: Option<u16>,
    pub severity_label: Option<String>,
    pub cause: Option<String>,
    pub actor: Option<String>,
    pub reason: Option<String>,
    pub comment: Option<String>,
    pub shelve_seconds: Option<u64>,
    pub previous_priority: Option<u16>,
    pub new_priority: Option<u16>,
}
pub(super) struct MessageRow {
    pub common: EventLogRow,
    pub message_template: String,
    pub severity: Option<u16>,
    pub severity_label: Option<String>,
    pub args: Vec<TypedDisplay>,
}
pub(super) struct TypedDisplay {
    pub type_name: String,
    pub display: String,
}
pub(super) struct StateRow {
    pub common: EventLogRow,
    pub state_machine: String,
    pub state_category: String,
    pub previous_state: String,
    pub previous_state_label: Option<String>,
    pub new_state: String,
    pub new_state_label: Option<String>,
}
pub(super) struct BatchRow {
    pub common: EventLogRow,
    pub batch_id: String,
    pub recipe_id: Option<String>,
    pub previous_state: Option<String>,
    pub new_state: String,
    pub new_state_label: Option<String>,
}
pub(super) struct RecipeRow {
    pub common: EventLogRow,
    pub action: String,
    pub recipe_id: String,
    pub recipe_version: Option<String>,
    pub batch_id: Option<String>,
    pub actor: Option<String>,
    pub authorization_result: Option<String>,
}
pub(super) struct MaterialRow {
    pub common: EventLogRow,
    pub batch_id: String,
    pub material_id: String,
    pub quantity: f64,
    pub exact_quantity: String,
    pub unit: Option<String>,
}
pub(super) struct OperatorRow {
    pub common: EventLogRow,
    pub action: String,
    pub action_id: Option<String>,
    pub actor: Option<String>,
    pub workstation: Option<String>,
    pub role: Option<String>,
    pub authorization_result: Option<String>,
    pub reason: Option<String>,
    pub context_references: Option<String>,
}
pub(super) struct AuditRow {
    pub common: EventLogRow,
    pub action: String,
    pub target: String,
    pub actor: String,
    pub reason: String,
    pub authorization_result: Option<String>,
    pub value_type: String,
    pub previous_value: String,
    pub current_value: String,
    pub workstation: Option<String>,
}
pub(super) struct SignatureRow {
    pub common: EventLogRow,
    pub action_id: String,
    pub actor: String,
    pub meaning: String,
    pub authorization_result: Option<String>,
    pub signed_source_id: u32,
    pub signed_sequence: String,
}
pub(super) struct SystemRow {
    pub common: EventLogRow,
    pub event_name: String,
    pub interval_ms: Option<u64>,
    pub sequence_base: Option<String>,
    pub dropped_count: Option<String>,
    pub first_sequence: Option<String>,
    pub last_sequence: Option<String>,
    pub registered_source_id: Option<String>,
    pub previous_definition_hash: Option<String>,
    pub new_definition_hash: Option<String>,
    pub changed_epoch_id: Option<String>,
    pub clock_quality: Option<String>,
    pub produced_count: Option<String>,
    pub cold_start: Option<bool>,
}
pub(super) struct LossRow {
    pub record_id: String,
    pub received_time: String,
    pub received_time_ns: String,
    pub source: Option<String>,
    pub source_id: u32,
    pub buffer_id: u32,
    pub run_id: String,
    pub epoch_id: String,
    pub definition_hash: String,
    pub first_sequence: String,
    pub last_sequence: String,
    pub lost_count: String,
    pub basis: &'static str,
}
pub(super) struct UnresolvedRow {
    pub record_id: String,
    pub event_time: Option<String>,
    pub event_time_ns: Option<String>,
    pub received_time: String,
    pub received_time_ns: String,
    pub source: Option<String>,
    pub source_id: u32,
    pub buffer_id: u32,
    pub run_id: String,
    pub epoch_id: String,
    pub sequence: String,
    pub definition_hash: String,
    pub event_type_id: u32,
    pub reason: String,
    pub diagnostic_summary: Option<String>,
}

pub(super) fn project_domain_rows(
    document: &Document,
    definition: Option<&open_ot_definition::DefinitionFile>,
) -> Result<Vec<DomainRow>, PersistenceError> {
    let Document::Event(event) = document else {
        return non_event_row(document);
    };
    use open_ot_carriage::registry::*;
    let common = event_log_row(document)?.expect("event document");
    let row = match event.event_type_id {
        EVENT_MESSAGE => DomainRow::Message(MessageRow {
            common,
            message_template: required_display(event, KEY_MESSAGE_TEMPLATE_ID)?,
            severity: optional_u16(event, KEY_SEVERITY)?,
            severity_label: optional_label(event, KEY_SEVERITY),
            args: fields(event, KEY_ARG)
                .take(4)
                .map(|field| TypedDisplay {
                    type_name: field.type_name.clone(),
                    display: display(&field.value),
                })
                .collect(),
        }),
        EVENT_STATE_TRANSITION => DomainRow::State(StateRow {
            common,
            state_machine: required_display(event, KEY_STATE_MACHINE_ID)?,
            state_category: required_label_or_display(event, KEY_CATEGORY)?,
            previous_state: required_display(event, KEY_PREVIOUS_STATE)?,
            previous_state_label: optional_label(event, KEY_PREVIOUS_STATE),
            new_state: required_display(event, KEY_NEW_STATE)?,
            new_state_label: optional_label(event, KEY_NEW_STATE),
        }),
        EVENT_CONDITION_ACTIVE..=EVENT_REFRESH_END => DomainRow::Alarm(AlarmRow {
            common,
            condition: required_display(event, KEY_CONDITION_ID)?,
            condition_class: optional_u16(event, KEY_CONDITION_CLASS)?.map(|value| match value {
                0 => "Alarm".into(),
                1 => "Interlock".into(),
                other => other.to_string(),
            }),
            lifecycle_action: event.event_name.clone(),
            correlation_id: optional_display(event, KEY_CORRELATION_ID),
            severity: optional_u16(event, KEY_SEVERITY)?,
            severity_label: optional_label(event, KEY_SEVERITY),
            cause: joined(event, KEY_CAUSE_OPERAND),
            actor: optional_display(event, KEY_ACK_BY),
            reason: optional_display(event, KEY_REASON),
            comment: optional_display(event, KEY_COMMENT),
            shelve_seconds: optional_u64(event, KEY_SHELVE_SECS)?,
            previous_priority: optional_u16(event, KEY_PREVIOUS_PRIORITY)?,
            new_priority: optional_u16(event, KEY_NEW_PRIORITY)?,
        }),
        EVENT_RECIPE_LOADED | EVENT_RECIPE_APPROVED => DomainRow::Recipe(RecipeRow {
            common,
            action: event.event_name.clone(),
            recipe_id: required_display(event, KEY_RECIPE_ID)?,
            recipe_version: optional_display(event, KEY_RECIPE_VERSION),
            batch_id: optional_display(event, KEY_BATCH_ID),
            actor: optional_display(event, KEY_ACK_BY),
            authorization_result: optional_label_or_display(event, KEY_AUTH_RESULT),
        }),
        EVENT_BATCH_EVENT => DomainRow::Batch(BatchRow {
            common,
            batch_id: required_display(event, KEY_BATCH_ID)?,
            recipe_id: optional_display(event, KEY_RECIPE_ID),
            previous_state: optional_display(event, KEY_PREVIOUS_STATE),
            new_state: required_display(event, KEY_NEW_STATE)?,
            new_state_label: optional_label(event, KEY_NEW_STATE),
        }),
        EVENT_MATERIAL_ADDITION => {
            let quantity_field = required(event, KEY_QUANTITY)?;
            let quantity = quantity_field
                .value
                .as_f64()
                .ok_or_else(|| malformed(event, "quantity must be numeric"))?;
            let unit =
                optional_display(event, KEY_UNIT).map(|value| resolve_unit(definition, value));
            DomainRow::Material(MaterialRow {
                common,
                batch_id: required_display(event, KEY_BATCH_ID)?,
                material_id: required_display(event, KEY_MATERIAL_ID)?,
                quantity,
                exact_quantity: display(&quantity_field.value),
                unit,
            })
        }
        EVENT_OPERATOR_ACTION
        | EVENT_OPERATOR_LOGIN
        | EVENT_OPERATOR_LOGOUT
        | EVENT_SECURITY_ACCESS_FAILURE
        | EVENT_PROGRAM_DOWNLOAD => DomainRow::Operator(OperatorRow {
            common,
            action: event.event_name.clone(),
            action_id: optional_display(event, KEY_ACTION_ID),
            actor: optional_display(event, KEY_ACTOR),
            workstation: optional_display(event, KEY_WORKSTATION),
            role: optional_label_or_display(event, KEY_ROLE),
            authorization_result: optional_label_or_display(event, KEY_AUTH_RESULT),
            reason: optional_display(event, KEY_REASON),
            context_references: joined(event, KEY_CONTEXT_REF),
        }),
        EVENT_PARAMETER_CHANGE => {
            let target = required_display(event, KEY_VALUE_ID)?;
            let declared_type = definition
                .and_then(|definition| definition.values.iter().find(|value| value.name == target))
                .and_then(|value| tlv_type_spec(value.data_type))
                .map(|spec| spec.name.to_string())
                .unwrap_or_else(|| {
                    required(event, KEY_NEW_VALUE)
                        .map(|field| field.type_name.clone())
                        .unwrap_or_default()
                });
            DomainRow::Audit(AuditRow {
                common,
                action: event.event_name.clone(),
                target,
                actor: required_display(event, KEY_ACTOR)?,
                reason: required_display(event, KEY_REASON)?,
                authorization_result: optional_label_or_display(event, KEY_AUTH_RESULT),
                value_type: declared_type,
                previous_value: required_display(event, KEY_PREVIOUS_VALUE)?,
                current_value: required_display(event, KEY_NEW_VALUE)?,
                workstation: optional_display(event, KEY_WORKSTATION),
            })
        }
        EVENT_ESIGNATURE => DomainRow::Signature(SignatureRow {
            common,
            action_id: required_display(event, KEY_ACTION_ID)?,
            actor: required_display(event, KEY_ACTOR)?,
            meaning: required_label_or_display(event, KEY_SIGNATURE_MEANING)?,
            authorization_result: optional_label_or_display(event, KEY_AUTH_RESULT),
            signed_source_id: event.provenance.source.id,
            signed_sequence: required_display(event, KEY_SIGNED_EVENT_SEQ)?,
        }),
        EVENT_HEARTBEAT..=EVENT_SOURCE_HIGH_WATER => DomainRow::System(SystemRow {
            common,
            event_name: event.event_name.clone(),
            interval_ms: optional_u64(event, KEY_INTERVAL_MS)?,
            sequence_base: optional_display(event, KEY_SEQ_BASE),
            dropped_count: optional_display(event, KEY_DROPPED_COUNT),
            first_sequence: optional_display(event, KEY_FIRST_LOST_SEQ),
            last_sequence: optional_display(event, KEY_LAST_LOST_SEQ),
            registered_source_id: optional_display(event, KEY_REGISTERED_SOURCE_ID),
            previous_definition_hash: optional_display(event, KEY_DEF_HASH_OLD),
            new_definition_hash: optional_display(event, KEY_DEF_HASH_NEW),
            changed_epoch_id: optional_display(event, KEY_EPOCH_ID),
            clock_quality: optional_label_or_display(event, KEY_CLOCK_QUALITY),
            produced_count: optional_display(event, KEY_SOURCE_HIGH_WATER),
            cold_start: optional(event, KEY_COLD_START).and_then(|field| field.value.as_bool()),
        }),
        _ => return Ok(Vec::new()),
    };
    Ok(vec![row])
}

fn non_event_row(document: &Document) -> Result<Vec<DomainRow>, PersistenceError> {
    match document {
        Document::Loss(loss) => Ok(vec![DomainRow::Loss(LossRow {
            record_id: super::projection::document_identity(document).storage_key(),
            received_time: timestamp(loss.provenance.receive_time_ns)?,
            received_time_ns: loss.provenance.receive_time_ns.to_string(),
            source: loss.provenance.source.name.clone(),
            source_id: loss.provenance.source.id,
            buffer_id: loss.provenance.buffer_id,
            run_id: loss.provenance.run_id.to_string(),
            epoch_id: loss.provenance.epoch.id.to_string(),
            definition_hash: loss.provenance.epoch.definition_hash.clone(),
            first_sequence: loss.first_seq.to_string(),
            last_sequence: loss.last_seq.to_string(),
            lost_count: loss.count.to_string(),
            basis: match loss.basis {
                open_ot_document::LossBasis::Authoritative => "authoritative",
                open_ot_document::LossBasis::Inferred => "inferred",
            },
        })]),
        Document::Placeholder(value) => Ok(vec![DomainRow::Unresolved(UnresolvedRow {
            record_id: super::projection::document_identity(document).storage_key(),
            event_time: value.provenance.source_time_ns.map(timestamp).transpose()?,
            event_time_ns: value.provenance.source_time_ns.map(|time| time.to_string()),
            received_time: timestamp(value.provenance.receive_time_ns)?,
            received_time_ns: value.provenance.receive_time_ns.to_string(),
            source: value.provenance.source.name.clone(),
            source_id: value.provenance.source.id,
            buffer_id: value.provenance.buffer_id,
            run_id: value.provenance.run_id.to_string(),
            epoch_id: value.provenance.epoch.id.to_string(),
            sequence: value.seq.to_string(),
            definition_hash: value.provenance.epoch.definition_hash.clone(),
            event_type_id: value.event_type_id,
            reason: format!("{:?}", value.reason.kind),
            diagnostic_summary: value.reason.detail.as_ref().map(ToString::to_string),
        })]),
        Document::Event(_) => unreachable!(),
    }
}

fn fields(event: &EventDocument, key: u16) -> impl Iterator<Item = &DocumentField> {
    event.fields.iter().filter(move |field| field.key == key)
}
fn required(event: &EventDocument, key: u16) -> Result<&DocumentField, PersistenceError> {
    fields(event, key)
        .next()
        .ok_or_else(|| malformed(event, &format!("missing field {key:#06x}")))
}
fn optional(event: &EventDocument, key: u16) -> Option<&DocumentField> {
    fields(event, key).next()
}
fn required_display(event: &EventDocument, key: u16) -> Result<String, PersistenceError> {
    required(event, key).map(|field| display(&field.value))
}
fn optional_display(event: &EventDocument, key: u16) -> Option<String> {
    optional(event, key).map(|field| display(&field.value))
}
fn required_label_or_display(event: &EventDocument, key: u16) -> Result<String, PersistenceError> {
    required(event, key).map(|field| {
        field
            .enum_label
            .clone()
            .unwrap_or_else(|| display(&field.value))
    })
}
fn optional_label(event: &EventDocument, key: u16) -> Option<String> {
    optional(event, key).and_then(|field| field.enum_label.clone())
}
fn optional_label_or_display(event: &EventDocument, key: u16) -> Option<String> {
    optional(event, key).map(|field| {
        field
            .enum_label
            .clone()
            .unwrap_or_else(|| display(&field.value))
    })
}
fn optional_u64(event: &EventDocument, key: u16) -> Result<Option<u64>, PersistenceError> {
    optional(event, key)
        .map(|field| {
            field
                .value
                .as_u64()
                .ok_or_else(|| malformed(event, &format!("field {key:#06x} must be unsigned")))
        })
        .transpose()
}
fn optional_u16(event: &EventDocument, key: u16) -> Result<Option<u16>, PersistenceError> {
    optional_u64(event, key)?
        .map(|value| {
            u16::try_from(value)
                .map_err(|_| malformed(event, &format!("field {key:#06x} exceeds UINT")))
        })
        .transpose()
}
fn joined(event: &EventDocument, key: u16) -> Option<String> {
    let values = fields(event, key)
        .map(|field| display(&field.value))
        .collect::<Vec<_>>();
    (!values.is_empty()).then(|| values.join(", "))
}
fn display(value: &serde_json::Value) -> String {
    value
        .as_str()
        .map(str::to_string)
        .unwrap_or_else(|| value.to_string())
}
fn malformed(event: &EventDocument, detail: &str) -> PersistenceError {
    PersistenceError::Commit(format!(
        "logging projection malformed {}: {detail}",
        event.event_name
    ))
}
fn resolve_unit(definition: Option<&open_ot_definition::DefinitionFile>, value: String) -> String {
    value
        .parse::<u16>()
        .ok()
        .and_then(|id| {
            definition.and_then(|definition| {
                definition
                    .units
                    .iter()
                    .find(|unit| unit.unit_id == id)
                    .map(|unit| unit.symbol.clone())
            })
        })
        .unwrap_or(value)
}
fn timestamp(ns: u64) -> Result<String, PersistenceError> {
    let value = time::OffsetDateTime::from_unix_timestamp_nanos(i128::from(ns))
        .map_err(|error| PersistenceError::Commit(format!("logging timestamp {ns}: {error}")))?;
    Ok(format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}.{:09}Z",
        value.year(),
        u8::from(value.month()),
        value.day(),
        value.hour(),
        value.minute(),
        value.second(),
        value.nanosecond()
    ))
}
