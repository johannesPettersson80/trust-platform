//! Live Values: the pending writes and active forces a control surface queues for the next cycle
//! boundary (specification 11, "Live Values"; specification 34 §4.3.1).
//!
//! [`queues`] is a plain structure with no lock, no clock and no
//! operating-system dependency, so a chip runtime can hold it too. The composing debugger owns
//! the lock and reports a refusal; the queues own the capacity, the upsert and release rules and
//! the drain order. [`apply`] owns the order in which a scan applies the queues at its cycle
//! boundaries and how a refusal is handled. [`retained_names`] resolves a retained management
//! label from the prepared program alone.

pub mod apply;
pub mod queues;
pub mod retained_names;

pub use apply::{is_cycle_operational_error, WriteSink};
pub use queues::{
    ForcedSnapshot, ForcedVar, ForcedVarTarget, LiveValueQueues, PendingLValueWrite,
    PendingVarTarget, PendingVarWrite, RequestQueueFull, REQUEST_QUEUE_CAPACITY,
};
pub use retained_names::RetainedNames;
