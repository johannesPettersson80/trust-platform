//! The program of an online change takes over from the running one (specification 44 §12.3, steps
//! 4 and 5). It lives here, outside `native::program`, so that this code, which runs once at a
//! boundary, is no part of the scan path that the C6 keeps in RAM (`ld/rwtext_hook_native.x`).

use alloc::vec::Vec;

use super::{lifecycle, ChangeReport, Tables};
use crate::error::RuntimeError;
use crate::native::program::{ModuleCode, NativeProgram};
use crate::value::Duration;

impl<C: ModuleCode> NativeProgram<'_, C> {
    /// Makes this program, built from its image with its module where the CPU runs it, the
    /// replacement of `old` at the logical time `now`. Under the CODESYS profiles, whose images
    /// have change entries, `old`'s change-exit entries of the variables the change copies or drops
    /// run first; then this program's areas take their starting memory and its change-start and
    /// change-init entries of the copied and the added variables run; an image without change
    /// entries runs its cold-init entry instead (§6.7). Then the running program's values are
    /// carried into its areas (§12.2), its change-reinit entries of the copied variables run, the
    /// images the drivers fill and drive keep the bytes both programs have, each task takes the
    /// schedule of the running program's task of the same name, and the variables located at `%M`
    /// are published (§12.3, step 4). Returns the events of the variables that did not go on with
    /// the running value as it was, and the faults of the change entries, which do not stop the
    /// change (specification 10 §4.8).
    ///
    /// # Errors
    ///
    /// The refusal of a program built with other conversion rules than `old`, which a running
    /// runtime keeps (specification 07 §2.10), the fault of the cold-init entry, or the error of a
    /// table of either image that does not read; this program is dropped then.
    pub fn change_from<D: ModuleCode>(
        &mut self,
        old: &mut NativeProgram<'_, D>,
        now: Duration,
    ) -> Result<ChangeReport, RuntimeError> {
        crate::native::image::same_conversion_rules(old.image().header(), self.image().header())?;
        let calls = lifecycle::calls(&Tables::of(old.image())?, &Tables::of(self.image())?)?;
        let mut faults = Vec::new();
        let mut record = |result: Result<(), RuntimeError>| {
            if let Err(fault) = result {
                faults.push(fault);
            }
        };
        for entry in calls.exits {
            record(old.call(entry, u32::MAX, now, None));
        }
        match calls.start {
            Some(start) => {
                self.start_replacement_memory()?;
                record(self.call(start, u32::MAX, now, None));
                for entry in calls.inits {
                    record(self.call(entry, u32::MAX, now, None));
                }
            }
            None => self.start_as_replacement(now)?,
        }
        let events = super::carry(old, self)?;
        for entry in calls.reinits {
            record(self.call(entry, u32::MAX, now, None));
        }
        self.take_over(old, now);
        Ok(ChangeReport { events, faults })
    }
}
