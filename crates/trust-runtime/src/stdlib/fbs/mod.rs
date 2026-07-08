//! Standard function blocks (TON, CTU, etc.).

#![allow(missing_docs)]

mod bistable;
mod counters;
mod instance;
mod registry;
mod state;
mod timers;
mod triggers;

pub use bistable::{Rs, Sr};
pub use counters::{CounterOutput, CounterUpDownOutput, Ctd, Ctu, Ctud};
pub use registry::{builtin_kind, builtin_kind_uppercase, standard_function_blocks, BuiltinFbKind};
pub use timers::{TimerOutput, Tof, Ton, Tp};
pub use triggers::{FTrig, RTrig};

use crate::error::RuntimeError;
use crate::memory::{InstanceId, VariableStorage};
use crate::value::Duration;

struct BuiltinExecContext<'a> {
    storage: &'a mut VariableStorage,
    now: Duration,
}

pub fn execute_builtin_in_storage(
    storage: &mut VariableStorage,
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
