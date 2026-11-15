//! What a Rust POU's unit tests use (specification 48 RU-56): a test calls a function with a
//! context and a function block with `call`, as the kernel does, and runs the POU's Rust code
//! alone, not the program.

use trust_plc_abi::kernel::WITHOUT_DEBUGGER;

use crate::Cx;

/// A context as the kernel gives an activation outside a task: logical time 0, no debugger.
#[must_use]
pub fn cx() -> Cx {
    Cx::new(&WITHOUT_DEBUGGER)
}
