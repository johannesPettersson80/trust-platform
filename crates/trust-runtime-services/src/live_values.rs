//! Live Values: the pending writes and active forces a control surface queues for the next cycle
//! boundary (specification 11, "Live Values"; specification 34 §4.3.1).
//!
//! [`queues`] is a plain structure with no lock, no clock and no
//! operating-system dependency, so a chip runtime can hold it too. The composing debugger owns
//! the lock and reports a refusal; the queues own the capacity, the upsert and release rules and
//! the drain order. [`apply`] owns the order in which a scan applies the queues at its cycle
//! boundaries and how a refusal is handled. [`io_snapshot`] holds the portable snapshot and
//! [`io_requests`] answers the `io.*` operations from it and the queues. [`var_requests`] answers
//! the `set`, `var.force`, `var.unforce` and `var.forced` operations from the variable queues, and
//! [`value_grammar`] parses the scalar value of every operation that carries one.

pub mod apply;
pub mod io_requests;
pub mod io_snapshot;
pub mod queues;
pub mod value_grammar;
pub mod var_requests;

pub use apply::{is_cycle_operational_error, WriteSink};
pub use queues::{
    ForcedSnapshot, ForcedVar, ForcedVarTarget, LiveValueQueues, PendingLValueWrite,
    PendingVarTarget, PendingVarWrite, RequestQueueFull, REQUEST_QUEUE_CAPACITY,
};
