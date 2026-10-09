//! HMI commands (specification 39, §12.5–12.8): what a command writes, on the runtime thread,
//! and the record of every command by its id.

pub mod capability;
pub mod capability_checks;
pub mod plan;
pub mod registry;
pub mod write;
