use open_ot_document::{
    Document, DocumentEpoch, DocumentField, DocumentFlags, DocumentKind, DocumentSource,
    EpochRelation, EventDocument, LossBasis, LossDocument, PlaceholderDocument,
    PlaceholderReasonDocument, PlaceholderReasonKind, Provenance, RawSlot,
};

use super::contracts::{
    DocumentSink, InMemoryDocumentSink, PersistenceBatch, PersistenceCheckpoint,
};
use super::OpenOtDocumentSink;
use super::SqliteDocumentSink;

const CANONICAL_DOCUMENT_COUNT: usize = 37;

#[test]
fn sink_checkpoint_contract_reads_none_then_committed_cursor() {
    let mut sink = super::contracts::InMemoryDocumentSink::new();
    assert_eq!(
        sink.load_checkpoint(9, 1).expect("load empty checkpoint"),
        None
    );
    let batch = PersistenceBatch {
        documents: Vec::new(),
        checkpoint: PersistenceCheckpoint {
            buffer_id: 9,
            run_id: 1,
            cursor_abs: 123,
        },
    };
    sink.commit(&batch).expect("commit checkpoint");
    assert_eq!(
        sink.load_checkpoint(9, 1)
            .expect("load committed checkpoint"),
        Some(batch.checkpoint)
    );
    assert_eq!(
        sink.load_checkpoint(10, 1).expect("load other buffer"),
        None
    );
}
#[cfg(feature = "openot-real-database-tests")]
use super::InfluxDb3DocumentSink;
#[cfg(feature = "openot-real-database-tests")]
use super::MySqlDocumentSink;
#[cfg(feature = "openot-real-database-tests")]
use super::PostgreSqlDocumentSink;
#[cfg(feature = "openot-real-database-tests")]
use super::SqlServerDocumentSink;
#[cfg(feature = "openot-real-database-tests")]
use super::TimescaleDbDocumentSink;

fn provenance(source_id: u32) -> Provenance {
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

fn open_test_sqlite(path: &std::path::Path) -> Result<SqliteDocumentSink, super::PersistenceError> {
    SqliteDocumentSink::open_with_definitions(path, vec![open_ot_definition::sample_definition()])
}

fn heartbeat_document() -> Document {
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

fn definition_hash_hex(definition: &open_ot_definition::DefinitionFile) -> String {
    open_ot_definition::compute_content_hash(definition)
        .expect("hash logging definition")
        .carriage_hash
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn value_changed_document(
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

fn canonical_documents() -> Vec<Document> {
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
            let fields = match event_type_id {
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
            };
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

fn assert_canonical_jsons(mut actual: Vec<String>) {
    actual.sort();
    assert_eq!(actual, expected_canonical_jsons());
}

#[test]
fn in_memory_sink_commits_event_loss_placeholder_and_checkpoint_unchanged() {
    let documents = canonical_documents();
    let checkpoint = PersistenceCheckpoint {
        buffer_id: 7,
        run_id: 1,
        cursor_abs: 4096,
    };
    let batch = PersistenceBatch {
        documents: documents.clone(),
        checkpoint,
    };
    let mut sink = InMemoryDocumentSink::new();

    let outcome = sink.commit(&batch).expect("batch should commit atomically");

    assert_eq!(sink.documents, documents);
    assert_eq!(sink.checkpoint, Some(checkpoint));
    assert_eq!(outcome.inserted, CANONICAL_DOCUMENT_COUNT);
    assert_eq!(outcome.duplicated, 0);
    assert_eq!(outcome.checkpoint, checkpoint);
}

#[test]
fn failed_sink_commit_advances_neither_documents_nor_checkpoint() {
    let original_checkpoint = PersistenceCheckpoint {
        buffer_id: 7,
        run_id: 1,
        cursor_abs: 100,
    };
    let original_batch = PersistenceBatch {
        documents: vec![canonical_documents().remove(0)],
        checkpoint: original_checkpoint,
    };
    let mut sink = InMemoryDocumentSink::new();
    sink.commit(&original_batch).expect("baseline commit");
    let original_documents = sink.documents.clone();

    sink.fail_next_commit();
    let failed_batch = PersistenceBatch {
        documents: canonical_documents(),
        checkpoint: PersistenceCheckpoint {
            buffer_id: 7,
            run_id: 1,
            cursor_abs: 200,
        },
    };
    let result = sink.commit(&failed_batch);

    assert!(
        result.is_err(),
        "injected transaction failure must be visible"
    );
    assert_eq!(sink.documents, original_documents);
    assert_eq!(sink.checkpoint, Some(original_checkpoint));
}

#[test]
fn retrying_identical_batch_is_idempotent() {
    let documents = canonical_documents();
    let checkpoint = PersistenceCheckpoint {
        buffer_id: 7,
        run_id: 1,
        cursor_abs: 4096,
    };
    let batch = PersistenceBatch {
        documents: documents.clone(),
        checkpoint,
    };
    let mut sink = InMemoryDocumentSink::new();
    sink.commit(&batch).expect("first commit");

    let retried = sink.commit(&batch).expect("idempotent retry");

    assert_eq!(sink.documents, documents);
    assert_eq!(sink.checkpoint, Some(checkpoint));
    assert_eq!(retried.inserted, 0);
    assert_eq!(retried.duplicated, CANONICAL_DOCUMENT_COUNT);
}

#[test]
fn sink_rejects_checkpoint_regression_without_changing_durable_state() {
    let documents = canonical_documents();
    let mut sink = InMemoryDocumentSink::new();
    let committed = PersistenceBatch {
        documents: documents.clone(),
        checkpoint: PersistenceCheckpoint {
            buffer_id: 7,
            run_id: 1,
            cursor_abs: 200,
        },
    };
    sink.commit(&committed).expect("baseline commit");

    let stale = PersistenceBatch {
        documents: Vec::new(),
        checkpoint: PersistenceCheckpoint {
            buffer_id: 7,
            run_id: 1,
            cursor_abs: 100,
        },
    };
    let result = sink.commit(&stale);

    assert!(result.is_err(), "a stale cursor must fail closed");
    assert_eq!(sink.documents, documents);
    assert_eq!(sink.checkpoint, Some(committed.checkpoint));
}

#[test]
fn sqlite_sink_opens_real_database_and_applies_schema_v3() {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "trust-openot-persistence-{}-{stamp}",
        std::process::id()
    ));
    let path = root.join("openot.sqlite3");

    let sink = open_test_sqlite(&path)
        .unwrap_or_else(|error| panic!("SQLite migration failed: {error:?}"));

    assert!(path.is_file(), "SQLite database was not created");
    assert_eq!(sink.schema_version().expect("schema version"), 3);
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn sqlite_sink_exposes_heartbeat_through_descriptive_event_log() {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "trust-logging-event-log-{}-{stamp}",
        std::process::id()
    ));
    let path = root.join("trust-logging.sqlite3");
    let mut sink = open_test_sqlite(&path).expect("open SQLite logging database");
    sink.commit(&PersistenceBatch {
        documents: vec![heartbeat_document()],
        checkpoint: PersistenceCheckpoint {
            buffer_id: 7,
            run_id: u64::from(std::process::id()),
            cursor_abs: 64,
        },
    })
    .expect("commit heartbeat");
    drop(sink);

    let connection = rusqlite::Connection::open(&path).expect("inspect logging database");
    let public_event_log_exists: bool = connection
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='event_log')",
            [],
            |row| row.get(0),
        )
        .expect("inspect public event log");
    assert!(
        public_event_log_exists,
        "schema v3 must expose the descriptive event_log table"
    );
    let internal_record_exists: bool = connection
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='logging_records')",
            [],
            |row| row.get(0),
        )
        .expect("inspect internal logging records");
    assert!(
        internal_record_exists,
        "schema v3 must use the descriptive internal logging_records name"
    );
    let stored: (String, i64, i64, String) = connection
        .query_row(
            "SELECT event_name,event_type_id,source_id,sequence FROM event_log",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .expect("query heartbeat without JSON extraction");
    assert_eq!(
        stored,
        ("Heartbeat".to_string(), 0x0100, 66, "1".to_string())
    );
    drop(connection);
    std::fs::remove_dir_all(root).ok();
}

#[test]
fn sqlite_sink_projects_named_bool_and_full_ulint_without_json() {
    let definition = open_ot_definition::sample_definition();
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "trust-logging-typed-values-{}-{stamp}",
        std::process::id()
    ));
    let path = root.join("trust-logging.sqlite3");
    let mut sink = SqliteDocumentSink::open_with_definitions(&path, vec![definition.clone()])
        .expect("open SQLite with logging definition");
    sink.commit(&PersistenceBatch {
        documents: vec![
            value_changed_document(&definition, 2, 2003, "Bool", serde_json::json!(true)),
            value_changed_document(&definition, 3, 2009, "ULInt", serde_json::json!(u64::MAX)),
        ],
        checkpoint: PersistenceCheckpoint {
            buffer_id: 7,
            run_id: u64::from(std::process::id()),
            cursor_abs: 128,
        },
    })
    .expect("commit typed values");
    drop(sink);

    let connection = rusqlite::Connection::open(&path).expect("inspect typed logging values");
    let mut statement = connection
        .prepare(
            "SELECT value_name,value_type,boolean_value,unsigned_value,exact_value \
             FROM logged_values ORDER BY sequence",
        )
        .expect("query typed values without JSON");
    let rows = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, Option<bool>>(2)?,
                row.get::<_, Option<String>>(3)?,
                row.get::<_, String>(4)?,
            ))
        })
        .expect("read typed values")
        .collect::<Result<Vec<_>, _>>()
        .expect("collect typed values");
    assert_eq!(
        rows,
        vec![
            (
                "Enabled".to_string(),
                "BOOL".to_string(),
                Some(true),
                None,
                "true".to_string(),
            ),
            (
                "UnsignedLong".to_string(),
                "ULINT".to_string(),
                None,
                Some(u64::MAX.to_string()),
                u64::MAX.to_string(),
            ),
        ]
    );
    drop(statement);
    drop(connection);
    std::fs::remove_dir_all(root).ok();
}

#[test]
fn sqlite_public_read_model_exposes_common_columns_on_every_object() {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "trust-logging-common-columns-{}-{stamp}",
        std::process::id()
    ));
    let path = root.join("trust-logging.sqlite3");
    drop(open_test_sqlite(&path).expect("create SQLite public read model"));
    let connection = rusqlite::Connection::open(&path).expect("inspect SQLite public read model");
    let required = [
        "record_id",
        "event_time",
        "event_time_ns",
        "received_time",
        "received_time_ns",
        "source",
        "source_id",
        "source_path",
        "source_hierarchy",
        "buffer_id",
        "run_id",
        "epoch_id",
        "sequence",
        "definition_hash",
        "time_unsynced",
        "synthetic_record",
        "partial_payload",
    ];
    for object in [
        "event_log",
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
        "system_events",
        "data_loss",
        "unresolved_records",
    ] {
        let mut statement = connection
            .prepare(&format!("PRAGMA table_info({object})"))
            .expect("inspect public object columns");
        let columns = statement
            .query_map([], |row| row.get::<_, String>(1))
            .expect("query public object columns")
            .collect::<Result<std::collections::HashSet<_>, _>>()
            .expect("collect public object columns");
        for column in required {
            assert!(
                columns.contains(column),
                "public object {object} must expose common column {column}"
            );
        }
    }
    drop(connection);
    std::fs::remove_dir_all(root).ok();
}

#[test]
fn sqlite_sink_migrates_v1_checkpoint_and_separates_recreated_ring_run() {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "trust-openot-v1-migration-{}-{stamp}",
        std::process::id()
    ));
    std::fs::create_dir_all(&root).expect("migration root");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o700))
            .expect("secure migration root");
    }
    let path = root.join("openot.sqlite3");
    let connection = rusqlite::Connection::open(&path).expect("create v1 database");
    connection
        .execute_batch(
            "CREATE TABLE openot_documents( \
                 identity_key TEXT PRIMARY KEY NOT NULL, \
                 document_kind TEXT NOT NULL, buffer_id INTEGER NOT NULL, \
                 run_id BLOB NOT NULL, source_id INTEGER NOT NULL, epoch_id BLOB NOT NULL, \
                 seq BLOB, first_seq BLOB, last_seq BLOB, loss_basis TEXT, \
                 source_time_ns BLOB, receive_time_ns BLOB NOT NULL, \
                 event_type_id INTEGER, event_name TEXT, definition_hash TEXT NOT NULL, \
                 canonical_json TEXT NOT NULL); \
             CREATE TABLE openot_checkpoint( \
                 singleton INTEGER PRIMARY KEY CHECK(singleton=1), \
                 buffer_id INTEGER NOT NULL, \
                 cursor_abs BLOB NOT NULL CHECK(length(cursor_abs)=8)); \
             INSERT INTO openot_checkpoint(singleton,buffer_id,cursor_abs) \
                 VALUES(1,7,X'000000000000007B'); \
             PRAGMA user_version=1;",
        )
        .expect("seed schema v1");
    drop(connection);

    let mut sink = open_test_sqlite(&path).expect("migrate schema v1 to v3");

    assert_eq!(sink.schema_version().expect("schema version"), 3);
    assert_eq!(
        sink.load_checkpoint(7, 0).expect("migrated old run"),
        Some(PersistenceCheckpoint {
            buffer_id: 7,
            run_id: 0,
            cursor_abs: 123,
        })
    );
    assert_eq!(
        sink.load_checkpoint(7, 1).expect("recreated ring run"),
        None
    );
    std::fs::remove_dir_all(root).ok();
}

#[test]
fn sqlite_sink_rejects_newer_schema_without_mutating_it() {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "trust-openot-newer-schema-{}-{stamp}",
        std::process::id()
    ));
    std::fs::create_dir_all(&root).expect("schema root");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o700))
            .expect("secure schema root");
    }
    let path = root.join("openot.sqlite3");
    let connection = rusqlite::Connection::open(&path).expect("create newer database");
    connection
        .execute_batch("CREATE TABLE sentinel(value TEXT); PRAGMA user_version=4;")
        .expect("seed newer schema");
    drop(connection);

    let error = open_test_sqlite(&path).expect_err("newer schema must fail closed");

    assert!(format!("{error:?}").contains("newer than supported version 3"));
    let connection = rusqlite::Connection::open(&path).expect("reopen untouched database");
    assert_eq!(
        connection
            .query_row("PRAGMA user_version", [], |row| row.get::<_, u32>(0))
            .expect("schema version"),
        4
    );
    std::fs::remove_dir_all(root).ok();
}

#[test]
fn sqlite_sink_rejects_malformed_checkpoint_run_identity() {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "trust-openot-corrupt-checkpoint-{}-{stamp}",
        std::process::id()
    ));
    let path = root.join("openot.sqlite3");
    drop(open_test_sqlite(&path).expect("create schema"));
    let connection = rusqlite::Connection::open(&path).expect("open database for corruption");
    connection
        .execute_batch(
            "PRAGMA ignore_check_constraints=ON; \
             INSERT INTO logging_checkpoint(singleton,buffer_id,run_id,cursor_abs) \
             VALUES(1,7,X'01',X'000000000000007B');",
        )
        .expect("inject malformed durable bytes");
    drop(connection);
    let mut sink = open_test_sqlite(&path).expect("schema itself remains readable");

    let error = sink
        .load_checkpoint(7, 1)
        .expect_err("malformed checkpoint must fail closed");

    assert!(format!("{error:?}").contains("checkpoint run is not an 8-byte unsigned value"));
    std::fs::remove_dir_all(root).ok();
}

#[test]
fn sqlite_sink_rejects_malformed_stored_canonical_document_on_reopen() {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "trust-openot-corrupt-document-{}-{stamp}",
        std::process::id()
    ));
    let path = root.join("openot.sqlite3");
    let mut sink = open_test_sqlite(&path).expect("create schema");
    sink.commit(&PersistenceBatch {
        documents: canonical_documents(),
        checkpoint: PersistenceCheckpoint {
            buffer_id: 7,
            run_id: 1,
            cursor_abs: 24_576,
        },
    })
    .expect("commit valid canonical documents");
    drop(sink);
    let connection = rusqlite::Connection::open(&path).expect("open database for corruption");
    connection
        .execute(
            "UPDATE logging_records SET canonical_json='{not-json' WHERE identity_key=(SELECT MIN(identity_key) FROM logging_records)",
            [],
        )
        .expect("inject malformed canonical JSON");
    drop(connection);

    let error = open_test_sqlite(&path)
        .expect_err("malformed stored canonical document must fail closed at startup");

    assert!(format!("{error:?}").contains("malformed canonical document"));
    std::fs::remove_dir_all(root).ok();
}

#[test]
fn sqlite_sink_rejects_corrupt_database_bytes() {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "trust-openot-corrupt-database-{}-{stamp}",
        std::process::id()
    ));
    std::fs::create_dir_all(&root).expect("database root");
    let path = root.join("openot.sqlite3");
    std::fs::write(&path, b"not a sqlite database").expect("seed corrupt database");

    let _error = open_test_sqlite(&path).expect_err("corrupt database must fail closed");
    std::fs::remove_dir_all(root).ok();
}

#[cfg(unix)]
#[test]
fn sqlite_sink_rejects_read_only_database_before_accepting_work() {
    use std::os::unix::fs::PermissionsExt;

    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "trust-openot-read-only-database-{}-{stamp}",
        std::process::id()
    ));
    let path = root.join("openot.sqlite3");
    drop(open_test_sqlite(&path).expect("create valid database"));
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o400))
        .expect("make database read-only");

    let error = open_test_sqlite(&path).expect_err("read-only database must fail closed");

    assert!(format!("{error:?}").contains("read-only"));
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).ok();
    std::fs::remove_dir_all(root).ok();
}

#[test]
fn sqlite_uncommitted_child_transaction_body() {
    let Ok(path) = std::env::var("TRUST_OPENOT_SQLITE_CRASH_CHILD_PATH") else {
        return;
    };
    let connection = rusqlite::Connection::open(path).expect("open child database");
    connection
        .execute_batch(
            "BEGIN IMMEDIATE; \
             UPDATE logging_records SET canonical_json='partial-child-write'; \
             UPDATE logging_checkpoint SET cursor_abs=X'000000000000270F';",
        )
        .expect("stage uncommitted child transaction");
    std::process::exit(86);
}

#[test]
fn sqlite_process_termination_recovers_before_or_after_batch_never_partial() {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "trust-openot-sqlite-crash-{}-{stamp}",
        std::process::id()
    ));
    std::fs::create_dir_all(&root).expect("crash root");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o700))
            .expect("secure crash root");
    }
    let path = root.join("openot.sqlite3");
    let baseline = PersistenceBatch {
        documents: canonical_documents(),
        checkpoint: PersistenceCheckpoint {
            buffer_id: 7,
            run_id: 1,
            cursor_abs: 4096,
        },
    };
    let mut sink = open_test_sqlite(&path).expect("open baseline database");
    sink.commit(&baseline).expect("commit baseline");
    drop(sink);
    let child = std::process::Command::new(std::env::current_exe().expect("current test binary"))
        .args([
            "--exact",
            "openot_persistence::contract_tests::sqlite_uncommitted_child_transaction_body",
            "--nocapture",
        ])
        .env("TRUST_OPENOT_SQLITE_CRASH_CHILD_PATH", &path)
        .status()
        .expect("run crash child");
    assert_eq!(child.code(), Some(86));

    let mut recovered = open_test_sqlite(&path).expect("recover after child exit");
    assert_eq!(
        recovered
            .load_checkpoint(7, 1)
            .expect("recovered checkpoint"),
        Some(baseline.checkpoint)
    );
    let connection = rusqlite::Connection::open(&path).expect("inspect recovered documents");
    let partial: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM logging_records WHERE canonical_json='partial-child-write'",
            [],
            |row| row.get(0),
        )
        .expect("partial row count");
    assert_eq!(partial, 0);
    let count: i64 = connection
        .query_row("SELECT COUNT(*) FROM logging_records", [], |row| row.get(0))
        .expect("recovered document count");
    assert_eq!(count, CANONICAL_DOCUMENT_COUNT as i64);
    drop(connection);
    std::fs::remove_dir_all(root).ok();
}

#[cfg(all(feature = "openot-real-database-tests", target_os = "linux"))]
#[test]
fn sqlite_disk_full_on_isolated_bounded_filesystem_preserves_last_checkpoint() {
    use std::os::unix::fs::{MetadataExt, PermissionsExt};

    struct MountGuard(std::path::PathBuf);
    impl Drop for MountGuard {
        fn drop(&mut self) {
            let _ = std::process::Command::new("sudo")
                .args(["umount", self.0.to_string_lossy().as_ref()])
                .status();
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "trust-openot-bounded-fs-{}-{stamp}",
        std::process::id()
    ));
    std::fs::create_dir_all(&root).expect("bounded mountpoint");
    let owner = std::fs::metadata(&root).expect("mountpoint metadata");
    let mounted = std::process::Command::new("sudo")
        .args([
            "mount",
            "-t",
            "tmpfs",
            "-o",
            "size=1m,nosuid,nodev,noexec",
            "tmpfs",
            root.to_string_lossy().as_ref(),
        ])
        .status()
        .expect("mount bounded tmpfs");
    assert!(mounted.success());
    let _guard = MountGuard(root.clone());
    let owned = std::process::Command::new("sudo")
        .args([
            "chown",
            &format!("{}:{}", owner.uid(), owner.gid()),
            root.to_string_lossy().as_ref(),
        ])
        .status()
        .expect("own bounded tmpfs");
    assert!(owned.success());
    std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o700))
        .expect("secure bounded tmpfs");
    let path = root.join("openot.sqlite3");
    let mut sink = open_test_sqlite(&path).expect("schema fits bounded filesystem");
    let mut last_checkpoint = None;
    let mut full_error = None;
    for run_id in 1..=10_000u64 {
        let batch = PersistenceBatch {
            documents: canonical_documents_for_run(run_id),
            checkpoint: PersistenceCheckpoint {
                buffer_id: 7,
                run_id,
                cursor_abs: run_id.saturating_mul(4096),
            },
        };
        match sink.commit(&batch) {
            Ok(_) => last_checkpoint = Some(batch.checkpoint),
            Err(error) => {
                full_error = Some((batch.checkpoint, error));
                break;
            }
        }
    }
    let (failed_checkpoint, error) = full_error.expect("bounded filesystem must become full");
    assert!(
        format!("{error:?}").to_ascii_lowercase().contains("full"),
        "expected explicit full-disk error, got {error:?}"
    );
    assert_ne!(last_checkpoint, Some(failed_checkpoint));
    if let Some(last_checkpoint) = last_checkpoint {
        assert_eq!(
            sink.load_checkpoint(last_checkpoint.buffer_id, last_checkpoint.run_id)
                .expect("checkpoint remains readable after full disk"),
            Some(last_checkpoint)
        );
        assert_eq!(
            sink.load_checkpoint(failed_checkpoint.buffer_id, failed_checkpoint.run_id)
                .expect("failed checkpoint remains absent"),
            None
        );
    }
}

#[test]
fn sqlite_sink_creates_missing_parent_directory_for_configured_path() {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "trust-openot-persistence-parent-{}-{stamp}",
        std::process::id()
    ));
    let path = root.join("history/openot.sqlite3");

    let sink = open_test_sqlite(&path).expect("create SQLite parent and database");

    assert!(path.is_file());
    drop(sink);
    std::fs::remove_dir_all(root).ok();
}

#[cfg(unix)]
#[test]
fn sqlite_sink_rejects_group_or_world_writable_parent() {
    use std::os::unix::fs::PermissionsExt;

    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "trust-openot-persistence-insecure-parent-{}-{stamp}",
        std::process::id()
    ));
    std::fs::create_dir_all(&root).expect("create insecure parent");
    std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o777))
        .expect("set insecure permissions");

    let result = open_test_sqlite(&root.join("openot.sqlite3"));

    assert!(matches!(
        result,
        Err(super::PersistenceError::InvalidConfig(_))
    ));
    std::fs::remove_dir_all(root).ok();
}

#[test]
fn sqlite_sink_commits_documents_and_checkpoint_in_one_real_transaction() {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "trust-openot-persistence-commit-{}-{stamp}",
        std::process::id()
    ));
    let path = root.join("openot.sqlite3");
    let checkpoint = PersistenceCheckpoint {
        buffer_id: 7,
        run_id: 1,
        cursor_abs: 8192,
    };
    let batch = PersistenceBatch {
        documents: canonical_documents(),
        checkpoint,
    };
    let mut sink = open_test_sqlite(&path).expect("open SQLite sink");

    let outcome = sink.commit(&batch).expect("commit SQLite batch");
    drop(sink);

    assert_eq!(outcome.inserted, CANONICAL_DOCUMENT_COUNT);
    assert_eq!(outcome.duplicated, 0);
    assert_eq!(outcome.checkpoint, checkpoint);
    let connection = rusqlite::Connection::open(&path).expect("independent SQLite inspection");
    let document_count: i64 = connection
        .query_row("SELECT COUNT(*) FROM logging_records", [], |row| row.get(0))
        .expect("document count");
    let (buffer_id, cursor_abs): (u32, Vec<u8>) = connection
        .query_row(
            "SELECT buffer_id, cursor_abs FROM logging_checkpoint WHERE singleton = 1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("checkpoint");
    assert_eq!(document_count, CANONICAL_DOCUMENT_COUNT as i64);
    let canonical_jsons = connection
        .prepare("SELECT canonical_json FROM logging_records ORDER BY identity_key")
        .expect("prepare canonical SQLite query")
        .query_map([], |row| row.get::<_, String>(0))
        .expect("query canonical SQLite documents")
        .collect::<Result<Vec<_>, _>>()
        .expect("decode canonical SQLite documents");
    assert_canonical_jsons(canonical_jsons);
    assert_eq!(buffer_id, checkpoint.buffer_id);
    assert_eq!(cursor_abs, checkpoint.cursor_abs.to_be_bytes());
    drop(connection);
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn sink_factory_opens_only_toml_selected_sqlite_at_bundle_relative_path() {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "trust-openot-persistence-factory-{}-{stamp}",
        std::process::id()
    ));
    std::fs::create_dir_all(root.join("history")).expect("create test bundle");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(root.join("history"), std::fs::Permissions::from_mode(0o700))
            .expect("secure test database directory");
    }
    let config = crate::config::OpenOtPersistenceConfig {
        enabled: true,
        backend: Some(crate::config::OpenOtPersistenceBackend::Sqlite),
        sqlite: Some(crate::config::OpenOtSqlitePersistenceConfig {
            path: "history/openot.sqlite3".into(),
        }),
        ..Default::default()
    };

    let sink = OpenOtDocumentSink::open(&config, &root).expect("open selected SQLite sink");

    assert!(matches!(sink, OpenOtDocumentSink::Sqlite(_)));
    assert!(root.join("history/openot.sqlite3").is_file());
    drop(sink);
    let _ = std::fs::remove_dir_all(root);
}

#[cfg(not(feature = "openot-database-postgresql"))]
#[test]
fn sink_factory_rejects_recognized_backend_omitted_from_binary_without_fallback() {
    let config = crate::config::OpenOtPersistenceConfig {
        enabled: true,
        backend: Some(crate::config::OpenOtPersistenceBackend::PostgreSql),
        postgresql: Some(crate::config::OpenOtPostgreSqlPersistenceConfig {
            connection_url_env: "TRUST_TEST_MUST_NOT_BE_READ".into(),
            schema: "openot".into(),
            tls: crate::config::OpenOtPersistenceTlsMode::Require,
            ca_cert_path: Some("unused-ca.pem".into()),
        }),
        ..Default::default()
    };

    let error = OpenOtDocumentSink::open(&config, std::path::Path::new("/"))
        .expect_err("an omitted recognized backend must fail before reading its settings");

    assert_eq!(
        error.to_string(),
        "backend_not_available: runtime.openot.persistence.backend 'postgresql' is not compiled into this binary"
    );
}

#[cfg(feature = "openot-real-database-tests")]
#[test]
fn postgresql_sink_connects_to_real_tls_server_and_applies_schema_v3_read_model() {
    let connection_url = std::env::var("TRUST_TEST_OPENOT_POSTGRES_URL")
        .expect("TRUST_TEST_OPENOT_POSTGRES_URL must identify the reviewed real server");
    let ca_cert_path = std::env::var("TRUST_TEST_OPENOT_POSTGRES_CA")
        .expect("TRUST_TEST_OPENOT_POSTGRES_CA must identify its CA certificate");

    let mut sink = PostgreSqlDocumentSink::open(
        &connection_url,
        "openot",
        std::path::Path::new(&ca_cert_path),
    )
    .expect("connect and migrate real PostgreSQL");

    assert_eq!(sink.schema_version().expect("PostgreSQL schema version"), 3);
    let public_objects: i64 = sink
        .client
        .query_one(
            "SELECT COUNT(*) FROM information_schema.tables \
             WHERE table_schema=$1 AND table_name IN ('event_log','logged_values','alarm_history')",
            &[&sink.schema],
        )
        .expect("inspect PostgreSQL public read model")
        .get(0);
    assert_eq!(public_objects, 3);
    let internal_objects: i64 = sink
        .client
        .query_one(
            "SELECT COUNT(*) FROM information_schema.tables \
             WHERE table_schema=$1 AND table_name IN ('logging_schema','logging_records','logging_checkpoint')",
            &[&sink.schema],
        )
        .expect("inspect PostgreSQL internal logging objects")
        .get(0);
    assert_eq!(internal_objects, 3);
    let legacy_objects: i64 = sink
        .client
        .query_one(
            "SELECT COUNT(*) FROM information_schema.tables \
             WHERE table_schema=$1 AND table_name LIKE 'openot_%'",
            &[&sink.schema],
        )
        .expect("inspect PostgreSQL legacy OpenOT names")
        .get(0);
    assert_eq!(legacy_objects, 0);
}

#[cfg(feature = "openot-real-database-tests")]
#[test]
fn postgresql_sink_commits_documents_and_checkpoint_on_real_server() {
    let connection_url = std::env::var("TRUST_TEST_OPENOT_POSTGRES_URL")
        .expect("TRUST_TEST_OPENOT_POSTGRES_URL must identify the reviewed real server");
    let ca_cert_path = std::env::var("TRUST_TEST_OPENOT_POSTGRES_CA")
        .expect("TRUST_TEST_OPENOT_POSTGRES_CA must identify its CA certificate");
    let schema = format!("openot_commit_{}", std::process::id());
    let checkpoint = PersistenceCheckpoint {
        buffer_id: 7,
        run_id: 1,
        cursor_abs: 12288,
    };
    let batch = PersistenceBatch {
        documents: canonical_documents(),
        checkpoint,
    };
    let mut sink = PostgreSqlDocumentSink::open_with_definitions(
        &connection_url,
        &schema,
        std::path::Path::new(&ca_cert_path),
        vec![open_ot_definition::sample_definition()],
    )
    .expect("connect and migrate real PostgreSQL");

    let outcome = sink.commit(&batch).expect("commit PostgreSQL batch");

    assert_eq!(outcome.inserted, CANONICAL_DOCUMENT_COUNT);
    assert_eq!(outcome.duplicated, 0);
    assert_eq!(outcome.checkpoint, checkpoint);
    assert_eq!(
        sink.document_count().expect("document count"),
        CANONICAL_DOCUMENT_COUNT as i64
    );
    assert_eq!(
        sink.checkpoint().expect("checkpoint"),
        Some((
            checkpoint.buffer_id,
            checkpoint.run_id.to_be_bytes().to_vec(),
            checkpoint.cursor_abs.to_be_bytes().to_vec()
        ))
    );
    for (table, expected) in [
        ("event_log", 35_i64),
        ("logged_values", 2),
        ("alarm_history", 13),
        ("message_log", 1),
        ("state_history", 1),
        ("batch_history", 1),
        ("recipe_history", 2),
        ("material_additions", 1),
        ("operator_activity", 4),
        ("audit_log", 1),
        ("electronic_signatures", 1),
        ("system_events", 9),
        ("data_loss", 1),
        ("unresolved_records", 1),
    ] {
        let count: i64 = sink
            .client
            .query_one(
                &format!("SELECT COUNT(*) FROM \"{}\".{table}", sink.schema),
                &[],
            )
            .unwrap_or_else(|error| panic!("query PostgreSQL {table}: {error}"))
            .get(0);
        assert_eq!(count, expected, "PostgreSQL {table} projection count");
    }
    assert_canonical_jsons(
        sink.canonical_jsons()
            .expect("canonical PostgreSQL documents"),
    );
}

#[cfg(feature = "openot-real-database-tests")]
#[test]
fn sink_factory_opens_only_toml_selected_postgresql() {
    let ca_cert_path = std::env::var("TRUST_TEST_OPENOT_POSTGRES_CA")
        .expect("TRUST_TEST_OPENOT_POSTGRES_CA must identify its CA certificate");
    let config = crate::config::OpenOtPersistenceConfig {
        enabled: true,
        backend: Some(crate::config::OpenOtPersistenceBackend::PostgreSql),
        postgresql: Some(crate::config::OpenOtPostgreSqlPersistenceConfig {
            connection_url_env: "TRUST_TEST_OPENOT_POSTGRES_URL".into(),
            schema: format!("openot_factory_{}", std::process::id()).into(),
            tls: crate::config::OpenOtPersistenceTlsMode::Require,
            ca_cert_path: Some(ca_cert_path.into()),
        }),
        ..Default::default()
    };

    let sink = OpenOtDocumentSink::open(&config, std::path::Path::new("/"))
        .expect("open selected PostgreSQL sink");

    assert!(matches!(sink, OpenOtDocumentSink::PostgreSql(_)));
}

#[cfg(feature = "openot-real-database-tests")]
#[test]
fn timescaledb_sink_requires_real_extension_and_creates_hypertable() {
    let connection_url = std::env::var("TRUST_TEST_OPENOT_TIMESCALE_URL")
        .expect("TRUST_TEST_OPENOT_TIMESCALE_URL must identify the reviewed real server");
    let ca_cert_path = std::env::var("TRUST_TEST_OPENOT_TIMESCALE_CA")
        .expect("TRUST_TEST_OPENOT_TIMESCALE_CA must identify its CA certificate");
    let schema = format!("openot_timescale_{}", std::process::id());

    let mut sink = TimescaleDbDocumentSink::open(
        &connection_url,
        &schema,
        std::path::Path::new(&ca_cert_path),
    )
    .expect("connect and migrate real TimescaleDB");

    assert_eq!(
        sink.extension_version().expect("extension version"),
        "2.29.2"
    );
    assert!(sink.hypertable_exists().expect("hypertable query"));
}

#[cfg(feature = "openot-real-database-tests")]
#[test]
fn sink_factory_selects_timescaledb_and_commits_to_real_hypertable() {
    let ca_cert_path = std::env::var("TRUST_TEST_OPENOT_TIMESCALE_CA")
        .expect("TRUST_TEST_OPENOT_TIMESCALE_CA must identify its CA certificate");
    let config = crate::config::OpenOtPersistenceConfig {
        enabled: true,
        backend: Some(crate::config::OpenOtPersistenceBackend::TimescaleDb),
        timescaledb: Some(crate::config::OpenOtTimescaleDbPersistenceConfig {
            connection_url_env: "TRUST_TEST_OPENOT_TIMESCALE_URL".into(),
            schema: format!("openot_ts_factory_{}", std::process::id()).into(),
            tls: crate::config::OpenOtPersistenceTlsMode::Require,
            ca_cert_path: Some(ca_cert_path.into()),
        }),
        ..Default::default()
    };
    let checkpoint = PersistenceCheckpoint {
        buffer_id: 7,
        run_id: 1,
        cursor_abs: 16384,
    };
    let batch = PersistenceBatch {
        documents: canonical_documents(),
        checkpoint,
    };
    let mut sink = OpenOtDocumentSink::open_with_definitions(
        &config,
        std::path::Path::new("/"),
        &[open_ot_definition::sample_definition()],
    )
    .expect("open selected TimescaleDB sink");

    let outcome = sink.commit(&batch).expect("commit TimescaleDB batch");
    let OpenOtDocumentSink::TimescaleDb(timescale) = &mut sink else {
        panic!("TOML selection did not construct TimescaleDB");
    };
    assert_eq!(outcome.inserted, CANONICAL_DOCUMENT_COUNT);
    assert_eq!(outcome.checkpoint, checkpoint);
    assert_eq!(timescale.time_index_count().expect("time index count"), 35);
    assert_canonical_jsons(
        timescale
            .canonical_jsons()
            .expect("canonical TimescaleDB documents"),
    );
}

#[cfg(feature = "openot-real-database-tests")]
fn assert_mysql_protocol_product(url_env: &str, ca_env: &str, expected_version_fragment: &str) {
    let connection_url = std::env::var(url_env)
        .unwrap_or_else(|_| panic!("{url_env} must identify the reviewed real server"));
    let ca_cert_path = std::env::var(ca_env)
        .unwrap_or_else(|_| panic!("{ca_env} must identify its CA certificate"));
    let checkpoint = PersistenceCheckpoint {
        buffer_id: 7,
        run_id: 1,
        cursor_abs: 20480,
    };
    let batch = PersistenceBatch {
        documents: canonical_documents(),
        checkpoint,
    };
    let mut sink = MySqlDocumentSink::open_with_definitions(
        &connection_url,
        "openot",
        std::path::Path::new(&ca_cert_path),
        vec![open_ot_definition::sample_definition()],
    )
    .expect("connect and migrate real MySQL-protocol server");

    sink.reset_test_state()
        .expect("reset reviewed test database");
    assert_eq!(sink.schema_version().expect("schema version"), 3);
    assert_eq!(
        sink.internal_name_counts().expect("internal logging names"),
        (3, 0)
    );
    assert!(sink
        .server_version()
        .expect("server version")
        .contains(expected_version_fragment));
    assert_eq!(
        sink.identity_collation().expect("identity collation"),
        "ascii_bin",
        "document identity must use bytewise collation on both MySQL and MariaDB"
    );
    let outcome = sink.commit(&batch).expect("commit document batch");
    assert_eq!(outcome.inserted, CANONICAL_DOCUMENT_COUNT);
    assert_eq!(outcome.duplicated, 0);
    assert_eq!(outcome.checkpoint, checkpoint);
    assert_eq!(
        sink.document_count().expect("document count"),
        CANONICAL_DOCUMENT_COUNT as u64
    );
    assert_eq!(sink.public_count("event_log").expect("event count"), 35);
    assert_eq!(sink.public_count("logged_values").expect("value count"), 2);
    assert_eq!(sink.public_count("alarm_history").expect("alarm count"), 13);
    for (table, expected) in [
        ("message_log", 1),
        ("state_history", 1),
        ("batch_history", 1),
        ("recipe_history", 2),
        ("material_additions", 1),
        ("operator_activity", 4),
        ("audit_log", 1),
        ("electronic_signatures", 1),
        ("system_events", 9),
        ("data_loss", 1),
        ("unresolved_records", 1),
    ] {
        assert_eq!(
            sink.public_count(table)
                .unwrap_or_else(|error| panic!("query MySQL-protocol {table}: {error}")),
            expected,
            "MySQL-protocol {table} projection count"
        );
    }
    assert_eq!(
        sink.checkpoint().expect("checkpoint"),
        Some((
            checkpoint.buffer_id,
            checkpoint.run_id.to_be_bytes().to_vec(),
            checkpoint.cursor_abs.to_be_bytes().to_vec()
        ))
    );
    assert_canonical_jsons(sink.canonical_jsons().expect("canonical MySQL documents"));
    let retried = sink.commit(&batch).expect("retry identical document batch");
    assert_eq!(retried.inserted, 0);
    assert_eq!(retried.duplicated, CANONICAL_DOCUMENT_COUNT);
    assert_eq!(
        sink.document_count().expect("idempotent document count"),
        CANONICAL_DOCUMENT_COUNT as u64
    );
}

#[cfg(feature = "openot-real-database-tests")]
#[test]
fn mysql_sink_migrates_and_commits_on_real_mysql_8_4_lts() {
    assert_mysql_protocol_product(
        "TRUST_TEST_OPENOT_MYSQL_URL",
        "TRUST_TEST_OPENOT_MYSQL_CA",
        "8.4.11",
    );
}

#[cfg(feature = "openot-real-database-tests")]
#[test]
fn mysql_sink_migrates_and_commits_on_real_mariadb_11_8_lts() {
    assert_mysql_protocol_product(
        "TRUST_TEST_OPENOT_MARIADB_URL",
        "TRUST_TEST_OPENOT_MARIADB_CA",
        "11.8.8-MariaDB",
    );
}

#[cfg(feature = "openot-real-database-tests")]
#[test]
fn mysql_sink_backfills_populated_v2_with_shared_projector() {
    let connection_url = std::env::var("TRUST_TEST_OPENOT_MYSQL_URL").expect("MySQL URL");
    let ca = std::env::var("TRUST_TEST_OPENOT_MYSQL_CA").expect("MySQL CA");
    let definition = open_ot_definition::sample_definition();
    let mut sink = MySqlDocumentSink::open_with_definitions(
        &connection_url,
        "openot",
        std::path::Path::new(&ca),
        vec![definition.clone()],
    )
    .expect("open MySQL v3 seed");
    sink.reset_test_state().expect("reset MySQL v2 seed");
    sink.commit(&PersistenceBatch {
        documents: canonical_documents(),
        checkpoint: PersistenceCheckpoint {
            buffer_id: 7,
            run_id: 1,
            cursor_abs: 4096,
        },
    })
    .expect("seed populated MySQL history");
    sink.seed_v2_without_projections()
        .expect("seed MySQL schema v2 projection gap");
    drop(sink);

    let mut migrated = MySqlDocumentSink::open_with_definitions(
        &connection_url,
        "openot",
        std::path::Path::new(&ca),
        vec![definition],
    )
    .expect("migrate populated MySQL v2");
    assert_eq!(migrated.schema_version().expect("migrated version"), 3);
    assert_eq!(
        migrated
            .public_count("event_log")
            .expect("backfilled events"),
        35
    );
    assert_eq!(
        migrated
            .public_count("logged_values")
            .expect("backfilled values"),
        2
    );
    assert_eq!(
        migrated
            .public_count("alarm_history")
            .expect("backfilled alarms"),
        13
    );
}

#[cfg(feature = "openot-real-database-tests")]
#[test]
fn mariadb_sink_backfills_populated_v2_with_shared_projector() {
    let connection_url = std::env::var("TRUST_TEST_OPENOT_MARIADB_URL").expect("MariaDB URL");
    let ca = std::env::var("TRUST_TEST_OPENOT_MARIADB_CA").expect("MariaDB CA");
    let definition = open_ot_definition::sample_definition();
    let mut sink = MySqlDocumentSink::open_with_definitions(
        &connection_url,
        "openot",
        std::path::Path::new(&ca),
        vec![definition.clone()],
    )
    .expect("open MariaDB v3 seed");
    sink.reset_test_state().expect("reset MariaDB v2 seed");
    sink.commit(&PersistenceBatch {
        documents: canonical_documents(),
        checkpoint: PersistenceCheckpoint {
            buffer_id: 7,
            run_id: 1,
            cursor_abs: 4096,
        },
    })
    .expect("seed populated MariaDB history");
    sink.seed_v2_without_projections()
        .expect("seed MariaDB schema v2 projection gap");
    drop(sink);
    let mut migrated = MySqlDocumentSink::open_with_definitions(
        &connection_url,
        "openot",
        std::path::Path::new(&ca),
        vec![definition],
    )
    .expect("migrate populated MariaDB v2");
    assert_eq!(migrated.schema_version().expect("migrated version"), 3);
    assert_eq!(
        migrated
            .public_count("event_log")
            .expect("backfilled events"),
        35
    );
    assert_eq!(
        migrated
            .public_count("logged_values")
            .expect("backfilled values"),
        2
    );
    assert_eq!(
        migrated
            .public_count("alarm_history")
            .expect("backfilled alarms"),
        13
    );
}

#[cfg(feature = "openot-real-database-tests")]
#[test]
fn sink_factory_opens_toml_selected_mysql_adapter() {
    let ca_cert_path = std::env::var("TRUST_TEST_OPENOT_MYSQL_CA")
        .expect("TRUST_TEST_OPENOT_MYSQL_CA must identify its CA certificate");
    let config = crate::config::OpenOtPersistenceConfig {
        enabled: true,
        backend: Some(crate::config::OpenOtPersistenceBackend::MySql),
        mysql: Some(crate::config::OpenOtMySqlPersistenceConfig {
            connection_url_env: "TRUST_TEST_OPENOT_MYSQL_URL".into(),
            database: "openot".into(),
            tls: crate::config::OpenOtPersistenceTlsMode::Require,
            ca_cert_path: Some(ca_cert_path.into()),
        }),
        ..Default::default()
    };

    let sink = OpenOtDocumentSink::open(&config, std::path::Path::new("/"))
        .expect("open selected MySQL sink");

    assert!(matches!(sink, OpenOtDocumentSink::MySql(_)));
}

#[cfg(feature = "openot-real-database-tests")]
#[test]
fn sqlserver_sink_migrates_and_commits_on_real_sql_server_2025() {
    let connection_url = std::env::var("TRUST_TEST_OPENOT_SQLSERVER_URL")
        .expect("TRUST_TEST_OPENOT_SQLSERVER_URL must identify the reviewed real server");
    let ca_cert_path = std::env::var("TRUST_TEST_OPENOT_SQLSERVER_CA")
        .expect("TRUST_TEST_OPENOT_SQLSERVER_CA must identify its CA certificate");
    let schema = format!("openot_{}", std::process::id());
    let checkpoint = PersistenceCheckpoint {
        buffer_id: 7,
        run_id: 1,
        cursor_abs: 24576,
    };
    let batch = PersistenceBatch {
        documents: canonical_documents(),
        checkpoint,
    };
    let mut sink = SqlServerDocumentSink::open_with_definitions(
        &connection_url,
        &schema,
        std::path::Path::new(&ca_cert_path),
        vec![open_ot_definition::sample_definition()],
    )
    .expect("connect and migrate real SQL Server");

    assert_eq!(sink.schema_version().expect("schema version"), 3);
    assert_eq!(
        sink.internal_name_counts().expect("internal logging names"),
        (3, 0)
    );
    assert!(sink
        .product_version()
        .expect("product version")
        .starts_with("17.0.4075.5"));
    let outcome = sink.commit(&batch).expect("commit SQL Server batch");
    assert_eq!(outcome.inserted, CANONICAL_DOCUMENT_COUNT);
    assert_eq!(outcome.duplicated, 0);
    assert_eq!(outcome.checkpoint, checkpoint);
    assert_eq!(
        sink.document_count().expect("document count"),
        CANONICAL_DOCUMENT_COUNT as i64
    );
    assert_eq!(
        sink.invalid_json_count().expect("canonical JSON validity"),
        0
    );
    assert_eq!(sink.public_count("event_log").expect("event count"), 35);
    assert_eq!(sink.public_count("logged_values").expect("value count"), 2);
    assert_eq!(sink.public_count("alarm_history").expect("alarm count"), 13);
    for (table, expected) in [
        ("message_log", 1),
        ("state_history", 1),
        ("batch_history", 1),
        ("recipe_history", 2),
        ("material_additions", 1),
        ("operator_activity", 4),
        ("audit_log", 1),
        ("electronic_signatures", 1),
        ("system_events", 9),
        ("data_loss", 1),
        ("unresolved_records", 1),
    ] {
        assert_eq!(
            sink.public_count(table)
                .unwrap_or_else(|error| panic!("query SQL Server {table}: {error}")),
            expected,
            "SQL Server {table} projection count"
        );
    }
    assert_eq!(
        sink.checkpoint().expect("checkpoint"),
        Some((
            checkpoint.buffer_id,
            checkpoint.run_id.to_be_bytes().to_vec(),
            checkpoint.cursor_abs.to_be_bytes().to_vec()
        ))
    );
    assert_canonical_jsons(
        sink.canonical_jsons()
            .expect("canonical SQL Server documents"),
    );
    let retried = sink.commit(&batch).expect("idempotent retry");
    assert_eq!(retried.inserted, 0);
    assert_eq!(retried.duplicated, CANONICAL_DOCUMENT_COUNT);
}

#[cfg(feature = "openot-real-database-tests")]
#[test]
fn sqlserver_sink_backfills_populated_v2_with_shared_projector() {
    let connection_url = std::env::var("TRUST_TEST_OPENOT_SQLSERVER_URL").expect("SQL Server URL");
    let ca = std::env::var("TRUST_TEST_OPENOT_SQLSERVER_CA").expect("SQL Server CA");
    let schema = format!("openot_v2_{}", std::process::id());
    let definition = open_ot_definition::sample_definition();
    let mut sink = SqlServerDocumentSink::open_with_definitions(
        &connection_url,
        &schema,
        std::path::Path::new(&ca),
        vec![definition.clone()],
    )
    .expect("open SQL Server v3 seed");
    sink.commit(&PersistenceBatch {
        documents: canonical_documents(),
        checkpoint: PersistenceCheckpoint {
            buffer_id: 7,
            run_id: 1,
            cursor_abs: 4096,
        },
    })
    .expect("seed populated SQL Server history");
    sink.seed_v2_without_projections()
        .expect("seed SQL Server schema v2 projection gap");
    drop(sink);

    let mut migrated = SqlServerDocumentSink::open_with_definitions(
        &connection_url,
        &schema,
        std::path::Path::new(&ca),
        vec![definition],
    )
    .expect("migrate populated SQL Server v2");
    assert_eq!(migrated.schema_version().expect("migrated version"), 3);
    assert_eq!(
        migrated
            .public_count("event_log")
            .expect("backfilled events"),
        35
    );
    assert_eq!(
        migrated
            .public_count("logged_values")
            .expect("backfilled values"),
        2
    );
    assert_eq!(
        migrated
            .public_count("alarm_history")
            .expect("backfilled alarms"),
        13
    );
}

#[cfg(feature = "openot-real-database-tests")]
#[test]
fn sink_factory_opens_toml_selected_sqlserver_adapter() {
    let ca_cert_path = std::env::var("TRUST_TEST_OPENOT_SQLSERVER_CA")
        .expect("TRUST_TEST_OPENOT_SQLSERVER_CA must identify its CA certificate");
    let config = crate::config::OpenOtPersistenceConfig {
        enabled: true,
        backend: Some(crate::config::OpenOtPersistenceBackend::SqlServer),
        sqlserver: Some(crate::config::OpenOtSqlServerPersistenceConfig {
            connection_url_env: "TRUST_TEST_OPENOT_SQLSERVER_URL".into(),
            schema: format!("openot_factory_{}", std::process::id()).into(),
            tls: crate::config::OpenOtPersistenceTlsMode::Require,
            ca_cert_path: Some(ca_cert_path.into()),
        }),
        ..Default::default()
    };

    let sink = OpenOtDocumentSink::open(&config, std::path::Path::new("/"))
        .expect("open selected SQL Server sink");

    assert!(matches!(sink, OpenOtDocumentSink::SqlServer(_)));
}

#[cfg(feature = "openot-real-database-tests")]
#[test]
fn influxdb3_sink_spools_and_delivers_to_real_influxdb_3_core() {
    let host = std::env::var("TRUST_TEST_OPENOT_INFLUX_HOST")
        .expect("TRUST_TEST_OPENOT_INFLUX_HOST must identify the reviewed real server");
    let token = std::env::var("TRUST_TEST_OPENOT_INFLUX_TOKEN")
        .expect("TRUST_TEST_OPENOT_INFLUX_TOKEN must hold the reviewed test token");
    let ca = std::env::var("TRUST_TEST_OPENOT_INFLUX_CA")
        .expect("TRUST_TEST_OPENOT_INFLUX_CA must identify its CA certificate");
    let spool_root =
        std::env::temp_dir().join(format!("trust-openot-influx-spool-{}", std::process::id()));
    let spool = spool_root.join("spool.sqlite3");
    let checkpoint = PersistenceCheckpoint {
        buffer_id: 7,
        run_id: 1,
        cursor_abs: 28672,
    };
    let batch = PersistenceBatch {
        documents: canonical_documents(),
        checkpoint,
    };
    let mut sink = InfluxDb3DocumentSink::open_bounded_with_definitions(
        &host,
        &token,
        "openot",
        &spool,
        std::path::Path::new(&ca),
        u64::MAX,
        vec![open_ot_definition::sample_definition()],
    )
    .expect("open real InfluxDB 3 sink and spool");

    assert_eq!(sink.server_version().expect("server version"), "3.11.2");
    assert_eq!(
        sink.internal_name_counts().expect("InfluxDB spool names"),
        (4, 0)
    );
    let outcome = sink
        .commit(&batch)
        .expect("durably accept batch into spool");
    assert_eq!(outcome.inserted, CANONICAL_DOCUMENT_COUNT);
    assert_eq!(outcome.duplicated, 0);
    assert_eq!(outcome.checkpoint, checkpoint);
    assert_eq!(outcome.remote_pending, CANONICAL_DOCUMENT_COUNT);
    assert_eq!(
        sink.pending_part_count().expect("pending delivery parts"),
        110,
        "each canonical and typed InfluxDB point must have durable per-part state"
    );
    assert_eq!(sink.maintenance().expect("deliver accepted spool batch"), 0);
    assert_eq!(sink.pending_count().expect("pending spool count"), 0);
    assert_eq!(
        sink.remote_document_count_for_run(u64::from(std::process::id()))
            .expect("remote document count"),
        CANONICAL_DOCUMENT_COUNT as i64
    );
    assert_eq!(
        sink.remote_measurement_count_for_run("message_log", u64::from(std::process::id()))
            .expect("InfluxDB message count"),
        1
    );
    for (measurement, expected) in [
        ("event_log", 35),
        ("logged_values", 2),
        ("alarm_history", 13),
        ("state_history", 1),
        ("batch_history", 1),
        ("recipe_history", 2),
        ("material_additions", 1),
        ("operator_activity", 4),
        ("audit_log", 1),
        ("electronic_signatures", 1),
        ("system_events", 9),
        ("data_loss", 1),
        ("unresolved_records", 1),
    ] {
        assert_eq!(
            sink.remote_measurement_count_for_run(measurement, u64::from(std::process::id()),)
                .unwrap_or_else(|error| panic!("query InfluxDB {measurement}: {error}")),
            expected,
            "InfluxDB {measurement} projection count"
        );
    }
    assert_canonical_jsons(
        sink.remote_canonical_jsons_for_run(u64::from(std::process::id()))
            .expect("canonical InfluxDB documents"),
    );
    let retried = sink.commit(&batch).expect("idempotent InfluxDB retry");
    assert_eq!(retried.inserted, 0);
    assert_eq!(retried.duplicated, CANONICAL_DOCUMENT_COUNT);
    let _ = std::fs::remove_dir_all(spool_root);
}

#[cfg(feature = "openot-real-database-tests")]
#[test]
fn influxdb3_sink_accepts_during_outage_then_catches_up_in_order() {
    let host = std::env::var("TRUST_TEST_OPENOT_INFLUX_HOST").expect("real InfluxDB host");
    let token = std::env::var("TRUST_TEST_OPENOT_INFLUX_TOKEN").expect("real InfluxDB token");
    let ca = std::env::var("TRUST_TEST_OPENOT_INFLUX_CA").expect("real InfluxDB CA");
    let spool_root =
        std::env::temp_dir().join(format!("trust-openot-influx-outage-{}", std::process::id()));
    let spool = spool_root.join("spool.sqlite3");
    let checkpoint = PersistenceCheckpoint {
        buffer_id: 7,
        run_id: 1,
        cursor_abs: 32768,
    };
    let batch = PersistenceBatch {
        documents: canonical_documents(),
        checkpoint,
    };
    let mut sink = InfluxDb3DocumentSink::open_bounded_with_definitions(
        &host,
        &token,
        "openot",
        &spool,
        std::path::Path::new(&ca),
        u64::MAX,
        vec![open_ot_definition::sample_definition()],
    )
    .expect("open InfluxDB sink before outage");

    sink.set_host_for_test("https://127.0.0.1:1");
    let accepted = sink
        .commit(&batch)
        .expect("local spool remains acceptance authority");
    assert_eq!(accepted.inserted, CANONICAL_DOCUMENT_COUNT);
    assert_eq!(accepted.remote_pending, CANONICAL_DOCUMENT_COUNT);
    assert_eq!(
        sink.pending_count().expect("pending during outage"),
        CANONICAL_DOCUMENT_COUNT as i64
    );
    sink.set_host_for_test(&host);
    assert_eq!(sink.maintenance().expect("catch up after recovery"), 0);
    assert_eq!(sink.pending_count().expect("pending after recovery"), 0);
    let _ = std::fs::remove_dir_all(spool_root);
}

#[cfg(feature = "openot-real-database-tests")]
#[test]
fn influxdb3_sink_rejects_a_spool_limit_smaller_than_its_schema() {
    let host = std::env::var("TRUST_TEST_OPENOT_INFLUX_HOST").expect("real InfluxDB host");
    let token = std::env::var("TRUST_TEST_OPENOT_INFLUX_TOKEN").expect("real InfluxDB token");
    let ca = std::env::var("TRUST_TEST_OPENOT_INFLUX_CA").expect("real InfluxDB CA");
    let spool_root = std::env::temp_dir().join(format!(
        "trust-openot-influx-tiny-spool-{}",
        std::process::id()
    ));
    let result = InfluxDb3DocumentSink::open_bounded(
        &host,
        &token,
        "openot",
        &spool_root.join("spool.sqlite3"),
        std::path::Path::new(&ca),
        1,
    );

    assert!(matches!(
        result,
        Err(super::PersistenceError::InvalidConfig(_))
    ));
    let _ = std::fs::remove_dir_all(spool_root);
}

#[cfg(feature = "openot-real-database-tests")]
#[test]
fn influxdb3_spool_full_rolls_back_documents_and_checkpoint() {
    let host = std::env::var("TRUST_TEST_OPENOT_INFLUX_HOST").expect("real InfluxDB host");
    let token = std::env::var("TRUST_TEST_OPENOT_INFLUX_TOKEN").expect("real InfluxDB token");
    let ca = std::env::var("TRUST_TEST_OPENOT_INFLUX_CA").expect("real InfluxDB CA");
    let spool_root = std::env::temp_dir().join(format!(
        "trust-openot-influx-full-spool-{}",
        std::process::id()
    ));
    let spool = spool_root.join("spool.sqlite3");
    let initial = InfluxDb3DocumentSink::open_bounded_with_definitions(
        &host,
        &token,
        "openot",
        &spool,
        std::path::Path::new(&ca),
        u64::MAX,
        vec![open_ot_definition::sample_definition()],
    )
    .expect("create baseline spool");
    let schema_bytes = initial.spool_logical_bytes().expect("schema footprint");
    drop(initial);
    let mut bounded = InfluxDb3DocumentSink::open_bounded_with_definitions(
        &host,
        &token,
        "openot",
        &spool,
        std::path::Path::new(&ca),
        schema_bytes,
        vec![open_ot_definition::sample_definition()],
    )
    .expect("open exactly schema-sized spool");
    let batch = PersistenceBatch {
        documents: canonical_documents(),
        checkpoint: PersistenceCheckpoint {
            buffer_id: 7,
            run_id: 1,
            cursor_abs: 36_864,
        },
    };

    assert!(matches!(
        bounded.commit(&batch),
        Err(super::PersistenceError::CapacityExhausted(_))
    ));
    assert_eq!(bounded.pending_count().expect("pending after rollback"), 0);
    assert_eq!(
        bounded
            .load_checkpoint(7, 1)
            .expect("checkpoint after rollback"),
        None
    );
    let _ = std::fs::remove_dir_all(spool_root);
}

#[cfg(feature = "openot-real-database-tests")]
#[test]
fn sink_factory_opens_toml_selected_influxdb3_adapter() {
    let ca = std::env::var("TRUST_TEST_OPENOT_INFLUX_CA").expect("real InfluxDB CA");
    let root = std::env::temp_dir().join(format!(
        "trust-openot-influx-factory-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&root).expect("create factory bundle root");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o700))
            .expect("secure test spool directory");
    }
    let config = crate::config::OpenOtPersistenceConfig {
        enabled: true,
        backend: Some(crate::config::OpenOtPersistenceBackend::InfluxDb3),
        influxdb3: Some(crate::config::OpenOtInfluxDb3PersistenceConfig {
            host_env: "TRUST_TEST_OPENOT_INFLUX_HOST".into(),
            token_env: "TRUST_TEST_OPENOT_INFLUX_TOKEN".into(),
            database: "openot".into(),
            spool_path: "spool.sqlite3".into(),
            max_bytes: 1_073_741_824,
            ca_cert_path: Some(ca.into()),
        }),
        ..Default::default()
    };

    let mut sink = OpenOtDocumentSink::open_with_definitions(
        &config,
        &root,
        &[open_ot_definition::sample_definition()],
    )
    .expect("open selected InfluxDB 3 sink");
    assert!(matches!(sink, OpenOtDocumentSink::InfluxDb3(_)));
    let checkpoint = PersistenceCheckpoint {
        buffer_id: 7,
        run_id: u64::from(std::process::id()),
        cursor_abs: 49_152,
    };
    let outcome = sink
        .commit(&PersistenceBatch {
            documents: canonical_documents(),
            checkpoint,
        })
        .expect("factory-selected Influx sink accepts the canonical batch into its spool");
    assert_eq!(outcome.remote_pending, CANONICAL_DOCUMENT_COUNT);
    assert_eq!(
        sink.maintenance()
            .expect("factory-selected maintenance delivers the spool"),
        0
    );
    let OpenOtDocumentSink::InfluxDb3(influx) = &mut sink else {
        unreachable!("selected InfluxDB 3 variant changed")
    };
    assert_canonical_jsons(
        influx
            .remote_canonical_jsons_for_run(checkpoint.run_id)
            .expect("query factory-delivered canonical documents"),
    );
    drop(sink);
    let _ = std::fs::remove_dir_all(root);
}

#[cfg(feature = "openot-real-database-tests")]
#[derive(Clone, Copy)]
enum RealRestartProduct {
    PostgreSql,
    TimescaleDb,
    MySql,
    MariaDb,
    SqlServer,
    InfluxDb3,
}

#[cfg(feature = "openot-real-database-tests")]
impl RealRestartProduct {
    fn label(self) -> &'static str {
        match self {
            Self::PostgreSql => "postgresql",
            Self::TimescaleDb => "timescaledb",
            Self::MySql => "mysql",
            Self::MariaDb => "mariadb",
            Self::SqlServer => "sqlserver",
            Self::InfluxDb3 => "influxdb3",
        }
    }

    fn container_env(self) -> &'static str {
        match self {
            Self::PostgreSql => "TRUST_TEST_OPENOT_POSTGRES_CONTAINER",
            Self::TimescaleDb => "TRUST_TEST_OPENOT_TIMESCALE_CONTAINER",
            Self::MySql => "TRUST_TEST_OPENOT_MYSQL_CONTAINER",
            Self::MariaDb => "TRUST_TEST_OPENOT_MARIADB_CONTAINER",
            Self::SqlServer => "TRUST_TEST_OPENOT_SQLSERVER_CONTAINER",
            Self::InfluxDb3 => "TRUST_TEST_OPENOT_INFLUX_CONTAINER",
        }
    }

    fn config(self, root: &std::path::Path, stamp: u64) -> crate::config::OpenOtPersistenceConfig {
        use crate::config::{
            OpenOtInfluxDb3PersistenceConfig, OpenOtMySqlPersistenceConfig,
            OpenOtPersistenceBackend, OpenOtPersistenceConfig, OpenOtPersistenceTlsMode,
            OpenOtPostgreSqlPersistenceConfig, OpenOtSqlServerPersistenceConfig,
            OpenOtTimescaleDbPersistenceConfig,
        };

        let mut config = OpenOtPersistenceConfig {
            enabled: true,
            ..OpenOtPersistenceConfig::default()
        };
        let schema = format!("openot_restart_{stamp}");
        match self {
            Self::PostgreSql => {
                config.backend = Some(OpenOtPersistenceBackend::PostgreSql);
                config.postgresql = Some(OpenOtPostgreSqlPersistenceConfig {
                    connection_url_env: "TRUST_TEST_OPENOT_POSTGRES_URL".into(),
                    schema: schema.into(),
                    tls: OpenOtPersistenceTlsMode::Require,
                    ca_cert_path: Some(
                        std::env::var("TRUST_TEST_OPENOT_POSTGRES_CA")
                            .expect("PostgreSQL CA")
                            .into(),
                    ),
                });
            }
            Self::TimescaleDb => {
                config.backend = Some(OpenOtPersistenceBackend::TimescaleDb);
                config.timescaledb = Some(OpenOtTimescaleDbPersistenceConfig {
                    connection_url_env: "TRUST_TEST_OPENOT_TIMESCALE_URL".into(),
                    schema: schema.into(),
                    tls: OpenOtPersistenceTlsMode::Require,
                    ca_cert_path: Some(
                        std::env::var("TRUST_TEST_OPENOT_TIMESCALE_CA")
                            .expect("TimescaleDB CA")
                            .into(),
                    ),
                });
            }
            Self::MySql | Self::MariaDb => {
                let (url_env, ca_env) = match self {
                    Self::MySql => ("TRUST_TEST_OPENOT_MYSQL_URL", "TRUST_TEST_OPENOT_MYSQL_CA"),
                    Self::MariaDb => (
                        "TRUST_TEST_OPENOT_MARIADB_URL",
                        "TRUST_TEST_OPENOT_MARIADB_CA",
                    ),
                    _ => unreachable!(),
                };
                config.backend = Some(OpenOtPersistenceBackend::MySql);
                config.mysql = Some(OpenOtMySqlPersistenceConfig {
                    connection_url_env: url_env.into(),
                    database: "openot".into(),
                    tls: OpenOtPersistenceTlsMode::Require,
                    ca_cert_path: Some(std::env::var(ca_env).expect("MySQL-family CA").into()),
                });
            }
            Self::SqlServer => {
                config.backend = Some(OpenOtPersistenceBackend::SqlServer);
                config.sqlserver = Some(OpenOtSqlServerPersistenceConfig {
                    connection_url_env: "TRUST_TEST_OPENOT_SQLSERVER_URL".into(),
                    schema: schema.into(),
                    tls: OpenOtPersistenceTlsMode::Require,
                    ca_cert_path: Some(
                        std::env::var("TRUST_TEST_OPENOT_SQLSERVER_CA")
                            .expect("SQL Server CA")
                            .into(),
                    ),
                });
            }
            Self::InfluxDb3 => {
                config.backend = Some(OpenOtPersistenceBackend::InfluxDb3);
                config.influxdb3 = Some(OpenOtInfluxDb3PersistenceConfig {
                    host_env: "TRUST_TEST_OPENOT_INFLUX_HOST".into(),
                    token_env: "TRUST_TEST_OPENOT_INFLUX_TOKEN".into(),
                    database: "openot".into(),
                    spool_path: root.join("influx-spool.sqlite3"),
                    max_bytes: 1_073_741_824,
                    ca_cert_path: Some(
                        std::env::var("TRUST_TEST_OPENOT_INFLUX_CA")
                            .expect("InfluxDB CA")
                            .into(),
                    ),
                });
            }
        }
        config
    }
}

#[cfg(feature = "openot-real-database-tests")]
fn canonical_documents_for_run(run_id: u64) -> Vec<Document> {
    let mut documents = canonical_documents();
    for document in &mut documents {
        match document {
            Document::Event(document) => document.provenance.run_id = run_id,
            Document::Loss(document) => document.provenance.run_id = run_id,
            Document::Placeholder(document) => document.provenance.run_id = run_id,
        }
    }
    documents
}

#[cfg(feature = "openot-real-database-tests")]
fn assert_real_product_restart_recovery(product: RealRestartProduct) {
    struct RestartGuard(String);
    impl Drop for RestartGuard {
        fn drop(&mut self) {
            let _ = std::process::Command::new("docker")
                .args(["start", self.0.as_str()])
                .status();
        }
    }

    let container = std::env::var(product.container_env()).unwrap_or_else(|_| {
        panic!(
            "{} must identify the real container",
            product.container_env()
        )
    });
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock")
        .as_nanos() as u64;
    let root = std::env::temp_dir().join(format!(
        "trust-openot-{}-restart-{}-{stamp}",
        product.label(),
        std::process::id()
    ));
    std::fs::create_dir_all(&root).expect("restart test root");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o700))
            .expect("secure restart test root");
    }
    let config = product.config(&root, stamp);
    let mut sink = OpenOtDocumentSink::open_with_definitions(
        &config,
        &root,
        &[open_ot_definition::sample_definition()],
    )
    .unwrap_or_else(|error| panic!("open {} before restart: {error:?}", product.label()));
    let baseline = PersistenceBatch {
        documents: canonical_documents_for_run(stamp),
        checkpoint: PersistenceCheckpoint {
            buffer_id: 7,
            run_id: stamp,
            cursor_abs: 4096,
        },
    };
    sink.commit(&baseline)
        .unwrap_or_else(|error| panic!("commit {} baseline: {error:?}", product.label()));
    let stopped = std::process::Command::new("docker")
        .args(["stop", container.as_str()])
        .status()
        .expect("stop real database product");
    assert!(stopped.success(), "stop {}", product.label());
    let _restart_guard = RestartGuard(container.clone());
    let recovery = PersistenceBatch {
        documents: canonical_documents_for_run(stamp.saturating_add(1)),
        checkpoint: PersistenceCheckpoint {
            buffer_id: 7,
            run_id: stamp.saturating_add(1),
            cursor_abs: 8192,
        },
    };

    if matches!(product, RealRestartProduct::InfluxDb3) {
        let outcome = sink
            .commit(&recovery)
            .expect("InfluxDB outage must accept into its durable spool");
        assert_eq!(outcome.inserted, CANONICAL_DOCUMENT_COUNT);
        assert!(outcome.remote_pending >= CANONICAL_DOCUMENT_COUNT);
    } else {
        assert!(
            sink.commit(&recovery).is_err(),
            "{} outage must not be acknowledged as a remote commit",
            product.label()
        );
    }

    let started = std::process::Command::new("docker")
        .args(["start", container.as_str()])
        .status()
        .expect("restart real database product");
    assert!(started.success(), "restart {}", product.label());
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(90);
    loop {
        let result = if matches!(product, RealRestartProduct::InfluxDb3) {
            sink.maintenance().and_then(|pending| {
                if pending == 0 {
                    Ok(())
                } else {
                    Err(super::PersistenceError::Commit(format!(
                        "InfluxDB restart still has {pending} pending documents"
                    )))
                }
            })
        } else {
            OpenOtDocumentSink::open_with_definitions(
                &config,
                &root,
                &[open_ot_definition::sample_definition()],
            )
            .and_then(|mut reopened| reopened.commit(&recovery).map(|_| ()))
        };
        if result.is_ok() {
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "{} did not recover before deadline: {result:?}",
            product.label()
        );
        std::thread::sleep(std::time::Duration::from_millis(500));
    }
    let checkpoint = if matches!(product, RealRestartProduct::InfluxDb3) {
        sink.load_checkpoint(7, recovery.checkpoint.run_id)
    } else {
        OpenOtDocumentSink::open_with_definitions(
            &config,
            &root,
            &[open_ot_definition::sample_definition()],
        )
        .and_then(|mut verifier| verifier.load_checkpoint(7, recovery.checkpoint.run_id))
    }
    .expect("load recovery checkpoint");
    assert_eq!(checkpoint, Some(recovery.checkpoint));
    std::fs::remove_dir_all(root).ok();
}

#[cfg(feature = "openot-real-database-tests")]
#[test]
fn postgresql_real_server_restart_recovers_without_backend_fallback() {
    assert_real_product_restart_recovery(RealRestartProduct::PostgreSql);
}

#[cfg(feature = "openot-real-database-tests")]
#[test]
fn timescaledb_real_server_restart_recovers_without_plain_postgresql_fallback() {
    assert_real_product_restart_recovery(RealRestartProduct::TimescaleDb);
}

#[cfg(feature = "openot-real-database-tests")]
#[test]
fn mysql_real_server_restart_recovers_without_backend_fallback() {
    assert_real_product_restart_recovery(RealRestartProduct::MySql);
}

#[cfg(feature = "openot-real-database-tests")]
#[test]
fn mariadb_real_server_restart_recovers_on_the_shared_mysql_adapter() {
    assert_real_product_restart_recovery(RealRestartProduct::MariaDb);
}

#[cfg(feature = "openot-real-database-tests")]
#[test]
fn sqlserver_real_server_restart_recovers_without_backend_fallback() {
    assert_real_product_restart_recovery(RealRestartProduct::SqlServer);
}

#[cfg(feature = "openot-real-database-tests")]
#[test]
fn influxdb3_real_server_restart_drains_the_required_durable_spool() {
    assert_real_product_restart_recovery(RealRestartProduct::InfluxDb3);
}

#[cfg(feature = "openot-real-database-tests")]
#[test]
fn every_real_network_backend_migrates_v1_and_rejects_newer_schema() {
    fn downgrade(sink: &mut OpenOtDocumentSink) {
        match sink {
            OpenOtDocumentSink::PostgreSql(sink) => sink
                .downgrade_checkpoint_to_v1_for_test()
                .expect("downgrade PostgreSQL fixture"),
            OpenOtDocumentSink::TimescaleDb(sink) => sink
                .downgrade_checkpoint_to_v1_for_test()
                .expect("downgrade TimescaleDB fixture"),
            OpenOtDocumentSink::MySql(sink) => sink
                .downgrade_checkpoint_to_v1_for_test()
                .expect("downgrade MySQL-family fixture"),
            OpenOtDocumentSink::SqlServer(sink) => sink
                .downgrade_checkpoint_to_v1_for_test()
                .expect("downgrade SQL Server fixture"),
            OpenOtDocumentSink::InfluxDb3(sink) => sink
                .downgrade_checkpoint_to_v1_for_test()
                .expect("downgrade InfluxDB spool fixture"),
            OpenOtDocumentSink::Sqlite(_) => unreachable!("network matrix excludes SQLite"),
        }
    }

    fn set_version(sink: &mut OpenOtDocumentSink, version: u32) {
        match sink {
            OpenOtDocumentSink::PostgreSql(sink) => sink
                .set_schema_version_for_test(version)
                .expect("set PostgreSQL schema version"),
            OpenOtDocumentSink::TimescaleDb(sink) => sink
                .set_schema_version_for_test(version)
                .expect("set TimescaleDB schema version"),
            OpenOtDocumentSink::MySql(sink) => sink
                .set_schema_version_for_test(version)
                .expect("set MySQL-family schema version"),
            OpenOtDocumentSink::SqlServer(sink) => sink
                .set_schema_version_for_test(version)
                .expect("set SQL Server schema version"),
            OpenOtDocumentSink::InfluxDb3(sink) => sink
                .set_schema_version_for_test(version)
                .expect("set InfluxDB spool schema version"),
            OpenOtDocumentSink::Sqlite(_) => unreachable!("network matrix excludes SQLite"),
        }
    }

    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock")
        .as_nanos() as u64;
    for (index, product) in [
        RealRestartProduct::PostgreSql,
        RealRestartProduct::TimescaleDb,
        RealRestartProduct::MySql,
        RealRestartProduct::MariaDb,
        RealRestartProduct::SqlServer,
        RealRestartProduct::InfluxDb3,
    ]
    .into_iter()
    .enumerate()
    {
        let root = std::env::temp_dir().join(format!(
            "trust-openot-{}-migration-{}-{stamp}",
            product.label(),
            std::process::id()
        ));
        std::fs::create_dir_all(&root).expect("migration root");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o700))
                .expect("secure migration root");
        }
        let config = product.config(&root, stamp.saturating_add(index as u64));
        let mut v2 = OpenOtDocumentSink::open_with_definitions(
            &config,
            &root,
            &[open_ot_definition::sample_definition()],
        )
        .unwrap_or_else(|error| panic!("open {} v2 fixture: {error:?}", product.label()));
        downgrade(&mut v2);
        drop(v2);

        let mut migrated = OpenOtDocumentSink::open_with_definitions(
            &config,
            &root,
            &[open_ot_definition::sample_definition()],
        )
        .unwrap_or_else(|error| panic!("migrate {} v1 fixture: {error:?}", product.label()));
        let run_id = stamp.saturating_add(10_000 + index as u64);
        let batch = PersistenceBatch {
            documents: canonical_documents_for_run(run_id),
            checkpoint: PersistenceCheckpoint {
                buffer_id: 7,
                run_id,
                cursor_abs: 24_576,
            },
        };
        migrated
            .commit(&batch)
            .unwrap_or_else(|error| panic!("commit migrated {}: {error:?}", product.label()));
        assert_eq!(
            migrated
                .load_checkpoint(7, run_id)
                .unwrap_or_else(|error| panic!("load migrated {}: {error:?}", product.label())),
            Some(batch.checkpoint)
        );

        set_version(&mut migrated, 4);
        let error = OpenOtDocumentSink::open_with_definitions(
            &config,
            &root,
            &[open_ot_definition::sample_definition()],
        )
        .expect_err("newer schema must fail closed");
        assert!(
            format!("{error:?}").contains("newer"),
            "{} newer-schema error was not actionable: {error:?}",
            product.label()
        );
        set_version(&mut migrated, 3);
        drop(migrated);
        std::fs::remove_dir_all(root).ok();
    }
}

#[cfg(feature = "openot-real-database-tests")]
fn benchmark_real_sink(
    name: &str,
    sink: &mut OpenOtDocumentSink,
    first_run_id: u64,
) -> (f64, f64, std::time::Duration) {
    const CATCH_UP_BATCHES: u64 = 32;
    const SUSTAINED_BATCHES: u64 = 32;
    const SUSTAINED_INTERVAL: std::time::Duration = std::time::Duration::from_millis(124);
    let storage_before = sink
        .storage_bytes()
        .unwrap_or_else(|error| panic!("{name} storage before benchmark: {error:?}"));
    let cpu_before = process_cpu_ticks();
    let rss_before_kib = process_rss_kib();
    let mut canonical_payload_bytes = 0u64;
    let mut maintenance_elapsed = std::time::Duration::ZERO;
    let mut latencies = Vec::new();
    let catch_up_started = std::time::Instant::now();
    for offset in 0..CATCH_UP_BATCHES {
        let run_id = first_run_id.saturating_add(offset);
        let batch = PersistenceBatch {
            documents: canonical_documents_for_run(run_id),
            checkpoint: PersistenceCheckpoint {
                buffer_id: 7,
                run_id,
                cursor_abs: run_id.saturating_mul(4096),
            },
        };
        canonical_payload_bytes = canonical_payload_bytes.saturating_add(
            batch
                .documents
                .iter()
                .map(|document| {
                    open_ot_document::to_json(document)
                        .expect("benchmark canonical JSON")
                        .len() as u64
                })
                .sum::<u64>(),
        );
        let started = std::time::Instant::now();
        sink.commit(&batch)
            .unwrap_or_else(|error| panic!("{name} catch-up commit: {error:?}"));
        latencies.push(started.elapsed());
    }
    let maintenance_started = std::time::Instant::now();
    while sink
        .maintenance()
        .unwrap_or_else(|error| panic!("{name} catch-up maintenance: {error:?}"))
        > 0
    {}
    maintenance_elapsed += maintenance_started.elapsed();
    let catch_up_elapsed = catch_up_started.elapsed();
    let catch_up_rate = (CATCH_UP_BATCHES as f64 * CANONICAL_DOCUMENT_COUNT as f64)
        / catch_up_elapsed.as_secs_f64();
    latencies.sort_unstable();
    let p95_index = (latencies.len() * 95).div_ceil(100).saturating_sub(1);
    let p95 = latencies[p95_index];

    let sustained_started = std::time::Instant::now();
    for offset in 0..SUSTAINED_BATCHES {
        let run_id = first_run_id
            .saturating_add(CATCH_UP_BATCHES)
            .saturating_add(offset);
        let batch = PersistenceBatch {
            documents: canonical_documents_for_run(run_id),
            checkpoint: PersistenceCheckpoint {
                buffer_id: 7,
                run_id,
                cursor_abs: run_id.saturating_mul(4096),
            },
        };
        canonical_payload_bytes = canonical_payload_bytes.saturating_add(
            batch
                .documents
                .iter()
                .map(|document| {
                    open_ot_document::to_json(document)
                        .expect("benchmark canonical JSON")
                        .len() as u64
                })
                .sum::<u64>(),
        );
        sink.commit(&batch)
            .unwrap_or_else(|error| panic!("{name} sustained commit: {error:?}"));
        let target = sustained_started
            + SUSTAINED_INTERVAL.saturating_mul(u32::try_from(offset + 1).expect("batch count"));
        if let Some(remaining) = target.checked_duration_since(std::time::Instant::now()) {
            std::thread::sleep(remaining);
        }
    }
    let maintenance_started = std::time::Instant::now();
    while sink
        .maintenance()
        .unwrap_or_else(|error| panic!("{name} sustained maintenance: {error:?}"))
        > 0
    {}
    maintenance_elapsed += maintenance_started.elapsed();
    let sustained_elapsed = sustained_started.elapsed();
    let sustained_rate = (SUSTAINED_BATCHES as f64 * CANONICAL_DOCUMENT_COUNT as f64)
        / sustained_elapsed.as_secs_f64();
    let storage_after = sink
        .storage_bytes()
        .unwrap_or_else(|error| panic!("{name} storage after benchmark: {error:?}"));
    let storage_growth = storage_after.saturating_sub(storage_before);
    let write_amplification = storage_growth as f64 / canonical_payload_bytes.max(1) as f64;
    let cpu_ticks = process_cpu_ticks().saturating_sub(cpu_before);
    let rss_after_kib = process_rss_kib();
    eprintln!(
        "OpenOT benchmark {name}: sustained={sustained_rate:.1} docs/s catch_up={catch_up_rate:.1} docs/s p95_commit_ms={:.1} canonical_bytes={canonical_payload_bytes} storage_before={storage_before} storage_after={storage_after} storage_growth={storage_growth} write_amplification={write_amplification:.3} cpu_ticks={cpu_ticks} rss_before_kib={rss_before_kib} rss_after_kib={rss_after_kib} maintenance_ms={:.1}",
        p95.as_secs_f64() * 1000.0,
        maintenance_elapsed.as_secs_f64() * 1000.0,
    );
    (sustained_rate, catch_up_rate, p95)
}

#[cfg(all(feature = "openot-real-database-tests", target_os = "linux"))]
fn process_cpu_ticks() -> u64 {
    let stat = std::fs::read_to_string("/proc/self/stat").expect("read process CPU stat");
    let fields = stat
        .rsplit_once(") ")
        .expect("process stat command delimiter")
        .1
        .split_whitespace()
        .collect::<Vec<_>>();
    fields[11]
        .parse::<u64>()
        .expect("process user CPU ticks")
        .saturating_add(fields[12].parse::<u64>().expect("process system CPU ticks"))
}

#[cfg(all(feature = "openot-real-database-tests", not(target_os = "linux")))]
fn process_cpu_ticks() -> u64 {
    0
}

#[cfg(all(feature = "openot-real-database-tests", target_os = "linux"))]
fn process_rss_kib() -> u64 {
    std::fs::read_to_string("/proc/self/status")
        .expect("read process status")
        .lines()
        .find_map(|line| {
            line.strip_prefix("VmRSS:")
                .and_then(|value| value.split_whitespace().next())
                .and_then(|value| value.parse().ok())
        })
        .expect("process resident memory")
}

#[cfg(all(feature = "openot-real-database-tests", not(target_os = "linux")))]
fn process_rss_kib() -> u64 {
    0
}

#[cfg(feature = "openot-real-database-tests")]
#[cfg_attr(
    debug_assertions,
    ignore = "OpenOT throughput floors are release-profile qualification"
)]
#[test]
fn every_real_backend_meets_openot_ingest_and_catch_up_qualification_floors() {
    use crate::config::{
        OpenOtPersistenceBackend, OpenOtPersistenceConfig, OpenOtSqlitePersistenceConfig,
    };

    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock")
        .as_nanos() as u64;
    let root = std::env::temp_dir().join(format!(
        "trust-openot-real-benchmark-{}-{stamp}",
        std::process::id()
    ));
    std::fs::create_dir_all(&root).expect("benchmark root");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o700))
            .expect("secure benchmark root");
    }
    let sqlite_config = OpenOtPersistenceConfig {
        enabled: true,
        backend: Some(OpenOtPersistenceBackend::Sqlite),
        sqlite: Some(OpenOtSqlitePersistenceConfig {
            path: root.join("sqlite/openot.sqlite3"),
        }),
        ..OpenOtPersistenceConfig::default()
    };
    let mut cases = Vec::new();
    cases.push(("SQLite", sqlite_config));
    for product in [
        RealRestartProduct::PostgreSql,
        RealRestartProduct::TimescaleDb,
        RealRestartProduct::MySql,
        RealRestartProduct::MariaDb,
        RealRestartProduct::SqlServer,
        RealRestartProduct::InfluxDb3,
    ] {
        let product_root = root.join(product.label());
        std::fs::create_dir_all(&product_root).expect("product benchmark root");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&product_root, std::fs::Permissions::from_mode(0o700))
                .expect("secure product benchmark root");
        }
        cases.push((product.label(), product.config(&product_root, stamp)));
    }

    for (index, (name, config)) in cases.into_iter().enumerate() {
        let mut sink = OpenOtDocumentSink::open_with_definitions(
            &config,
            &root,
            &[open_ot_definition::sample_definition()],
        )
        .unwrap_or_else(|error| panic!("open {name} benchmark sink: {error:?}"));
        let (sustained, catch_up, p95) = benchmark_real_sink(
            name,
            &mut sink,
            stamp.saturating_add((index as u64).saturating_mul(10_000)),
        );
        assert!(sustained >= 100.0, "{name} sustained {sustained:.1} docs/s");
        assert!(catch_up >= 250.0, "{name} catch-up {catch_up:.1} docs/s");
        assert!(
            p95 <= std::time::Duration::from_millis(500),
            "{name} p95 commit {p95:?}"
        );
    }
    std::fs::remove_dir_all(root).ok();
}
