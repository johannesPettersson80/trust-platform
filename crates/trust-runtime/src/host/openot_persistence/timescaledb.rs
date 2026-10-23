use std::path::Path;

use super::{
    CommitOutcome, DocumentSink, PersistenceBatch, PersistenceError, PostgreSqlDocumentSink,
};

/// TimescaleDB-backed OpenOT sink with an extension-owned time index.
#[derive(Debug)]
pub struct TimescaleDbDocumentSink {
    postgresql: PostgreSqlDocumentSink,
}

impl TimescaleDbDocumentSink {
    /// Returns the compatible truST-owned relational schema version.
    pub fn schema_version(&mut self) -> Result<u32, PersistenceError> {
        self.postgresql.schema_version()
    }

    /// Connects with authenticated TLS and opens the initial TimescaleDB schema.
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
        PostgreSqlDocumentSink::open_timescale(connection_url, schema, ca_cert_path, definitions)
            .map(|postgresql| Self { postgresql })
    }

    /// Returns the loaded TimescaleDB extension version.
    pub fn extension_version(&mut self) -> Result<String, PersistenceError> {
        self.postgresql
            .client
            .query_one(
                "SELECT extversion FROM pg_extension WHERE extname = 'timescaledb'",
                &[],
            )
            .map(|row| row.get(0))
            .map_err(super::postgresql::pg_error(
                "read TimescaleDB extension version",
            ))
    }

    /// Reports whether every required public time-series object is a hypertable.
    pub fn hypertable_exists(&mut self) -> Result<bool, PersistenceError> {
        self.postgresql
            .client
            .query_one(
                "SELECT COUNT(*) = 5 FROM timescaledb_information.hypertables
                 WHERE hypertable_schema = $1
                   AND hypertable_name IN ('event_log','logged_values','alarm_history','message_log','state_history')",
                &[&self.postgresql.schema],
            )
            .map(|row| row.get(0))
            .map_err(super::postgresql::pg_error(
                "inspect TimescaleDB hypertable",
            ))
    }

    #[cfg(all(test, feature = "openot-real-database-tests"))]
    pub(crate) fn time_index_count(&mut self) -> Result<i64, PersistenceError> {
        self.postgresql
            .client
            .query_one(
                &format!(
                    "SELECT COUNT(*) FROM \"{}\".event_log",
                    self.postgresql.schema
                ),
                &[],
            )
            .map(|row| row.get(0))
            .map_err(super::postgresql::pg_error("count TimescaleDB time index"))
    }

    #[cfg(all(test, feature = "openot-real-database-tests"))]
    pub(crate) fn audited_value_projection(
        &mut self,
    ) -> Result<super::contracts::AuditedValueProjection, PersistenceError> {
        self.postgresql
            .client
            .query_one(
                &format!(
                    "SELECT previous_boolean_value,is_audited,actor,reason,authorization_result \
                     FROM \"{}\".logged_values WHERE is_audited LIMIT 1",
                    self.postgresql.schema
                ),
                &[],
            )
            .map(|row| (row.get(0), row.get(1), row.get(2), row.get(3), row.get(4)))
            .map_err(super::postgresql::pg_error(
                "read TimescaleDB audited value projection",
            ))
    }

    #[cfg(all(test, feature = "openot-real-database-tests"))]
    pub(crate) fn set_required_index_present_for_test(
        &mut self,
        present: bool,
    ) -> Result<(), PersistenceError> {
        let statement = if present {
            format!(
                "CREATE INDEX logging_records_receive_time ON \"{}\".logging_records(receive_time_ns)",
                self.postgresql.schema
            )
        } else {
            format!(
                "DROP INDEX \"{}\".logging_records_receive_time",
                self.postgresql.schema
            )
        };
        self.postgresql
            .client
            .batch_execute(&statement)
            .map_err(super::postgresql::pg_error(
                "change TimescaleDB required index for compatibility test",
            ))
    }

    #[cfg(feature = "openot-real-database-tests")]
    #[doc(hidden)]
    pub fn canonical_jsons(&mut self) -> Result<Vec<String>, PersistenceError> {
        self.postgresql.canonical_jsons()
    }

    #[cfg(all(test, feature = "openot-real-database-tests"))]
    pub(crate) fn set_schema_version_for_test(
        &mut self,
        version: u32,
    ) -> Result<(), PersistenceError> {
        self.postgresql.set_schema_version_for_test(version)
    }

    #[cfg(all(test, feature = "openot-real-database-tests"))]
    pub(crate) fn remove_schema_marker_for_test(&mut self) -> Result<(), PersistenceError> {
        self.postgresql.remove_schema_marker_for_test()
    }

    #[cfg(all(test, feature = "openot-real-database-tests"))]
    pub(crate) fn storage_bytes(&mut self) -> Result<u64, PersistenceError> {
        self.postgresql.storage_bytes()
    }
}

impl DocumentSink for TimescaleDbDocumentSink {
    fn load_checkpoint(
        &mut self,
        buffer_id: u32,
        run_id: u64,
    ) -> Result<Option<super::PersistenceCheckpoint>, PersistenceError> {
        self.postgresql.load_checkpoint(buffer_id, run_id)
    }

    fn commit(&mut self, batch: &PersistenceBatch) -> Result<CommitOutcome, PersistenceError> {
        self.postgresql.commit(batch)
    }
}
