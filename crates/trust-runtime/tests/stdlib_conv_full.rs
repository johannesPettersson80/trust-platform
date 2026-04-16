use trust_runtime::stdlib::StandardLibrary;
use trust_runtime::value::{
    DateTimeValue, DateValue, LDateTimeValue, LTimeOfDayValue, TimeOfDayValue, Value,
};

#[test]
fn conversion_full() {
    let lib = StandardLibrary::new();

    // Numeric conversions (round to nearest even).
    assert_eq!(
        lib.call("REAL_TO_INT", &[Value::Real(1.6)]).unwrap(),
        Value::Int(2)
    );
    assert_eq!(
        lib.call("REAL_TO_INT", &[Value::Real(1.4)]).unwrap(),
        Value::Int(1)
    );
    assert_eq!(
        lib.call("REAL_TO_INT", &[Value::Real(2.5)]).unwrap(),
        Value::Int(2)
    );
    assert_eq!(
        lib.call("REAL_TO_INT", &[Value::Real(-2.5)]).unwrap(),
        Value::Int(-2)
    );
    assert_eq!(
        lib.call("TRUNC_INT", &[Value::Real(1.6)]).unwrap(),
        Value::Int(1)
    );
    assert_eq!(
        lib.call("LREAL_TO_DINT", &[Value::LReal(42.0)]).unwrap(),
        Value::DInt(42)
    );
    assert_eq!(
        lib.call("TO_LREAL", &[Value::Int(-7)]).unwrap(),
        Value::LReal(-7.0)
    );

    // Bit string conversions.
    assert_eq!(
        lib.call("WORD_TO_BYTE", &[Value::Word(0x1234)]).unwrap(),
        Value::Byte(0x34)
    );
    assert_eq!(
        lib.call("BYTE_TO_WORD", &[Value::Byte(0x12)]).unwrap(),
        Value::Word(0x0012)
    );

    // Bit <-> numeric (binary transfer).
    assert_eq!(
        lib.call("SINT_TO_WORD", &[Value::SInt(18)]).unwrap(),
        Value::Word(0x0012)
    );
    assert_eq!(
        lib.call("WORD_TO_SINT", &[Value::Word(0x1234)]).unwrap(),
        Value::SInt(0x34)
    );

    // Bool conversions to integer.
    assert_eq!(
        lib.call("BOOL_TO_INT", &[Value::Bool(true)]).unwrap(),
        Value::Int(1)
    );

    // REAL/LREAL bit transfers.
    let bits = f32::to_bits(1.0);
    assert_eq!(
        lib.call("REAL_TO_DWORD", &[Value::Real(1.0)]).unwrap(),
        Value::DWord(bits)
    );
    assert_eq!(
        lib.call("DWORD_TO_REAL", &[Value::DWord(bits)]).unwrap(),
        Value::Real(1.0)
    );

    // TIME/DATE conversions.
    let date = Value::Date(DateValue::new(0));
    let tod = Value::Tod(TimeOfDayValue::new(3_600_000));
    let dt = Value::Dt(DateTimeValue::new(3_600_000));
    assert_eq!(
        lib.call("DT_TO_DATE", std::slice::from_ref(&dt)).unwrap(),
        date
    );
    assert_eq!(
        lib.call("DT_TO_TOD", std::slice::from_ref(&dt)).unwrap(),
        tod
    );
    let ldt = Value::Ldt(LDateTimeValue::new(3_600_000_000_000));
    assert_eq!(
        lib.call("LDT_TO_DT", std::slice::from_ref(&ldt)).unwrap(),
        dt
    );
    let ltod = Value::LTod(LTimeOfDayValue::new(3_600_000_000_000));
    assert_eq!(
        lib.call("LTOD_TO_TOD", std::slice::from_ref(&ltod))
            .unwrap(),
        tod
    );
    assert_eq!(
        lib.call("TOD_TO_LTOD", std::slice::from_ref(&tod)).unwrap(),
        ltod
    );
    assert_eq!(
        lib.call(
            "TIME_TO_DWORD",
            &[Value::Time(trust_runtime::value::Duration::from_millis(0))]
        )
        .unwrap(),
        Value::DWord(0)
    );
    assert_eq!(
        lib.call("DWORD_TO_TIME", &[Value::DWord(0)]).unwrap(),
        Value::Time(trust_runtime::value::Duration::from_millis(0))
    );
    assert_eq!(
        lib.call(
            "TIME_TO_DWORD",
            &[Value::Time(trust_runtime::value::Duration::from_millis(
                123
            ))]
        )
        .unwrap(),
        Value::DWord(123)
    );
    assert_eq!(
        lib.call("DWORD_TO_TIME", &[Value::DWord(123)]).unwrap(),
        Value::Time(trust_runtime::value::Duration::from_millis(123))
    );
    assert!(lib
        .call(
            "TIME_TO_DWORD",
            &[Value::Time(trust_runtime::value::Duration::from_millis(-1))]
        )
        .is_err());
    assert!(lib
        .call(
            "TIME_TO_DWORD",
            &[Value::Time(trust_runtime::value::Duration::from_millis(
                i64::from(u32::MAX) + 1
            ))]
        )
        .is_err());

    // String/char conversions.
    assert_eq!(
        lib.call("STRING_TO_WSTRING", &[Value::String("A".into())])
            .unwrap(),
        Value::WString("A".to_string())
    );
    assert_eq!(
        lib.call("STRING_TO_CHAR", &[Value::String("A".into())])
            .unwrap(),
        Value::Char(b'A')
    );
    assert_eq!(
        lib.call("BYTE_TO_CHAR", &[Value::Byte(b'A')]).unwrap(),
        Value::Char(b'A')
    );
    assert_eq!(
        lib.call("CHAR_TO_BYTE", &[Value::Char(b'A')]).unwrap(),
        Value::Byte(b'A')
    );
    assert_eq!(
        lib.call("WCHAR_TO_WORD", &[Value::WChar(b'B' as u16)])
            .unwrap(),
        Value::Word(b'B' as u16)
    );
    assert_eq!(
        lib.call("DINT_TO_STRING", &[Value::DInt(42)]).unwrap(),
        Value::String("42".into())
    );
    assert_eq!(
        lib.call("REAL_TO_STRING", &[Value::Real(1.25)]).unwrap(),
        Value::String("1.25".into())
    );
    assert_eq!(
        lib.call("DWORD_TO_STRING", &[Value::DWord(42)]).unwrap(),
        Value::String("42".into())
    );
    assert_eq!(
        lib.call("STRING_TO_DINT", &[Value::String("42".into())])
            .unwrap(),
        Value::DInt(42)
    );
    assert_eq!(
        lib.call("WCHAR_TO_CHAR", &[Value::WChar(b'B' as u16)])
            .unwrap(),
        Value::Char(b'B')
    );

    // BCD conversions.
    assert_eq!(
        lib.call("USINT_TO_BCD_BYTE", &[Value::USInt(25)]).unwrap(),
        Value::Byte(0x25)
    );
    assert_eq!(
        lib.call("BYTE_BCD_TO_UINT", &[Value::Byte(0x25)]).unwrap(),
        Value::UInt(25)
    );
    assert!(lib.call("BYTE_BCD_TO_UINT", &[Value::Byte(0xFA)]).is_err());
}
