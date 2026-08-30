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
    /// Connects with authenticated TLS, requires TimescaleDB, and migrates.
    pub fn open(
        connection_url: &str,
        schema: &str,
        ca_cert_path: &Path,
    ) -> Result<Self, PersistenceError> {
        PostgreSqlDocumentSink::open_timescale(connection_url, schema, ca_cert_path)
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
            .map_err(|error| {
                PersistenceError::Commit(format!("TimescaleDB read extension version: {error}"))
            })
    }

    /// Reports whether the OpenOT time index is an actual hypertable.
    pub fn hypertable_exists(&mut self) -> Result<bool, PersistenceError> {
        self.postgresql
            .client
            .query_one(
                "SELECT EXISTS (\n\
                     SELECT 1 FROM timescaledb_information.hypertables\n\
                     WHERE hypertable_schema = $1 AND hypertable_name = 'openot_time_index'\n\
                 )",
                &[&self.postgresql.schema],
            )
            .map(|row| row.get(0))
            .map_err(|error| {
                PersistenceError::Commit(format!("TimescaleDB inspect hypertable: {error}"))
            })
    }

    #[cfg(all(test, feature = "openot-real-database-tests"))]
    pub(crate) fn time_index_count(&mut self) -> Result<i64, PersistenceError> {
        self.postgresql
            .client
            .query_one(
                &format!(
                    "SELECT COUNT(*) FROM \"{}\".openot_time_index",
                    self.postgresql.schema
                ),
                &[],
            )
            .map(|row| row.get(0))
            .map_err(|error| {
                PersistenceError::Commit(format!("TimescaleDB count time index: {error}"))
            })
    }

    #[cfg(feature = "openot-real-database-tests")]
    #[doc(hidden)]
    pub fn canonical_jsons(&mut self) -> Result<Vec<String>, PersistenceError> {
        self.postgresql.canonical_jsons()
    }

    #[cfg(all(test, feature = "openot-real-database-tests"))]
    pub(crate) fn downgrade_checkpoint_to_v1_for_test(&mut self) -> Result<(), PersistenceError> {
        self.postgresql.downgrade_checkpoint_to_v1_for_test()
    }

    #[cfg(all(test, feature = "openot-real-database-tests"))]
    pub(crate) fn set_schema_version_for_test(
        &mut self,
        version: u32,
    ) -> Result<(), PersistenceError> {
        self.postgresql.set_schema_version_for_test(version)
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
