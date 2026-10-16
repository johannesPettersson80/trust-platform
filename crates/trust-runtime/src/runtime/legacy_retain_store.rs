//! Private historical fixture behavior; excluded from every product build.
//! Retain storage integration with the runtime.

#![allow(missing_docs)]

use crate::error::RuntimeError;

use super::core::Runtime;

impl Runtime {
    /// Persist retained values if the save interval has elapsed.
    pub fn maybe_save_retain_store(&mut self) -> Result<(), RuntimeError> {
        if !self.retain.should_save(self.current_time) {
            return Ok(());
        }
        let snapshot = self.retain_snapshot();
        self.retain.save_snapshot(snapshot, self.current_time)
    }
}
