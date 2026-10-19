//! Specification 34 MEM-12: a leaf's bytes at its declared size, little-endian, with no tag.

use super::*;
use crate::memory::cells::Cell;
use crate::memory::layout::primitive_shape;

/// The bytes a leaf of `prim_id` takes, from the layout's shape (MEM-12).
fn width(prim_id: u16) -> usize {
    primitive_shape(prim_id, 0).expect("a primitive").bytes as usize
}

/// Specification 34 MEM-12: each primitive is its declared size, little-endian, and reads back as the
/// value it was given. Worked by hand: `INT` -2 is `FE FF`, `DINT` 0x12345678 is `78 56 34 12`, `BOOL`
/// TRUE is `01`, `REAL` 1.5 is `00 00 C0 3F`, a `TIME` of 5 ticks is 8 bytes `05 00 .. 00`. Catches a
/// codec that writes big-endian, that writes more bytes than the declared size, and one that reads
/// another primitive's width.
#[test]
fn every_primitive_round_trips_at_its_declared_size() {
    let cases: [(u16, Cell, &[u8]); 8] = [
        (1, Cell::Bool(true), &[1]),
        (7, Cell::Int(-2), &[0xFE, 0xFF]),
        (8, Cell::DInt(0x1234_5678), &[0x78, 0x56, 0x34, 0x12]),
        (14, Cell::Real(1.5), &[0x00, 0x00, 0xC0, 0x3F]),
        (16, Cell::Time(5), &[5, 0, 0, 0, 0, 0, 0, 0]),
        (2, Cell::Byte(0xAB), &[0xAB]),
        (11, Cell::UInt(0xBEEF), &[0xEF, 0xBE]),
        (27, Cell::WChar(0x20AC), &[0xAC, 0x20]),
    ];
    for (prim_id, cell, expected) in cases {
        assert_eq!(width(prim_id), expected.len(), "primitive {prim_id}");
        // One byte of guard after the leaf, which the codec must not touch.
        let mut bytes = vec![0xEE; expected.len() + 1];
        encode(LeafKind::Primitive(prim_id), cell, &mut bytes).expect("encodes");
        assert_eq!(&bytes[..expected.len()], expected, "primitive {prim_id}");
        assert_eq!(
            bytes[expected.len()],
            0xEE,
            "primitive {prim_id} wrote past its size"
        );
        assert_eq!(
            decode(LeafKind::Primitive(prim_id), &bytes).expect("decodes"),
            cell
        );
    }
}

/// Specification 34 MEM-12: an enumeration is stored as its base type. An enumeration on `DWORD`
/// (primitive 4) with the value 2 is `02 00 00 00`, and reads back as that enumeration's value. A value
/// that its base cannot hold is refused and nothing is written. Catches an enumeration stored with its
/// type index in the block, and a base wider or narrower than declared.
#[test]
fn an_enumeration_is_stored_as_its_base_type() {
    let kind = LeafKind::Enumeration {
        type_idx: 17,
        base: 4,
    };
    let mut bytes = [0u8; 4];
    encode(
        kind,
        Cell::Enum {
            type_idx: 17,
            value: 2,
        },
        &mut bytes,
    )
    .expect("encodes");
    assert_eq!(bytes, [2, 0, 0, 0]);
    assert_eq!(
        decode(kind, &bytes).expect("decodes"),
        Cell::Enum {
            type_idx: 17,
            value: 2
        }
    );
    let small = LeafKind::Enumeration {
        type_idx: 3,
        base: 6,
    };
    let mut one = [9u8; 1];
    assert!(encode(
        small,
        Cell::Enum {
            type_idx: 3,
            value: 300
        },
        &mut one
    )
    .is_err());
    assert_eq!(one, [9], "a refused value writes nothing");
}

/// Specification 34 MEM-12 and specification 02 §6: a `STRING[n]` is a 2-byte length and `n` bytes of
/// UTF-8, and a text longer than `n` bytes is cut at the last whole character that fits, as
/// assignment cuts it. `STRING[5]` given "aä€" (1 + 2 + 3 = 6 bytes) keeps "aä" (3 bytes): length
/// `03 00`, then `61 C3 A4`. A `WSTRING[2]` given "a€b" keeps "a€" as the code units `61 00 AC 20`,
/// length 2. Catches a cut inside a character, a length counted in characters for `STRING`, and a
/// `WSTRING` written as UTF-8.
#[test]
fn a_text_is_its_length_and_its_characters_cut_as_assignment_cuts() {
    let mut narrow = [0u8; 2 + 5];
    write_text(&mut narrow, 2, 5, "aä€", false).expect("writes");
    assert_eq!(&narrow[..5], &[3, 0, 0x61, 0xC3, 0xA4]);
    assert_eq!(
        read_text(&narrow, 2, false).expect("reads"),
        TextBytes::Narrow("aä")
    );
    let mut wide = [0u8; 2 + 4];
    write_text(&mut wide, 2, 4, "a€b", true).expect("writes");
    assert_eq!(wide, [2, 0, 0x61, 0x00, 0xAC, 0x20]);
    let units: Vec<u16> = read_text(&wide, 2, true).expect("reads").units().collect();
    assert_eq!(units, vec![0x61, 0x20AC]);
}

/// Specification 34 MEM-12: a text leaf's bytes depend only on its text. "abcde" and then "ab" written
/// into a `STRING[5]` leave `02 00 61 62 00 00 00`, and "ab" and then "a" into a `WSTRING[2]` leave
/// `01 00 61 00 00 00`. Catches a write that keeps the earlier text's characters after the new one.
#[test]
fn a_shorter_text_clears_the_rest_of_its_leaf() {
    let mut narrow = [0u8; 2 + 5];
    write_text(&mut narrow, 2, 5, "abcde", false).expect("writes");
    write_text(&mut narrow, 2, 5, "ab", false).expect("writes");
    assert_eq!(narrow, [2, 0, 0x61, 0x62, 0, 0, 0]);
    let mut wide = [0u8; 2 + 4];
    write_text(&mut wide, 2, 4, "ab", true).expect("writes");
    write_text(&mut wide, 2, 4, "a", true).expect("writes");
    assert_eq!(wide, [1, 0, 0x61, 0, 0, 0]);
}

/// Specification 34 MEM-12: a declared length above 65,535 takes a 4-byte length field. A text of
/// 70,000 bytes written into a `STRING[70000]` reads back whole with its length `70 11 01 00`.
/// Catches a 2-byte length that wraps at 65,536.
#[test]
fn a_long_text_takes_a_four_byte_length() {
    let text = "x".repeat(70_000);
    let mut bytes = vec![0u8; 4 + 70_000];
    write_text(&mut bytes, 4, 70_000, &text, false).expect("writes");
    assert_eq!(&bytes[..4], &70_000u32.to_le_bytes());
    assert_eq!(
        read_text(&bytes, 4, false).expect("reads"),
        TextBytes::Narrow(text.as_str())
    );
}

/// Specification 34 MEM-12: an instance leaf is the identity and a byte that says the leaf holds one,
/// and a reference leaf's first byte says whether it holds a reference; an empty leaf reads as empty.
/// Catches an empty instance leaf read as instance 0, and a null reference read as a reference.
#[test]
fn instance_and_reference_leaves_say_whether_they_hold_one() {
    let mut instance = [0u8; 8];
    assert_eq!(
        decode(LeafKind::Instance, &instance).expect("decodes"),
        Cell::Null
    );
    encode(
        LeafKind::Instance,
        Cell::Instance(InstanceId(7)),
        &mut instance,
    )
    .expect("encodes");
    assert_eq!(
        decode(LeafKind::Instance, &instance).expect("decodes"),
        Cell::Instance(InstanceId(7))
    );
    let mut reference = [0u8; 16];
    assert_eq!(
        decode(LeafKind::Reference, &reference).expect("decodes"),
        Cell::NullRef
    );
    encode(
        LeafKind::Reference,
        Cell::Ref(crate::memory::cells::RefSlot(0)),
        &mut reference,
    )
    .expect("encodes");
    assert_eq!(reference[0], 1);
}

/// The native counter slot of STBC 2.0 (primitive `0x0100`) stays unbound until its first call and
/// then keeps the caller's integer width and signedness (`primitive_value_matches`). Unbound, its 16
/// bytes read as no value; bound with a `UINT` 40000, they read back as that `UINT`. Catches a slot
/// read as 0 before its first call, and one that loses the caller's kind.
#[test]
fn a_native_counter_slot_is_unbound_until_its_first_call() {
    let mut slot = [0u8; 16];
    assert_eq!(width(0x0100), 16);
    assert_eq!(
        decode(LeafKind::Primitive(0x0100), &slot).expect("decodes"),
        Cell::Null
    );
    encode(LeafKind::Primitive(0x0100), Cell::UInt(40_000), &mut slot).expect("encodes");
    assert!(native_counter_bound(&slot));
    assert_eq!(
        decode(LeafKind::Primitive(0x0100), &slot).expect("decodes"),
        Cell::UInt(40_000)
    );
}
