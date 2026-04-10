use crate::memory::InstanceId;
use crate::value::Value;

use super::errors::VmTrap;

pub(super) const VM_MAX_CALL_DEPTH: usize = 1024;

pub(super) fn ensure_global_call_depth(
    depth_offset: u32,
    local_depth: usize,
) -> Result<(), VmTrap> {
    let base_depth = usize::try_from(depth_offset).unwrap_or(usize::MAX);
    let total_depth = base_depth.saturating_add(local_depth);
    if total_depth > VM_MAX_CALL_DEPTH {
        return Err(VmTrap::CallStackOverflow);
    }
    Ok(())
}

#[derive(Debug, Clone)]
pub(super) struct VmFrame {
    pub(super) pou_id: u32,
    pub(super) return_pc: usize,
    pub(super) code_start: usize,
    pub(super) code_end: usize,
    pub(super) local_ref_start: u32,
    pub(super) local_ref_count: u32,
    pub(super) locals: Vec<Value>,
    pub(super) runtime_instance: Option<InstanceId>,
    pub(super) instance_owner: Option<u32>,
}

impl VmFrame {
    pub(super) fn local_slot_index(&self, ref_index: u32) -> Result<usize, VmTrap> {
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

    pub(super) fn load_local(&self, ref_index: u32) -> Result<Value, VmTrap> {
        let index = self.local_slot_index(ref_index)?;
        self.locals
            .get(index)
            .cloned()
            .ok_or(VmTrap::InvalidLocalRef {
                ref_index,
                start: self.local_ref_start,
                count: self.local_ref_count,
            })
    }

    pub(super) fn store_local(&mut self, ref_index: u32, value: Value) -> Result<(), VmTrap> {
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

#[derive(Debug, Default)]
pub(super) struct FrameStack {
    frames: Vec<VmFrame>,
}

impl FrameStack {
    pub(super) fn clear(&mut self) {
        self.frames.clear();
    }

    pub(super) fn push(&mut self, frame: VmFrame) -> Result<(), VmTrap> {
        if self.frames.len() >= VM_MAX_CALL_DEPTH {
            return Err(VmTrap::CallStackOverflow);
        }
        self.frames.push(frame);
        Ok(())
    }

    pub(super) fn pop(&mut self) -> Result<VmFrame, VmTrap> {
        self.frames.pop().ok_or(VmTrap::CallStackUnderflow)
    }

    pub(super) fn current(&self) -> Option<&VmFrame> {
        self.frames.last()
    }

    pub(super) fn current_mut(&mut self) -> Option<&mut VmFrame> {
        self.frames.last_mut()
    }

    pub(super) fn is_empty(&self) -> bool {
        self.frames.is_empty()
    }

    pub(super) fn len(&self) -> usize {
        self.frames.len()
    }
}
