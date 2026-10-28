//! The two-sector record (specification 34 TARGET-02; specification 11 §6.9.6): every expected value
//! is worked out here from the record's layout.

use alloc::boxed::Box;
use alloc::rc::Rc;
use alloc::vec;
use alloc::vec::Vec;
use core::cell::RefCell;

use trust_runtime_core::retain::checkpoint::FlashError;

use super::*;

const MAGIC: [u8; 8] = *b"TRSTPR01";

/// Two sectors of 64 bytes in memory, shared with the test so it can read and damage them.
#[derive(Clone)]
struct Sectors(Rc<RefCell<[Vec<u8>; 2]>>);

impl Sectors {
    fn erased() -> Self {
        Self(Rc::new(RefCell::new([vec![0xFF; 64], vec![0xFF; 64]])))
    }
}

impl SectorFlash for Sectors {
    fn sector_bytes(&self) -> usize {
        64
    }

    fn read(&mut self, sector: usize, bytes: &mut [u8]) -> Result<(), FlashError> {
        let sectors = self.0.borrow();
        bytes.copy_from_slice(&sectors[sector][..bytes.len()]);
        Ok(())
    }

    fn erase(&mut self, sector: usize) -> Result<(), FlashError> {
        self.0.borrow_mut()[sector].fill(0xFF);
        Ok(())
    }

    fn program(&mut self, sector: usize, bytes: &[u8]) -> Result<(), FlashError> {
        self.0.borrow_mut()[sector][..bytes.len()].copy_from_slice(bytes);
        Ok(())
    }
}

/// A record is the magic, the sequence, the text's length, its CRC-32 and the text: `abc` as
/// sequence 2 is 8 + 4 + 4 + 4 + 3 = 23 bytes, with the CRC-32 of `abc`, 0x352441C2 (the check
/// value the transfer tests use). It reads back under its magic and under no other, and a sector
/// one byte short of it, or with a byte of its text changed, holds none. Catches a header written in
/// another order or width, a record of the device settings taken for a pairing record, and a write
/// that power ended taken as a record.
#[test]
fn a_record_is_its_header_and_its_text() {
    let record = encode(&MAGIC, 2, b"abc", 64).expect("it fits");
    let mut expected = MAGIC.to_vec();
    expected.extend_from_slice(&2u32.to_le_bytes());
    expected.extend_from_slice(&3u32.to_le_bytes());
    expected.extend_from_slice(&0x3524_41C2u32.to_le_bytes());
    expected.extend_from_slice(b"abc");
    assert_eq!(record, expected);
    assert_eq!(decode(&MAGIC, &record), Some((2, &b"abc"[..])));
    assert_eq!(decode(b"TRSTDS01", &record), None);
    assert_eq!(decode(&MAGIC, &record[..22]), None);
    let mut changed = record.clone();
    changed[21] ^= 1;
    assert_eq!(decode(&MAGIC, &changed), None);
    // 20 + 45 bytes do not fit 64.
    assert_eq!(encode(&MAGIC, 1, &[0u8; 45], 64), None);
    assert!(encode(&MAGIC, 1, &[0u8; 44], 64).is_some());
}

/// The store writes each record into the sector that does not hold the newest, with the next
/// sequence: the first into sector 0 as 1, then sector 1 as 2, then sector 0 as 3; opening it again
/// finds the newest, 3. A record that power ended in the middle of its write leaves the one before
/// it as the newest. A text longer than a sector less the header, 44 bytes, is refused. Catches a
/// store that writes over its newest record, which a power cut would then lose, and a sequence that
/// does not rise.
#[test]
fn the_store_alternates_its_sectors_and_keeps_the_newest() {
    let flash = Sectors::erased();
    let (mut store, found) = RecordStore::open(Box::new(flash.clone()), MAGIC);
    assert_eq!(found, None);
    assert_eq!(store.capacity(), 44);
    assert_eq!(store.store(b"one"), Ok(1));
    assert_eq!(store.store(b"two"), Ok(2));
    assert_eq!(store.store(b"three"), Ok(3));
    assert_eq!(
        decode(&MAGIC, &flash.0.borrow()[0]),
        Some((3, &b"three"[..]))
    );
    assert_eq!(decode(&MAGIC, &flash.0.borrow()[1]), Some((2, &b"two"[..])));
    let (_, found) = RecordStore::open(Box::new(flash.clone()), MAGIC);
    assert_eq!(found, Some((3, b"three".to_vec())));
    // A write of sequence 4 into sector 1 that power ended after the header: its CRC-32 fails.
    flash.0.borrow_mut()[1][..20]
        .copy_from_slice(&encode(&MAGIC, 4, b"four", 64).expect("fits")[..20]);
    let (_, found) = RecordStore::open(Box::new(flash.clone()), MAGIC);
    assert_eq!(found, Some((3, b"three".to_vec())));
    assert_eq!(store.store(&[0u8; 45]), Err(StoreError::TooLong));
}
