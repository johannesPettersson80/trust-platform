//! Composition services used by the shared reference and execution paths.

use crate::memory::view::{StoreAccess, StoreMutate, ValueView};
use crate::memory::{MemoryLocation, VariableStorage};
use crate::value::{RefSegment, Value};

/// Storage access and assignment type information supplied by a runtime composition.
/// Legacy hosts obtain type information from HIR; source-free execution uses the
/// prepared artifact. Reference traversal itself has one shared implementation.
pub trait ReferenceContext {
    /// The store the composition's variables live in, reached only through the
    /// store-access seam (specification 34 §6.2, MEM-10). A hosted composition uses
    /// [`VariableStorage`].
    type Store: StoreAccess + StoreMutate;

    /// Prepared candidate references for a dynamic target, in wire order.
    /// None retains full discovery for legacy or temporary modules. An empty
    /// slice means the prepared module has no candidate for this storage slot.
    fn dynamic_reference_candidates(
        &self,
        _module: &super::module::VmModule,
        _reference: crate::value::ValueRefView<'_>,
    ) -> Result<Option<&[u32]>, crate::error::RuntimeError> {
        Ok(None)
    }

    /// Charge text work, one unit per UTF-8 byte, to the current execution allowance
    /// (spec 34 TIME-08, TEXT). Compositions without a work allowance charge nothing.
    fn charge_text_work(&self, _units: usize) -> Result<(), crate::error::RuntimeError> {
        Ok(())
    }

    /// Charge a STRING or WSTRING result of a native call as a new value: one work unit plus
    /// its copy allocation (spec 34 TIME-08, TEXT): the UTF-8 bytes of a STRING, two bytes per
    /// UTF-16 code unit of a WSTRING. Compositions without a work allowance charge nothing.
    fn charge_text_result(&self, _result: &Value) -> Result<(), crate::error::RuntimeError> {
        Ok(())
    }

    /// Charge the text `text`, a `WSTRING` when `wide`, that a standard text function made, as
    /// [`Self::charge_text_result`] charges its value (C1-12).
    fn charge_new_text(&self, _text: &str, _wide: bool) -> Result<(), crate::error::RuntimeError> {
        Ok(())
    }

    /// Admit an owned reference path before an address instruction copies it.
    fn before_reference_path_clone(
        &self,
        _reference: crate::value::ValueRefView<'_>,
    ) -> Result<(), crate::error::RuntimeError> {
        Ok(())
    }

    /// An empty buffer for a reference's path: one the composition keeps, so that after the first
    /// scan a reference allocates nothing (spec 34 ARCH-05), else a new one.
    fn take_reference_path(&self) -> crate::value::RefPath {
        crate::value::RefPath::new()
    }

    /// An empty buffer for an index segment's subscripts, kept as [`Self::take_reference_path`]'s.
    fn take_reference_indices(&self) -> crate::value::RefIndices {
        crate::value::RefIndices::new()
    }

    /// Give back the buffers of a reference path the execution has used.
    fn recycle_reference_path(&self, _path: crate::value::RefPath) {}

    /// Admit backing storage and copy work before extending an owned reference.
    fn before_reference_path_growth(
        &self,
        _bytes: usize,
        _work: usize,
    ) -> Result<(), crate::error::RuntimeError> {
        Ok(())
    }

    /// Normalize an assignment using the composition's declared type policy.
    fn normalize_assignment(
        &self,
        module: &super::module::VmModule,
        ty: u32,
        value: Value,
    ) -> Result<Value, crate::error::RuntimeError> {
        super::type_policy::normalize_vm_value_for_type(module, ty, value)
    }
    /// Charge allocations needed to update a selected field/element before mutation.
    fn before_path_write(
        &self,
        _root: &Value,
        _path: &[RefSegment],
        _input: &Value,
    ) -> Result<(), crate::error::RuntimeError> {
        Ok(())
    }
    /// Admit the allocation of a path write into a stored view (C1-9d2a, item (a)): the view form of
    /// [`ReferenceContext::before_path_write`]. A `Value` view is admitted as that method admits it; a
    /// cell view is admitted by a composition that reads cells, so this default refuses one.
    fn before_view_path_write(
        &self,
        root: &ValueView<'_>,
        path: &[RefSegment],
        input: &Value,
    ) -> Result<(), crate::error::RuntimeError> {
        match root {
            ValueView::Value(value) => self.before_path_write(value, path, input),
            ValueView::Cells { .. } | ValueView::Bits { .. } => {
                Err(crate::error::RuntimeError::InvalidExecutionState)
            }
        }
    }
    /// Final operational check after detached staging and before installed mutation.
    fn before_write_commit(&self) -> Result<(), crate::error::RuntimeError> {
        Ok(())
    }
    /// Journal an actual native copy-back destination immediately before mutation.
    fn before_output_write(
        &mut self,
        _frame: &super::VmFrame,
        _reference: crate::value::ValueRefView<'_>,
    ) -> Result<(), crate::error::RuntimeError> {
        Ok(())
    }
    /// Admit the work and allocation needed to materialize this value.
    #[cfg_attr(all(feature = "std", feature = "test-support"), track_caller)]
    fn before_value_clone(&self, _value: &Value) -> Result<(), crate::error::RuntimeError> {
        Ok(())
    }
    /// Admit the work and allocation of one copy of a stored view (C1-9d2a, item (a)): the view form of
    /// [`ReferenceContext::before_value_clone`]. A `Value` view is admitted as that method admits its
    /// value; a cell view is admitted by a composition that reads cells, so this default refuses one.
    fn before_view_clone(&self, view: &ValueView<'_>) -> Result<(), crate::error::RuntimeError> {
        match view {
            ValueView::Value(value) => self.before_value_clone(value),
            ValueView::Cells { .. } | ValueView::Bits { .. } => {
                Err(crate::error::RuntimeError::InvalidExecutionState)
            }
        }
    }
    /// The place of `view` (C1-10c), when a store may copy the value it shows as bytes: an array or
    /// a structure in a flat block whose type holds no text, reference, instance or overlap. A
    /// composition that holds no flat block has none.
    fn place_of(&self, _view: &ValueView<'_>) -> Option<super::Place> {
        None
    }
    /// The view of the compound `place` stands for, read from the flat block it lies in (C1-10c).
    fn place_view(&self, _place: super::Place) -> Option<ValueView<'_>> {
        None
    }
    /// Where a store through `reference` copies `place` (C1-10c): the destination's bytes, when
    /// they lie in the flat block and hold the place's type, which `declared`, the type the store
    /// normalizes to when it has one, must be too. `None` when the store is given the value the
    /// place stands for instead.
    fn place_destination(
        &self,
        _reference: crate::value::ValueRefView<'_>,
        _place: super::Place,
        _declared: Option<u32>,
    ) -> Option<super::PlaceDestination> {
        None
    }
    /// The checks and charges of [`ReferenceContext::check_reference_write`] for a write of the
    /// compound `place` stands for (C1-10c).
    fn check_place_write(
        &self,
        _reference: crate::value::ValueRefView<'_>,
        _place: super::Place,
    ) -> Result<(), crate::error::RuntimeError> {
        Err(crate::error::RuntimeError::InvalidExecutionState)
    }
    /// Copy the bytes of `place` to `destination` (C1-10c); `Ok(false)` when the block refuses
    /// either range. A copy into a structure nested in a backing is charged and checked first, as a
    /// write of the value there is, and may be refused (C1-15b).
    fn copy_place(
        &mut self,
        _place: super::Place,
        _destination: super::PlaceDestination,
    ) -> Result<bool, crate::error::RuntimeError> {
        Ok(false)
    }
    /// Charge what normalizing the value `place` stands for to its own type charges, so that a copy
    /// of its bytes charges what a store of the value does (C1-12): the capacity checks of its
    /// texts. A composition that holds no flat block has no place.
    fn charge_place_normalization(
        &self,
        _place: super::Place,
    ) -> Result<(), crate::error::RuntimeError> {
        Ok(())
    }
    /// The bytes of `place`, which a call that leaves it on the operand stack copies for the store
    /// that comes after the call (C1-19b). `None`, the default, has the place read into its value.
    fn place_copy(&self, _place: super::Place) -> Option<&[u8]> {
        None
    }
    /// Charge what normalizing the value the copy `bytes` of `place` stands for to its own type
    /// charges, as [`ReferenceContext::charge_place_normalization`] charges it for the place
    /// (C1-19b).
    fn charge_copy_normalization(
        &self,
        _bytes: &[u8],
        _place: super::Place,
    ) -> Result<(), crate::error::RuntimeError> {
        Ok(())
    }
    /// The value of `type_id` that the copy `bytes` stands for, as a read of the place it was
    /// copied from gives it (C1-19b). Only a composition that copies places is asked.
    fn copy_value(
        &self,
        _bytes: &[u8],
        _type_id: u32,
    ) -> Result<Value, crate::error::RuntimeError> {
        Err(crate::error::RuntimeError::InvalidExecutionState)
    }
    /// Write the copy `bytes` to `destination` as [`ReferenceContext::copy_place`] copies a place
    /// there (C1-19b); `Ok(false)` when the block refuses the destination.
    fn write_copy(
        &mut self,
        _bytes: &[u8],
        _destination: super::PlaceDestination,
    ) -> Result<bool, crate::error::RuntimeError> {
        Ok(false)
    }
    /// Store `text`, a `WSTRING` when `wide`, through `reference` into a text leaf of the flat
    /// store, normalized to `declared` and checked as a store of its value is, the check first when
    /// `check_first`, as a dynamic store takes them (C1-12). `None` when the store is given the
    /// value instead; `Some(Ok(false))` when the block refuses the write.
    fn store_text(
        &mut self,
        _reference: crate::value::ValueRefView<'_>,
        _text: &str,
        _wide: bool,
        _declared: Option<u32>,
        _check_first: bool,
    ) -> Option<Result<bool, crate::error::RuntimeError>> {
        None
    }
    /// Charge a copy of `text`, a `WSTRING` when `wide`, as [`Self::before_value_clone`] charges a
    /// copy of its value (C1-12). Only a composition that makes temporary texts copies one.
    fn before_text_clone(
        &self,
        _text: &str,
        _wide: bool,
    ) -> Result<(), crate::error::RuntimeError> {
        Ok(())
    }
    /// The enumeration operand that a load of `view` pushes (C1-19a): an enumeration leaf of the
    /// flat store, as its type and number. `None`, the default, has the load push the value.
    fn enum_operand(&self, _view: &crate::memory::view::ValueView<'_>) -> Option<super::EnumTemp> {
        None
    }
    /// The enumeration operand that a load of the constant `value` pushes (C1-19a). `None`, the
    /// default, has the load push a copy of the value.
    fn enum_constant(&self, _value: &Value) -> Option<super::EnumTemp> {
        None
    }
    /// The value the enumeration operand `temp` stands for (C1-19a). Only a composition that
    /// pushes enumeration operands is asked.
    fn enum_value(&self, _temp: super::EnumTemp) -> Result<Value, crate::error::RuntimeError> {
        Err(crate::error::RuntimeError::InvalidExecutionState)
    }
    /// Whether the enumeration operands `left` and `right` are equal, as `=` compares their values:
    /// the same type and the same number (C1-19a).
    fn enums_equal(&self, left: super::EnumTemp, right: super::EnumTemp) -> bool {
        left == right
    }
    /// Charge a copy of the enumeration operand `temp` as [`Self::before_value_clone`] charges a
    /// copy of its value (C1-19a). Only a composition that pushes enumeration operands copies one.
    fn before_enum_clone(&self, _temp: super::EnumTemp) -> Result<(), crate::error::RuntimeError> {
        Ok(())
    }
    /// Store the enumeration `temp` through `reference` into an enumeration leaf of the flat store,
    /// checked against `declared`, the type the store normalizes to when it has one, and as a write,
    /// as a store of its value is (C1-19a). Neither check charges, and an enumeration they would
    /// refuse is stored as its value, so their order is a store's of its value. `None` when the
    /// store is given the value instead; `Some(Ok(false))` when the block refuses the write.
    fn store_enum(
        &mut self,
        _reference: crate::value::ValueRefView<'_>,
        _temp: super::EnumTemp,
        _declared: Option<u32>,
    ) -> Option<Result<bool, crate::error::RuntimeError>> {
        None
    }
    /// `reference` in its canonical form (C1-11d): a composition with a flat store makes a
    /// reference to a whole writable target in its persistent block flat, and gives the path's
    /// buffers back; any other reference, and every reference of another composition, comes back
    /// as it is.
    fn flatten_reference(&self, reference: crate::value::ValueRef) -> crate::value::ValueRef {
        reference
    }
    /// Whether slot `slot` of `frame` lies in the frame's bytes on the activation stack (C1-11): it
    /// is then read and written through the store as `MemoryLocation::Local` of the frame's
    /// activation, not as a value of the frame. A composition without an activation stack has none.
    fn frame_slot_in_block(&self, _frame: &super::VmFrame, _slot: usize) -> bool {
        false
    }
    /// The value of slot `slot` of `frame` read from the frame's bytes on the activation stack, or
    /// `None` when the frame keeps the slot as a value (C1-11).
    fn read_frame_slot(
        &self,
        _frame: &super::VmFrame,
        _slot: usize,
    ) -> Option<Result<Value, crate::error::RuntimeError>> {
        None
    }
    /// The frame `frame` has on the activation stack, for reading its outputs after it finishes
    /// (C1-11).
    fn frame_block(&self, _frame: &super::VmFrame) -> Option<crate::memory::regions::FrameBlock> {
        None
    }
    /// The value of slot `slot` of the finished frame `block`, read from its bytes, or `None` when
    /// the frame kept the slot as a value (C1-11).
    fn read_finished_slot(
        &self,
        _block: crate::memory::regions::FrameBlock,
        _slot: usize,
    ) -> Option<Result<Value, crate::error::RuntimeError>> {
        None
    }
    /// Admit a private call-transaction snapshot: a VAR_IN_OUT copy-in. IEC passes
    /// IN_OUT by reference, so a work-charging composition admits only its
    /// allocation (spec 34 TIME-08). Other compositions treat it as a copy.
    fn before_transaction_snapshot(&self, value: &Value) -> Result<(), crate::error::RuntimeError> {
        self.before_value_clone(value)
    }
    /// Check mutability, visibility and escaping references before a write.
    fn check_reference_write(
        &self,
        reference: crate::value::ValueRefView<'_>,
        value: &Value,
    ) -> Result<(), crate::error::RuntimeError>;
    /// Check lifetime and lexical visibility without allocating an address.
    fn check_reference_read(
        &self,
        reference: crate::value::ValueRefView<'_>,
    ) -> Result<(), crate::error::RuntimeError>;
    /// Resolve the active initializer's private result slot and borrowed path.
    fn initializer_reference<'a>(
        &self,
        initializer_id: u32,
        path: &'a [RefSegment],
    ) -> Result<crate::value::ValueRefView<'a>, super::VmTrap>;
    /// Map an artifact owner to the physical invocation instance.
    fn resolve_instance_owner(
        &self,
        encoded: u32,
        frame: &super::VmFrame,
    ) -> Result<crate::memory::InstanceId, super::VmTrap> {
        Ok(if frame.instance_owner == Some(encoded) {
            frame
                .runtime_instance
                .unwrap_or(crate::memory::InstanceId(encoded))
        } else {
            crate::memory::InstanceId(encoded)
        })
    }
    /// Borrow the composition's shared storage.
    fn storage(&self) -> &Self::Store;
    /// Access internal storage after the shared policy checks.
    fn storage_mut(&mut self) -> &mut Self::Store;
    /// Commit a value already normalized and lifetime-checked by the shared store path.
    /// Preserve operational failures; false means only an invalid storage target. `capacity` is
    /// the declared string capacity of the target element write, in UTF-8 bytes for `STRING` and
    /// UTF-16 code units for `WSTRING`.
    fn write_typed_storage(
        &mut self,
        location: MemoryLocation,
        offset: usize,
        path: &[RefSegment],
        value: Value,
        capacity: usize,
    ) -> Result<bool, crate::error::RuntimeError>;
    /// Write an already checked and normalized value into a local path.
    fn write_typed_local(&self, root: &mut Value, path: &[RefSegment], value: Value) -> bool;
    /// Checked local assignment; legacy compositions retain their boolean compatibility hook.
    fn write_typed_local_checked(
        &self,
        root: &mut Value,
        path: &[RefSegment],
        value: Value,
        capacity: usize,
    ) -> Result<bool, crate::error::RuntimeError> {
        let _ = capacity;
        Ok(self.write_typed_local(root, path, value))
    }
    /// The declared capacity in storage units of a bounded `STRING`/`WSTRING` that the reference's
    /// path selects an element of, or the implementer maximum when the target is not such an
    /// element. A character write is cut to this capacity (spec 02 §6). Compositions without
    /// declared string metadata return [`crate::value::MAX_STRING_CAPACITY`].
    fn reference_string_capacity(&self, _reference: crate::value::ValueRefView<'_>) -> usize {
        crate::value::MAX_STRING_CAPACITY
    }
}

/// Read-only invocation data borrowed for one instruction observer callback.
/// It is neither an executable frame nor a mutation capability; its local slice
/// cannot outlive the callback's borrow of the currently executing invocation.
#[derive(Clone, Copy, Debug)]
pub struct StatementFrame<'a> {
    activation: Option<crate::memory::FrameId>,
    pou_id: Option<u32>,
    runtime_instance: Option<crate::memory::InstanceId>,
    locals: &'a [crate::value::Value],
    storage: Option<&'a VariableStorage>,
    /// The flat store the engine holds, where the frames' typed locals, the globals and the
    /// instances lie (specification 34 §6.3; C1-17).
    cells: Option<crate::memory::cell_access::CellReader<'a>>,
}

impl<'a> StatementFrame<'a> {
    pub(crate) fn from_frame(frame: &'a super::VmFrame) -> Self {
        Self {
            activation: frame.activation,
            pou_id: frame.pou_id,
            runtime_instance: frame.runtime_instance,
            locals: &frame.locals,
            storage: None,
            cells: None,
        }
    }

    pub(crate) fn with_call_stack<'b>(self, storage: &'b VariableStorage) -> StatementFrame<'b>
    where
        'a: 'b,
    {
        StatementFrame {
            storage: Some(storage),
            ..self
        }
    }

    /// The view on the flat store `cells`, which the frames' typed locals, the globals and the
    /// instances are read from where they lie (C1-17).
    pub(crate) fn with_cells(self, cells: crate::memory::cell_access::CellReader<'a>) -> Self {
        Self {
            cells: Some(cells),
            ..self
        }
    }
    /// Current invocation identity. Legacy fixtures and initializer bodies
    /// without a lexical invocation may have no activation identity.
    pub fn activation(self) -> Option<crate::memory::FrameId> {
        self.activation
    }
    /// Artifact identity of the active POU, absent for ownerless initializer code.
    pub fn pou_id(self) -> Option<u32> {
        self.pou_id
    }
    /// Physical instance receiving the invocation, if it has one.
    pub fn runtime_instance(self) -> Option<crate::memory::InstanceId> {
        self.runtime_instance
    }
    /// Current local values, including bound parameters and the return slot. On the flat store a
    /// typed local lies on the activation stack and its entry here is `Null`: [`Self::local`] reads
    /// every local where it lies.
    pub fn locals(self) -> &'a [crate::value::Value] {
        self.locals
    }

    /// The value of local slot `slot` as the program holds it: read from the frame's bytes on the
    /// activation stack when the flat store keeps the slot there, and otherwise the frame's value
    /// (specification 34 §6.3; C1-17). On the flat store each flat reference it holds, nested ones
    /// too, is named by the variable and the path that hold its target, as the value store names it
    /// (C1-11d, C1-19c4).
    ///
    /// # Errors
    ///
    /// [`crate::error::RuntimeError::InvalidExecutionState`] for a slot the frame does not have, and
    /// the error of a value that does not materialize.
    pub fn local(self, slot: usize) -> Result<crate::value::Value, crate::error::RuntimeError> {
        let value = if let (Some(cells), Some(activation), Some(pou)) =
            (self.cells, self.activation, self.pou_id)
            && let Some(view) = cells.frame_local(activation, pou, slot)
        {
            view.materialize()?
        } else {
            self.locals
                .get(slot)
                .cloned()
                .ok_or(crate::error::RuntimeError::InvalidExecutionState)?
        };
        match self.cells {
            Some(cells) => cells.named_value(value),
            None => Ok(value),
        }
    }

    /// The PLC storage the statement sees, as one owned copy that holds no named frame: a copy of
    /// `storage`, the storage the engine passed with this frame, or on the flat store its `Value`
    /// half with every global and every instance read from the blocks, as the value store would
    /// hold them (specification 34 §6.3; C1-17). The debugger makes it when a stop, a condition or
    /// a logpoint needs it, never at every statement (specification 11 §6.13.1).
    ///
    /// # Errors
    ///
    /// The error of a value that does not materialize.
    pub fn storage_copy(
        self,
        storage: &VariableStorage,
    ) -> Result<VariableStorage, crate::error::RuntimeError> {
        match self.cells {
            Some(cells) => cells.project_storage(),
            None => Ok(storage.clone()),
        }
    }

    /// Visit live suspended ancestors followed by the currently executing frame.
    /// Values and identities are borrowed from the owner; no snapshot allocation
    /// or mutable frame capability is introduced into the execution path.
    /// Each returned frame has a single-frame view rather than a recursive stack.
    pub fn call_stack(self) -> impl Iterator<Item = StatementFrame<'a>> {
        self.storage
            .into_iter()
            .flat_map(VariableStorage::suspended_execution_activations)
            .filter_map(move |(activation, owner, locals)| {
                if Some(activation) == self.activation {
                    return None;
                }
                Some(StatementFrame {
                    activation: Some(activation),
                    pou_id: Some(owner.pou_id),
                    runtime_instance: owner.runtime_instance,
                    locals,
                    storage: None,
                    cells: self.cells,
                })
            })
            .chain(core::iter::once(StatementFrame {
                storage: None,
                ..self
            }))
    }
}

/// Legacy local-reference encoding, retained only at the hosted compatibility boundary.
pub const VM_LOCAL_SENTINEL_FRAME_ID: u32 = u32::MAX;

/// Host extensions around the single bytecode dispatcher.
pub trait ExecutionContext: super::call::context::CallContext {
    /// The buffers of a call of a built-in text function, taken for the call (C1-12). `None`, the
    /// default, has every text function bind values.
    fn take_text_buffers(&mut self) -> Option<super::TextBuffers> {
        None
    }
    /// Give back the buffers [`Self::take_text_buffers`] took.
    fn return_text_buffers(&mut self, _buffers: super::TextBuffers) {}
    /// Whether texts the program makes go into the operand stack's text area: the composition
    /// stores and passes them there without a value (C1-12). `false`, the default, keeps values.
    fn keeps_texts(&self) -> bool {
        false
    }
    /// Initialize one invocation using the existing shared work allowance.
    fn initialize_frame(
        &mut self,
        module: &super::module::VmModule,
        frame: &mut super::VmFrame,
        depth: u32,
    ) -> Result<(), crate::error::RuntimeError>;
    /// Maximum admitted logical call depth, including the enclosing invocation.
    fn call_depth_limit(&self) -> usize {
        super::VM_MAX_CALL_DEPTH
    }
    /// Admit continuation storage bytes before fallible allocation and relocation.
    /// The work argument is frame-stack bookkeeping (TIME-08) and is not charged.
    fn charge_call_storage(
        &self,
        _bytes: usize,
        _work: usize,
    ) -> Result<(), crate::error::RuntimeError> {
        Ok(())
    }
    /// Return the single allowance shared by instructions and nested helpers.
    fn execution_budget(&self) -> &super::budget::ExecutionBudget;
    /// Sample the physical deadline at an entry or successful completion.
    fn check_execution_deadline(&self) -> Result<(), crate::error::RuntimeError> {
        #[cfg(all(feature = "std", feature = "test-support"))]
        let _timing =
            super::diagnostic_timing::begin(super::diagnostic_timing::Phase::DeadlineCheck);
        if self.deadline_exceeded() {
            Err(super::VmTrap::DeadlineExceeded.into_runtime_error())
        } else {
            Ok(())
        }
    }
    /// Start a root allowance or join a nested one, then check its deadline.
    fn begin_execution(
        &self,
        entry: super::budget::ExecutionEntry,
        limit: usize,
    ) -> Result<(), crate::error::RuntimeError> {
        if entry == super::budget::ExecutionEntry::Root {
            self.execution_budget().reset(limit);
        }
        self.check_execution_deadline()
    }
    /// Consume shared fuel and poll the deadline only at its bounded stride.
    #[cfg_attr(all(feature = "std", feature = "test-support"), track_caller)]
    fn charge_execution_work(&self, units: usize) -> Result<(), crate::error::RuntimeError> {
        #[cfg(all(feature = "std", feature = "test-support"))]
        let _timing = super::diagnostic_timing::begin(super::diagnostic_timing::Phase::WorkCharge);

        #[cfg(not(all(feature = "std", feature = "test-support")))]
        {
            if self.execution_budget().charge(units)? {
                self.check_execution_deadline()?;
            }
            Ok(())
        }

        #[cfg(all(feature = "std", feature = "test-support"))]
        {
            let caller = if super::diagnostic_timing::callsite_accounting_log_enabled() {
                Some(core::panic::Location::caller())
            } else {
                None
            };
            let result = match self.execution_budget().charge(units) {
                Err(error) => Err(error),
                Ok(true) => self.check_execution_deadline(),
                Ok(false) => Ok(()),
            };
            if let Some(caller) = caller {
                let (consumed, outcome) = match &result {
                    Ok(()) => (units, super::diagnostic_timing::WorkChargeOutcome::Charged),
                    Err(error) => {
                        let outcome = super::diagnostic_timing::work_outcome(error);
                        let consumed = if matches!(
                            error,
                            crate::error::RuntimeError::ExecutionLimit {
                                kind: crate::error::ExecutionLimitKind::Work,
                                ..
                            }
                        ) {
                            0
                        } else {
                            units
                        };
                        (consumed, outcome)
                    }
                };
                super::diagnostic_timing::record_work_charge(caller, units, consumed, outcome);
            }
            result
        }
    }
    /// Invalidate an invocation and release owned temporaries even when ownership checks fail.
    /// Return that failure only after all identities are retired.
    fn retire_frame(&mut self, frame: &super::VmFrame) -> Result<(), crate::error::RuntimeError>;
    /// Reject values whose references would escape the returning invocation.
    fn check_frame_return(
        &self,
        frame: &super::VmFrame,
        value: &Value,
    ) -> Result<(), crate::error::RuntimeError>;
    /// Charge and check the result `frame` keeps in slot 0 of its bytes as reading and returning
    /// its value is charged and checked, when a store copies its bytes, so that the caller takes a
    /// copy of them (C1-19c4); `None` when the frame keeps any other result. A composition without
    /// an activation stack keeps none.
    fn check_return_place(
        &self,
        _frame: &super::VmFrame,
    ) -> Option<Result<(), crate::error::RuntimeError>> {
        None
    }
    /// Push a copy of the bytes of the result the finished frame `block` keeps in slot 0, which
    /// [`Self::check_return_place`] admitted, onto `stack` (C1-19c4).
    fn push_return_copy(
        &self,
        _block: crate::memory::regions::FrameBlock,
        _stack: &mut super::stack::OperandStack,
    ) -> Result<(), crate::error::RuntimeError> {
        Err(crate::error::RuntimeError::InvalidExecutionState)
    }
    /// Build the typed value of `DEFAULT_TYPED`, `COERCE_INIT_VALUE` or `APPLY_INIT_VALUE` (`opcode`)
    /// where the staged result of the active initializer `initializer_id` lies, and push its place,
    /// when the next instruction stores it as that result (C1-13). `Ok(false)`, with nothing popped
    /// or charged, when the value is built as a value instead.
    fn construct_result_in_place(
        &mut self,
        _frame: &mut super::VmFrame,
        _opcode: u8,
        _operand: u32,
        _stack: &mut super::OperandStack,
        _initializer_id: u32,
    ) -> Result<bool, crate::error::RuntimeError> {
        Ok(false)
    }
    /// Execute a typed construction opcode through the same dispatcher.
    #[allow(clippy::too_many_arguments)]
    fn execute_initialization_opcode(
        &mut self,
        module: &super::module::VmModule,
        frame: &mut super::VmFrame,
        opcode: u8,
        operand: u32,
        stack: &mut super::OperandStack,
        depth: u32,
    ) -> Result<(), crate::error::RuntimeError>;
    /// Sample the composition's physical deadline, independent of PLC time.
    fn deadline_exceeded(&self) -> bool;
    /// Notify optional debug/profiling hooks at this instruction boundary.
    fn on_statement(
        &mut self,
        module: &super::module::VmModule,
        pou_id: u32,
        pc: usize,
        depth: u32,
    );
    /// Notify an observer with the active invocation's read-only local view.
    /// Existing adapters retain their old hook through this default delegation.
    fn on_statement_with_frame(
        &mut self,
        module: &super::module::VmModule,
        pou_id: u32,
        pc: usize,
        depth: u32,
        _frame: StatementFrame<'_>,
    ) {
        self.on_statement(module, pou_id, pc, depth);
    }

    /// Invoke the observer without hiding a composition invariant failure.
    /// The dispatcher owns cleanup if this callback fails.
    fn on_statement_with_frame_checked(
        &mut self,
        module: &super::module::VmModule,
        pou_id: u32,
        pc: usize,
        depth: u32,
        frame: StatementFrame<'_>,
    ) -> Result<(), crate::error::RuntimeError> {
        self.on_statement_with_frame(module, pou_id, pc, depth, frame);
        Ok(())
    }
    /// Measure a runtime value under the same resource allowance.
    fn sizeof_value(&self, value: &Value) -> Result<u64, crate::error::RuntimeError>;
    /// Whether this context owns the iterative register activation pump.
    #[cfg(feature = "register-ir")]
    #[doc(hidden)]
    fn has_register_activation_pump(&self) -> bool {
        false
    }
    /// Start a prepared register activation without recursively executing it.
    /// The default keeps older `try_execute_optimized` adapters available.
    #[cfg(feature = "register-ir")]
    #[doc(hidden)]
    #[allow(clippy::too_many_arguments)]
    fn start_register_activation(
        &mut self,
        _module: &super::module::VmModule,
        _pou_id: u32,
        _entry_instance: Option<crate::memory::InstanceId>,
        _initial_locals: Option<&[Value]>,
        _owned_initial_locals: Option<&mut Option<alloc::vec::Vec<Value>>>,
        _parameter_values_present: Option<&[bool]>,
        _capture_return: bool,
        _depth: u32,
        _entry: super::budget::ExecutionEntry,
        _capture_result_locals: bool,
    ) -> Result<Option<super::RegisterActivation>, crate::error::RuntimeError> {
        Ok(None)
    }
    /// Advance one prepared register activation until it calls or returns.
    #[cfg(feature = "register-ir")]
    #[doc(hidden)]
    fn step_register_activation(
        &mut self,
        _module: &super::module::VmModule,
        _activation: &mut super::RegisterActivation,
    ) -> Result<super::RegisterActivationStep, crate::error::RuntimeError> {
        Err(crate::error::RuntimeError::InvalidExecutionState)
    }
    /// Recycle a completed activation's reserved register scratch.
    #[cfg(feature = "register-ir")]
    #[doc(hidden)]
    fn recycle_register_activation(
        &mut self,
        _activation: super::RegisterActivation,
    ) -> Result<(), crate::error::RuntimeError> {
        Err(crate::error::RuntimeError::InvalidExecutionState)
    }
    /// Clean and recycle an activation interrupted by a child fault.
    #[cfg(feature = "register-ir")]
    #[doc(hidden)]
    fn abort_register_activation(
        &mut self,
        _activation: super::RegisterActivation,
    ) -> Result<(), crate::error::RuntimeError> {
        Err(crate::error::RuntimeError::InvalidExecutionState)
    }
    /// Acquire cleared scratch buffers for a dispatch entry.
    fn take_execution_buffers(&mut self) -> super::dispatch::ExecutionBuffers {
        super::dispatch::ExecutionBuffers::default()
    }
    /// Return cleared scratch buffers after entry cleanup.
    fn recycle_execution_buffers(&mut self, _buffers: super::dispatch::ExecutionBuffers) {}
    /// The local slots of a new stack activation, `count` NULL values. A context with
    /// payloads reserved at construction hands one out instead of allocating.
    fn take_frame_locals(&mut self, count: usize) -> alloc::vec::Vec<Value> {
        alloc::vec![Value::Null; count]
    }
    /// Take back the local slots of a finished stack activation once nothing reads them.
    fn recycle_frame_locals(&mut self, _locals: alloc::vec::Vec<Value>) {}
    /// The parameter-presence bits of a new stack activation, none set. A context with payloads
    /// reserved at construction hands out a reserved vector instead of allocating.
    fn take_frame_presence(&mut self) -> alloc::vec::Vec<bool> {
        alloc::vec::Vec::new()
    }
    /// Take back the presence bits of a finished stack activation.
    fn recycle_frame_presence(&mut self, _present: alloc::vec::Vec<bool>) {}
}

#[cfg(test)]
#[path = "context/tests.rs"]
mod tests;
