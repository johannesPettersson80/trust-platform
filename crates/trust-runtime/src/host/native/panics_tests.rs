//! The panic machinery is named, and each of its calls gets the form that returns a fault instead
//! (specification 48 RU-14, RU-45).

use super::{advice, is_panic_symbol};

#[test]
fn the_panic_machinery_is_known_by_its_names() {
    // Catches a module check that misses a panic in either mangling, or calls an ordinary
    // function of `core` a panic.
    for name in [
        "_ZN4core9panicking5panic17h0123456789abcdefE",
        "_RNvNtCs1234_4core9panicking9panic_fmt",
        "_ZN4core9panicking18panic_bounds_check17h0123456789abcdefE",
        "panic_bounds_check",
    ] {
        assert!(is_panic_symbol(name), "{name}");
    }
    for name in ["_ZN4core3fmt5write17h0123456789abcdefE", "trust_entry_p0"] {
        assert!(!is_panic_symbol(name), "{name}");
    }
}

#[test]
fn each_panic_gets_the_form_that_returns_a_fault() {
    // RU-14: integer `/` and `%`, indexing, slicing, `unwrap` and `expect` and shifts each have a
    // form of the SDK or of `core` that returns the fault instead of panicking.
    let cases = [
        (
            "core::panicking::panic_const::panic_const_div_by_zero",
            "trust_plc::checked::div",
        ),
        (
            "core::panicking::panic_const::panic_const_rem_by_zero",
            "trust_plc::checked::rem",
        ),
        ("core::panicking::panic_bounds_check", "`get(index)`"),
        (
            "core::slice::index::slice_end_index_len_fail",
            "`get(range)`",
        ),
        ("core::option::unwrap_failed", "`ok_or(...)?`"),
        ("core::result::unwrap_failed", "`ok_or(...)?`"),
        (
            "core::panicking::panic_const::panic_const_shl_overflow",
            "`checked_shl`",
        ),
        ("core::panicking::panic_fmt", "rust_fault"),
    ];
    for (panic, expected) in cases {
        let (_, form) = advice(panic);
        assert!(form.contains(expected), "{panic}: {form}");
    }
}
