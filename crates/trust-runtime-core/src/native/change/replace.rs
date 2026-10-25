//! The program of an online change takes over from the running one (specification 44 §12.3, steps
//! 4 and 5). It lives here, outside `native::program`, so that this code, which runs once at a
//! boundary, is no part of the scan path that the C6 keeps in RAM (`ld/rwtext_hook_native.x`).

use alloc::vec::Vec;

use crate::error::RuntimeError;
use crate::native::change::CarriedValueEvent;
use crate::native::program::{ModuleCode, NativeProgram};
use crate::value::Duration;

impl<C: ModuleCode> NativeProgram<'_, C> {
    /// Makes this program, built from its image with its module where the CPU runs it, the
    /// replacement of `old` at the logical time `now`: its areas take their starting memory and its
    /// cold-init entry runs (§6.7), the running program's values are carried into its areas
    /// (§12.2), the images the drivers fill and drive keep the bytes both programs have, each task
    /// takes the schedule of the running program's task of the same name, and the variables located
    /// at `%M` are published. Returns the events of the variables that did not go on with the
    /// running value as it was.
    ///
    /// # Errors
    ///
    /// The fault of the cold-init entry, or the error of a table of either image that does not
    /// read. `old` is only read; this program is dropped then.
    pub fn change_from<D: ModuleCode>(
        &mut self,
        old: &NativeProgram<'_, D>,
        now: Duration,
    ) -> Result<Vec<CarriedValueEvent>, RuntimeError> {
        self.start_as_replacement(now)?;
        let events = super::carry(old, self)?;
        self.take_over(old, now);
        Ok(events)
    }
}
