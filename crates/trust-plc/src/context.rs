//! The context of a Rust POU's call (specification 48 RU-11, RU-16): what the POU reads of its
//! activation, the task's logical time and the wall clock, and where the faults it returns are
//! recorded.
//!
//! The kernel interface's own context (specification 44 §8.2) holds the statement budget, the call
//! depth, the breakpoints and the kernel's pointers, which only the generated code and the kernel
//! may change. The generated code gives each call of a Rust POU a [`Cx`] made from a clone of it and
//! takes back only the record of the fault the call returns, so no Rust code reaches the rest.

use trust_plc_abi::module;

use crate::{Dt, Fault, Time};

/// The context of one call of a Rust POU (specification 48 RU-11): the task's logical time
/// ([`now`](Cx::now)), the wall clock ([`wall_clock`](Cx::wall_clock)) and the record of the fault
/// the POU returns, which the SDK's functions and [`rust_fault`](crate::rust_fault) write.
///
/// It keeps the kernel interface's context to itself: a Rust POU changes neither the statement
/// budget, the call depth nor the breakpoints, and reaches none of the kernel's pointers.
///
/// ```compile_fail
/// fn reach(cx: &mut trust_plc::Cx) {
///     cx.budget = u32::MAX;
/// }
/// ```
pub struct Cx {
    raw: trust_plc_abi::Cx,
}

impl Cx {
    /// The task's logical time, Structured Text's `TIME()` (specification 11 §5.2.1): the same
    /// for every POU of one activation.
    #[must_use]
    pub fn now(&self) -> Time {
        Time::from_nanos(self.raw.now_ns)
    }

    /// The platform's wall clock in UTC, as Structured Text's `CURRENT_DT` reads it through the
    /// kernel (specification 44 §8.2).
    ///
    /// # Errors
    ///
    /// `Overflow` for a time before 1970 or one no `DT` holds, `ModuleInternal` on a device without
    /// a wall clock.
    pub fn wall_clock(&mut self) -> Result<Dt, Fault> {
        module::wall_clock(&mut self.raw).map(Dt::from_millis)
    }

    /// A context holding `raw`, the kernel interface's.
    pub(crate) fn of(raw: trust_plc_abi::Cx) -> Self {
        Self { raw }
    }

    /// The kernel interface's context, which the program library's functions take.
    pub(crate) fn raw(&mut self) -> &mut trust_plc_abi::Cx {
        &mut self.raw
    }
}

/// The context of a Rust POU's call, made from a clone of `raw`, the generated code's.
#[must_use]
#[inline]
pub fn enter(raw: &trust_plc_abi::Cx) -> Cx {
    Cx::of(raw.clone())
}

/// Ends the call's context `cx` and takes back into `kernel` the record of the fault the call
/// returns, the only part of the context a Rust POU writes.
#[inline]
pub fn leave(cx: Cx, kernel: &mut trust_plc_abi::Cx) {
    let Cx { raw: called } = cx;
    kernel.fault = called.fault;
}
