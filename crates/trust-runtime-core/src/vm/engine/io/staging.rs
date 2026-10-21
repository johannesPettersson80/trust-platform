//! Stage bound values/windows, never the unused storage or process image.
use super::super::cycle_port::{CycleAccess, CycleHooks, CyclePhase};
use super::super::destinations::{Key, SavedDestinations};
use super::output_journal::OutputJournal;
use super::*;
use crate::io_image::PreparedIoBinding;
use crate::memory::view::{StoreAccess, StoreMutate};
use crate::value::ValueRefView;
use crate::vm::prepared::IoLeaf;

/// One staged change of the process image: the bytes or the bit of a flat window, or one
/// hierarchical key. Every check runs when a patch is built, so applying it cannot fail. The
/// scan stages into `ImageStaging`; a binding-only update takes the patches it staged as
/// these owned values. Each staged patch is charged as one of these (spec 34 TIME-08).
#[cfg_attr(not(feature = "std"), allow(dead_code))]
pub(in crate::vm::engine) enum OutputPatch {
    Flat {
        area: IoArea,
        start: usize,
        bytes: Vec<u8>,
        bit: Option<u8>,
    },
    Hierarchical(IoAddressKey, Value),
}

/// Write a staged patch into the images. The window was checked against the length of the image
/// it is applied to when the patch was built.
#[cfg(feature = "std")]
pub(in crate::vm::engine) fn apply_output_patch(images: &mut ProcessImages, patch: OutputPatch) {
    match patch {
        OutputPatch::Hierarchical(key, value) => {
            images.hierarchical.insert(key, value);
        }
        OutputPatch::Flat {
            area,
            start,
            bytes,
            bit,
        } => write_window(images, area, start, &bytes, bit),
    }
}

/// Keep `outputs`, the output image a publication begins with, in `copy`, its reserved storage.
pub(super) fn keep_outputs(copy: &mut Vec<u8>, outputs: &[u8]) {
    copy.clear();
    copy.extend_from_slice(outputs);
}

/// Write one checked flat window, or only its bit, into the output or marker image.
pub(super) fn write_window(
    images: &mut ProcessImages,
    area: IoArea,
    start: usize,
    bytes: &[u8],
    bit: Option<u8>,
) {
    let image = match area {
        IoArea::Output => &mut images.outputs,
        _ => &mut images.memory,
    };
    if let Some(bit) = bit {
        let mask = 1u8 << bit;
        image[start] = (image[start] & !mask) | (bytes[0] & mask);
    } else {
        image[start..start + bytes.len()].copy_from_slice(bytes);
    }
}

/// One sampled input value and the slot its binding's persistent reference names (census
/// S1, S2). The reference's path stays in the prepared module.
pub(in crate::vm::engine) struct StagedInput {
    location: MemoryLocation,
    offset: usize,
    ref_idx: u32,
    value: StagedValue,
}

/// What a sampled input writes: a value, or the text of a byte-string window of the input image,
/// `len` bytes of UTF-8 from byte `start` of `area`, which a text leaf of the flat store takes as
/// it lies, so that sampling builds no value (C1-16). An image holds at most 16 MiB, so both fit
/// 32 bits and the staged input is no larger than with a value.
pub(in crate::vm::engine) enum StagedValue {
    Value(Value),
    Text { area: IoArea, start: u32, len: u32 },
}

impl StagedInput {
    fn path<'p>(&self, prepared: &'p PreparedModule) -> &'p [RefSegment] {
        persistent_path(prepared, self.ref_idx)
    }

    fn view<'p>(
        &self,
        prepared: &'p PreparedModule,
        generation: crate::memory::Generation,
    ) -> ValueRefView<'p> {
        ValueRefView {
            location: self.location,
            offset: self.offset,
            generation,
            path: self.path(prepared),
        }
    }
}

/// The path of a persistent reference; other references have none to stage.
fn persistent_path(prepared: &PreparedModule, ref_idx: u32) -> &[RefSegment] {
    match prepared.vm.refs.get(ref_idx as usize) {
        Some(
            VmRef::Global { path, .. } | VmRef::Retain { path, .. } | VmRef::Instance { path, .. },
        ) => path,
        _ => &[],
    }
}

/// A staged patch whose flat bytes are in the byte buffer of `ImageStaging`.
enum StagedPatch {
    Flat {
        area: IoArea,
        start: usize,
        offset: usize,
        len: usize,
        bit: Option<u8>,
    },
    /// The value of the hierarchical binding at this index of the prepared bindings.
    Hierarchical(usize, Value),
}

/// Process-image staging that construction reserves for the bindings and every scan reuses
/// (spec 34 MEM-09; census S1 and S5 to S7): the sampled inputs, the snapshot of their
/// destinations with its sorting keys, the staged output patches, one encoding window, the
/// bytes of every staged window and the copy of the output image that a publication to output
/// hooks puts back when they refuse it (C1-19f). A scan leaves it empty with its storage kept.
#[derive(Default)]
pub(in crate::vm::engine) struct ImageStaging {
    inputs: Vec<StagedInput>,
    keys: Vec<Key>,
    /// The snapshot of the input destinations; the inputs sampled into their leaves keep the leaves'
    /// bytes in it (`input_leaves`).
    pub(super) saved: SavedDestinations,
    patches: Vec<StagedPatch>,
    window: Vec<u8>,
    bytes: Vec<u8>,
    /// For each published hierarchical binding, by its index among the prepared bindings, the key
    /// of its image entry, built at construction and moved into the entry when the binding first
    /// publishes, so that no scan builds one (C1-19c).
    published_keys: Vec<Option<IoAddressKey>>,
    pub(super) outputs_before: Vec<u8>,
}

/// The staging that the bindings of a prepared module use in one scan.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(in crate::vm::engine) struct StagingDemand {
    /// `%I` and `%M` bindings, sampled at each scan start.
    pub(in crate::vm::engine) inputs: usize,
    /// `%Q` and `%M` bindings, published at each scan end.
    pub(in crate::vm::engine) outputs: usize,
    /// The widest flat `%Q` or `%M` window.
    pub(in crate::vm::engine) window: usize,
    /// The bytes of all flat `%Q` and `%M` windows together.
    pub(in crate::vm::engine) bytes: usize,
    /// Every binding, each with room for the key of a published hierarchical entry.
    pub(in crate::vm::engine) bindings: usize,
    /// The path bytes of the keys of the published hierarchical bindings.
    pub(in crate::vm::engine) key_bytes: usize,
}

impl StagingDemand {
    /// Count the bindings and their flat windows. An address whose window is invalid stays
    /// out of the sums; the scan refuses it exactly as before.
    pub(in crate::vm::engine) fn of(bindings: &[PreparedIoBinding]) -> Option<Self> {
        let mut demand = Self {
            bindings: bindings.len(),
            ..Self::default()
        };
        for binding in bindings {
            let address = &binding.address;
            if matches!(address.area, IoArea::Input | IoArea::Memory) {
                demand.inputs = demand.inputs.checked_add(1)?;
            }
            if !matches!(address.area, IoArea::Output | IoArea::Memory) {
                continue;
            }
            demand.outputs = demand.outputs.checked_add(1)?;
            if address.path.len() > 1 {
                demand.key_bytes = address
                    .path
                    .len()
                    .checked_mul(core::mem::size_of::<u32>())
                    .and_then(|bytes| demand.key_bytes.checked_add(bytes))?;
            }
            if let Ok(Some(end)) = address.flat_byte_end() {
                let width = (end as usize).saturating_sub(address.byte as usize);
                demand.window = demand.window.max(width);
                demand.bytes = demand.bytes.checked_add(width)?;
            }
        }
        Some(demand)
    }

    /// The bytes that `ImageStaging::reserve` takes for this demand.
    pub(in crate::vm::engine) fn reservation_bytes(self) -> Option<usize> {
        let inputs = self.inputs.checked_mul(
            core::mem::size_of::<StagedInput>()
                + core::mem::size_of::<Key>()
                + core::mem::size_of::<(Key, Value)>(),
        )?;
        let outputs = self
            .outputs
            .checked_mul(core::mem::size_of::<StagedPatch>())?;
        let keys = self
            .bindings
            .checked_mul(core::mem::size_of::<Option<IoAddressKey>>())?
            .checked_add(self.key_bytes)?;
        inputs
            .checked_add(outputs)?
            .checked_add(self.window)?
            .checked_add(self.bytes)?
            .checked_add(keys)
    }
}

impl ImageStaging {
    /// Reserve the staging of `demand`, the demand of `bindings`, at construction.
    pub(in crate::vm::engine) fn reserve(
        &mut self,
        demand: StagingDemand,
        bindings: &[PreparedIoBinding],
    ) -> Result<(), RuntimeError> {
        let refused = |_| crate::vm::VmTrap::BudgetExceeded.into_runtime_error();
        self.published_keys
            .try_reserve_exact(bindings.len())
            .map_err(refused)?;
        for binding in bindings {
            let address = &binding.address;
            let published =
                matches!(address.area, IoArea::Output | IoArea::Memory) && address.path.len() > 1;
            self.published_keys
                .push(published.then(|| IoAddressKey::from(address)));
        }
        self.inputs
            .try_reserve_exact(demand.inputs)
            .map_err(refused)?;
        self.keys
            .try_reserve_exact(demand.inputs)
            .map_err(refused)?;
        self.saved.reserve_storage(demand.inputs)?;
        self.patches
            .try_reserve_exact(demand.outputs)
            .map_err(refused)?;
        self.window
            .try_reserve_exact(demand.window)
            .map_err(refused)?;
        self.bytes.try_reserve_exact(demand.bytes).map_err(refused)
    }

    /// Reserve the copy of an output image of `bytes` bytes (C1-19f).
    pub(in crate::vm::engine) fn reserve_output_copy(
        &mut self,
        bytes: usize,
    ) -> Result<(), RuntimeError> {
        self.outputs_before
            .try_reserve_exact(bytes)
            .map_err(|_| crate::vm::VmTrap::BudgetExceeded.into_runtime_error())
    }

    /// Keep the output image a publication begins with, in the reserved copy.
    fn keep_outputs(&mut self, outputs: &[u8]) {
        keep_outputs(&mut self.outputs_before, outputs);
    }

    /// Reserve room for `bytes` bytes of input destinations saved as their cells (C1-16).
    pub(in crate::vm::engine) fn reserve_snapshot_bytes(
        &mut self,
        bytes: usize,
    ) -> Result<(), RuntimeError> {
        self.saved.reserve_bytes(bytes)
    }

    /// Empty every part, keeping its storage.
    fn clear(&mut self) {
        self.inputs.clear();
        self.keys.clear();
        self.saved.clear();
        self.patches.clear();
        self.window.clear();
        self.bytes.clear();
        self.outputs_before.clear();
    }

    /// Apply the staged patches in binding order. A hierarchical value is stored in its address's
    /// entry, so a scan builds no key (C1-16).
    fn apply(&mut self, images: &mut ProcessImages, bindings: &[PreparedIoBinding]) {
        for patch in self.patches.drain(..) {
            match patch {
                StagedPatch::Hierarchical(index, value) => {
                    if let Some(binding) = bindings.get(index) {
                        let key = self.published_keys.get_mut(index);
                        super::store_hierarchical(images, &binding.address, value, key);
                    }
                }
                StagedPatch::Flat {
                    area,
                    start,
                    offset,
                    len,
                    bit,
                } => write_window(images, area, start, &self.bytes[offset..offset + len], bit),
            }
        }
    }

    /// The staged patches as owned values, for a binding-only update to apply at its commit.
    #[cfg(feature = "std")]
    fn take_patches(&mut self, bindings: &[PreparedIoBinding]) -> Vec<OutputPatch> {
        let bytes = &self.bytes;
        self.patches
            .drain(..)
            .map(|patch| match patch {
                StagedPatch::Hierarchical(index, value) => {
                    OutputPatch::Hierarchical(IoAddressKey::from(&bindings[index].address), value)
                }
                StagedPatch::Flat {
                    area,
                    start,
                    offset,
                    len,
                    bit,
                } => OutputPatch::Flat {
                    area,
                    start,
                    bytes: bytes[offset..offset + len].to_vec(),
                    bit,
                },
            })
            .collect()
    }
}

/// Whether two bindings bind the same variable to the same address through the same codec. The
/// codec follows from the declared type, which a binding-only update never changes.
#[cfg(feature = "std")]
fn same_binding(a: &PreparedIoBinding, b: &PreparedIoBinding) -> bool {
    a.ref_idx == b.ref_idx && a.address == b.address && a.type_id == b.type_id
}

impl EngineState<'_> {
    #[cfg(test)]
    pub(in crate::vm::engine) fn publish_output_image(&mut self) -> Result<(), RuntimeError> {
        self.publish_output_image_with_hooks(&mut super::super::cycle_port::NoopCycleHooks)
    }

    pub(in crate::vm::engine) fn sample_input_image(&mut self) -> Result<(), RuntimeError> {
        // The reserved staging is taken out for the scan and returned empty.
        let mut staging = core::mem::take(&mut self.data.resources.staging);
        let result = self.sample_inputs(&mut staging);
        staging.clear();
        self.data.resources.staging = staging;
        result
    }

    fn sample_inputs(&mut self, staging: &mut ImageStaging) -> Result<(), RuntimeError> {
        let prepared = self.prepared;
        // Every input and marker of a scalar variable is sampled into its leaf (checklist P1-6),
        // except while an initializer stages its result, whose writes the visibility rules check.
        if let Some(leaves) = prepared.input_leaves
            && self.data.construction.initializers.is_empty()
        {
            return self.sample_input_leaves(staging, leaves);
        }
        let count = prepared
            .io_bindings
            .iter()
            .filter(|binding| matches!(binding.address.area, IoArea::Input | IoArea::Memory))
            .count();
        // The staged values are charged as the owned reference/value pairs they replace.
        self.charge_allocation_bytes(
            count
                .checked_mul(size_of::<(ValueRef, Value)>())
                .ok_or_else(|| RuntimeError::Overflow)?,
        )?;
        staging
            .inputs
            .try_reserve_exact(count)
            .map_err(|_| RuntimeError::Overflow)?;
        for binding in &prepared.io_bindings {
            if !matches!(binding.address.area, IoArea::Input | IoArea::Memory) {
                continue;
            }
            let reference = self.persistent_io_view(binding.ref_idx)?;
            let value = match self.stage_text_input(binding, reference)? {
                Some(text) => text,
                None => {
                    let value = binding
                        .codec
                        .decode(self.read_image_address(&binding.address)?)?;
                    self.check_write(reference, &value)?;
                    StagedValue::Value(match binding.type_id {
                        Some(ty) => self.normalize_assignment_value(ty, value)?,
                        None => value,
                    })
                }
            };
            staging.inputs.push(StagedInput {
                location: reference.location,
                offset: reference.offset,
                ref_idx: binding.ref_idx,
                value,
            });
        }
        // Admitted I/O codecs produce scalars or byte strings, never instance or
        // reference roots, so these physical destinations cannot redirect each other.
        // The inputs are applied through the store the engine holds, so the cell arm receives them (R1).
        let generation = StoreAccess::generation(self);
        self.snapshot_input_destinations(
            staging
                .inputs
                .iter()
                .map(|input| input.view(prepared, generation)),
            &mut staging.keys,
            &mut staging.saved,
        )?;
        for input in staging.inputs.drain(..) {
            let path = input.path(prepared);
            let StagedInput {
                location,
                offset,
                value,
                ..
            } = input;
            let value = match value {
                StagedValue::Value(value) => value,
                StagedValue::Text { area, start, len } => {
                    let bytes = start as usize..start as usize + len as usize;
                    if let Err(error) = self.write_text_input(location, offset, path, area, bytes) {
                        self.restore_destinations(None, &mut staging.saved);
                        return Err(error);
                    }
                    continue;
                }
            };
            let capacity = self.declared_string_capacity(crate::value::ValueRefView {
                location,
                offset,
                generation,
                path,
            });
            let result = self
                .charge_storage_path_write(&*self, location, offset, path, &value)
                .and_then(|()| {
                    let write = self
                        .prepare_write(location, offset, path, &value)?
                        .ok_or_else(|| RuntimeError::NullReference)?;
                    self.check_entry_deadline()?;
                    self.commit_write(path, value, write, capacity)
                        .then_some(())
                        .ok_or_else(|| RuntimeError::NullReference)
                });
            if let Err(error) = result {
                self.restore_destinations(None, &mut staging.saved);
                return Err(error);
            }
        }
        Ok(())
    }

    pub(in crate::vm::engine) fn publish_output_image_with_hooks(
        &mut self,
        hooks: &mut dyn CycleHooks,
    ) -> Result<(), RuntimeError> {
        self.publish_images(hooks, false)
    }

    /// Publish the value of every variable located at `%M` into the marker image, as the
    /// end-of-cycle synchronization does, and touch nothing else. Construction and every restart
    /// or online change call this once the variables hold their initial or retained values, so
    /// the first scan, which loads the marker image into the bound variables, sees the value the
    /// variable already has (IEC 61131-3 Ed.3 6.5.6; specification 11 6.7). `%I` is left to
    /// the input sample and `%Q` to the first completed scan. The publication is atomic: a
    /// refusal changes no image.
    pub(in crate::vm::engine) fn publish_marker_image(&mut self) -> Result<(), RuntimeError> {
        if !self
            .prepared
            .io_bindings
            .iter()
            .any(|binding| binding.address.area == IoArea::Memory)
        {
            return Ok(());
        }
        self.publish_images(&mut super::super::cycle_port::NoopCycleHooks, true)
    }

    /// The staged, all-or-nothing image publication behind the end of a cycle. `markers_only`
    /// limits it to the `%M` bindings and runs no output hook.
    fn publish_images(
        &mut self,
        hooks: &mut dyn CycleHooks,
        markers_only: bool,
    ) -> Result<(), RuntimeError> {
        // Copied leaves are published from where they lie, without staging (checklist P1-6).
        if let Some(result) = self.publish_copied(hooks, markers_only) {
            return result;
        }
        // The reserved staging is taken out for the publication and returned empty.
        let mut staging = core::mem::take(&mut self.data.resources.staging);
        let result = self.publish_staged(&mut staging, hooks, markers_only);
        staging.clear();
        self.data.resources.staging = staging;
        result
    }

    pub(super) fn publish_staged(
        &mut self,
        staging: &mut ImageStaging,
        hooks: &mut dyn CycleHooks,
        markers_only: bool,
    ) -> Result<(), RuntimeError> {
        let published = |binding: &PreparedIoBinding| {
            let area = binding.address.area;
            area == IoArea::Memory || (area == IoArea::Output && !markers_only)
        };
        let windows = [
            self.data.images.outputs.len(),
            self.data.images.memory.len(),
        ];
        self.stage_patches(staging, &published, windows)?;
        // The no-op composition retains its original publication checks/cost.
        self.check_entry_deadline()?;
        let output_hooks = hooks.has_output_hooks();
        let mut journal = None;
        if output_hooks {
            let windows = staging
                .patches
                .iter()
                .filter(|patch| {
                    matches!(
                        patch,
                        StagedPatch::Flat {
                            area: IoArea::Output,
                            ..
                        }
                    )
                })
                .count();
            let mut opened = self.open_output_journal(windows)?;
            for patch in &staging.patches {
                if let StagedPatch::Flat {
                    area: IoArea::Output,
                    start,
                    len,
                    ..
                } = patch
                {
                    self.journal_output_window(&mut opened, *start, *len)?;
                }
            }
            staging.keep_outputs(&self.data.images.outputs);
            journal = Some(opened);
        }
        // All fallible conversion/allocation/policy work precedes publication.
        staging.apply(&mut self.data.images, &self.prepared.io_bindings);
        self.submit_outputs(hooks, journal, &staging.outputs_before)
    }

    /// What follows a publication's writes into the images: with output hooks, which `journal`
    /// then is for, the hooks of `AfterOutputSample` and the submission of the outputs; a refusal
    /// puts back `outputs_before`, the output image the publication began with.
    pub(super) fn submit_outputs(
        &mut self,
        hooks: &mut dyn CycleHooks,
        journal: Option<OutputJournal>,
        outputs_before: &[u8],
    ) -> Result<(), RuntimeError> {
        if let Some(mut journal) = journal {
            // Provisional output views are callback-scoped. Preserve the hosted
            // rollback domain: flat Q only, not M/hierarchical state effects.
            // Physical effects already completed by a driver cannot be rolled back.
            let before = self.services.allocator_operations();
            let sampled = hooks.on_phase(
                CyclePhase::AfterOutputSample,
                &mut CycleAccess {
                    engine: self,
                    phase: CyclePhase::AfterOutputSample,
                    output_undo: Some(&mut journal),
                },
            );
            self.count_service_operations(before);
            let result = sampled
                .and_then(|()| self.check_entry_deadline())
                .and_then(|()| {
                    let before = self.services.allocator_operations();
                    let published = hooks.publish_outputs(&CycleAccess {
                        engine: self,
                        phase: CyclePhase::AfterOutputSample,
                        output_undo: None,
                    });
                    self.count_service_operations(before);
                    published
                });
            if let Err(error) = result {
                self.data.images.outputs.copy_from_slice(outputs_before);
                return Err(error);
            }
            // Successful submission commits Q. A late deadline/observation
            // failure cannot undo the logical image already accepted by drivers.
            self.check_entry_deadline()?;
            self.cycle_hook(hooks, CyclePhase::CycleEnd)?;
        }
        Ok(())
    }

    /// Stage into `staging`, in binding order, the patches that publish the variables of the
    /// bindings that `selected` accepts. Every fallible conversion, allocation and window check
    /// runs here and no image changes. `windows` are the lengths of the flat output and marker
    /// images that the patches will be applied to.
    fn stage_patches(
        &mut self,
        staging: &mut ImageStaging,
        selected: &dyn Fn(&PreparedIoBinding) -> bool,
        windows: [usize; 2],
    ) -> Result<(), RuntimeError> {
        let prepared = self.prepared;
        let mut count = 0usize;
        for binding in &prepared.io_bindings {
            if selected(binding) {
                count += 1;
            }
        }
        self.charge_allocation_bytes(
            count
                .checked_mul(size_of::<OutputPatch>())
                .ok_or_else(|| RuntimeError::Overflow)?,
        )?;
        staging
            .patches
            .try_reserve_exact(count)
            .map_err(|_| RuntimeError::Overflow)?;
        let mut hierarchical_count = 0usize;
        for (index, binding) in prepared.io_bindings.iter().enumerate() {
            if !selected(binding) {
                continue;
            }
            let window = if binding.address.area == IoArea::Output {
                windows[0]
            } else {
                windows[1]
            };
            let patch = self.binding_patch(index, binding, window, staging)?;
            if matches!(patch, StagedPatch::Hierarchical(..)) {
                hierarchical_count += 1;
            }
            staging.patches.push(patch);
        }
        self.data
            .images
            .hierarchical
            .try_reserve(hierarchical_count)
            .map_err(|_| RuntimeError::Overflow)?;
        Ok(())
    }

    /// The value of binding `index`'s scalar variable, read from its leaf (checklist P1-6), as the
    /// read through its static reference reads it: the reference has no path, so it charges no
    /// path bytes, and a scalar's copy charges nothing (TIME-08). `None` for a binding whose
    /// variable has no leaf of its own, and while an initializer stages its result, whose reads the
    /// visibility rules check; such a binding is read through its reference.
    fn binding_leaf_value(&self, index: usize) -> Option<Value> {
        if !self.data.construction.initializers.is_empty() {
            return None;
        }
        let leaf = (*self.prepared.io_leaves.get(index)?)?;
        let range = self.leaf_range(leaf)?;
        self.data.reader(self.prepared).scalar_at(range, leaf.prim)
    }

    /// The patch that publishes the current value of one binding's variable, encoded for the
    /// binding: it changes exactly the bytes, the bit or the hierarchical key that the binding
    /// covers. `window` is the length of the flat image of the binding's area that the patch will
    /// be applied to; a flat patch's bytes go to the end of the staged bytes.
    fn binding_patch(
        &mut self,
        index: usize,
        binding: &PreparedIoBinding,
        window: usize,
        staging: &mut ImageStaging,
    ) -> Result<StagedPatch, RuntimeError> {
        let address = &binding.address;
        if let Some((leaf, held)) = self.copied_output(index) {
            // A copied binding's leaf bytes are its image bytes (`IoLeaf::copy`): encoding them
            // changes no bit.
            if address.area == IoArea::Input {
                return Err(RuntimeError::TypeMismatch);
            }
            self.copy_flat_window(address, leaf, &held, window, &mut staging.window)?;
        } else {
            let value = match self.binding_leaf_value(index) {
                Some(value) => value,
                None => {
                    let reference = self.persistent_io_view(binding.ref_idx)?;
                    self.check_read(reference)?;
                    let view = self
                        .read_path(reference.location, reference.offset, reference.path)
                        .map_err(|_| RuntimeError::NullReference)?;
                    self.charge_allocation_bytes(self.view_clone_charge(&view)?)?;
                    // The read above already resolved this path, so the view holds exactly the
                    // leaf the old whole-value materialize returned.
                    self.materialize(&view)?
                }
            };
            let value = binding.codec.encode(value, address.size)?;
            if address.path.len() > 1 {
                self.charge_hierarchical_entry(address)?;
                return Ok(StagedPatch::Hierarchical(index, value));
            }
            if address.area == IoArea::Input {
                return Err(RuntimeError::TypeMismatch);
            }
            self.encode_flat_window(address, value, window, &mut staging.window)?;
        }
        let offset = staging.bytes.len();
        staging
            .bytes
            .try_reserve(staging.window.len())
            .map_err(|_| RuntimeError::Overflow)?;
        staging.bytes.extend_from_slice(&staging.window);
        Ok(StagedPatch::Flat {
            area: address.area,
            start: address.byte as usize,
            offset,
            len: staging.window.len(),
            bit: matches!(address.size, crate::io_address::IoSize::Bit).then_some(address.bit),
        })
    }

    /// Stage what a binding-only update owes the variables it binds to `%M`: the current value of
    /// each marker binding of this module that `previous`, the bindings it replaces, does not
    /// already hold, as the patches that change exactly the bytes, bits or hierarchical keys that
    /// binding covers. A marker binding that the update leaves as it was, and every other bit and
    /// byte, is not touched, so what a driver wrote since the last scan survives. `memory_window`
    /// is the length that the marker image has once the update commits. Nothing is applied here
    /// (specification 11 6.7).
    #[cfg(feature = "std")]
    pub(in crate::vm::engine) fn stage_marker_patches(
        &mut self,
        previous: &[PreparedIoBinding],
        memory_window: usize,
    ) -> Result<Vec<OutputPatch>, RuntimeError> {
        let added_or_changed = |binding: &PreparedIoBinding| {
            binding.address.area == IoArea::Memory
                && !previous.iter().any(|before| same_binding(before, binding))
        };
        let mut staging = core::mem::take(&mut self.data.resources.staging);
        let result = self
            .stage_patches(&mut staging, &added_or_changed, [0, memory_window])
            .and_then(|()| self.check_entry_deadline())
            .map(|()| staging.take_patches(&self.prepared.io_bindings));
        staging.clear();
        self.data.resources.staging = staging;
        result
    }

    pub(in crate::vm::engine) fn read_cycle_image(
        &mut self,
        address: &IoAddress,
    ) -> Result<Value, RuntimeError> {
        // Process-image reads are bookkeeping under TIME-08 and charge no work;
        // the entry deadline is still checked before and after the read.
        self.check_entry_deadline()?;
        let value = self.read_image_address(address)?;
        self.check_entry_deadline()?;
        Ok(value)
    }

    pub(in crate::vm::engine) fn validate_cycle_image_write(
        &mut self,
        address: &IoAddress,
        value: Value,
    ) -> Result<(), RuntimeError> {
        validate_process_image_address(address)?;
        if address.path.len() > 1 {
            let binding = self
                .prepared
                .io_bindings
                .iter()
                .find(|binding| binding.address == *address)
                .ok_or_else(|| {
                    RuntimeError::InvalidIoAddress("unbound hierarchical image".into())
                })?;
            binding
                .codec
                .encode(binding.codec.decode(value)?, address.size)?;
        } else {
            let len = match address.area {
                IoArea::Input => self.data.images.inputs.len(),
                IoArea::Output => self.data.images.outputs.len(),
                IoArea::Memory => self.data.images.memory.len(),
            };
            self.encode_flat_window(address, value, len, &mut Vec::new())?;
        }
        self.check_entry_deadline()
    }

    pub(in crate::vm::engine) fn write_cycle_image(
        &mut self,
        address: &IoAddress,
        value: Value,
        journal: Option<&mut OutputJournal>,
    ) -> Result<(), RuntimeError> {
        validate_process_image_address(address)?;
        if address.path.len() > 1 {
            let binding = self
                .prepared
                .io_bindings
                .iter()
                .find(|binding| binding.address == *address)
                .ok_or_else(|| {
                    RuntimeError::InvalidIoAddress("unbound hierarchical image".into())
                })?;
            // This port accepts wire values; the shared codec validates their
            // declared semantic representation before any image mutation.
            let value = binding
                .codec
                .encode(binding.codec.decode(value)?, address.size)?;
            self.charge_hierarchical_entry(address)?;
            self.data
                .images
                .hierarchical
                .try_reserve(1)
                .map_err(|_| RuntimeError::PreparationLimit)?;
            self.check_entry_deadline()?;
            super::store_hierarchical(&mut self.data.images, address, value, None);
        } else {
            let len = match address.area {
                IoArea::Input => self.data.images.inputs.len(),
                IoArea::Output => self.data.images.outputs.len(),
                IoArea::Memory => self.data.images.memory.len(),
            };
            self.check_image_window(address, len)?;
            let end = address
                .flat_byte_end()?
                .ok_or_else(|| RuntimeError::TypeMismatch)? as usize;
            let width = end
                .checked_sub(address.byte as usize)
                .ok_or_else(|| RuntimeError::Overflow)?;
            self.charge_allocation_bytes(width)?;
            // Encode into only the addressed window so rejection cannot change
            // neighboring bytes or require a whole-image rollback copy.
            let mut patch = Vec::new();
            patch
                .try_reserve_exact(width)
                .map_err(|_| RuntimeError::PreparationLimit)?;
            let image = match address.area {
                IoArea::Input => &self.data.images.inputs,
                IoArea::Output => &self.data.images.outputs,
                IoArea::Memory => &self.data.images.memory,
            };
            patch.extend_from_slice(&image[address.byte as usize..end]);
            let local = IoAddress {
                area: address.area,
                size: address.size,
                bit: address.bit,
                byte: 0,
                path: Vec::new(),
                wildcard: false,
            };
            write_flat_image(&mut patch, &local, value)?;
            if address.area == IoArea::Output
                && let Some(journal) = journal
            {
                self.journal_output_window(journal, address.byte as usize, width)?;
            }
            self.check_entry_deadline()?;
            let image = match address.area {
                IoArea::Input => &mut self.data.images.inputs,
                IoArea::Output => &mut self.data.images.outputs,
                IoArea::Memory => &mut self.data.images.memory,
            };
            image[address.byte as usize..end].copy_from_slice(&patch);
        }
        Ok(())
    }
    /// Encode the flat window of `address` into `bytes`, which keeps its storage, after
    /// checking it against `len`, the length of the image of the address's area. The window's
    /// bytes are charged as a new allocation (spec 34 TIME-08).
    /// The leaf of copied output or marker binding `index` (`IoLeaf::copy`) and its bytes, read
    /// where the value path reads them (`binding_leaf_value`); `None` takes the value path.
    pub(super) fn copied_output(&self, index: usize) -> Option<(IoLeaf, [u8; 8])> {
        if !self.data.construction.initializers.is_empty() {
            return None;
        }
        let leaf = (*self.prepared.io_leaves.get(index)?)?;
        if !leaf.copy {
            return None;
        }
        let held = self.data.store.bytes(self.leaf_range(leaf)?)?;
        let mut bytes = [0u8; 8];
        bytes.get_mut(..held.len())?.copy_from_slice(held);
        Some((leaf, bytes))
    }

    /// [`Self::encode_flat_window`] for a copied binding whose leaf holds `held`: the same window
    /// check and charge, and the window holds the leaf's bytes, or for a bit the BOOL's.
    fn copy_flat_window(
        &self,
        address: &IoAddress,
        leaf: IoLeaf,
        held: &[u8; 8],
        len: usize,
        bytes: &mut Vec<u8>,
    ) -> Result<(), RuntimeError> {
        self.check_image_window(address, len)?;
        let end = address
            .flat_byte_end()?
            .ok_or_else(|| RuntimeError::TypeMismatch)? as usize;
        let width = end - address.byte as usize;
        self.charge_allocation_bytes(width)?;
        bytes.clear();
        bytes
            .try_reserve_exact(width)
            .map_err(|_| RuntimeError::Overflow)?;
        bytes.resize(width, 0);
        if address.size == crate::io_address::IoSize::Bit {
            if held[0] != 0 {
                bytes[0] = 1 << address.bit;
            }
        } else if width == usize::from(leaf.bytes) {
            bytes.copy_from_slice(&held[..width]);
        } else {
            return Err(RuntimeError::TypeMismatch);
        }
        Ok(())
    }

    fn encode_flat_window(
        &self,
        address: &IoAddress,
        value: Value,
        len: usize,
        bytes: &mut Vec<u8>,
    ) -> Result<(), RuntimeError> {
        self.check_image_window(address, len)?;
        let end = address
            .flat_byte_end()?
            .ok_or_else(|| RuntimeError::TypeMismatch)? as usize;
        let width = end - address.byte as usize;
        self.charge_allocation_bytes(width)?;
        bytes.clear();
        bytes
            .try_reserve_exact(width)
            .map_err(|_| RuntimeError::Overflow)?;
        bytes.resize(width, 0);
        let local = IoAddress {
            area: address.area,
            size: address.size,
            bit: address.bit,
            byte: 0,
            path: Vec::new(),
            wildcard: false,
        };
        write_flat_image(bytes, &local, value)
    }
}

#[cfg(test)]
mod tests;
