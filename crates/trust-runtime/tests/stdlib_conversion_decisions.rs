//! Text separators (spec 07 section 2.8, IEC 61131-3 section 6.3.2) and
//! SRC_TO_DST source normalization per family (spec 07 sections 2.2, 2.3, 2.8).

use trust_runtime::error::RuntimeError;
use trust_runtime::stdlib::StandardLibrary;
use trust_runtime::value::{DateTimeValue, DateValue, Duration, Value};

#[test]
fn text_separators_accept_one_underscore_between_two_digits() {
    let lib = StandardLibrary::new();

    assert_eq!(
        lib.call("STRING_TO_DINT", &[Value::String("1_000".into())]),
        Ok(Value::DInt(1000))
    );
    assert_eq!(
        lib.call("STRING_TO_DINT", &[Value::String("2#1111_1111".into())]),
        Ok(Value::DInt(255))
    );
    assert_eq!(
        lib.call("WSTRING_TO_DINT", &[Value::WString("16#FF_FF".into())]),
        Ok(Value::DInt(65_535))
    );
    assert_eq!(
        lib.call("STRING_TO_LREAL", &[Value::String("1_000.5".into())]),
        Ok(Value::LReal(1000.5))
    );
    assert_eq!(
        lib.call("STRING_TO_LREAL", &[Value::String("1.000_5".into())]),
        Ok(Value::LReal(1.0005))
    );
    assert_eq!(
        lib.call("STRING_TO_LREAL", &[Value::String("1e1_0".into())]),
        Ok(Value::LReal(1e10))
    );
}

#[test]
fn misplaced_separators_are_type_mismatch_for_integers_and_reals() {
    let lib = StandardLibrary::new();

    for text in ["_1", "1_", "1__0", "2#_1", "1._5"] {
        assert_eq!(
            lib.call("STRING_TO_DINT", &[Value::String(text.into())]),
            Err(RuntimeError::TypeMismatch),
            "integer {text}"
        );
        // The real parser reports invalid text as TypeMismatch.
        assert_eq!(
            lib.call("STRING_TO_LREAL", &[Value::String(text.into())]),
            Err(RuntimeError::TypeMismatch),
            "real {text}"
        );
    }
    assert_eq!(
        lib.call("WSTRING_TO_DINT", &[Value::WString("16#F__F".into())]),
        Err(RuntimeError::TypeMismatch)
    );
}

#[test]
fn integer_sources_fault_only_on_values_outside_their_own_type() {
    let lib = StandardLibrary::new();

    // INT#256 is not an SINT value, so the source is normalized and faults first.
    assert_eq!(
        lib.call("SINT_TO_BYTE", &[Value::Int(256)]),
        Err(RuntimeError::Overflow)
    );
    assert_eq!(
        lib.call("SINT_TO_BYTE", &[Value::Int(127)]),
        Ok(Value::Byte(127))
    );
    assert_eq!(
        lib.call("SINT_TO_INT", &[Value::Int(-5)]),
        Ok(Value::Int(-5))
    );
    assert_eq!(
        lib.call("USINT_TO_INT", &[Value::Int(-1)]),
        Err(RuntimeError::Overflow)
    );
}

#[test]
fn real_sources_normalize_to_real_then_round_or_fault() {
    let lib = StandardLibrary::new();

    // LREAL#1e300 is outside REAL: the REAL source normalization overflows.
    assert_eq!(
        lib.call("REAL_TO_INT", &[Value::LReal(1e300)]),
        Err(RuntimeError::Overflow)
    );
    assert_eq!(
        lib.call("REAL_TO_INT", &[Value::LReal(2.5)]),
        Ok(Value::Int(2))
    );
    assert_eq!(
        lib.call("REAL_TO_INT", &[Value::LReal(3.5)]),
        Ok(Value::Int(4))
    );
}

#[test]
fn bit_string_sources_keep_the_rightmost_bits_of_their_argument() {
    let lib = StandardLibrary::new();

    // A WORD argument is narrowed to its low byte (spec 07 section 2.2).
    assert_eq!(
        lib.call("BYTE_TO_INT", &[Value::Word(0x1FF)]),
        Ok(Value::Int(255))
    );
    // An INT argument is a binary transfer into BYTE (spec 07 section 2.3): no Overflow.
    assert_eq!(
        lib.call("BYTE_TO_INT", &[Value::Int(256)]),
        Ok(Value::Int(0))
    );
}

#[test]
fn bool_sources_are_exact_and_convert_to_integers() {
    let lib = StandardLibrary::new();

    assert_eq!(
        lib.call("BOOL_TO_INT", &[Value::Bool(true)]),
        Ok(Value::Int(1))
    );
    assert_eq!(
        lib.call("BOOL_TO_INT", &[Value::Byte(1)]),
        Err(RuntimeError::TypeMismatch)
    );
}

#[test]
fn character_sources_check_the_code_range_and_one_scalar() {
    let lib = StandardLibrary::new();

    assert_eq!(
        lib.call("CHAR_TO_INT", &[Value::Int(256)]),
        Err(RuntimeError::Overflow)
    );
    assert_eq!(
        lib.call("CHAR_TO_INT", &[Value::Char(65)]),
        Ok(Value::Int(65))
    );
    assert_eq!(
        lib.call("CHAR_TO_INT", &[Value::String("A".into())]),
        Ok(Value::Int(65))
    );
    assert_eq!(
        lib.call("CHAR_TO_INT", &[Value::String("AB".into())]),
        Err(RuntimeError::TypeMismatch)
    );
}

#[test]
fn time_and_date_sources_follow_the_listed_pairs_only() {
    let lib = StandardLibrary::new();

    assert_eq!(
        lib.call("TIME_TO_LTIME", &[Value::LTime(Duration::from_secs(5))]),
        Ok(Value::LTime(Duration::from_secs(5)))
    );
    // DATE and LDATE are distinct sources: neither normalizes to the other.
    assert_eq!(
        lib.call("LDATE_TO_DATE", &[Value::Date(DateValue::new(0))]),
        Err(RuntimeError::TypeMismatch)
    );
    // DT_TO_DATE keeps the calendar day, floored toward the epoch.
    assert_eq!(
        lib.call(
            "DT_TO_DATE",
            &[Value::Dt(DateTimeValue::new(1_672_617_599_999))]
        ),
        Ok(Value::Date(DateValue::new(1_672_531_200_000)))
    );
}

#[test]
fn bcd_sources_require_their_declared_type() {
    let lib = StandardLibrary::new();

    assert_eq!(
        lib.call("BYTE_BCD_TO_USINT", &[Value::Byte(0x42)]),
        Ok(Value::USInt(42))
    );
    assert_eq!(
        lib.call("BYTE_BCD_TO_USINT", &[Value::Word(0x42)]),
        Err(RuntimeError::TypeMismatch)
    );
    assert_eq!(
        lib.call("BYTE_BCD_TO_USINT", &[Value::Byte(0x4A)]),
        Err(RuntimeError::TypeMismatch)
    );
    assert_eq!(
        lib.call("USINT_TO_BCD_BYTE", &[Value::USInt(42)]),
        Ok(Value::Byte(0x42))
    );
    assert_eq!(
        lib.call("USINT_TO_BCD_BYTE", &[Value::Int(42)]),
        Err(RuntimeError::TypeMismatch)
    );
    assert_eq!(
        lib.call("USINT_TO_BCD_BYTE", &[Value::USInt(100)]),
        Err(RuntimeError::Overflow)
    );
}

#[test]
fn numeric_arguments_of_a_string_source_become_decimal_text_first() {
    let lib = StandardLibrary::new();

    assert_eq!(
        lib.call("STRING_TO_INT", &[Value::Int(5)]),
        Ok(Value::Int(5))
    );
}
