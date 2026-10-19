//! A region's text buffers and the sink that writes into them.
//!
//! Specification: `docs/specs/34-runtime-portability.md` §6.2 (MEM-10), the text-store sentence
//! after the cell-store paragraph, and `docs/IEC_DECISIONS.md` 2026-10-17 (a stored text is cut to
//! its capacity in whole characters, never inside a Unicode scalar value or a surrogate pair);
//! design `~/projects/.artifacts/bounded-c/design.md` "Regions" (one fixed buffer per text leaf)
//! and "Standard library" (a bounded output sink). Slice table row 14 (C1-7 text, test
//! `store_text`).
//!
//! Where the sink stops is proved against `normalize_primitive_value` by the crate's unit test
//! `memory::text::tests`; these integration tests cover the reservation, the buffer that never
//! grows, and the empty, full and repeated round trip.

use trust_runtime_core::memory::cells::TextSlot;
use trust_runtime_core::memory::layout::{LayoutRegion, TextLeaf};
use trust_runtime_core::memory::text::TextStore;

#[test]
fn a_text_buffer_never_grows() {
    // Specification 34 §6.2 and design "Regions": each buffer reserves its leaf's capacity once and
    // never grows, so writing an empty text, a text that fills the buffer, an over-long text and
    // many of them in turn leaves the reservation and every capacity unchanged. A store that
    // reserved lazily and reallocated on the first write, or that reserved a longer text's bytes,
    // makes `reserved` differ from the two leaves' 4 and 16 bytes.
    let leaves = [
        TextLeaf {
            offset: 0,
            capacity_bytes: 4,
            byte_offset: 0,
        },
        TextLeaf {
            offset: 1,
            capacity_bytes: 16,
            byte_offset: 0,
        },
    ];
    let mut store =
        TextStore::reserve_in(LayoutRegion::Persistent, &leaves).expect("the text buffers reserve");
    assert_eq!(store.region(), LayoutRegion::Persistent);
    let reserved = store.reserved();
    assert_eq!(reserved, (2, 20), "two buffers of 4 and 16 bytes");
    assert_eq!(store.capacity(TextSlot(0)), Some(4));
    assert_eq!(store.capacity(TextSlot(1)), Some(16));

    let long = "🙂🙂🙂🙂🙂🙂🙂🙂";
    for text in ["", "x", "abcdef", long, "ÄÖÜ"] {
        let _ = store.write(TextSlot(0), false, text);
        let _ = store.write(TextSlot(1), true, text);
        assert_eq!(store.reserved(), reserved, "text {text:?}");
        assert_eq!(store.capacity(TextSlot(0)), Some(4), "text {text:?}");
        assert_eq!(store.capacity(TextSlot(1)), Some(16), "text {text:?}");
        assert!(store.length(TextSlot(0)).is_some_and(|length| length <= 4));
        assert!(store.length(TextSlot(1)).is_some_and(|length| length <= 16));
    }
}

#[test]
fn an_empty_and_a_full_text_round_trip() {
    // Specification 34 §6.2 and design "Regions"/"Standard library": an empty text uses no bytes,
    // a text that exactly fills its buffer is kept whole, and a shorter text after a full one
    // leaves no stale tail. A sink that reserved a terminator, that dropped the last whole
    // character of a full buffer, or that kept the previous bytes after a shorter write fails here.
    let mut store = TextStore::reserve_in(
        LayoutRegion::Persistent,
        &[
            TextLeaf {
                offset: 0,
                capacity_bytes: 4,
                byte_offset: 0,
            },
            TextLeaf {
                offset: 1,
                capacity_bytes: 8,
                byte_offset: 0,
            },
        ],
    )
    .expect("the text buffers reserve");

    // A STRING[4]: empty, exactly full, then shorter.
    assert_eq!(store.write(TextSlot(0), false, ""), Some(0));
    assert_eq!(store.length(TextSlot(0)), Some(0));
    assert_eq!(store.text(TextSlot(0), false).as_deref(), Some(""));
    assert_eq!(store.write(TextSlot(0), false, "abcd"), Some(4));
    assert_eq!(store.length(TextSlot(0)), Some(4));
    assert_eq!(store.text(TextSlot(0), false).as_deref(), Some("abcd"));
    assert_eq!(store.write(TextSlot(0), false, "xy"), Some(2));
    assert_eq!(store.text(TextSlot(0), false).as_deref(), Some("xy"));

    // A WSTRING[4]: four Basic Multilingual Plane scalars and two surrogate pairs both fill it.
    assert_eq!(store.write(TextSlot(1), true, "ÄÖéα"), Some(8));
    assert_eq!(store.text(TextSlot(1), true).as_deref(), Some("ÄÖéα"));
    assert_eq!(store.write(TextSlot(1), true, "🙂🙂"), Some(8));
    assert_eq!(store.length(TextSlot(1)), Some(8));
    assert_eq!(store.text(TextSlot(1), true).as_deref(), Some("🙂🙂"));
    assert_eq!(store.write(TextSlot(1), true, ""), Some(0));
    assert_eq!(store.text(TextSlot(1), true).as_deref(), Some(""));

    // The sink appends until the buffer is full and then keeps nothing more.
    let mut sink = store.sink(TextSlot(0), false).expect("the leaf has a sink");
    sink.clear();
    assert_eq!(sink.write_str("AB"), 2);
    assert_eq!(sink.write_str("CD"), 2);
    assert_eq!(sink.write_str("EF"), 0);
    assert_eq!(store.length(TextSlot(0)), Some(4));
    assert_eq!(store.text(TextSlot(0), false).as_deref(), Some("ABCD"));
}
