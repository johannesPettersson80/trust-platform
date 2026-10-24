//! Shared debug state: the span a breakpoint covers, the breakpoint record with its hit condition
//! and count, the cycle suppression of one scan, and the run control that decides when a scan holds
//! and when a stop is emitted (specification 13, "Breakpoint evaluation, source resolution, and hook
//! contract", "Pause", "Continue" and "Stepping Semantics"; specification 34, ARCH-11).
//!
//! [`statement`] holds [`Span`] and the identity that cycle suppression keys on. [`breakpoint`]
//! holds the record, the hit condition, the hit count and [`CycleSuppression`]. The one hit rule is
//! [`count_hit`], and [`observe_hit`] applies it and stores the count. Positions are generic over
//! an ordered type: a runtime that holds the source text uses byte offsets, and a chip uses the
//! `(line, column)` pairs of its line table. [`run`] holds [`RunControl`], which the composing
//! debugger calls for every decision. The composing debugger owns the lock, the condition variable
//! and the wait that holds a scan.
//!
//! The debug operations of the control protocol are answered here as well, for a runtime that holds
//! no source text (specification 11 §6.13; specification 13): [`requests`] parses a request and
//! answers it from a [`Debugger`], [`reads`] lists the frames, scopes and
//! variables of the published debug state, [`paging`] cuts a variables listing to the page a
//! request asks for, [`projection`] projects values onto the DAP shapes,
//! [`replies`] writes the reply shapes and the stable refusals, and [`sources`] resolves
//! breakpoints and names files from the program's `LINE_TABLE`.

pub mod breakpoint;
#[cfg(feature = "native")]
mod compiled;
pub mod expression;
pub mod logpoint;
pub mod options;
pub mod paging;
pub mod projection;
pub mod reads;
pub mod replies;
pub mod requests;
pub mod run;
pub mod sources;
pub mod statement;

pub use breakpoint::{
    count_hit, observe_hit, Breakpoint, BreakpointOptions, BreakpointRecord, CountedHit,
    CycleSuppression, HitCondition,
};
#[cfg(feature = "native")]
pub(crate) use compiled::FrameNames;
pub use requests::{answer, is_debug_operation, BreakpointView, Debugger, StopRecord};
pub use run::{
    BoundaryStop, RunAction, RunControl, RunEffect, RunMode, RunOutcome, StepKind, StepState,
    StopReason,
};
pub use sources::LineSources;
pub use statement::{Span, StatementId};

#[cfg(test)]
mod variable_pages_contract_tests;
