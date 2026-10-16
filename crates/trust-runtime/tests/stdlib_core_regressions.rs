//! Regression tests for runtime core standard functions: shift counts over the
//! full 64-bit range (spec 07 section 4), string lengths past the end (section
//! 8.1), single-sign numeric text (section 2.8) and negative durations in
//! bit-string conversions (section 2.8).

use trust_runtime::error::RuntimeError;
use trust_runtime::stdlib::StandardLibrary;
use trust_runtime::value::{Duration, Value};

// A count is passed as ULINT, and also as LINT when it fits in i64.
fn count_values(count: u64) -> Vec<Value> {
    let mut values = vec![Value::ULInt(count)];
    if let Ok(signed) = i64::try_from(count) {
        values.push(Value::LInt(signed));
    }
    values
}

#[test]
fn byte_shift_and_rotate_counts_cover_the_full_64_bit_range() {
    let lib = StandardLibrary::new();
    let input = Value::Byte(0xB5);
    // Count, then the expected SHL, SHR, ROL and ROR results.
    let rows: [(u64, u8, u8, u8, u8); 7] = [
        (7, 0x80, 0x01, 0xDA, 0x6B),
        (8, 0x00, 0x00, 0xB5, 0xB5),
        (9, 0x00, 0x00, 0x6B, 0xDA),
        (1 << 32, 0x00, 0x00, 0xB5, 0xB5),
        ((1 << 32) + 1, 0x00, 0x00, 0x6B, 0xDA),
        (i64::MAX as u64, 0x00, 0x00, 0xDA, 0x6B),
        (u64::MAX, 0x00, 0x00, 0xDA, 0x6B),
    ];
    for (count, shl, shr, rol, ror) in rows {
        for count_value in count_values(count) {
            for (name, expected) in [("SHL", shl), ("SHR", shr), ("ROL", rol), ("ROR", ror)] {
                assert_eq!(
                    lib.call(name, &[input.clone(), count_value.clone()]),
                    Ok(Value::Byte(expected)),
                    "{name} with count {count_value:?}"
                );
            }
        }
    }
}

#[test]
fn lword_shift_and_rotate_counts_cover_the_full_64_bit_range() {
    // INPUT rotated left and right by one bit.
    const INPUT: u64 = 0x8123_4567_89AB_CDEF;
    const ROTL1: u64 = 0x0246_8ACF_1357_9BDF;
    const ROTR1: u64 = 0xC091_A2B3_C4D5_E6F7;
    let lib = StandardLibrary::new();
    let input = Value::LWord(INPUT);
    // Count, then the expected SHL, SHR, ROL and ROR results.
    let rows: [(u64, u64, u64, u64, u64); 7] = [
        (63, 0x8000_0000_0000_0000, 0x1, ROTR1, ROTL1),
        (64, 0, 0, INPUT, INPUT),
        (65, 0, 0, ROTL1, ROTR1),
        (1 << 32, 0, 0, INPUT, INPUT),
        ((1 << 32) + 1, 0, 0, ROTL1, ROTR1),
        (i64::MAX as u64, 0, 0, ROTR1, ROTL1),
        (u64::MAX, 0, 0, ROTR1, ROTL1),
    ];
    for (count, shl, shr, rol, ror) in rows {
        for count_value in count_values(count) {
            for (name, expected) in [("SHL", shl), ("SHR", shr), ("ROL", rol), ("ROR", ror)] {
                assert_eq!(
                    lib.call(name, &[input.clone(), count_value.clone()]),
                    Ok(Value::LWord(expected)),
                    "{name} with count {count_value:?}"
                );
            }
        }
    }
}

#[test]
fn negative_shift_counts_remain_type_mismatch_at_every_width() {
    let lib = StandardLibrary::new();
    for count in [Value::LInt(-1), Value::LInt(i64::MIN), Value::Int(-1)] {
        for name in ["SHL", "SHR", "ROL", "ROR"] {
            for input in [Value::Byte(0xB5), Value::LWord(1)] {
                assert_eq!(
                    lib.call(name, &[input, count.clone()]),
                    Err(RuntimeError::TypeMismatch),
                    "{name} with count {count:?}"
                );
            }
        }
    }
}

#[test]
fn string_length_past_the_end_clamps_to_the_end() {
    let lib = StandardLibrary::new();
    let huge = Value::LInt(i64::MAX);
    let two = Value::Int(2);
    let narrow = Value::String("ABCDEF".into());
    let narrow_xy = Value::String("XY".into());
    let wide = Value::WString("ABCDEF".into());
    let wide_xy = Value::WString("XY".into());
    assert_eq!(
        lib.call("MID", &[narrow.clone(), huge.clone(), two.clone()]),
        Ok(Value::String("BCDEF".into()))
    );
    assert_eq!(
        lib.call("DELETE", &[narrow.clone(), huge.clone(), two.clone()]),
        Ok(Value::String("A".into()))
    );
    assert_eq!(
        lib.call("REPLACE", &[narrow, narrow_xy, huge.clone(), two.clone()]),
        Ok(Value::String("AXY".into()))
    );
    assert_eq!(
        lib.call("MID", &[wide.clone(), huge.clone(), two.clone()]),
        Ok(Value::WString("BCDEF".into()))
    );
    assert_eq!(
        lib.call("DELETE", &[wide.clone(), huge.clone(), two.clone()]),
        Ok(Value::WString("A".into()))
    );
    assert_eq!(
        lib.call("REPLACE", &[wide, wide_xy, huge, two]),
        Ok(Value::WString("AXY".into()))
    );
}

#[test]
fn numeric_text_accepts_at_most_one_sign() {
    let lib = StandardLibrary::new();
    // Spec 07 section 2.8: one optional sign, on the digits only. A second sign and a
    // sign on the base are malformed text.
    for text in [
        "2#+-1", "2#--1", "2#++1", "2#-+1", "+-1", "--1", "++1", "-+1", "+2#1", "-2#1",
    ] {
        assert_eq!(
            lib.call("STRING_TO_DINT", &[Value::String(text.into())]),
            Err(RuntimeError::TypeMismatch),
            "{text}"
        );
    }
    assert_eq!(
        lib.call("WSTRING_TO_DINT", &[Value::WString("2#+-1".into())]),
        Err(RuntimeError::TypeMismatch)
    );
    assert_eq!(
        lib.call("STRING_TO_DINT", &[Value::String("2#-1".into())]),
        Ok(Value::DInt(-1))
    );
    assert_eq!(
        lib.call("STRING_TO_DINT", &[Value::String("-1".into())]),
        Ok(Value::DInt(-1))
    );
    assert_eq!(
        lib.call("STRING_TO_DINT", &[Value::String("2#+1".into())]),
        Ok(Value::DInt(1))
    );
}

#[test]
fn time_to_dword_rejects_negative_durations_before_truncating_to_millis() {
    let lib = StandardLibrary::new();
    let time = |duration: Duration| lib.call("TIME_TO_DWORD", &[Value::Time(duration)]);
    assert_eq!(time(Duration::from_nanos(-1)), Err(RuntimeError::Overflow));
    assert_eq!(
        time(Duration::from_nanos(-999_999)),
        Err(RuntimeError::Overflow)
    );
    assert_eq!(time(Duration::from_millis(-1)), Err(RuntimeError::Overflow));
    assert_eq!(time(Duration::ZERO), Ok(Value::DWord(0)));
    assert_eq!(
        time(Duration::from_millis(4_294_967_295)),
        Ok(Value::DWord(u32::MAX))
    );
    assert_eq!(
        time(Duration::from_millis(4_294_967_296)),
        Err(RuntimeError::Overflow)
    );
}
