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
mod bistables;
mod bits;
mod convert;
mod counters;
mod dates;
mod edges;
mod math;
mod select;
mod text;
mod text_functions;
mod timers;

pub use arith::{
    abs, add, div, fadd, fdiv, finite, fmul, fneg, fpow, fsub, mul, neg, pow, rem, sub, Integer,
    Real, Signed,
};
pub use bistables::{rs, sr, Rs, Sr};
pub use bits::{rol, ror, shift_bool, shift_count, shl, shr, Bits};
pub use convert::{
    bits_to_real32, bits_to_real64, dword_to_time, real32_to_real, real64_to_real32, real_to_int,
    round_ties_to_even, sint_to, sint_to_real32, sint_to_real64, time_to_dword, uint_to,
    uint_to_real32, uint_to_real64, IntTarget, Rounding,
};
pub use counters::{
    ctd_dint, ctd_int, ctd_lint, ctd_udint, ctd_ulint, ctu_dint, ctu_int, ctu_lint, ctu_udint,
    ctu_ulint, ctud_dint, ctud_int, ctud_lint, ctud_udint, ctud_ulint, CtdDint, CtdInt, CtdLint,
    CtdUdint, CtdUlint, CtuDint, CtuInt, CtuLint, CtuUdint, CtuUlint, CtudDint, CtudInt, CtudLint,
    CtudUdint, CtudUlint,
};
pub use dates::{
    concat_date, concat_date_ltod, concat_date_tod, concat_dt, concat_ldt, concat_ltod, concat_tod,
    day_of_week, dt_to_date, dt_to_ltod, dt_to_tod, ldt_to_date, ldt_to_ltod, ldt_to_tod,
    nanos_add, nanos_to_ticks, scale_real, scale_signed, scale_unsigned, split_date, split_dt,
    split_ldt, split_ltod, split_tod, ticks_add, ticks_diff, ticks_to_nanos, NANOS_PER_DAY,
    NANOS_PER_TICK, TICKS_PER_DAY,
};
pub use edges::{f_trig, r_trig, FTrig, RTrig};
pub use math::{atan2_32, atan2_64, expt32, expt64, real32, real64, Math};
pub use select::{at, fabs, limit, max, min, mux_index, subrange};
pub use text::{
    assign, assign_bytes, compare, concat, len, write_int, write_uint, Piece, MAX_TEXT,
};
pub use text_functions::{
    char_at, delete, find, insert, left, len_int, mid, replace, right, set_char,
};
pub use timers::{tof, ton, tp, Tof, Ton, Tp};

#[cfg(test)]
mod tests;
