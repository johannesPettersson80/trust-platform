//! The SDK of truST's Rust POUs (specification 48).
//!
//! A Rust function, function block or program of a truST project is compiled into the program's
//! module with the generated Structured Text, and keeps the scan's rules (specification 48 §5): it
//! returns faults instead of panicking, allocates nothing and runs without the standard library.
//! This crate gives it what it needs from the two crates every module already links: the
//! activation's context and the faults of the kernel interface (`trust-plc-abi`), and the
//! standard's arithmetic and mathematics of the program library (`trust-plc-lib`). A Rust POU
//! therefore computes and faults as the Structured Text beside it does: both call the same
//! functions.
//!
//! The crate holds no `unsafe` code; the `unsafe` of a module stays in the interface crate
//! (specification 44 §6.9).

#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod checked;
pub mod math;
pub mod prelude;

pub use trust_plc_abi::{Cx, Fault, FaultCode, Str, WStr};

#[cfg(test)]
mod tests;
