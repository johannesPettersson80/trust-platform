use open_ot_document::{
    Document, DocumentEpoch, DocumentField, DocumentFlags, DocumentKind, DocumentSource,
    EpochRelation, EventDocument, LossBasis, LossDocument, PlaceholderDocument,
    PlaceholderReasonDocument, PlaceholderReasonKind, Provenance, RawSlot,
};

use super::contracts::{
    DocumentSink, InMemoryDocumentSink, PersistenceBatch, PersistenceCheckpoint,
};
#[cfg(feature = "openot-database-sqlite")]
use super::SqliteDocumentSink;
#[cfg(feature = "openot-real-database-tests")]
use super::{
    InfluxDb3DocumentSink, MySqlDocumentSink, PostgreSqlDocumentSink, SqlServerDocumentSink,
    TimescaleDbDocumentSink,
};
#[cfg(any(
    feature = "openot-database-sqlite",
    feature = "openot-real-database-tests"
))]
use super::{OpenOtDocumentSink, PersistenceError};

const CANONICAL_DOCUMENT_COUNT: usize = 37;

mod core;
mod fixtures;
#[cfg(feature = "openot-real-database-tests")]
mod performance;
#[cfg(feature = "openot-real-database-tests")]
mod real_influx;
#[cfg(feature = "openot-real-database-tests")]
mod real_relational;
#[cfg(feature = "openot-real-database-tests")]
mod real_restart;
#[cfg(feature = "openot-database-sqlite")]
mod sqlite;

use fixtures::*;
#[cfg(feature = "openot-real-database-tests")]
use real_restart::*;
