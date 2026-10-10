# Independent B-R3 M2 numeric source review

Accepted; no blocker found in the three pinned files against the retained M1 core-src snapshot. No edits, formatter, builds, tests, checks or measurements run.

The checked signed/unsigned 64-bit arithmetic preserves the former wide-operation then checked-narrowing result. Original operands are already at most 64 bits after unchanged category/coercion checks. Signed minimum division by -1 remains Overflow; minimum remainder by -1 explicitly remains zero; zero modulo/division retain their distinct errors. Integer exponent negative/greater-than-u32 handling still precedes checked power, including zero/one bases. Narrow destination conversion remains in the existing helper; this review makes no claim that all wide helpers unlink.

REAL/LREAL construction now uses the existing numeric to_f64 helper, whose accepted value variants match the previous Real/LReal-or-integer branches exactly. Finite and f32-narrowing checks still return TypeMismatch here. Integer-to-f64 conversion preserves exact rounding because the original i128 inputs were exact promotions of i64/u64 values; tests compare original wide conversion and output bits at signed/unsigned and 2^53 boundaries, signed zero, nonfinite values and excluded categories.

The one validate_initializers inline(always) annotation is removed with no logic/budget change. This is restoration of normal compiler choice, not a claimed guaranteed saving.

New native differential tests retain the old complete arithmetic function as the oracle and cover all operand categories, widths, extremes and mixed kinds; no behavior assertions were weakened. Test imports, enum variants and enclosing include context were inspected. Source review does not prove compilation or execution. numeric_arith.rs is an include! fragment and must join explicit formatter preparation/check allocation before freeze.
