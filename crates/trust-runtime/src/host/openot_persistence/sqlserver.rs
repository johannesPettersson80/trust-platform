use std::path::Path;

use tiberius::{Client, Config, EncryptionLevel, Query, Row};
use tokio::net::TcpStream;
use tokio_util::compat::{Compat, TokioAsyncWriteCompatExt};

use super::projection::document_row;
use super::{CommitOutcome, DocumentSink, PersistenceBatch, PersistenceError};

type TdsClient = Client<Compat<TcpStream>>;
const SCHEMA_VERSION: u32 = 2;

/// Dedicated Microsoft SQL Server/Azure SQL TDS persistence adapter.
pub struct SqlServerDocumentSink {
    runtime: tokio::runtime::Runtime,
    client: TdsClient,
    schema: String,
}

impl std::fmt::Debug for SqlServerDocumentSink {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SqlServerDocumentSink")
            .field("schema", &self.schema)
            .finish_non_exhaustive()
    }
}

impl SqlServerDocumentSink {
    /// Connects with CA-verified TLS and applies compatible migrations.
    pub fn open(url: &str, schema: &str, ca: &Path) -> Result<Self, PersistenceError> {
        validate_identifier(schema)?;
        let mut config = Config::from_ado_string(url)
            .map_err(|error| sql_error("parse connection string", error))?;
        config.encryption(EncryptionLevel::Required);
        config.trust_cert_ca(ca.display().to_string());
        let runtime = tokio::runtime::Runtime::new()
            .map_err(|error| PersistenceError::Commit(format!("SQL Server runtime: {error}")))?;
        let client = runtime.block_on(async {
            let tcp = TcpStream::connect(config.get_addr())
                .await
                .map_err(|error| {
                    PersistenceError::Commit(format!("SQL Server TCP connect: {error}"))
                })?;
            tcp.set_nodelay(true).map_err(|error| {
                PersistenceError::Commit(format!("SQL Server TCP options: {error}"))
            })?;
            Client::connect(config, tcp.compat_write())
                .await
                .map_err(|error| sql_error("connect with required TLS", error))
        })?;
        let mut sink = Self {
            runtime,
            client,
            schema: schema.to_string(),
        };
        sink.migrate()?;
        Ok(sink)
    }

    /// Returns the truST-owned schema version.
    pub fn schema_version(&mut self) -> Result<u32, PersistenceError> {
        let row = self.one(&format!(
            "SELECT version FROM [{}].openot_schema WHERE singleton=1",
            self.schema
        ))?;
        let version: i32 = row
            .get(0)
            .ok_or_else(|| commit_error("schema version is absent"))?;
        u32::try_from(version).map_err(|_| commit_error("schema version is negative"))
    }

    #[cfg(all(test, feature = "openot-real-database-tests"))]
    pub(crate) fn product_version(&mut self) -> Result<String, PersistenceError> {
        let row = self.one("SELECT CONVERT(NVARCHAR(128), SERVERPROPERTY('ProductVersion'))")?;
        row.get::<&str, _>(0)
            .map(str::to_string)
            .ok_or_else(|| commit_error("product version is absent"))
    }

    #[cfg(all(test, feature = "openot-real-database-tests"))]
    pub(crate) fn document_count(&mut self) -> Result<i64, PersistenceError> {
        self.one(&format!(
            "SELECT COUNT_BIG(*) FROM [{}].openot_documents",
            self.schema
        ))?
        .get(0)
        .ok_or_else(|| commit_error("document count is absent"))
    }

    #[cfg(feature = "openot-real-database-tests")]
    #[doc(hidden)]
    pub fn canonical_jsons(&mut self) -> Result<Vec<String>, PersistenceError> {
        self.rows(&format!(
            "SELECT canonical_json FROM [{}].openot_documents ORDER BY identity_key",
            self.schema
        ))?
        .into_iter()
        .map(|row| {
            row.get::<&str, _>(0)
                .map(str::to_string)
                .ok_or_else(|| commit_error("canonical JSON is absent"))
        })
        .collect()
    }

    #[cfg(all(test, feature = "openot-real-database-tests"))]
    pub(crate) fn downgrade_checkpoint_to_v1_for_test(&mut self) -> Result<(), PersistenceError> {
        let s = &self.schema;
        self.batch(&format!(
            "DELETE FROM [{s}].openot_checkpoint;
             DECLARE @constraint NVARCHAR(128);
             SELECT @constraint=dc.name FROM sys.default_constraints dc
             JOIN sys.columns c ON c.default_object_id=dc.object_id
             WHERE dc.parent_object_id=OBJECT_ID(N'[{s}].openot_checkpoint') AND c.name=N'run_id';
             IF @constraint IS NOT NULL EXEC(N'ALTER TABLE [{s}].openot_checkpoint DROP CONSTRAINT ['+@constraint+N']');
             ALTER TABLE [{s}].openot_checkpoint DROP COLUMN run_id;
             UPDATE [{s}].openot_schema SET version=1 WHERE singleton=1;"
        ))
    }

    #[cfg(all(test, feature = "openot-real-database-tests"))]
    pub(crate) fn set_schema_version_for_test(
        &mut self,
        version: u32,
    ) -> Result<(), PersistenceError> {
        let s = &self.schema;
        self.batch(&format!(
            "UPDATE [{s}].openot_schema SET version={version} WHERE singleton=1"
        ))
    }

    #[cfg(all(test, feature = "openot-real-database-tests"))]
    pub(crate) fn storage_bytes(&mut self) -> Result<u64, PersistenceError> {
        let s = &self.schema;
        let row = self.one(&format!(
            "SELECT COALESCE(SUM(reserved_page_count),0)*8192 FROM sys.dm_db_partition_stats WHERE object_id IN (OBJECT_ID(N'[{s}].openot_schema'),OBJECT_ID(N'[{s}].openot_documents'),OBJECT_ID(N'[{s}].openot_checkpoint'))"
        ))?;
        let bytes: i64 = row
            .get(0)
            .ok_or_else(|| commit_error("storage size is absent"))?;
        u64::try_from(bytes)
            .map_err(|_| PersistenceError::Commit("SQL Server storage size is negative".into()))
    }

    #[cfg(all(test, feature = "openot-real-database-tests"))]
    pub(crate) fn invalid_json_count(&mut self) -> Result<i64, PersistenceError> {
        self.one(&format!(
            "SELECT COUNT_BIG(*) FROM [{}].openot_documents WHERE ISJSON(canonical_json)<>1",
            self.schema
        ))?
        .get(0)
        .ok_or_else(|| commit_error("invalid JSON count is absent"))
    }

    #[cfg(all(test, feature = "openot-real-database-tests"))]
    pub(crate) fn checkpoint(
        &mut self,
    ) -> Result<Option<super::contracts::StoredCheckpointRow>, PersistenceError> {
        let row = self
            .rows(&format!(
                "SELECT buffer_id,run_id,cursor_abs FROM [{}].openot_checkpoint WHERE singleton=1",
                self.schema
            ))?
            .into_iter()
            .next();
        row.map(|row| {
            let buffer: i64 = row
                .get(0)
                .ok_or_else(|| commit_error("checkpoint buffer is absent"))?;
            let run: &[u8] = row
                .get(1)
                .ok_or_else(|| commit_error("checkpoint run is absent"))?;
            let cursor: &[u8] = row
                .get(2)
                .ok_or_else(|| commit_error("checkpoint cursor is absent"))?;
            Ok((buffer as u32, run.to_vec(), cursor.to_vec()))
        })
        .transpose()
    }

    fn migrate(&mut self) -> Result<(), PersistenceError> {
        let s = &self.schema;
        self.batch(&format!(
            "IF SCHEMA_ID(N'{s}') IS NULL EXEC(N'CREATE SCHEMA [{s}]');
             IF OBJECT_ID(N'[{s}].openot_schema',N'U') IS NULL CREATE TABLE [{s}].openot_schema(singleton BIT PRIMARY KEY,version INT NOT NULL);
             IF EXISTS(SELECT 1 FROM [{s}].openot_schema WHERE singleton=1 AND version>{SCHEMA_VERSION}) THROW 51000,'OpenOT schema is newer than supported',1;
             IF OBJECT_ID(N'[{s}].openot_documents',N'U') IS NULL BEGIN
               CREATE TABLE [{s}].openot_documents(
                 identity_key NVARCHAR(255) COLLATE Latin1_General_100_BIN2 PRIMARY KEY,
                 document_kind NVARCHAR(16) NOT NULL,buffer_id BIGINT NOT NULL,run_id BINARY(8) NOT NULL,
                 source_id BIGINT NOT NULL,epoch_id BINARY(8) NOT NULL,seq BINARY(8) NULL,
                 first_seq BINARY(8) NULL,last_seq BINARY(8) NULL,loss_basis NVARCHAR(16) NULL,
                 source_time_ns BINARY(8) NULL,receive_time_ns BINARY(8) NOT NULL,event_type_id BIGINT NULL,
                 event_name NVARCHAR(MAX) NULL,definition_hash NVARCHAR(MAX) NOT NULL,canonical_json NVARCHAR(MAX) NOT NULL);
               CREATE INDEX openot_source_sequence ON [{s}].openot_documents(buffer_id,run_id,source_id,seq);
               CREATE INDEX openot_receive_time ON [{s}].openot_documents(receive_time_ns);
               CREATE INDEX openot_event_type ON [{s}].openot_documents(event_type_id); END;
             IF OBJECT_ID(N'[{s}].openot_checkpoint',N'U') IS NULL CREATE TABLE [{s}].openot_checkpoint(singleton BIT PRIMARY KEY,buffer_id BIGINT NOT NULL,run_id BINARY(8) NOT NULL,cursor_abs BINARY(8) NOT NULL);
             IF COL_LENGTH(N'[{s}].openot_checkpoint',N'run_id') IS NULL ALTER TABLE [{s}].openot_checkpoint ADD run_id BINARY(8) NOT NULL DEFAULT 0x0000000000000000;
             IF NOT EXISTS(SELECT 1 FROM [{s}].openot_schema WHERE singleton=1) INSERT INTO [{s}].openot_schema VALUES(1,{SCHEMA_VERSION});
             ELSE UPDATE [{s}].openot_schema SET version={SCHEMA_VERSION} WHERE singleton=1;"
        ))
    }

    fn batch(&mut self, sql: &str) -> Result<(), PersistenceError> {
        self.runtime
            .block_on(self.client.simple_query(sql))
            .map_err(|e| sql_error("execute batch", e))?;
        Ok(())
    }

    fn rows(&mut self, sql: &str) -> Result<Vec<Row>, PersistenceError> {
        self.runtime
            .block_on(async {
                self.client
                    .simple_query(sql)
                    .await?
                    .into_first_result()
                    .await
            })
            .map_err(|e| sql_error("query", e))
    }

    fn one(&mut self, sql: &str) -> Result<Row, PersistenceError> {
        self.rows(sql)?
            .into_iter()
            .next()
            .ok_or_else(|| commit_error("query returned no row"))
    }
}

impl DocumentSink for SqlServerDocumentSink {
    fn load_checkpoint(
        &mut self,
        buffer_id: u32,
        run_id: u64,
    ) -> Result<Option<super::PersistenceCheckpoint>, PersistenceError> {
        let row = self
            .rows(&format!(
                "SELECT buffer_id,run_id,cursor_abs FROM [{}].openot_checkpoint WHERE singleton=1",
                self.schema
            ))?
            .into_iter()
            .next();
        row.map(|row| {
            let stored_buffer: i64 = row
                .get(0)
                .ok_or_else(|| commit_error("checkpoint buffer is absent"))?;
            let stored_run: &[u8] = row
                .get(1)
                .ok_or_else(|| commit_error("checkpoint run is absent"))?;
            let cursor: &[u8] = row
                .get(2)
                .ok_or_else(|| commit_error("checkpoint cursor is absent"))?;
            if stored_buffer != i64::from(buffer_id)
                || decode_u64(stored_run, "checkpoint run")? != run_id
            {
                return Ok(None);
            }
            Ok(Some(super::PersistenceCheckpoint {
                buffer_id,
                run_id,
                cursor_abs: decode_u64(cursor, "checkpoint cursor")?,
            }))
        })
        .transpose()
        .map(Option::flatten)
    }

    fn commit(&mut self, batch: &PersistenceBatch) -> Result<CommitOutcome, PersistenceError> {
        self.batch("BEGIN TRANSACTION")?;
        let result = self.commit_inner(batch);
        if result.is_ok() {
            self.batch("COMMIT TRANSACTION")?;
        } else {
            let _ = self.batch("IF @@TRANCOUNT>0 ROLLBACK TRANSACTION");
        }
        result
    }
}

impl SqlServerDocumentSink {
    fn commit_inner(
        &mut self,
        batch: &PersistenceBatch,
    ) -> Result<CommitOutcome, PersistenceError> {
        let s = self.schema.clone();
        if let Some(row) = self.rows(&format!("SELECT buffer_id,run_id,cursor_abs FROM [{s}].openot_checkpoint WITH(UPDLOCK,HOLDLOCK) WHERE singleton=1"))?.first() {
            let buffer: i64 = row.get(0).ok_or_else(|| commit_error("checkpoint buffer is absent"))?;
            let run_bytes: &[u8] = row.get(1).ok_or_else(|| commit_error("checkpoint run is absent"))?;
            let current_run = decode_u64(run_bytes, "checkpoint run")?;
            let cursor: &[u8] = row.get(2).ok_or_else(|| commit_error("checkpoint cursor is absent"))?;
            let current = decode_u64(cursor, "checkpoint cursor")?;
            if buffer == i64::from(batch.checkpoint.buffer_id) && current_run == batch.checkpoint.run_id && batch.checkpoint.cursor_abs < current {
                return Err(PersistenceError::CheckpointRegression { current, requested: batch.checkpoint.cursor_abs });
            }
        }
        let mut inserted = 0;
        let mut duplicated = 0;
        for document in &batch.documents {
            let row = document_row(document)?;
            let mut lookup = Query::new(format!("SELECT canonical_json FROM [{s}].openot_documents WITH(UPDLOCK,HOLDLOCK) WHERE identity_key=@P1"));
            lookup.bind(row.identity_key.as_str());
            let existing = self
                .runtime
                .block_on(async { lookup.query(&mut self.client).await?.into_row().await })
                .map_err(|e| sql_error("check identity", e))?;
            if let Some(existing) = existing {
                let json: &str = existing
                    .get(0)
                    .ok_or_else(|| commit_error("canonical JSON is absent"))?;
                if json == row.canonical_json {
                    duplicated += 1;
                    continue;
                }
                return Err(PersistenceError::IdentityConflict(row.identity_key));
            }
            let mut q = Query::new(format!("INSERT INTO [{s}].openot_documents(identity_key,document_kind,buffer_id,run_id,source_id,epoch_id,seq,first_seq,last_seq,loss_basis,source_time_ns,receive_time_ns,event_type_id,event_name,definition_hash,canonical_json) VALUES(@P1,@P2,@P3,@P4,@P5,@P6,@P7,@P8,@P9,@P10,@P11,@P12,@P13,@P14,@P15,@P16)"));
            q.bind(row.identity_key.as_str());
            q.bind(row.document_kind);
            q.bind(i64::from(row.buffer_id));
            q.bind(row.run_id.as_slice());
            q.bind(i64::from(row.source_id));
            q.bind(row.epoch_id.as_slice());
            q.bind(row.seq.as_deref());
            q.bind(row.first_seq.as_deref());
            q.bind(row.last_seq.as_deref());
            q.bind(row.loss_basis);
            q.bind(row.source_time_ns.as_deref());
            q.bind(row.receive_time_ns.as_slice());
            q.bind(row.event_type_id.map(i64::from));
            q.bind(row.event_name.as_deref());
            q.bind(row.definition_hash.as_str());
            q.bind(row.canonical_json.as_str());
            self.runtime
                .block_on(q.execute(&mut self.client))
                .map_err(|e| sql_error("insert document", e))?;
            inserted += 1;
        }
        let mut q = Query::new(format!("MERGE [{s}].openot_checkpoint WITH(HOLDLOCK) AS t USING(SELECT CAST(1 AS BIT) singleton,@P1 buffer_id,@P2 run_id,@P3 cursor_abs) AS x ON t.singleton=x.singleton WHEN MATCHED THEN UPDATE SET buffer_id=x.buffer_id,run_id=x.run_id,cursor_abs=x.cursor_abs WHEN NOT MATCHED THEN INSERT(singleton,buffer_id,run_id,cursor_abs) VALUES(x.singleton,x.buffer_id,x.run_id,x.cursor_abs);"));
        q.bind(i64::from(batch.checkpoint.buffer_id));
        let run_id = batch.checkpoint.run_id.to_be_bytes();
        q.bind(run_id.as_slice());
        let cursor = batch.checkpoint.cursor_abs.to_be_bytes();
        q.bind(cursor.as_slice());
        self.runtime
            .block_on(q.execute(&mut self.client))
            .map_err(|e| sql_error("write checkpoint", e))?;
        Ok(CommitOutcome {
            inserted,
            duplicated,
            remote_pending: 0,
            checkpoint: batch.checkpoint,
        })
    }
}

fn validate_identifier(value: &str) -> Result<(), PersistenceError> {
    let mut chars = value.chars();
    let valid = chars
        .next()
        .is_some_and(|c| c == '_' || c.is_ascii_alphabetic())
        && chars.all(|c| c == '_' || c.is_ascii_alphanumeric());
    if valid {
        Ok(())
    } else {
        Err(PersistenceError::InvalidConfig(
            "SQL Server schema must be a SQL identifier".to_string(),
        ))
    }
}

fn decode_u64(bytes: &[u8], context: &str) -> Result<u64, PersistenceError> {
    let bytes: [u8; 8] = bytes
        .try_into()
        .map_err(|_| commit_error(&format!("{context} is not an 8-byte unsigned value")))?;
    Ok(u64::from_be_bytes(bytes))
}

fn commit_error(message: &str) -> PersistenceError {
    PersistenceError::Commit(format!("SQL Server {message}"))
}
fn sql_error(context: &str, error: tiberius::error::Error) -> PersistenceError {
    commit_error(&format!("{context}: {error}"))
}
