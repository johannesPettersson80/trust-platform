//! Shared-memory carriage source for the host persistence worker.

#[cfg(unix)]
mod imp {
    use std::collections::VecDeque;
    use std::path::Path;

    use open_ot_carriage::concurrent::{ConcurrentRawConsumer, ConcurrentStore};
    use open_ot_carriage::control::ControlBlockSnapshot;
    use open_ot_carriage::ring::{ReadBatch, ReadRecord};
    use open_ot_shm::SharedConcurrentStore;

    use super::super::{OpenOtDocumentSource, OpenOtSourcePoll, PersistenceError};

    /// Raw OpenOT shared-memory reader used only by the host worker.
    #[derive(Debug)]
    pub struct SharedMemoryOpenOtSource {
        consumer: ConcurrentRawConsumer<SharedConcurrentStore>,
        batch_size: usize,
        queue_capacity: usize,
        pending: VecDeque<ReadRecord>,
        pending_snapshot: Option<ControlBlockSnapshot>,
    }

    /// Non-consuming view of the producer control block used while a durable
    /// sink is unavailable.
    #[derive(Debug)]
    pub(crate) struct SharedMemoryOpenOtSourceObserver {
        store: SharedConcurrentStore,
    }

    impl SharedMemoryOpenOtSourceObserver {
        pub(crate) fn open(path: &Path) -> Result<Self, PersistenceError> {
            let store = SharedConcurrentStore::open_existing(path).map_err(|error| {
                PersistenceError::Commit(format!(
                    "open OpenOT shared-memory observer '{}': {error}",
                    path.display()
                ))
            })?;
            Ok(Self { store })
        }

        pub(crate) fn snapshot(&self) -> Result<ControlBlockSnapshot, PersistenceError> {
            self.store.read_control_snapshot().map_err(|error| {
                PersistenceError::Commit(format!(
                    "read OpenOT shared-memory control snapshot: {error:?}"
                ))
            })
        }
    }

    impl SharedMemoryOpenOtSource {
        /// Opens the existing carriage created by the runtime publisher.
        pub fn open(path: &Path) -> Result<Self, PersistenceError> {
            let store = SharedConcurrentStore::open_existing(path).map_err(|error| {
                PersistenceError::Commit(format!(
                    "open OpenOT shared-memory source '{}': {error}",
                    path.display()
                ))
            })?;
            Ok(Self {
                consumer: ConcurrentRawConsumer::with_store(store),
                batch_size: usize::MAX,
                queue_capacity: usize::MAX,
                pending: VecDeque::new(),
                pending_snapshot: None,
            })
        }

        /// Opens the source with configured transaction and in-memory queue bounds.
        pub fn open_with_limits(
            path: &Path,
            batch_size: usize,
            queue_capacity: usize,
        ) -> Result<Self, PersistenceError> {
            let mut source = Self::open(path)?;
            source.batch_size = batch_size;
            source.queue_capacity = queue_capacity;
            Ok(source)
        }

        fn take_pending(&mut self) -> Option<OpenOtSourcePoll> {
            if self.pending.is_empty() {
                return None;
            }
            let count = self.batch_size.min(self.pending.len());
            let records: Vec<_> = self.pending.drain(..count).collect();
            let next_abs = records.last().map_or(0, |record| record.end_abs);
            let snapshot = self
                .pending_snapshot
                .clone()
                .expect("pending records retain their source snapshot");
            if self.pending.is_empty() {
                self.pending_snapshot = None;
            }
            Some(OpenOtSourcePoll {
                batch: ReadBatch {
                    records,
                    next_abs,
                    lapped: false,
                },
                snapshot,
                rejected_total: self.consumer.rejected_records(),
            })
        }
    }

    impl OpenOtDocumentSource for SharedMemoryOpenOtSource {
        fn poll(&mut self) -> Result<OpenOtSourcePoll, PersistenceError> {
            if let Some(poll) = self.take_pending() {
                return Ok(poll);
            }
            let mut batch = self.consumer.poll().map_err(|error| {
                PersistenceError::Commit(format!("poll OpenOT shared-memory source: {error:?}"))
            })?;
            let snapshot = self
                .consumer
                .store()
                .read_control_snapshot()
                .map_err(|error| {
                    PersistenceError::Commit(format!(
                        "read OpenOT shared-memory control snapshot: {error:?}"
                    ))
                })?;
            if batch.records.len() > self.queue_capacity {
                return Err(PersistenceError::InvalidConfig(format!(
                    "OpenOT source poll contained {} records, exceeding queue_capacity {}; increase the configured bound and restart before the ring overwrites the uncommitted cursor",
                    batch.records.len(), self.queue_capacity
                )));
            }
            if batch.records.len() > self.batch_size {
                let remainder = batch.records.split_off(self.batch_size);
                self.pending = remainder.into();
                self.pending_snapshot = Some(snapshot.clone());
                batch.next_abs = batch
                    .records
                    .last()
                    .map_or(batch.next_abs, |record| record.end_abs);
            }
            Ok(OpenOtSourcePoll {
                batch,
                snapshot,
                rejected_total: self.consumer.rejected_records(),
            })
        }
    }
}

#[cfg(unix)]
pub use imp::SharedMemoryOpenOtSource;
#[cfg(unix)]
pub(crate) use imp::SharedMemoryOpenOtSourceObserver;

#[cfg(all(test, unix))]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use open_ot_carriage::registry::EVENT_HEARTBEAT;
    use open_ot_carriage::wire::Record;
    use open_ot_shm::SharedRecordPublisher;

    use super::*;
    use crate::openot_persistence::OpenOtDocumentSource;

    #[test]
    fn shared_memory_source_returns_published_records_and_control_snapshot() {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "trust-openot-persistence-source-{}-{stamp}.shm",
            std::process::id()
        ));
        let mut publisher = SharedRecordPublisher::create(&path, 4096).expect("publisher");
        publisher
            .append_record(&Record::new(11, 1, 0, 7, EVENT_HEARTBEAT))
            .expect("publish record");
        let mut source = SharedMemoryOpenOtSource::open(&path).expect("open source");

        let poll = source.poll().expect("poll source");

        assert_eq!(poll.batch.records.len(), 1);
        assert_eq!(poll.batch.records[0].record.source_id, 7);
        assert_eq!(poll.batch.next_abs, poll.snapshot.head_abs);
        assert_eq!(poll.snapshot.buffer_bytes, 4096);
        std::fs::remove_file(path).ok();
    }

    #[test]
    fn shared_memory_source_returns_no_more_than_the_configured_batch_size() {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "trust-openot-persistence-batch-source-{}-{stamp}.shm",
            std::process::id()
        ));
        let mut publisher = SharedRecordPublisher::create(&path, 4096).expect("publisher");
        for sequence in 1..=3 {
            publisher
                .append_record(&Record::new(11, sequence, 0, 7, EVENT_HEARTBEAT))
                .expect("publish record");
        }
        let mut source =
            SharedMemoryOpenOtSource::open_with_limits(&path, 2, 4).expect("open source");

        let first = source.poll().expect("first bounded poll");
        let second = source.poll().expect("second bounded poll");

        assert_eq!(first.batch.records.len(), 2);
        assert_eq!(second.batch.records.len(), 1);
        assert_eq!(first.batch.next_abs, first.batch.records[1].end_abs);
        assert_eq!(second.batch.next_abs, second.snapshot.head_abs);
        std::fs::remove_file(path).ok();
    }
}
