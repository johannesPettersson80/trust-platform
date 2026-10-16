use alloc::vec::Vec;

use crate::memory::{FrameId, InstanceId};
use crate::value::Value;

#[cfg(any(feature = "hir", test))]
use super::materialize_borrowed_value;
use super::{VmTrap, VM_MAX_CALL_DEPTH};

/// Validate that a caller-supplied depth offset plus local frame depth fits the VM limit.
pub fn ensure_global_call_depth(depth_offset: u32, local_depth: usize) -> Result<(), VmTrap> {
    let base_depth = usize::try_from(depth_offset).unwrap_or(usize::MAX);
    let total_depth = base_depth.saturating_add(local_depth);
    if total_depth > VM_MAX_CALL_DEPTH {
        return Err(VmTrap::CallStackOverflow);
    }
    Ok(())
}

/// One active VM call frame.
#[derive(Debug, Clone)]
pub struct VmFrame {
    /// Live source-free invocation identity; absent only for legacy hosted frames.
    pub activation: Option<FrameId>,
    /// Lexical POU, absent for resource-global initializer bodies.
    pub pou_id: Option<u32>,
    /// Program counter to restore on return.
    pub return_pc: usize,
    /// First bytecode offset owned by this frame.
    pub code_start: usize,
    /// One-past-last bytecode offset owned by this frame.
    pub code_end: usize,
    /// First local reference index owned by this frame.
    pub local_ref_start: u32,
    /// Number of local references owned by this frame.
    pub local_ref_count: u32,
    /// Local slot values for this frame.
    pub locals: Vec<Value>,
    /// Slots already bound to input values, independent of whether the value is NULL.
    pub parameter_values_present: Vec<bool>,
    /// Runtime instance backing this frame, when executing an FB/class instance.
    pub runtime_instance: Option<InstanceId>,
    /// POU id that owns the backing runtime instance, when applicable.
    pub instance_owner: Option<u32>,
}

impl VmFrame {
    /// Reference identity of this invocation, with the legacy sentinel fallback.
    pub fn reference_frame_id(&self) -> FrameId {
        self.activation
            .unwrap_or(FrameId(super::context::VM_LOCAL_SENTINEL_FRAME_ID))
    }

    /// Whether a reference addresses this invocation's currently resident locals.
    pub fn owns_local_reference(&self, reference: &crate::value::ValueRef) -> bool {
        reference.location == crate::memory::MemoryLocation::Local(self.reference_frame_id())
    }

    /// Convert a bytecode local reference index into a frame-local slot index.
    pub fn local_slot_index(&self, ref_index: u32) -> Result<usize, VmTrap> {
        if ref_index < self.local_ref_start
            || ref_index >= self.local_ref_start.saturating_add(self.local_ref_count)
        {
            return Err(VmTrap::InvalidLocalRef {
                ref_index,
                start: self.local_ref_start,
                count: self.local_ref_count,
            });
        }
        Ok((ref_index - self.local_ref_start) as usize)
    }

    /// Load a local value by bytecode reference index.
    #[cfg(any(feature = "hir", test))]
    pub fn load_local(&self, ref_index: u32) -> Result<Value, VmTrap> {
        let index = self.local_slot_index(ref_index)?;
        self.locals
            .get(index)
            .map(|value| materialize_borrowed_value(value).0)
            .ok_or_else(|| VmTrap::InvalidLocalRef {
                ref_index,
                start: self.local_ref_start,
                count: self.local_ref_count,
            })
    }

    /// Store a local value by bytecode reference index.
    pub fn store_local(&mut self, ref_index: u32, value: Value) -> Result<(), VmTrap> {
        let index = self.local_slot_index(ref_index)?;
        let slot = self
            .locals
            .get_mut(index)
            .ok_or_else(|| VmTrap::InvalidLocalRef {
                ref_index,
                start: self.local_ref_start,
                count: self.local_ref_count,
            })?;
        *slot = value;
        Ok(())
    }
}

/// Stack of active VM call frames.
#[derive(Debug, Default)]
pub struct FrameStack {
    frames: Vec<VmFrame>,
    #[cfg(feature = "register-ir")]
    reserved_locals: Vec<Value>,
    #[cfg(feature = "register-ir")]
    reserved_parameter_presence: Vec<bool>,
    #[cfg(feature = "register-ir")]
    reserved_payload_count: usize,
}

impl FrameStack {
    /// Payload bytes held by the pre-reserved scratch backing.
    #[cfg(all(feature = "register-ir", any(test, feature = "test-support")))]
    pub fn storage_bytes(&self) -> usize {
        let active_payload_bytes = self.frames.iter().fold(0usize, |bytes, frame| {
            bytes
                .saturating_add(
                    frame
                        .locals
                        .capacity()
                        .saturating_mul(core::mem::size_of::<Value>()),
                )
                .saturating_add(
                    frame
                        .parameter_values_present
                        .capacity()
                        .saturating_mul(core::mem::size_of::<bool>()),
                )
        });
        self.frames
            .capacity()
            .saturating_mul(core::mem::size_of::<VmFrame>())
            .saturating_add(active_payload_bytes)
            .saturating_add(
                self.reserved_locals
                    .capacity()
                    .saturating_mul(core::mem::size_of::<Value>()),
            )
            .saturating_add(
                self.reserved_parameter_presence
                    .capacity()
                    .saturating_mul(core::mem::size_of::<bool>()),
            )
    }
    /// Reserve invocation scratch during controller construction.
    #[cfg(feature = "register-ir")]
    pub fn try_reserve_frames(&mut self, capacity: usize) -> Result<(), VmTrap> {
        if capacity > VM_MAX_CALL_DEPTH {
            return Err(VmTrap::CallStackOverflow);
        }
        self.frames
            .try_reserve_exact(capacity.saturating_sub(self.frames.len()))
            .map_err(|_| VmTrap::Runtime(crate::error::RuntimeError::PreparationLimit))
    }

    /// Reserve one reusable register-entry frame payload during construction.
    #[cfg(feature = "register-ir")]
    pub fn try_reserve_frame_payload(&mut self, local_count: usize) -> Result<(), VmTrap> {
        self.reserved_locals
            .try_reserve_exact(local_count.saturating_sub(self.reserved_locals.len()))
            .map_err(|_| VmTrap::Runtime(crate::error::RuntimeError::PreparationLimit))?;
        self.reserved_parameter_presence
            .try_reserve_exact(local_count.saturating_sub(self.reserved_parameter_presence.len()))
            .map_err(|_| VmTrap::Runtime(crate::error::RuntimeError::PreparationLimit))?;
        self.reserved_locals.resize(local_count, Value::Null);
        self.reserved_parameter_presence.resize(local_count, false);
        self.reserved_payload_count = local_count;
        Ok(())
    }

    #[cfg(feature = "register-ir")]
    pub(super) fn take_reserved_frame_payload(
        &mut self,
        local_count: usize,
    ) -> Result<(Vec<Value>, Vec<bool>), VmTrap> {
        if self.reserved_locals.len() != local_count
            || self.reserved_parameter_presence.len() != local_count
        {
            return Err(VmTrap::Runtime(
                crate::error::RuntimeError::PreparationLimit,
            ));
        }
        Ok((
            core::mem::take(&mut self.reserved_locals),
            core::mem::take(&mut self.reserved_parameter_presence),
        ))
    }

    #[cfg(feature = "register-ir")]
    pub(super) fn has_reserved_frame_payload(&self) -> bool {
        self.reserved_locals.len() == self.reserved_payload_count
            && self.reserved_parameter_presence.len() == self.reserved_payload_count
    }

    /// Move an invocation-owned payload into the current reserved frame after
    /// validating its length before moving any values.
    #[cfg(feature = "register-ir")]
    pub(super) fn move_current_locals_from(&mut self, values: &mut [Value]) -> Result<(), VmTrap> {
        let frame = self
            .current_mut()
            .ok_or_else(|| VmTrap::CallStackUnderflow)?;
        if values.len() > frame.locals.len() {
            return Err(VmTrap::BytecodeDecode(
                "register-ir call initial local payload exceeds frame local capacity".into(),
            ));
        }
        for (destination, source) in frame.locals.iter_mut().zip(values) {
            *destination = core::mem::replace(source, Value::Null);
        }
        Ok(())
    }

    #[cfg(all(test, feature = "register-ir"))]
    pub(super) fn reserved_payload_addresses(&self) -> (*const Value, *const bool) {
        (
            self.reserved_locals.as_ptr(),
            self.reserved_parameter_presence.as_ptr(),
        )
    }

    #[cfg(feature = "register-ir")]
    pub(super) fn recycle_frame_presence(&mut self, frame: &mut VmFrame) {
        if frame.return_pc != usize::MAX
            || frame.parameter_values_present.len() != self.reserved_payload_count
            || !self.reserved_parameter_presence.is_empty()
        {
            return;
        }
        frame.parameter_values_present.clear();
        frame
            .parameter_values_present
            .resize(self.reserved_payload_count, false);
        self.reserved_parameter_presence = core::mem::take(&mut frame.parameter_values_present);
    }

    #[cfg(feature = "register-ir")]
    pub(super) fn recycle_returned_locals(
        &mut self,
        mut locals: Vec<Value>,
    ) -> Result<(), crate::error::RuntimeError> {
        if locals.len() != self.reserved_payload_count || !self.reserved_locals.is_empty() {
            return Err(crate::error::RuntimeError::InvalidExecutionState);
        }
        for value in &mut locals {
            *value = Value::Null;
        }
        self.reserved_locals = locals;
        Ok(())
    }

    #[cfg(feature = "register-ir")]
    pub(super) fn recycle_frame_payload(&mut self, mut frame: VmFrame) {
        if frame.return_pc != usize::MAX
            || frame.locals.len() != self.reserved_payload_count
            || frame.parameter_values_present.len() != self.reserved_payload_count
            || !self.reserved_locals.is_empty()
            || !self.reserved_parameter_presence.is_empty()
        {
            return;
        }
        frame.locals.clear();
        frame
            .locals
            .resize(frame.local_ref_count as usize, Value::Null);
        frame.parameter_values_present.clear();
        frame
            .parameter_values_present
            .resize(self.reserved_payload_count, false);
        self.reserved_locals = frame.locals;
        self.reserved_parameter_presence = frame.parameter_values_present;
    }

    /// Remove every active frame.
    pub fn clear(&mut self) {
        self.frames.clear();
    }

    pub(super) fn growth_demand(&self) -> Result<(usize, usize), VmTrap> {
        if self.frames.len() < self.frames.capacity() {
            return Ok((0, 0));
        }
        let capacity = self
            .frames
            .capacity()
            .checked_mul(2)
            .ok_or_else(|| VmTrap::Runtime(crate::error::RuntimeError::Overflow))?
            .clamp(1, VM_MAX_CALL_DEPTH);
        if capacity <= self.frames.len() {
            return Err(VmTrap::CallStackOverflow);
        }
        let bytes = capacity
            .checked_mul(core::mem::size_of::<VmFrame>())
            .ok_or_else(|| VmTrap::Runtime(crate::error::RuntimeError::Overflow))?;
        Ok((bytes, self.frames.len()))
    }

    pub(super) fn reserve_for_push(&mut self) -> Result<(), VmTrap> {
        if self.frames.len() >= VM_MAX_CALL_DEPTH {
            return Err(VmTrap::CallStackOverflow);
        }
        if self.frames.len() == self.frames.capacity() {
            let (bytes, _) = self.growth_demand()?;
            let capacity = bytes / core::mem::size_of::<VmFrame>();
            self.frames
                .try_reserve_exact(capacity - self.frames.len())
                .map_err(|_| VmTrap::Runtime(crate::error::RuntimeError::Overflow))?;
        }
        Ok(())
    }

    /// Push one frame while enforcing the VM call-depth limit.
    pub fn push(&mut self, frame: VmFrame) -> Result<(), VmTrap> {
        if self.frames.len() >= VM_MAX_CALL_DEPTH {
            return Err(VmTrap::CallStackOverflow);
        }
        self.reserve_for_push()?;
        self.frames.push(frame);
        Ok(())
    }

    /// Pop the current frame.
    pub fn pop(&mut self) -> Result<VmFrame, VmTrap> {
        self.frames.pop().ok_or_else(|| VmTrap::CallStackUnderflow)
    }

    /// Borrow the current frame.
    #[must_use]
    pub fn current(&self) -> Option<&VmFrame> {
        self.frames.last()
    }

    /// Mutably borrow the current frame.
    pub fn current_mut(&mut self) -> Option<&mut VmFrame> {
        self.frames.last_mut()
    }

    /// Return true when no frames are active.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.frames.is_empty()
    }

    /// Return the active frame count.
    #[must_use]
    pub fn len(&self) -> usize {
        self.frames.len()
    }
}

#[cfg(all(test, feature = "register-ir"))]
#[path = "frames/recycling_tests.rs"]
mod recycling_tests;
