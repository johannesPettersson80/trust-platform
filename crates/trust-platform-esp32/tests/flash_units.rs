//! The C6 flash transfers of any length (specification 34 §5.1 and §10.1): `esp-storage` reads and
//! programs whole 4-byte words at word-aligned addresses only, so the adapter rounds the last piece
//! of a transfer to its word. A host fake that refuses anything else stands in for `esp-storage`.

use trust_platform_esp32::checkpoint::{program_whole_units, read_whole_units};
use trust_runtime_core::retain::checkpoint::FlashError;

/// A word flash that, like `esp-storage` without `bytewise-read`, takes only whole 4-byte words at
/// word-aligned addresses, and programs as NOR flash does: a bit can only go from 1 to 0.
struct WordFlash {
    bytes: Vec<u8>,
    calls: Vec<(u32, usize)>,
}

impl WordFlash {
    fn erased(len: usize) -> Self {
        Self {
            bytes: vec![0xFF; len],
            calls: Vec::new(),
        }
    }

    fn read(&mut self, at: u32, buffer: &mut [u8]) -> Result<(), FlashError> {
        assert!(
            at.is_multiple_of(4) && buffer.len().is_multiple_of(4),
            "esp-storage refuses {at}+{}",
            buffer.len()
        );
        self.calls.push((at, buffer.len()));
        let at = at as usize;
        buffer.copy_from_slice(&self.bytes[at..at + buffer.len()]);
        Ok(())
    }

    fn program(&mut self, at: u32, buffer: &[u8]) -> Result<(), FlashError> {
        assert!(
            at.is_multiple_of(4) && buffer.len().is_multiple_of(4),
            "esp-storage refuses {at}+{}",
            buffer.len()
        );
        self.calls.push((at, buffer.len()));
        let at = at as usize;
        for (cell, byte) in self.bytes[at..at + buffer.len()].iter_mut().zip(buffer) {
            *cell &= *byte;
        }
        Ok(())
    }
}

#[test]
fn a_read_of_any_length_takes_whole_words_and_keeps_its_own_bytes() {
    // Specification 34 §5.1: the boot reads an installed container of its own length, here 3,871
    // bytes, the bench's math program (18 October 2026), whose last 31 bytes end inside a word. The
    // reads are 15 pages of 256 bytes and one of 32 (31 rounded up), and the bytes kept are exactly
    // the container's. Catches a last piece passed to esp-storage unrounded (the read failed), and a
    // rounded piece that writes past the caller's buffer.
    let mut flash = WordFlash::erased(8192);
    for (index, byte) in flash.bytes.iter_mut().enumerate() {
        *byte = (index % 251) as u8;
    }
    let mut page = [0u8; 256];
    let mut container = vec![0u8; 3871];
    read_whole_units(0, &mut container, &mut page, |at, buffer| {
        flash.read(at, buffer)
    })
    .expect("a read of any length");
    let expected: Vec<u8> = (0..3871).map(|index| (index % 251) as u8).collect();
    assert_eq!(container, expected);
    assert_eq!(flash.calls.len(), 16);
    assert_eq!(flash.calls.last(), Some(&(3840, 32)));
}

#[test]
fn a_program_of_any_length_pads_its_last_word_with_erased_bytes() {
    // Specification 34 §5.1 and §10.1: a program of 6 bytes at 0 programs two whole words; the two
    // padding bytes are 0xFF, which leave their NOR bits as they are, so byte 6 and 7 stay erased and
    // a later program can still write them. Catches padding with zeros, which would clear bytes the
    // caller never wrote.
    let mut flash = WordFlash::erased(16);
    let mut page = [0u8; 256];
    program_whole_units(0, &[1, 2, 3, 4, 5, 6], &mut page, |at, buffer| {
        flash.program(at, buffer)
    })
    .expect("a program of any length");
    assert_eq!(&flash.bytes[..8], &[1, 2, 3, 4, 5, 6, 0xFF, 0xFF]);
    assert_eq!(flash.calls, vec![(0, 8)]);
    program_whole_units(4, &[0xA5, 0xA5, 0x5A, 0x5A], &mut page, |at, buffer| {
        flash.program(at, buffer)
    })
    .expect("the padded bytes are still programmable");
    assert_eq!(&flash.bytes[4..8], &[0xA5 & 5, 0xA5 & 6, 0x5A, 0x5A]);
}

#[test]
fn an_unaligned_address_or_page_is_refused_before_the_flash_is_called() {
    // The helpers round lengths only: an address inside a word, or a page that is not whole words,
    // is refused with the transfer's own error and the flash is not called. Catches an unaligned
    // address passed through, which esp-storage refuses with a different error.
    let mut flash = WordFlash::erased(16);
    let mut page = [0u8; 256];
    let mut bytes = [0u8; 4];
    assert_eq!(
        read_whole_units(2, &mut bytes, &mut page, |at, buffer| flash
            .read(at, buffer)),
        Err(FlashError::Read)
    );
    assert_eq!(
        program_whole_units(1, &[0], &mut page, |at, buffer| flash.program(at, buffer)),
        Err(FlashError::Program)
    );
    let mut odd_page = [0u8; 6];
    assert_eq!(
        read_whole_units(0, &mut bytes, &mut odd_page, |at, buffer| flash
            .read(at, buffer)),
        Err(FlashError::Read)
    );
    assert!(flash.calls.is_empty());
}
