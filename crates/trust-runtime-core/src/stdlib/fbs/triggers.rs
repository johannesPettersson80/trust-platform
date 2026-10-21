use crate::error::RuntimeError;
use crate::memory::view::{StoreAccess, StoreMutate};
use crate::memory::InstanceId;

use super::instance::{get_or_init_bool, read_bool, write_bool};
use super::members::Member;
use super::BuiltinExecContext;

/// Rising-edge trigger state.
#[derive(Debug, Clone)]
pub struct RTrig {
    prev: bool,
}

impl RTrig {
    /// Create the standard initial function-block state.
    #[must_use]
    pub fn new() -> Self {
        Self { prev: false }
    }

    /// Sample CLK and return the pulse for this trigger edge.
    pub fn step(&mut self, clk: bool) -> bool {
        let q = clk && !self.prev;
        self.prev = clk;
        q
    }
}

impl Default for RTrig {
    fn default() -> Self {
        Self::new()
    }
}

/// Falling-edge trigger state.
#[derive(Debug, Clone)]
pub struct FTrig {
    prev: bool,
}

impl FTrig {
    /// Create the standard initial function-block state.
    #[must_use]
    pub fn new() -> Self {
        Self { prev: false }
    }

    /// Sample CLK and return the pulse for this trigger edge.
    pub fn step(&mut self, clk: bool) -> bool {
        let not_clk = !clk;
        let q = not_clk && !self.prev;
        self.prev = not_clk;
        q
    }
}

impl Default for FTrig {
    fn default() -> Self {
        Self::new()
    }
}

pub(super) fn exec_r_trig<S: StoreAccess + StoreMutate>(
    ctx: &mut BuiltinExecContext<'_, S>,
    instance_id: InstanceId,
) -> Result<(), RuntimeError> {
    let clk = read_bool(ctx, instance_id, Member::Clk)?;
    let prev = get_or_init_bool(ctx, instance_id, Member::TrigM, false)?;
    let q = clk && !prev;
    write_bool(ctx, instance_id, Member::Q, q);
    write_bool(ctx, instance_id, Member::TrigM, clk);
    Ok(())
}

pub(super) fn exec_f_trig<S: StoreAccess + StoreMutate>(
    ctx: &mut BuiltinExecContext<'_, S>,
    instance_id: InstanceId,
) -> Result<(), RuntimeError> {
    let clk = read_bool(ctx, instance_id, Member::Clk)?;
    let prev_not_clk = get_or_init_bool(ctx, instance_id, Member::TrigM, false)?;
    let not_clk = !clk;
    let q = not_clk && !prev_not_clk;
    write_bool(ctx, instance_id, Member::Q, q);
    write_bool(ctx, instance_id, Member::TrigM, not_clk);
    Ok(())
}
