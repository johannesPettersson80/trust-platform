//! Direct-address images of the temporal types (IEC 61131-3 Table 10; spec 10 section 1.2 and spec
//! 12 section 4.1). LTIME, LDATE, LTOD and LDT are 64-bit families and each occupies one LWORD
//! image that carries its signed nanoseconds as a two's-complement bit pattern. DATE, TOD and DT
//! have no fixed width in Table 10, so no direct-address width accepts them.

use trust_runtime_core::error::RuntimeError;
use trust_runtime_core::io_address::IoSize;
use trust_runtime_core::io_image::{coerce_from_io, coerce_to_io, expected_size_for_type};
use trust_runtime_core::stdlib::conversions::ConversionType;
use trust_runtime_core::value::{
    DateTimeValue, DateValue, Duration, LDateTimeValue, LDateValue, LTimeOfDayValue,
    TimeOfDayValue, Value,
};

/// 1970-01-02 as nanoseconds since the LDATE epoch.
const ONE_DAY_NANOS: i64 = 86_400_000_000_000;
/// 01:02:03.004 as nanoseconds since midnight.
const CLOCK_NANOS: i64 = 3_723_004_000_000;

#[test]
fn long_temporal_types_occupy_one_long_word_image() {
    for ty in [
        ConversionType::LTime,
        ConversionType::LDate,
        ConversionType::LTod,
        ConversionType::Ldt,
    ] {
        assert_eq!(expected_size_for_type(ty), Some(IoSize::LWord), "{ty:?}");
    }
}

#[test]
fn long_temporal_images_carry_the_signed_nanoseconds_as_their_bit_pattern() {
    assert_eq!(
        coerce_to_io(
            Value::LTime(Duration::from_nanos(5_000_000_000)),
            ConversionType::LTime,
            IoSize::LWord,
        ),
        Ok(Value::LWord(5_000_000_000))
    );
    assert_eq!(
        coerce_to_io(
            Value::LTime(Duration::from_nanos(-1)),
            ConversionType::LTime,
            IoSize::LWord,
        ),
        Ok(Value::LWord(u64::MAX))
    );
    assert_eq!(
        coerce_to_io(
            Value::LDate(LDateValue::new(ONE_DAY_NANOS)),
            ConversionType::LDate,
            IoSize::LWord,
        ),
        Ok(Value::LWord(ONE_DAY_NANOS as u64))
    );
    assert_eq!(
        coerce_to_io(
            Value::LTod(LTimeOfDayValue::new(CLOCK_NANOS)),
            ConversionType::LTod,
            IoSize::LWord,
        ),
        Ok(Value::LWord(CLOCK_NANOS as u64))
    );
    assert_eq!(
        coerce_to_io(
            Value::Ldt(LDateTimeValue::new(-1)),
            ConversionType::Ldt,
            IoSize::LWord,
        ),
        Ok(Value::LWord(u64::MAX))
    );
}

#[test]
fn long_temporal_words_decode_back_to_their_declared_values() {
    assert_eq!(
        coerce_from_io(Value::LWord(u64::MAX), ConversionType::LTime),
        Ok(Value::LTime(Duration::from_nanos(-1)))
    );
    assert_eq!(
        coerce_from_io(Value::LWord(ONE_DAY_NANOS as u64), ConversionType::LDate),
        Ok(Value::LDate(LDateValue::new(ONE_DAY_NANOS)))
    );
    assert_eq!(
        coerce_from_io(Value::LWord(CLOCK_NANOS as u64), ConversionType::LTod),
        Ok(Value::LTod(LTimeOfDayValue::new(CLOCK_NANOS)))
    );
    assert_eq!(
        coerce_from_io(Value::LWord(u64::MAX), ConversionType::Ldt),
        Ok(Value::Ldt(LDateTimeValue::new(-1)))
    );
}

#[test]
fn long_temporal_images_reject_a_narrower_width_and_a_different_value_type() {
    assert_eq!(
        coerce_to_io(
            Value::LTime(Duration::ZERO),
            ConversionType::LTime,
            IoSize::DWord,
        ),
        Err(RuntimeError::TypeMismatch)
    );
    assert_eq!(
        coerce_to_io(
            Value::Time(Duration::ZERO),
            ConversionType::LTime,
            IoSize::LWord,
        ),
        Err(RuntimeError::TypeMismatch)
    );
    assert_eq!(
        coerce_from_io(Value::DWord(0), ConversionType::LDate),
        Err(RuntimeError::TypeMismatch)
    );
}

#[test]
fn date_tod_and_dt_have_no_direct_address_width_in_either_direction() {
    let widths = [IoSize::Byte, IoSize::Word, IoSize::DWord, IoSize::LWord];
    let cases = [
        (ConversionType::Date, Value::Date(DateValue::new(0))),
        (ConversionType::Tod, Value::Tod(TimeOfDayValue::new(0))),
        (ConversionType::Dt, Value::Dt(DateTimeValue::new(0))),
    ];
    for (ty, value) in cases {
        assert_eq!(expected_size_for_type(ty), None, "{ty:?}");
        assert_eq!(
            coerce_from_io(Value::LWord(0), ty),
            Err(RuntimeError::TypeMismatch),
            "{ty:?}"
        );
        for width in widths {
            assert_eq!(
                coerce_to_io(value.clone(), ty, width),
                Err(RuntimeError::TypeMismatch),
                "{ty:?} at {width:?}"
            );
        }
    }
}
