//! Cached call target handles and their checked read/write operations.
use super::{
    is_current_location, is_vm_local_sentinel, peek_vm_reference, read_vm_reference,
    write_vm_reference,
};
use crate::memory::view::{StoreAccess, StoreMutate, ValueView};
use crate::vm::call::context::{CallContext, RegisterValueOpKind};
use crate::vm::dispatch_refs::owned_reference;
use crate::vm::{VmFrame, VmTrap};
use crate::{
    error::RuntimeError,
    memory::{Generation, InstanceId, MemoryLocation},
    value::{normalize_assignment_for_target, Value, ValueRef},
};
use alloc::{borrow::Cow, format, vec::Vec};
use smol_str::SmolStr;

/// Cached source of a function-block output value.
#[derive(Debug, Clone)]
pub enum VmFbOutSource {
    /// Resolved instance slot with no aggregate path.
    Direct {
        /// Runtime identity of the owning function block.
        instance_id: InstanceId,
        /// Slot index within the addressed storage domain.
        offset: usize,
    },
    /// A reference requiring shared path resolution.
    Reference(ValueRef),
}

impl VmFbOutSource {
    /// Give the buffers of this source's path back to `runtime` (spec 34 MEM-09; C1-19c4).
    pub(crate) fn recycle(self, runtime: &impl CallContext) {
        if let Self::Reference(reference) = self {
            runtime.recycle_reference_path(reference.path);
        }
    }

    /// The view of the bound value after reference-read policy checks (C1-9d2a, item (c)).
    pub fn read_checked<'a>(
        &'a self,
        runtime: &'a impl CallContext,
    ) -> Result<Option<ValueView<'a>>, VmTrap> {
        let reference = match self {
            Self::Direct {
                instance_id,
                offset,
            } => crate::value::ValueRefView {
                location: MemoryLocation::Instance(*instance_id),
                offset: *offset,
                generation: runtime.storage().generation(),
                path: &[],
            },
            Self::Reference(reference) => reference.as_view(),
        };
        runtime
            .check_reference_read(reference)
            .map_err(VmTrap::Runtime)?;
        Ok(self.read(runtime))
    }

    /// Borrow the bound storage value; callers must perform required policy checks.
    pub fn read<'a>(&'a self, runtime: &'a impl CallContext) -> Option<ValueView<'a>> {
        match self {
            Self::Direct {
                instance_id,
                offset,
            } => runtime
                .storage()
                .read_slot(MemoryLocation::Instance(*instance_id), *offset)
                .ok(),
            Self::Reference(reference) => {
                runtime.storage().read_reference(reference.as_view()).ok()
            }
        }
    }
}

/// Caller destination retained for ordered output copy-back.
#[derive(Debug, Clone)]
pub enum VmWriteTarget {
    /// A local slot in the active caller frame.
    CallerLocalDirect {
        /// Slot index within the addressed storage domain.
        offset: usize,
    },
    /// A resolved physical storage slot.
    DirectStorage {
        /// Physical storage domain for this direct target.
        location: MemoryLocation,
        /// Slot index within the addressed storage domain.
        offset: usize,
    },
    /// A reference requiring shared path resolution.
    Reference(ValueRef),
}

impl VmWriteTarget {
    /// Cache a direct slot when possible, retaining an owned path only when needed. A call binds
    /// its targets in the engine's kept buffers; the tests build targets with this.
    #[cfg(any(test, feature = "test-support"))]
    pub fn from_reference(reference: &ValueRef) -> Self {
        Self::direct(reference).unwrap_or_else(|| Self::Reference(reference.clone()))
    }

    /// The direct slot `reference` names, or the reference with its path copied into the buffers
    /// `runtime` keeps, so that binding a call's output allocates nothing after the first scan
    /// (spec 34 MEM-09; C1-19c4). The call gives them back by [`Self::recycle`] once the output is
    /// copied back.
    pub(crate) fn kept(runtime: &impl CallContext, reference: &ValueRef) -> Result<Self, VmTrap> {
        match Self::direct(reference) {
            Some(target) => Ok(target),
            None => owned_reference(runtime, reference.as_view()).map(Self::Reference),
        }
    }

    /// Give the buffers of this target's path back to `runtime` (C1-19c4).
    pub(crate) fn recycle(self, runtime: &impl CallContext) {
        if let Self::Reference(reference) = self {
            runtime.recycle_reference_path(reference.path);
        }
    }

    /// The direct slot `reference` names, or `None` when it needs its path resolved.
    pub(super) fn direct(reference: &ValueRef) -> Option<Self> {
        if is_vm_local_sentinel(reference) && reference.path.is_empty() {
            return Some(Self::CallerLocalDirect {
                offset: reference.offset as usize,
            });
        }
        if reference.path.is_empty() {
            match reference.location {
                MemoryLocation::Global | MemoryLocation::Local(_) | MemoryLocation::Instance(_) => {
                    return Some(Self::DirectStorage {
                        location: reference.location,
                        offset: reference.offset as usize,
                    });
                }
                MemoryLocation::Io(_) | MemoryLocation::Retain | MemoryLocation::Flat { .. } => {}
            }
        }
        None
    }

    /// Borrow the resolved target path for policy checks and transaction planning.
    /// A direct slot is a slot of the call's own storage, so it carries that
    /// storage's `generation`; a reference keeps its own.
    pub fn reference_view(
        &self,
        caller_frame: &VmFrame,
        generation: Generation,
    ) -> crate::value::ValueRefView<'_> {
        match self {
            Self::CallerLocalDirect { offset } => crate::value::ValueRefView {
                location: MemoryLocation::Local(caller_frame.reference_frame_id()),
                offset: *offset,
                generation,
                path: &[],
            },
            Self::DirectStorage { location, offset } => crate::value::ValueRefView {
                location: *location,
                offset: *offset,
                generation,
                path: &[],
            },
            Self::Reference(reference) => reference.as_view(),
        }
    }

    /// The reference this target names: a reference target's own, borrowed, so that checking a
    /// write copies no path (C1-19c4), and a direct slot's, whose path is empty.
    fn reference(
        &self,
        caller_frame: &VmFrame,
        generation: Generation,
    ) -> Result<Cow<'_, ValueRef>, VmTrap> {
        let slot = |offset: usize| {
            u32::try_from(offset).map_err(|_| VmTrap::Runtime(RuntimeError::Overflow))
        };
        Ok(match self {
            Self::CallerLocalDirect { offset } => Cow::Owned(ValueRef {
                location: MemoryLocation::Local(caller_frame.reference_frame_id()),
                offset: slot(*offset)?,
                generation,
                path: Vec::new(),
            }),
            Self::DirectStorage { location, offset } => Cow::Owned(ValueRef {
                location: *location,
                offset: slot(*offset)?,
                generation,
                path: Vec::new(),
            }),
            Self::Reference(reference) => Cow::Borrowed(reference),
        })
    }

    /// The caller's slot this target names when the slot lies on the caller's frame on the
    /// activation stack (C1-11).
    fn caller_block_slot(
        &self,
        runtime: &impl CallContext,
        caller_frame: &VmFrame,
    ) -> Option<usize> {
        let offset = match self {
            Self::CallerLocalDirect { offset } => *offset,
            Self::DirectStorage { location, offset }
                if is_current_location(*location, caller_frame) =>
            {
                *offset
            }
            _ => return None,
        };
        runtime
            .frame_slot_in_block(caller_frame, offset)
            .then_some(offset)
    }

    /// Check destination visibility, lifetime and native-output policy.
    pub fn check_write(
        &self,
        runtime: &impl CallContext,
        caller_frame: &VmFrame,
        value: &Value,
    ) -> Result<(), VmTrap> {
        let reference = self.reference(caller_frame, runtime.storage().generation())?;
        runtime
            .check_native_write(Some(caller_frame), &reference, value)
            .map_err(VmTrap::Runtime)
    }

    /// The view of the target `reference` names, as [`Self::read`] reads it through the target a
    /// call binds for it, without building that target (C1-12).
    pub fn peek_reference<'a>(
        runtime: &'a impl CallContext,
        caller_frame: &'a VmFrame,
        reference: &ValueRef,
    ) -> Result<ValueView<'a>, VmTrap> {
        if reference.path.is_empty() {
            if is_vm_local_sentinel(reference) {
                let target = Self::CallerLocalDirect {
                    offset: reference.offset as usize,
                };
                return target.peek(runtime, caller_frame);
            }
            if let MemoryLocation::Global | MemoryLocation::Local(_) | MemoryLocation::Instance(_) =
                reference.location
            {
                let target = Self::DirectStorage {
                    location: reference.location,
                    offset: reference.offset as usize,
                };
                return target.peek(runtime, caller_frame);
            }
        }
        peek_vm_reference(runtime, caller_frame, reference)
    }

    /// Borrow the current target value without cloning its path.
    pub fn peek<'a>(
        &self,
        runtime: &'a impl CallContext,
        caller_frame: &'a VmFrame,
    ) -> Result<ValueView<'a>, VmTrap> {
        runtime
            .check_reference_read(self.reference_view(caller_frame, runtime.storage().generation()))
            .map_err(VmTrap::Runtime)?;
        // A caller's slot on the activation stack is read through the store (C1-11).
        if let Some(offset) = self.caller_block_slot(runtime, caller_frame) {
            return runtime
                .storage()
                .read_slot(
                    MemoryLocation::Local(caller_frame.reference_frame_id()),
                    offset,
                )
                .map_err(|_| VmTrap::Runtime(RuntimeError::NullReference));
        }
        match self {
            Self::CallerLocalDirect { offset } => caller_frame
                .locals
                .get(*offset)
                .map(ValueView::Value)
                .ok_or_else(|| {
                    VmTrap::InvalidNativeCall(
                        format!(
                            "local reference offset {} out of range for VM frame (locals={})",
                            offset,
                            caller_frame.locals.len()
                        )
                        .into(),
                    )
                }),
            Self::DirectStorage { location, offset }
                if is_current_location(*location, caller_frame) =>
            {
                caller_frame
                    .locals
                    .get(*offset)
                    .map(ValueView::Value)
                    .ok_or_else(|| VmTrap::Runtime(RuntimeError::NullReference))
            }
            Self::DirectStorage { location, offset } => runtime
                .storage()
                .read_slot(*location, *offset)
                .map_err(|_| VmTrap::Runtime(RuntimeError::NullReference)),
            Self::Reference(reference) => peek_vm_reference(runtime, caller_frame, reference),
        }
    }

    /// Materialize the checked target value after charging any owned copy.
    pub fn read(
        &self,
        runtime: &mut impl CallContext,
        caller_frame: &VmFrame,
    ) -> Result<Value, VmTrap> {
        #[cfg(all(feature = "std", feature = "test-support"))]
        let _read = crate::vm::diagnostic_timing::attribution::read(match self {
            Self::CallerLocalDirect { .. } => {
                crate::vm::diagnostic_timing::CopyReadKind::CallerLocal
            }
            Self::DirectStorage { .. } => crate::vm::diagnostic_timing::CopyReadKind::DirectStorage,
            Self::Reference(_) => crate::vm::diagnostic_timing::CopyReadKind::Reference,
        });
        match self {
            Self::Reference(reference) => read_vm_reference(runtime, caller_frame, reference),
            Self::CallerLocalDirect { .. } | Self::DirectStorage { .. } => {
                let (value, cloned) = self
                    .peek(runtime, caller_frame)?
                    .materialize_flagged()
                    .map_err(VmTrap::Runtime)?;
                runtime
                    .before_value_clone(&value)
                    .map_err(VmTrap::Runtime)?;
                if cloned {
                    runtime.record_value_op(RegisterValueOpKind::ReadReference);
                }
                Ok(value)
            }
        }
    }

    /// Materialize the private VAR_IN_OUT snapshot of the target. Its allocation is
    /// admitted; its copy is call-transaction bookkeeping (spec 34 TIME-08).
    pub fn snapshot(
        &self,
        runtime: &mut impl CallContext,
        caller_frame: &VmFrame,
    ) -> Result<Value, VmTrap> {
        #[cfg(all(feature = "std", feature = "test-support"))]
        let _read = crate::vm::diagnostic_timing::attribution::read(match self {
            Self::CallerLocalDirect { .. } => {
                crate::vm::diagnostic_timing::CopyReadKind::CallerLocal
            }
            Self::DirectStorage { .. } => crate::vm::diagnostic_timing::CopyReadKind::DirectStorage,
            Self::Reference(_) => crate::vm::diagnostic_timing::CopyReadKind::Reference,
        });
        match self.peek(runtime, caller_frame) {
            Ok(view) => {
                let (value, cloned) = view.materialize_flagged().map_err(VmTrap::Runtime)?;
                runtime
                    .before_transaction_snapshot(&value)
                    .map_err(VmTrap::Runtime)?;
                if cloned {
                    runtime.record_value_op(RegisterValueOpKind::ReadReference);
                }
                Ok(value)
            }
            // A string element synthesizes its character through the checked read.
            Err(VmTrap::Runtime(RuntimeError::NullReference)) => self.read(runtime, caller_frame),
            Err(error) => Err(error),
        }
    }

    /// Store a value through the bound target; the caller owns type normalization.
    pub fn write(
        &self,
        runtime: &mut impl CallContext,
        caller_frame: &mut VmFrame,
        value: Value,
    ) -> Result<(), VmTrap> {
        self.check_write(runtime, caller_frame, &value)?;
        let generation = runtime.storage().generation();
        runtime
            .before_output_write(caller_frame, self.reference_view(caller_frame, generation))
            .map_err(VmTrap::Runtime)?;
        // A caller's slot on the activation stack is written through the store (C1-11).
        if let Some(offset) = self.caller_block_slot(runtime, caller_frame) {
            let location = MemoryLocation::Local(caller_frame.reference_frame_id());
            return runtime
                .storage_mut()
                .write_slot(location, offset, value)
                .then_some(())
                .ok_or_else(|| VmTrap::Runtime(RuntimeError::NullReference));
        }
        match self {
            Self::CallerLocalDirect { offset } => {
                let local_count = caller_frame.locals.len();
                let Some(slot) = caller_frame.locals.get_mut(*offset) else {
                    return Err(VmTrap::InvalidNativeCall(
                        format!(
                            "local reference offset {} out of range for VM frame (locals={local_count})",
                            offset,
                        )
                        .into(),
                    ));
                };
                *slot = value;
                Ok(())
            }
            Self::DirectStorage { location, offset }
                if is_current_location(*location, caller_frame) =>
            {
                let slot = caller_frame
                    .locals
                    .get_mut(*offset)
                    .ok_or_else(|| VmTrap::Runtime(RuntimeError::NullReference))?;
                *slot = value;
                Ok(())
            }
            Self::DirectStorage { location, offset } => runtime
                .storage_mut()
                .write_slot(*location, *offset, value)
                .then_some(())
                .ok_or_else(|| VmTrap::Runtime(RuntimeError::NullReference)),
            Self::Reference(reference) => {
                write_vm_reference(runtime, caller_frame, reference, value)
            }
        }
    }
}

/// Cached function-block member binding.
#[derive(Debug, Clone)]
pub enum VmFbFieldBinding {
    /// Resolved instance slot with no aggregate path.
    Direct {
        /// Runtime identity of the owning function block.
        instance_id: InstanceId,
        /// Slot index within the addressed storage domain.
        offset: usize,
    },
    /// A reference requiring shared path resolution.
    Reference(ValueRef),
}

impl VmFbFieldBinding {
    /// Resolve an instance member to a cached slot or reference.
    pub fn resolve(
        runtime: &impl CallContext,
        instance_id: InstanceId,
        field_name: &SmolStr,
    ) -> Result<Self, VmTrap> {
        if let Some(offset) = runtime
            .storage()
            .declared_instance_field_offset(instance_id, field_name.as_str())
        {
            return Ok(Self::Direct {
                instance_id,
                offset,
            });
        }

        runtime
            .storage()
            .ref_for_instance_recursive(instance_id, field_name.as_str())
            .map(Self::Reference)
            .ok_or_else(|| VmTrap::Runtime(RuntimeError::UndefinedField(field_name.clone())))
    }

    /// Borrow the bound value after reference-read policy checks.
    pub fn read_checked<'a>(
        &'a self,
        runtime: &'a impl CallContext,
    ) -> Result<Option<ValueView<'a>>, VmTrap> {
        let reference = match self {
            Self::Direct {
                instance_id,
                offset,
            } => crate::value::ValueRefView {
                location: MemoryLocation::Instance(*instance_id),
                offset: *offset,
                generation: runtime.storage().generation(),
                path: &[],
            },
            Self::Reference(reference) => reference.as_view(),
        };
        runtime
            .check_reference_read(reference)
            .map_err(VmTrap::Runtime)?;
        Ok(self.read(runtime))
    }

    /// Borrow the bound storage value; callers must perform required policy checks.
    pub fn read<'a>(&'a self, runtime: &'a impl CallContext) -> Option<ValueView<'a>> {
        match self {
            Self::Direct {
                instance_id,
                offset,
            } => runtime
                .storage()
                .read_slot(MemoryLocation::Instance(*instance_id), *offset)
                .ok(),
            Self::Reference(reference) => {
                runtime.storage().read_reference(reference.as_view()).ok()
            }
        }
    }

    pub(super) fn check_write(
        &self,
        runtime: &impl CallContext,
        value: &Value,
    ) -> Result<(), VmTrap> {
        let reference = match self {
            Self::Direct {
                instance_id,
                offset,
            } => ValueRef {
                location: MemoryLocation::Instance(*instance_id),
                offset: u32::try_from(*offset)
                    .map_err(|_| VmTrap::Runtime(RuntimeError::Overflow))?,
                generation: runtime.storage().generation(),
                path: Vec::new(),
            },
            Self::Reference(reference) => reference.clone(),
        };
        runtime
            .check_native_write(None, &reference, value)
            .map_err(VmTrap::Runtime)
    }

    /// Store through the admitted engine writer, preserving typed physical faults.
    /// The caller owns parameter normalization and native write-policy checks.
    pub fn write_checked(
        &self,
        runtime: &mut impl CallContext,
        value: Value,
    ) -> Result<bool, VmTrap> {
        let value = match self.read(runtime) {
            Some(ValueView::Value(target)) => normalize_assignment_for_target(target, value),
            // A cell target is normalized against its scalar, which reads without allocating.
            Some(cells) => match cells.scalar().map_err(VmTrap::Runtime)? {
                Some(target) => normalize_assignment_for_target(&target, value),
                None => {
                    let target = cells.materialize().map_err(VmTrap::Runtime)?;
                    normalize_assignment_for_target(&target, value)
                }
            },
            None => value,
        };
        let (location, offset, path, capacity) = match self {
            Self::Direct {
                instance_id,
                offset,
            } => (
                MemoryLocation::Instance(*instance_id),
                *offset,
                &[][..],
                crate::value::MAX_STRING_CAPACITY,
            ),
            Self::Reference(reference) => (
                reference.location,
                reference.offset as usize,
                reference.path.as_slice(),
                runtime.reference_string_capacity(reference.as_view()),
            ),
        };
        runtime
            .write_typed_storage(location, offset, path, value, capacity)
            .map_err(VmTrap::Runtime)
    }

    /// Compatibility wrapper for consumers whose API cannot expose a typed fault.
    #[cfg(feature = "test-support")]
    pub fn write(&self, runtime: &mut impl CallContext, value: Value) -> bool {
        self.write_checked(runtime, value).unwrap_or(false)
    }

    /// Retain a copy of this binding as a function-block output source, for the tests; a call moves
    /// the binding with [`Self::into_out_source`].
    #[cfg(any(test, feature = "test-support"))]
    pub fn out_source(&self) -> VmFbOutSource {
        self.clone().into_out_source()
    }

    /// Retain this binding as a function-block output source, moving its reference rather than
    /// copying its path (C1-19c4).
    pub fn into_out_source(self) -> VmFbOutSource {
        match self {
            Self::Direct {
                instance_id,
                offset,
            } => VmFbOutSource::Direct {
                instance_id,
                offset,
            },
            Self::Reference(reference) => VmFbOutSource::Reference(reference),
        }
    }
}
