//! Retain storage integration with the runtime.

#![allow(missing_docs)]

use crate::error::RuntimeError;
use crate::RetainSnapshot;

use super::core::Runtime;

impl Runtime {
    /// Load retained values from the configured store.
    pub fn load_retain_store(&mut self) -> Result<(), RuntimeError> {
        let snapshot = self.retain.load()?;
        self.apply_retain_snapshot(&snapshot)?;
        Ok(())
    }

    /// Persist retained values to the configured store.
    pub fn save_retain_store(&mut self) -> Result<(), RuntimeError> {
        let snapshot = RetainSnapshot::from_runtime(self);
        self.retain.save_snapshot(snapshot, self.current_time)
    }

    /// Persist retained values if the save interval has elapsed.
    pub fn maybe_save_retain_store(&mut self) -> Result<(), RuntimeError> {
        if !self.retain.should_save(self.current_time) {
            return Ok(());
        }
        let snapshot = RetainSnapshot::from_runtime(self);
        self.retain.save_snapshot(snapshot, self.current_time)
    }
}
