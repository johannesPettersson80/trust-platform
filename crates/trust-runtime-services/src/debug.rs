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

pub mod breakpoint;
pub mod run;
pub mod statement;

pub use breakpoint::{
    count_hit, observe_hit, Breakpoint, BreakpointRecord, CountedHit, CycleSuppression,
    HitCondition,
};
pub use run::{
    BoundaryStop, RunAction, RunControl, RunEffect, RunMode, RunOutcome, StepKind, StepState,
    StopReason,
};
pub use statement::{Span, StatementId};
