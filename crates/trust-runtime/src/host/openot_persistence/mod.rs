//! Backend-neutral host contracts for OpenOT document persistence.

mod consumer;
mod contracts;
mod factory;
#[cfg(feature = "openot-database-influxdb3")]
mod influxdb3;
#[cfg(feature = "openot-database-influxdb3")]
mod influxdb3_read_model;
#[cfg(feature = "openot-database-mysql")]
mod mysql;
#[cfg(feature = "openot-database-mysql")]
mod mysql_read_model;
#[cfg(any(
    feature = "openot-database-postgresql",
    feature = "openot-database-timescaledb"
))]
mod postgres_read_model;
#[cfg(any(
    feature = "openot-database-postgresql",
    feature = "openot-database-timescaledb"
))]
mod postgresql;
mod projection;
mod projection_domains;
mod service;
mod source;
#[cfg(feature = "openot-database-sqlite")]
mod sqlite;
#[cfg(feature = "openot-database-sqlite")]
mod sqlite_read_model;
#[cfg(feature = "openot-database-sqlserver")]
mod sqlserver;
#[cfg(feature = "openot-database-sqlserver")]
mod sqlserver_read_model;
#[cfg(feature = "openot-database-timescaledb")]
mod timescaledb;
mod worker;

pub use consumer::OpenOtPersistenceConsumer;
pub use contracts::{
    CommitOutcome, DocumentSink, PersistenceBatch, PersistenceCheckpoint, PersistenceError,
};
pub use factory::OpenOtDocumentSink;
#[cfg(feature = "openot-database-influxdb3")]
pub use influxdb3::InfluxDb3DocumentSink;
#[cfg(feature = "openot-database-mysql")]
pub use mysql::MySqlDocumentSink;
#[cfg(any(
    feature = "openot-database-postgresql",
    feature = "openot-database-timescaledb"
))]
pub use postgresql::PostgreSqlDocumentSink;
pub use service::{OpenOtPersistenceService, OpenOtPersistenceState, OpenOtPersistenceStatus};
#[cfg(unix)]
pub use source::SharedMemoryOpenOtSource;
#[cfg(feature = "openot-database-sqlite")]
pub use sqlite::SqliteDocumentSink;
#[cfg(feature = "openot-database-sqlserver")]
pub use sqlserver::SqlServerDocumentSink;
#[cfg(feature = "openot-database-timescaledb")]
pub use timescaledb::TimescaleDbDocumentSink;
pub use worker::{
    OpenOtDocumentSource, OpenOtPersistenceWorker, OpenOtPersistenceWorkerStatus, OpenOtSourcePoll,
};

#[cfg(test)]
mod contract_tests;
