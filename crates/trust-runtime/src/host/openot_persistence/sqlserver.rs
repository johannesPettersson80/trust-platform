use std::path::Path;

use tiberius::{Client, Config, EncryptionLevel, Query, Row};
use tokio::net::TcpStream;
use tokio_util::compat::{Compat, TokioAsyncWriteCompatExt};

use super::contracts::LOGGING_SCHEMA_GENERATION;
use super::projection::LoggingProjector;
use super::{CommitOutcome, DocumentSink, PersistenceBatch, PersistenceError};

type TdsClient = Client<Compat<TcpStream>>;

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

/// Dedicated Microsoft SQL Server/Azure SQL TDS persistence adapter.
pub struct SqlServerDocumentSink {
    runtime: tokio::runtime::Runtime,
    client: TdsClient,
    schema: String,
    projector: LoggingProjector,
}

impl std::fmt::Debug for SqlServerDocumentSink {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SqlServerDocumentSink")
            .field("schema", &self.schema)
            .finish_non_exhaustive()
    }
}

impl SqlServerDocumentSink {
    /// Connects with CA-verified TLS and opens the initial schema generation.
    pub fn open(url: &str, schema: &str, ca: &Path) -> Result<Self, PersistenceError> {
        Self::open_with_definitions(url, schema, ca, Vec::new())
    }

    #[doc(hidden)]
    pub fn open_with_definitions(
        url: &str,
        schema: &str,
        ca: &Path,
        definitions: Vec<open_ot_definition::DefinitionFile>,
    ) -> Result<Self, PersistenceError> {
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
                    PersistenceError::Connection(format!("SQL Server TCP connect: {error}"))
                })?;
            tcp.set_nodelay(true).map_err(|error| {
                PersistenceError::Commit(format!("SQL Server TCP options: {error}"))
            })?;
            Client::connect(config, tcp.compat_write())
                .await
                .map_err(sqlserver_connect_error)
        })?;
        let projector = LoggingProjector::new(definitions)?;
        let mut sink = Self {
            runtime,
            client,
            schema: schema.to_string(),
            projector,
        };
        sink.batch("SET NOCOUNT ON")?;
        sink.initialize_schema()?;
        Ok(sink)
    }

    /// Returns the truST-owned schema version.
    pub fn schema_version(&mut self) -> Result<u32, PersistenceError> {
        let row = self.one(&format!(
            "SELECT version FROM [{}].logging_schema WHERE singleton=1",
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
            "SELECT COUNT_BIG(*) FROM [{}].logging_records",
            self.schema
        ))?
        .get(0)
        .ok_or_else(|| commit_error("document count is absent"))
    }

    #[cfg(all(test, feature = "openot-real-database-tests"))]
    pub(crate) fn internal_name_counts(&mut self) -> Result<(i64, i64), PersistenceError> {
        let row = self.one(&format!(
            "SELECT
               SUM(CONVERT(BIGINT,CASE WHEN t.name IN ('logging_schema','logging_records','logging_checkpoint') THEN 1 ELSE 0 END)),
               SUM(CONVERT(BIGINT,CASE WHEN t.name IN ('openot_schema','openot_documents','openot_checkpoint') THEN 1 ELSE 0 END))
             FROM sys.tables t JOIN sys.schemas s ON s.schema_id=t.schema_id
             WHERE s.name=N'{}'",
            self.schema
        ))?;
        Ok((
            row.get(0)
                .ok_or_else(|| commit_error("logging name count is absent"))?,
            row.get(1)
                .ok_or_else(|| commit_error("legacy name count is absent"))?,
        ))
    }

    #[cfg(all(test, feature = "openot-real-database-tests"))]
    pub(crate) fn public_count(&mut self, table: &str) -> Result<i64, PersistenceError> {
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
                "unsupported SQL Server test table".into(),
            ));
        }
        self.one(&format!(
            "SELECT COUNT_BIG(*) FROM [{}].[{table}]",
            self.schema
        ))?
        .get(0)
        .ok_or_else(|| commit_error("public count is absent"))
    }

    #[cfg(all(test, feature = "openot-real-database-tests"))]
    pub(crate) fn public_provenance(
        &mut self,
        table: &str,
    ) -> Result<(String, String, bool, bool, bool), PersistenceError> {
        if !matches!(table, "data_loss" | "unresolved_records") {
            return Err(PersistenceError::InvalidConfig(
                "unsupported SQL Server provenance table".into(),
            ));
        }
        let row = self.one(&format!(
            "SELECT TOP 1 source_path,source_hierarchy,time_unsynced,synthetic_record,partial_payload FROM [{}].[{table}] ORDER BY record_id",
            self.schema
        ))?;
        Ok((
            row.get::<&str, _>(0)
                .map(str::to_string)
                .ok_or_else(|| commit_error("source path is absent"))?,
            row.get::<&str, _>(1)
                .map(str::to_string)
                .ok_or_else(|| commit_error("source hierarchy is absent"))?,
            row.get(2)
                .ok_or_else(|| commit_error("time-unsynced flag is absent"))?,
            row.get(3)
                .ok_or_else(|| commit_error("synthetic-record flag is absent"))?,
            row.get(4)
                .ok_or_else(|| commit_error("partial-payload flag is absent"))?,
        ))
    }

    #[cfg(all(test, feature = "openot-real-database-tests"))]
    pub(crate) fn audited_value_projection(
        &mut self,
    ) -> Result<
        (
            Option<bool>,
            bool,
            Option<String>,
            Option<String>,
            Option<String>,
        ),
        PersistenceError,
    > {
        let row = self.one(&format!(
            "SELECT TOP 1 previous_boolean_value,is_audited,actor,reason,authorization_result \
             FROM [{}].logged_values WHERE is_audited=1",
            self.schema
        ))?;
        Ok((
            row.get(0),
            row.get(1)
                .ok_or_else(|| commit_error("audited flag is absent"))?,
            row.get::<&str, _>(2).map(str::to_string),
            row.get::<&str, _>(3).map(str::to_string),
            row.get::<&str, _>(4).map(str::to_string),
        ))
    }

    #[cfg(all(test, feature = "openot-real-database-tests"))]
    pub(crate) fn set_required_index_present_for_test(
        &mut self,
        present: bool,
    ) -> Result<(), PersistenceError> {
        let statement = if present {
            format!(
                "CREATE INDEX logging_receive_time ON [{}].logging_records(receive_time_ns)",
                self.schema
            )
        } else {
            format!(
                "DROP INDEX logging_receive_time ON [{}].logging_records",
                self.schema
            )
        };
        self.batch(&statement)
    }

    #[cfg(feature = "openot-real-database-tests")]
    #[doc(hidden)]
    pub fn canonical_jsons(&mut self) -> Result<Vec<String>, PersistenceError> {
        self.rows(&format!(
            "SELECT canonical_json FROM [{}].logging_records ORDER BY identity_key",
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
    pub(crate) fn set_schema_version_for_test(
        &mut self,
        version: u32,
    ) -> Result<(), PersistenceError> {
        let s = self.schema.clone();
        let catalog_fingerprint = self.sqlserver_catalog_fingerprint()?;
        self.batch(&format!(
            "IF EXISTS (SELECT 1 FROM [{s}].logging_schema WHERE singleton=1)
                 UPDATE [{s}].logging_schema SET version={version},catalog_fingerprint=N'{catalog_fingerprint}' WHERE singleton=1;
             ELSE INSERT INTO [{s}].logging_schema(singleton,version,catalog_fingerprint) VALUES (1,{version},N'{catalog_fingerprint}');"
        ))
    }

    #[cfg(all(test, feature = "openot-real-database-tests"))]
    pub(crate) fn set_unrelated_table_present_for_test(
        &mut self,
        present: bool,
    ) -> Result<(), PersistenceError> {
        let schema = self.schema.clone();
        if present {
            self.batch(&format!(
                "CREATE TABLE [{schema}].operator_owned_notes(id BIGINT PRIMARY KEY)"
            ))
        } else {
            self.batch(&format!("DROP TABLE [{schema}].operator_owned_notes"))
        }
    }

    #[cfg(all(test, feature = "openot-real-database-tests"))]
    pub(crate) fn remove_schema_marker_for_test(&mut self) -> Result<(), PersistenceError> {
        let s = self.schema.clone();
        self.batch(&format!(
            "DELETE FROM [{s}].logging_schema WHERE singleton=1"
        ))
    }

    #[cfg(all(test, feature = "openot-real-database-tests"))]
    pub(crate) fn seed_incompatible_generation_for_test(&mut self) -> Result<(), PersistenceError> {
        let s = self.schema.clone();
        self.batch(&format!(
            "DELETE FROM [{s}].message_log; DELETE FROM [{s}].state_history;
             DELETE FROM [{s}].batch_history; DELETE FROM [{s}].recipe_history;
             DELETE FROM [{s}].material_additions; DELETE FROM [{s}].operator_activity;
             DELETE FROM [{s}].audit_log; DELETE FROM [{s}].electronic_signatures;
             DELETE FROM [{s}].system_events; DELETE FROM [{s}].data_loss;
             DELETE FROM [{s}].unresolved_records; DELETE FROM [{s}].alarm_history;
             DELETE FROM [{s}].logged_values; DELETE FROM [{s}].event_log;
             DELETE FROM [{s}].logging_schema WHERE singleton=1;"
        ))
    }

    #[cfg(all(test, feature = "openot-real-database-tests"))]
    pub(crate) fn storage_bytes(&mut self) -> Result<u64, PersistenceError> {
        let s = self.schema.clone();
        let row = self.one(&format!(
            "SELECT COALESCE(SUM(reserved_page_count),0)*8192 FROM sys.dm_db_partition_stats WHERE object_id IN (OBJECT_ID(N'[{s}].logging_schema'),OBJECT_ID(N'[{s}].logging_records'),OBJECT_ID(N'[{s}].logging_checkpoint'))"
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
            "SELECT COUNT_BIG(*) FROM [{}].logging_records WHERE ISJSON(canonical_json)<>1",
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
                "SELECT buffer_id,run_id,cursor_abs FROM [{}].logging_checkpoint WHERE singleton=1",
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

    fn initialize_schema(&mut self) -> Result<(), PersistenceError> {
        let s = self.schema.clone();
        let owned = self.one(&format!(
            "SELECT COUNT_BIG(*) FROM sys.tables t JOIN sys.schemas s ON s.schema_id=t.schema_id WHERE s.name=N'{s}' AND (t.name LIKE N'logging[_]%' OR t.name IN (N'event_log',N'logged_values',N'alarm_history',N'message_log',N'state_history',N'batch_history',N'recipe_history',N'material_additions',N'operator_activity',N'audit_log',N'electronic_signatures',N'system_events',N'data_loss',N'unresolved_records',N'openot_schema',N'openot_documents',N'openot_checkpoint'))"
        ))?.get::<i64,_>(0).unwrap_or(0);
        if owned > 0 {
            let version = self.schema_version().map_err(|_| PersistenceError::Commit(
                "SQL Server incompatible pre-release schema; back up and recreate the development database".into(),
            ))?;
            if version != LOGGING_SCHEMA_GENERATION {
                return Err(PersistenceError::Commit(format!(
                    "SQL Server incompatible pre-release schema generation {version}; back up and recreate the development database"
                )));
            }
            self.validate_schema_shape()?;
            let expected = self
                .one(&format!(
                    "SELECT catalog_fingerprint FROM [{s}].logging_schema WHERE singleton=1"
                ))?
                .get::<&str, _>(0)
                .map(str::to_string)
                .ok_or_else(|| commit_error("catalog fingerprint is absent"))?;
            if self.sqlserver_catalog_fingerprint()? != expected {
                return Err(super::schema_contract::incompatible("SQL Server"));
            }
            return Ok(());
        }
        self.batch("BEGIN TRANSACTION")?;
        let initialization = (|| -> Result<(), PersistenceError> {
            self.batch(&format!(
            "IF SCHEMA_ID(N'{s}') IS NULL EXEC(N'CREATE SCHEMA [{s}]');
             IF OBJECT_ID(N'[{s}].logging_schema',N'U') IS NULL CREATE TABLE [{s}].logging_schema(
               singleton BIT PRIMARY KEY CHECK(singleton=1),version INT NOT NULL CHECK(version=1),
               catalog_fingerprint CHAR(64) COLLATE Latin1_General_100_BIN2 NOT NULL);
             IF OBJECT_ID(N'[{s}].logging_records',N'U') IS NULL BEGIN
               CREATE TABLE [{s}].logging_records(
                 identity_key NVARCHAR(255) COLLATE Latin1_General_100_BIN2 PRIMARY KEY,
                 document_kind NVARCHAR(16) NOT NULL,buffer_id BIGINT NOT NULL,run_id BINARY(8) NOT NULL,
                 source_id BIGINT NOT NULL,epoch_id BINARY(8) NOT NULL,seq BINARY(8) NULL,
                 first_seq BINARY(8) NULL,last_seq BINARY(8) NULL,loss_basis NVARCHAR(16) NULL,
                 source_time_ns BINARY(8) NULL,receive_time_ns BINARY(8) NOT NULL,event_type_id BIGINT NULL,
                 event_name NVARCHAR(MAX) NULL,definition_hash NVARCHAR(MAX) NOT NULL,canonical_json NVARCHAR(MAX) NOT NULL);
               CREATE INDEX logging_source_sequence ON [{s}].logging_records(buffer_id,run_id,source_id,seq);
               CREATE INDEX logging_receive_time ON [{s}].logging_records(receive_time_ns);
               CREATE INDEX logging_event_type ON [{s}].logging_records(event_type_id); END;
             IF OBJECT_ID(N'[{s}].logging_checkpoint',N'U') IS NULL CREATE TABLE [{s}].logging_checkpoint(singleton BIT PRIMARY KEY,buffer_id BIGINT NOT NULL,run_id BINARY(8) NOT NULL,cursor_abs BINARY(8) NOT NULL);
             IF OBJECT_ID(N'[{s}].event_log',N'U') IS NULL CREATE TABLE [{s}].event_log(
               record_id NVARCHAR(255) COLLATE Latin1_General_100_BIN2 PRIMARY KEY REFERENCES [{s}].logging_records(identity_key),
               event_time DATETIME2(7) NULL,event_time_ns DECIMAL(20,0) NULL,
               received_time DATETIME2(7) NOT NULL,received_time_ns DECIMAL(20,0) NOT NULL,
               source NVARCHAR(MAX) NULL,source_id BIGINT NOT NULL,source_path NVARCHAR(MAX) NOT NULL,
               source_hierarchy NVARCHAR(MAX) NOT NULL,buffer_id BIGINT NOT NULL,run_id DECIMAL(20,0) NOT NULL,
               epoch_id DECIMAL(20,0) NOT NULL,sequence DECIMAL(20,0) NOT NULL,definition_hash NVARCHAR(MAX) NOT NULL,
               time_unsynced BIT NOT NULL,synthetic_record BIT NOT NULL,partial_payload BIT NOT NULL,
               event_type_id BIGINT NOT NULL,event_name NVARCHAR(MAX) NOT NULL,has_unclassified_fields BIT NOT NULL);
             IF OBJECT_ID(N'[{s}].logged_values',N'U') IS NULL CREATE TABLE [{s}].logged_values(
               record_id NVARCHAR(255) COLLATE Latin1_General_100_BIN2 PRIMARY KEY REFERENCES [{s}].logging_records(identity_key),
               event_time DATETIME2(7) NULL,event_time_ns DECIMAL(20,0) NULL,received_time DATETIME2(7) NOT NULL,
               received_time_ns DECIMAL(20,0) NOT NULL,source NVARCHAR(MAX) NULL,source_id BIGINT NOT NULL,
               source_path NVARCHAR(MAX) NOT NULL,source_hierarchy NVARCHAR(MAX) NOT NULL,buffer_id BIGINT NOT NULL,
               run_id DECIMAL(20,0) NOT NULL,epoch_id DECIMAL(20,0) NOT NULL,sequence DECIMAL(20,0) NOT NULL,
               definition_hash NVARCHAR(MAX) NOT NULL,time_unsynced BIT NOT NULL,synthetic_record BIT NOT NULL,
               partial_payload BIT NOT NULL,value_id BIGINT NOT NULL,value_name NVARCHAR(MAX) NOT NULL,
               value_type NVARCHAR(32) NOT NULL,unit NVARCHAR(MAX) NULL,quality INT NULL,semantic_role INT NOT NULL,
               boolean_value BIT NULL,signed_value BIGINT NULL,unsigned_value DECIMAL(20,0) NULL,
               number_value FLOAT NULL,text_value NVARCHAR(MAX) NULL,exact_value NVARCHAR(MAX) NOT NULL,
               previous_boolean_value BIT NULL,previous_signed_value BIGINT NULL,
               previous_unsigned_value DECIMAL(20,0) NULL,previous_number_value FLOAT NULL,
               previous_text_value NVARCHAR(MAX) NULL,previous_exact_value NVARCHAR(MAX) NULL,
               is_audited BIT NOT NULL,actor NVARCHAR(MAX) NULL,reason NVARCHAR(MAX) NULL,
               authorization_result NVARCHAR(MAX) NULL);
             IF OBJECT_ID(N'[{s}].alarm_history',N'U') IS NULL CREATE TABLE [{s}].alarm_history(
               record_id NVARCHAR(255) COLLATE Latin1_General_100_BIN2 PRIMARY KEY REFERENCES [{s}].logging_records(identity_key),
               event_time DATETIME2(7) NULL,event_time_ns DECIMAL(20,0) NULL,received_time DATETIME2(7) NOT NULL,
               received_time_ns DECIMAL(20,0) NOT NULL,source NVARCHAR(MAX) NULL,source_id BIGINT NOT NULL,
               source_path NVARCHAR(MAX) NOT NULL,source_hierarchy NVARCHAR(MAX) NOT NULL,buffer_id BIGINT NOT NULL,
               run_id DECIMAL(20,0) NOT NULL,epoch_id DECIMAL(20,0) NOT NULL,sequence DECIMAL(20,0) NOT NULL,
               definition_hash NVARCHAR(MAX) NOT NULL,time_unsynced BIT NOT NULL,synthetic_record BIT NOT NULL,
               partial_payload BIT NOT NULL,condition NVARCHAR(MAX) NOT NULL,condition_class NVARCHAR(MAX) NULL,
               lifecycle_action NVARCHAR(MAX) NOT NULL);"
            ))?;
            super::sqlserver_read_model::create_domain_schema(&self.runtime, &mut self.client, &s)?;
            self.validate_schema_shape()?;
            let catalog_fingerprint = self.sqlserver_catalog_fingerprint()?;
            self.batch(&format!(
                "INSERT INTO [{s}].logging_schema(singleton,version,catalog_fingerprint) VALUES(1,{LOGGING_SCHEMA_GENERATION},N'{catalog_fingerprint}')"
            ))?;
            if self.schema_version()? != LOGGING_SCHEMA_GENERATION {
                return Err(commit_error("schema generation marker was not recorded"));
            }
            Ok(())
        })();
        if let Err(error) = initialization {
            if let Err(rollback_error) = self.batch("IF @@TRANCOUNT > 0 ROLLBACK TRANSACTION") {
                return Err(PersistenceError::Commit(format!(
                    "SQL Server schema initialization failed: {error}; rollback also failed: {rollback_error}"
                )));
            }
            return Err(error);
        }
        if let Err(error) = self.batch("COMMIT TRANSACTION") {
            let _ = self.batch("IF @@TRANCOUNT > 0 ROLLBACK TRANSACTION");
            return Err(error);
        }
        Ok(())
    }

    fn validate_schema_shape(&mut self) -> Result<(), PersistenceError> {
        let s = self.schema.clone();
        for table in REQUIRED_TABLES {
            let exists = self
                .one(&format!(
                    "SELECT COUNT_BIG(*) FROM sys.tables t JOIN sys.schemas s ON s.schema_id=t.schema_id WHERE s.name=N'{s}' AND t.name=N'{table}'"
                ))?
                .get::<i64, _>(0)
                .unwrap_or(0);
            if exists != 1 {
                return Err(PersistenceError::Commit(format!(
                    "SQL Server incompatible pre-release schema: required object {table} is missing; back up and recreate the development database"
                )));
            }
        }
        Ok(())
    }

    fn sqlserver_catalog_fingerprint(&mut self) -> Result<String, PersistenceError> {
        let s = self.schema.clone();
        let table_filter = REQUIRED_TABLES
            .iter()
            .map(|table| format!("N'{table}'"))
            .collect::<Vec<_>>()
            .join(",");
        let queries = [
            format!("SELECT (SELECT object_name,type_desc FROM (SELECT t.name AS object_name,N'USER_TABLE' AS type_desc FROM sys.tables t JOIN sys.schemas schema_row ON schema_row.schema_id=t.schema_id WHERE schema_row.name=N'{s}' AND t.name IN ({table_filter}) UNION ALL SELECT v.name AS object_name,N'VIEW' AS type_desc FROM sys.views v JOIN sys.schemas schema_row ON schema_row.schema_id=v.schema_id WHERE schema_row.name=N'{s}' AND v.name IN ({table_filter})) objects ORDER BY object_name,type_desc FOR JSON PATH)"),
            format!("SELECT (SELECT t.name AS table_name,c.column_id,c.name AS column_name,ty.name AS type_name,c.max_length,c.precision,c.scale,c.is_nullable,c.is_identity,c.is_computed,COALESCE(dc.definition,N'') AS default_definition FROM sys.tables t JOIN sys.schemas s ON s.schema_id=t.schema_id JOIN sys.columns c ON c.object_id=t.object_id JOIN sys.types ty ON ty.user_type_id=c.user_type_id LEFT JOIN sys.default_constraints dc ON dc.object_id=c.default_object_id WHERE s.name=N'{s}' AND t.name IN ({table_filter}) ORDER BY t.name,c.column_id FOR JSON PATH)"),
            format!("SELECT (SELECT t.name AS table_name,i.name AS index_name,i.type_desc,i.is_unique,i.is_primary_key,i.is_unique_constraint,COALESCE(i.filter_definition,N'') AS filter_definition,ic.key_ordinal,ic.is_descending_key,ic.is_included_column,c.name AS column_name FROM sys.tables t JOIN sys.schemas s ON s.schema_id=t.schema_id JOIN sys.indexes i ON i.object_id=t.object_id JOIN sys.index_columns ic ON ic.object_id=i.object_id AND ic.index_id=i.index_id JOIN sys.columns c ON c.object_id=ic.object_id AND c.column_id=ic.column_id WHERE s.name=N'{s}' AND t.name IN ({table_filter}) AND i.index_id>0 ORDER BY t.name,i.name,ic.index_column_id FOR JSON PATH)"),
            format!("SELECT (SELECT t.name AS table_name,cc.name AS constraint_name,cc.definition,cc.is_disabled,cc.is_not_trusted FROM sys.check_constraints cc JOIN sys.tables t ON t.object_id=cc.parent_object_id JOIN sys.schemas s ON s.schema_id=t.schema_id WHERE s.name=N'{s}' AND t.name IN ({table_filter}) ORDER BY t.name,cc.name FOR JSON PATH)"),
            format!("SELECT (SELECT parent_table.name AS parent_table,fk.name AS foreign_key,parent_column.name AS parent_column,referenced_table.name AS referenced_table,referenced_column.name AS referenced_column,fkc.constraint_column_id,fk.delete_referential_action,fk.update_referential_action FROM sys.foreign_keys fk JOIN sys.foreign_key_columns fkc ON fkc.constraint_object_id=fk.object_id JOIN sys.tables parent_table ON parent_table.object_id=fk.parent_object_id JOIN sys.schemas parent_schema ON parent_schema.schema_id=parent_table.schema_id JOIN sys.columns parent_column ON parent_column.object_id=fkc.parent_object_id AND parent_column.column_id=fkc.parent_column_id JOIN sys.tables referenced_table ON referenced_table.object_id=fk.referenced_object_id JOIN sys.columns referenced_column ON referenced_column.object_id=fkc.referenced_object_id AND referenced_column.column_id=fkc.referenced_column_id WHERE parent_schema.name=N'{s}' AND parent_table.name IN ({table_filter}) ORDER BY parent_table.name,fk.name,fkc.constraint_column_id FOR JSON PATH)"),
        ];
        let mut rows = Vec::with_capacity(queries.len());
        for query in queries {
            let value = self
                .one(&query)?
                .get::<&str, _>(0)
                .unwrap_or("[]")
                .to_string();
            rows.push(value);
        }
        Ok(super::schema_contract::fingerprint(rows))
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
                "SELECT buffer_id,run_id,cursor_abs FROM [{}].logging_checkpoint WHERE singleton=1",
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
        let result = self.commit_inner(batch);
        if result.is_err() {
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
        if let Some(row) = self.rows(&format!("BEGIN TRANSACTION; SELECT buffer_id,run_id,cursor_abs FROM [{s}].logging_checkpoint WITH(UPDLOCK,HOLDLOCK) WHERE singleton=1"))?.first() {
            let buffer: i64 = row.get(0).ok_or_else(|| commit_error("checkpoint buffer is absent"))?;
            let run_bytes: &[u8] = row.get(1).ok_or_else(|| commit_error("checkpoint run is absent"))?;
            let current_run = decode_u64(run_bytes, "checkpoint run")?;
            let cursor: &[u8] = row.get(2).ok_or_else(|| commit_error("checkpoint cursor is absent"))?;
            let current = decode_u64(cursor, "checkpoint cursor")?;
            if buffer == i64::from(batch.checkpoint.buffer_id) && current_run == batch.checkpoint.run_id && batch.checkpoint.cursor_abs < current {
                return Err(PersistenceError::CheckpointRegression { current, requested: batch.checkpoint.cursor_abs });
            }
        }
        let projected = batch
            .documents
            .iter()
            .map(|document| self.projector.project(document))
            .collect::<Result<Vec<_>, _>>()?;
        let mut existing = std::collections::HashMap::new();
        if !projected.is_empty() {
            let placeholders = (1..=projected.len())
                .map(|index| format!("@P{index}"))
                .collect::<Vec<_>>()
                .join(",");
            let mut lookup = Query::new(format!(
                "SELECT identity_key,canonical_json FROM [{s}].logging_records WITH(UPDLOCK,HOLDLOCK) WHERE identity_key IN ({placeholders})"
            ));
            for document in &projected {
                lookup.bind(document.canonical.identity_key.as_str());
            }
            let rows = self
                .runtime
                .block_on(async {
                    lookup
                        .query(&mut self.client)
                        .await?
                        .into_first_result()
                        .await
                })
                .map_err(|error| sql_error("check identities", error))?;
            for row in rows {
                let identity = row
                    .get::<&str, _>(0)
                    .ok_or_else(|| commit_error("identity key is absent"))?;
                let json = row
                    .get::<&str, _>(1)
                    .ok_or_else(|| commit_error("canonical JSON is absent"))?;
                existing.insert(identity.to_string(), json.to_string());
            }
        }
        let mut duplicated = 0;
        let mut pending = Vec::new();
        for projected in projected {
            let row = &projected.canonical;
            if let Some(json) = existing.get(&row.identity_key) {
                if json == &row.canonical_json {
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
            let mut parameter = 1;
            let tuples = pending
                .iter()
                .map(|_| {
                    let tuple = (0..16)
                        .map(|_| {
                            let placeholder = format!("@P{parameter}");
                            parameter += 1;
                            placeholder
                        })
                        .collect::<Vec<_>>()
                        .join(",");
                    format!("({tuple})")
                })
                .collect::<Vec<_>>()
                .join(",");
            let mut q = Query::new(format!("INSERT INTO [{s}].logging_records(identity_key,document_kind,buffer_id,run_id,source_id,epoch_id,seq,first_seq,last_seq,loss_basis,source_time_ns,receive_time_ns,event_type_id,event_name,definition_hash,canonical_json) VALUES {tuples}"));
            for projected in &pending {
                let row = &projected.canonical;
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
            }
            self.runtime
                .block_on(q.execute(&mut self.client))
                .map_err(|e| sql_error("insert document batch", e))?;
            let events = pending
                .iter()
                .filter_map(|document| document.event.as_ref())
                .collect::<Vec<_>>();
            super::sqlserver_read_model::insert_event_batch(
                &self.runtime,
                &mut self.client,
                &s,
                &events,
            )?;
            super::sqlserver_read_model::insert_repeated_domains_combined(
                &self.runtime,
                &mut self.client,
                &s,
                &pending,
            )?;
            let values = pending
                .iter()
                .flat_map(|document| document.logged_values.iter())
                .collect::<Vec<_>>();
            super::sqlserver_read_model::insert_value_batch(
                &self.runtime,
                &mut self.client,
                &s,
                &values,
            )?;
            super::sqlserver_read_model::insert_singleton_event_domains_batch(
                &self.runtime,
                &mut self.client,
                &s,
                &pending,
            )?;
            super::sqlserver_read_model::insert_loss_and_unresolved_batch(
                &self.runtime,
                &mut self.client,
                &s,
                &pending,
            )?;
        }
        for mut projected in pending {
            projected.event = None;
            projected.logged_values.clear();
            projected.domains.retain(|domain| {
                !matches!(
                    domain,
                    super::projection_domains::DomainRow::Alarm(_)
                        | super::projection_domains::DomainRow::System(_)
                        | super::projection_domains::DomainRow::Operator(_)
                        | super::projection_domains::DomainRow::Recipe(_)
                        | super::projection_domains::DomainRow::Message(_)
                        | super::projection_domains::DomainRow::State(_)
                        | super::projection_domains::DomainRow::Batch(_)
                        | super::projection_domains::DomainRow::Material(_)
                        | super::projection_domains::DomainRow::Audit(_)
                        | super::projection_domains::DomainRow::Signature(_)
                        | super::projection_domains::DomainRow::Loss(_)
                        | super::projection_domains::DomainRow::Unresolved(_)
                )
            });
            super::sqlserver_read_model::insert_projection(
                &self.runtime,
                &mut self.client,
                &s,
                projected.event,
                projected.logged_values,
                projected.domains,
            )?;
        }
        self.batch(&format!("MERGE [{s}].logging_checkpoint WITH(HOLDLOCK) AS t USING(SELECT CAST(1 AS BIT) singleton,{} buffer_id,0x{:016x} run_id,0x{:016x} cursor_abs) AS x ON t.singleton=x.singleton WHEN MATCHED THEN UPDATE SET buffer_id=x.buffer_id,run_id=x.run_id,cursor_abs=x.cursor_abs WHEN NOT MATCHED THEN INSERT(singleton,buffer_id,run_id,cursor_abs) VALUES(x.singleton,x.buffer_id,x.run_id,x.cursor_abs); COMMIT TRANSACTION;", batch.checkpoint.buffer_id, batch.checkpoint.run_id, batch.checkpoint.cursor_abs))?;
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

fn sqlserver_connect_error(error: tiberius::error::Error) -> PersistenceError {
    let retryable = matches!(
        error,
        tiberius::error::Error::Io { .. } | tiberius::error::Error::Routing { .. }
    );
    let message = format!("SQL Server connect with required TLS: {error}");
    if retryable {
        PersistenceError::Connection(message)
    } else {
        PersistenceError::Commit(message)
    }
}
fn sql_error(context: &str, error: tiberius::error::Error) -> PersistenceError {
    commit_error(&format!("{context}: {error}"))
}
