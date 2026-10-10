//! Hosted declarations and shared execution of standard function blocks.

mod registry;
mod state;

pub use registry::standard_function_blocks;
pub(crate) use state::builtin_state_layout;
pub use trust_runtime_core::stdlib::fbs::{
    builtin_kind, builtin_kind_uppercase, execute_builtin_in_storage, BuiltinFbKind, CounterOutput,
    CounterUpDownOutput, Ctd, Ctu, Ctud, FTrig, RTrig, Rs, Sr, TimerOutput, Tof, Ton, Tp,
};
