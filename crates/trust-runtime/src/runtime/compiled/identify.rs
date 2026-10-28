//! Identify on a computer's compiled runtime (specification 11, "Identify"): the runtime's side of
//! the [`IdentifyHandle`] that it shares with its control state.
//!
//! The scan ticks the blink where the process I/O hands the outputs to the drivers
//! (`process_io.rs`). In a pass in which no scan runs, the scheduler's paused, stopped and faulted
//! waits call [`CompiledRuntime::drive_identify_while_idle`], which ticks the blink and hands the
//! drivers the committed output image again whenever the identify bit changes. A stop or a fault
//! does not end identify; only an output that `[[io.safe_state]]` gives a safe value keeps that
//! value while the safe state is in force.

use super::CompiledRuntime;
use crate::error::RuntimeError;
use crate::io::{check_identify_replacement, safe_outputs_hold_bit, IdentifyHandle, IdleOutput};
use trust_runtime_core::io_image::PreparedIoBinding;

impl CompiledRuntime {
    /// Gives the runtime the identify state it shares with the control state. The program's start
    /// sets the output of `[runtime.identify]` into the handle (specification 11, "Identify").
    pub fn set_identify_handle(&mut self, handle: IdentifyHandle) {
        self.cycle.process_io_mut().set_identify(handle.clone());
        self.identify = handle;
    }

    /// Applies the blink in a pass in which no scan hands out the outputs: the resource is paused,
    /// stopped or faulted. The committed output image is handed to the drivers again when the bit
    /// of an image output changes; a GPIO line is driven by the tick itself.
    ///
    /// While the safe state of a stop or a fault is in force, an output that `[[io.safe_state]]`
    /// names keeps its safe value: the run goes on, but the bit is not set.
    pub(crate) fn drive_identify_while_idle(&mut self) {
        let safe_state_in_force = self.safe_state_delivered;
        let output = {
            let safe_outputs = &self.cycle.settings().safe_outputs;
            self.identify.idle_publication(|address| {
                safe_state_in_force && safe_outputs_hold_bit(safe_outputs, address)
            })
        };
        if let IdleOutput::Hand(bit) = output {
            let image = self.cycle.program().outputs.clone();
            self.cycle.process_io_mut().republish_outputs(&image, bit);
        }
    }

    /// Whether `identify` runs now.
    pub(crate) fn identify_is_running(&self) -> bool {
        self.identify.is_identifying()
    }

    /// Ends `identify` when the process of the resource ends (an exit request): drives the output
    /// off and, if the drivers hold an output image with the identify bit, hands them the
    /// committed image without it.
    pub(crate) fn end_identify(&mut self) {
        if self.identify.end() {
            let image = self.cycle.program().outputs.clone();
            self.cycle.process_io_mut().republish_outputs(&image, None);
        }
    }

    /// Records that the safe state of a stop or a fault was handed to the drivers: the image they
    /// got has no identify bit, and an output that the safe state names keeps its safe value until
    /// the next scan.
    pub(super) fn note_safe_state_delivered(&mut self) {
        if !self.cycle.settings().safe_outputs.is_empty() {
            self.safe_state_delivered = true;
            self.identify.image_handed_out_without_bit();
        }
    }

    /// Refuses a changed program whose `bindings` use the output image bit that identify drives,
    /// or whose output image of `output_image_len` bytes has no such bit, as a project that does
    /// is refused at load (specification 11, "Identify"). The running program stays.
    pub(super) fn refuse_replacement_using_identify_output(
        &self,
        bindings: &[PreparedIoBinding],
        output_image_len: usize,
    ) -> Result<(), RuntimeError> {
        let Some(address) = self.identify.image_address() else {
            return Ok(());
        };
        let addresses: Vec<_> = bindings
            .iter()
            .map(|binding| binding.address.clone())
            .collect();
        check_identify_replacement(&address, &addresses, output_image_len)
    }
}
