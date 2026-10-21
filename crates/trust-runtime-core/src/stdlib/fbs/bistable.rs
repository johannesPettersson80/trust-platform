use crate::error::RuntimeError;
use crate::memory::view::{StoreAccess, StoreMutate};
use crate::memory::InstanceId;

use super::instance::{read_bool, write_bool};
use super::members::Member;
use super::BuiltinExecContext;

/// Set-dominant bistable state.
#[derive(Debug, Clone)]
pub struct Sr {
    q: bool,
}

impl Sr {
    /// Create the standard initial function-block state.
    #[must_use]
    pub fn new() -> Self {
        Self { q: false }
    }

    /// Apply set/reset inputs using this block's declared dominance.
    pub fn step(&mut self, set: bool, reset: bool) -> bool {
        if set {
            self.q = true;
        } else if reset {
            self.q = false;
        }
        self.q
    }
}

impl Default for Sr {
    fn default() -> Self {
        Self::new()
    }
}

/// Reset-dominant bistable state.
#[derive(Debug, Clone)]
pub struct Rs {
    q: bool,
}

impl Rs {
    /// Create the standard initial function-block state.
    #[must_use]
    pub fn new() -> Self {
        Self { q: false }
    }

    /// Apply set/reset inputs using this block's declared dominance.
    pub fn step(&mut self, set: bool, reset: bool) -> bool {
        if reset {
            self.q = false;
        } else if set {
            self.q = true;
        }
        self.q
    }
}

impl Default for Rs {
    fn default() -> Self {
        Self::new()
    }
}

pub(super) fn exec_rs<S: StoreAccess + StoreMutate>(
    ctx: &mut BuiltinExecContext<'_, S>,
    instance_id: InstanceId,
) -> Result<(), RuntimeError> {
    let set = read_bool(ctx, instance_id, Member::S)?;
    let reset = read_bool(ctx, instance_id, Member::R1)?;
    let mut q = read_bool(ctx, instance_id, Member::Q1)?;
    if reset {
        q = false;
    } else if set {
        q = true;
    }
    write_bool(ctx, instance_id, Member::Q1, q);
    Ok(())
}

pub(super) fn exec_sr<S: StoreAccess + StoreMutate>(
    ctx: &mut BuiltinExecContext<'_, S>,
    instance_id: InstanceId,
) -> Result<(), RuntimeError> {
    let set = read_bool(ctx, instance_id, Member::S1)?;
    let reset = read_bool(ctx, instance_id, Member::R)?;
    let mut q = read_bool(ctx, instance_id, Member::Q1)?;
    if set {
        q = true;
    } else if reset {
        q = false;
    }
    write_bool(ctx, instance_id, Member::Q1, q);
    Ok(())
}
