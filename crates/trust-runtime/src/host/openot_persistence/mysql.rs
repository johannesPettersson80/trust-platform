use std::path::Path;

use mysql::{params, prelude::Queryable, Conn, Opts, OptsBuilder, SslOpts, TxOpts};

use super::contracts::LOGGING_SCHEMA_GENERATION;
use super::projection::LoggingProjector;
use super::{CommitOutcome, DocumentSink, PersistenceBatch, PersistenceError};

const REQUIRED_TABLES: &[&str] = &[
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

/// Shared MySQL-protocol sink used by reviewed MySQL and MariaDB servers.
pub struct MySqlDocumentSink {
    connection: Conn,
    database: String,
    projector: LoggingProjector,
}

impl std::fmt::Debug for MySqlDocumentSink {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("MySqlDocumentSink")
            .field("database", &self.database)
            .finish_non_exhaustive()
    }
}

impl MySqlDocumentSink {
    /// Connects with authenticated TLS and opens the initial schema generation.
    pub fn open(
        connection_url: &str,
        database: &str,
        ca_cert_path: &Path,
    ) -> Result<Self, PersistenceError> {
        Self::open_with_definitions(connection_url, database, ca_cert_path, Vec::new())
    }

    #[doc(hidden)]
    pub fn open_with_definitions(
        connection_url: &str,
        database: &str,
        ca_cert_path: &Path,
        definitions: Vec<open_ot_definition::DefinitionFile>,
    ) -> Result<Self, PersistenceError> {
        validate_identifier(database)?;
        let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();
        let options = Opts::from_url(connection_url).map_err(|error| {
            PersistenceError::InvalidConfig(format!("MySQL parse connection URL: {error}"))
        })?;
        let ssl = SslOpts::default().with_root_cert_path(Some(ca_cert_path.to_path_buf()));
        let options = OptsBuilder::from_opts(options).ssl_opts(Some(ssl));
        let mut connection = Conn::new(options).map_err(|error| {
            let retryable = error.is_connectivity_error();
            let message = format!("MySQL connect with required TLS: {error}");
            if retryable {
                PersistenceError::Connection(message)
            } else {
                PersistenceError::Commit(message)
            }
        })?;
        let ssl_cipher: Option<(String, String)> = connection
            .query_first("SHOW SESSION STATUS LIKE 'Ssl_cipher'")
            .map_err(|error| mysql_error("inspect TLS session", error))?;
        if !ssl_cipher.is_some_and(|(_, cipher)| !cipher.is_empty()) {
            return Err(PersistenceError::Commit(
                "MySQL connection did not negotiate TLS".to_string(),
            ));
        }
        connection
            .query_drop(format!("USE `{database}`"))
            .map_err(|error| mysql_error("select configured database", error))?;
        let projector = LoggingProjector::new(definitions)?;
        initialize_schema(&mut connection)?;
        Ok(Self {
            connection,
            database: database.to_string(),
            projector,
        })
    }

    /// Returns the truST-owned schema version.
    pub fn schema_version(&mut self) -> Result<u32, PersistenceError> {
        self.connection
            .query_first::<u32, _>("SELECT version FROM logging_schema WHERE singleton = 1")
            .map_err(|error| mysql_error("read schema version", error))?
            .ok_or_else(|| PersistenceError::Commit("MySQL schema version is absent".to_string()))
    }

    #[cfg(all(test, feature = "openot-real-database-tests"))]
    pub(crate) fn server_version(&mut self) -> Result<String, PersistenceError> {
        self.connection
            .query_first::<String, _>("SELECT VERSION()")
            .map_err(|error| mysql_error("read server version", error))?
            .ok_or_else(|| PersistenceError::Commit("MySQL server version is absent".to_string()))
    }

    #[cfg(all(test, feature = "openot-real-database-tests"))]
    pub(crate) fn identity_collation(&mut self) -> Result<String, PersistenceError> {
        self.connection
            .query_first::<String, _>(
                "SELECT COLLATION_NAME FROM information_schema.COLUMNS \
                 WHERE TABLE_SCHEMA=DATABASE() AND TABLE_NAME='logging_records' \
                   AND COLUMN_NAME='identity_key'",
            )
            .map_err(|error| mysql_error("read identity collation", error))?
            .ok_or_else(|| PersistenceError::Commit("MySQL identity collation is absent".into()))
    }

    #[cfg(all(test, feature = "openot-real-database-tests"))]
    pub(crate) fn document_count(&mut self) -> Result<u64, PersistenceError> {
        self.connection
            .query_first::<u64, _>("SELECT COUNT(*) FROM logging_records")
            .map_err(|error| mysql_error("count documents", error))?
            .ok_or_else(|| PersistenceError::Commit("MySQL document count is absent".to_string()))
    }

    #[cfg(all(test, feature = "openot-real-database-tests"))]
    pub(crate) fn public_count(&mut self, table: &str) -> Result<u64, PersistenceError> {
        if !matches!(
            table,
            "event_log"
                | "logged_values"
                | "alarm_history"
                | "message_log"
                | "state_history"
                | "batch_history"
                | "recipe_history"
                | "material_additions"
                | "operator_activity"
                | "audit_log"
                | "electronic_signatures"
                | "system_events"
                | "data_loss"
                | "unresolved_records"
        ) {
            return Err(PersistenceError::InvalidConfig(
                "unsupported MySQL test table".into(),
            ));
        }
        self.connection
            .query_first::<u64, _>(format!("SELECT COUNT(*) FROM `{table}`"))
            .map_err(|error| mysql_error("count public rows", error))?
            .ok_or_else(|| PersistenceError::Commit("MySQL public count is absent".into()))
    }

    #[cfg(all(test, feature = "openot-real-database-tests"))]
    pub(crate) fn audited_value_projection(
        &mut self,
    ) -> Result<super::contracts::AuditedValueProjection, PersistenceError> {
        self.connection
            .query_first(
                "SELECT previous_boolean_value,is_audited,actor,reason,authorization_result \
                 FROM logged_values WHERE is_audited=TRUE LIMIT 1",
            )
            .map_err(|error| mysql_error("read audited value projection", error))?
            .ok_or_else(|| PersistenceError::Commit("MySQL audited value row is absent".into()))
    }

    #[cfg(all(test, feature = "openot-real-database-tests"))]
    pub(crate) fn set_required_index_present_for_test(
        &mut self,
        present: bool,
    ) -> Result<(), PersistenceError> {
        let statement = if present {
            "CREATE INDEX logging_records_receive_time ON logging_records(receive_time_ns)"
        } else {
            "DROP INDEX logging_records_receive_time ON logging_records"
        };
        self.connection
            .query_drop(statement)
            .map_err(|error| mysql_error("change required index for compatibility test", error))
    }

    #[cfg(all(test, feature = "openot-real-database-tests"))]
    pub(crate) fn internal_name_counts(&mut self) -> Result<(u64, u64), PersistenceError> {
        self.connection
            .exec_first::<(u64, u64), _, _>(
                "SELECT SUM(TABLE_NAME IN ('logging_schema','logging_records','logging_checkpoint')),
                        SUM(TABLE_NAME LIKE 'openot\\_%')
                 FROM information_schema.TABLES WHERE TABLE_SCHEMA=DATABASE()",
                (),
            )
            .map_err(|error| mysql_error("inspect internal logging names", error))?
            .ok_or_else(|| PersistenceError::Commit("MySQL internal name count absent".into()))
    }

    #[cfg(all(test, feature = "openot-real-database-tests"))]
    pub(crate) fn seed_incompatible_generation_for_test(&mut self) -> Result<(), PersistenceError> {
        self.connection
            .query_drop("DELETE FROM logging_schema WHERE singleton=1")
            .map_err(|error| mysql_error("remove schema generation marker", error))
    }

    #[cfg(feature = "openot-real-database-tests")]
    #[doc(hidden)]
    pub fn canonical_jsons(&mut self) -> Result<Vec<String>, PersistenceError> {
        self.connection
            .query_map(
                "SELECT canonical_json FROM logging_records ORDER BY identity_key",
                |json: String| json,
            )
            .map_err(|error| mysql_error("read canonical documents", error))
    }

    #[cfg(all(test, feature = "openot-real-database-tests"))]
    pub(crate) fn set_schema_version_for_test(
        &mut self,
        version: u32,
    ) -> Result<(), PersistenceError> {
        let catalog_fingerprint = mysql_catalog_fingerprint(&mut self.connection)?;
        self.connection
            .exec_drop(
                "INSERT INTO logging_schema(singleton,version,catalog_fingerprint)
                 VALUES (1,:version,:catalog_fingerprint)
                 ON DUPLICATE KEY UPDATE version=VALUES(version),catalog_fingerprint=VALUES(catalog_fingerprint)",
                params! { "version" => version, "catalog_fingerprint" => catalog_fingerprint },
            )
            .map_err(|error| mysql_error("seed schema version", error))
    }

    #[cfg(all(test, feature = "openot-real-database-tests"))]
    pub(crate) fn storage_bytes(&mut self) -> Result<u64, PersistenceError> {
        let bytes: Option<u64> = self
            .connection
            .query_first(
                "SELECT COALESCE(SUM(DATA_LENGTH + INDEX_LENGTH),0) FROM information_schema.TABLES WHERE TABLE_SCHEMA=DATABASE() AND TABLE_NAME IN ('logging_schema','logging_records','logging_checkpoint')",
            )
            .map_err(|error| mysql_error("measure database storage", error))?;
        Ok(bytes.unwrap_or(0))
    }

    #[cfg(all(test, feature = "openot-real-database-tests"))]
    pub(crate) fn checkpoint(
        &mut self,
    ) -> Result<Option<super::contracts::StoredCheckpointRow>, PersistenceError> {
        self.connection
            .query_first::<(u32, Vec<u8>, Vec<u8>), _>(
                "SELECT buffer_id, run_id, cursor_abs FROM logging_checkpoint WHERE singleton = 1",
            )
            .map_err(|error| mysql_error("read checkpoint", error))
    }

    #[cfg(feature = "openot-real-database-tests")]
    #[doc(hidden)]
    pub fn reset_test_state(&mut self) -> Result<(), PersistenceError> {
        for table in [
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
            "alarm_history",
            "logged_values",
            "event_log",
        ] {
            self.connection
                .query_drop(format!("DELETE FROM `{table}`"))
                .map_err(|error| mysql_error("clear test public rows", error))?;
        }
        self.connection
            .query_drop("DELETE FROM logging_records")
            .map_err(|error| mysql_error("clear test documents", error))?;
        self.connection
            .query_drop("DELETE FROM logging_checkpoint")
            .map_err(|error| mysql_error("clear test checkpoint", error))
    }
}

impl DocumentSink for MySqlDocumentSink {
    fn load_checkpoint(
        &mut self,
        buffer_id: u32,
        run_id: u64,
    ) -> Result<Option<super::PersistenceCheckpoint>, PersistenceError> {
        let stored = self
            .connection
            .query_first::<(u32, Vec<u8>, Vec<u8>), _>(
                "SELECT buffer_id, run_id, cursor_abs FROM logging_checkpoint WHERE singleton = 1",
            )
            .map_err(|error| mysql_error("read checkpoint", error))?;
        let Some((stored_buffer, stored_run, cursor)) = stored else {
            return Ok(None);
        };
        let stored_run = decode_u64(&stored_run, "checkpoint run")?;
        if stored_buffer != buffer_id || stored_run != run_id {
            return Ok(None);
        }
        let cursor_abs = decode_u64(&cursor, "checkpoint cursor")?;
        Ok(Some(super::PersistenceCheckpoint {
            buffer_id,
            run_id,
            cursor_abs,
        }))
    }

    fn commit(&mut self, batch: &PersistenceBatch) -> Result<CommitOutcome, PersistenceError> {
        let mut transaction = self
            .connection
            .start_transaction(TxOpts::default())
            .map_err(|error| mysql_error("begin document transaction", error))?;
        if let Some((buffer_id, run_bytes, cursor_bytes)) = transaction
            .query_first::<(u32, Vec<u8>, Vec<u8>), _>(
                "SELECT buffer_id, run_id, cursor_abs FROM logging_checkpoint WHERE singleton = 1 FOR UPDATE",
            )
            .map_err(|error| mysql_error("lock checkpoint", error))?
        {
            let current_run = decode_u64(&run_bytes, "checkpoint run")?;
            let cursor_abs = decode_u64(&cursor_bytes, "checkpoint cursor")?;
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

        let projected = batch
            .documents
            .iter()
            .map(|document| self.projector.project(document))
            .collect::<Result<Vec<_>, _>>()?;
        let identities = projected
            .iter()
            .map(|document| document.canonical.identity_key.as_str())
            .collect::<Vec<_>>();
        let existing = if identities.is_empty() {
            std::collections::HashMap::new()
        } else {
            let placeholders = std::iter::repeat_n("?", identities.len())
                .collect::<Vec<_>>()
                .join(",");
            transaction
                .exec::<(String, String), _, _>(
                    format!(
                        "SELECT identity_key,canonical_json FROM logging_records WHERE identity_key IN ({placeholders})"
                    ),
                    mysql::Params::Positional(
                        identities
                            .iter()
                            .map(|identity| mysql::Value::from(*identity))
                            .collect(),
                    ),
                )
                .map_err(|error| mysql_error("check document identities", error))?
                .into_iter()
                .collect::<std::collections::HashMap<_, _>>()
        };
        let mut duplicated = 0;
        let mut pending = Vec::new();
        for projected in projected {
            let row = &projected.canonical;
            if let Some(canonical_json) = existing.get(&row.identity_key) {
                if canonical_json == &row.canonical_json {
                    duplicated += 1;
                    continue;
                }
                return Err(PersistenceError::IdentityConflict(row.identity_key.clone()));
            }
            pending.push(projected);
        }
        let inserted = pending.len();
        let projection_rows_committed = pending
            .iter()
            .map(super::projection::ProjectedDocument::public_row_count)
            .sum();
        let unclassified_events = pending
            .iter()
            .filter(|document| document.has_unclassified_event())
            .count();
        let (unresolved_documents, loss_ranges, lost_records) =
            super::projection::committed_special_counts(&pending)?;
        if !pending.is_empty() {
            let placeholders =
                std::iter::repeat_n("(?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?)", pending.len())
                    .collect::<Vec<_>>()
                    .join(",");
            let mut values = Vec::with_capacity(pending.len() * 16);
            for projected in &pending {
                let row = &projected.canonical;
                values.extend([
                    mysql::Value::from(row.identity_key.as_str()),
                    mysql::Value::from(row.document_kind),
                    mysql::Value::from(row.buffer_id),
                    mysql::Value::from(row.run_id.to_vec()),
                    mysql::Value::from(row.source_id),
                    mysql::Value::from(row.epoch_id.to_vec()),
                    mysql::Value::from(row.seq.clone()),
                    mysql::Value::from(row.first_seq.clone()),
                    mysql::Value::from(row.last_seq.clone()),
                    mysql::Value::from(row.loss_basis.map(str::to_string)),
                    mysql::Value::from(row.source_time_ns.clone()),
                    mysql::Value::from(row.receive_time_ns.to_vec()),
                    mysql::Value::from(row.event_type_id),
                    mysql::Value::from(row.event_name.clone()),
                    mysql::Value::from(row.definition_hash.as_str()),
                    mysql::Value::from(row.canonical_json.as_str()),
                ]);
            }
            transaction
                .exec_drop(
                    format!(
                        "INSERT INTO logging_records (identity_key,document_kind,buffer_id,run_id,source_id,epoch_id,seq,first_seq,last_seq,loss_basis,source_time_ns,receive_time_ns,event_type_id,event_name,definition_hash,canonical_json) VALUES {placeholders}"
                    ),
                    mysql::Params::Positional(values),
                )
                .map_err(|error| mysql_error("insert document batch", error))?;
            let events = pending
                .iter()
                .filter_map(|document| document.event.as_ref())
                .collect::<Vec<_>>();
            super::mysql_read_model::insert_event_batch(&mut transaction, &events)?;
        }
        for mut projected in pending {
            projected.event = None;
            super::mysql_read_model::insert_projection(
                &mut transaction,
                projected.event,
                projected.logged_values,
                projected.domains,
            )?;
        }
        transaction
            .exec_drop(
                "INSERT INTO logging_checkpoint (singleton, buffer_id, run_id, cursor_abs)
                 VALUES (1, :buffer_id, :run_id, :cursor_abs)
                 ON DUPLICATE KEY UPDATE buffer_id = VALUES(buffer_id), run_id = VALUES(run_id), cursor_abs = VALUES(cursor_abs)",
                params! {
                    "buffer_id" => batch.checkpoint.buffer_id,
                    "run_id" => batch.checkpoint.run_id.to_be_bytes().to_vec(),
                    "cursor_abs" => batch.checkpoint.cursor_abs.to_be_bytes().to_vec(),
                },
            )
            .map_err(|error| mysql_error("write checkpoint", error))?;
        transaction
            .commit()
            .map_err(|error| mysql_error("commit document transaction", error))?;
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

fn initialize_schema(connection: &mut Conn) -> Result<(), PersistenceError> {
    let owned_object: Option<String> = connection
        .query_first(
            "SELECT TABLE_NAME FROM information_schema.TABLES WHERE TABLE_SCHEMA=DATABASE() AND (LEFT(TABLE_NAME,8)='logging_' OR TABLE_NAME IN ('event_log','logged_values','alarm_history','message_log','state_history','batch_history','recipe_history','material_additions','operator_activity','audit_log','electronic_signatures','system_events','data_loss','unresolved_records','openot_schema','openot_documents','openot_checkpoint')) ORDER BY TABLE_NAME LIMIT 1",
        )
        .map_err(|error| mysql_error("inspect schema ownership", error))?;
    if owned_object.is_some() {
        let marker: Option<(u32, String)> = connection
            .query_first("SELECT version,catalog_fingerprint FROM logging_schema WHERE singleton=1")
            .map_err(|_| PersistenceError::Commit(
                "MySQL incompatible pre-release schema; back up and recreate the development database".into(),
            ))?;
        if marker.as_ref().map(|value| value.0) != Some(LOGGING_SCHEMA_GENERATION) {
            return Err(PersistenceError::Commit(format!(
                "MySQL incompatible pre-release schema generation {:?}; back up and recreate the development database",
                marker.as_ref().map(|value| value.0)
            )));
        }
        validate_schema_shape(connection)?;
        if mysql_catalog_fingerprint(connection)? != marker.expect("validated marker").1 {
            return Err(super::schema_contract::incompatible("MySQL"));
        }
        return Ok(());
    }
    connection
        .query_drop(
            "CREATE TABLE logging_schema (
                 singleton TINYINT UNSIGNED PRIMARY KEY,
                 version INT UNSIGNED NOT NULL CHECK(version=1),
                 catalog_fingerprint CHAR(64) CHARACTER SET ascii COLLATE ascii_bin NOT NULL
             ) ENGINE=InnoDB",
        )
        .map_err(|error| mysql_error("create schema metadata", error))?;
    connection
        .query_drop(
            "CREATE TABLE logging_records (
                 identity_key VARCHAR(255) CHARACTER SET ascii COLLATE ascii_bin PRIMARY KEY,
                 document_kind VARCHAR(16) NOT NULL,
                 buffer_id INT UNSIGNED NOT NULL,
                 run_id BINARY(8) NOT NULL,
                 source_id INT UNSIGNED NOT NULL,
                 epoch_id BINARY(8) NOT NULL,
                 seq BINARY(8) NULL,
                 first_seq BINARY(8) NULL,
                 last_seq BINARY(8) NULL,
                 loss_basis VARCHAR(16) NULL,
                 source_time_ns BINARY(8) NULL,
                 receive_time_ns BINARY(8) NOT NULL,
                 event_type_id INT UNSIGNED NULL,
                 event_name TEXT NULL,
                 definition_hash TEXT NOT NULL,
                 canonical_json LONGTEXT NOT NULL,
                 INDEX logging_records_source_sequence (buffer_id, run_id, source_id, seq),
                 INDEX logging_records_receive_time (receive_time_ns),
                 INDEX logging_records_event_type (event_type_id)
             ) ENGINE=InnoDB",
        )
        .map_err(|error| mysql_error("create document table", error))?;
    connection
        .query_drop(
            "CREATE TABLE logging_checkpoint (
                 singleton TINYINT UNSIGNED PRIMARY KEY,
                 buffer_id INT UNSIGNED NOT NULL,
                 run_id BINARY(8) NOT NULL,
                 cursor_abs BINARY(8) NOT NULL
             ) ENGINE=InnoDB",
        )
        .map_err(|error| mysql_error("create checkpoint table", error))?;
    connection
        .query_drop(
            "CREATE TABLE IF NOT EXISTS event_log (
                 record_id VARCHAR(255) CHARACTER SET ascii COLLATE ascii_bin PRIMARY KEY,
                 event_time DATETIME(6) NULL,event_time_ns DECIMAL(20,0) NULL,
                 received_time DATETIME(6) NOT NULL,received_time_ns DECIMAL(20,0) NOT NULL,
                 source TEXT NULL,source_id INT UNSIGNED NOT NULL,source_path TEXT NOT NULL,
                 source_hierarchy TEXT NOT NULL,buffer_id INT UNSIGNED NOT NULL,
                 run_id DECIMAL(20,0) NOT NULL,epoch_id DECIMAL(20,0) NOT NULL,
                 sequence DECIMAL(20,0) NOT NULL,definition_hash TEXT NOT NULL,
                 time_unsynced BOOLEAN NOT NULL,synthetic_record BOOLEAN NOT NULL,
                 partial_payload BOOLEAN NOT NULL,event_type_id INT UNSIGNED NOT NULL,
                 event_name TEXT NOT NULL,has_unclassified_fields BOOLEAN NOT NULL,
                 FOREIGN KEY(record_id) REFERENCES logging_records(identity_key)
             ) ENGINE=InnoDB",
        )
        .map_err(|error| mysql_error("create event log", error))?;
    connection
        .query_drop(
            "CREATE TABLE IF NOT EXISTS logged_values (
                 record_id VARCHAR(255) CHARACTER SET ascii COLLATE ascii_bin PRIMARY KEY,
                 event_time DATETIME(6) NULL,event_time_ns DECIMAL(20,0) NULL,
                 received_time DATETIME(6) NOT NULL,received_time_ns DECIMAL(20,0) NOT NULL,
                 source TEXT NULL,source_id INT UNSIGNED NOT NULL,source_path TEXT NOT NULL,
                 source_hierarchy TEXT NOT NULL,buffer_id INT UNSIGNED NOT NULL,
                 run_id DECIMAL(20,0) NOT NULL,epoch_id DECIMAL(20,0) NOT NULL,
                 sequence DECIMAL(20,0) NOT NULL,definition_hash TEXT NOT NULL,
                 time_unsynced BOOLEAN NOT NULL,synthetic_record BOOLEAN NOT NULL,
                 partial_payload BOOLEAN NOT NULL,value_id INT UNSIGNED NOT NULL,
                 value_name TEXT NOT NULL,value_type VARCHAR(32) NOT NULL,unit TEXT NULL,
                 quality INT UNSIGNED NULL,semantic_role INT UNSIGNED NOT NULL,
                 boolean_value BOOLEAN NULL,signed_value BIGINT NULL,unsigned_value DECIMAL(20,0) NULL,
                 number_value DOUBLE NULL,text_value TEXT NULL,exact_value TEXT NOT NULL,
                 previous_boolean_value BOOLEAN NULL,previous_signed_value BIGINT NULL,
                 previous_unsigned_value DECIMAL(20,0) NULL,previous_number_value DOUBLE NULL,
                 previous_text_value TEXT NULL,previous_exact_value TEXT NULL,
                 is_audited BOOLEAN NOT NULL,actor TEXT NULL,reason TEXT NULL,
                 authorization_result TEXT NULL,
                 FOREIGN KEY(record_id) REFERENCES logging_records(identity_key)
             ) ENGINE=InnoDB",
        )
        .map_err(|error| mysql_error("create logged values", error))?;
    connection
        .query_drop(
            "CREATE TABLE IF NOT EXISTS alarm_history (
                 record_id VARCHAR(255) CHARACTER SET ascii COLLATE ascii_bin PRIMARY KEY,
                 event_time DATETIME(6) NULL,event_time_ns DECIMAL(20,0) NULL,
                 received_time DATETIME(6) NOT NULL,received_time_ns DECIMAL(20,0) NOT NULL,
                 source TEXT NULL,source_id INT UNSIGNED NOT NULL,source_path TEXT NOT NULL,
                 source_hierarchy TEXT NOT NULL,buffer_id INT UNSIGNED NOT NULL,
                 run_id DECIMAL(20,0) NOT NULL,epoch_id DECIMAL(20,0) NOT NULL,
                 sequence DECIMAL(20,0) NOT NULL,definition_hash TEXT NOT NULL,
                 time_unsynced BOOLEAN NOT NULL,synthetic_record BOOLEAN NOT NULL,
                 partial_payload BOOLEAN NOT NULL,`condition` TEXT NOT NULL,
                 condition_class TEXT NULL,lifecycle_action TEXT NOT NULL,
                 FOREIGN KEY(record_id) REFERENCES logging_records(identity_key)
             ) ENGINE=InnoDB",
        )
        .map_err(|error| mysql_error("create alarm history", error))?;
    super::mysql_read_model::create_domain_schema(connection)?;
    validate_schema_shape(connection)?;
    let catalog_fingerprint = mysql_catalog_fingerprint(connection)?;
    connection
        .exec_drop(
            "INSERT INTO logging_schema(singleton,version,catalog_fingerprint)
             VALUES(1,:version,:catalog_fingerprint)",
            params! { "version" => LOGGING_SCHEMA_GENERATION, "catalog_fingerprint" => catalog_fingerprint },
        )
        .map_err(|error| mysql_error("record schema generation", error))?;
    Ok(())
}

fn validate_schema_shape(connection: &mut Conn) -> Result<(), PersistenceError> {
    let found: Vec<String> = connection
        .query_map(
            "SELECT TABLE_NAME FROM information_schema.TABLES WHERE TABLE_SCHEMA=DATABASE()",
            |name: String| name,
        )
        .map_err(|error| mysql_error("validate schema objects", error))?;
    for required in REQUIRED_TABLES {
        if !found.iter().any(|candidate| candidate == required) {
            return Err(PersistenceError::Commit(format!(
                "MySQL incompatible pre-release schema: required object {required} is missing; back up and recreate the development database"
            )));
        }
    }
    Ok(())
}

fn mysql_catalog_fingerprint(connection: &mut Conn) -> Result<String, PersistenceError> {
    let table_filter = REQUIRED_TABLES
        .iter()
        .map(|table| format!("'{table}'"))
        .collect::<Vec<_>>()
        .join(",");
    let queries = [
        format!("SELECT CONCAT_WS('|','table',TABLE_NAME,TABLE_TYPE,COALESCE(ENGINE,''),COALESCE(TABLE_COLLATION,'')) FROM information_schema.TABLES WHERE TABLE_SCHEMA=DATABASE() AND TABLE_NAME IN ({table_filter})"),
        format!("SELECT CONCAT_WS('|','column',TABLE_NAME,ORDINAL_POSITION,COLUMN_NAME,COLUMN_TYPE,IS_NULLABLE,COALESCE(COLUMN_DEFAULT,'<NULL>'),EXTRA,COALESCE(COLLATION_NAME,'')) FROM information_schema.COLUMNS WHERE TABLE_SCHEMA=DATABASE() AND TABLE_NAME IN ({table_filter})"),
        format!("SELECT CONCAT_WS('|','index',TABLE_NAME,INDEX_NAME,NON_UNIQUE,SEQ_IN_INDEX,COLUMN_NAME,COALESCE(COLLATION,''),COALESCE(SUB_PART,''),NULLABLE,INDEX_TYPE) FROM information_schema.STATISTICS WHERE TABLE_SCHEMA=DATABASE() AND TABLE_NAME IN ({table_filter})"),
        format!("SELECT CONCAT_WS('|','constraint',tc.TABLE_NAME,tc.CONSTRAINT_NAME,tc.CONSTRAINT_TYPE,COALESCE(kcu.ORDINAL_POSITION,''),COALESCE(kcu.COLUMN_NAME,''),COALESCE(kcu.REFERENCED_TABLE_NAME,''),COALESCE(kcu.REFERENCED_COLUMN_NAME,'')) FROM information_schema.TABLE_CONSTRAINTS tc LEFT JOIN information_schema.KEY_COLUMN_USAGE kcu ON kcu.CONSTRAINT_SCHEMA=tc.CONSTRAINT_SCHEMA AND kcu.TABLE_NAME=tc.TABLE_NAME AND kcu.CONSTRAINT_NAME=tc.CONSTRAINT_NAME WHERE tc.CONSTRAINT_SCHEMA=DATABASE() AND tc.TABLE_NAME IN ({table_filter})"),
        format!("SELECT CONCAT_WS('|','check',tc.TABLE_NAME,tc.CONSTRAINT_NAME,cc.CHECK_CLAUSE) FROM information_schema.TABLE_CONSTRAINTS tc JOIN information_schema.CHECK_CONSTRAINTS cc ON cc.CONSTRAINT_SCHEMA=tc.CONSTRAINT_SCHEMA AND cc.CONSTRAINT_NAME=tc.CONSTRAINT_NAME WHERE tc.CONSTRAINT_SCHEMA=DATABASE() AND tc.TABLE_NAME IN ({table_filter}) AND tc.CONSTRAINT_TYPE='CHECK'"),
    ];
    let mut rows = Vec::new();
    for query in queries {
        rows.extend(
            connection
                .query_map(query, |row: String| row)
                .map_err(|error| mysql_error("read generation-1 catalog fingerprint", error))?,
        );
    }
    Ok(super::schema_contract::fingerprint(rows))
}

fn validate_identifier(value: &str) -> Result<(), PersistenceError> {
    let mut chars = value.chars();
    let valid = chars
        .next()
        .is_some_and(|first| first == '_' || first.is_ascii_alphabetic())
        && chars.all(|character| character == '_' || character.is_ascii_alphanumeric());
    if valid {
        Ok(())
    } else {
        Err(PersistenceError::InvalidConfig(
            "MySQL database must be a SQL identifier".to_string(),
        ))
    }
}

fn decode_u64(bytes: &[u8], context: &str) -> Result<u64, PersistenceError> {
    let bytes: [u8; 8] = bytes.try_into().map_err(|_| {
        PersistenceError::Commit(format!("MySQL {context} is not an 8-byte unsigned value"))
    })?;
    Ok(u64::from_be_bytes(bytes))
}

fn mysql_error(context: &'static str, error: mysql::Error) -> PersistenceError {
    PersistenceError::Commit(format!("MySQL {context}: {error}"))
}
