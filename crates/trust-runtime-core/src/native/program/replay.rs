//! A replay's scan (specification 48 RU-60): the tasks a recorded cycle ran, at the times it ran
//! them, not the tasks due at the scan's time.

use super::{ModuleCode, NativeProgram, ScanHooks};
use crate::error::RuntimeError;
use crate::value::Duration;

impl<C: ModuleCode> NativeProgram<'_, C> {
    /// Runs one scan of a replay (specification 48 RU-60): the inputs sampled, then exactly the
    /// task activations of a recorded cycle, in their order, each at the logical time it ran at
    /// (`TIME()`, specification 44 §8.2), and the outputs published. The tasks due at those times
    /// play no part, and their schedule stays where it was: a replay runs the tasks the trace ran.
    ///
    /// # Errors
    ///
    /// [`RuntimeError::InvalidExecutionState`] for an activation of a task the program does not
    /// have, and the fault of the program that faulted; the scan ends there and publishes nothing.
    pub fn replay_scan(
        &mut self,
        activations: &[(usize, Duration)],
        hooks: &mut dyn ScanHooks,
    ) -> Result<(), RuntimeError> {
        self.scanning = true;
        let result = self.replay_tasks(activations, hooks);
        self.scanning = false;
        result
    }

    /// The scan of [`Self::replay_scan`], from the input sample to the publication.
    fn replay_tasks(
        &mut self,
        activations: &[(usize, Duration)],
        hooks: &mut dyn ScanHooks,
    ) -> Result<(), RuntimeError> {
        self.check_declared()?;
        self.check_finite()?;
        self.sample();
        self.live = true;
        self.ran = true;
        for &(index, now) in activations {
            if index >= self.tasks.len() {
                return Err(RuntimeError::InvalidExecutionState);
            }
            self.run_task(index, now, hooks)?;
        }
        self.publish()
    }
}
