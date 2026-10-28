//! HMI commands (specification 39, §12.5–12.8): what a command writes, on the runtime thread,
//! and the record of every command by its id. A method command and a setting are checked against
//! the running program's image (§21.2, §21.4).

pub mod capability;
pub mod capability_checks;
pub mod plan;
pub(crate) mod program_checks;
pub mod registry;
pub mod write;
