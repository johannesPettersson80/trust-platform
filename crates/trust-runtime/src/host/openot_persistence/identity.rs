//! Canonical document identities shared by sinks and their contract fixtures.
use open_ot_document::{Document, LossBasis};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum DocumentIdentity {
    Record {
        buffer_id: u32,
        run_id: u64,
        source_id: u32,
        seq: u64,
    },
    Loss {
        buffer_id: u32,
        run_id: u64,
        source_id: u32,
        epoch_id: u64,
        first_seq: u64,
        last_seq: u64,
        basis: &'static str,
    },
}

impl DocumentIdentity {
    #[cfg(any(
        feature = "openot-database-sqlite",
        feature = "openot-database-postgresql",
        feature = "openot-database-timescaledb",
        feature = "openot-database-mysql",
        feature = "openot-database-sqlserver",
        feature = "openot-database-influxdb3",
    ))]
    pub(super) fn storage_key(&self) -> String {
        match self {
            Self::Record {
                buffer_id,
                run_id,
                source_id,
                seq,
            } => format!("r:{buffer_id:08x}:{run_id:016x}:{source_id:08x}:{seq:016x}"),
            Self::Loss {
                buffer_id,
                run_id,
                source_id,
                epoch_id,
                first_seq,
                last_seq,
                basis,
            } => format!(
                "l:{buffer_id:08x}:{run_id:016x}:{source_id:08x}:{epoch_id:016x}:{first_seq:016x}:{last_seq:016x}:{basis}"
            ),
        }
    }
}

pub(super) fn document_identity(document: &Document) -> DocumentIdentity {
    match document {
        Document::Event(event) => DocumentIdentity::Record {
            buffer_id: event.provenance.buffer_id,
            run_id: event.provenance.run_id,
            source_id: event.provenance.source.id,
            seq: event.seq,
        },
        Document::Placeholder(placeholder) => DocumentIdentity::Record {
            buffer_id: placeholder.provenance.buffer_id,
            run_id: placeholder.provenance.run_id,
            source_id: placeholder.provenance.source.id,
            seq: placeholder.seq,
        },
        Document::Loss(loss) => DocumentIdentity::Loss {
            buffer_id: loss.provenance.buffer_id,
            run_id: loss.provenance.run_id,
            source_id: loss.provenance.source.id,
            epoch_id: loss.provenance.epoch.id,
            first_seq: loss.first_seq,
            last_seq: loss.last_seq,
            basis: match loss.basis {
                LossBasis::Authoritative => "authoritative",
                LossBasis::Inferred => "inferred",
            },
        },
    }
}
