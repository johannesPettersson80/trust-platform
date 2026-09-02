use super::*;
pub(super) fn provenance(source_id: u32) -> Provenance {
    let definition = open_ot_definition::sample_definition();
    Provenance {
        buffer_id: 7,
        source: DocumentSource::unresolved(source_id),
        run_id: u64::from(std::process::id()),
        epoch: DocumentEpoch {
            id: 13,
            relation: EpochRelation::Current,
            definition_hash: definition_hash_hex(&definition),
            semantic_version: Some("1.0.0".to_string()),
        },
        source_time_ns: Some(17),
        receive_time_ns: 19,
        flags: DocumentFlags::default(),
    }
}

pub(super) fn open_test_sqlite(
    path: &std::path::Path,
) -> Result<SqliteDocumentSink, PersistenceError> {
    SqliteDocumentSink::open_with_definitions(path, vec![open_ot_definition::sample_definition()])
}

pub(super) fn heartbeat_document() -> Document {
    Document::Event(EventDocument {
        kind: DocumentKind::Event,
        provenance: provenance(66),
        event_name: "Heartbeat".to_string(),
        event_type_id: open_ot_carriage::registry::EVENT_HEARTBEAT,
        seq: 1,
        fields: Vec::new(),
        extension_fields: Vec::new(),
    })
}

pub(super) fn definition_hash_hex(definition: &open_ot_definition::DefinitionFile) -> String {
    open_ot_definition::compute_content_hash(definition)
        .expect("hash logging definition")
        .carriage_hash
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

pub(super) fn value_changed_document(
    definition: &open_ot_definition::DefinitionFile,
    seq: u64,
    value_id: u32,
    type_name: &str,
    value: serde_json::Value,
) -> Document {
    let mut provenance = provenance(66);
    provenance.source = DocumentSource {
        id: 66,
        name: Some("UnitA.Phase1".to_string()),
        path: vec![
            "Area1".to_string(),
            "UnitA".to_string(),
            "Phase1".to_string(),
        ],
        hierarchy: vec!["area".to_string(), "unit".to_string(), "phase".to_string()],
        dynamic: Some(false),
    };
    provenance.epoch.definition_hash = definition_hash_hex(definition);
    Document::Event(EventDocument {
        kind: DocumentKind::Event,
        provenance,
        event_name: "ValueChanged".to_string(),
        event_type_id: open_ot_carriage::registry::EVENT_VALUE_CHANGED,
        seq,
        fields: vec![
            DocumentField {
                key: open_ot_carriage::registry::KEY_VALUE_ID,
                name: "valueId".to_string(),
                type_name: "UDInt".to_string(),
                value: serde_json::json!(value_id),
                unit: None,
                enum_label: None,
            },
            DocumentField {
                key: open_ot_carriage::registry::KEY_NEW_VALUE,
                name: "newValue".to_string(),
                type_name: type_name.to_string(),
                value,
                unit: None,
                enum_label: None,
            },
        ],
        extension_fields: Vec::new(),
    })
}

fn field(key: u16, name: &str, value: serde_json::Value) -> DocumentField {
    DocumentField {
        key,
        name: name.to_string(),
        type_name: match &value {
            serde_json::Value::Bool(_) => "Bool",
            serde_json::Value::Number(_) => "ULInt",
            _ => "String",
        }
        .to_string(),
        value,
        unit: None,
        enum_label: None,
    }
}

fn canonical_event_fields(event_type_id: u32) -> Vec<DocumentField> {
    match event_type_id {
        open_ot_carriage::registry::EVENT_MESSAGE => vec![field(
            open_ot_carriage::registry::KEY_MESSAGE_TEMPLATE_ID,
            "messageTemplate",
            serde_json::json!("test message"),
        )],
        open_ot_carriage::registry::EVENT_STATE_TRANSITION => vec![
            field(
                open_ot_carriage::registry::KEY_STATE_MACHINE_ID,
                "stateMachine",
                serde_json::json!("Machine"),
            ),
            field(
                open_ot_carriage::registry::KEY_CATEGORY,
                "category",
                serde_json::json!("Operating"),
            ),
            field(
                open_ot_carriage::registry::KEY_PREVIOUS_STATE,
                "previousState",
                serde_json::json!("Idle"),
            ),
            field(
                open_ot_carriage::registry::KEY_NEW_STATE,
                "newState",
                serde_json::json!("Running"),
            ),
        ],
        open_ot_carriage::registry::EVENT_VALUE_CHANGED => vec![
            DocumentField {
                key: open_ot_carriage::registry::KEY_VALUE_ID,
                name: "valueId".into(),
                type_name: "UDInt".into(),
                value: serde_json::json!(2003),
                unit: None,
                enum_label: None,
            },
            DocumentField {
                key: open_ot_carriage::registry::KEY_NEW_VALUE,
                name: "newValue".into(),
                type_name: "Bool".into(),
                value: serde_json::json!(true),
                unit: None,
                enum_label: None,
            },
        ],
        open_ot_carriage::registry::EVENT_PARAMETER_CHANGE => vec![
            DocumentField {
                key: open_ot_carriage::registry::KEY_VALUE_ID,
                name: "valueId".into(),
                type_name: "UDInt".into(),
                value: serde_json::json!(2003),
                unit: None,
                enum_label: None,
            },
            DocumentField {
                key: open_ot_carriage::registry::KEY_PREVIOUS_VALUE,
                name: "previousValue".into(),
                type_name: "Bool".into(),
                value: serde_json::json!(false),
                unit: None,
                enum_label: None,
            },
            DocumentField {
                key: open_ot_carriage::registry::KEY_NEW_VALUE,
                name: "newValue".into(),
                type_name: "Bool".into(),
                value: serde_json::json!(true),
                unit: None,
                enum_label: None,
            },
            DocumentField {
                key: open_ot_carriage::registry::KEY_ACTOR,
                name: "actor".into(),
                type_name: "String".into(),
                value: serde_json::json!("operator-a"),
                unit: None,
                enum_label: None,
            },
            DocumentField {
                key: open_ot_carriage::registry::KEY_REASON,
                name: "reason".into(),
                type_name: "String".into(),
                value: serde_json::json!("approved change"),
                unit: None,
                enum_label: None,
            },
            DocumentField {
                key: open_ot_carriage::registry::KEY_AUTH_RESULT,
                name: "authorizationResult".into(),
                type_name: "Enum".into(),
                value: serde_json::json!(1),
                unit: None,
                enum_label: Some("authorized".into()),
            },
        ],
        open_ot_carriage::registry::EVENT_CONDITION_ACTIVE
            ..=open_ot_carriage::registry::EVENT_REFRESH_END => vec![field(
            open_ot_carriage::registry::KEY_CONDITION_ID,
            "condition",
            serde_json::json!("HighTemperature"),
        )],
        open_ot_carriage::registry::EVENT_RECIPE_LOADED
        | open_ot_carriage::registry::EVENT_RECIPE_APPROVED => vec![field(
            open_ot_carriage::registry::KEY_RECIPE_ID,
            "recipeId",
            serde_json::json!("Recipe-1"),
        )],
        open_ot_carriage::registry::EVENT_MATERIAL_ADDITION => vec![
            field(
                open_ot_carriage::registry::KEY_BATCH_ID,
                "batchId",
                serde_json::json!("Batch-1"),
            ),
            field(
                open_ot_carriage::registry::KEY_MATERIAL_ID,
                "materialId",
                serde_json::json!("Material-1"),
            ),
            field(
                open_ot_carriage::registry::KEY_QUANTITY,
                "quantity",
                serde_json::json!(12.5),
            ),
        ],
        open_ot_carriage::registry::EVENT_BATCH_EVENT => vec![
            field(
                open_ot_carriage::registry::KEY_BATCH_ID,
                "batchId",
                serde_json::json!("Batch-1"),
            ),
            field(
                open_ot_carriage::registry::KEY_NEW_STATE,
                "newState",
                serde_json::json!("Running"),
            ),
        ],
        open_ot_carriage::registry::EVENT_ESIGNATURE => vec![
            field(
                open_ot_carriage::registry::KEY_ACTION_ID,
                "actionId",
                serde_json::json!("Action-1"),
            ),
            field(
                open_ot_carriage::registry::KEY_ACTOR,
                "actor",
                serde_json::json!("operator-a"),
            ),
            field(
                open_ot_carriage::registry::KEY_SIGNATURE_MEANING,
                "meaning",
                serde_json::json!("Approved"),
            ),
            field(
                open_ot_carriage::registry::KEY_SIGNED_EVENT_SEQ,
                "signedSequence",
                serde_json::json!(1),
            ),
        ],
        _ => Vec::new(),
    }
}

pub(super) fn canonical_documents() -> Vec<Document> {
    let event_families = [
        ("Message", 0x0003),
        ("StateTransition", 0x0001),
        ("ValueChanged", 0x0002),
        ("ParameterChange", 0x0403),
        ("ConditionActive", 0x0200),
        ("ConditionCleared", 0x0201),
        ("ConditionAcknowledged", 0x0202),
        ("ConditionConfirmed", 0x0203),
        ("ConditionShelved", 0x0204),
        ("ConditionUnshelved", 0x0205),
        ("ConditionSuppressed", 0x0206),
        ("ConditionUnsuppressed", 0x0207),
        ("ConditionOutOfService", 0x0208),
        ("ConditionInService", 0x0209),
        ("ConditionCommented", 0x020A),
        ("ConditionReset", 0x020B),
        ("ConditionPriorityChanged", 0x020C),
        ("RecipeLoaded", 0x0301),
        ("RecipeApproved", 0x0302),
        ("MaterialAddition", 0x0304),
        ("BatchEvent", 0x0303),
        ("OperatorAction", 0x0400),
        ("OperatorLogin", 0x0401),
        ("OperatorLogout", 0x0402),
        ("SecurityAccessFailure", 0x0405),
        ("ESignature", 0x0404),
        ("Heartbeat", 0x0100),
        ("LoggerStarted", 0x0101),
        ("LoggerStopped", 0x0102),
        ("BufferCleared", 0x0103),
        ("RecordsDropped", 0x0104),
        ("SourceRegistered", 0x0105),
        ("DefinitionChanged", 0x0106),
        ("TimeSyncChanged", 0x0107),
        ("SourceHighWater", 0x0108),
    ];
    let mut documents = event_families
        .into_iter()
        .enumerate()
        .map(|(seq, (event_name, event_type_id))| {
            let fields = canonical_event_fields(event_type_id);
            Document::Event(EventDocument {
                kind: DocumentKind::Event,
                provenance: provenance(1),
                event_name: event_name.to_string(),
                event_type_id,
                seq: seq as u64,
                fields,
                extension_fields: Vec::new(),
            })
        })
        .collect::<Vec<_>>();
    documents.push(Document::Loss(LossDocument {
        kind: DocumentKind::Loss,
        provenance: provenance(1),
        first_seq: 35,
        last_seq: 36,
        count: 2,
        basis: LossBasis::Authoritative,
    }));
    documents.push(Document::Placeholder(PlaceholderDocument {
        kind: DocumentKind::Placeholder,
        provenance: provenance(1),
        event_type_id: 999,
        seq: 37,
        reason: PlaceholderReasonDocument {
            kind: PlaceholderReasonKind::UnknownEventId,
            detail: None,
        },
        raw_slots: vec![RawSlot {
            key: 44,
            type_tag: 5,
            payload_hex: "0102".to_string(),
        }],
    }));
    documents
}

fn expected_canonical_jsons() -> Vec<String> {
    let mut documents = canonical_documents()
        .iter()
        .map(|document| open_ot_document::to_json(document).expect("serialize canonical fixture"))
        .collect::<Vec<_>>();
    documents.sort();
    documents
}

pub(super) fn assert_canonical_jsons(mut actual: Vec<String>) {
    actual.sort();
    assert_eq!(actual, expected_canonical_jsons());
}
