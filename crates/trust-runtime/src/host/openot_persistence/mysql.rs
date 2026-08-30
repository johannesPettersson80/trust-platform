use std::path::Path;

use mysql::{params, prelude::Queryable, Conn, Opts, OptsBuilder, SslOpts, TxOpts};

use super::projection::LoggingProjector;
use super::{CommitOutcome, DocumentSink, PersistenceBatch, PersistenceError};

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

const SCHEMA_VERSION: u32 = 3;

impl MySqlDocumentSink {
    /// Connects with authenticated TLS and applies compatible migrations.
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
        let mut connection =
            Conn::new(options).map_err(|error| mysql_error("connect with required TLS", error))?;
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
        migrate(&mut connection, &projector)?;
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
    pub(crate) fn seed_v2_without_projections(&mut self) -> Result<(), PersistenceError> {
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
                .map_err(|error| mysql_error("seed empty v2 projections", error))?;
        }
        self.connection
            .query_drop("UPDATE logging_schema SET version=2 WHERE singleton=1")
            .map_err(|error| mysql_error("seed v2 version", error))?;
        self.connection
            .query_drop(
                "RENAME TABLE logging_schema TO openot_schema,
                              logging_records TO openot_documents,
                              logging_checkpoint TO openot_checkpoint",
            )
            .map_err(|error| mysql_error("seed v2 internal names", error))
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
    pub(crate) fn downgrade_checkpoint_to_v1_for_test(&mut self) -> Result<(), PersistenceError> {
        self.connection
            .query_drop("DELETE FROM logging_checkpoint")
            .and_then(|_| {
                self.connection
                    .query_drop("ALTER TABLE logging_checkpoint DROP COLUMN run_id")
            })
            .and_then(|_| {
                self.connection
                    .query_drop("UPDATE logging_schema SET version=1 WHERE singleton=1")
            })
            .map_err(|error| mysql_error("seed schema v1", error))
    }

    #[cfg(all(test, feature = "openot-real-database-tests"))]
    pub(crate) fn set_schema_version_for_test(
        &mut self,
        version: u32,
    ) -> Result<(), PersistenceError> {
        self.connection
            .exec_drop(
                "UPDATE logging_schema SET version=:version WHERE singleton=1",
                params! { "version" => version },
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
            pending_parts: 0,
            checkpoint: batch.checkpoint,
        })
    }
}

fn migrate(connection: &mut Conn, projector: &LoggingProjector) -> Result<(), PersistenceError> {
    let has_logging_schema: u64 = connection
        .query_first(
            "SELECT COUNT(*) FROM information_schema.TABLES
             WHERE TABLE_SCHEMA=DATABASE() AND TABLE_NAME='logging_schema'",
        )
        .map_err(|error| mysql_error("inspect migrated schema", error))?
        .unwrap_or(0);
    if has_logging_schema == 1 {
        let version: u32 = connection
            .query_first("SELECT version FROM logging_schema WHERE singleton=1")
            .map_err(|error| mysql_error("read migrated schema version", error))?
            .ok_or_else(|| PersistenceError::Commit("MySQL schema version is absent".into()))?;
        if version > SCHEMA_VERSION {
            return Err(PersistenceError::Commit(format!(
                "MySQL OpenOT schema version {version} is newer than supported version {SCHEMA_VERSION}"
            )));
        }
        if version == SCHEMA_VERSION {
            return Ok(());
        }
        connection
            .query_drop(
                "RENAME TABLE logging_schema TO openot_schema,
                              logging_records TO openot_documents,
                              logging_checkpoint TO openot_checkpoint",
            )
            .map_err(|error| mysql_error("prepare legacy logging migration", error))?;
    }
    connection
        .query_drop(
            "CREATE TABLE IF NOT EXISTS openot_schema (
                 singleton TINYINT UNSIGNED PRIMARY KEY,
                 version INT UNSIGNED NOT NULL
             ) ENGINE=InnoDB",
        )
        .map_err(|error| mysql_error("create schema metadata", error))?;
    let version: Option<u32> = connection
        .query_first("SELECT version FROM openot_schema WHERE singleton = 1")
        .map_err(|error| mysql_error("read migration version", error))?;
    if version.is_some_and(|version| version > SCHEMA_VERSION) {
        return Err(PersistenceError::Commit(format!(
            "MySQL OpenOT schema version {} is newer than supported version {SCHEMA_VERSION}",
            version.unwrap_or_default()
        )));
    }
    connection
        .query_drop(
            "CREATE TABLE IF NOT EXISTS openot_documents (
                 identity_key VARCHAR(255) PRIMARY KEY,
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
                 INDEX openot_documents_source_sequence (buffer_id, run_id, source_id, seq),
                 INDEX openot_documents_receive_time (receive_time_ns),
                 INDEX openot_documents_event_type (event_type_id)
             ) ENGINE=InnoDB",
        )
        .map_err(|error| mysql_error("create document table", error))?;
    connection
        .query_drop(
            "CREATE TABLE IF NOT EXISTS openot_checkpoint (
                 singleton TINYINT UNSIGNED PRIMARY KEY,
                 buffer_id INT UNSIGNED NOT NULL,
                 run_id BINARY(8) NOT NULL,
                 cursor_abs BINARY(8) NOT NULL
             ) ENGINE=InnoDB",
        )
        .map_err(|error| mysql_error("create checkpoint table", error))?;
    let identity_collation: Option<String> = connection
        .query_first(
            "SELECT COLLATION_NAME FROM information_schema.COLUMNS \
             WHERE TABLE_SCHEMA=DATABASE() AND TABLE_NAME='openot_documents' \
               AND COLUMN_NAME='identity_key'",
        )
        .map_err(|error| mysql_error("inspect identity collation", error))?;
    if identity_collation.as_deref() != Some("ascii_bin") {
        connection
            .query_drop(
                "ALTER TABLE openot_documents MODIFY identity_key \
                 VARCHAR(255) CHARACTER SET ascii COLLATE ascii_bin NOT NULL",
            )
            .map_err(|error| mysql_error("migrate identity to bytewise collation", error))?;
    }
    let has_checkpoint_run_id: u64 = connection
        .query_first(
            "SELECT COUNT(*) FROM information_schema.COLUMNS \
             WHERE TABLE_SCHEMA = DATABASE() \
               AND TABLE_NAME = 'openot_checkpoint' \
               AND COLUMN_NAME = 'run_id'",
        )
        .map_err(|error| mysql_error("inspect checkpoint migration", error))?
        .unwrap_or(0);
    if has_checkpoint_run_id == 0 {
        connection
            .query_drop(
                "ALTER TABLE openot_checkpoint ADD COLUMN run_id BINARY(8) NOT NULL \
                 DEFAULT 0x0000000000000000 AFTER buffer_id",
            )
            .map_err(|error| mysql_error("migrate checkpoint run identity", error))?;
    }
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
                 FOREIGN KEY(record_id) REFERENCES openot_documents(identity_key)
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
                 FOREIGN KEY(record_id) REFERENCES openot_documents(identity_key)
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
                 FOREIGN KEY(record_id) REFERENCES openot_documents(identity_key)
             ) ENGINE=InnoDB",
        )
        .map_err(|error| mysql_error("create alarm history", error))?;
    super::mysql_read_model::create_domain_schema(connection)?;
    if version.is_some_and(|version| version < SCHEMA_VERSION) {
        backfill_v3(connection, projector)?;
    }
    if version.is_none() {
        connection
            .exec_drop(
                "INSERT INTO openot_schema (singleton, version) VALUES (1, :version)",
                params! { "version" => SCHEMA_VERSION },
            )
            .map_err(|error| mysql_error("write schema version", error))?;
    } else if version != Some(SCHEMA_VERSION) {
        connection
            .exec_drop(
                "UPDATE openot_schema SET version = :version WHERE singleton = 1",
                params! { "version" => SCHEMA_VERSION },
            )
            .map_err(|error| mysql_error("update schema version", error))?;
    }
    connection
        .query_drop(
            "RENAME TABLE openot_schema TO logging_schema,
                          openot_documents TO logging_records,
                          openot_checkpoint TO logging_checkpoint",
        )
        .map_err(|error| mysql_error("rename internal logging tables", error))?;
    Ok(())
}

fn backfill_v3(
    connection: &mut Conn,
    projector: &LoggingProjector,
) -> Result<(), PersistenceError> {
    let canonical = connection
        .query_map(
            "SELECT canonical_json FROM openot_documents ORDER BY identity_key",
            |json: String| json,
        )
        .map_err(|error| mysql_error("read v2 canonical backfill", error))?;
    let mut transaction = connection
        .start_transaction(TxOpts::default())
        .map_err(|error| mysql_error("begin v3 projection backfill", error))?;
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
        transaction
            .query_drop(format!("DELETE FROM `{table}`"))
            .map_err(|error| mysql_error("reset v3 projection backfill", error))?;
    }
    for canonical_json in canonical {
        let document: open_ot_document::Document =
            serde_json::from_str(&canonical_json).map_err(|error| {
                PersistenceError::Commit(format!(
                    "MySQL v2 canonical document is malformed: {error}"
                ))
            })?;
        let projected = projector.project(&document)?;
        super::mysql_read_model::insert_projection(
            &mut transaction,
            projected.event,
            projected.logged_values,
            projected.domains,
        )?;
    }
    transaction
        .commit()
        .map_err(|error| mysql_error("commit v3 projection backfill", error))
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
