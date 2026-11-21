//! The wait points of a `#[trust_sequence]` (specification 48 §15, RU-72), as the state machine the
//! attribute lowers a sequence to runs them. A wait begins at the activation that reaches it, which
//! then returns, and completes at a later activation, never the one that began it: `next_cycle` at
//! the next, `until` at the first at whose start its predicate holds, `wait(t)` at the first whose
//! logical time is at least `t` after the one that began it. `.within(t)` ends a wait that lasts
//! longer than `t` with the fault `RustFault`, its number the wait point's and its two numbers `t`
//! and the time waited, in milliseconds.

use crate::{rust_fault, Cx, Fault, Str, Time};

/// Where a sequence stands: the wait point it waits at, numbered from 1 in the order of the
/// sequence's body and 0 at its top, and the logical time of the activation that began the wait.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Waiting {
    /// The wait point's number; 0 at the top, before the first.
    pub point: u16,
    /// The logical time of the activation that began the wait.
    pub began: Time,
}

impl Waiting {
    /// Begins the wait at `point`, at the activation of `cx`.
    pub fn begin(&mut self, cx: &Cx, point: u16) {
        self.point = point;
        self.began = cx.now();
    }

    /// The logical time since the wait began; never below zero, even for a time that went back.
    #[must_use]
    pub fn waited(&self, cx: &Cx) -> Time {
        Time::from_nanos(
            cx.now()
                .as_nanos()
                .saturating_sub(self.began.as_nanos())
                .max(0),
        )
    }

    /// Whether `wait(t)` is over: at least `t` has passed since the wait began.
    #[must_use]
    pub fn waited_for(&self, cx: &Cx, t: Time) -> bool {
        self.waited(cx).as_nanos() >= t.as_nanos()
    }

    /// The fault of `.within(t)`: `RustFault` when the wait has lasted longer than `t`, with the wait
    /// point's number, `t` and the time waited in milliseconds; `None` while it has not.
    pub fn outlasted(&self, cx: &mut Cx, t: Time) -> Option<Fault> {
        let waited = self.waited(cx);
        (waited.as_nanos() > t.as_nanos())
            .then(|| rust_fault(cx, i64::from(self.point), millis(t), millis(waited)))
    }
}

/// Names the step the sequence is in, in the step output `output`: `s.step(&mut self.step, "GRIP")`.
/// A name longer than the output keeps its longest prefix of whole characters.
pub fn step<const N: usize>(output: &mut Str<N>, name: &str) {
    output.set(name);
}

/// The whole milliseconds of `t`.
fn millis(t: Time) -> i64 {
    t.as_nanos().checked_div(1_000_000).unwrap_or(0)
}
