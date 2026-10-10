use trust_runtime_core::bytecode::{IoBinding, TypeData, TypeEntry, TypeKind, TypeTable};
use trust_runtime_core::error::RuntimeError;
use trust_runtime_core::io_address::{IoAddress, IoSize};
use trust_runtime_core::io_image::{
    coerce_from_io, coerce_to_io, read_flat_image, write_flat_image, IoEnumBinding, IoValueCodec,
    PreparedIoBinding,
};
use trust_runtime_core::stdlib::conversions::ConversionType;
use trust_runtime_core::value::{Duration, EnumValue, Value};

#[test]
fn shared_flat_codec_keeps_zero_fill_little_endian_and_neighbor_bits() {
    let address = IoAddress::parse("%QW3").unwrap();
    assert_eq!(read_flat_image(&[], &address), Ok(Value::Word(0)));
    let mut image = vec![0xaa; 7];
    write_flat_image(&mut image, &address, Value::Word(0x1234)).unwrap();
    assert_eq!(image, vec![0xaa, 0xaa, 0xaa, 0x34, 0x12, 0xaa, 0xaa]);
    assert_eq!(read_flat_image(&image, &address), Ok(Value::Word(0x1234)));
    let bit = IoAddress::parse("%QX0.1").unwrap();
    write_flat_image(&mut image, &bit, Value::Bool(false)).unwrap();
    assert_eq!(image[0], 0xa8);
    let mut invalid = bit;
    invalid.bit = 8;
    let before = image.clone();
    assert!(write_flat_image(&mut image, &invalid, Value::Bool(true)).is_err());
    assert_eq!(image, before);
}
#[test]
fn shared_typed_codec_retains_signed_bits_time_and_finite_float_errors() {
    assert_eq!(
        coerce_from_io(Value::Word(0xffff), ConversionType::Int),
        Ok(Value::Int(-1))
    );
    assert_eq!(
        coerce_from_io(Value::DWord(25), ConversionType::Time),
        Ok(Value::Time(Duration::from_millis(25)))
    );
    assert_eq!(
        coerce_to_io(Value::Int(-1), ConversionType::Int, IoSize::Word),
        Ok(Value::Word(0xffff))
    );
    assert!(matches!(
        coerce_from_io(Value::DWord(f32::NAN.to_bits()), ConversionType::Real),
        Err(RuntimeError::IoDriver(_))
    ));
    assert!(matches!(
        coerce_to_io(Value::LReal(f64::MAX), ConversionType::Real, IoSize::DWord),
        Err(RuntimeError::IoDriver(_))
    ));
}
#[test]
fn preparation_restores_declared_string_window_and_overflow_is_nonmutating() {
    let strings = vec!["%QB20".into()];
    let types = TypeTable {
        offsets: vec![],
        entries: vec![TypeEntry {
            kind: TypeKind::Primitive,
            name_idx: None,
            data: TypeData::Primitive {
                prim_id: 24,
                max_length: 8,
            },
        }],
    };
    let binding = PreparedIoBinding::from_tables(
        &IoBinding {
            address_str_idx: 0,
            ref_idx: 3,
            type_id: Some(0),
        },
        &types,
        &strings,
    )
    .unwrap();
    assert_eq!(binding.address.size, IoSize::Bytes(8));
    assert_eq!(binding.ref_idx, 3);
    let mut image = vec![0x55; 28];
    write_flat_image(&mut image, &binding.address, Value::String("abc".into())).unwrap();
    assert_eq!(&image[20..], b"abc\0\0\0\0\0");
    let before = image.clone();
    assert_eq!(
        write_flat_image(
            &mut image,
            &binding.address,
            Value::String("123456789".into())
        ),
        Err(RuntimeError::Overflow)
    );
    assert_eq!(image, before);
}
#[test]
fn enum_codec_keeps_closed_identity_and_rejects_undeclared_image_values() {
    let codec = IoValueCodec {
        wire_type: Some(ConversionType::Int),
        enum_type: Some(IoEnumBinding {
            type_name: "Mode".into(),
            variants: vec![("Run".into(), 2)],
        }),
        string_capacity: None,
    };
    let expected = Value::Enum(Box::new(EnumValue::from_canonical_parts(
        "Mode".into(),
        "Run".into(),
        2,
    )));
    assert_eq!(codec.decode(Value::Word(2)), Ok(expected.clone()));
    assert_eq!(codec.encode(expected, IoSize::Word), Ok(Value::Word(2)));
    assert!(matches!(
        codec.decode(Value::Word(3)),
        Err(RuntimeError::IoDriver(_))
    ));
    let forged = Value::Enum(Box::new(EnumValue::from_canonical_parts(
        "Other".into(),
        "Run".into(),
        2,
    )));
    assert_eq!(
        codec.encode(forged, IoSize::Word),
        Err(RuntimeError::TypeMismatch)
    );
}
