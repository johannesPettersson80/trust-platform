use std::path::Path;

use crate::config::{OpenOtPersistenceBackend, OpenOtPersistenceConfig};

#[cfg(feature = "openot-database-influxdb3")]
use super::InfluxDb3DocumentSink;
#[cfg(feature = "openot-database-mysql")]
use super::MySqlDocumentSink;
#[cfg(feature = "openot-database-postgresql")]
use super::PostgreSqlDocumentSink;
#[cfg(feature = "openot-database-sqlserver")]
use super::SqlServerDocumentSink;
#[cfg(feature = "openot-database-sqlite")]
use super::SqliteDocumentSink;
#[cfg(feature = "openot-database-timescaledb")]
use super::TimescaleDbDocumentSink;
use super::{CommitOutcome, DocumentSink, PersistenceBatch, PersistenceError};

/// Exactly one concrete sink selected by `runtime.openot.persistence.backend`.
#[derive(Debug)]
pub enum OpenOtDocumentSink {
    /// Local SQLite adapter.
    #[cfg(feature = "openot-database-sqlite")]
    Sqlite(SqliteDocumentSink),
    /// Remote PostgreSQL adapter.
    #[cfg(feature = "openot-database-postgresql")]
    PostgreSql(PostgreSqlDocumentSink),
    /// TimescaleDB adapter with a verified hypertable.
    #[cfg(feature = "openot-database-timescaledb")]
    TimescaleDb(TimescaleDbDocumentSink),
    /// Shared MySQL/MariaDB adapter.
    #[cfg(feature = "openot-database-mysql")]
    MySql(MySqlDocumentSink),
    /// Microsoft SQL Server/Azure SQL adapter.
    #[cfg(feature = "openot-database-sqlserver")]
    SqlServer(SqlServerDocumentSink),
    /// InfluxDB 3 adapter with mandatory local spool.
    #[cfg(feature = "openot-database-influxdb3")]
    InfluxDb3(InfluxDb3DocumentSink),
}

#[cfg(not(all(
    feature = "openot-database-sqlite",
    feature = "openot-database-postgresql",
    feature = "openot-database-timescaledb",
    feature = "openot-database-mysql",
    feature = "openot-database-sqlserver",
    feature = "openot-database-influxdb3"
)))]
fn backend_unavailable(backend: &str) -> PersistenceError {
    PersistenceError::BackendUnavailable(format!(
        "runtime.openot.persistence.backend '{backend}' is not compiled into this binary"
    ))
}

impl OpenOtDocumentSink {
    /// Validates and opens only the explicitly selected adapter.
    pub fn open(
        config: &OpenOtPersistenceConfig,
        bundle_root: &Path,
    ) -> Result<Self, PersistenceError> {
        Self::open_with_definitions(config, bundle_root, &[])
    }

    pub(crate) fn open_with_definitions(
        config: &OpenOtPersistenceConfig,
        bundle_root: &Path,
        definitions: &[open_ot_definition::DefinitionFile],
    ) -> Result<Self, PersistenceError> {
        if !config.enabled {
            return Err(PersistenceError::InvalidConfig(
                "runtime.openot.persistence is disabled".to_string(),
            ));
        }
        let backend = config.backend.ok_or_else(|| {
            PersistenceError::InvalidConfig(
                "runtime.openot.persistence.backend is required".to_string(),
            )
        })?;
        match backend {
            #[cfg(feature = "openot-database-sqlite")]
            OpenOtPersistenceBackend::Sqlite => {
                let sqlite = config.sqlite.as_ref().ok_or_else(|| {
                    PersistenceError::InvalidConfig(
                        "runtime.openot.persistence.sqlite is required".to_string(),
                    )
                })?;
                let path = if sqlite.path.is_absolute() {
                    sqlite.path.clone()
                } else {
                    bundle_root.join(&sqlite.path)
                };
                SqliteDocumentSink::open_with_definitions(&path, definitions.to_vec())
                    .map(Self::Sqlite)
            }
            #[cfg(not(feature = "openot-database-sqlite"))]
            OpenOtPersistenceBackend::Sqlite => Err(backend_unavailable("sqlite")),
            #[cfg(feature = "openot-database-postgresql")]
            OpenOtPersistenceBackend::PostgreSql => {
                let postgresql = config.postgresql.as_ref().ok_or_else(|| {
                    PersistenceError::InvalidConfig(
                        "runtime.openot.persistence.postgresql is required".to_string(),
                    )
                })?;
                let connection_url = std::env::var(postgresql.connection_url_env.as_str())
                    .map_err(|_| {
                        PersistenceError::InvalidConfig(format!(
                            "environment variable '{}' is not set",
                            postgresql.connection_url_env
                        ))
                    })?;
                let ca_cert_path = postgresql.ca_cert_path.as_ref().ok_or_else(|| {
                    PersistenceError::InvalidConfig(
                        "runtime.openot.persistence.postgresql.ca_cert_path is required"
                            .to_string(),
                    )
                })?;
                let ca_cert_path = if ca_cert_path.is_absolute() {
                    ca_cert_path.clone()
                } else {
                    bundle_root.join(ca_cert_path)
                };
                PostgreSqlDocumentSink::open_with_definitions(
                    &connection_url,
                    postgresql.schema.as_str(),
                    &ca_cert_path,
                    definitions.to_vec(),
                )
                .map(Self::PostgreSql)
            }
            #[cfg(not(feature = "openot-database-postgresql"))]
            OpenOtPersistenceBackend::PostgreSql => Err(backend_unavailable("postgresql")),
            #[cfg(feature = "openot-database-timescaledb")]
            OpenOtPersistenceBackend::TimescaleDb => {
                let timescaledb = config.timescaledb.as_ref().ok_or_else(|| {
                    PersistenceError::InvalidConfig(
                        "runtime.openot.persistence.timescaledb is required".to_string(),
                    )
                })?;
                let connection_url = std::env::var(timescaledb.connection_url_env.as_str())
                    .map_err(|_| {
                        PersistenceError::InvalidConfig(format!(
                            "environment variable '{}' is not set",
                            timescaledb.connection_url_env
                        ))
                    })?;
                let ca_cert_path = timescaledb.ca_cert_path.as_ref().ok_or_else(|| {
                    PersistenceError::InvalidConfig(
                        "runtime.openot.persistence.timescaledb.ca_cert_path is required"
                            .to_string(),
                    )
                })?;
                let ca_cert_path = if ca_cert_path.is_absolute() {
                    ca_cert_path.clone()
                } else {
                    bundle_root.join(ca_cert_path)
                };
                TimescaleDbDocumentSink::open_with_definitions(
                    &connection_url,
                    timescaledb.schema.as_str(),
                    &ca_cert_path,
                    definitions.to_vec(),
                )
                .map(Self::TimescaleDb)
            }
            #[cfg(not(feature = "openot-database-timescaledb"))]
            OpenOtPersistenceBackend::TimescaleDb => Err(backend_unavailable("timescaledb")),
            #[cfg(feature = "openot-database-mysql")]
            OpenOtPersistenceBackend::MySql => {
                let mysql = config.mysql.as_ref().ok_or_else(|| {
                    PersistenceError::InvalidConfig(
                        "runtime.openot.persistence.mysql is required".to_string(),
                    )
                })?;
                let connection_url =
                    std::env::var(mysql.connection_url_env.as_str()).map_err(|_| {
                        PersistenceError::InvalidConfig(format!(
                            "environment variable '{}' is not set",
                            mysql.connection_url_env
                        ))
                    })?;
                let ca_cert_path = mysql.ca_cert_path.as_ref().ok_or_else(|| {
                    PersistenceError::InvalidConfig(
                        "runtime.openot.persistence.mysql.ca_cert_path is required".to_string(),
                    )
                })?;
                let ca_cert_path = if ca_cert_path.is_absolute() {
                    ca_cert_path.clone()
                } else {
                    bundle_root.join(ca_cert_path)
                };
                MySqlDocumentSink::open_with_definitions(
                    &connection_url,
                    mysql.database.as_str(),
                    &ca_cert_path,
                    definitions.to_vec(),
                )
                .map(Self::MySql)
            }
            #[cfg(not(feature = "openot-database-mysql"))]
            OpenOtPersistenceBackend::MySql => Err(backend_unavailable("mysql")),
            #[cfg(feature = "openot-database-sqlserver")]
            OpenOtPersistenceBackend::SqlServer => {
                let sqlserver = config.sqlserver.as_ref().ok_or_else(|| {
                    PersistenceError::InvalidConfig(
                        "runtime.openot.persistence.sqlserver is required".to_string(),
                    )
                })?;
                let connection_url =
                    std::env::var(sqlserver.connection_url_env.as_str()).map_err(|_| {
                        PersistenceError::InvalidConfig(format!(
                            "environment variable '{}' is not set",
                            sqlserver.connection_url_env
                        ))
                    })?;
                let ca_cert_path = sqlserver.ca_cert_path.as_ref().ok_or_else(|| {
                    PersistenceError::InvalidConfig(
                        "runtime.openot.persistence.sqlserver.ca_cert_path is required".to_string(),
                    )
                })?;
                let ca_cert_path = if ca_cert_path.is_absolute() {
                    ca_cert_path.clone()
                } else {
                    bundle_root.join(ca_cert_path)
                };
                SqlServerDocumentSink::open_with_definitions(
                    &connection_url,
                    sqlserver.schema.as_str(),
                    &ca_cert_path,
                    definitions.to_vec(),
                )
                .map(Self::SqlServer)
            }
            #[cfg(not(feature = "openot-database-sqlserver"))]
            OpenOtPersistenceBackend::SqlServer => Err(backend_unavailable("sqlserver")),
            #[cfg(feature = "openot-database-influxdb3")]
            OpenOtPersistenceBackend::InfluxDb3 => {
                let influx = config.influxdb3.as_ref().ok_or_else(|| {
                    PersistenceError::InvalidConfig(
                        "runtime.openot.persistence.influxdb3 is required".to_string(),
                    )
                })?;
                let host = std::env::var(influx.host_env.as_str()).map_err(|_| {
                    PersistenceError::InvalidConfig(format!(
                        "environment variable '{}' is not set",
                        influx.host_env
                    ))
                })?;
                let token = std::env::var(influx.token_env.as_str()).map_err(|_| {
                    PersistenceError::InvalidConfig(format!(
                        "environment variable '{}' is not set",
                        influx.token_env
                    ))
                })?;
                let spool = if influx.spool_path.is_absolute() {
                    influx.spool_path.clone()
                } else {
                    bundle_root.join(&influx.spool_path)
                };
                let ca = influx.ca_cert_path.as_ref().ok_or_else(|| {
                    PersistenceError::InvalidConfig(
                        "runtime.openot.persistence.influxdb3.ca_cert_path is required".to_string(),
                    )
                })?;
                let ca = if ca.is_absolute() {
                    ca.clone()
                } else {
                    bundle_root.join(ca)
                };
                InfluxDb3DocumentSink::open_bounded_with_definitions(
                    &host,
                    &token,
                    influx.database.as_str(),
                    &spool,
                    &ca,
                    influx.max_bytes,
                    definitions.to_vec(),
                )
                .map(Self::InfluxDb3)
            }
            #[cfg(not(feature = "openot-database-influxdb3"))]
            OpenOtPersistenceBackend::InfluxDb3 => Err(backend_unavailable("influxdb3")),
        }
    }

    #[cfg(all(test, feature = "openot-real-database-tests"))]
    pub(crate) fn storage_bytes(&mut self) -> Result<u64, PersistenceError> {
        match self {
            #[cfg(feature = "openot-database-sqlite")]
            Self::Sqlite(sink) => sink.storage_bytes(),
            #[cfg(feature = "openot-database-postgresql")]
            Self::PostgreSql(sink) => sink.storage_bytes(),
            #[cfg(feature = "openot-database-timescaledb")]
            Self::TimescaleDb(sink) => sink.storage_bytes(),
            #[cfg(feature = "openot-database-mysql")]
            Self::MySql(sink) => sink.storage_bytes(),
            #[cfg(feature = "openot-database-sqlserver")]
            Self::SqlServer(sink) => sink.storage_bytes(),
            #[cfg(feature = "openot-database-influxdb3")]
            Self::InfluxDb3(sink) => sink.spool_logical_bytes(),
        }
    }
}

impl DocumentSink for OpenOtDocumentSink {
    fn maintenance(&mut self) -> Result<usize, PersistenceError> {
        match self {
            #[cfg(feature = "openot-database-sqlite")]
            Self::Sqlite(sink) => sink.maintenance(),
            #[cfg(feature = "openot-database-postgresql")]
            Self::PostgreSql(sink) => sink.maintenance(),
            #[cfg(feature = "openot-database-timescaledb")]
            Self::TimescaleDb(sink) => sink.maintenance(),
            #[cfg(feature = "openot-database-mysql")]
            Self::MySql(sink) => sink.maintenance(),
            #[cfg(feature = "openot-database-sqlserver")]
            Self::SqlServer(sink) => sink.maintenance(),
            #[cfg(feature = "openot-database-influxdb3")]
            Self::InfluxDb3(sink) => sink.maintenance(),
        }
    }

    fn load_checkpoint(
        &mut self,
        buffer_id: u32,
        run_id: u64,
    ) -> Result<Option<super::PersistenceCheckpoint>, PersistenceError> {
        match self {
            #[cfg(feature = "openot-database-sqlite")]
            Self::Sqlite(sink) => sink.load_checkpoint(buffer_id, run_id),
            #[cfg(feature = "openot-database-postgresql")]
            Self::PostgreSql(sink) => sink.load_checkpoint(buffer_id, run_id),
            #[cfg(feature = "openot-database-timescaledb")]
            Self::TimescaleDb(sink) => sink.load_checkpoint(buffer_id, run_id),
            #[cfg(feature = "openot-database-mysql")]
            Self::MySql(sink) => sink.load_checkpoint(buffer_id, run_id),
            #[cfg(feature = "openot-database-sqlserver")]
            Self::SqlServer(sink) => sink.load_checkpoint(buffer_id, run_id),
            #[cfg(feature = "openot-database-influxdb3")]
            Self::InfluxDb3(sink) => sink.load_checkpoint(buffer_id, run_id),
        }
    }

    fn commit(&mut self, batch: &PersistenceBatch) -> Result<CommitOutcome, PersistenceError> {
        match self {
            #[cfg(feature = "openot-database-sqlite")]
            Self::Sqlite(sink) => sink.commit(batch),
            #[cfg(feature = "openot-database-postgresql")]
            Self::PostgreSql(sink) => sink.commit(batch),
            #[cfg(feature = "openot-database-timescaledb")]
            Self::TimescaleDb(sink) => sink.commit(batch),
            #[cfg(feature = "openot-database-mysql")]
            Self::MySql(sink) => sink.commit(batch),
            #[cfg(feature = "openot-database-sqlserver")]
            Self::SqlServer(sink) => sink.commit(batch),
            #[cfg(feature = "openot-database-influxdb3")]
            Self::InfluxDb3(sink) => sink.commit(batch),
        }
    }
}
