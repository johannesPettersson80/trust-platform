//! Product-owned conversion from OpenOT carriage batches to persistence batches.

use open_ot_carriage::consumer::LossAccountingConsumer;
use open_ot_carriage::control::ControlBlockSnapshot;
use open_ot_carriage::ring::ReadBatch;
use open_ot_definition::{compute_content_hash, resolve_record, DefinitionFile, DefinitionSet};
use open_ot_document::{
    document_from_loss, document_from_resolution, EpochRelation, LossDocumentContext,
    RecordDocumentContext,
};
use std::collections::BTreeSet;

use super::{PersistenceBatch, PersistenceCheckpoint, PersistenceError};

/// Stateful carriage-to-document converter owned by the runtime host.
#[derive(Debug)]
pub struct OpenOtPersistenceConsumer {
    current_definition: DefinitionFile,
    prior_definition: Option<DefinitionFile>,
    current_carriage_hash: [u8; 8],
    prior_carriage_hash: Option<[u8; 8]>,
    loss_accounting: Option<(u32, LossAccountingConsumer)>,
    emitted_loss: BTreeSet<(u32, u64, u32, u64, u64, bool)>,
}

impl OpenOtPersistenceConsumer {
    /// Validates and retains the current and optional immediately-prior definitions.
    pub fn new(
        current_definition: DefinitionFile,
        prior_definition: Option<DefinitionFile>,
    ) -> Result<Self, PersistenceError> {
        let current_carriage_hash = compute_content_hash(&current_definition)
            .map_err(|error| {
                PersistenceError::InvalidConfig(format!("hash OpenOT definition: {error}"))
            })?
            .carriage_hash;
        let prior_carriage_hash = prior_definition
            .as_ref()
            .map(compute_content_hash)
            .transpose()
            .map_err(|error| {
                PersistenceError::InvalidConfig(format!("hash prior OpenOT definition: {error}"))
            })?
            .map(|hash| hash.carriage_hash);
        Ok(Self {
            current_definition,
            prior_definition,
            current_carriage_hash,
            prior_carriage_hash,
            loss_accounting: None,
            emitted_loss: BTreeSet::new(),
        })
    }

    /// Resolves one carriage poll and binds it to the poll's next durable cursor.
    pub fn prepare_batch(
        &mut self,
        batch: &ReadBatch,
        snapshot: &ControlBlockSnapshot,
        receive_time_ns: u64,
        durable_cursor_abs: u64,
    ) -> Result<PersistenceBatch, PersistenceError> {
        let mut effective_snapshot = snapshot.clone();
        if snapshot.run_id == 0
            && snapshot.epoch_id == 0
            && snapshot.definition_hash == [0; 8]
            && snapshot.prev_definition_hash == [0; 8]
        {
            effective_snapshot.definition_hash = self.current_carriage_hash;
        }
        let snapshot = &effective_snapshot;
        let accounting = match &mut self.loss_accounting {
            Some((buffer_id, accounting)) if *buffer_id == snapshot.buffer_id => accounting,
            slot => {
                *slot = Some((
                    snapshot.buffer_id,
                    LossAccountingConsumer::with_buffer_id(snapshot.buffer_id),
                ));
                &mut slot.as_mut().expect("loss accounting initialized").1
            }
        };
        accounting.account_batch(batch);

        let definitions = match &self.prior_definition {
            Some(prior) => DefinitionSet::current_and_prior(&self.current_definition, prior),
            None => DefinitionSet::current(&self.current_definition),
        };
        let mut documents = Vec::new();
        for read in batch
            .records
            .iter()
            .filter(|read| read.end_abs > durable_cursor_abs)
        {
            let (definition_hash, semantic_version) = if read.start_abs >= snapshot.epoch_first_abs
            {
                (
                    snapshot.definition_hash,
                    (snapshot.definition_hash == self.current_carriage_hash)
                        .then_some(self.current_definition.header.semantic_version.as_str()),
                )
            } else {
                (
                    snapshot.prev_definition_hash,
                    self.prior_definition.as_ref().and_then(|prior| {
                        (Some(snapshot.prev_definition_hash) == self.prior_carriage_hash)
                            .then_some(prior.header.semantic_version.as_str())
                    }),
                )
            };
            let resolution = resolve_record(&read.record, read.start_abs, snapshot, &definitions);
            let mut context = RecordDocumentContext::new(
                snapshot.buffer_id,
                receive_time_ns,
                definition_hash,
                read.record.flags,
            )
            .with_source_time(read.record.source_time);
            if let Some(version) = semantic_version {
                context = context.with_semantic_version(version);
            }
            documents.push(document_from_resolution(&resolution, &context));
        }

        for loss in accounting.loss_events() {
            let key = (
                loss.buffer_id,
                loss.run_id,
                loss.source_id,
                loss.first_seq,
                loss.last_seq,
                loss.synthetic,
            );
            if !self.emitted_loss.insert(key) {
                continue;
            }
            let mut context = LossDocumentContext::new(
                receive_time_ns,
                snapshot.epoch_id,
                EpochRelation::Current,
                snapshot.definition_hash,
            );
            if snapshot.definition_hash == self.current_carriage_hash {
                context = context
                    .with_semantic_version(self.current_definition.header.semantic_version.clone());
            }
            documents.push(document_from_loss(&loss, &context));
        }

        Ok(PersistenceBatch {
            documents,
            checkpoint: PersistenceCheckpoint {
                buffer_id: snapshot.buffer_id,
                run_id: snapshot.run_id,
                cursor_abs: batch.next_abs,
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use open_ot_carriage::registry::EVENT_HEARTBEAT;
    use open_ot_carriage::ring::{ReadBatch, ReadRecord, DEFAULT_BUFFER_ID};
    use open_ot_carriage::wire::Record;
    use open_ot_definition::{compute_content_hash, sample_definition};
    use open_ot_document::{Document, EpochRelation};

    use super::*;

    #[test]
    fn consumer_resolves_records_accounts_gap_and_checkpoints_poll_cursor() {
        let definition = sample_definition();
        let hash = compute_content_hash(&definition).expect("hash sample definition");
        let first = Record::new(11, 7, 0, 1, EVENT_HEARTBEAT);
        let after_gap = Record::new(13, 7, 2, 1, EVENT_HEARTBEAT);
        let batch = ReadBatch {
            records: vec![
                ReadRecord {
                    start_abs: 0,
                    end_abs: 64,
                    record: first,
                },
                ReadRecord {
                    start_abs: 64,
                    end_abs: 128,
                    record: after_gap,
                },
            ],
            next_abs: 128,
            lapped: false,
        };
        let snapshot = ControlBlockSnapshot {
            version: 2,
            caps: 0,
            buffer_id: DEFAULT_BUFFER_ID,
            buffer_bytes: 4096,
            head_abs: 128,
            oldest_abs: 0,
            lost_count: 0,
            run_id: 7,
            epoch_id: 3,
            epoch_first_abs: 0,
            definition_hash: hash.carriage_hash,
            prev_definition_hash: [0; 8],
        };

        let mut consumer =
            OpenOtPersistenceConsumer::new(definition, None).expect("create consumer");
        let prepared = consumer
            .prepare_batch(&batch, &snapshot, 99, 0)
            .expect("resolve carriage batch");

        assert_eq!(prepared.checkpoint.buffer_id, DEFAULT_BUFFER_ID);
        assert_eq!(prepared.checkpoint.cursor_abs, 128);
        assert_eq!(prepared.documents.len(), 3);
        assert_eq!(
            prepared
                .documents
                .iter()
                .filter(|document| matches!(document, Document::Event(_)))
                .count(),
            2
        );
        let loss = prepared
            .documents
            .iter()
            .find_map(|document| match document {
                Document::Loss(loss) => Some(loss),
                _ => None,
            })
            .expect("inferred gap document");
        assert_eq!((loss.first_seq, loss.last_seq), (1, 1));
        assert_eq!(loss.provenance.receive_time_ns, 99);
    }

    #[test]
    fn consumer_binds_initial_zero_hash_epoch_to_compiled_bundle_definition() {
        let definition = sample_definition();
        let batch = ReadBatch {
            records: vec![ReadRecord {
                start_abs: 0,
                end_abs: 64,
                record: Record::new(11, 1, 0, 1, EVENT_HEARTBEAT),
            }],
            next_abs: 64,
            lapped: false,
        };
        let snapshot = ControlBlockSnapshot {
            version: 2,
            caps: 0,
            buffer_id: DEFAULT_BUFFER_ID,
            buffer_bytes: 4096,
            head_abs: 64,
            oldest_abs: 0,
            lost_count: 0,
            run_id: 0,
            epoch_id: 0,
            epoch_first_abs: 0,
            definition_hash: [0; 8],
            prev_definition_hash: [0; 8],
        };
        let mut consumer =
            OpenOtPersistenceConsumer::new(definition, None).expect("create consumer");

        let prepared = consumer
            .prepare_batch(&batch, &snapshot, 99, 0)
            .expect("resolve initial cold-start batch");

        assert!(matches!(
            prepared.documents.as_slice(),
            [Document::Event(_)]
        ));
    }

    #[test]
    fn consumer_resolves_warm_definition_change_against_prior_and_current_epochs() {
        let prior = sample_definition();
        let prior_hash = compute_content_hash(&prior).expect("hash prior definition");
        let mut current = prior.clone();
        current.header.semantic_version = "2.0.0".to_string();
        let current_hash = compute_content_hash(&current).expect("hash current definition");
        assert_ne!(prior_hash.carriage_hash, current_hash.carriage_hash);
        let batch = ReadBatch {
            records: vec![
                ReadRecord {
                    start_abs: 0,
                    end_abs: 64,
                    record: Record::new(11, 9, 0, 1, EVENT_HEARTBEAT),
                },
                ReadRecord {
                    start_abs: 64,
                    end_abs: 128,
                    record: Record::new(12, 9, 1, 1, EVENT_HEARTBEAT),
                },
            ],
            next_abs: 128,
            lapped: false,
        };
        let snapshot = ControlBlockSnapshot {
            version: 2,
            caps: 0,
            buffer_id: DEFAULT_BUFFER_ID,
            buffer_bytes: 4096,
            head_abs: 128,
            oldest_abs: 0,
            lost_count: 0,
            run_id: 9,
            epoch_id: 2,
            epoch_first_abs: 64,
            definition_hash: current_hash.carriage_hash,
            prev_definition_hash: prior_hash.carriage_hash,
        };
        let mut consumer =
            OpenOtPersistenceConsumer::new(current, Some(prior)).expect("create epoch consumer");

        let prepared = consumer
            .prepare_batch(&batch, &snapshot, 99, 0)
            .expect("resolve warm definition change");

        let events = prepared
            .documents
            .iter()
            .filter_map(|document| match document {
                Document::Event(event) => Some(event),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].provenance.epoch.relation, EpochRelation::Prior);
        assert_eq!(
            events[0].provenance.epoch.semantic_version.as_deref(),
            Some("1.0.0")
        );
        assert_eq!(events[1].provenance.epoch.relation, EpochRelation::Current);
        assert_eq!(
            events[1].provenance.epoch.semantic_version.as_deref(),
            Some("2.0.0")
        );
    }

    #[test]
    fn consumer_preserves_placeholder_when_definition_hash_does_not_match() {
        let definition = sample_definition();
        let batch = ReadBatch {
            records: vec![ReadRecord {
                start_abs: 0,
                end_abs: 64,
                record: Record::new(11, 3, 0, 1, EVENT_HEARTBEAT),
            }],
            next_abs: 64,
            lapped: false,
        };
        let snapshot = ControlBlockSnapshot {
            version: 2,
            caps: 0,
            buffer_id: DEFAULT_BUFFER_ID,
            buffer_bytes: 4096,
            head_abs: 64,
            oldest_abs: 0,
            lost_count: 0,
            run_id: 3,
            epoch_id: 4,
            epoch_first_abs: 0,
            definition_hash: [0xAA; 8],
            prev_definition_hash: [0; 8],
        };
        let mut consumer =
            OpenOtPersistenceConsumer::new(definition, None).expect("create consumer");

        let prepared = consumer
            .prepare_batch(&batch, &snapshot, 99, 0)
            .expect("preserve unresolved record");

        assert!(matches!(
            prepared.documents.as_slice(),
            [Document::Placeholder(_)]
        ));
    }
}
