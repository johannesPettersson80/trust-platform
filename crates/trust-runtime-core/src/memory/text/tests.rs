//! The cut a text buffer makes, proved against the normalization the engine already performs.
//!
//! Specification: `docs/specs/34-runtime-portability.md` §6.2 (MEM-10) — the text store's sink cuts
//! a written text exactly where assignment normalization cuts it — and `docs/IEC_DECISIONS.md`
//! 2026-10-17: a `STRING[n]` holds at most `n` bytes of UTF-8 and a `WSTRING[n]` at most `n` UTF-16
//! code units, and a cut never splits a character.
//!
//! This test lives inside the crate because the oracle is
//! [`normalize_primitive_value`](crate::vm::type_policy::normalize_primitive_value) itself: the
//! store's cut is right only while it stops exactly where that normalization stops. A store that
//! cut at a byte count inside a multi-byte scalar, that measured a `WSTRING` in bytes, that split a
//! surrogate pair or that kept one character too many would stop matching the normalized value.

use super::*;
use crate::memory::cells::TextSlot;
use crate::memory::layout::{LayoutRegion, TextLeaf};
use crate::value::Value;
use crate::vm::type_policy::normalize_primitive_value;
use alloc::string::String;
use alloc::vec::Vec;

/// The primitive encoding of `STRING` (`vm::type_policy`); `WSTRING` is 25.
const STRING_PRIMITIVE: u16 = 24;

/// The primitive encoding of `WSTRING`.
const WSTRING_PRIMITIVE: u16 = 25;

/// The scalars a generated text is built from: ASCII, two-, three- and four-byte scalars, and the
/// last two are surrogate pairs in UTF-16, so a cut must never split one.
const SCALARS: [char; 12] = [
    'a', 'Z', '7', ' ', 'Ä', 'Ö', 'é', 'α', '€', '中', '🙂', '😀',
];

/// A one-leaf store of `capacity_bytes` bytes at cell 0, reserved once.
fn single_leaf(capacity_bytes: u32) -> TextStore {
    TextStore::reserve_in(
        LayoutRegion::Persistent,
        &[TextLeaf {
            offset: 0,
            capacity_bytes,
            byte_offset: 0,
        }],
    )
    .expect("the one text buffer reserves")
}

#[test]
fn text_is_cut_exactly_where_normalization_cuts_it() {
    // Specification 34 §6.2 and `docs/IEC_DECISIONS.md` 2026-10-17: a written text is cut to its
    // leaf's capacity exactly where `normalize_primitive_value` cuts it. Over 2,000 seeded texts of
    // each buffer — lengths zero, one, one short of the capacity, the capacity and three past it —
    // the stored text, its bytes and the sink's answer must equal the normalized value's, byte for
    // byte. A store whose cut drifted from normalization, that cut `WSTRING` at a byte count, that
    // cut `STRING` at a code-unit count, that split '🙂' or that kept one character too many fails
    // here.
    let mut rng = Rng::new(0x00C1_0700_5EED_F00D);
    for (wide, capacity) in [(false, 8usize), (false, 4), (true, 8), (true, 4)] {
        let prim_id = if wide {
            WSTRING_PRIMITIVE
        } else {
            STRING_PRIMITIVE
        };
        let max_length = u16::try_from(capacity).expect("a capacity fits a u16");
        let capacity_bytes = if wide { capacity * 2 } else { capacity };
        let lengths = [0usize, 1, capacity - 1, capacity, capacity + 3];
        for _ in 0..2_000 {
            let pick = usize::try_from(rng.next() % u64::try_from(lengths.len()).expect("a count"))
                .expect("a length index");
            let text = generate_text(&mut rng, lengths[pick]);
            let value = if wide {
                Value::WString(text.clone())
            } else {
                Value::String(text.as_str().into())
            };
            let normalized = normalize_primitive_value(prim_id, max_length, value)
                .expect("normalization keeps the text");
            let expected_text: String = match normalized {
                Value::String(text) => text.as_str().into(),
                Value::WString(text) => text,
                other => panic!("normalization produced {other:?}, not a text"),
            };
            let expected_bytes: Vec<u8> = if wide {
                expected_text
                    .encode_utf16()
                    .flat_map(u16::to_le_bytes)
                    .collect()
            } else {
                expected_text.as_bytes().to_vec()
            };

            let mut store = single_leaf(u32::try_from(capacity_bytes).expect("a capacity"));
            let written = store
                .write(TextSlot(0), wide, &text)
                .expect("the leaf writes");
            assert_eq!(
                store.text(TextSlot(0), wide).as_deref(),
                Some(expected_text.as_str()),
                "wide {wide}, capacity {capacity}, text {text:?}"
            );
            assert_eq!(
                store.bytes(TextSlot(0)),
                Some(expected_bytes.as_slice()),
                "wide {wide}, capacity {capacity}, text {text:?}"
            );
            assert_eq!(
                store.length(TextSlot(0)),
                Some(expected_bytes.len()),
                "wide {wide}, capacity {capacity}, text {text:?}"
            );
            assert_eq!(
                written,
                expected_bytes.len(),
                "wide {wide}, capacity {capacity}, text {text:?}"
            );
            assert!(
                expected_bytes.len() <= capacity_bytes,
                "wide {wide}, capacity {capacity}, text {text:?}"
            );
        }
    }
}

/// One generated text of `length` scalars, each drawn from [`SCALARS`].
fn generate_text(rng: &mut Rng, length: usize) -> String {
    let pool = u64::try_from(SCALARS.len()).expect("a pool size");
    let mut text = String::with_capacity(length);
    for _ in 0..length {
        let index = usize::try_from(rng.next() % pool).expect("a scalar index");
        text.push(SCALARS[index]);
    }
    text
}

/// A `SplitMix64` generator, written here so the test needs no new dependency.
struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Self {
        Self(seed)
    }

    /// The next 64 generated bits.
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mixed = (self.0 ^ (self.0 >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        let mixed = (mixed ^ (mixed >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        mixed ^ (mixed >> 31)
    }
}
