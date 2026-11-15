//! The context of a Rust POU's call (specification 48 RU-11, RU-16): what the POU reads of its
//! activation, the task's logical time and interval and the wall clock, and where the faults it
//! returns are recorded.
//!
//! The kernel interface's own context (specification 44 §8.2) holds the statement budget, the call
//! depth, the breakpoints and the kernel's pointers, which only the generated code and the kernel
//! may change. The generated code gives each call of a Rust POU a [`Cx`] made from a clone of it and
//! takes back only the record of the fault the call returns, so no Rust code reaches the rest.

use trust_plc_abi::module;

use crate::{Dt, Fault, Time};

/// The context of one call of a Rust POU (specification 48 RU-11): the task's logical time
/// ([`now`](Cx::now)) and interval ([`interval`](Cx::interval)), the wall clock
/// ([`wall_clock`](Cx::wall_clock)) and the record of the fault the POU returns, which the SDK's
/// functions and [`rust_fault`](crate::rust_fault) write.
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
    eno: bool,
}

impl Cx {
    /// The task's logical time, Structured Text's `TIME()` (specification 11 §5.2.1): the same
    /// for every POU of one activation.
    #[must_use]
    pub fn now(&self) -> Time {
        Time::from_nanos(self.raw.now_ns)
    }

    /// The task's interval (specification 48 RU-16): its `INTERVAL`, the resource's cycle interval
    /// for a program without a task, and `T#0s` for a task that only its `SINGLE` variable starts
    /// and outside a task (specification 44 §8.2). A block that integrates or differentiates over
    /// its calls reads its sample time here.
    #[must_use]
    pub fn interval(&self) -> Time {
        Time::from_nanos(self.raw.interval_ns)
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

    /// Sets the POU's `ENO` (specification 04 §EN/ENO; specification 48 RU-30), which a call
    /// starts `TRUE` and the generated code writes into the POU's output when the body returns
    /// `Ok`: `cx.set_eno(false)` tells the caller that the call did not do its work, as
    /// specification 04's `SafeDiv` does for a zero divisor.
    pub fn set_eno(&mut self, eno: bool) {
        self.eno = eno;
    }

    /// The POU's `ENO` as the body has set it, `TRUE` until it sets it `FALSE`.
    #[must_use]
    pub fn eno(&self) -> bool {
        self.eno
    }

    /// A context holding `raw`, the kernel interface's, with `ENO` `TRUE`.
    pub(crate) fn of(raw: trust_plc_abi::Cx) -> Self {
        Self { raw, eno: true }
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

/// Ends the call's context `cx`: takes back into `kernel` the record of the fault the call returns,
/// the only part of the kernel's context a Rust POU writes, and gives the POU's `ENO` (RU-30).
#[inline]
#[must_use]
pub fn leave(cx: Cx, kernel: &mut trust_plc_abi::Cx) -> bool {
    let Cx { raw: called, eno } = cx;
    kernel.fault = called.fault;
    eno
}
