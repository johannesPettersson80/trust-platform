use std::{fs, path::Path};

use native_tls::{Certificate, TlsConnector};
use postgres::error::SqlState;
use postgres::Client;
use postgres_native_tls::MakeTlsConnector;

use super::projection::LoggingProjector;
use super::{CommitOutcome, DocumentSink, PersistenceBatch, PersistenceError};

/// PostgreSQL-backed durable OpenOT document sink.
pub struct PostgreSqlDocumentSink {
    pub(super) client: Client,
    pub(super) schema: String,
    timescale: bool,
    projector: LoggingProjector,
}

impl std::fmt::Debug for PostgreSqlDocumentSink {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("PostgreSqlDocumentSink")
            .field("schema", &self.schema)
            .finish_non_exhaustive()
    }
}

const SCHEMA_VERSION: u32 = 3;

impl PostgreSqlDocumentSink {
    /// Connects with authenticated TLS and applies compatible migrations.
    pub fn open(
        connection_url: &str,
        schema: &str,
        ca_cert_path: &Path,
    ) -> Result<Self, PersistenceError> {
        Self::open_with_definitions(connection_url, schema, ca_cert_path, Vec::new())
    }

    #[doc(hidden)]
    pub fn open_with_definitions(
        connection_url: &str,
        schema: &str,
        ca_cert_path: &Path,
        definitions: Vec<open_ot_definition::DefinitionFile>,
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
            .map_err(|error| {
                let message = format!("PostgreSQL connect with required TLS: {error}");
                if postgresql_connect_error_is_transient(&error) {
                    PersistenceError::Connection(message)
                } else {
                    PersistenceError::Commit(message)
                }
            })?;
        migrate(&mut client, schema)?;
        let projector = LoggingProjector::new(definitions)?;
        Ok(Self {
            client,
            schema: schema.to_string(),
            timescale: false,
            projector,
        })
    }

    pub(super) fn open_timescale(
        connection_url: &str,
        schema: &str,
        ca_cert_path: &Path,
        definitions: Vec<open_ot_definition::DefinitionFile>,
    ) -> Result<Self, PersistenceError> {
        let mut sink =
            Self::open_with_definitions(connection_url, schema, ca_cert_path, definitions)?;
        sink.client
            .batch_execute("CREATE EXTENSION IF NOT EXISTS timescaledb")
            .map_err(pg_error("require TimescaleDB extension"))?;
        sink.client
            .batch_execute(&format!(
                "DROP TABLE IF EXISTS \"{schema}\".openot_time_index"
            ))
            .map_err(pg_error("remove legacy detached TimescaleDB time index"))?;
        for table in [
            "event_log",
            "logged_values",
            "alarm_history",
            "message_log",
            "state_history",
        ] {
            sink.client
                .batch_execute(&format!(
                    "ALTER TABLE \"{schema}\".{table} DROP CONSTRAINT IF EXISTS {table}_pkey;
                     DO $constraint$ BEGIN
                       IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conrelid='\"{schema}\".{table}'::regclass AND conname='{table}_received_record_key') THEN
                         ALTER TABLE \"{schema}\".{table} ADD CONSTRAINT {table}_received_record_key UNIQUE(received_time,record_id);
                       END IF;
                     END $constraint$;"
                ))
                .map_err(pg_error("prepare TimescaleDB public hypertable"))?;
            sink.client
                .query_one(
                    "SELECT * FROM create_hypertable($1::text::regclass, by_range('received_time', INTERVAL '1 day'), if_not_exists => TRUE, migrate_data => TRUE)",
                    &[&format!("\"{schema}\".{table}")],
                )
                .map_err(pg_error("create TimescaleDB public hypertable"))?;
        }
        sink.timescale = true;
        Ok(sink)
    }

    /// Returns the truST-owned schema version.
    pub fn schema_version(&mut self) -> Result<u32, PersistenceError> {
        let sql = format!(
            "SELECT version FROM \"{}\".logging_schema WHERE singleton = TRUE",
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
        let sql = format!("SELECT COUNT(*) FROM \"{}\".logging_records", self.schema);
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
            "SELECT canonical_json FROM \"{}\".logging_records ORDER BY identity_key",
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
                "DELETE FROM \"{}\".logging_checkpoint; \
                 ALTER TABLE \"{}\".logging_checkpoint DROP COLUMN run_id; \
                 UPDATE \"{}\".logging_schema SET version=1 WHERE singleton=TRUE;",
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
                    "UPDATE \"{}\".logging_schema SET version=$1 WHERE singleton=TRUE",
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
    ) -> Result<Option<super::contracts::StoredCheckpointRow>, PersistenceError> {
        let sql = format!(
            "SELECT buffer_id, run_id, cursor_abs FROM \"{}\".logging_checkpoint WHERE singleton = TRUE",
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

fn postgresql_connect_error_is_transient(error: &postgres::Error) -> bool {
    let Some(code) = error.code() else {
        return true;
    };
    code.code().starts_with("08")
        || matches!(
            code,
            &SqlState::ADMIN_SHUTDOWN | &SqlState::CRASH_SHUTDOWN | &SqlState::CANNOT_CONNECT_NOW
        )
}

fn migrate(client: &mut Client, schema: &str) -> Result<(), PersistenceError> {
    let mut transaction = client
        .transaction()
        .map_err(pg_error("begin schema migration"))?;
    transaction
        .batch_execute(&format!(
            "CREATE SCHEMA IF NOT EXISTS \"{schema}\";\n\
             DO $migration$ BEGIN
               IF to_regclass('\"{schema}\".logging_schema') IS NULL AND to_regclass('\"{schema}\".openot_schema') IS NOT NULL THEN
                 ALTER TABLE \"{schema}\".openot_schema RENAME TO logging_schema;
               END IF;
               IF to_regclass('\"{schema}\".logging_records') IS NULL AND to_regclass('\"{schema}\".openot_documents') IS NOT NULL THEN
                 ALTER TABLE \"{schema}\".openot_documents RENAME TO logging_records;
               END IF;
               IF to_regclass('\"{schema}\".logging_checkpoint') IS NULL AND to_regclass('\"{schema}\".openot_checkpoint') IS NOT NULL THEN
                 ALTER TABLE \"{schema}\".openot_checkpoint RENAME TO logging_checkpoint;
               END IF;
             END $migration$;
             CREATE TABLE IF NOT EXISTS \"{schema}\".logging_schema (\n\
                 singleton BOOLEAN PRIMARY KEY CHECK (singleton),\n\
                 version INTEGER NOT NULL CHECK (version >= 1)\n\
             );"
        ))
        .map_err(pg_error("create schema metadata"))?;
    let version = transaction
        .query_opt(
            &format!(
                "SELECT version FROM \"{schema}\".logging_schema WHERE singleton = TRUE FOR UPDATE"
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
                    "INSERT INTO \"{schema}\".logging_schema (singleton, version) VALUES (TRUE, $1)"
                ),
                &[&(SCHEMA_VERSION as i32)],
            )
            .map_err(pg_error("write schema version"))?;
    }
    transaction
        .batch_execute(&format!(
            "CREATE TABLE IF NOT EXISTS \"{schema}\".logging_records (\n\
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
             CREATE INDEX IF NOT EXISTS logging_records_source_sequence\n\
                 ON \"{schema}\".logging_records (buffer_id, run_id, source_id, seq);\n\
             CREATE INDEX IF NOT EXISTS logging_records_receive_time\n\
                 ON \"{schema}\".logging_records (receive_time_ns);\n\
             CREATE INDEX IF NOT EXISTS logging_records_event_type\n\
                 ON \"{schema}\".logging_records (event_type_id);\n\
             CREATE TABLE IF NOT EXISTS \"{schema}\".logging_checkpoint (\n\
                 singleton BOOLEAN PRIMARY KEY CHECK (singleton),\n\
                 buffer_id BIGINT NOT NULL CHECK (buffer_id BETWEEN 0 AND 4294967295),\n\
                 run_id BYTEA NOT NULL CHECK (octet_length(run_id) = 8),\n\
                 cursor_abs BYTEA NOT NULL CHECK (octet_length(cursor_abs) = 8)\n\
             );\n\
             ALTER TABLE \"{schema}\".logging_checkpoint\n\
                 ADD COLUMN IF NOT EXISTS run_id BYTEA NOT NULL DEFAULT decode('0000000000000000', 'hex') CHECK (octet_length(run_id) = 8);\n\
             UPDATE \"{schema}\".logging_schema SET version = {SCHEMA_VERSION} WHERE singleton = TRUE;"
        ))
        .map_err(pg_error("apply schema migration"))?;
    transaction
        .batch_execute(&format!(
            "CREATE TABLE IF NOT EXISTS \"{schema}\".event_log (
                 record_id TEXT PRIMARY KEY REFERENCES \"{schema}\".logging_records(identity_key),
                 event_time TIMESTAMPTZ, event_time_ns NUMERIC(20),
                 received_time TIMESTAMPTZ NOT NULL, received_time_ns NUMERIC(20) NOT NULL,
                 source TEXT, source_id BIGINT NOT NULL, source_path TEXT NOT NULL,
                 source_hierarchy TEXT NOT NULL, buffer_id BIGINT NOT NULL,
                 run_id NUMERIC(20) NOT NULL, epoch_id NUMERIC(20) NOT NULL,
                 sequence NUMERIC(20) NOT NULL, definition_hash TEXT NOT NULL,
                 time_unsynced BOOLEAN NOT NULL, synthetic_record BOOLEAN NOT NULL,
                 partial_payload BOOLEAN NOT NULL, event_type_id BIGINT NOT NULL,
                 event_name TEXT NOT NULL, has_unclassified_fields BOOLEAN NOT NULL
             );
             CREATE TABLE IF NOT EXISTS \"{schema}\".logged_values (
                 record_id TEXT PRIMARY KEY REFERENCES \"{schema}\".logging_records(identity_key),
                 event_time TIMESTAMPTZ, event_time_ns NUMERIC(20),
                 received_time TIMESTAMPTZ NOT NULL, received_time_ns NUMERIC(20) NOT NULL,
                 source TEXT, source_id BIGINT NOT NULL, source_path TEXT NOT NULL,
                 source_hierarchy TEXT NOT NULL, buffer_id BIGINT NOT NULL,
                 run_id NUMERIC(20) NOT NULL, epoch_id NUMERIC(20) NOT NULL,
                 sequence NUMERIC(20) NOT NULL, definition_hash TEXT NOT NULL,
                 time_unsynced BOOLEAN NOT NULL, synthetic_record BOOLEAN NOT NULL,
                 partial_payload BOOLEAN NOT NULL, value_id BIGINT NOT NULL,
                 value_name TEXT NOT NULL, value_type TEXT NOT NULL, unit TEXT,
                 quality INTEGER, semantic_role INTEGER NOT NULL, boolean_value BOOLEAN,
                 signed_value BIGINT, unsigned_value NUMERIC(20), number_value DOUBLE PRECISION,
                 text_value TEXT, exact_value TEXT NOT NULL
             );
             CREATE TABLE IF NOT EXISTS \"{schema}\".alarm_history (
                 record_id TEXT PRIMARY KEY REFERENCES \"{schema}\".logging_records(identity_key),
                 event_time TIMESTAMPTZ, event_time_ns NUMERIC(20),
                 received_time TIMESTAMPTZ NOT NULL, received_time_ns NUMERIC(20) NOT NULL,
                 source TEXT, source_id BIGINT NOT NULL, source_path TEXT NOT NULL,
                 source_hierarchy TEXT NOT NULL, buffer_id BIGINT NOT NULL,
                 run_id NUMERIC(20) NOT NULL, epoch_id NUMERIC(20) NOT NULL,
                 sequence NUMERIC(20) NOT NULL, definition_hash TEXT NOT NULL,
                 time_unsynced BOOLEAN NOT NULL, synthetic_record BOOLEAN NOT NULL,
                 partial_payload BOOLEAN NOT NULL, condition TEXT NOT NULL,
                 condition_class TEXT, lifecycle_action TEXT NOT NULL
             );"
        ))
        .map_err(pg_error("create schema v3 public read model"))?;
    super::postgres_read_model::create_domain_schema(&mut transaction, schema)?;
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
            "SELECT buffer_id, run_id, cursor_abs FROM \"{}\".logging_checkpoint WHERE singleton = TRUE",
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
        let upsert_sql = format!(
            "INSERT INTO \"{schema}\".logging_checkpoint (singleton, buffer_id, run_id, cursor_abs)\n\
             VALUES (TRUE, $1, $2, $3)\n\
             ON CONFLICT (singleton) DO UPDATE SET\n\
                 buffer_id = EXCLUDED.buffer_id, run_id = EXCLUDED.run_id, cursor_abs = EXCLUDED.cursor_abs\n\
             WHERE \"{schema}\".logging_checkpoint.buffer_id <> EXCLUDED.buffer_id\n\
                OR \"{schema}\".logging_checkpoint.run_id <> EXCLUDED.run_id\n\
                OR \"{schema}\".logging_checkpoint.cursor_abs <= EXCLUDED.cursor_abs"
        );
        let updated = transaction
            .execute(
                &upsert_sql,
                &[
                    &i64::from(batch.checkpoint.buffer_id),
                    &batch.checkpoint.run_id.to_be_bytes().as_slice(),
                    &batch.checkpoint.cursor_abs.to_be_bytes().as_slice(),
                ],
            )
            .map_err(pg_error("write checkpoint"))?;
        if updated == 0 {
            let cursor: Vec<u8> = transaction
                .query_one(
                    &format!(
                        "SELECT cursor_abs FROM \"{schema}\".logging_checkpoint WHERE singleton = TRUE"
                    ),
                    &[],
                )
                .map_err(pg_error("read regressed checkpoint"))?
                .get(0);
            return Err(PersistenceError::CheckpointRegression {
                current: decode_u64(&cursor, "checkpoint cursor")?,
                requested: batch.checkpoint.cursor_abs,
            });
        }

        let projected = batch
            .documents
            .iter()
            .map(|document| self.projector.project(document))
            .collect::<Result<Vec<_>, _>>()?;
        let identities = projected
            .iter()
            .map(|document| document.canonical.identity_key.clone())
            .collect::<Vec<_>>();
        let existing = transaction
            .query(
                &format!("SELECT identity_key,canonical_json FROM \"{schema}\".logging_records WHERE identity_key=ANY($1)"),
                &[&identities],
            )
            .map_err(pg_error("check document identities"))?
            .into_iter()
            .map(|row| (row.get::<_, String>(0), row.get::<_, String>(1)))
            .collect::<std::collections::HashMap<_, _>>();
        let mut projection_statements = super::postgres_read_model::StatementCache::default();
        let mut duplicated = 0;
        let mut pending = Vec::new();
        for document in projected {
            if let Some(canonical_json) = existing.get(&document.canonical.identity_key) {
                if canonical_json == &document.canonical.canonical_json {
                    duplicated += 1;
                    continue;
                }
                return Err(PersistenceError::IdentityConflict(
                    document.canonical.identity_key,
                ));
            }
            pending.push(document);
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
            let rows = pending
                .iter()
                .map(|document| &document.canonical)
                .collect::<Vec<_>>();
            let identity_keys = rows
                .iter()
                .map(|row| row.identity_key.as_str())
                .collect::<Vec<_>>();
            let document_kinds = rows.iter().map(|row| row.document_kind).collect::<Vec<_>>();
            let buffer_ids = rows
                .iter()
                .map(|row| i64::from(row.buffer_id))
                .collect::<Vec<_>>();
            let run_ids = rows
                .iter()
                .map(|row| row.run_id.to_vec())
                .collect::<Vec<_>>();
            let source_ids = rows
                .iter()
                .map(|row| i64::from(row.source_id))
                .collect::<Vec<_>>();
            let epoch_ids = rows
                .iter()
                .map(|row| row.epoch_id.to_vec())
                .collect::<Vec<_>>();
            let seqs = rows.iter().map(|row| row.seq.clone()).collect::<Vec<_>>();
            let first_seqs = rows
                .iter()
                .map(|row| row.first_seq.clone())
                .collect::<Vec<_>>();
            let last_seqs = rows
                .iter()
                .map(|row| row.last_seq.clone())
                .collect::<Vec<_>>();
            let loss_bases = rows.iter().map(|row| row.loss_basis).collect::<Vec<_>>();
            let source_times = rows
                .iter()
                .map(|row| row.source_time_ns.clone())
                .collect::<Vec<_>>();
            let receive_times = rows
                .iter()
                .map(|row| row.receive_time_ns.to_vec())
                .collect::<Vec<_>>();
            let event_type_ids = rows
                .iter()
                .map(|row| row.event_type_id.map(i64::from))
                .collect::<Vec<_>>();
            let event_names = rows
                .iter()
                .map(|row| row.event_name.as_deref())
                .collect::<Vec<_>>();
            let definition_hashes = rows
                .iter()
                .map(|row| row.definition_hash.as_str())
                .collect::<Vec<_>>();
            let canonical_jsons = rows
                .iter()
                .map(|row| row.canonical_json.as_str())
                .collect::<Vec<_>>();
            transaction.execute(
                &format!("INSERT INTO \"{schema}\".logging_records(identity_key,document_kind,buffer_id,run_id,source_id,epoch_id,seq,first_seq,last_seq,loss_basis,source_time_ns,receive_time_ns,event_type_id,event_name,definition_hash,canonical_json) SELECT * FROM UNNEST($1::text[],$2::text[],$3::bigint[],$4::bytea[],$5::bigint[],$6::bytea[],$7::bytea[],$8::bytea[],$9::bytea[],$10::text[],$11::bytea[],$12::bytea[],$13::bigint[],$14::text[],$15::text[],$16::text[])"),
                &[&identity_keys,&document_kinds,&buffer_ids,&run_ids,&source_ids,&epoch_ids,&seqs,&first_seqs,&last_seqs,&loss_bases,&source_times,&receive_times,&event_type_ids,&event_names,&definition_hashes,&canonical_jsons],
            ).map_err(pg_error("insert document batch"))?;
            let events = pending
                .iter()
                .filter_map(|document| document.event.as_ref())
                .collect::<Vec<_>>();
            super::postgres_read_model::insert_event_batch(&mut transaction, &schema, &events)?;
            super::postgres_read_model::insert_high_volume_domain_batches(
                &mut transaction,
                &schema,
                &pending,
            )?;
        }
        for mut projected in pending {
            projected.domains.retain(|domain| {
                !matches!(
                    domain,
                    super::projection_domains::DomainRow::Alarm(_)
                        | super::projection_domains::DomainRow::System(_)
                        | super::projection_domains::DomainRow::Operator(_)
                        | super::projection_domains::DomainRow::Recipe(_)
                )
            });
            super::postgres_read_model::insert_projection(
                &mut transaction,
                &schema,
                projected.logged_values,
                projected.domains,
                &mut projection_statements,
            )?;
        }
        transaction
            .commit()
            .map_err(pg_error("commit document transaction"))?;
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

fn decode_u64(bytes: &[u8], context: &str) -> Result<u64, PersistenceError> {
    let bytes: [u8; 8] = bytes.try_into().map_err(|_| {
        PersistenceError::Commit(format!(
            "PostgreSQL {context} is not an 8-byte unsigned value"
        ))
    })?;
    Ok(u64::from_be_bytes(bytes))
}
