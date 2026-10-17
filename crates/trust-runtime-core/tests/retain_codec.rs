//! The shared STRN v3 retain payload codec (specification 11 §6.7, specification 34 §10.1): a
//! value of every kind round-trips, the bytes of a known snapshot are unchanged, and a damaged
//! payload is refused with the codec's error.

use std::sync::Arc;

use trust_runtime_core::error::RuntimeError;
use trust_runtime_core::retain::codec::{decode_snapshot, encode_snapshot};
use trust_runtime_core::retain::RetainSnapshot;
use trust_runtime_core::value::{
    ArrayValue, DateTimeValue, DateValue, Duration, EnumValue, LDateTimeValue, LDateValue,
    LTimeOfDayValue, StructValue, TimeOfDayValue, Value,
};

/// One retained value of every kind the codec writes, in a fixed order.
fn snapshot_with_every_value_kind() -> RetainSnapshot {
    let fields = [
        ("x".into(), Value::DInt(1)),
        ("y".into(), Value::LReal(2.5)),
    ]
    .into_iter()
    .collect();
    let mut snapshot = RetainSnapshot::default();
    snapshot.insert("bool", Value::Bool(true));
    snapshot.insert("sint", Value::SInt(-1));
    snapshot.insert("int", Value::Int(-2));
    snapshot.insert("dint", Value::DInt(-3));
    snapshot.insert("lint", Value::LInt(-4));
    snapshot.insert("usint", Value::USInt(5));
    snapshot.insert("uint", Value::UInt(6));
    snapshot.insert("udint", Value::UDInt(7));
    snapshot.insert("ulint", Value::ULInt(8));
    snapshot.insert("real", Value::Real(1.5));
    snapshot.insert("lreal", Value::LReal(2.5));
    snapshot.insert("byte", Value::Byte(9));
    snapshot.insert("word", Value::Word(10));
    snapshot.insert("dword", Value::DWord(11));
    snapshot.insert("lword", Value::LWord(12));
    snapshot.insert("time", Value::Time(Duration::from_nanos(1_500_000)));
    snapshot.insert("ltime", Value::LTime(Duration::from_nanos(2_500_000)));
    snapshot.insert("date", Value::Date(DateValue::new(100)));
    snapshot.insert("ldate", Value::LDate(LDateValue::new(200)));
    snapshot.insert("tod", Value::Tod(TimeOfDayValue::new(300)));
    snapshot.insert("ltod", Value::LTod(LTimeOfDayValue::new(400)));
    snapshot.insert("dt", Value::Dt(DateTimeValue::new(500)));
    snapshot.insert("ldt", Value::Ldt(LDateTimeValue::new(600)));
    snapshot.insert("string", Value::String("text".into()));
    snapshot.insert("wstring", Value::WString(String::from("wide")));
    snapshot.insert("char", Value::Char(0x41));
    snapshot.insert("wchar", Value::WChar(0x42));
    snapshot.insert(
        "array",
        Value::Array(Box::new(
            ArrayValue::from_untyped_parts(
                vec![Value::DInt(1), Value::DInt(2), Value::DInt(3)],
                vec![(1, 3)],
            )
            .expect("three elements fill the 1..3 dimension"),
        )),
    );
    snapshot.insert(
        "struct",
        Value::Struct(Arc::new(StructValue::from_untyped_parts(
            "Point".into(),
            fields,
        ))),
    );
    snapshot.insert(
        "enum",
        Value::Enum(Box::new(EnumValue::from_canonical_parts(
            "Color".into(),
            "Red".into(),
            1,
        ))),
    );
    snapshot.insert("null", Value::Null);
    snapshot
}

#[test]
fn strn_v3_payload_round_trips_every_value_kind() {
    let snapshot = snapshot_with_every_value_kind();

    let encoded = encode_snapshot(&snapshot).expect("encode a snapshot of every value kind");
    let decoded = decode_snapshot(&encoded).expect("decode the encoded snapshot");

    // Every value kind must survive unchanged: a decoder that dropped a value, swapped two tags,
    // read a field with the wrong width or endianness, or mis-framed an array, structure or enum
    // would return a different snapshot here.
    assert_eq!(decoded, snapshot);
}

#[test]
fn strn_v3_payload_bytes_are_unchanged_for_a_known_snapshot() {
    let mut snapshot = RetainSnapshot::default();
    snapshot.insert("b", Value::Bool(true));
    snapshot.insert("i", Value::Int(-2));
    snapshot.insert("s", Value::String("ab".into()));

    // Worked out by hand from the format the code defines: the magic, the version, the payload
    // length, the value count, then each name and tagged value, then the payload CRC-32, then the
    // trailer. The payload is 31 bytes: 4 count + (5 name + 2 BOOL) + (5 name + 3 INT) +
    // (5 name + 7 STRING).
    let expected: [u8; 53] = [
        0x53, 0x54, 0x52, 0x4e, // magic "STRN"
        0x03, 0x00, // version 3 (u16 LE)
        0x1f, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // payload length 31 (u64 LE)
        0x03, 0x00, 0x00, 0x00, // value count 3 (u32 LE)
        0x01, 0x00, 0x00, 0x00, 0x62, // name "b": length 1 (u32 LE) + 'b'
        0x01, 0x01, // tag BOOL 1 + true 1
        0x01, 0x00, 0x00, 0x00, 0x69, // name "i": length 1 + 'i'
        0x03, 0xfe, 0xff, // tag INT 3 + -2 (i16 LE)
        0x01, 0x00, 0x00, 0x00, 0x73, // name "s": length 1 + 's'
        0x18, 0x02, 0x00, 0x00, 0x00, 0x61, 0x62, // tag STRING 24 + length 2 + "ab"
        0x56, 0x0b, 0x76, 0x06, // payload CRC-32 0x0676_0B56 (u32 LE)
        0x45, 0x4e, 0x44, 0x52, // trailer "ENDR"
    ];

    let encoded = encode_snapshot(&snapshot).expect("encode the known snapshot");
    assert_eq!(encoded, expected);
}

#[test]
fn a_damaged_payload_is_refused_with_its_error() {
    let mut snapshot = RetainSnapshot::default();
    snapshot.insert("count", Value::DInt(-7));
    let encoded = encode_snapshot(&snapshot).expect("encode a valid snapshot");

    // Flip one bit of the payload CRC, the four bytes before the four-byte trailer. The payload
    // itself is intact, so only the checksum check can refuse it; a codec that ignored the
    // checksum would return the snapshot.
    let mut flipped_crc = encoded.clone();
    let crc_byte = flipped_crc.len() - 8;
    flipped_crc[crc_byte] ^= 0x01;
    let error = decode_snapshot(&flipped_crc).expect_err("a flipped CRC byte must be refused");
    assert!(
        matches!(error, RuntimeError::RetainCorruption(_)),
        "expected retain corruption, got {error:?}"
    );
    assert!(
        error.to_string().contains("retain checksum mismatch"),
        "expected a checksum mismatch, got {error}"
    );

    // Truncate inside the payload, before its checksum and trailer. A codec that read past the end
    // or returned a partly decoded snapshot would not refuse it.
    let truncated = &encoded[..20];
    let error = decode_snapshot(truncated).expect_err("a truncated payload must be refused");
    assert!(
        matches!(error, RuntimeError::RetainCorruption(_)),
        "expected retain corruption, got {error:?}"
    );
    assert!(
        error.to_string().contains("retain data truncated"),
        "expected truncation, got {error}"
    );
}
