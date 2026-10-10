//! Cached call target handles and their checked read/write operations.
use super::{
    is_current_location, is_vm_local_sentinel, peek_vm_reference, read_vm_reference,
    write_vm_reference,
};
use crate::vm::call::context::{CallContext, RegisterValueOpKind};
use crate::vm::{materialize_borrowed_value, VmFrame, VmTrap};
use crate::{
    error::RuntimeError,
    memory::{InstanceId, MemoryLocation},
    value::{normalize_assignment_for_target, Value, ValueRef},
};
use alloc::{format, vec::Vec};
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
    /// Borrow the bound value after reference-read policy checks.
    pub fn read_checked<'a>(
        &self,
        runtime: &'a impl CallContext,
    ) -> Result<Option<&'a Value>, VmTrap> {
        let reference = match self {
            Self::Direct {
                instance_id,
                offset,
            } => crate::value::ValueRefView {
                location: MemoryLocation::Instance(*instance_id),
                offset: *offset,
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
    pub fn read<'a>(&self, runtime: &'a impl CallContext) -> Option<&'a Value> {
        match self {
            Self::Direct {
                instance_id,
                offset,
            } => runtime
                .storage()
                .read_instance_field_by_offset(*instance_id, *offset),
            Self::Reference(reference) => runtime.storage().read_by_ref_ref(reference),
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
    /// Cache a direct slot when possible, retaining an owned path only when needed.
    pub fn from_reference(reference: &ValueRef) -> Self {
        if is_vm_local_sentinel(reference) && reference.path.is_empty() {
            return Self::CallerLocalDirect {
                offset: reference.offset,
            };
        }
        if reference.path.is_empty() {
            match reference.location {
                MemoryLocation::Global | MemoryLocation::Local(_) | MemoryLocation::Instance(_) => {
                    return Self::DirectStorage {
                        location: reference.location,
                        offset: reference.offset,
                    };
                }
                MemoryLocation::Io(_) | MemoryLocation::Retain => {}
            }
        }
        Self::Reference(reference.clone())
    }

    /// Borrow the resolved target path for policy checks and transaction planning.
    pub fn reference_view(&self, caller_frame: &VmFrame) -> crate::value::ValueRefView<'_> {
        match self {
            Self::CallerLocalDirect { offset } => crate::value::ValueRefView {
                location: MemoryLocation::Local(caller_frame.reference_frame_id()),
                offset: *offset,
                path: &[],
            },
            Self::DirectStorage { location, offset } => crate::value::ValueRefView {
                location: *location,
                offset: *offset,
                path: &[],
            },
            Self::Reference(reference) => reference.as_view(),
        }
    }

    fn reference(&self, caller_frame: &VmFrame) -> ValueRef {
        match self {
            Self::CallerLocalDirect { offset } => ValueRef {
                location: MemoryLocation::Local(caller_frame.reference_frame_id()),
                offset: *offset,
                path: Vec::new(),
            },
            Self::DirectStorage { location, offset } => ValueRef {
                location: *location,
                offset: *offset,
                path: Vec::new(),
            },
            Self::Reference(reference) => reference.clone(),
        }
    }

    /// Check destination visibility, lifetime and native-output policy.
    pub fn check_write(
        &self,
        runtime: &impl CallContext,
        caller_frame: &VmFrame,
        value: &Value,
    ) -> Result<(), VmTrap> {
        runtime
            .check_native_write(Some(caller_frame), &self.reference(caller_frame), value)
            .map_err(VmTrap::Runtime)
    }

    /// Borrow the current target value without cloning its path.
    pub fn peek<'a>(
        &self,
        runtime: &'a impl CallContext,
        caller_frame: &'a VmFrame,
    ) -> Result<&'a Value, VmTrap> {
        runtime
            .check_reference_read(self.reference_view(caller_frame))
            .map_err(VmTrap::Runtime)?;
        match self {
            Self::CallerLocalDirect { offset } => {
                caller_frame.locals.get(*offset).ok_or_else(|| {
                    VmTrap::InvalidNativeCall(
                        format!(
                            "local reference offset {} out of range for VM frame (locals={})",
                            offset,
                            caller_frame.locals.len()
                        )
                        .into(),
                    )
                })
            }
            Self::DirectStorage { location, offset }
                if is_current_location(*location, caller_frame) =>
            {
                caller_frame
                    .locals
                    .get(*offset)
                    .ok_or(VmTrap::Runtime(RuntimeError::NullReference))
            }
            Self::DirectStorage { location, offset } => runtime
                .storage()
                .read_direct_slot_by_location(*location, *offset)
                .ok_or(VmTrap::Runtime(RuntimeError::NullReference)),
            Self::Reference(reference) => peek_vm_reference(runtime, caller_frame, reference),
        }
    }

    /// Materialize the checked target value after charging any owned copy.
    pub fn read(
        &self,
        runtime: &mut impl CallContext,
        caller_frame: &VmFrame,
    ) -> Result<Value, VmTrap> {
        match self {
            Self::Reference(reference) => read_vm_reference(runtime, caller_frame, reference),
            Self::CallerLocalDirect { .. } | Self::DirectStorage { .. } => {
                let value = {
                    let value = self.peek(runtime, caller_frame)?;
                    runtime.before_value_clone(value).map_err(VmTrap::Runtime)?;
                    let (value, cloned) = materialize_borrowed_value(value);
                    if cloned {
                        runtime.record_value_op(RegisterValueOpKind::ReadValueClone);
                    }
                    value
                };
                Ok(value)
            }
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
        runtime
            .before_output_write(caller_frame, self.reference_view(caller_frame))
            .map_err(VmTrap::Runtime)?;
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
                    .ok_or(VmTrap::Runtime(RuntimeError::NullReference))?;
                *slot = value;
                Ok(())
            }
            Self::DirectStorage { location, offset } => runtime
                .storage_mut()
                .write_direct_slot_by_location(*location, *offset, value)
                .then_some(())
                .ok_or(VmTrap::Runtime(RuntimeError::NullReference)),
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
        &self,
        runtime: &'a impl CallContext,
    ) -> Result<Option<&'a Value>, VmTrap> {
        let reference = match self {
            Self::Direct {
                instance_id,
                offset,
            } => crate::value::ValueRefView {
                location: MemoryLocation::Instance(*instance_id),
                offset: *offset,
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
    pub fn read<'a>(&self, runtime: &'a impl CallContext) -> Option<&'a Value> {
        match self {
            Self::Direct {
                instance_id,
                offset,
            } => runtime
                .storage()
                .read_instance_field_by_offset(*instance_id, *offset),
            Self::Reference(reference) => runtime.storage().read_by_ref_ref(reference),
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
                offset: *offset,
                path: Vec::new(),
            },
            Self::Reference(reference) => reference.clone(),
        };
        runtime
            .check_native_write(None, &reference, value)
            .map_err(VmTrap::Runtime)
    }

    /// Store a value through the bound target; the caller owns type normalization.
    pub fn write(&self, runtime: &mut impl CallContext, value: Value) -> bool {
        let value = match self.read(runtime) {
            Some(target) => normalize_assignment_for_target(target, value),
            None => value,
        };
        match self {
            Self::Direct {
                instance_id,
                offset,
            } => runtime
                .storage_mut()
                .write_instance_field_by_offset(*instance_id, *offset, value),
            Self::Reference(reference) => runtime.storage_mut().write_by_ref_ref(reference, value),
        }
    }

    /// Retain this binding as a function-block output source.
    pub fn out_source(&self) -> VmFbOutSource {
        match self {
            Self::Direct {
                instance_id,
                offset,
            } => VmFbOutSource::Direct {
                instance_id: *instance_id,
                offset: *offset,
            },
            Self::Reference(reference) => VmFbOutSource::Reference(reference.clone()),
        }
    }
}
