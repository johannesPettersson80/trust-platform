use std::path::Path;

use mysql::{params, prelude::Queryable, Conn, Opts, OptsBuilder, SslOpts, TxOpts};

use super::projection::document_row;
use super::{CommitOutcome, DocumentSink, PersistenceBatch, PersistenceError};

/// Shared MySQL-protocol sink used by reviewed MySQL and MariaDB servers.
pub struct MySqlDocumentSink {
    connection: Conn,
    database: String,
}

impl std::fmt::Debug for MySqlDocumentSink {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("MySqlDocumentSink")
            .field("database", &self.database)
            .finish_non_exhaustive()
    }
}

const SCHEMA_VERSION: u32 = 2;

impl MySqlDocumentSink {
    /// Connects with authenticated TLS and applies compatible migrations.
    pub fn open(
        connection_url: &str,
        database: &str,
        ca_cert_path: &Path,
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
        migrate(&mut connection)?;
        Ok(Self {
            connection,
            database: database.to_string(),
        })
    }

    /// Returns the truST-owned schema version.
    pub fn schema_version(&mut self) -> Result<u32, PersistenceError> {
        self.connection
            .query_first::<u32, _>("SELECT version FROM openot_schema WHERE singleton = 1")
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
                 WHERE TABLE_SCHEMA=DATABASE() AND TABLE_NAME='openot_documents' \
                   AND COLUMN_NAME='identity_key'",
            )
            .map_err(|error| mysql_error("read identity collation", error))?
            .ok_or_else(|| PersistenceError::Commit("MySQL identity collation is absent".into()))
    }

    #[cfg(all(test, feature = "openot-real-database-tests"))]
    pub(crate) fn document_count(&mut self) -> Result<u64, PersistenceError> {
        self.connection
            .query_first::<u64, _>("SELECT COUNT(*) FROM openot_documents")
            .map_err(|error| mysql_error("count documents", error))?
            .ok_or_else(|| PersistenceError::Commit("MySQL document count is absent".to_string()))
    }

    #[cfg(feature = "openot-real-database-tests")]
    #[doc(hidden)]
    pub fn canonical_jsons(&mut self) -> Result<Vec<String>, PersistenceError> {
        self.connection
            .query_map(
                "SELECT canonical_json FROM openot_documents ORDER BY identity_key",
                |json: String| json,
            )
            .map_err(|error| mysql_error("read canonical documents", error))
    }

    #[cfg(all(test, feature = "openot-real-database-tests"))]
    pub(crate) fn downgrade_checkpoint_to_v1_for_test(&mut self) -> Result<(), PersistenceError> {
        self.connection
            .query_drop("DELETE FROM openot_checkpoint")
            .and_then(|_| {
                self.connection
                    .query_drop("ALTER TABLE openot_checkpoint DROP COLUMN run_id")
            })
            .and_then(|_| {
                self.connection
                    .query_drop("UPDATE openot_schema SET version=1 WHERE singleton=1")
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
                "UPDATE openot_schema SET version=:version WHERE singleton=1",
                params! { "version" => version },
            )
            .map_err(|error| mysql_error("seed schema version", error))
    }

    #[cfg(all(test, feature = "openot-real-database-tests"))]
    pub(crate) fn storage_bytes(&mut self) -> Result<u64, PersistenceError> {
        let bytes: Option<u64> = self
            .connection
            .query_first(
                "SELECT COALESCE(SUM(DATA_LENGTH + INDEX_LENGTH),0) FROM information_schema.TABLES WHERE TABLE_SCHEMA=DATABASE() AND TABLE_NAME IN ('openot_schema','openot_documents','openot_checkpoint')",
            )
            .map_err(|error| mysql_error("measure database storage", error))?;
        Ok(bytes.unwrap_or(0))
    }

    #[cfg(all(test, feature = "openot-real-database-tests"))]
    pub(crate) fn checkpoint(
        &mut self,
    ) -> Result<Option<(u32, Vec<u8>, Vec<u8>)>, PersistenceError> {
        self.connection
            .query_first::<(u32, Vec<u8>, Vec<u8>), _>(
                "SELECT buffer_id, run_id, cursor_abs FROM openot_checkpoint WHERE singleton = 1",
            )
            .map_err(|error| mysql_error("read checkpoint", error))
    }

    #[cfg(feature = "openot-real-database-tests")]
    #[doc(hidden)]
    pub fn reset_test_state(&mut self) -> Result<(), PersistenceError> {
        self.connection
            .query_drop("DELETE FROM openot_documents")
            .map_err(|error| mysql_error("clear test documents", error))?;
        self.connection
            .query_drop("DELETE FROM openot_checkpoint")
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
                "SELECT buffer_id, run_id, cursor_abs FROM openot_checkpoint WHERE singleton = 1",
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
                "SELECT buffer_id, run_id, cursor_abs FROM openot_checkpoint WHERE singleton = 1 FOR UPDATE",
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

        let mut inserted = 0;
        let mut duplicated = 0;
        for document in &batch.documents {
            let row = document_row(document)?;
            let existing: Option<String> = transaction
                .exec_first(
                    "SELECT canonical_json FROM openot_documents WHERE identity_key = :identity",
                    params! { "identity" => &row.identity_key },
                )
                .map_err(|error| mysql_error("check document identity", error))?;
            if let Some(canonical_json) = existing {
                if canonical_json == row.canonical_json {
                    duplicated += 1;
                    continue;
                }
                return Err(PersistenceError::IdentityConflict(row.identity_key));
            }
            transaction
                .exec_drop(
                    "INSERT INTO openot_documents (
                         identity_key, document_kind, buffer_id, run_id, source_id, epoch_id,
                         seq, first_seq, last_seq, loss_basis, source_time_ns, receive_time_ns,
                         event_type_id, event_name, definition_hash, canonical_json
                     ) VALUES (
                         :identity_key, :document_kind, :buffer_id, :run_id, :source_id, :epoch_id,
                         :seq, :first_seq, :last_seq, :loss_basis, :source_time_ns, :receive_time_ns,
                         :event_type_id, :event_name, :definition_hash, :canonical_json
                     )",
                    params! {
                        "identity_key" => row.identity_key,
                        "document_kind" => row.document_kind,
                        "buffer_id" => row.buffer_id,
                        "run_id" => row.run_id.to_vec(),
                        "source_id" => row.source_id,
                        "epoch_id" => row.epoch_id.to_vec(),
                        "seq" => row.seq,
                        "first_seq" => row.first_seq,
                        "last_seq" => row.last_seq,
                        "loss_basis" => row.loss_basis,
                        "source_time_ns" => row.source_time_ns,
                        "receive_time_ns" => row.receive_time_ns.to_vec(),
                        "event_type_id" => row.event_type_id,
                        "event_name" => row.event_name,
                        "definition_hash" => row.definition_hash,
                        "canonical_json" => row.canonical_json,
                    },
                )
                .map_err(|error| mysql_error("insert document", error))?;
            inserted += 1;
        }
        transaction
            .exec_drop(
                "INSERT INTO openot_checkpoint (singleton, buffer_id, run_id, cursor_abs)
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
            checkpoint: batch.checkpoint,
        })
    }
}

fn migrate(connection: &mut Conn) -> Result<(), PersistenceError> {
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
    Ok(())
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
