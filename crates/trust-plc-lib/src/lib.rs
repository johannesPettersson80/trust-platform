//! The program library of truST's compiled programs (specification 44 §6.8 and §6.9): the
//! operators that can fault, the standard functions and the standard function blocks, as generated
//! code calls them.
//!
//! Every function computes what specification 05, 07 and 08 and `docs/IEC_DECISIONS.md` say, with
//! the results and faults of the runtime's own implementation, so that a program computes the same
//! values in a module as it did in the virtual machine (specification 44 §9). A function that can
//! fail takes the context and returns the fault it recorded there
//! ([`trust_plc_abi::module::fault`]). Nothing allocates and nothing panics.
//!
//! The function block types lie in areas: their fields are cells, named as the standard names the
//! function block's variables (`IN`, `PT`, `Q`, `ET`), inputs and outputs first and the hidden state
//! after them (specification 44 §7.2).

#![no_std]
#![warn(missing_docs)]
// The fields of a standard function block are the standard's names of its variables.
#![allow(non_snake_case)]

mod arith;
mod convert;
mod edges;
mod select;
mod text;
mod timers;

pub use arith::{
    abs, add, div, fadd, fdiv, finite, fmul, fneg, fpow, fsub, mul, neg, pow, rem, sub, Integer,
    Real, Signed,
};
pub use convert::{
    bits_to_real32, bits_to_real64, dword_to_time, real32_to_real, real64_to_real32, real_to_int,
    round_ties_to_even, sint_to, sint_to_real32, sint_to_real64, time_to_dword, uint_to,
    uint_to_real32, uint_to_real64, IntTarget, Rounding,
};
pub use edges::{f_trig, r_trig, FTrig, RTrig};
pub use select::{at, fabs, limit, max, min, subrange};
pub use text::{
    assign, assign_bytes, compare, concat, len, write_int, write_uint, Piece, MAX_TEXT,
};
pub use timers::{tof, ton, tp, Tof, Ton, Tp};

#[cfg(test)]
mod tests;
