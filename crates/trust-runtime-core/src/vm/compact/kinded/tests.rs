use super::*;
use crate::memory::InstanceId;

/// Every scalar cell, an instance and no value, at the ends of their ranges.
fn cells() -> [Cell; 33] {
    [
        Cell::Bool(false),
        Cell::Bool(true),
        Cell::Byte(0xA5),
        Cell::Word(0xBEEF),
        Cell::DWord(0xDEAD_BEEF),
        Cell::LWord(u64::MAX),
        Cell::SInt(i8::MIN),
        Cell::SInt(-1),
        Cell::Int(i16::MIN),
        Cell::DInt(i32::MIN),
        Cell::DInt(70_000),
        Cell::LInt(i64::MIN),
        Cell::LInt(-1),
        Cell::USInt(u8::MAX),
        Cell::UInt(u16::MAX),
        Cell::UDInt(u32::MAX),
        Cell::ULInt(u64::MAX),
        Cell::Real(-1.5),
        Cell::LReal(f64::MIN_POSITIVE),
        Cell::Time(-3_600_000_000_000),
        Cell::LTime(i64::MAX),
        Cell::Date(1),
        Cell::LDate(2),
        Cell::Tod(3),
        Cell::LTod(4),
        Cell::Dt(5),
        Cell::Ldt(6),
        Cell::Char(b'A'),
        Cell::WChar(0x263A),
        Cell::Instance(InstanceId(u32::MAX)),
        Cell::Instance(InstanceId(7)),
        Cell::Null,
        Cell::Real(f32::NAN),
    ]
}

/// A cell's bits give the cell back with its own kind, and a scalar's kind is its primitive's number
/// with a width; a signed integer's bits extend its sign. Catches a kind numbered otherwise than the
/// layout numbers the primitive, a sign lost or a value cut on the way to the bits, and a NaN's
/// payload changed.
#[test]
fn a_cell_and_its_bits_give_each_other() {
    for cell in cells() {
        let (kind, bits) = bits_of(cell).expect("every cell here has bits");
        let back = cell_of_bits(kind, bits).expect("the bits give a cell");
        match (cell, back) {
            (Cell::Real(a), Cell::Real(b)) => assert_eq!(a.to_bits(), b.to_bits()),
            _ => assert_eq!(back, cell),
        }
        if !matches!(cell, Cell::Instance(_) | Cell::Null) {
            assert!(is_scalar(kind), "{cell:?}");
        }
    }
    assert_eq!(bits_of(Cell::SInt(-1)), Some((6, u64::MAX)));
    assert_eq!(
        bits_of(Cell::DInt(i32::MIN)),
        Some((8, 0xFFFF_FFFF_8000_0000))
    );
    assert_eq!(bits_of(Cell::UDInt(u32::MAX)), Some((12, 0xFFFF_FFFF)));
    assert_eq!(bits_of(Cell::Bool(true)), Some((BOOL, 1)));
    assert_eq!(bits_of(Cell::Time(1)), Some((TIME, 1)));
}

/// The widths are the primitives' leaf widths (specification 34 MEM-12): one byte for BOOL, BYTE,
/// SINT, USINT and CHAR, two for WORD, INT, UINT and WCHAR, four for DWORD, DINT, UDINT and REAL,
/// eight for the rest; texts, instances and no value have none, and a native slot is no leaf a load
/// reads as its bits. Catches a leaf read or written with another width than the codec's.
#[test]
fn a_kind_has_its_primitives_leaf_width() {
    for (kinds, bytes) in [
        (&[1, 2, 6, 10, 26][..], 1),
        (&[3, 7, 11, 27][..], 2),
        (&[4, 8, 12, 14][..], 4),
        (&[5, 9, 13, 15, 16, 17, 18, 19, 20, 21, 22, 23][..], 8),
    ] {
        for &kind in kinds {
            assert_eq!(width(kind), Some(bytes), "{kind}");
        }
    }
    for kind in [24, 25, INSTANCE, NULL] {
        assert_eq!(width(kind), None, "{kind}");
    }
    for prim in [24, 25, 0x0100, DURATION_SLOT] {
        assert_eq!(load_kind(prim), None, "{prim}");
    }
    assert_eq!(load_kind(13), Some(13));
}

/// The integer kinds and their signedness, as `Integer::of` of the typed loop reads them: SINT to
/// LINT signed, USINT to ULINT unsigned, nothing else an integer. Catches a bit string or a
/// character taken for an integer, which `apply_binary` computes otherwise.
#[test]
fn only_the_integers_have_a_signedness() {
    for kind in 6..=9 {
        assert_eq!(
            integer(kind).map(|(signed, _)| signed),
            Some(true),
            "{kind}"
        );
    }
    for kind in 10..=13 {
        assert_eq!(
            integer(kind).map(|(signed, _)| signed),
            Some(false),
            "{kind}"
        );
    }
    for kind in [1, 2, 3, 4, 5, 14, 15, 16, 26, 27, INSTANCE, NULL] {
        assert_eq!(integer(kind), None, "{kind}");
    }
}
