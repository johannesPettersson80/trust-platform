use open_ot_document::{
    Document, DocumentEpoch, DocumentField, DocumentFlags, DocumentKind, DocumentSource,
    EpochRelation, EventDocument, LossBasis, LossDocument, PlaceholderDocument,
    PlaceholderReasonDocument, PlaceholderReasonKind, Provenance, RawSlot,
};

use super::contracts::{
    DocumentSink, InMemoryDocumentSink, PersistenceBatch, PersistenceCheckpoint,
};
#[cfg(feature = "openot-real-database-tests")]
use super::{
    InfluxDb3DocumentSink, MySqlDocumentSink, PostgreSqlDocumentSink, SqlServerDocumentSink,
    TimescaleDbDocumentSink,
};
use super::{OpenOtDocumentSink, PersistenceError, SqliteDocumentSink};

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
mod sqlite;

use fixtures::*;
#[cfg(feature = "openot-real-database-tests")]
use real_restart::*;
