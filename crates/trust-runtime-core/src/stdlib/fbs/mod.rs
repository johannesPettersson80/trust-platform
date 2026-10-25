//! The standard function blocks (TON, CTU, etc.): their names and the state each instance carries
//! beside its declared members. The program library implements them (specification 44 §6.8).

mod registry;
pub(crate) mod state;

pub use registry::{
    builtin_kind, builtin_kind_uppercase, uppercase_builtin_name, BuiltinFbKind, BUILTIN_NAME_ROOM,
};
