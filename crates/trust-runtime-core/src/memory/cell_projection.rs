//! The flat store as the debugger reads it (specification 34 §6.3, specification 11 §6.13.1;
//! C1-17).
//!
//! The debugger shows the values a program holds as the value store holds them: a detached copy of the
//! storage, and the locals of each frame on the call stack. On the flat store the globals, the
//! instances and the typed locals of the frames lie in the blocks, so the copy is the `Value` half with
//! every global and every instance materialized from its bytes, and a local is read from its frame's
//! bytes on the activation stack. A flat reference is named by the variable and the path that hold its
//! target, as the value store names it. None of this runs unless the debugger asks for it.

use super::*;
use alloc::vec::Vec;

impl<'a> CellReader<'a> {
    /// The PLC storage as one owned [`VariableStorage`] that holds no named frame: the `Value` half,
    /// every declared global in declaration order and every instance, the `Value` half's and those the
    /// blocks hold, in identity order, with the flat store's identities. A copy the value store would
    /// hold for the same program, so a reference the debugger evaluates resolves in it.
    ///
    /// # Errors
    ///
    /// The error of a value that does not materialize, and
    /// [`RuntimeError::InvalidExecutionState`] when the blocks and the module disagree.
    pub(crate) fn project_storage(self) -> Result<VariableStorage, RuntimeError> {
        let mut storage = self.values.clone();
        let declared = self.prepared.declarations(StorageOwner::Global, None).len();
        let mut globals = OrderedMap::default();
        for ordinal in 0..declared {
            let name = self
                .global_name(ordinal)
                .ok_or(RuntimeError::InvalidExecutionState)?;
            let value = match self.global(ordinal) {
                Some((range, type_id, presence)) => {
                    self.view(range, type_id, presence).materialize()?
                }
                // A global that owns no cells, a program root, is the `Value` half's.
                None => self
                    .values
                    .get_global(name)
                    .cloned()
                    .ok_or(RuntimeError::InvalidExecutionState)?,
            };
            globals.insert(SmolStr::new(name), self.named(value, 0)?);
        }
        storage.replace_globals(globals);
        // A store without `std` keeps its instances in identity order, so every instance is put back
        // in that order.
        let mut instances = Vec::new();
        #[allow(
            clippy::iter_kv_map,
            reason = "the instance map of a store without `std` has no `keys`"
        )]
        let held: Vec<InstanceId> = storage.instances().iter().map(|(id, _)| *id).collect();
        for id in held {
            let data = storage
                .take_instance(id)
                .ok_or(RuntimeError::InvalidExecutionState)?;
            instances.push((id, data));
        }
        for id in self.arm.cells.regions().instance_ids() {
            let mut data = self
                .snapshot_instance(id)
                .ok_or(RuntimeError::InvalidExecutionState)?;
            for value in data.variables.values_mut() {
                let held = core::mem::replace(value, Value::Null);
                *value = self.named(held, 0)?;
            }
            instances.push((id, data));
        }
        instances.sort_unstable_by_key(|(id, _)| *id);
        for (id, data) in instances {
            storage.put_instance(id, data)?;
        }
        self.arm.cells.lend_identities(&mut storage);
        Ok(storage)
    }

    /// Read every global and every persistent instance into a value once, dropping each, so that a
    /// store some value cannot be read from is refused at construction and
    /// [`CellReader::project_storage`] cannot fail afterwards (C1-19d; decision O3 (a)).
    ///
    /// # Errors
    ///
    /// The error of a value that does not materialize, and
    /// [`RuntimeError::InvalidExecutionState`] when the blocks and the module disagree.
    #[cfg(feature = "std")]
    pub(crate) fn check_materializes(self) -> Result<(), RuntimeError> {
        let declared = self.prepared.declarations(StorageOwner::Global, None).len();
        for ordinal in 0..declared {
            if let Some((range, type_id, presence)) = self.global(ordinal) {
                drop(self.view(range, type_id, presence).materialize()?);
            }
        }
        for id in self.arm.cells.regions().instance_ids() {
            drop(
                self.snapshot_instance(id)
                    .ok_or(RuntimeError::InvalidExecutionState)?,
            );
        }
        Ok(())
    }

    /// The view of slot `slot` of the frame of activation `id`, a frame of `pou`, when the frame lies
    /// on the activation stack and holds the slot there; `None` when the frame keeps the slot as a
    /// value (a compiler scratch slot, or a variable whose value holds a reference).
    pub(crate) fn frame_local(self, id: FrameId, pou: u32, slot: usize) -> Option<ValueView<'a>> {
        let block = self.arm.frame(id).filter(|block| block.pou == pou)?;
        self.frame_slot_view(block.range, block.pou, slot)
    }

    /// `value` with each flat reference it holds, nested ones too, named by the variable and the
    /// path that hold its target, as the value store names it (C1-11d): what a reader at the
    /// boundary, such as the debugger, shows.
    ///
    /// # Errors
    ///
    /// [`RuntimeError::TypeMismatch`] for a value nested deeper than the admitted type nesting.
    pub(crate) fn named_value(self, value: Value) -> Result<Value, RuntimeError> {
        self.named(value, 0)
    }

    /// `value` with each flat reference it holds named by the variable and the path that hold its
    /// target ([`CellReader::path_reference`]).
    fn named(self, mut value: Value, depth: usize) -> Result<Value, RuntimeError> {
        if depth > BYTECODE_MAX_TYPE_NESTING {
            return Err(RuntimeError::TypeMismatch);
        }
        match &mut value {
            Value::Reference(Some(reference)) => {
                if let Some(named) = self.path_reference(reference) {
                    *reference = named;
                }
            }
            Value::Array(array) => {
                for element in array.elements_mut() {
                    let held = core::mem::replace(element, Value::Null);
                    *element = self.named(held, depth + 1)?;
                }
            }
            Value::Struct(structure) => {
                alloc::sync::Arc::make_mut(structure)
                    .remap_retained_identities(|_, held| self.named(held, depth + 1))?;
            }
            _ => {}
        }
        Ok(value)
    }
}
