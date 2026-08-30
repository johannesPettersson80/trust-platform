//! Supervised persistence work kept outside the PLC scan thread.

use open_ot_carriage::{control::ControlBlockSnapshot, ring::ReadBatch};

use super::{CommitOutcome, DocumentSink, OpenOtPersistenceConsumer, PersistenceError};

/// One coherent source poll and the control metadata used to interpret it.
#[derive(Debug, Clone)]
pub struct OpenOtSourcePoll {
    /// Records and next absolute cursor returned by the carriage reader.
    pub batch: ReadBatch,
    /// Coherent producer control metadata used for resolution and lag.
    pub snapshot: ControlBlockSnapshot,
    /// Cumulative malformed carriage records rejected by the raw reader.
    pub rejected_total: u64,
}

/// Source boundary used by the persistence worker.
pub trait OpenOtDocumentSource {
    /// Reads one available carriage batch without blocking the PLC scan thread.
    fn poll(&mut self) -> Result<OpenOtSourcePoll, PersistenceError>;
}

/// Mutable worker counters suitable for projection into runtime status.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct OpenOtPersistenceWorkerStatus {
    /// Canonical documents prepared from source records and loss accounting.
    pub documents_read: u64,
    /// Documents newly inserted by durable sink transactions.
    pub documents_committed: u64,
    /// Byte-identical documents accepted as idempotent duplicates.
    pub documents_duplicated: u64,
    /// Documents durable in a required local spool but not acknowledged remotely.
    pub remote_pending: u64,
    /// Descriptive public read-model rows newly committed by this worker.
    pub projection_rows_committed: u64,
    /// Future events committed with fields retained as unclassified.
    pub unclassified_event_count: u64,
    /// Delivery parts confirmed remotely during this worker lifetime.
    pub reconciled_part_count: u64,
    /// Delivery parts still awaiting remote confirmation.
    pub pending_part_count: u64,
    /// Malformed carriage records rejected by the source reader.
    pub rejected: u64,
    /// Placeholder documents emitted because resolution correctly failed closed.
    pub unresolved: u64,
    /// Distinct loss documents emitted by the converter.
    pub loss_range_count: u64,
    /// Source records represented by emitted loss documents.
    pub lost_record_count: u64,
    /// Last cursor durably acknowledged by the selected sink.
    pub cursor_abs: u64,
    /// Producer head observed by the most recent source poll.
    pub head_abs: u64,
}

/// One source, resolver, and sink composed without scan-thread ownership.
pub struct OpenOtPersistenceWorker<S, D> {
    source: S,
    consumer: OpenOtPersistenceConsumer,
    sink: D,
    status: OpenOtPersistenceWorkerStatus,
    pending: Option<super::PersistenceBatch>,
}

impl<S, D> OpenOtPersistenceWorker<S, D>
where
    S: OpenOtDocumentSource,
    D: DocumentSink,
{
    /// Composes source, resolver, and durable sink without starting a thread.
    pub fn new(source: S, consumer: OpenOtPersistenceConsumer, sink: D) -> Self {
        Self {
            source,
            consumer,
            sink,
            status: OpenOtPersistenceWorkerStatus::default(),
            pending: None,
        }
    }

    /// Polls, resolves, and durably commits at most one source batch.
    pub fn run_once(
        &mut self,
        receive_time_ns: u64,
    ) -> Result<Option<CommitOutcome>, PersistenceError> {
        if self.pending.is_some() {
            return self.commit_pending().map(Some);
        }
        let poll = self.source.poll()?;
        let durable_cursor = self
            .sink
            .load_checkpoint(poll.snapshot.buffer_id, poll.snapshot.run_id)?
            .map_or(0, |checkpoint| checkpoint.cursor_abs);
        let prepared = self.consumer.prepare_batch(
            &poll.batch,
            &poll.snapshot,
            receive_time_ns,
            durable_cursor,
        )?;
        self.status.documents_read = self
            .status
            .documents_read
            .saturating_add(prepared.documents.len() as u64);
        self.status.unresolved = self.status.unresolved.saturating_add(
            prepared
                .documents
                .iter()
                .filter(|document| matches!(document, open_ot_document::Document::Placeholder(_)))
                .count() as u64,
        );
        for document in &prepared.documents {
            if let open_ot_document::Document::Loss(loss) = document {
                self.status.loss_range_count = self.status.loss_range_count.saturating_add(1);
                self.status.lost_record_count =
                    self.status.lost_record_count.saturating_add(loss.count);
            }
        }
        self.status.head_abs = poll.snapshot.head_abs;
        self.status.rejected = poll.rejected_total;

        if prepared.documents.is_empty() && prepared.checkpoint.cursor_abs <= durable_cursor {
            self.status.cursor_abs = durable_cursor;
            let maintenance = self.sink.maintenance_status()?;
            self.status.remote_pending = maintenance.remote_pending as u64;
            self.status.reconciled_part_count = self
                .status
                .reconciled_part_count
                .saturating_add(maintenance.reconciled_parts as u64);
            self.status.pending_part_count = maintenance.pending_parts as u64;
            return Ok(None);
        }
        self.pending = Some(prepared);
        self.commit_pending().map(Some)
    }

    fn commit_pending(&mut self) -> Result<CommitOutcome, PersistenceError> {
        let outcome = self
            .sink
            .commit(self.pending.as_ref().expect("pending batch checked"))?;
        self.pending = None;
        self.status.documents_committed = self
            .status
            .documents_committed
            .saturating_add(outcome.inserted as u64);
        self.status.documents_duplicated = self
            .status
            .documents_duplicated
            .saturating_add(outcome.duplicated as u64);
        self.status.remote_pending = outcome.remote_pending as u64;
        self.status.projection_rows_committed = self
            .status
            .projection_rows_committed
            .saturating_add(outcome.projection_rows_committed as u64);
        self.status.unclassified_event_count = self
            .status
            .unclassified_event_count
            .saturating_add(outcome.unclassified_events as u64);
        self.status.pending_part_count = outcome.pending_parts as u64;
        self.status.cursor_abs = outcome.checkpoint.cursor_abs;
        Ok(outcome)
    }

    #[must_use]
    /// Returns the latest worker-owned counters.
    pub fn status(&self) -> &OpenOtPersistenceWorkerStatus {
        &self.status
    }
}

#[cfg(test)]
mod tests {
    use open_ot_carriage::registry::EVENT_HEARTBEAT;
    use open_ot_carriage::ring::{ReadRecord, DEFAULT_BUFFER_ID};
    use open_ot_carriage::wire::Record;
    use open_ot_definition::{compute_content_hash, sample_definition};

    use super::*;
    use crate::openot_persistence::contracts::InMemoryDocumentSink;

    struct OnePoll(Option<OpenOtSourcePoll>);

    impl OpenOtDocumentSource for OnePoll {
        fn poll(&mut self) -> Result<OpenOtSourcePoll, PersistenceError> {
            self.0.take().ok_or_else(|| {
                PersistenceError::Commit("test source polled more than once".to_string())
            })
        }
    }

    #[test]
    fn worker_loads_checkpoint_resolves_and_commits_one_source_poll() {
        let definition = sample_definition();
        let hash = compute_content_hash(&definition).expect("hash sample definition");
        let poll = OpenOtSourcePoll {
            batch: ReadBatch {
                records: vec![ReadRecord {
                    start_abs: 0,
                    end_abs: 64,
                    record: Record::new(11, 1, 0, 1, EVENT_HEARTBEAT),
                }],
                next_abs: 64,
                lapped: false,
            },
            snapshot: ControlBlockSnapshot {
                version: 2,
                caps: 0,
                buffer_id: DEFAULT_BUFFER_ID,
                buffer_bytes: 4096,
                head_abs: 64,
                oldest_abs: 0,
                lost_count: 0,
                run_id: 1,
                epoch_id: 1,
                epoch_first_abs: 0,
                definition_hash: hash.carriage_hash,
                prev_definition_hash: [0; 8],
            },
            rejected_total: 0,
        };
        let consumer = OpenOtPersistenceConsumer::new(definition, None).expect("consumer");
        let mut worker = OpenOtPersistenceWorker::new(
            OnePoll(Some(poll)),
            consumer,
            InMemoryDocumentSink::new(),
        );

        let committed = worker
            .run_once(99)
            .expect("worker poll")
            .expect("non-empty commit");
        assert_eq!(committed.inserted, 1);
        assert_eq!(committed.checkpoint.cursor_abs, 64);
        assert_eq!(
            worker.status(),
            &OpenOtPersistenceWorkerStatus {
                documents_read: 1,
                documents_committed: 1,
                documents_duplicated: 0,
                remote_pending: 0,
                projection_rows_committed: 0,
                unclassified_event_count: 0,
                reconciled_part_count: 0,
                pending_part_count: 0,
                rejected: 0,
                unresolved: 0,
                loss_range_count: 0,
                lost_record_count: 0,
                cursor_abs: 64,
                head_abs: 64,
            }
        );
    }

    #[test]
    fn worker_ignores_checkpoint_from_recreated_ring_run() {
        let definition = sample_definition();
        let hash = compute_content_hash(&definition).expect("hash sample definition");
        let poll = OpenOtSourcePoll {
            batch: ReadBatch {
                records: vec![ReadRecord {
                    start_abs: 0,
                    end_abs: 64,
                    record: Record::new(11, 1, 0, 1, EVENT_HEARTBEAT),
                }],
                next_abs: 64,
                lapped: false,
            },
            snapshot: ControlBlockSnapshot {
                version: 2,
                caps: 0,
                buffer_id: DEFAULT_BUFFER_ID,
                buffer_bytes: 4096,
                head_abs: 64,
                oldest_abs: 0,
                lost_count: 0,
                run_id: 2,
                epoch_id: 1,
                epoch_first_abs: 0,
                definition_hash: hash.carriage_hash,
                prev_definition_hash: [0; 8],
            },
            rejected_total: 0,
        };
        let consumer = OpenOtPersistenceConsumer::new(definition, None).expect("consumer");
        let mut sink = InMemoryDocumentSink::new();
        sink.commit(&super::super::PersistenceBatch {
            documents: Vec::new(),
            checkpoint: super::super::PersistenceCheckpoint {
                buffer_id: DEFAULT_BUFFER_ID,
                run_id: 1,
                cursor_abs: 200,
            },
        })
        .expect("seed prior ring run checkpoint");
        let mut worker = OpenOtPersistenceWorker::new(OnePoll(Some(poll)), consumer, sink);

        let committed = worker
            .run_once(99)
            .expect("new ring run must be readable")
            .expect("new ring record must be committed");

        assert_eq!(committed.inserted, 1);
        assert_eq!(committed.checkpoint.run_id, 2);
        assert_eq!(committed.checkpoint.cursor_abs, 64);
        assert_eq!(worker.status().cursor_abs, 64);
        assert_eq!(worker.status().head_abs, 64);
    }

    #[test]
    fn worker_exposes_documents_awaiting_required_remote_delivery() {
        let definition = sample_definition();
        let hash = compute_content_hash(&definition).expect("hash sample definition");
        let poll = OpenOtSourcePoll {
            batch: ReadBatch {
                records: vec![ReadRecord {
                    start_abs: 0,
                    end_abs: 64,
                    record: Record::new(11, 1, 0, 1, EVENT_HEARTBEAT),
                }],
                next_abs: 64,
                lapped: false,
            },
            snapshot: ControlBlockSnapshot {
                version: 2,
                caps: 0,
                buffer_id: DEFAULT_BUFFER_ID,
                buffer_bytes: 4096,
                head_abs: 64,
                oldest_abs: 0,
                lost_count: 0,
                run_id: 1,
                epoch_id: 1,
                epoch_first_abs: 0,
                definition_hash: hash.carriage_hash,
                prev_definition_hash: [0; 8],
            },
            rejected_total: 0,
        };
        let consumer = OpenOtPersistenceConsumer::new(definition, None).expect("consumer");
        let mut sink = InMemoryDocumentSink::new();
        sink.set_remote_pending(1);
        let mut worker = OpenOtPersistenceWorker::new(OnePoll(Some(poll)), consumer, sink);

        worker.run_once(22).expect("durable local acceptance");

        assert_eq!(worker.status().remote_pending, 1);
    }

    #[test]
    fn idle_worker_runs_sink_maintenance_and_exposes_remote_backlog() {
        let definition = sample_definition();
        let hash = compute_content_hash(&definition).expect("hash sample definition");
        let poll = OpenOtSourcePoll {
            batch: ReadBatch {
                records: vec![],
                next_abs: 0,
                lapped: false,
            },
            snapshot: ControlBlockSnapshot {
                version: 2,
                caps: 0,
                buffer_id: DEFAULT_BUFFER_ID,
                buffer_bytes: 4096,
                head_abs: 0,
                oldest_abs: 0,
                lost_count: 0,
                run_id: 1,
                epoch_id: 1,
                epoch_first_abs: 0,
                definition_hash: hash.carriage_hash,
                prev_definition_hash: [0; 8],
            },
            rejected_total: 0,
        };
        let consumer = OpenOtPersistenceConsumer::new(definition, None).expect("consumer");
        let mut sink = InMemoryDocumentSink::new();
        sink.set_remote_pending(2);
        let mut worker = OpenOtPersistenceWorker::new(OnePoll(Some(poll)), consumer, sink);

        assert_eq!(worker.run_once(22).expect("idle maintenance"), None);

        assert_eq!(worker.status().remote_pending, 2);
    }

    #[test]
    fn worker_retries_failed_prepared_batch_without_polling_past_it() {
        let definition = sample_definition();
        let hash = compute_content_hash(&definition).expect("hash sample definition");
        let poll = OpenOtSourcePoll {
            batch: ReadBatch {
                records: vec![ReadRecord {
                    start_abs: 0,
                    end_abs: 64,
                    record: Record::new(11, 1, 0, 1, EVENT_HEARTBEAT),
                }],
                next_abs: 64,
                lapped: false,
            },
            snapshot: ControlBlockSnapshot {
                version: 2,
                caps: 0,
                buffer_id: DEFAULT_BUFFER_ID,
                buffer_bytes: 4096,
                head_abs: 64,
                oldest_abs: 0,
                lost_count: 0,
                run_id: 1,
                epoch_id: 1,
                epoch_first_abs: 0,
                definition_hash: hash.carriage_hash,
                prev_definition_hash: [0; 8],
            },
            rejected_total: 3,
        };
        let consumer = OpenOtPersistenceConsumer::new(definition, None).expect("consumer");
        let mut sink = InMemoryDocumentSink::new();
        sink.fail_next_commit();
        let mut worker = OpenOtPersistenceWorker::new(OnePoll(Some(poll)), consumer, sink);

        assert!(matches!(
            worker.run_once(99),
            Err(PersistenceError::Commit(_))
        ));
        let committed = worker
            .run_once(100)
            .expect("retry prepared batch")
            .expect("retry commits");

        assert_eq!(committed.inserted, 1);
        assert_eq!(committed.checkpoint.cursor_abs, 64);
        assert_eq!(worker.status().rejected, 3);
    }
}
