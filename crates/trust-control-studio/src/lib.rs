//! Control Studio's engine (specification 35, §0.1a): the `controlStudio/…` methods, their long
//! operations, the runtime operations of the protocol annex, the study files and the operation
//! journal. The calculations and rules are in `trust-control`; the program `trust-control-studio`
//! serves these methods to the extension and to the command line, and holds no rule of its own.

pub mod busy_loops;
pub mod cli;
pub mod commands;
pub mod identify_action;
pub mod identify_adapters;
pub mod interrupt;
pub mod journal;
pub mod loops;
pub mod operation;
pub mod preview_action;
pub mod rpc;
pub mod runtime_client;
mod runtime_file;
pub mod serve;
pub mod settings;
pub mod storage;
pub mod tune_action;
pub mod validate_action;
pub mod workflow;
