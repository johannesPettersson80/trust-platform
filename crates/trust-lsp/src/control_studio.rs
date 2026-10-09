//! Control Studio's orchestration (specification 35, §0.1a): the `trust-lsp.controlStudio…`
//! commands, their long operations, the runtime operations of the protocol annex, and the study
//! files. The calculations and rules are in `trust-control`; the extension only shows the result.

pub mod commands;
pub mod journal;
pub mod loops;
pub mod operation;
pub mod runtime_client;
pub mod storage;
