use open_ot_document::{Document, LossBasis};
use std::collections::HashMap;

use super::PersistenceError;

pub(super) struct ProjectedDocument {
    pub(super) canonical: DocumentRow,
    pub(super) event: Option<EventLogRow>,
    pub(super) logged_values: Vec<LoggedValueRow>,
    pub(super) domains: Vec<super::projection_domains::DomainRow>,
}

impl ProjectedDocument {
    pub(super) fn public_row_count(&self) -> usize {
        usize::from(self.event.is_some()) + self.logged_values.len() + self.domains.len()
    }

    pub(super) fn has_unclassified_event(&self) -> bool {
        self.event
            .as_ref()
            .is_some_and(|event| event.has_unclassified_fields)
    }

    pub(super) fn loss_and_unresolved_counts(
        &self,
    ) -> Result<(usize, usize, u64), PersistenceError> {
        self.domains
            .iter()
            .try_fold((0usize, 0usize, 0u64), |mut counts, domain| {
                match domain {
                    super::projection_domains::DomainRow::Unresolved(_) => counts.0 += 1,
                    super::projection_domains::DomainRow::Loss(loss) => {
                        counts.1 += 1;
                        let lost = loss.lost_count.parse::<u64>().map_err(|error| {
                            PersistenceError::Commit(format!(
                                "logging loss projection contains invalid count: {error}"
                            ))
                        })?;
                        counts.2 = counts.2.saturating_add(lost);
                    }
                    _ => {}
                }
                Ok(counts)
            })
    }
}

pub(super) fn committed_special_counts(
    documents: &[ProjectedDocument],
) -> Result<(usize, usize, u64), PersistenceError> {
    documents
        .iter()
        .try_fold((0usize, 0usize, 0u64), |counts, document| {
            let current = document.loss_and_unresolved_counts()?;
            Ok((
                counts.0 + current.0,
                counts.1 + current.1,
                counts.2.saturating_add(current.2),
            ))
        })
}

#[derive(Debug)]
pub(super) struct LoggingProjector {
    definitions: HashMap<String, open_ot_definition::DefinitionFile>,
}

impl LoggingProjector {
    pub(super) fn new(
        definitions: impl IntoIterator<Item = open_ot_definition::DefinitionFile>,
    ) -> Result<Self, PersistenceError> {
        let mut by_carriage_hash = HashMap::new();
        for (index, definition) in definitions.into_iter().enumerate() {
            let hash = open_ot_definition::compute_content_hash(&definition).map_err(|error| {
                PersistenceError::InvalidConfig(format!(
                    "compute OpenOT definition hash for logging projection: {error}"
                ))
            })?;
            if !definition.header.content_hash.is_empty()
                && definition.header.content_hash != hash.content_hash
            {
                return Err(PersistenceError::InvalidConfig(format!(
                    "OpenOT logging definition content hash mismatch: declared {}, computed {}",
                    definition.header.content_hash, hash.content_hash
                )));
            }
            let key = hash
                .carriage_hash
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>();
            if by_carriage_hash
                .insert(key.clone(), definition.clone())
                .is_some()
            {
                return Err(PersistenceError::InvalidConfig(format!(
                    "duplicate OpenOT logging definition carriage hash {key}"
                )));
            }
            if index == 0 {
                by_carriage_hash.insert("0000000000000000".to_string(), definition);
            }
        }
        Ok(Self {
            definitions: by_carriage_hash,
        })
    }

    pub(super) fn project(
        &self,
        document: &Document,
    ) -> Result<ProjectedDocument, PersistenceError> {
        let logged_values = match document {
            Document::Event(event)
                if matches!(
                    event.event_type_id,
                    open_ot_carriage::registry::EVENT_VALUE_CHANGED
                        | open_ot_carriage::registry::EVENT_PARAMETER_CHANGE
                ) =>
            {
                let definition_hash = document_definition_hash(document);
                let definition = self.definitions.get(definition_hash).ok_or_else(|| {
                    PersistenceError::Commit(format!(
                        "logging projection missing definition {definition_hash} for {}",
                        document_identity(document).storage_key()
                    ))
                })?;
                vec![logged_value_row(document, event, definition)?]
            }
            _ => Vec::new(),
        };
        let domains = super::projection_domains::project_domain_rows(
            document,
            self.definitions.get(document_definition_hash(document)),
        )?;
        Ok(ProjectedDocument {
            canonical: document_row(document)?,
            event: event_log_row(document)?,
            logged_values,
            domains,
        })
    }
}

fn document_definition_hash(document: &Document) -> &str {
    match document {
        Document::Event(event) => &event.provenance.epoch.definition_hash,
        Document::Loss(loss) => &loss.provenance.epoch.definition_hash,
        Document::Placeholder(placeholder) => &placeholder.provenance.epoch.definition_hash,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum DocumentIdentity {
    Record {
        buffer_id: u32,
        run_id: u64,
        source_id: u32,
        seq: u64,
    },
    Loss {
        buffer_id: u32,
        run_id: u64,
        source_id: u32,
        epoch_id: u64,
        first_seq: u64,
        last_seq: u64,
        basis: &'static str,
    },
}

impl DocumentIdentity {
    pub(super) fn storage_key(&self) -> String {
        match self {
            Self::Record {
                buffer_id,
                run_id,
                source_id,
                seq,
            } => format!("r:{buffer_id:08x}:{run_id:016x}:{source_id:08x}:{seq:016x}"),
            Self::Loss {
                buffer_id,
                run_id,
                source_id,
                epoch_id,
                first_seq,
                last_seq,
                basis,
            } => format!(
                "l:{buffer_id:08x}:{run_id:016x}:{source_id:08x}:{epoch_id:016x}:{first_seq:016x}:{last_seq:016x}:{basis}"
            ),
        }
    }
}

pub(super) fn document_identity(document: &Document) -> DocumentIdentity {
    match document {
        Document::Event(event) => DocumentIdentity::Record {
            buffer_id: event.provenance.buffer_id,
            run_id: event.provenance.run_id,
            source_id: event.provenance.source.id,
            seq: event.seq,
        },
        Document::Placeholder(placeholder) => DocumentIdentity::Record {
            buffer_id: placeholder.provenance.buffer_id,
            run_id: placeholder.provenance.run_id,
            source_id: placeholder.provenance.source.id,
            seq: placeholder.seq,
        },
        Document::Loss(loss) => DocumentIdentity::Loss {
            buffer_id: loss.provenance.buffer_id,
            run_id: loss.provenance.run_id,
            source_id: loss.provenance.source.id,
            epoch_id: loss.provenance.epoch.id,
            first_seq: loss.first_seq,
            last_seq: loss.last_seq,
            basis: match loss.basis {
                LossBasis::Authoritative => "authoritative",
                LossBasis::Inferred => "inferred",
            },
        },
    }
}

pub(super) struct DocumentRow {
    pub(super) identity_key: String,
    pub(super) document_kind: &'static str,
    pub(super) buffer_id: u32,
    pub(super) run_id: [u8; 8],
    pub(super) source_id: u32,
    pub(super) epoch_id: [u8; 8],
    pub(super) seq: Option<Vec<u8>>,
    pub(super) first_seq: Option<Vec<u8>>,
    pub(super) last_seq: Option<Vec<u8>>,
    pub(super) loss_basis: Option<&'static str>,
    pub(super) source_time_ns: Option<Vec<u8>>,
    pub(super) receive_time_ns: [u8; 8],
    pub(super) event_type_id: Option<u32>,
    pub(super) event_name: Option<String>,
    pub(super) definition_hash: String,
    pub(super) canonical_json: String,
}

#[derive(Clone)]
pub(super) struct EventLogRow {
    pub(super) record_id: String,
    pub(super) event_time: Option<String>,
    pub(super) event_time_ns: Option<String>,
    pub(super) received_time: String,
    pub(super) received_time_ns: String,
    pub(super) source: Option<String>,
    pub(super) source_id: u32,
    pub(super) source_path: String,
    pub(super) source_hierarchy: String,
    pub(super) buffer_id: u32,
    pub(super) run_id: String,
    pub(super) epoch_id: String,
    pub(super) sequence: String,
    pub(super) definition_hash: String,
    pub(super) time_unsynced: bool,
    pub(super) synthetic_record: bool,
    pub(super) partial_payload: bool,
    pub(super) event_type_id: u32,
    pub(super) event_name: String,
    pub(super) has_unclassified_fields: bool,
}

pub(super) struct LoggedValueRow {
    pub(super) common: EventLogRow,
    pub(super) value_id: u32,
    pub(super) value_name: String,
    pub(super) value_type: &'static str,
    pub(super) unit: Option<String>,
    pub(super) quality: Option<u16>,
    pub(super) semantic_role: u16,
    pub(super) boolean_value: Option<bool>,
    pub(super) signed_value: Option<i64>,
    pub(super) unsigned_value: Option<String>,
    pub(super) number_value: Option<f64>,
    pub(super) text_value: Option<String>,
    pub(super) exact_value: String,
    pub(super) previous_boolean_value: Option<bool>,
    pub(super) previous_signed_value: Option<i64>,
    pub(super) previous_unsigned_value: Option<String>,
    pub(super) previous_number_value: Option<f64>,
    pub(super) previous_text_value: Option<String>,
    pub(super) previous_exact_value: Option<String>,
    pub(super) is_audited: bool,
    pub(super) actor: Option<String>,
    pub(super) reason: Option<String>,
    pub(super) authorization_result: Option<String>,
}

struct ValueLanes {
    boolean: Option<bool>,
    signed: Option<i64>,
    unsigned: Option<String>,
    number: Option<f64>,
    text: Option<String>,
    exact: String,
}

fn logged_value_row(
    document: &Document,
    event: &open_ot_document::EventDocument,
    definition: &open_ot_definition::DefinitionFile,
) -> Result<LoggedValueRow, PersistenceError> {
    use open_ot_carriage::registry::{
        KEY_ACTOR, KEY_AUTH_RESULT, KEY_NEW_VALUE, KEY_PREVIOUS_VALUE, KEY_QUALITY, KEY_REASON,
        KEY_VALUE_ID,
    };

    let value_reference = &required_field(event, KEY_VALUE_ID)?.value;
    let definition_value = if let Some(name) = value_reference.as_str() {
        definition
            .values
            .iter()
            .find(|candidate| candidate.name == name)
    } else {
        value_reference
            .as_u64()
            .or_else(|| {
                value_reference
                    .as_i64()
                    .and_then(|value| u64::try_from(value).ok())
            })
            .and_then(|value| u32::try_from(value).ok())
            .and_then(|value_id| {
                definition
                    .values
                    .iter()
                    .find(|candidate| candidate.value_id == value_id)
            })
    }
    .ok_or_else(|| {
        malformed_event(
            document,
            &format!("unknown value reference {value_reference}"),
        )
    })?;
    let value_id = definition_value.value_id;
    let current = value_lanes(
        definition_value.data_type,
        &required_field(event, KEY_NEW_VALUE)?.value,
        document,
    )?;
    let previous = optional_field(event, KEY_PREVIOUS_VALUE)
        .map(|field| value_lanes(definition_value.data_type, &field.value, document))
        .transpose()?;
    let quality = optional_field(event, KEY_QUALITY)
        .map(|field| {
            field
                .value
                .as_u64()
                .and_then(|value| u16::try_from(value).ok())
                .ok_or_else(|| malformed_event(document, "quality must be a UINT"))
        })
        .transpose()?;
    let unit = definition_value.unit.and_then(|unit_id| {
        definition
            .units
            .iter()
            .find(|candidate| candidate.unit_id == unit_id)
            .map(|unit| unit.symbol.clone())
    });
    let actor = optional_string(event, KEY_ACTOR, document)?;
    let reason = optional_string(event, KEY_REASON, document)?;
    let authorization_result = optional_field(event, KEY_AUTH_RESULT)
        .map(|field| {
            Ok(field
                .enum_label
                .clone()
                .unwrap_or_else(|| field.value.to_string()))
        })
        .transpose()?;
    let common = event_log_row(document)?.expect("value events are event documents");
    Ok(LoggedValueRow {
        common,
        value_id,
        value_name: definition_value.name.clone(),
        value_type: value_type_name(definition_value.data_type)
            .ok_or_else(|| malformed_event(document, "unsupported logged value type"))?,
        unit,
        quality,
        semantic_role: definition_value.semantic_role,
        boolean_value: current.boolean,
        signed_value: current.signed,
        unsigned_value: current.unsigned,
        number_value: current.number,
        text_value: current.text,
        exact_value: current.exact,
        previous_boolean_value: previous.as_ref().and_then(|value| value.boolean),
        previous_signed_value: previous.as_ref().and_then(|value| value.signed),
        previous_unsigned_value: previous.as_ref().and_then(|value| value.unsigned.clone()),
        previous_number_value: previous.as_ref().and_then(|value| value.number),
        previous_text_value: previous.as_ref().and_then(|value| value.text.clone()),
        previous_exact_value: previous.map(|value| value.exact),
        is_audited: event.event_type_id == open_ot_carriage::registry::EVENT_PARAMETER_CHANGE,
        actor,
        reason,
        authorization_result,
    })
}

fn required_field(
    event: &open_ot_document::EventDocument,
    key: u16,
) -> Result<&open_ot_document::DocumentField, PersistenceError> {
    let mut matching = event.fields.iter().filter(|field| field.key == key);
    let field = matching.next().ok_or_else(|| {
        PersistenceError::Commit(format!(
            "logging projection event {} missing required field key {key:#06x}",
            event.event_name
        ))
    })?;
    if matching.next().is_some() {
        return Err(PersistenceError::Commit(format!(
            "logging projection event {} repeats singular field key {key:#06x}",
            event.event_name
        )));
    }
    Ok(field)
}

fn optional_field(
    event: &open_ot_document::EventDocument,
    key: u16,
) -> Option<&open_ot_document::DocumentField> {
    event.fields.iter().find(|field| field.key == key)
}

fn optional_string(
    event: &open_ot_document::EventDocument,
    key: u16,
    document: &Document,
) -> Result<Option<String>, PersistenceError> {
    optional_field(event, key)
        .map(|field| {
            field
                .value
                .as_str()
                .map(str::to_string)
                .ok_or_else(|| malformed_event(document, &format!("field {key:#06x} is not text")))
        })
        .transpose()
}

fn value_type_name(data_type: u8) -> Option<&'static str> {
    use open_ot_carriage::registry::*;
    Some(match data_type {
        TY_BOOL => "BOOL",
        TY_SINT => "SINT",
        TY_USINT => "USINT",
        TY_INT => "INT",
        TY_UINT => "UINT",
        TY_DINT => "DINT",
        TY_UDINT => "UDINT",
        TY_LINT => "LINT",
        TY_ULINT => "ULINT",
        TY_REAL => "REAL",
        TY_LREAL => "LREAL",
        TY_DATE_TIME => "DATE_AND_TIME",
        TY_STRING => "STRING",
        _ => return None,
    })
}

fn value_lanes(
    data_type: u8,
    value: &serde_json::Value,
    document: &Document,
) -> Result<ValueLanes, PersistenceError> {
    use open_ot_carriage::registry::*;
    let mut lanes = ValueLanes {
        boolean: None,
        signed: None,
        unsigned: None,
        number: None,
        text: None,
        exact: String::new(),
    };
    match data_type {
        TY_BOOL => {
            let value = value
                .as_bool()
                .ok_or_else(|| malformed_event(document, "BOOL value is not Boolean"))?;
            lanes.boolean = Some(value);
            lanes.exact = value.to_string();
        }
        TY_SINT | TY_INT | TY_DINT | TY_LINT => {
            let value = value
                .as_i64()
                .ok_or_else(|| malformed_event(document, "signed value is not an integer"))?;
            lanes.signed = Some(value);
            lanes.exact = value.to_string();
        }
        TY_USINT | TY_UINT | TY_UDINT | TY_ULINT | TY_DATE_TIME => {
            let value = value
                .as_u64()
                .ok_or_else(|| malformed_event(document, "unsigned value is not an integer"))?;
            lanes.unsigned = Some(value.to_string());
            lanes.exact = value.to_string();
        }
        TY_REAL | TY_LREAL => {
            let value = value
                .as_f64()
                .filter(|value| value.is_finite())
                .ok_or_else(|| malformed_event(document, "floating value is not finite"))?;
            lanes.number = Some(value);
            lanes.exact = value.to_string();
        }
        TY_STRING => {
            let value = value
                .as_str()
                .ok_or_else(|| malformed_event(document, "STRING value is not text"))?;
            lanes.text = Some(value.to_string());
            lanes.exact = value.to_string();
        }
        _ => return Err(malformed_event(document, "unsupported logged value type")),
    }
    Ok(lanes)
}

fn malformed_event(document: &Document, detail: &str) -> PersistenceError {
    PersistenceError::Commit(format!(
        "logging projection malformed known event {}: {detail}",
        document_identity(document).storage_key()
    ))
}

pub(super) fn event_log_row(document: &Document) -> Result<Option<EventLogRow>, PersistenceError> {
    let Document::Event(event) = document else {
        return Ok(None);
    };
    let provenance = &event.provenance;
    Ok(Some(EventLogRow {
        record_id: document_identity(document).storage_key(),
        event_time: provenance
            .source_time_ns
            .map(format_timestamp)
            .transpose()?,
        event_time_ns: provenance.source_time_ns.map(|value| value.to_string()),
        received_time: format_timestamp(provenance.receive_time_ns)?,
        received_time_ns: provenance.receive_time_ns.to_string(),
        source: provenance.source.name.clone(),
        source_id: provenance.source.id,
        source_path: provenance.source.path.join("/"),
        source_hierarchy: provenance.source.hierarchy.join("/"),
        buffer_id: provenance.buffer_id,
        run_id: provenance.run_id.to_string(),
        epoch_id: provenance.epoch.id.to_string(),
        sequence: event.seq.to_string(),
        definition_hash: provenance.epoch.definition_hash.clone(),
        time_unsynced: provenance.flags.time_unsynced,
        synthetic_record: provenance.flags.synthetic_record,
        partial_payload: provenance.flags.partial_payload,
        event_type_id: event.event_type_id,
        event_name: event.event_name.clone(),
        has_unclassified_fields: !open_ot_carriage::registry::EVENT_SPECS
            .iter()
            .any(|spec| spec.id == event.event_type_id),
    }))
}

fn format_timestamp(timestamp_ns: u64) -> Result<String, PersistenceError> {
    let datetime = time::OffsetDateTime::from_unix_timestamp_nanos(i128::from(timestamp_ns))
        .map_err(|error| {
            PersistenceError::Commit(format!(
                "OpenOT timestamp {timestamp_ns} cannot be represented: {error}"
            ))
        })?;
    Ok(format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}.{:09}Z",
        datetime.year(),
        u8::from(datetime.month()),
        datetime.day(),
        datetime.hour(),
        datetime.minute(),
        datetime.second(),
        datetime.nanosecond()
    ))
}

pub(super) fn document_row(document: &Document) -> Result<DocumentRow, PersistenceError> {
    let canonical_json = open_ot_document::to_json(document)
        .map_err(|error| PersistenceError::Commit(format!("serialize OpenOT document: {error}")))?;
    let (
        document_kind,
        provenance,
        seq,
        first_seq,
        last_seq,
        loss_basis,
        event_type_id,
        event_name,
    ) = match document {
        Document::Event(event) => (
            "event",
            &event.provenance,
            Some(event.seq.to_be_bytes().to_vec()),
            None,
            None,
            None,
            Some(event.event_type_id),
            Some(event.event_name.clone()),
        ),
        Document::Placeholder(placeholder) => (
            "placeholder",
            &placeholder.provenance,
            Some(placeholder.seq.to_be_bytes().to_vec()),
            None,
            None,
            None,
            Some(placeholder.event_type_id),
            None,
        ),
        Document::Loss(loss) => (
            "loss",
            &loss.provenance,
            None,
            Some(loss.first_seq.to_be_bytes().to_vec()),
            Some(loss.last_seq.to_be_bytes().to_vec()),
            Some(match loss.basis {
                LossBasis::Authoritative => "authoritative",
                LossBasis::Inferred => "inferred",
            }),
            None,
            None,
        ),
    };
    Ok(DocumentRow {
        identity_key: document_identity(document).storage_key(),
        document_kind,
        buffer_id: provenance.buffer_id,
        run_id: provenance.run_id.to_be_bytes(),
        source_id: provenance.source.id,
        epoch_id: provenance.epoch.id.to_be_bytes(),
        seq,
        first_seq,
        last_seq,
        loss_basis,
        source_time_ns: provenance
            .source_time_ns
            .map(|value| value.to_be_bytes().to_vec()),
        receive_time_ns: provenance.receive_time_ns.to_be_bytes(),
        event_type_id,
        event_name,
        definition_hash: provenance.epoch.definition_hash.clone(),
        canonical_json,
    })
}
