//! Bounded exhaustive check of process-image address extents and typed flat-image access.
//!
//! Specification: `docs/specs/11-runtime-engine.md`, "Process-image bounds, reads, snapshots, and
//! metadata application" and "Process-image interface and binding transaction" (each byte area holds
//! at most 16 MiB; a window beyond the cap, a zero-length window, a wildcard, a wrong value kind or
//! an overflowing address is refused before allocation or mutation; reads of unallocated bytes return
//! zero without growth; a byte window reads to its first NUL and rejects invalid UTF-8 before it;
//! a string write is refused when its byte length exceeds the window); and the control-surface rule
//! that flat addresses must fit the 16,777,216-byte area, including the full width of each access.
//!
//! Production under test: `IoAddress::flat_byte_end`
//! (`crates/trust-runtime-core/src/io_address.rs:45`), `validate_process_image_address`
//! (`crates/trust-runtime-core/src/io_image/mod.rs:198`), `read_flat_image` (`:20`),
//! `write_flat_image` (`:76`) and `ensure_len` (`:187`).
//!
//! Model. A flat address occupies the window `[byte, byte + width)` of one byte area. The width is 1
//! for BIT and BYTE, 2 for WORD, 4 for DWORD, 8 for LWORD, and the declared length for a byte window
//! (zero-length windows are refused). The oracle accepts an address exactly when it is concrete, a
//! BIT index is below 8, the window is non-empty, and `byte + width <= 16,777,216`. A hierarchical
//! address (two or more path components) has no flat window: validation checks its bit index only,
//! and flat access refuses it.
//!
//! Bounds. Areas: input, output and memory. Offsets: 0..=3, the eleven offsets from 16,777,207 to
//! 16,777,217 around the cap, and the three largest u32 values. Sizes: BIT with bits 0, 3, 7 and 8;
//! BYTE, WORD, DWORD and LWORD; byte windows of 0, 1, 2, 3, 4, 8, 4,096 and u32::MAX bytes. Every
//! address is checked both as flat and as hierarchical: 1,728 addresses. The round trips use a 16 MiB
//! image, so each accepted window touches the cap boundary.
//!
//! Assumptions. A 64-bit host: `byte + window` is computed in `usize` by the production code and in
//! `u64` by the oracle, which agree there.
//!
//! Invariants checked on every address: validation accepts exactly the oracle's windows; a flat end
//! is `byte + width` and lies inside the cap exactly when validation accepts; a successful write
//! reads back the same value, changes no byte outside its window, and grows an empty image to exactly
//! the window's end.

use trust_runtime_core::error::RuntimeError;
use trust_runtime_core::io_address::{IoAddress, IoSize, PROCESS_IMAGE_AREA_LIMIT};
use trust_runtime_core::io_image::{
    ensure_len, read_flat_image, validate_process_image_address, write_flat_image,
};
use trust_runtime_core::memory::IoArea;
use trust_runtime_core::value::Value;

const _: () = assert!(PROCESS_IMAGE_AREA_LIMIT == 16 * 1024 * 1024);
const CAP: u64 = 16 * 1024 * 1024;
const CAP_U32: u32 = 16 * 1024 * 1024;
const SENTINEL: u8 = 0x5A;
const PAYLOAD: &str = "ABC";

/// Width of the flat window of `size`, or `None` for a zero-length byte window.
fn window(size: IoSize) -> Option<u64> {
    match size {
        IoSize::Bit | IoSize::Byte => Some(1),
        IoSize::Word => Some(2),
        IoSize::DWord => Some(4),
        IoSize::LWord => Some(8),
        IoSize::Bytes(0) => None,
        IoSize::Bytes(len) => Some(u64::from(len)),
    }
}

/// The oracle: which addresses the specification lets validation and access accept.
fn oracle_accepts(address: &IoAddress) -> bool {
    if address.wildcard {
        return false;
    }
    if matches!(address.size, IoSize::Bit) && address.bit >= 8 {
        return false;
    }
    if address.path.len() > 1 {
        return true;
    }
    match window(address.size) {
        Some(width) => u64::from(address.byte) + width <= CAP,
        None => false,
    }
}

fn address(area: IoArea, size: IoSize, byte: u32, bit: u8, hierarchical: bool) -> IoAddress {
    let path = if hierarchical {
        vec![byte, 1]
    } else {
        vec![byte]
    };
    IoAddress {
        area,
        size,
        byte,
        bit,
        path,
        wildcard: false,
    }
}

fn offsets() -> Vec<u32> {
    (0..4)
        .chain(CAP_U32 - 9..=CAP_U32 + 1)
        .chain(u32::MAX - 2..=u32::MAX)
        .collect()
}

fn sizes() -> Vec<IoSize> {
    let mut sizes = vec![IoSize::Byte, IoSize::Word, IoSize::DWord, IoSize::LWord];
    sizes.extend([0, 1, 2, 3, 4, 8, 4096, u32::MAX].map(IoSize::Bytes));
    sizes
}

/// Flat access kinds, each with the BIT index it uses.
fn flat_kinds() -> Vec<(IoSize, u8)> {
    let mut kinds: Vec<(IoSize, u8)> = [0_u8, 3, 7, 8]
        .into_iter()
        .map(|bit| (IoSize::Bit, bit))
        .collect();
    kinds.extend(sizes().into_iter().map(|size| (size, 0)));
    kinds
}

/// The bounded address domain: flat and hierarchical forms of every offset, size and area.
fn domain() -> Vec<IoAddress> {
    let mut out = Vec::new();
    for area in [IoArea::Input, IoArea::Output, IoArea::Memory] {
        for byte in offsets() {
            for (size, bit) in flat_kinds() {
                for hierarchical in [false, true] {
                    out.push(address(area, size, byte, bit, hierarchical));
                }
            }
        }
    }
    out
}

/// A value of the kind that `size` stores, chosen to differ in every byte.
fn value_for(size: IoSize) -> Value {
    match size {
        IoSize::Bit => Value::Bool(true),
        IoSize::Byte => Value::Byte(0xA5),
        IoSize::Word => Value::Word(0xBEEF),
        IoSize::DWord => Value::DWord(0xDEAD_BEEF),
        IoSize::LWord => Value::LWord(0x0123_4567_89AB_CDEF),
        IoSize::Bytes(len) => {
            let take = usize::try_from(len.min(3)).expect("payload length fits usize");
            Value::String(PAYLOAD[..take].into())
        }
    }
}

const _: () = assert!(
    PAYLOAD.len() == 3,
    "value_for relies on a three-byte payload"
);

#[test]
fn validation_accepts_exactly_the_windows_inside_the_cap_and_the_flat_end_agrees() {
    let addresses = domain();
    for address in &addresses {
        let accepted = validate_process_image_address(address).is_ok();
        assert_eq!(accepted, oracle_accepts(address), "{address:?}");
        match address.flat_byte_end() {
            Ok(None) => assert!(
                address.path.len() > 1,
                "{address:?} has no flat window, so it has no flat end"
            ),
            Ok(Some(end)) => {
                assert_eq!(address.path.len(), 1, "{address:?}");
                let width = window(address.size).expect("a flat end needs a non-empty window");
                assert_eq!(
                    u64::from(end),
                    u64::from(address.byte) + width,
                    "{address:?}"
                );
                let bit_ok = !matches!(address.size, IoSize::Bit) || address.bit < 8;
                assert_eq!(
                    accepted,
                    u64::from(end) <= CAP && bit_ok,
                    "{address:?} is accepted exactly when its window is inside the cap"
                );
            }
            Err(_) => {
                assert_eq!(address.path.len(), 1, "{address:?}");
                assert!(!accepted, "{address:?}");
            }
        }
    }
    assert_eq!(addresses.len(), 3 * 18 * 2 * (4 + 12));
}

#[test]
fn wildcard_addresses_are_refused_by_validation_extent_and_access() {
    let wildcard = IoAddress {
        area: IoArea::Memory,
        size: IoSize::Byte,
        byte: 0,
        bit: 0,
        path: vec![0],
        wildcard: true,
    };
    assert!(validate_process_image_address(&wildcard).is_err());
    assert!(wildcard.flat_byte_end().is_err());
    let mut image = Vec::new();
    assert!(write_flat_image(&mut image, &wildcard, Value::Byte(1)).is_err());
    assert!(image.is_empty());
    assert!(read_flat_image(&image, &wildcard).is_err());
}

#[test]
fn every_accepted_window_round_trips_and_writes_no_neighbouring_byte() {
    // One 16 MiB image serves every case, so windows at the cap boundary are real.
    let mut image = vec![0_u8; PROCESS_IMAGE_AREA_LIMIT];
    let mut windows = 0_u32;
    for address in domain() {
        if address.area != IoArea::Memory || address.path.len() != 1 || !oracle_accepts(&address) {
            continue;
        }
        let start = usize::try_from(address.byte).expect("u32 fits usize");
        let width = usize::try_from(window(address.size).expect("accepted windows are non-empty"))
            .expect("window fits usize");
        let end = start + width;
        let value = value_for(address.size);
        let before = start.checked_sub(1);
        if let Some(index) = before {
            image[index] = SENTINEL;
        }
        if end < image.len() {
            image[end] = SENTINEL;
        }

        write_flat_image(&mut image, &address, value.clone())
            .unwrap_or_else(|error| panic!("{address:?} is accepted but refused: {error:?}"));
        assert_eq!(
            read_flat_image(&image, &address).expect("accepted windows read"),
            value,
            "{address:?} reads back what it wrote"
        );
        if let Some(index) = before {
            assert_eq!(
                image[index], SENTINEL,
                "byte before {address:?} was written"
            );
        }
        if end < image.len() {
            assert_eq!(image[end], SENTINEL, "byte after {address:?} was written");
        }
        windows += 1;
    }
    // The memory area alone holds at least these windows: at offsets 0..=3, every flat kind except
    // BIT index 8, the zero-length window and the 4 GiB window (3 BIT kinds + 10 sized kinds).
    assert!(
        windows >= 4 * 13,
        "only {windows} memory windows were checked"
    );
}

#[test]
fn bit_writes_change_only_the_addressed_bit() {
    let mut image = vec![0_u8; PROCESS_IMAGE_AREA_LIMIT];
    for byte in [0_u32, 5, CAP_U32 - 1] {
        let index = usize::try_from(byte).expect("u32 fits usize");
        for bit in 0_u8..8 {
            let bit_address = address(IoArea::Memory, IoSize::Bit, byte, bit, false);
            let mask = 1_u8 << bit;
            image[index] = 0xFF;
            write_flat_image(&mut image, &bit_address, Value::Bool(false)).expect("accepted bit");
            assert_eq!(image[index], !mask, "clearing bit {bit} of byte {byte}");
            assert_eq!(
                read_flat_image(&image, &bit_address).expect("accepted bit"),
                Value::Bool(false)
            );
            write_flat_image(&mut image, &bit_address, Value::Bool(true)).expect("accepted bit");
            assert_eq!(image[index], 0xFF, "setting bit {bit} of byte {byte}");
            assert_eq!(
                read_flat_image(&image, &bit_address).expect("accepted bit"),
                Value::Bool(true)
            );
        }
    }
}

#[test]
fn an_empty_image_grows_exactly_to_each_accepted_window() {
    for byte in 0_u32..=3 {
        for (size, bit) in flat_kinds() {
            let kind_address = address(IoArea::Memory, size, byte, bit, false);
            if !oracle_accepts(&kind_address) {
                continue;
            }
            let mut image = Vec::new();
            write_flat_image(&mut image, &kind_address, value_for(size))
                .expect("small windows fit");
            let end = kind_address
                .flat_byte_end()
                .expect("flat extent")
                .expect("flat window");
            assert_eq!(
                image.len(),
                usize::try_from(end).expect("u32 fits usize"),
                "{kind_address:?}"
            );
            assert_eq!(
                read_flat_image(&image, &kind_address).expect("accepted"),
                value_for(size),
                "{kind_address:?}"
            );
        }
    }
}

#[test]
fn refused_writes_change_no_byte_and_grow_nothing() {
    // A value of the wrong kind, a string longer than its window, and a window beyond the cap.
    let mut image = vec![0xEE_u8; 8];
    assert!(write_flat_image(
        &mut image,
        &address(IoArea::Memory, IoSize::Word, 0, 0, false),
        Value::Byte(1)
    )
    .is_err());
    assert!(write_flat_image(
        &mut image,
        &address(IoArea::Memory, IoSize::Bytes(2), 0, 0, false),
        Value::String(PAYLOAD.into())
    )
    .is_err());
    assert_eq!(image, vec![0xEE_u8; 8]);

    let mut empty = Vec::new();
    let beyond_cap = address(IoArea::Memory, IoSize::Word, CAP_U32 - 1, 0, false);
    assert!(write_flat_image(&mut empty, &beyond_cap, Value::Word(1)).is_err());
    assert!(empty.is_empty());
}

#[test]
fn reads_of_unallocated_windows_are_zero_or_empty_and_the_cap_still_applies() {
    let empty: Vec<u8> = Vec::new();
    let read = |size, byte| read_flat_image(&empty, &address(IoArea::Memory, size, byte, 3, false));
    assert_eq!(read(IoSize::Bit, 5).expect("bit"), Value::Bool(false));
    assert_eq!(read(IoSize::Byte, 5).expect("byte"), Value::Byte(0));
    assert_eq!(read(IoSize::Word, 5).expect("word"), Value::Word(0));
    assert_eq!(read(IoSize::DWord, 5).expect("dword"), Value::DWord(0));
    assert_eq!(read(IoSize::LWord, 5).expect("lword"), Value::LWord(0));
    assert_eq!(
        read(IoSize::Bytes(4), 5).expect("bytes"),
        Value::String("".into())
    );
    assert!(read(IoSize::Word, CAP_U32 - 1).is_err());
}

#[test]
fn byte_windows_stop_at_the_first_nul_and_count_bytes_not_characters() {
    let window_text = [0x41_u8, 0x00, 0xFF];
    assert_eq!(
        read_flat_image(
            &window_text,
            &address(IoArea::Memory, IoSize::Bytes(3), 0, 0, false)
        )
        .expect("text before the NUL"),
        Value::String("A".into())
    );

    let invalid = [0xFF_u8, 0x00];
    assert!(matches!(
        read_flat_image(
            &invalid,
            &address(IoArea::Memory, IoSize::Bytes(2), 0, 0, false)
        ),
        Err(RuntimeError::TypeMismatch)
    ));

    let umlaut = [0xC3_u8, 0x84, 0x00];
    assert_eq!(
        read_flat_image(
            &umlaut,
            &address(IoArea::Memory, IoSize::Bytes(3), 0, 0, false)
        )
        .expect("two-byte character"),
        Value::String("\u{C4}".into())
    );

    let mut image = Vec::new();
    assert!(matches!(
        write_flat_image(
            &mut image,
            &address(IoArea::Memory, IoSize::Bytes(1), 0, 0, false),
            Value::String("\u{C4}".into())
        ),
        Err(RuntimeError::Overflow)
    ));
    assert!(image.is_empty());
    write_flat_image(
        &mut image,
        &address(IoArea::Memory, IoSize::Bytes(2), 0, 0, false),
        Value::String("\u{C4}".into()),
    )
    .expect("two bytes hold the character");
    assert_eq!(image, vec![0xC3, 0x84]);
}

#[test]
fn growth_stops_at_the_cap() {
    let mut image = Vec::new();
    assert!(ensure_len(&mut image, PROCESS_IMAGE_AREA_LIMIT).is_err());
    assert!(image.is_empty());
    ensure_len(&mut image, PROCESS_IMAGE_AREA_LIMIT - 1).expect("the last byte is inside the cap");
    assert_eq!(image.len(), PROCESS_IMAGE_AREA_LIMIT);
}

#[test]
fn hierarchical_addresses_have_no_flat_window_and_refuse_flat_access() {
    let hierarchical = address(IoArea::Memory, IoSize::Word, 0, 0, true);
    assert!(matches!(hierarchical.flat_byte_end(), Ok(None)));
    let mut image = Vec::new();
    assert!(write_flat_image(&mut image, &hierarchical, Value::Word(1)).is_err());
    assert!(image.is_empty());
    assert!(read_flat_image(&image, &hierarchical).is_err());
}
