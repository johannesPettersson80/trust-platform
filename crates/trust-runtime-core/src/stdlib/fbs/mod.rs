//! Standard function blocks (TON, CTU, etc.).

mod bistable;
mod counters;
mod instance;
mod registry;
pub(crate) mod state;
mod timers;
mod triggers;

pub use bistable::{Rs, Sr};
pub use counters::{CounterOutput, CounterUpDownOutput, Ctd, Ctu, Ctud};
pub use registry::{builtin_kind, builtin_kind_uppercase, BuiltinFbKind};
pub use timers::{TimerOutput, Tof, Ton, Tp};
pub use triggers::{FTrig, RTrig};

use crate::error::RuntimeError;
use crate::memory::view::{StoreAccess, StoreMutate};
use crate::memory::{InstanceId, VariableStorage};
use crate::value::Duration;

/// The store a native block runs on, and the logical time it runs at. The block reads and writes its
/// instance's members through the store-access seam only, so it runs on whichever store holds the
/// instance (specification 34 §6.2, MEM-10; C1-9d3a).
struct BuiltinExecContext<'a, S> {
    storage: &'a mut S,
    now: Duration,
}

/// Execute a native function block against its declared storage and supplied logical time.
pub fn execute_builtin_in_storage(
    storage: &mut VariableStorage,
    now: Duration,
    instance_id: InstanceId,
    kind: BuiltinFbKind,
) -> Result<(), RuntimeError> {
    execute_builtin_in_store(storage, now, instance_id, kind)
}

/// Execute a native function block on `storage`, the store that holds its instance, at the supplied
/// logical time: the `Value` storage, or the engine's routed store (C1-9d3a, R1).
pub(crate) fn execute_builtin_in_store<S: StoreAccess + StoreMutate>(
    storage: &mut S,
    now: Duration,
    instance_id: InstanceId,
    kind: BuiltinFbKind,
) -> Result<(), RuntimeError> {
    let mut ctx = BuiltinExecContext { storage, now };
    match kind {
        BuiltinFbKind::Rs => bistable::exec_rs(&mut ctx, instance_id),
        BuiltinFbKind::Sr => bistable::exec_sr(&mut ctx, instance_id),
        BuiltinFbKind::RTrig => triggers::exec_r_trig(&mut ctx, instance_id),
        BuiltinFbKind::FTrig => triggers::exec_f_trig(&mut ctx, instance_id),
        BuiltinFbKind::Ctu => counters::exec_ctu(&mut ctx, instance_id),
        BuiltinFbKind::Ctd => counters::exec_ctd(&mut ctx, instance_id),
        BuiltinFbKind::Ctud => counters::exec_ctud(&mut ctx, instance_id),
        BuiltinFbKind::Tp => timers::exec_tp(&mut ctx, instance_id),
        BuiltinFbKind::Ton => timers::exec_ton(&mut ctx, instance_id),
        BuiltinFbKind::Tof => timers::exec_tof(&mut ctx, instance_id),
    }
}
