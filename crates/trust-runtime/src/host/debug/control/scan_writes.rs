//! Checked writes at a scan boundary (a task's start or end): Control Studio's task-boundary
//! commits write through the runtime's own checks, never into a copy of the program's storage.

use crate::memory::VariableStorage;
use crate::value::{Value, ValueRef};

/// The values at a scan boundary and the runtime's checked write.
pub(crate) trait ScanWrites {
    /// The program's values now.
    fn storage(&self) -> &VariableStorage;
    /// Writes one value through the runtime's checks; false when the write is refused.
    fn write(&mut self, reference: &ValueRef, value: Value) -> bool;
}

/// Plain storage, for tests and detached fixtures.
impl ScanWrites for VariableStorage {
    fn storage(&self) -> &VariableStorage {
        self
    }

    fn write(&mut self, reference: &ValueRef, value: Value) -> bool {
        self.write_by_ref_ref(reference, value)
    }
}
