use std::collections::BTreeMap;

use crate::harness::SourceFile;
use open_ot_definition::model::canonical_event_types;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use trust_hir::openot_authoring::OotKind;

use super::types::*;
use super::validation::{collect_authoring_model, validate_authoring_sources};
use super::ST_PRODUCER_MAX_RECORD_SIZE;

/// Generate a deterministic definition-file JSON document from OpenOT
/// declaration attributes.
///
/// The `contentHash` field is SHA-256 over the generated JSON with an empty
/// `contentHash`, encoded as lowercase hex.
pub fn definition_json_from_sources(sources: &[SourceFile]) -> Result<Value, String> {
    let validation_errors = validate_authoring_sources(sources);
    if !validation_errors.is_empty() {
        return Err(validation_errors.join("; "));
    }

    let annotations = collect_authoring_model(sources).annotations;
    if annotations.is_empty() {
        return Err("no OpenOT declaration attributes found".to_string());
    }

    let mut sources_by_id = BTreeMap::<u32, SourceDescriptor>::new();
    let mut values = Vec::new();
    let mut states = Vec::new();
    let mut conditions = Vec::new();
    let mut messages = Vec::new();
    let mut units_by_symbol = BTreeMap::<String, u16>::new();
    let mut enum_sets = BTreeMap::<String, Value>::new();

    for annotation in &annotations {
        sources_by_id
            .entry(annotation.source_id)
            .or_insert_with(|| annotation.source.clone());
        match annotation.kind {
            OotKind::Value => {
                let unit = annotation.unit.as_ref().map(|symbol| {
                    let unit_id = canonical_unit_id(symbol)
                        .unwrap_or_else(|| panic!("unit '{symbol}' should have been validated"));
                    units_by_symbol.entry(symbol.to_string()).or_insert(unit_id);
                    unit_id
                });
                values.push(json!({
                    "valueId": annotation.id,
                    "name": annotation.var_name,
                    "dataType": annotation.tlv_type(),
                    "semanticRole": annotation.semantic_role,
                    "unit": unit,
                    "deadband": annotation.deadband.as_ref().map(|decimal| json!({
                        "decimal": decimal,
                        "scaled": null
                    })),
                    "samplingPolicy": annotation.sampling_policy_json()
                }));
            }
            OotKind::State => {
                let enum_set = annotation.enum_state.as_ref().map_or_else(
                    || format!("{}States", annotation.var_name),
                    |state| state.enum_set_name.clone(),
                );
                if let Some(enum_state) = &annotation.enum_state {
                    enum_sets
                        .entry(enum_state.enum_set_name.clone())
                        .or_insert_with(|| enum_state.definition_json());
                }
                states.push(json!({
                    "stateMachineId": annotation.id,
                    "name": annotation.var_name,
                    "category": annotation.category,
                    "proceduralModel": annotation.model,
                    "enumSet": enum_set
                }));
            }
            OotKind::Alarm => {
                conditions.push(json!({
                    "conditionId": annotation.id,
                    "name": annotation.var_name,
                    "conditionClass": annotation.condition_class,
                    "defaultSeverity": annotation.severity,
                    "causeOperands": annotation
                        .cause_operand
                        .as_ref()
                        .map(|operand| vec![json!({
                            "operandId": operand.operand_id,
                            "name": operand.name
                        })])
                        .unwrap_or_default()
                }));
            }
            OotKind::Message => {
                messages.push(json!({
                    "messageTemplateId": annotation.id,
                    "name": annotation.var_name,
                    "format": annotation
                        .message
                        .clone()
                        .unwrap_or_else(|| annotation.var_name.clone()),
                    "argTypes": annotation
                        .message_args
                        .iter()
                        .map(MessageArgInfo::tlv_type)
                        .collect::<Vec<_>>()
                }));
            }
            OotKind::Condition => {}
            OotKind::Batch
            | OotKind::RecipeLoaded
            | OotKind::RecipeApproved
            | OotKind::MaterialAddition
            | OotKind::OperatorAction
            | OotKind::OperatorLogin
            | OotKind::OperatorLogout
            | OotKind::SecurityFailure
            | OotKind::ESignature => {}
        }
    }

    let units = units_by_symbol
        .iter()
        .map(|(symbol, unit_id)| {
            json!({
                "unitId": unit_id,
                "symbol": symbol
            })
        })
        .collect::<Vec<_>>();

    let sources_json = sources_by_id
        .iter()
        .map(|(source_id, source)| {
            json!({
                "sourceId": source_id,
                "name": source.name,
                "path": source.path,
                "hierarchy": source.hierarchy,
                "dynamic": false
            })
        })
        .collect::<Vec<_>>();

    let event_types = canonical_event_types()
        .into_iter()
        .map(|event_type| {
            serde_json::to_value(event_type).expect("canonical event type serializes")
        })
        .collect::<Vec<_>>();

    let mut definition = json!({
        "header": {
            "wireVersion": 2,
            "semanticVersion": "1.0.0",
            "profiles": ["Core", "Full"],
            "conformanceLevel": "Producer-Full",
            "caps": {
                "crc": true,
                "sourceHighWater": true
            },
            "constraints": {
                "maxRecordSize": ST_PRODUCER_MAX_RECORD_SIZE,
                "maxSlots": 16,
                "overflowPolicy": "overwrite-oldest"
            },
            "epochStrategy": "retain",
            "contentHash": ""
        },
        "eventTypes": event_types,
        "sources": sources_json,
        "stateMachines": states,
        "conditions": conditions,
        "messageTemplates": messages,
        "values": values,
        "units": units,
        "enumSets": enum_sets.into_values().collect::<Vec<_>>(),
        "recipeDefinitions": [],
        "batchDefinitions": [],
        "materialDefinitions": [],
        "operatorDefinitions": [],
        "eSignatureMeanings": [
            { "meaning": 0, "label": "Authored" },
            { "meaning": 1, "label": "Reviewed" },
            { "meaning": 2, "label": "Approved" },
            { "meaning": 3, "label": "Verified" },
            { "meaning": 4, "label": "Performed" },
            { "meaning": 5, "label": "Witnessed" }
        ],
        "severityScale": {
            "name": "baseline",
            "low": { "min": 1, "max": 332 },
            "medium": { "min": 333, "max": 666 },
            "high": { "min": 667, "max": 1000 }
        }
    });

    let canonical = serde_json::to_vec(&definition).map_err(|err| err.to_string())?;
    let digest = Sha256::digest(canonical);
    let hash = hex_lower(&digest);
    definition["header"]["contentHash"] = json!(hash);
    Ok(definition)
}

pub(super) fn canonical_unit_id(symbol: &str) -> Option<u16> {
    match symbol.to_ascii_lowercase().as_str() {
        "1" => Some(1),
        "l" => Some(2),
        "degc" => Some(3),
        "bar" => Some(4),
        "rpm" => Some(5),
        "s" => Some(6),
        "ms" => Some(7),
        "kg" => Some(8),
        "m" => Some(9),
        "%" => Some(10),
        _ => None,
    }
}

fn hex_lower(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}
