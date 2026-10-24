//! A compiled program's cycle in an online change (specification 44 §12.3; specification 34
//! §11.3): the candidate is the new program over a process I/O that claims no line, with the
//! running cycle's settings, since a change brings no record. Once its module is where the CPU runs
//! it, it takes the running program's values and task schedule, and at the commit the running
//! cycle's lines and checkpoint store.

use alloc::vec::Vec;

use trust_runtime_core::error::RuntimeError;
use trust_runtime_core::io_image::write_flat_image;
use trust_runtime_core::native::program::{ModuleCode, NativeProgram};
use trust_runtime_core::ports::MonotonicClock;
use trust_runtime_core::vm::CarriedValueEvent;

use super::CompiledCycle;
use crate::runner::engine::ProcessIo;
use crate::runner::hold::CompiledHold;
use crate::runner::retain::CycleRetain;

impl<'i, C, I, K, H, R> CompiledCycle<'i, C, I, K, H, R>
where
    C: MonotonicClock + Clone,
    I: ProcessIo,
    K: ModuleCode,
    H: CompiledHold,
    R: CycleRetain,
{
    /// The candidate of an online change: `program`, built from the new image and not started,
    /// over the process I/O `io`, with the settings, the clock, the allocator count, the logical
    /// time and the generation of `running`. It takes the running values with
    /// [`Self::take_values`].
    #[must_use]
    pub fn candidate(program: NativeProgram<'i, K>, io: I, running: &Self) -> Self {
        let mut cycle = Self::built(program, io, running.settings.clone(), running.clock.clone());
        cycle.operations = running.operations;
        cycle.now = running.now;
        cycle.generation = running.generation;
        cycle
    }

    /// The candidate takes the values of `running` (specification 44 §12.2): its cold-init entry
    /// runs, the running program's values are carried into its areas, its tasks continue the
    /// running program's schedule from the logical time of the last scan, and it installs the next
    /// generation (specification 34 UPDATE-04). Returns the events of the variables that did not
    /// go on with the running value as it was.
    ///
    /// # Errors
    ///
    /// [`RuntimeError::Overflow`] when the generations are used up, the fault of the cold-init
    /// entry, or the error of a table that does not read; `running` is as it was.
    pub fn take_values<D: ModuleCode>(
        &mut self,
        running: &CompiledCycle<'_, C, I, D, H, R>,
    ) -> Result<Vec<CarriedValueEvent>, RuntimeError> {
        let generation = running.generation.next().ok_or(RuntimeError::Overflow)?;
        let events = self.program.change_from(&running.program, running.now)?;
        self.program.copy_images(&mut self.images);
        self.generation = generation;
        Ok(events)
    }

    /// Whether the running program's safe state is a valid write of the candidate's output image,
    /// as a stop or a fault would write it (specification 34 §11.3, the lines, check 2).
    ///
    /// # Errors
    ///
    /// The error of the first entry the candidate's output image does not take.
    pub fn check_safe_outputs(&self) -> Result<(), RuntimeError> {
        let mut outputs = self.program.outputs.clone();
        for (address, value) in &self.settings.safe_outputs {
            write_flat_image(&mut outputs, address, value.clone())?;
        }
        Ok(())
    }

    /// Gives the running cycle's checkpoint store room for a capture of `bytes`, keeping the room
    /// it has, so that the candidate's first capture grows nothing (specification 34 MEM-09). A
    /// cycle without a checkpoint store needs none.
    ///
    /// # Errors
    ///
    /// The [`RuntimeError`] of a heap without that room.
    pub fn reserve_retain(&mut self, bytes: usize) -> Result<(), RuntimeError> {
        match self.retain.as_mut() {
            Some(retain) => retain.reserve(bytes),
            None => Ok(()),
        }
    }

    /// The commit of an online change (specification 34 §11.3): the running cycle's process I/O,
    /// which holds the claimed lines with the levels their outputs hold, and its checkpoint store
    /// change places with the candidate's. No line is released or claimed and nothing is written.
    pub fn hand_over_to(&mut self, candidate: &mut Self) {
        core::mem::swap(&mut self.io, &mut candidate.io);
        core::mem::swap(&mut self.retain, &mut candidate.retain);
    }
}
