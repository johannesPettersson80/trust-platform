//! Outputs and markers published from their leaves without staging their bytes (checklist P1-6;
//! specification 34 PERF-05).
//!
//! When every binding a publication selects is a copied leaf (`IoLeaf::copy`) at a flat address,
//! the publication reads each leaf's bytes, makes the staged publication's charges and checks in the
//! same order (`staging::publish_staged`), and then writes each leaf's bytes into its window: a
//! refusal still changes no image, and since no program code runs between the reads and the writes,
//! each window takes the bytes the staged publication would have staged. A publication with any
//! other binding, or more of them than [`MOST`], is staged.

use super::output_journal::OutputJournal;
use super::staging::{keep_outputs, write_window, OutputPatch};
use super::*;
use crate::io_address::IoSize;
use crate::vm::engine::cycle_port::CycleHooks;
use crate::vm::prepared::IoLeaf;

/// The most bindings a publication from the leaves holds.
const MOST: usize = 16;

/// One binding of a publication from the leaves: its index and the bytes its leaf holds.
#[derive(Clone, Copy)]
struct Held {
    index: usize,
    bytes: [u8; 8],
}

impl EngineState<'_> {
    /// Publish the bindings that the publication selects (every marker binding, and every output
    /// binding unless `markers_only`) from their leaves, when each is a copied leaf at a flat address
    /// and there are at most [`MOST`]; `None`, having done nothing, otherwise.
    pub(super) fn publish_copied(
        &mut self,
        hooks: &mut dyn CycleHooks,
        markers_only: bool,
    ) -> Option<Result<(), RuntimeError>> {
        let prepared = self.prepared;
        let mut held = [Held {
            index: 0,
            bytes: [0; 8],
        }; MOST];
        let mut count = 0;
        for (index, binding) in prepared.io_bindings.iter().enumerate() {
            let area = binding.address.area;
            if !(area == IoArea::Memory || (area == IoArea::Output && !markers_only)) {
                continue;
            }
            if binding.address.path.len() > 1 {
                return None;
            }
            let (_, bytes) = self.copied_output(index)?;
            *held.get_mut(count)? = Held { index, bytes };
            count += 1;
        }
        Some(self.publish_held(&held[..count], hooks))
    }

    /// Publish `held`, the leaves' bytes of the selected bindings in binding order, with the staged
    /// publication's charges and checks before any image changes.
    fn publish_held(
        &mut self,
        held: &[Held],
        hooks: &mut dyn CycleHooks,
    ) -> Result<(), RuntimeError> {
        let prepared = self.prepared;
        let windows = [
            self.data.images.outputs.len(),
            self.data.images.memory.len(),
        ];
        // The staged patches (`stage_patches`).
        self.charge_allocation_bytes(
            held.len()
                .checked_mul(size_of::<OutputPatch>())
                .ok_or_else(|| RuntimeError::Overflow)?,
        )?;
        for entry in held {
            let (leaf, address) = self.held_binding(entry.index)?;
            // The window of a copied binding (`copy_flat_window`).
            let window = if address.area == IoArea::Output {
                windows[0]
            } else {
                windows[1]
            };
            self.check_image_window(address, window)?;
            let end = address
                .flat_byte_end()?
                .ok_or_else(|| RuntimeError::TypeMismatch)? as usize;
            let width = end - address.byte as usize;
            self.charge_allocation_bytes(width)?;
            if address.size != IoSize::Bit && width != usize::from(leaf.bytes) {
                return Err(RuntimeError::TypeMismatch);
            }
        }
        // The no-op composition retains its original publication checks/cost.
        self.check_entry_deadline()?;
        let mut outputs_before = core::mem::take(&mut self.data.resources.staging.outputs_before);
        let result = self.write_held(held, hooks, &mut outputs_before, prepared);
        outputs_before.clear();
        self.data.resources.staging.outputs_before = outputs_before;
        result
    }

    /// Journal the output windows for the output hooks, write `held` into the images and submit
    /// the outputs, as the staged publication does after its checks.
    fn write_held(
        &mut self,
        held: &[Held],
        hooks: &mut dyn CycleHooks,
        outputs_before: &mut Vec<u8>,
        prepared: &PreparedModule,
    ) -> Result<(), RuntimeError> {
        let mut journal: Option<OutputJournal> = None;
        if hooks.has_output_hooks() {
            let outputs = held
                .iter()
                .filter(|entry| prepared.io_bindings[entry.index].address.area == IoArea::Output)
                .count();
            let mut opened = self.open_output_journal(outputs)?;
            for entry in held {
                let address = &prepared.io_bindings[entry.index].address;
                if address.area == IoArea::Output {
                    let width = flat_width(address)?;
                    self.journal_output_window(&mut opened, address.byte as usize, width)?;
                }
            }
            keep_outputs(outputs_before, &self.data.images.outputs);
            journal = Some(opened);
        }
        for entry in held {
            let address = &prepared.io_bindings[entry.index].address;
            let width = flat_width(address)?;
            if address.size == IoSize::Bit {
                let bit = [u8::from(entry.bytes[0] != 0) << address.bit];
                write_window(
                    &mut self.data.images,
                    address.area,
                    address.byte as usize,
                    &bit,
                    Some(address.bit),
                );
            } else {
                write_window(
                    &mut self.data.images,
                    address.area,
                    address.byte as usize,
                    &entry.bytes[..width],
                    None,
                );
            }
        }
        self.submit_outputs(hooks, journal, outputs_before)
    }

    /// The leaf and the address of binding `index`.
    fn held_binding(&self, index: usize) -> Result<(IoLeaf, &IoAddress), RuntimeError> {
        let prepared = self.prepared;
        let leaf = prepared
            .io_leaves
            .get(index)
            .copied()
            .flatten()
            .ok_or_else(|| RuntimeError::InvalidExecutionState)?;
        let binding = prepared
            .io_bindings
            .get(index)
            .ok_or_else(|| RuntimeError::InvalidExecutionState)?;
        Ok((leaf, &binding.address))
    }
}

/// The bytes of the flat window of `address`, which the checks before found in its image.
fn flat_width(address: &IoAddress) -> Result<usize, RuntimeError> {
    let end = address
        .flat_byte_end()?
        .ok_or_else(|| RuntimeError::TypeMismatch)? as usize;
    Ok(end - address.byte as usize)
}

#[cfg(test)]
mod tests {
    use super::super::super::services;
    use super::*;
    use crate::memory::view::StoreMutate;
    use crate::vm::engine::cycle_port::{CycleAccess, NoopCycleHooks};

    /// Output hooks that take every phase and submit the outputs, or refuse to submit them.
    struct Submitting {
        refuse: bool,
    }

    impl CycleHooks for Submitting {
        fn publish_outputs(&mut self, _access: &CycleAccess<'_, '_>) -> Result<(), RuntimeError> {
            if self.refuse {
                Err(RuntimeError::IoDriver(smol_str::SmolStr::new_static(
                    "refused",
                )))
            } else {
                Ok(())
            }
        }
    }

    /// How a publication ended: its result, the output image and the allocation it charged.
    type Ending = (Result<(), RuntimeError>, alloc::vec::Vec<u8>, usize);

    /// Publish the gpio fixture's `led`, TRUE, with `room` bytes of the allocation allowance left,
    /// from the leaves when `copied` and staged otherwise, to `hooks`.
    fn publish(copied: bool, room: usize, hooks: &mut dyn CycleHooks) -> Ending {
        let prepared = PreparedModule::from_bytes(
            include_bytes!(
                "../../../../../trust-runtime/tests/fixtures/portability/f401/gpio.stbc"
            ),
            crate::vm::PreparationLimits::default(),
        )
        .expect("the fixture prepares");
        let mut data = EngineState::construct_data(&prepared, 0, &services::LOGICAL_ONLY)
            .expect("the fixture constructs");
        let mut state = EngineState::borrow(&mut data, &prepared, &services::LOGICAL_ONLY);
        let plant = crate::vm::engine::test_store::global_instance(&state, "Plant");
        assert!(StoreMutate::write_instance_var(
            &mut state,
            plant,
            "led",
            Value::Bool(true)
        ));
        let limit = prepared.limits.max_construction_bytes;
        state
            .data
            .resources
            .construction_bytes
            .set(limit.saturating_sub(room));
        let result = if copied {
            state
                .publish_copied(hooks, false)
                .expect("the fixture's output is a copied leaf")
        } else {
            let mut staging = core::mem::take(&mut state.data.resources.staging);
            let result = state.publish_staged(&mut staging, hooks, false);
            state.data.resources.staging = staging;
            result
        };
        let used = state.data.resources.construction_bytes.get();
        (result, state.data.images.outputs.clone(), used)
    }

    /// Specification 34 PERF-05 (checklist P1-6): a publication from the leaves ends as the staged
    /// publication ends under every allowance of allocation bytes, with no output hooks, with
    /// output hooks that submit and with hooks that refuse: the same result, the same output image
    /// (the LED's bit set, or the image the publication began with) and the same bytes charged.
    /// Catches a charge skipped, moved or of another size, a window written before a refusal, and
    /// a refused submission whose outputs are not put back.
    #[test]
    fn copied_outputs_end_as_the_staged_publication() {
        for room in 0..=128 {
            let staged = publish(false, room, &mut NoopCycleHooks);
            assert_eq!(
                publish(true, room, &mut NoopCycleHooks),
                staged,
                "room {room}"
            );
            for refuse in [false, true] {
                let staged = publish(false, room, &mut Submitting { refuse });
                let copied = publish(true, room, &mut Submitting { refuse });
                assert_eq!(copied, staged, "room {room}, refuse {refuse}");
            }
        }
        let (result, outputs, _) = publish(true, 128, &mut NoopCycleHooks);
        assert_eq!(result, Ok(()));
        assert_eq!(outputs, [1], "the LED's bit");
        let (result, outputs, _) = publish(true, 128, &mut Submitting { refuse: true });
        assert!(result.is_err());
        assert_eq!(outputs, [0], "a refused submission puts the image back");
    }
}
