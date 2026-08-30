use std::{fs, path::Path};

use native_tls::{Certificate, TlsConnector};
use postgres::Client;
use postgres_native_tls::MakeTlsConnector;

use super::projection::document_row;
use super::{CommitOutcome, DocumentSink, PersistenceBatch, PersistenceError};

/// PostgreSQL-backed durable OpenOT document sink.
pub struct PostgreSqlDocumentSink {
    pub(super) client: Client,
    pub(super) schema: String,
    timescale: bool,
}

impl std::fmt::Debug for PostgreSqlDocumentSink {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("PostgreSqlDocumentSink")
            .field("schema", &self.schema)
            .finish_non_exhaustive()
    }
}

const SCHEMA_VERSION: u32 = 2;

impl PostgreSqlDocumentSink {
    /// Connects with authenticated TLS and applies compatible migrations.
    pub fn open(
        connection_url: &str,
        schema: &str,
        ca_cert_path: &Path,
    ) -> Result<Self, PersistenceError> {
        validate_identifier(schema)?;
        let ca_pem = fs::read(ca_cert_path).map_err(|error| {
            PersistenceError::Commit(format!("PostgreSQL read CA certificate: {error}"))
        })?;
        let ca = Certificate::from_pem(&ca_pem).map_err(|error| {
            PersistenceError::Commit(format!("PostgreSQL parse CA certificate: {error}"))
        })?;
        let mut connector = TlsConnector::builder();
        connector.add_root_certificate(ca);
        let connector = connector.build().map_err(|error| {
            PersistenceError::Commit(format!("PostgreSQL build TLS connector: {error}"))
        })?;
        let mut client = Client::connect(connection_url, MakeTlsConnector::new(connector))
            .map_err(pg_error("connect with required TLS"))?;
        migrate(&mut client, schema)?;
        Ok(Self {
            client,
            schema: schema.to_string(),
            timescale: false,
        })
    }

    pub(super) fn open_timescale(
        connection_url: &str,
        schema: &str,
        ca_cert_path: &Path,
    ) -> Result<Self, PersistenceError> {
        let mut sink = Self::open(connection_url, schema, ca_cert_path)?;
        sink.client
            .batch_execute("CREATE EXTENSION IF NOT EXISTS timescaledb")
            .map_err(pg_error("require TimescaleDB extension"))?;
        sink.client
            .batch_execute(&format!(
                "CREATE TABLE IF NOT EXISTS \"{schema}\".openot_time_index (\n\
                     receive_time_ns BIGINT NOT NULL,\n\
                     identity_key TEXT NOT NULL,\n\
                     document_kind TEXT NOT NULL,\n\
                     PRIMARY KEY (receive_time_ns, identity_key)\n\
                 );"
            ))
            .map_err(pg_error("create TimescaleDB time index"))?;
        sink.client
            .query_one(
                "SELECT create_hypertable($1::text::regclass, 'receive_time_ns',\n\
                     chunk_time_interval => 86400000000000::BIGINT, if_not_exists => TRUE)",
                &[&format!("{schema}.openot_time_index")],
            )
            .map_err(pg_error("create TimescaleDB hypertable"))?;
        sink.timescale = true;
        Ok(sink)
    }

    /// Returns the truST-owned schema version.
    pub fn schema_version(&mut self) -> Result<u32, PersistenceError> {
        let sql = format!(
            "SELECT version FROM \"{}\".openot_schema WHERE singleton = TRUE",
            self.schema
        );
        let version: i32 = self
            .client
            .query_one(&sql, &[])
            .map_err(pg_error("read schema version"))?
            .get(0);
        u32::try_from(version).map_err(|_| {
            PersistenceError::Commit("PostgreSQL schema version is negative".to_string())
        })
    }

    #[cfg(all(test, feature = "openot-real-database-tests"))]
    pub(crate) fn document_count(&mut self) -> Result<i64, PersistenceError> {
        let sql = format!("SELECT COUNT(*) FROM \"{}\".openot_documents", self.schema);
        Ok(self
            .client
            .query_one(&sql, &[])
            .map_err(pg_error("count documents"))?
            .get(0))
    }

    #[cfg(feature = "openot-real-database-tests")]
    #[doc(hidden)]
    pub fn canonical_jsons(&mut self) -> Result<Vec<String>, PersistenceError> {
        let sql = format!(
            "SELECT canonical_json FROM \"{}\".openot_documents ORDER BY identity_key",
            self.schema
        );
        self.client
            .query(&sql, &[])
            .map(|rows| rows.into_iter().map(|row| row.get(0)).collect())
            .map_err(pg_error("read canonical documents"))
    }

    #[cfg(all(test, feature = "openot-real-database-tests"))]
    pub(crate) fn downgrade_checkpoint_to_v1_for_test(&mut self) -> Result<(), PersistenceError> {
        self.client
            .batch_execute(&format!(
                "DELETE FROM \"{}\".openot_checkpoint; \
                 ALTER TABLE \"{}\".openot_checkpoint DROP COLUMN run_id; \
                 UPDATE \"{}\".openot_schema SET version=1 WHERE singleton=TRUE;",
                self.schema, self.schema, self.schema
            ))
            .map_err(pg_error("seed schema v1"))
    }

    #[cfg(all(test, feature = "openot-real-database-tests"))]
    pub(crate) fn set_schema_version_for_test(
        &mut self,
        version: u32,
    ) -> Result<(), PersistenceError> {
        self.client
            .execute(
                &format!(
                    "UPDATE \"{}\".openot_schema SET version=$1 WHERE singleton=TRUE",
                    self.schema
                ),
                &[&(version as i32)],
            )
            .map(|_| ())
            .map_err(pg_error("seed schema version"))
    }

    #[cfg(all(test, feature = "openot-real-database-tests"))]
    pub(crate) fn storage_bytes(&mut self) -> Result<u64, PersistenceError> {
        let bytes: i64 = self
            .client
            .query_one(
                "SELECT COALESCE(SUM(pg_total_relation_size(format('%I.%I', schemaname, tablename)::regclass)),0)::bigint FROM pg_tables WHERE schemaname=$1",
                &[&self.schema],
            )
            .map_err(pg_error("measure schema storage"))?
            .get(0);
        u64::try_from(bytes)
            .map_err(|_| PersistenceError::Commit("PostgreSQL storage size is negative".into()))
    }

    #[cfg(all(test, feature = "openot-real-database-tests"))]
    pub(crate) fn checkpoint(
        &mut self,
    ) -> Result<Option<(u32, Vec<u8>, Vec<u8>)>, PersistenceError> {
        let sql = format!(
            "SELECT buffer_id, run_id, cursor_abs FROM \"{}\".openot_checkpoint WHERE singleton = TRUE",
            self.schema
        );
        self.client
            .query_opt(&sql, &[])
            .map(|row| {
                row.map(|row| {
                    let buffer_id: i64 = row.get(0);
                    let run: Vec<u8> = row.get(1);
                    let cursor: Vec<u8> = row.get(2);
                    (buffer_id as u32, run, cursor)
                })
            })
            .map_err(pg_error("read checkpoint"))
    }
}

fn migrate(client: &mut Client, schema: &str) -> Result<(), PersistenceError> {
    let mut transaction = client
        .transaction()
        .map_err(pg_error("begin schema migration"))?;
    transaction
        .batch_execute(&format!(
            "CREATE SCHEMA IF NOT EXISTS \"{schema}\";\n\
             CREATE TABLE IF NOT EXISTS \"{schema}\".openot_schema (\n\
                 singleton BOOLEAN PRIMARY KEY CHECK (singleton),\n\
                 version INTEGER NOT NULL CHECK (version >= 1)\n\
             );"
        ))
        .map_err(pg_error("create schema metadata"))?;
    let version = transaction
        .query_opt(
            &format!(
                "SELECT version FROM \"{schema}\".openot_schema WHERE singleton = TRUE FOR UPDATE"
            ),
            &[],
        )
        .map_err(pg_error("lock schema version"))?
        .map(|row| row.get::<_, i32>(0));
    if let Some(version) = version {
        if version > SCHEMA_VERSION as i32 {
            return Err(PersistenceError::Commit(format!(
                "PostgreSQL OpenOT schema version {version} is newer than supported version {SCHEMA_VERSION}"
            )));
        }
    } else {
        transaction
            .execute(
                &format!(
                    "INSERT INTO \"{schema}\".openot_schema (singleton, version) VALUES (TRUE, $1)"
                ),
                &[&(SCHEMA_VERSION as i32)],
            )
            .map_err(pg_error("write schema version"))?;
    }
    transaction
        .batch_execute(&format!(
            "CREATE TABLE IF NOT EXISTS \"{schema}\".openot_documents (\n\
                 identity_key TEXT PRIMARY KEY,\n\
                 document_kind TEXT NOT NULL CHECK (document_kind IN ('event', 'loss', 'placeholder')),\n\
                 buffer_id BIGINT NOT NULL CHECK (buffer_id BETWEEN 0 AND 4294967295),\n\
                 run_id BYTEA NOT NULL CHECK (octet_length(run_id) = 8),\n\
                 source_id BIGINT NOT NULL CHECK (source_id BETWEEN 0 AND 4294967295),\n\
                 epoch_id BYTEA NOT NULL CHECK (octet_length(epoch_id) = 8),\n\
                 seq BYTEA CHECK (seq IS NULL OR octet_length(seq) = 8),\n\
                 first_seq BYTEA CHECK (first_seq IS NULL OR octet_length(first_seq) = 8),\n\
                 last_seq BYTEA CHECK (last_seq IS NULL OR octet_length(last_seq) = 8),\n\
                 loss_basis TEXT CHECK (loss_basis IS NULL OR loss_basis IN ('authoritative', 'inferred')),\n\
                 source_time_ns BYTEA CHECK (source_time_ns IS NULL OR octet_length(source_time_ns) = 8),\n\
                 receive_time_ns BYTEA NOT NULL CHECK (octet_length(receive_time_ns) = 8),\n\
                 event_type_id BIGINT CHECK (event_type_id BETWEEN 0 AND 4294967295),\n\
                 event_name TEXT,\n\
                 definition_hash TEXT NOT NULL,\n\
                 canonical_json TEXT NOT NULL\n\
             );\n\
             CREATE INDEX IF NOT EXISTS openot_documents_source_sequence\n\
                 ON \"{schema}\".openot_documents (buffer_id, run_id, source_id, seq);\n\
             CREATE INDEX IF NOT EXISTS openot_documents_receive_time\n\
                 ON \"{schema}\".openot_documents (receive_time_ns);\n\
             CREATE INDEX IF NOT EXISTS openot_documents_event_type\n\
                 ON \"{schema}\".openot_documents (event_type_id);\n\
             CREATE TABLE IF NOT EXISTS \"{schema}\".openot_checkpoint (\n\
                 singleton BOOLEAN PRIMARY KEY CHECK (singleton),\n\
                 buffer_id BIGINT NOT NULL CHECK (buffer_id BETWEEN 0 AND 4294967295),\n\
                 run_id BYTEA NOT NULL CHECK (octet_length(run_id) = 8),\n\
                 cursor_abs BYTEA NOT NULL CHECK (octet_length(cursor_abs) = 8)\n\
             );\n\
             ALTER TABLE \"{schema}\".openot_checkpoint\n\
                 ADD COLUMN IF NOT EXISTS run_id BYTEA NOT NULL DEFAULT decode('0000000000000000', 'hex') CHECK (octet_length(run_id) = 8);\n\
             UPDATE \"{schema}\".openot_schema SET version = {SCHEMA_VERSION} WHERE singleton = TRUE;"
        ))
        .map_err(pg_error("apply schema migration"))?;
    transaction
        .commit()
        .map_err(pg_error("commit schema migration"))
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
            "PostgreSQL schema must be a SQL identifier".to_string(),
        ))
    }
}

fn pg_error(context: &'static str) -> impl FnOnce(postgres::Error) -> PersistenceError {
    move |error| PersistenceError::Commit(format!("PostgreSQL {context}: {error}"))
}

impl DocumentSink for PostgreSqlDocumentSink {
    fn load_checkpoint(
        &mut self,
        buffer_id: u32,
        run_id: u64,
    ) -> Result<Option<super::PersistenceCheckpoint>, PersistenceError> {
        let sql = format!(
            "SELECT buffer_id, run_id, cursor_abs FROM \"{}\".openot_checkpoint WHERE singleton = TRUE",
            self.schema
        );
        let stored = self
            .client
            .query_opt(&sql, &[])
            .map_err(pg_error("read checkpoint"))?
            .map(|row| {
                (
                    row.get::<_, i64>(0),
                    row.get::<_, Vec<u8>>(1),
                    row.get::<_, Vec<u8>>(2),
                )
            });
        let Some((stored_buffer, stored_run, cursor)) = stored else {
            return Ok(None);
        };
        let stored_run = decode_u64(&stored_run, "checkpoint run")?;
        if stored_buffer != i64::from(buffer_id) || stored_run != run_id {
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
        let schema = self.schema.clone();
        let mut transaction = self
            .client
            .transaction()
            .map_err(pg_error("begin document transaction"))?;
        let checkpoint_sql = format!(
            "SELECT buffer_id, run_id, cursor_abs FROM \"{schema}\".openot_checkpoint WHERE singleton = TRUE FOR UPDATE"
        );
        if let Some(row) = transaction
            .query_opt(&checkpoint_sql, &[])
            .map_err(pg_error("lock checkpoint"))?
        {
            let buffer_id: i64 = row.get(0);
            let current_run = decode_u64(&row.get::<_, Vec<u8>>(1), "checkpoint run")?;
            let cursor_bytes: Vec<u8> = row.get(2);
            let cursor_abs = decode_u64(&cursor_bytes, "checkpoint cursor")?;
            if buffer_id == i64::from(batch.checkpoint.buffer_id)
                && current_run == batch.checkpoint.run_id
                && batch.checkpoint.cursor_abs < cursor_abs
            {
                return Err(PersistenceError::CheckpointRegression {
                    current: cursor_abs,
                    requested: batch.checkpoint.cursor_abs,
                });
            }
        }

        let identity_sql = format!(
            "SELECT canonical_json FROM \"{schema}\".openot_documents WHERE identity_key = $1"
        );
        let insert_sql = format!(
            "INSERT INTO \"{schema}\".openot_documents (\n\
                 identity_key, document_kind, buffer_id, run_id, source_id, epoch_id,\n\
                 seq, first_seq, last_seq, loss_basis, source_time_ns, receive_time_ns,\n\
                 event_type_id, event_name, definition_hash, canonical_json\n\
             ) VALUES (\n\
                 $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16\n\
             )"
        );
        let mut inserted = 0;
        let mut duplicated = 0;
        for document in &batch.documents {
            let row = document_row(document)?;
            if let Some(existing) = transaction
                .query_opt(&identity_sql, &[&row.identity_key])
                .map_err(pg_error("check document identity"))?
            {
                let canonical_json: String = existing.get(0);
                if canonical_json == row.canonical_json {
                    duplicated += 1;
                    continue;
                }
                return Err(PersistenceError::IdentityConflict(row.identity_key));
            }
            let buffer_id = i64::from(row.buffer_id);
            let source_id = i64::from(row.source_id);
            let event_type_id = row.event_type_id.map(i64::from);
            transaction
                .execute(
                    &insert_sql,
                    &[
                        &row.identity_key,
                        &row.document_kind,
                        &buffer_id,
                        &row.run_id.as_slice(),
                        &source_id,
                        &row.epoch_id.as_slice(),
                        &row.seq,
                        &row.first_seq,
                        &row.last_seq,
                        &row.loss_basis,
                        &row.source_time_ns,
                        &row.receive_time_ns.as_slice(),
                        &event_type_id,
                        &row.event_name,
                        &row.definition_hash,
                        &row.canonical_json,
                    ],
                )
                .map_err(pg_error("insert document"))?;
            if self.timescale {
                let receive_time_ns = decode_u64(&row.receive_time_ns, "receive time")?;
                let receive_time_ns = i64::try_from(receive_time_ns).map_err(|_| {
                    PersistenceError::Commit(
                        "TimescaleDB receive_time_ns exceeds signed BIGINT range".to_string(),
                    )
                })?;
                transaction
                    .execute(
                        &format!(
                            "INSERT INTO \"{schema}\".openot_time_index\n\
                                 (receive_time_ns, identity_key, document_kind)\n\
                             VALUES ($1, $2, $3) ON CONFLICT DO NOTHING"
                        ),
                        &[&receive_time_ns, &row.identity_key, &row.document_kind],
                    )
                    .map_err(pg_error("insert TimescaleDB time index"))?;
            }
            inserted += 1;
        }
        let upsert_sql = format!(
            "INSERT INTO \"{schema}\".openot_checkpoint (singleton, buffer_id, run_id, cursor_abs)\n\
             VALUES (TRUE, $1, $2, $3)\n\
             ON CONFLICT (singleton) DO UPDATE SET\n\
                 buffer_id = EXCLUDED.buffer_id, run_id = EXCLUDED.run_id, cursor_abs = EXCLUDED.cursor_abs"
        );
        transaction
            .execute(
                &upsert_sql,
                &[
                    &i64::from(batch.checkpoint.buffer_id),
                    &batch.checkpoint.run_id.to_be_bytes().as_slice(),
                    &batch.checkpoint.cursor_abs.to_be_bytes().as_slice(),
                ],
            )
            .map_err(pg_error("write checkpoint"))?;
        transaction
            .commit()
            .map_err(pg_error("commit document transaction"))?;
        Ok(CommitOutcome {
            inserted,
            duplicated,
            remote_pending: 0,
            checkpoint: batch.checkpoint,
        })
    }
}

fn decode_u64(bytes: &[u8], context: &str) -> Result<u64, PersistenceError> {
    let bytes: [u8; 8] = bytes.try_into().map_err(|_| {
        PersistenceError::Commit(format!(
            "PostgreSQL {context} is not an 8-byte unsigned value"
        ))
    })?;
    Ok(u64::from_be_bytes(bytes))
}
