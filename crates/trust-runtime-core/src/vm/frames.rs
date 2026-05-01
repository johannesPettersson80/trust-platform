use alloc::vec::Vec;

use crate::memory::InstanceId;
use crate::value::Value;

use super::{materialize_borrowed_value, VmTrap};

/// Maximum recursive VM call depth accepted by the bytecode executor.
pub const VM_MAX_CALL_DEPTH: usize = 1024;

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
    /// POU id being executed.
    pub pou_id: u32,
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
    /// Runtime instance backing this frame, when executing an FB/class instance.
    pub runtime_instance: Option<InstanceId>,
    /// POU id that owns the backing runtime instance, when applicable.
    pub instance_owner: Option<u32>,
}

impl VmFrame {
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
    pub fn load_local(&self, ref_index: u32) -> Result<Value, VmTrap> {
        let index = self.local_slot_index(ref_index)?;
        self.locals
            .get(index)
            .map(|value| materialize_borrowed_value(value).0)
            .ok_or(VmTrap::InvalidLocalRef {
                ref_index,
                start: self.local_ref_start,
                count: self.local_ref_count,
            })
    }

    /// Store a local value by bytecode reference index.
    pub fn store_local(&mut self, ref_index: u32, value: Value) -> Result<(), VmTrap> {
        let index = self.local_slot_index(ref_index)?;
        let slot = self.locals.get_mut(index).ok_or(VmTrap::InvalidLocalRef {
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
}

impl FrameStack {
    /// Remove every active frame.
    pub fn clear(&mut self) {
        self.frames.clear();
    }

    /// Push one frame while enforcing the VM call-depth limit.
    pub fn push(&mut self, frame: VmFrame) -> Result<(), VmTrap> {
        if self.frames.len() >= VM_MAX_CALL_DEPTH {
            return Err(VmTrap::CallStackOverflow);
        }
        self.frames.push(frame);
        Ok(())
    }

    /// Pop the current frame.
    pub fn pop(&mut self) -> Result<VmFrame, VmTrap> {
        self.frames.pop().ok_or(VmTrap::CallStackUnderflow)
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
