//! Shared debug state: the span a breakpoint covers, the breakpoint record with its hit condition
//! and count, and the cycle suppression of one scan (specification 13, "Breakpoint evaluation,
//! source resolution, and hook contract"; specification 34, ARCH-11).
//!
//! [`statement`] holds [`Span`] and the identity that cycle suppression keys on. [`breakpoint`]
//! holds the record, the hit condition, the hit count and [`CycleSuppression`]. The one hit rule is
//! [`count_hit`], and [`observe_hit`] applies it and stores the count. Positions are generic over
//! an ordered type: a runtime that holds the source text uses byte offsets, and a chip uses the
//! `(line, column)` pairs of its line table. Run control, holds and waits are not here; the
//! composing debugger owns them.

pub mod breakpoint;
pub mod statement;

pub use breakpoint::{
    count_hit, observe_hit, Breakpoint, BreakpointRecord, CountedHit, CycleSuppression,
    HitCondition,
};
pub use statement::{Span, StatementId};
