//! Explicit compatibility input; changing it invalidates semantic queries, not source text.
use super::*;
use salsa::Setter;

impl Database {
    /// Select storage layout before compiling or importing an explicitly versioned artifact.
    /// Ordinary source analysis defaults to the uniform eight-byte temporal layout.
    pub fn set_temporal_storage(&mut self, storage: TemporalStorage) {
        if self.temporal_storage == storage {
            return;
        }
        self.temporal_storage = storage;
        let revision = self.source_revision.fetch_add(1, Ordering::Relaxed) + 1;
        self.with_salsa_state(|state| {
            for input in state.sources.values().copied() {
                input.set_temporal_storage(&mut state.db).to(storage);
            }
            state.synced_revision = revision;
        });
    }

    /// Storage layout selected for semantic SIZEOF and relative-address checks.
    #[must_use]
    pub fn temporal_storage(&self) -> TemporalStorage {
        self.temporal_storage
    }
}
