//! The capture of a resource's retained values, which the services persist when a save is due
//! (specification 11 §6.7; specification 34 §10). It is not on the scan path: on a chip it runs
//! from flash (checklist P1-7).
use super::*;
use crate::bytecode::{StorageOwner, StorageRole};
use crate::memory::layout::CELL_BYTES;
use crate::memory::view::{StoreAccess, ValueView};
use crate::memory::MemoryLocation;
use crate::retain::capture::RetainCapture;
use crate::retain::codec::{decode_snapshot_payload_bytes, view_is_retainable, PayloadWriter};
use crate::retain::RetainSnapshot;

impl EngineState<'_> {
    /// Capture the retained image this resource would persist, in the registration and declaration
    /// order of specification 11 §6.7, through the shared capture: its reserved staging buffer
    /// holds the STRN v3 payload bytes the codec writes, so a steady-state capture allocates
    /// nothing (specification 34 MEM-04).
    pub(super) fn capture_retained(
        &mut self,
        capture: &mut RetainCapture,
    ) -> Result<(), RuntimeError> {
        capture.refill(|payload| self.fill_retained_payload(payload))
    }

    /// An explicitly requested retained checkpoint as a fresh snapshot, through the shared capture.
    pub(super) fn cycle_retain_snapshot(&mut self) -> Result<RetainSnapshot, RuntimeError> {
        let mut capture = RetainCapture::new();
        self.capture_retained(&mut capture)?;
        decode_snapshot_payload_bytes(capture.staged_payload())
    }

    /// The most bytes [`Self::fill_retained_payload`] appends for this resource, with the entry
    /// count: each retained global and each root's retained program variable whose type holds no
    /// reference and no instance, under the name it is written with and at the largest encoding of
    /// its type (specification 34 MEM-09; C1-19c). `None` on overflow.
    pub(super) fn retained_payload_bound(&self) -> Option<usize> {
        retained_payload_bound(self.prepared)
    }

    /// Reserve the retain capture the services persist through, when they have one, for the largest
    /// payload of this resource, charged with the construction (specification 34 MEM-09; C1-19c).
    pub(super) fn reserve_retain_capture(&mut self) -> Result<(), RuntimeError> {
        let Some(capture) = self.services.retain_capture() else {
            return Ok(());
        };
        let bytes = self
            .retained_payload_bound()
            .ok_or(RuntimeError::Overflow)?;
        self.charge_allocation_bytes(bytes.checked_mul(2).ok_or(RuntimeError::Overflow)?)?;
        capture.borrow_mut().reserve(bytes)
    }

    /// Append the retained values to `payload`: the resource's retained globals and then each root's
    /// retained program variables, in registration and declaration order (specification 11 §6.7).
    ///
    /// Each value is read as a view of the store the engine holds and written from that view, so a value in
    /// cells is encoded from its cells without building a `Value` (specification 34 §10; C1-9d2b-2). Its
    /// globals are reached by slot (O22).
    fn fill_retained_payload(
        &mut self,
        payload: &mut PayloadWriter<'_>,
    ) -> Result<(), RuntimeError> {
        let prepared = self.prepared;
        for &id in prepared.retained_declarations(StorageOwner::Global, None) {
            let declaration = &prepared.layout.entries[id as usize];
            let name = &prepared.vm.strings[declaration.name_idx as usize];
            if let Ok(view) = self.read_slot(MemoryLocation::Global, declaration.slot as usize)
                && view_is_retainable(&view)?
            {
                let bytes = self.snapshot_entry_charge(&view, name.len())?;
                self.charge_allocation_bytes(bytes)?;
                payload.entry_view(name, &view)?;
            }
        }
        for (index, root) in prepared.roots.entries.iter().enumerate() {
            let root_declaration = &prepared.layout.entries[root.declaration_idx as usize];
            if root.parent_root_idx.is_some() || root_declaration.role != StorageRole::ProgramRoot {
                continue;
            }
            let instance =
                self.data.construction.roots[index].ok_or(RuntimeError::NullReference)?;
            let program_name = &prepared.vm.strings[root_declaration.name_idx as usize];
            for &id in prepared.retained_declarations(StorageOwner::Instance, root.template_pou_id)
            {
                let declaration = &prepared.layout.entries[id as usize];
                let name = &prepared.vm.strings[declaration.name_idx as usize];
                if let Some(view) = self.read_instance_var(instance, name)
                    && view_is_retainable(&view)?
                {
                    let key_len = program_name
                        .len()
                        .checked_add(name.len())
                        .and_then(|length| length.checked_add(10))
                        .ok_or(RuntimeError::Overflow)?;
                    let bytes = self.snapshot_entry_charge(&view, key_len)?;
                    self.charge_allocation_bytes(bytes)?;
                    if self.retained_program_key_conflicts(program_name, name) {
                        return Err(RuntimeError::RetainMigration(
                            "ambiguous retained checkpoint label".into(),
                        ));
                    }
                    // The reserved `@program/<program-identity>/<variable-identity>` key is
                    // appended as pieces, so an identity of any length builds no key string
                    // (specification 11 §6.7).
                    payload.entry_view_pieces(&["@program/", program_name, "/", name], &view)?;
                }
            }
        }
        Ok(())
    }

    /// Whether a retained global's declared name is exactly the `@program/<program>/<name>` key of
    /// the program value about to be appended (specification 11 §6.7).
    ///
    /// A program identity and variable identity pair is unique, so only a retained global's
    /// declared name can collide with a program key. The `@program/` prefix cannot be an IEC
    /// identifier, but a wire name is opaque, so the capture refuses a collision instead of writing
    /// two entries under one name.
    fn retained_program_key_conflicts(&self, program: &str, name: &str) -> bool {
        let prepared = self.prepared;
        prepared
            .retained_declarations(StorageOwner::Global, None)
            .iter()
            .any(|&id| {
                let declaration = &prepared.layout.entries[id as usize];
                let global = &prepared.vm.strings[declaration.name_idx as usize];
                global
                    .strip_prefix("@program/")
                    .and_then(|rest| rest.rsplit_once('/'))
                    == Some((program, name))
            })
    }

    fn snapshot_entry_charge(
        &self,
        view: &ValueView<'_>,
        name_bytes: usize,
    ) -> Result<usize, RuntimeError> {
        // A retained entry costs four cells of allocation (TIME-08 retained-value snapshot row). The
        // value's own copy charge is the one a `Value` pays, computed from the cells for a cell view.
        self.view_clone_charge(view)?
            .checked_add(name_bytes)
            .and_then(|bytes| bytes.checked_add(4 * CELL_BYTES))
            .ok_or_else(|| RuntimeError::Overflow)
    }
}

/// The most bytes a retained payload of `prepared`'s resource takes, with the entry count: each
/// retained global and each root's retained program variable whose type holds no reference and no
/// instance, under the name it is written with and at the largest encoding of its type
/// (specification 34 MEM-09; C1-19c). `None` on overflow.
pub(crate) fn retained_payload_bound(prepared: &PreparedModule) -> Option<usize> {
    // A variable whose value can hold a reference or an instance is left out of the payload.
    let entry = |key: usize, type_id: Option<u32>| -> Option<usize> {
        let Some(type_id) = type_id.filter(|&type_id| {
            prepared
                .type_layout(type_id)
                .is_some_and(|layout| !layout.holds_reference_or_instance)
        }) else {
            return Some(0);
        };
        crate::memory::cells::max_encoded_len(prepared, type_id, 0)?.checked_add(key + 4)
    };
    let mut total = 4usize;
    for &id in prepared.retained_declarations(StorageOwner::Global, None) {
        let declaration = prepared.layout.entries.get(id as usize)?;
        let name = prepared.vm.strings.get(declaration.name_idx as usize)?;
        total = total.checked_add(entry(name.len(), declaration.type_id)?)?;
    }
    for root in &prepared.roots.entries {
        let root_declaration = prepared.layout.entries.get(root.declaration_idx as usize)?;
        if root.parent_root_idx.is_some() || root_declaration.role != StorageRole::ProgramRoot {
            continue;
        }
        let program = prepared
            .vm
            .strings
            .get(root_declaration.name_idx as usize)?;
        for &id in prepared.retained_declarations(StorageOwner::Instance, root.template_pou_id) {
            let declaration = prepared.layout.entries.get(id as usize)?;
            let name = prepared.vm.strings.get(declaration.name_idx as usize)?;
            // The key `@program/<program>/<variable>`.
            let key = program.len().checked_add(name.len())?.checked_add(10)?;
            total = total.checked_add(entry(key, declaration.type_id)?)?;
        }
    }
    Some(total)
}
