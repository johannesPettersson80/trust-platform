use std::path::Path;

use rusqlite::{params, Connection, OptionalExtension};

use super::projection::document_row;
use super::{CommitOutcome, DocumentSink, PersistenceBatch, PersistenceError};

/// SQLite-backed durable OpenOT document sink.
#[derive(Debug)]
pub struct SqliteDocumentSink {
    connection: Connection,
}

const SCHEMA_VERSION: u32 = 2;

impl SqliteDocumentSink {
    /// Opens the database and applies compatible truST-owned migrations.
    pub fn open(path: &Path) -> Result<Self, PersistenceError> {
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
        migrate(&mut connection)?;
        validate_existing_documents(&connection)?;
        Ok(Self { connection })
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
            "SELECT identity_key FROM openot_documents WHERE json_valid(canonical_json)=0 LIMIT 1",
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
                "SELECT buffer_id, run_id, cursor_abs FROM openot_checkpoint WHERE singleton = 1",
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
                "SELECT buffer_id, run_id, cursor_abs FROM openot_checkpoint WHERE singleton = 1",
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
        for document in &batch.documents {
            let row = document_row(document)?;
            let existing = transaction
                .query_row(
                    "SELECT canonical_json FROM openot_documents WHERE identity_key = ?1",
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
                    "INSERT INTO openot_documents (\n\
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
        }
        transaction
            .execute(
                "INSERT INTO openot_checkpoint (singleton, buffer_id, run_id, cursor_abs)\n\
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

fn migrate(connection: &mut Connection) -> Result<(), PersistenceError> {
    let version: u32 = connection
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .map_err(sqlite_error("read schema version"))?;
    if version > SCHEMA_VERSION {
        return Err(PersistenceError::Commit(format!(
            "SQLite OpenOT schema version {version} is newer than supported version {SCHEMA_VERSION}"
        )));
    }
    if version == SCHEMA_VERSION {
        return Ok(());
    }

    let transaction = connection
        .transaction()
        .map_err(sqlite_error("begin schema migration"))?;
    if version == 1 {
        transaction
            .execute_batch(
                "ALTER TABLE openot_checkpoint ADD COLUMN run_id BLOB NOT NULL DEFAULT X'0000000000000000' CHECK (length(run_id) = 8);\n\
                 PRAGMA user_version = 2;",
            )
            .map_err(sqlite_error("apply schema migration 1 to 2"))?;
        return transaction
            .commit()
            .map_err(sqlite_error("commit schema migration 1 to 2"));
    }
    transaction
        .execute_batch(
            "CREATE TABLE openot_documents (\n\
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
             CREATE INDEX openot_documents_source_sequence\n\
                 ON openot_documents (buffer_id, run_id, source_id, seq);\n\
             CREATE INDEX openot_documents_receive_time\n\
                 ON openot_documents (receive_time_ns);\n\
             CREATE INDEX openot_documents_event_type\n\
                 ON openot_documents (event_type_id);\n\
             CREATE TABLE openot_checkpoint (\n\
                 singleton INTEGER PRIMARY KEY CHECK (singleton = 1),\n\
                 buffer_id INTEGER NOT NULL,\n\
                 run_id BLOB NOT NULL CHECK (length(run_id) = 8),\n\
                 cursor_abs BLOB NOT NULL CHECK (length(cursor_abs) = 8)\n\
             );\n\
             PRAGMA user_version = 2;",
        )
        .map_err(sqlite_error("apply schema migration"))?;
    transaction
        .commit()
        .map_err(sqlite_error("commit schema migration"))
}

fn sqlite_error(context: &'static str) -> impl FnOnce(rusqlite::Error) -> PersistenceError {
    move |error| PersistenceError::Commit(format!("SQLite {context}: {error}"))
}
