//! A call of Structured Text that a Rust POU makes (specification 48 RU-38, RU-39).

use crate::{Cx, Fault};

/// A call of Structured Text from a Rust POU: the generated bindings of layer 1 implement it for
/// each Structured Text POU a Rust POU calls (specification 48 §8, §9.2), and the SDK's context
/// runs it on the kernel's context that only the SDK holds (`trust_plc::Cx`, RU-11). A Rust POU's
/// crate cannot name this crate (RU-20), so no Rust POU implements it, and none receives the
/// kernel's context through it.
pub trait StructuredTextCall {
    /// What the call gives back: the function's result, or `()`.
    type Output;

    /// Runs the call on the kernel's context `cx`, whose statement budget its statements count
    /// against (specification 44 §8.4).
    ///
    /// # Errors
    ///
    /// The fault that ends the Structured Text, recorded in `cx`.
    fn call(self, cx: &mut Cx) -> Result<Self::Output, Fault>;
}
