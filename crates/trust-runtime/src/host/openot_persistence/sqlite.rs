use std::path::Path;

use rusqlite::{params, Connection, OptionalExtension};

use super::projection::LoggingProjector;
use super::{CommitOutcome, DocumentSink, PersistenceBatch, PersistenceError};

/// SQLite-backed durable OpenOT document sink.
#[derive(Debug)]
pub struct SqliteDocumentSink {
    connection: Connection,
    projector: LoggingProjector,
}

use super::contracts::LOGGING_SCHEMA_GENERATION;

impl SqliteDocumentSink {
    /// Opens an exact generation-1 database or initializes an empty one.
    pub fn open(path: &Path) -> Result<Self, PersistenceError> {
        Self::open_with_definitions(path, Vec::new())
    }

    #[doc(hidden)]
    pub fn open_with_definitions(
        path: &Path,
        definitions: Vec<open_ot_definition::DefinitionFile>,
    ) -> Result<Self, PersistenceError> {
        super::contracts::ensure_private_parent(path, "SQLite")?;
        if path
            .metadata()
            .is_ok_and(|metadata| metadata.permissions().readonly())
        {
            return Err(PersistenceError::InvalidConfig(
                "SQLite database is read-only".to_string(),
            ));
        }
        let mut connection = Connection::open(path).map_err(sqlite_error("open database"))?;
        connection
            .execute_batch(
                "PRAGMA foreign_keys = ON;\n\
                 PRAGMA journal_mode = WAL;\n\
                 PRAGMA synchronous = FULL;",
            )
            .map_err(sqlite_error("configure durability"))?;
        let projector = LoggingProjector::new(definitions)?;
        initialize_schema(&mut connection)?;
        validate_existing_documents(&connection)?;
        Ok(Self {
            connection,
            projector,
        })
    }

    /// Returns the compatible schema version opened by this sink.
    pub fn schema_version(&self) -> Result<u32, PersistenceError> {
        self.connection
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .map_err(sqlite_error("read schema version"))
    }

    #[cfg(all(test, feature = "openot-real-database-tests"))]
    pub(crate) fn storage_bytes(&self) -> Result<u64, PersistenceError> {
        let page_count: i64 = self
            .connection
            .query_row("PRAGMA page_count", [], |row| row.get(0))
            .map_err(sqlite_error("read page count"))?;
        let page_size: i64 = self
            .connection
            .query_row("PRAGMA page_size", [], |row| row.get(0))
            .map_err(sqlite_error("read page size"))?;
        let page_count = u64::try_from(page_count)
            .map_err(|_| PersistenceError::Commit("SQLite page count is negative".into()))?;
        let page_size = u64::try_from(page_size)
            .map_err(|_| PersistenceError::Commit("SQLite page size is negative".into()))?;
        Ok(page_count.saturating_mul(page_size))
    }
}

fn validate_existing_documents(connection: &Connection) -> Result<(), PersistenceError> {
    let malformed = connection
        .query_row(
            "SELECT identity_key FROM logging_records WHERE json_valid(canonical_json)=0 LIMIT 1",
            [],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(sqlite_error("validate stored canonical documents"))?;
    if let Some(identity) = malformed {
        return Err(PersistenceError::Commit(format!(
            "SQLite malformed canonical document at identity {identity}"
        )));
    }
    Ok(())
}

impl DocumentSink for SqliteDocumentSink {
    fn load_checkpoint(
        &mut self,
        buffer_id: u32,
        run_id: u64,
    ) -> Result<Option<super::PersistenceCheckpoint>, PersistenceError> {
        let stored = self
            .connection
            .query_row(
                "SELECT buffer_id, run_id, cursor_abs FROM logging_checkpoint WHERE singleton = 1",
                [],
                |row| {
                    Ok((
                        row.get::<_, u32>(0)?,
                        row.get::<_, Vec<u8>>(1)?,
                        row.get::<_, Vec<u8>>(2)?,
                    ))
                },
            )
            .optional()
            .map_err(sqlite_error("read checkpoint"))?;
        let Some((stored_buffer, stored_run, cursor)) = stored else {
            return Ok(None);
        };
        let stored_run = decode_u64_blob(&stored_run, "checkpoint run")?;
        if stored_buffer != buffer_id || stored_run != run_id {
            return Ok(None);
        }
        let cursor_abs = decode_u64_blob(&cursor, "checkpoint cursor")?;
        Ok(Some(super::PersistenceCheckpoint {
            buffer_id,
            run_id,
            cursor_abs,
        }))
    }

    fn commit(&mut self, batch: &PersistenceBatch) -> Result<CommitOutcome, PersistenceError> {
        let transaction = self
            .connection
            .transaction()
            .map_err(sqlite_error("begin document transaction"))?;
        let current_checkpoint = transaction
            .query_row(
                "SELECT buffer_id, run_id, cursor_abs FROM logging_checkpoint WHERE singleton = 1",
                [],
                |row| {
                    Ok((
                        row.get::<_, u32>(0)?,
                        row.get::<_, Vec<u8>>(1)?,
                        row.get::<_, Vec<u8>>(2)?,
                    ))
                },
            )
            .optional()
            .map_err(sqlite_error("read checkpoint"))?;
        if let Some((buffer_id, run_bytes, cursor_bytes)) = current_checkpoint {
            let current_run = decode_u64_blob(&run_bytes, "checkpoint run")?;
            let cursor_abs = decode_u64_blob(&cursor_bytes, "checkpoint cursor")?;
            if buffer_id == batch.checkpoint.buffer_id
                && current_run == batch.checkpoint.run_id
                && batch.checkpoint.cursor_abs < cursor_abs
            {
                return Err(PersistenceError::CheckpointRegression {
                    current: cursor_abs,
                    requested: batch.checkpoint.cursor_abs,
                });
            }
        }

        let mut inserted = 0;
        let mut duplicated = 0;
        let mut projection_rows_committed = 0;
        let mut unclassified_events = 0;
        let mut unresolved_documents = 0;
        let mut loss_ranges = 0;
        let mut lost_records = 0u64;
        for document in &batch.documents {
            let projected = self.projector.project(document)?;
            let projected_row_count = projected.public_row_count();
            let has_unclassified_event = projected.has_unclassified_event();
            let special_counts = projected.loss_and_unresolved_counts()?;
            let row = projected.canonical;
            let existing = transaction
                .query_row(
                    "SELECT canonical_json FROM logging_records WHERE identity_key = ?1",
                    [&row.identity_key],
                    |row| row.get::<_, String>(0),
                )
                .optional()
                .map_err(sqlite_error("check document identity"))?;
            if let Some(existing) = existing {
                if existing == row.canonical_json {
                    duplicated += 1;
                    continue;
                }
                return Err(PersistenceError::IdentityConflict(row.identity_key));
            }
            transaction
                .execute(
                    "INSERT INTO logging_records (\n\
                         identity_key, document_kind, buffer_id, run_id, source_id, epoch_id,\n\
                         seq, first_seq, last_seq, loss_basis, source_time_ns, receive_time_ns,\n\
                         event_type_id, event_name, definition_hash, canonical_json\n\
                     ) VALUES (\n\
                         ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16\n\
                     )",
                    params![
                        row.identity_key,
                        row.document_kind,
                        i64::from(row.buffer_id),
                        row.run_id.as_slice(),
                        i64::from(row.source_id),
                        row.epoch_id.as_slice(),
                        row.seq.as_deref(),
                        row.first_seq.as_deref(),
                        row.last_seq.as_deref(),
                        row.loss_basis,
                        row.source_time_ns.as_deref(),
                        row.receive_time_ns.as_slice(),
                        row.event_type_id.map(i64::from),
                        row.event_name,
                        row.definition_hash,
                        row.canonical_json,
                    ],
                )
                .map_err(sqlite_error("insert document"))?;
            inserted += 1;
            projection_rows_committed += projected_row_count;
            unclassified_events += usize::from(has_unclassified_event);
            unresolved_documents += special_counts.0;
            loss_ranges += special_counts.1;
            lost_records = lost_records.saturating_add(special_counts.2);
            if let Some(event) = projected.event {
                transaction
                    .execute(
                        "INSERT INTO event_log (record_id,event_time,event_time_ns,received_time,received_time_ns,source,source_id,source_path,source_hierarchy,buffer_id,run_id,epoch_id,sequence,definition_hash,time_unsynced,synthetic_record,partial_payload,event_type_id,event_name,has_unclassified_fields) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19,?20)",
                        params![
                            event.record_id,
                            event.event_time,
                            event.event_time_ns,
                            event.received_time,
                            event.received_time_ns,
                            event.source,
                            event.source_id,
                            event.source_path,
                            event.source_hierarchy,
                            event.buffer_id,
                            event.run_id,
                            event.epoch_id,
                            event.sequence,
                            event.definition_hash,
                            event.time_unsynced,
                            event.synthetic_record,
                            event.partial_payload,
                            event.event_type_id,
                            event.event_name,
                            event.has_unclassified_fields,
                        ],
                    )
                    .map_err(sqlite_error("insert event projection"))?;
            }
            for value in projected.logged_values {
                let common = value.common;
                transaction
                    .execute(
                        "INSERT INTO logged_values (record_id,event_time,event_time_ns,received_time,received_time_ns,source,source_id,source_path,source_hierarchy,buffer_id,run_id,epoch_id,sequence,definition_hash,time_unsynced,synthetic_record,partial_payload,value_id,value_name,value_type,unit,quality,semantic_role,boolean_value,signed_value,unsigned_value,number_value,text_value,exact_value,previous_boolean_value,previous_signed_value,previous_unsigned_value,previous_number_value,previous_text_value,previous_exact_value,is_audited,actor,reason,authorization_result) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19,?20,?21,?22,?23,?24,?25,?26,?27,?28,?29,?30,?31,?32,?33,?34,?35,?36,?37,?38,?39)",
                        params![
                            common.record_id, common.event_time, common.event_time_ns,
                            common.received_time, common.received_time_ns, common.source,
                            common.source_id, common.source_path, common.source_hierarchy,
                            common.buffer_id, common.run_id, common.epoch_id, common.sequence,
                            common.definition_hash, common.time_unsynced, common.synthetic_record,
                            common.partial_payload, value.value_id, value.value_name,
                            value.value_type, value.unit, value.quality, value.semantic_role,
                            value.boolean_value, value.signed_value, value.unsigned_value,
                            value.number_value, value.text_value, value.exact_value,
                            value.previous_boolean_value, value.previous_signed_value,
                            value.previous_unsigned_value, value.previous_number_value,
                            value.previous_text_value, value.previous_exact_value,
                            value.is_audited, value.actor, value.reason,
                            value.authorization_result,
                        ],
                    )
                    .map_err(sqlite_error("insert logged value projection"))?;
            }
            super::sqlite_read_model::insert_domains(&transaction, projected.domains)?;
        }
        transaction
            .execute(
                "INSERT INTO logging_checkpoint (singleton, buffer_id, run_id, cursor_abs)\n\
                 VALUES (1, ?1, ?2, ?3)\n\
                 ON CONFLICT(singleton) DO UPDATE SET\n\
                     buffer_id = excluded.buffer_id, run_id = excluded.run_id, cursor_abs = excluded.cursor_abs",
                params![
                    i64::from(batch.checkpoint.buffer_id),
                    batch.checkpoint.run_id.to_be_bytes().as_slice(),
                    batch.checkpoint.cursor_abs.to_be_bytes().as_slice()
                ],
            )
            .map_err(sqlite_error("write checkpoint"))?;
        transaction
            .commit()
            .map_err(sqlite_error("commit document transaction"))?;
        Ok(CommitOutcome {
            inserted,
            duplicated,
            remote_pending: 0,
            projection_rows_committed,
            unclassified_events,
            unresolved_documents,
            loss_ranges,
            lost_records,
            pending_parts: 0,
            checkpoint: batch.checkpoint,
        })
    }
}

fn decode_u64_blob(bytes: &[u8], context: &str) -> Result<u64, PersistenceError> {
    let bytes: [u8; 8] = bytes.try_into().map_err(|_| {
        PersistenceError::Commit(format!("SQLite {context} is not an 8-byte unsigned value"))
    })?;
    Ok(u64::from_be_bytes(bytes))
}

fn initialize_schema(connection: &mut Connection) -> Result<(), PersistenceError> {
    let version: u32 = connection
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .map_err(sqlite_error("read schema version"))?;
    if version == LOGGING_SCHEMA_GENERATION {
        validate_schema_shape(connection)?;
        return Ok(());
    }
    if version != 0 {
        return Err(PersistenceError::Commit(format!(
            "SQLite incompatible pre-release schema generation {version}; back up and recreate the development database"
        )));
    }
    let occupied: Option<String> = connection
        .query_row(
            "SELECT name FROM sqlite_master WHERE type IN ('table','view') AND (substr(name,1,8)='logging_' OR name IN ('event_log','logged_values','alarm_history','message_log','state_history','batch_history','recipe_history','material_additions','operator_activity','audit_log','electronic_signatures','system_events','data_loss','unresolved_records','openot_schema','openot_documents','openot_checkpoint')) LIMIT 1",
            [],
            |row| row.get(0),
        )
        .optional()
        .map_err(sqlite_error("inspect schema ownership"))?;
    if let Some(object) = occupied {
        return Err(PersistenceError::Commit(format!(
            "SQLite incompatible pre-release schema object {object}; back up and recreate the development database"
        )));
    }

    let transaction = connection
        .transaction()
        .map_err(sqlite_error("begin schema initialization"))?;
    transaction
            .execute_batch(
                "CREATE TABLE logging_records (\n\
                 identity_key TEXT PRIMARY KEY NOT NULL,\n\
                 document_kind TEXT NOT NULL CHECK (document_kind IN ('event', 'loss', 'placeholder')),\n\
                 buffer_id INTEGER NOT NULL,\n\
                 run_id BLOB NOT NULL CHECK (length(run_id) = 8),\n\
                 source_id INTEGER NOT NULL,\n\
                 epoch_id BLOB NOT NULL CHECK (length(epoch_id) = 8),\n\
                 seq BLOB CHECK (seq IS NULL OR length(seq) = 8),\n\
                 first_seq BLOB CHECK (first_seq IS NULL OR length(first_seq) = 8),\n\
                 last_seq BLOB CHECK (last_seq IS NULL OR length(last_seq) = 8),\n\
                 loss_basis TEXT CHECK (loss_basis IS NULL OR loss_basis IN ('authoritative', 'inferred')),\n\
                 source_time_ns BLOB CHECK (source_time_ns IS NULL OR length(source_time_ns) = 8),\n\
                 receive_time_ns BLOB NOT NULL CHECK (length(receive_time_ns) = 8),\n\
                 event_type_id INTEGER,\n\
                 event_name TEXT,\n\
                 definition_hash TEXT NOT NULL,\n\
                 canonical_json TEXT NOT NULL\n\
             );\n\
             CREATE INDEX logging_records_source_sequence\n\
                 ON logging_records (buffer_id, run_id, source_id, seq);\n\
             CREATE INDEX logging_records_receive_time\n\
                 ON logging_records (receive_time_ns);\n\
             CREATE INDEX logging_records_event_type\n\
                 ON logging_records (event_type_id);\n\
             CREATE TABLE logging_checkpoint (\n\
                 singleton INTEGER PRIMARY KEY CHECK (singleton = 1),\n\
                 buffer_id INTEGER NOT NULL,\n\
                 run_id BLOB NOT NULL CHECK (length(run_id) = 8),\n\
                 cursor_abs BLOB NOT NULL CHECK (length(cursor_abs) = 8)\n\
             );",
            )
            .map_err(sqlite_error("create generation-1 internal tables"))?;
    transaction
        .execute_batch(
            "CREATE TABLE logging_schema (\n\
                 singleton INTEGER PRIMARY KEY CHECK (singleton = 1),\n\
                 version INTEGER NOT NULL CHECK (version = 1),\n\
                 catalog_fingerprint TEXT NOT NULL\n\
             );\n\
             INSERT INTO logging_schema(singleton,version,catalog_fingerprint) VALUES(1,1,'');\n\
             CREATE TABLE event_log (\n\
                 record_id TEXT PRIMARY KEY NOT NULL REFERENCES logging_records(identity_key),\n\
                 event_time TEXT,\n\
                 event_time_ns TEXT,\n\
                 received_time TEXT NOT NULL,\n\
                 received_time_ns TEXT NOT NULL,\n\
                 source TEXT,\n\
                 source_id INTEGER NOT NULL,\n\
                 source_path TEXT NOT NULL,\n\
                 source_hierarchy TEXT NOT NULL,\n\
                 buffer_id INTEGER NOT NULL,\n\
                 run_id TEXT NOT NULL,\n\
                 epoch_id TEXT NOT NULL,\n\
                 sequence TEXT NOT NULL,\n\
                 definition_hash TEXT NOT NULL,\n\
                 time_unsynced INTEGER NOT NULL CHECK(time_unsynced IN (0,1)),\n\
                 synthetic_record INTEGER NOT NULL CHECK(synthetic_record IN (0,1)),\n\
                 partial_payload INTEGER NOT NULL CHECK(partial_payload IN (0,1)),\n\
                 event_type_id INTEGER NOT NULL,\n\
                 event_name TEXT NOT NULL,\n\
                 has_unclassified_fields INTEGER NOT NULL CHECK(has_unclassified_fields IN (0,1))\n\
             );\n\
             CREATE INDEX event_log_time ON event_log(event_time);\n\
             CREATE INDEX event_log_source_sequence ON event_log(source_id,run_id,sequence);\n\
             CREATE INDEX event_log_type_time ON event_log(event_type_id,event_time);",
        )
        .map_err(sqlite_error("create generation-1 read model"))?;
    transaction
        .execute_batch(
            "CREATE TABLE logged_values (\n\
                 record_id TEXT PRIMARY KEY NOT NULL REFERENCES logging_records(identity_key),\n\
                 event_time TEXT, event_time_ns TEXT, received_time TEXT NOT NULL, received_time_ns TEXT NOT NULL,\n\
                 source TEXT, source_id INTEGER NOT NULL, source_path TEXT NOT NULL, source_hierarchy TEXT NOT NULL,\n\
                 buffer_id INTEGER NOT NULL, run_id TEXT NOT NULL, epoch_id TEXT NOT NULL, sequence TEXT NOT NULL,\n\
                 definition_hash TEXT NOT NULL,\n\
                 time_unsynced INTEGER NOT NULL CHECK(time_unsynced IN (0,1)),\n\
                 synthetic_record INTEGER NOT NULL CHECK(synthetic_record IN (0,1)),\n\
                 partial_payload INTEGER NOT NULL CHECK(partial_payload IN (0,1)),\n\
                 value_id INTEGER NOT NULL, value_name TEXT NOT NULL, value_type TEXT NOT NULL, unit TEXT, quality INTEGER, semantic_role INTEGER NOT NULL,\n\
                 boolean_value INTEGER CHECK(boolean_value IS NULL OR boolean_value IN (0,1)),\n\
                 signed_value INTEGER, unsigned_value TEXT, number_value REAL, text_value TEXT, exact_value TEXT NOT NULL,\n\
                 previous_boolean_value INTEGER CHECK(previous_boolean_value IS NULL OR previous_boolean_value IN (0,1)),\n\
                 previous_signed_value INTEGER, previous_unsigned_value TEXT, previous_number_value REAL, previous_text_value TEXT, previous_exact_value TEXT,\n\
                 is_audited INTEGER NOT NULL CHECK(is_audited IN (0,1)), actor TEXT, reason TEXT, authorization_result TEXT,\n\
                 CHECK((boolean_value IS NOT NULL)+(signed_value IS NOT NULL)+(unsigned_value IS NOT NULL)+(number_value IS NOT NULL)+(text_value IS NOT NULL)=1),\n\
                 CHECK((previous_boolean_value IS NOT NULL)+(previous_signed_value IS NOT NULL)+(previous_unsigned_value IS NOT NULL)+(previous_number_value IS NOT NULL)+(previous_text_value IS NOT NULL)<=1)\n\
             );\n\
             CREATE INDEX logged_values_name_time ON logged_values(value_name,event_time);\n\
             CREATE INDEX logged_values_source_time ON logged_values(source_id,event_time);",
        )
        .map_err(sqlite_error("create generation-1 logged values"))?;
    super::sqlite_read_model::create_domain_schema(&transaction)?;
    transaction
        .execute_batch("PRAGMA user_version = 1;")
        .map_err(sqlite_error("record schema generation 1"))?;
    let catalog_fingerprint = sqlite_catalog_fingerprint(&transaction)?;
    transaction
        .execute(
            "UPDATE logging_schema SET catalog_fingerprint=?1 WHERE singleton=1",
            [&catalog_fingerprint],
        )
        .map_err(sqlite_error("record generation-1 catalog fingerprint"))?;
    transaction
        .commit()
        .map_err(sqlite_error("commit schema initialization"))?;
    validate_schema_shape(connection)
}

fn validate_schema_shape(connection: &Connection) -> Result<(), PersistenceError> {
    let version: u32 = connection
        .query_row(
            "SELECT version FROM logging_schema WHERE singleton=1",
            [],
            |row| row.get(0),
        )
        .map_err(sqlite_error("validate schema generation marker"))?;
    if version != LOGGING_SCHEMA_GENERATION {
        return Err(PersistenceError::Commit(format!(
            "SQLite incompatible pre-release schema generation {}; back up and recreate the development database",
            version
        )));
    }
    const REQUIRED_OBJECTS: &[&str] = &[
        "logging_schema",
        "logging_records",
        "logging_checkpoint",
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
    ];
    for object in REQUIRED_OBJECTS {
        let exists: bool = connection
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type IN ('table','view') AND name=?1)",
                [object],
                |row| row.get(0),
            )
            .map_err(sqlite_error("validate schema object"))?;
        if !exists {
            return Err(PersistenceError::Commit(format!(
                "SQLite incompatible pre-release schema: required object {object} is missing; back up and recreate the development database"
            )));
        }
    }
    let catalog_fingerprint: String = connection
        .query_row(
            "SELECT catalog_fingerprint FROM logging_schema WHERE singleton=1",
            [],
            |row| row.get(0),
        )
        .map_err(sqlite_error("validate schema catalog marker"))?;
    if sqlite_catalog_fingerprint(connection)? != catalog_fingerprint {
        return Err(super::schema_contract::incompatible("SQLite"));
    }
    Ok(())
}

fn sqlite_catalog_fingerprint(connection: &Connection) -> Result<String, PersistenceError> {
    let mut statement = connection
        .prepare(
            "SELECT type || '|' || name || '|' || tbl_name || '|' || COALESCE(sql,'') \
             FROM sqlite_master WHERE name NOT LIKE 'sqlite_%' AND (\
               name LIKE 'logging_%' OR tbl_name LIKE 'logging_%' OR \
               tbl_name IN ('event_log','logged_values','alarm_history','message_log',\
                 'state_history','batch_history','recipe_history','material_additions',\
                 'operator_activity','audit_log','electronic_signatures','system_events',\
                 'data_loss','unresolved_records'))",
        )
        .map_err(sqlite_error("prepare catalog fingerprint"))?;
    let rows = statement
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(sqlite_error("read catalog fingerprint"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(sqlite_error("decode catalog fingerprint"))?;
    Ok(super::schema_contract::fingerprint(rows))
}

fn sqlite_error(context: &'static str) -> impl FnOnce(rusqlite::Error) -> PersistenceError {
    move |error| PersistenceError::Commit(format!("SQLite {context}: {error}"))
}

#[cfg(test)]
mod schema_contract_tests {
    use super::*;

    #[test]
    fn generation_1_database_with_changed_index_fails_closed() {
        static CASE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let root = std::env::temp_dir().join(format!(
            "trust-logging-schema-contract-{}-{}",
            std::process::id(),
            CASE.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&root).expect("create isolated database directory");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o700))
                .expect("secure isolated database directory");
        }
        let path = root.join("trust-logging.sqlite3");
        let sink = SqliteDocumentSink::open(&path).expect("initialize generation-1 database");
        sink.connection
            .execute_batch("DROP INDEX logging_records_receive_time;")
            .expect("damage required generation-1 index");
        let reopened = SqliteDocumentSink::open(&path);
        sink.connection
            .execute_batch(
                "CREATE INDEX logging_records_receive_time ON logging_records(receive_time_ns);",
            )
            .expect("restore required index");
        let error = reopened.expect_err("changed generation-1 index must fail closed");
        assert!(
            error
                .to_string()
                .contains("incompatible pre-release schema"),
            "unexpected compatibility error: {error}"
        );
        drop(sink);
        std::fs::remove_dir_all(root).expect("remove isolated database directory");
    }
}
