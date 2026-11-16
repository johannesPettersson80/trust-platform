//! The context of a Rust POU's call (specification 48 RU-11, RU-16): what the POU reads of its
//! activation, the task's logical time and interval and the wall clock, and where the faults it
//! returns are recorded.
//!
//! The kernel interface's own context (specification 44 §8.2) holds the statement budget, the call
//! depth, the breakpoints and the kernel's pointers, which only the generated code and the kernel
//! may change. The generated code gives each call of a Rust POU a [`Cx`] made from a clone of it and
//! takes back the record of the fault the call returns and the statements the Structured Text it
//! calls spent, so no Rust code reaches the rest. That Structured Text runs on the clone through
//! the generated bindings (RU-38), which no Rust code can make.

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

    /// Runs `call`, a call of Structured Text that the generated bindings make (specification 48
    /// RU-38): on the kernel's context, so its statements count against the activation's statement
    /// budget and stop at their breakpoints (RU-52), with the context's statement as it was once
    /// the call returns, with a fault too: the Structured Text's fault keeps its own statement in
    /// its record, and a fault the POU raises after the call names the POU's calling statement
    /// (RU-53). Only generated code calls it: `C` is a trait of the interface crate, which a Rust
    /// POU's crate cannot name (RU-20).
    ///
    /// # Errors
    ///
    /// The Structured Text's fault, with the statement it ended.
    #[doc(hidden)]
    pub fn __call_structured_text<C: trust_plc_abi::StructuredTextCall>(
        &mut self,
        call: C,
    ) -> Result<C::Output, Fault> {
        let stmt = self.raw.stmt;
        let output = call.call(&mut self.raw);
        self.raw.stmt = stmt;
        output
    }
}

/// The context of a Rust POU's call, made from a clone of `raw`, the generated code's.
#[must_use]
#[inline]
pub fn enter(raw: &trust_plc_abi::Cx) -> Cx {
    Cx::of(raw.clone())
}

/// Ends the call's context `cx`: takes back into `kernel` the record of the fault the call returns
/// and the statement budget the Structured Text it called spent (RU-38), the only parts of the
/// kernel's context its call changes, and gives the POU's `ENO` (RU-30).
#[inline]
#[must_use]
pub fn leave(cx: Cx, kernel: &mut trust_plc_abi::Cx) -> bool {
    let Cx { raw: called, eno } = cx;
    kernel.fault = called.fault;
    kernel.budget = kernel.budget.min(called.budget);
    eno
}
