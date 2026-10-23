use super::*;
use alloc::{format, string::ToString};

fn check(value: f64) {
    let expected = value.to_string();
    assert_eq!(display(value), expected, "bits={:016x}", value.to_bits());
    let expected_real =
        if expected.contains('.') || expected.contains('e') || expected.contains('E') {
            expected.clone()
        } else {
            format!("{expected}.0")
        };
    assert_eq!(real(value), expected_real, "bits={:016x}", value.to_bits());
    if value.is_finite() {
        assert_eq!(
            parse(&expected).unwrap().to_bits(),
            value.to_bits(),
            "{expected}"
        );
    }
}

#[test]
fn compact_text_preserves_edges_and_every_binary_exponent_neighborhood() {
    for value in [
        0.0,
        -0.0,
        f64::MIN,
        f64::MAX,
        f64::MIN_POSITIVE,
        f64::EPSILON,
        f64::INFINITY,
        f64::NEG_INFINITY,
        f64::NAN,
        0.1,
        1e23,
        1e-7,
        1.2345678901234567,
    ] {
        check(value);
    }
    for exponent in 0..2047u64 {
        let pivot = exponent << 52;
        for bits in [
            pivot.saturating_sub(1),
            pivot,
            pivot + 1,
            pivot + 0x0007_ffff_ffff_ffff,
            pivot + 0x000f_ffff_ffff_ffff,
        ] {
            check(f64::from_bits(bits));
            check(f64::from_bits(bits | (1 << 63)));
        }
    }
}

#[test]
fn compact_text_matches_previous_spelling_and_bits_for_deterministic_samples() {
    let mut state = 0x915e_782b_aae9_620du64;
    for _ in 0..100_000 {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        check(f64::from_bits(state));
        check(f64::from(f32::from_bits(state as u32)));
    }
}

fn parse_bits(text: &str) {
    let expected = text.parse::<f64>();
    let actual = parse(text);
    match (actual, expected) {
        (Ok(actual), Ok(expected)) if expected.is_nan() => assert!(actual.is_nan(), "{text}"),
        (Ok(actual), Ok(expected)) => assert_eq!(actual.to_bits(), expected.to_bits(), "{text}"),
        (Err(_), Err(_)) => {}
        (actual, expected) => {
            panic!("parse disagreement for {text:?}: {actual:?} versus {expected:?}")
        }
    }
}

#[test]
fn compact_parser_matches_syntax_rounding_and_long_mantissas() {
    for text in ["", "+", "-", ".", "+.", "1.", ".1", "+.1", "1.e2", "1E-2",
        "0x1", "1_0", "1e", "1e+", "1e--2", "1 2", "1\0", " 1", "1 ", "NaN",
        "nan", "NAN", "-NaN", "+nan", "inf", "INF", "infinity", "Infinity", "+INF",
        "-infinity", "infinite", "nan(1)", "1e9999999999999999999999999999",
        "-1e-9999999999999999999999999999", "0e9999999999999999999999999999",
        "-0e-9999999999999999999999999999", "2.2250738585072012e-308",
        "2.4703282292062327208828439643411068618252990130716238221279284125033775363510437593264991818081799618989828234772285886546332835517796989819938739800539093906315035659515570226392290858392449105184435931802849936536152500319370457678249219365623669863658480757001585769269903706311928279558551332927834338409351978015531246597263579574622766465272827220056374006485499977096599470454020828166226237857393450736339007967761930577506740176324673600968951340535537458516661134223766678604162159680461914467291840300530057530849048765391711386591646239524912623653881879636239373280423891018672348497668235089863388587925628302755995657524455507255189313690836254779186948667994968324049705821028513185451396213837722826145437693412532098591327667236328125e-324"] {
        parse_bits(text);
    }
    for digits in [20, 100, 1000, 10_000] {
        let ones = "1".repeat(digits);
        for exponent in [-99999, -(digits as i32), -324, -308, 0, 308, 99999] {
            parse_bits(&format!("{ones}e{exponent}"));
            parse_bits(&format!("-0.{ones}e{exponent}"));
        }
    }
    // Decimal half-way neighborhoods beyond binary64's exact-integer range.
    for value in (1u64 << 53)..(1u64 << 53) + 1000 {
        parse_bits(&value.to_string());
        parse_bits(&format!("{value}.5"));
    }
}

#[test]
fn all_float_string_conversion_names_and_assertion_diagnostics_remain_available() {
    use crate::{error::RuntimeError, stdlib::StandardLibrary, value::Value};
    let library = StandardLibrary::new();
    // The product entry points retain their value types and normalization.
    let cases = [
        (" 1_2.5e1 ", 125.0),
        ("-0.0", -0.0),
        ("5e-324", f64::from_bits(1)),
    ];
    for (text, expected) in cases {
        let cleaned: String = text.trim().chars().filter(|c| *c != '_').collect();
        assert_eq!(parse(&cleaned).unwrap().to_bits(), expected.to_bits());
    }
    assert_eq!(parse("bad"), Err(RuntimeError::TypeMismatch));
    assert_eq!(
        crate::value::format_user_value(&Value::Real(0.1)),
        real(f64::from(0.1f32))
    );
    for (source, argument) in [
        ("STRING", Value::String(" 1_2.5e1 ".into())),
        ("WSTRING", Value::WString(" 1_2.5e1 ".into())),
    ] {
        assert_eq!(
            library.call(
                &format!("{source}_TO_REAL"),
                core::slice::from_ref(&argument)
            ),
            Ok(Value::Real(125.0))
        );
        assert_eq!(
            library.call(
                &format!("{source}_TO_LREAL"),
                core::slice::from_ref(&argument)
            ),
            Ok(Value::LReal(125.0))
        );
    }
    for (source, argument) in [("REAL", Value::Real(1.25)), ("LREAL", Value::LReal(1.25))] {
        assert_eq!(
            library.call(
                &format!("{source}_TO_STRING"),
                core::slice::from_ref(&argument)
            ),
            Ok(Value::String("1.25".into()))
        );
        assert_eq!(
            library.call(
                &format!("{source}_TO_WSTRING"),
                core::slice::from_ref(&argument)
            ),
            Ok(Value::WString("1.25".into()))
        );
    }
    for (source, argument) in [("REAL", Value::Real(-0.0)), ("LREAL", Value::LReal(-0.0))] {
        assert_eq!(
            library.call(
                &format!("{source}_TO_STRING"),
                core::slice::from_ref(&argument)
            ),
            Ok(Value::String("-0.0".into()))
        );
        assert_eq!(
            library.call(
                &format!("{source}_TO_WSTRING"),
                core::slice::from_ref(&argument)
            ),
            Ok(Value::WString("-0.0".into()))
        );
    }
    for text in ["nan", "inf", "1e9999"] {
        assert_eq!(
            library.call("STRING_TO_LREAL", &[Value::String(text.into())]),
            Err(RuntimeError::Overflow)
        );
    }
    assert_eq!(
        library.call(
            "ASSERT_NEAR",
            &[Value::LReal(1.0), Value::LReal(3.0), Value::LReal(0.1)]
        ),
        Err(RuntimeError::AssertionFailed(
            "ASSERT_NEAR failed: expected 1, actual 3, delta 0.1, diff 2".into()
        ))
    );
}

#[test]
fn decimal_halfway_spelling_keeps_the_previous_round_up_convention() {
    for (bits, expected) in [
        (0x3e60_0000_0000_0000, "0.000000029802322387695313"),
        (0xc08d_2f43_2000_0000, "-933.9077758789063"),
    ] {
        assert_eq!(display(f64::from_bits(bits)), expected);
        for neighbor in [bits - 1, bits, bits + 1, bits ^ (1 << 63)] {
            check(f64::from_bits(neighbor));
        }
    }
    let upper_even = 1_000_000_000_000_000.0 + 0.75;
    assert_eq!(display(upper_even), "1000000000000000.8");
    assert!(!is_lower_decimal_halfway(
        upper_even,
        b"10000000000000008",
        -1
    ));
    check(upper_even);
    check(-upper_even);
}
