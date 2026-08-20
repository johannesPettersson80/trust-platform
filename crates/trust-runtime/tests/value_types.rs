use trust_runtime::value::{
    DateTimeProfile, DateTimeValue, DateValue, Duration, LDateTimeValue, LDateValue,
    LTimeOfDayValue, TimeOfDayValue, Value,
};

#[test]
fn supports_elementary_types() {
    let values = vec![
        Value::Bool(false),
        Value::SInt(0),
        Value::Int(0),
        Value::DInt(0),
        Value::LInt(0),
        Value::USInt(0),
        Value::UInt(0),
        Value::UDInt(0),
        Value::ULInt(0),
        Value::Real(0.0),
        Value::LReal(0.0),
        Value::Byte(0),
        Value::Word(0),
        Value::DWord(0),
        Value::LWord(0),
        Value::Time(Duration::ZERO),
        Value::LTime(Duration::ZERO),
        Value::Date(DateValue::new(0)),
        Value::LDate(LDateValue::new(0)),
        Value::Tod(TimeOfDayValue::new(0)),
        Value::LTod(LTimeOfDayValue::new(0)),
        Value::Dt(DateTimeValue::new(0)),
        Value::Ldt(LDateTimeValue::new(0)),
        Value::String("".into()),
        Value::WString(String::new()),
        Value::Char(0),
        Value::WChar(0),
    ];

    assert!(matches!(
        values.as_slice(),
        [
            Value::Bool(false),
            Value::SInt(0),
            Value::Int(0),
            Value::DInt(0),
            Value::LInt(0),
            Value::USInt(0),
            Value::UInt(0),
            Value::UDInt(0),
            Value::ULInt(0),
            Value::Real(0.0),
            Value::LReal(0.0),
            Value::Byte(0),
            Value::Word(0),
            Value::DWord(0),
            Value::LWord(0),
            Value::Time(value_time),
            Value::LTime(value_ltime),
            Value::Date(value_date),
            Value::LDate(value_ldate),
            Value::Tod(value_tod),
            Value::LTod(value_ltod),
            Value::Dt(value_dt),
            Value::Ldt(value_ldt),
            Value::String(value_string),
            Value::WString(value_wstring),
            Value::Char(0),
            Value::WChar(0),
        ] if *value_time == Duration::ZERO
            && *value_ltime == Duration::ZERO
            && value_date.ticks() == 0
            && value_ldate.nanos() == 0
            && value_tod.ticks() == 0
            && value_ltod.nanos() == 0
            && value_dt.ticks() == 0
            && value_ldt.nanos() == 0
            && value_string.is_empty()
            && value_wstring.is_empty()
    ));

    let profile = DateTimeProfile::default();
    assert_eq!(profile.epoch.ticks(), 0);
    assert_eq!(profile.resolution, Duration::from_millis(1));
}
