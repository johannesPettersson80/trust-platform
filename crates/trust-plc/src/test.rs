//! What a Rust POU's unit tests use (specification 48 RU-56): a test calls a function with a
//! context and a function block with `call`, as the kernel does, and runs the POU's Rust code
//! alone, not the program. [`task`] and [`advance`] simulate the task the POU runs in.

use trust_plc_abi::kernel::WITHOUT_DEBUGGER;

use crate::{Cx, Time};

/// A context as the kernel gives an activation outside a task: the logical time 0, the interval
/// `T#0s`, no debugger, no wall clock.
#[must_use]
pub fn cx() -> Cx {
    Cx::of(trust_plc_abi::Cx::new(&WITHOUT_DEBUGGER))
}

/// The context of a simulated task, the task 0 with the interval `interval`, at the logical time 0
/// (specification 48 RU-56); [`advance`] moves it on to the task's next activation.
///
/// ```
/// use trust_plc::{test, Time};
///
/// let mut cx = test::task(Time::from_nanos(10_000_000));
/// test::advance(&mut cx);
/// assert_eq!(cx.now(), Time::from_nanos(10_000_000));
/// assert_eq!(cx.interval(), Time::from_nanos(10_000_000));
/// ```
#[must_use]
pub fn task(interval: Time) -> Cx {
    let mut raw = trust_plc_abi::Cx::new(&WITHOUT_DEBUGGER);
    raw.task = 0;
    raw.interval_ns = interval.as_nanos();
    Cx::of(raw)
}

/// Moves the simulated task of `cx` on to its next activation: its logical time one interval
/// later, as the kernel starts the task's next activation; at most the longest `TIME`.
pub fn advance(cx: &mut Cx) {
    let raw = cx.raw();
    raw.now_ns = raw.now_ns.saturating_add(raw.interval_ns);
}
