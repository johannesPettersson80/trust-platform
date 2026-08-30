use open_ot_document::{Document, LossBasis};

use super::PersistenceError;

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

pub(super) struct DocumentRow {
    pub(super) identity_key: String,
    pub(super) document_kind: &'static str,
    pub(super) buffer_id: u32,
    pub(super) run_id: [u8; 8],
    pub(super) source_id: u32,
    pub(super) epoch_id: [u8; 8],
    pub(super) seq: Option<Vec<u8>>,
    pub(super) first_seq: Option<Vec<u8>>,
    pub(super) last_seq: Option<Vec<u8>>,
    pub(super) loss_basis: Option<&'static str>,
    pub(super) source_time_ns: Option<Vec<u8>>,
    pub(super) receive_time_ns: [u8; 8],
    pub(super) event_type_id: Option<u32>,
    pub(super) event_name: Option<String>,
    pub(super) definition_hash: String,
    pub(super) canonical_json: String,
}

pub(super) fn document_row(document: &Document) -> Result<DocumentRow, PersistenceError> {
    let canonical_json = open_ot_document::to_json(document)
        .map_err(|error| PersistenceError::Commit(format!("serialize OpenOT document: {error}")))?;
    let (
        document_kind,
        provenance,
        seq,
        first_seq,
        last_seq,
        loss_basis,
        event_type_id,
        event_name,
    ) = match document {
        Document::Event(event) => (
            "event",
            &event.provenance,
            Some(event.seq.to_be_bytes().to_vec()),
            None,
            None,
            None,
            Some(event.event_type_id),
            Some(event.event_name.clone()),
        ),
        Document::Placeholder(placeholder) => (
            "placeholder",
            &placeholder.provenance,
            Some(placeholder.seq.to_be_bytes().to_vec()),
            None,
            None,
            None,
            Some(placeholder.event_type_id),
            None,
        ),
        Document::Loss(loss) => (
            "loss",
            &loss.provenance,
            None,
            Some(loss.first_seq.to_be_bytes().to_vec()),
            Some(loss.last_seq.to_be_bytes().to_vec()),
            Some(match loss.basis {
                LossBasis::Authoritative => "authoritative",
                LossBasis::Inferred => "inferred",
            }),
            None,
            None,
        ),
    };
    Ok(DocumentRow {
        identity_key: document_identity(document).storage_key(),
        document_kind,
        buffer_id: provenance.buffer_id,
        run_id: provenance.run_id.to_be_bytes(),
        source_id: provenance.source.id,
        epoch_id: provenance.epoch.id.to_be_bytes(),
        seq,
        first_seq,
        last_seq,
        loss_basis,
        source_time_ns: provenance
            .source_time_ns
            .map(|value| value.to_be_bytes().to_vec()),
        receive_time_ns: provenance.receive_time_ns.to_be_bytes(),
        event_type_id,
        event_name,
        definition_hash: provenance.epoch.definition_hash.clone(),
        canonical_json,
    })
}
