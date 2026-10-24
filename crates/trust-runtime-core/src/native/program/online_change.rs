//! What an online change asks of the program that replaces the running one (specification 44
//! §12.3): its own areas' starting memory, its cold-init entry, and the images and task schedule it
//! takes over. The change itself, which carries the running program's values, is
//! [`NativeProgram::change_from`] in `native::change`, outside the scan path that the C6 keeps in
//! RAM (`ld/rwtext_hook_native.x`), since it runs once, at a boundary.

use super::{ModuleCode, NativeProgram};
use crate::error::RuntimeError;
use crate::task::TaskState;
use crate::value::Duration;

impl<C: ModuleCode> NativeProgram<'_, C> {
    /// The program's own areas take their starting memory and its cold-init entry runs at `now`,
    /// as at a cold restart (§6.7); its process images keep what they hold.
    ///
    /// # Errors
    ///
    /// The fault of the cold-init entry, or the error of an area table that does not read.
    pub(crate) fn start_as_replacement(&mut self, now: Duration) -> Result<(), RuntimeError> {
        self.start_memory(|id| !super::process_image(id))?;
        if let Some(entry) = self.cold {
            self.call(entry, u32::MAX, now, None)?;
        }
        Ok(())
    }

    /// Takes over from `old` at the logical time `now`: the images the drivers fill and drive keep
    /// the bytes both programs have, each task takes the schedule of `old`'s task of the same name,
    /// compared without regard to case, while a new task counts its first activation from `now`
    /// (decision OC-2), and the variables located at `%M` are published into the marker image, as
    /// a cold start publishes them.
    pub(crate) fn take_over<D: ModuleCode>(&mut self, old: &NativeProgram<'_, D>, now: Duration) {
        keep_shared(&mut self.inputs, &old.inputs);
        keep_shared(&mut self.outputs, &old.outputs);
        for task in &mut self.tasks {
            task.state = old
                .tasks
                .iter()
                .find(|running| running.name.eq_ignore_ascii_case(&task.name))
                .map_or_else(|| TaskState::new(now), |running| running.state.clone());
        }
        self.publish_markers();
    }
}

/// Copies the bytes `old` and `new` both have into `new`.
fn keep_shared(new: &mut [u8], old: &[u8]) {
    let shared = new.len().min(old.len());
    new[..shared].copy_from_slice(&old[..shared]);
}
