//! Tests of the retained encoder over cells (specification 11 §6.7, specification 34 §6.2 and §10;
//! C1-9d2b-2, R1). A retained value held in cells is written to the STRN v3 payload from its cells, its
//! text buffers and its compound members, and the bytes are those of the same value held as a `Value`.
//!
//! The payload layout, from which every expected byte below is worked out by hand: a `u32` entry count,
//! then for each entry a `u32` name length, the name and the tagged value. Integers are little-endian.
//! The tags are `INT` 3, `DINT` 4, `REAL` 10, `STRING` 24, `WSTRING` 25, `ARRAY` 28, `STRUCT` 29, `ENUM`
//! 30 and `NULL` 31. A `STRING` or `WSTRING` is its tag, the `u32` length of its UTF-8 text and the text.
//! An `ARRAY` is its tag, the `u32` element count, the `u32` dimension count, the `i64` lower and upper
//! bound of each dimension and the elements. A `STRUCT` is its tag, its type name as a string, the `u32`
//! member count, each member's name as a string and its value, and a final 0 byte for the absent physical
//! checkpoint. An `ENUM` is its tag, its type name and variant name as strings and its `i64` value.
//!
//! The values come from the fixture of `super::fixture`, whose ST source is in `super::tests`.

use smol_str::SmolStr;

use super::fixture::{
    prepared_with_holder, prepared_with_mode, BASE_G_INSTANCE, DINT_DECL, MODE, MODE_DECL,
    REAL_DECL, WIDE_DECL,
};
use super::tests::{field_segment, Fixture};
use super::CellArm;
use crate::error::RuntimeError;
use crate::memory::regions::CellStorage;
use crate::memory::view::{StoreAccess, ValueView};
use crate::memory::{Generation, MemoryLocation, VariableStorage};
use crate::retain::codec::{view_is_retainable, PayloadWriter};
use crate::value::Value;
use crate::vm::PreparedModule;

/// The payload of one entry named `v` that holds `view`.
fn view_payload(view: &ValueView<'_>) -> Result<Vec<u8>, RuntimeError> {
    let mut out = Vec::new();
    let mut writer = PayloadWriter::new(&mut out);
    writer.entry_view("v", view)?;
    writer.finish();
    Ok(out)
}

/// The payload of one entry named `v` that holds `value`, written by the codec's own `entry`.
fn value_payload(value: &Value) -> Result<Vec<u8>, RuntimeError> {
    let mut out = Vec::new();
    let mut writer = PayloadWriter::new(&mut out);
    writer.entry("v", value)?;
    writer.finish();
    Ok(out)
}

/// The payload of one entry named `v`, given the bytes of its tagged value: the count 1, the name `v`
/// (length 1) and `value`.
fn one_entry(value: &[u8]) -> Vec<u8> {
    let mut bytes = vec![1, 0, 0, 0, 1, 0, 0, 0, b'v'];
    bytes.extend_from_slice(value);
    bytes
}

/// Write `value`, of the `TYPE_TABLE` type `type_id`, into the cells of the global declaration
/// `declaration`.
fn write_global(fixture: &mut Fixture, declaration: u32, type_id: u32, value: &Value) {
    let range = fixture
        .arm
        .cells()
        .regions()
        .global_range(declaration as usize)
        .expect("the global owns cells");
    let (persistent, references) = fixture.arm.cells_mut().regions_mut().split_mut();
    let bytes = persistent
        .bytes_mut(range)
        .expect("the range lies in the persistent region");
    crate::memory::cells::write_value_in(
        bytes,
        type_id,
        value,
        &fixture.prepared,
        Some(references),
        range.start,
    )
    .expect("the value writes");
}

/// A fixture over `prepared`, holding the cells that construction writes and no instance.
fn constructed(prepared: PreparedModule) -> Fixture {
    let cells = CellStorage::construct(&prepared, Generation::FIRST, |_| Ok(()))
        .expect("the persistent region reserves");
    Fixture {
        prepared,
        arm: CellArm::new(cells),
        half: VariableStorage::new(),
        values: VariableStorage::new(),
    }
}

#[test]
fn a_view_of_cells_encodes_every_value_kind_to_the_bytes_of_the_value_tree() {
    // Specification 11 §6.7: the payload bytes of a retained value do not depend on where the runtime keeps
    // it. For each kind the fixture holds, the bytes written from the cells equal the bytes worked out by
    // hand from the layout in the module comment, and equal the bytes the codec writes for the same value
    // held as a `Value` (which are read from the `Value` store holding the same data). Worked out by hand:
    //
    // - `dint_g` 7: tag 4 and 7 as four bytes;
    // - `real_g` 1.5: tag 10 and 0x3FC00000 (sign 0, exponent 127, mantissa 0.5) as 00 00 C0 3F;
    // - `text_g` 'abc': tag 24, length 3 and the three bytes;
    // - `wide_g` 'hi': tag 25, UTF-8 length 2 and the two bytes, although the buffer holds four;
    // - `series_v` [1, 2, 3] of INT: tag 28, 3 elements, 1 dimension of 1 to 3, then three INTs of tag 3;
    // - `pair_v` of the structure Pair, x 10 and y 20: tag 29, the name 'Pair', 2 members, 'x' as INT 10, 'y'
    //   as INT 20 and the final 0.
    //
    // An encoder that wrote a text with the UTF-16 length of a WSTRING, an array without its bounds or a
    // structure without its closing byte would differ from these bytes.
    let fixture = Fixture::new();
    let reader = fixture.reader();
    let base = |name: &str| {
        reader
            .read_instance_var(BASE_G_INSTANCE, name)
            .unwrap_or_else(|| panic!("base_g holds {name}"))
    };
    let cases: Vec<(&str, ValueView<'_>, Value, Vec<u8>)> = vec![
        (
            "dint_g",
            reader.read_global("dint_g").expect("dint_g"),
            Value::DInt(7),
            vec![4, 7, 0, 0, 0],
        ),
        (
            "real_g",
            reader.read_global("real_g").expect("real_g"),
            Value::Real(1.5),
            vec![10, 0x00, 0x00, 0xC0, 0x3F],
        ),
        (
            "text_g",
            reader.read_global("text_g").expect("text_g"),
            Value::String(SmolStr::new("abc")),
            vec![24, 3, 0, 0, 0, b'a', b'b', b'c'],
        ),
        (
            "wide_g",
            reader.read_global("wide_g").expect("wide_g"),
            Value::WString(String::from("hi")),
            vec![25, 2, 0, 0, 0, b'h', b'i'],
        ),
        (
            "series_v",
            base("series_v"),
            super::fixture::series_value(),
            vec![
                28, 3, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 3, 0, 0, 0, 0, 0, 0, 0, 3, 1,
                0, 3, 2, 0, 3, 3, 0,
            ],
        ),
        (
            "pair_v",
            base("pair_v"),
            super::fixture::pair_value(),
            vec![
                29, 4, 0, 0, 0, b'P', b'a', b'i', b'r', 2, 0, 0, 0, 1, 0, 0, 0, b'x', 3, 10, 0, 1,
                0, 0, 0, b'y', 3, 20, 0, 0,
            ],
        ),
    ];
    for (label, view, value, bytes) in cases {
        let expected = one_entry(&bytes);
        assert_eq!(view_payload(&view), Ok(expected.clone()), "{label}: cells");
        assert_eq!(
            value_payload(&value),
            Ok(expected.clone()),
            "{label}: the value tree"
        );
        // The same data in the `Value` store, read as a view of the tree, gives the same bytes.
        let tree = match label {
            "series_v" | "pair_v" => fixture.values.read_instance_var(BASE_G_INSTANCE, label),
            global => fixture.values.read_global(global),
        }
        .expect("the Value store holds it");
        assert_eq!(
            view_payload(&tree),
            Ok(expected),
            "{label}: view of the tree"
        );
    }
}

#[test]
fn a_wide_text_in_cells_is_transcoded_to_its_utf8_length() {
    // Specification 11 §6.7 and specification 34 §6.2: a `WSTRING` buffer holds UTF-16 code units, and the
    // payload holds the text as UTF-8 with its UTF-8 byte length. Worked out by hand:
    //
    // - 'é' (U+00E9) then U+1F600: three code units (E9; the surrogate pair D83D DE00), six buffer bytes,
    //   and in UTF-8 C3 A9 then F0 9F 98 80, six bytes: tag 25, length 6 and those bytes;
    // - 'éé': two code units, four buffer bytes, four UTF-8 bytes C3 A9 C3 A9;
    // - the empty text: tag 25 and length 0.
    //
    // For the first text the number of code units (3) and the UTF-8 length (6) differ, and for the second
    // the UTF-8 length (4) differs from the number of code units (2), so an encoder that wrote either of
    // them as the length, or that copied the buffer bytes, would not write these bytes.
    let mut fixture = Fixture::new();
    for (text, utf8) in [
        ("é\u{1F600}", vec![0xC3, 0xA9, 0xF0, 0x9F, 0x98, 0x80]),
        ("éé", vec![0xC3, 0xA9, 0xC3, 0xA9]),
        ("", vec![]),
    ] {
        write_global(
            &mut fixture,
            WIDE_DECL,
            super::fixture::WSTRING_5,
            &Value::WString(String::from(text)),
        );
        let view = fixture.reader().read_global("wide_g").expect("wide_g");
        let mut value = vec![25, u8::try_from(utf8.len()).expect("a short text"), 0, 0, 0];
        value.extend_from_slice(&utf8);
        assert_eq!(view_payload(&view), Ok(one_entry(&value)), "{text:?}");
        assert_eq!(
            value_payload(&Value::WString(String::from(text))),
            Ok(one_entry(&value)),
            "{text:?}: the value tree"
        );
    }
}

#[test]
fn a_null_write_encodes_as_the_types_zero() {
    // Specification 34 MEM-12 and specification 11 §6.7: a DINT holds four bytes and no tag, so the
    // null value written into `dint_g` leaves zero bytes, which the payload writes as the DINT 0: the
    // DINT tag 4 and four zero bytes, as the `Value::DInt(0)` that reading them gives. An encoder
    // that still wrote the null tag 31 would disagree with the read.
    let mut fixture = Fixture::new();
    write_global(&mut fixture, DINT_DECL, super::fixture::DINT, &Value::Null);
    let view = fixture.reader().read_global("dint_g").expect("dint_g");
    assert_eq!(view_payload(&view), Ok(one_entry(&[4, 0, 0, 0, 0])));
    assert_eq!(
        value_payload(&Value::DInt(0)),
        Ok(one_entry(&[4, 0, 0, 0, 0]))
    );
}

#[test]
fn an_enumeration_in_cells_encodes_its_type_variant_and_value() {
    // Specification 11 §6.7: an enumeration value is its type name, the name of its variant and its value.
    // `mode_g : Mode` is written Stopped, whose value is 5 and whose position among the variants is 2, so an
    // encoder that wrote the position, or that named the variant by position, would be wrong. Worked out by
    // hand: tag 30, 'Mode' (length 4), 'Stopped' (length 7) and 5 as eight bytes.
    let mut fixture = constructed(prepared_with_mode());
    let stopped = Value::Enum(Box::new(crate::value::EnumValue::from_canonical_parts(
        SmolStr::new("Mode"),
        SmolStr::new("Stopped"),
        5,
    )));
    write_global(&mut fixture, MODE_DECL, MODE, &stopped);
    let view = fixture.reader().read_global("mode_g").expect("mode_g");
    let mut bytes = vec![30, 4, 0, 0, 0, b'M', b'o', b'd', b'e', 7, 0, 0, 0];
    bytes.extend_from_slice(b"Stopped");
    bytes.extend_from_slice(&[5, 0, 0, 0, 0, 0, 0, 0]);
    assert_eq!(view_payload(&view), Ok(one_entry(&bytes)));
    assert_eq!(value_payload(&stopped), Ok(one_entry(&bytes)));
}

#[test]
fn a_reference_or_instance_view_is_refused_by_the_cell_encoder() {
    // Specification 11 §6.7: a durable snapshot holds no reference and no instance. `ref_g` is a null
    // reference, `base_g` holds an instance, and the structure `holder_g` of `prepared_with_holder` holds a
    // reference in its member `r`. Each is refused with the error the codec gives a `Value::Reference` or a
    // `Value::Instance`, `RetainStore('cannot retain reference/instance values')`, and none is retainable,
    // so the capture leaves its variable out. The scalar `dint_g` is retainable. An encoder that wrote a
    // reference as null, an instance as its identity or a structure with its reference member skipped
    // would not return this error.
    let refused = RuntimeError::RetainStore("cannot retain reference/instance values".into());
    assert_eq!(value_payload(&Value::Reference(None)), Err(refused.clone()));
    assert_eq!(
        value_payload(&Value::Instance(BASE_G_INSTANCE)),
        Err(refused.clone())
    );

    let fixture = Fixture::new();
    let reader = fixture.reader();
    for name in ["ref_g", "base_g"] {
        let view = reader.read_global(name).expect("the global exists");
        assert_eq!(view_payload(&view), Err(refused.clone()), "{name}");
        assert_eq!(view_is_retainable(&view), Ok(false), "{name}");
    }
    let scalar = reader.read_global("dint_g").expect("dint_g");
    assert_eq!(view_is_retainable(&scalar), Ok(true));

    let holder = constructed(prepared_with_holder());
    let view = holder.reader().read_global("holder_g").expect("holder_g");
    assert_eq!(view_payload(&view), Err(refused));
    assert_eq!(view_is_retainable(&view), Ok(false));
    // The structure's other member is a plain DINT, which is retainable by itself.
    let member = holder
        .reader()
        .read_path(
            MemoryLocation::Global,
            super::fixture::HOLDER_G,
            &[field_segment("n")],
        )
        .expect("holder_g.n");
    assert_eq!(view_is_retainable(&member), Ok(true));
}

#[test]
fn a_non_finite_real_in_cells_is_refused_as_the_value_tree_refuses_it() {
    // Specification 11 §6.7: the file-backed retain boundary accepts a REAL or LREAL only when it is
    // finite. A NaN and an infinity in cells are refused with the codec's own error for the same `Value`,
    // `RetainStore('cannot persist non-finite REAL retain value')`; a finite value is written. An encoder
    // that copied the cell's bits would write the NaN into the checkpoint.
    let refused = RuntimeError::RetainStore("cannot persist non-finite REAL retain value".into());
    let mut fixture = Fixture::new();
    for value in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        write_global(
            &mut fixture,
            REAL_DECL,
            super::fixture::REAL,
            &Value::Real(value),
        );
        let view = fixture.reader().read_global("real_g").expect("real_g");
        assert_eq!(view_payload(&view), Err(refused.clone()), "{value}");
        assert_eq!(
            value_payload(&Value::Real(value)),
            Err(refused.clone()),
            "{value}: the value tree"
        );
    }
    write_global(
        &mut fixture,
        REAL_DECL,
        super::fixture::REAL,
        &Value::Real(-2.0),
    );
    let view = fixture.reader().read_global("real_g").expect("real_g");
    // -2.0 is sign 1, exponent 128, mantissa 0: 0xC0000000, as 00 00 00 C0.
    assert_eq!(view_payload(&view), Ok(one_entry(&[10, 0, 0, 0, 0xC0])));
}

#[test]
fn a_view_entry_written_by_pieces_is_the_entry_under_the_joined_name() {
    // Specification 11 §6.7: a retained program variable is keyed `@program/<program>/<variable>`, and the
    // capture appends the key as pieces. For `Plant` and `counter`, the key is 22 characters, '@program/'
    // (9), 'Plant' (5), '/' (1) and 'counter' (7): the count 1, the length 22, the key and `dint_g`'s value
    // 7. A piece-wise entry that wrote the pieces' lengths separately would not equal it.
    let fixture = Fixture::new();
    let view = fixture.reader().read_global("dint_g").expect("dint_g");
    let mut out = Vec::new();
    let mut writer = PayloadWriter::new(&mut out);
    writer
        .entry_view_pieces(&["@program/", "Plant", "/", "counter"], &view)
        .expect("the view writes");
    writer.finish();
    let mut expected = vec![1, 0, 0, 0, 22, 0, 0, 0];
    expected.extend_from_slice(b"@program/Plant/counter");
    expected.extend_from_slice(&[4, 7, 0, 0, 0]);
    assert_eq!(out, expected);
}
