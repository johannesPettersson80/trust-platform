//! Fixed-size reset record kept in the memory each chip preserves across a reset
//! (specification 34's boot sequence, design S1b §1.1).
//!
//! The record says why the previous boot ended: the panic path, the hardware watchdog, or a
//! requested restart. It is a fixed number of 32-bit words so a chip can keep it in the C6's
//! persistent RTC fast memory or the F407's RTC backup registers. A magic, a version and a
//! CRC-32/ISO-HDLC over the record's own bytes make an unwritten, torn or damaged record read as
//! `None`, never as a cause.
//!
//! The crate knows no address: a chip supplies its persistent memory through [`ResetStore`], and
//! the host tests drive a fake store with literal words.

/// Words the encoded record occupies; a store keeps at least this many.
pub const RECORD_WORDS: usize = 12;
/// Bytes the encoded record occupies.
pub const RECORD_BYTES: usize = RECORD_WORDS * 4;
/// Longest panic-location text kept; a longer text is truncated to this many bytes.
pub const TEXT_BYTES: usize = 32;

/// Magic of an encoded record: the bytes `TRSR` written little-endian.
const MAGIC: u32 = 0x5452_5352;
/// Format version of this record.
const VERSION: u16 = 1;
/// Offset of the version in the encoded bytes.
const VERSION_OFFSET: usize = 4;
/// Offset of the cause byte in the encoded bytes.
const CAUSE_OFFSET: usize = 6;
/// Offset of the text length byte in the encoded bytes.
const TEXT_LENGTH_OFFSET: usize = 7;
/// Offset of the fixed-size text in the encoded bytes.
const TEXT_OFFSET: usize = 8;
/// Offset of the boot counter in the encoded bytes.
const BOOT_COUNT_OFFSET: usize = TEXT_OFFSET + TEXT_BYTES;
/// Offset of the CRC-32 in the encoded bytes; it covers every earlier byte.
const CRC_OFFSET: usize = BOOT_COUNT_OFFSET + 4;
const _: () = assert!(CRC_OFFSET + 4 == RECORD_BYTES);

/// Why the previous boot ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResetCause {
    /// The firmware's panic path ended the boot.
    Panic,
    /// The hardware watchdog reset the chip without a fed heartbeat.
    Watchdog,
    /// A requested restart, for example a warm or cold restart command.
    Requested,
}

impl ResetCause {
    /// The byte the record stores for this cause.
    const fn code(self) -> u8 {
        match self {
            Self::Panic => 1,
            Self::Watchdog => 2,
            Self::Requested => 3,
        }
    }

    /// The cause a stored byte names, or `None` for an unknown code.
    const fn from_code(code: u8) -> Option<Self> {
        match code {
            1 => Some(Self::Panic),
            2 => Some(Self::Watchdog),
            3 => Some(Self::Requested),
            _ => None,
        }
    }
}

/// One decoded reset record.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResetRecord {
    cause: ResetCause,
    text: [u8; TEXT_BYTES],
    text_length: u8,
    boot_count: u32,
}

impl ResetRecord {
    /// A record for `cause` with no text, stamped with `boot_count`.
    pub const fn new(cause: ResetCause, boot_count: u32) -> Self {
        Self {
            cause,
            text: [0; TEXT_BYTES],
            text_length: 0,
            boot_count,
        }
    }

    /// A record for `cause` with the leading bytes of `text` (at most [`TEXT_BYTES`]) and
    /// `boot_count`.
    pub fn with_text(cause: ResetCause, text: &str, boot_count: u32) -> Self {
        let mut record = Self::new(cause, boot_count);
        let kept = text.len().min(TEXT_BYTES);
        record.text[..kept].copy_from_slice(&text.as_bytes()[..kept]);
        record.text_length = kept as u8;
        record
    }

    /// Why the previous boot ended.
    pub const fn cause(&self) -> ResetCause {
        self.cause
    }

    /// The panic location the record carried, or an empty slice.
    pub fn text(&self) -> &[u8] {
        &self.text[..usize::from(self.text_length)]
    }

    /// The boot counter the record was written with.
    pub const fn boot_count(&self) -> u32 {
        self.boot_count
    }

    /// Encode the record into the words a chip preserves across a reset.
    #[must_use]
    pub fn words(&self) -> [u32; RECORD_WORDS] {
        let mut bytes = [0u8; RECORD_BYTES];
        bytes[..4].copy_from_slice(&MAGIC.to_le_bytes());
        bytes[VERSION_OFFSET..VERSION_OFFSET + 2].copy_from_slice(&VERSION.to_le_bytes());
        bytes[CAUSE_OFFSET] = self.cause.code();
        bytes[TEXT_LENGTH_OFFSET] = self.text_length;
        bytes[TEXT_OFFSET..TEXT_OFFSET + TEXT_BYTES].copy_from_slice(&self.text);
        bytes[BOOT_COUNT_OFFSET..BOOT_COUNT_OFFSET + 4]
            .copy_from_slice(&self.boot_count.to_le_bytes());
        seal(&mut bytes);
        pack(&bytes)
    }

    /// Decode words written by [`ResetRecord::words`]. A wrong magic, version or CRC, an unknown
    /// cause or an overlong text is `None`, so a damaged record never reads as a cause.
    #[must_use]
    pub fn decode(words: [u32; RECORD_WORDS]) -> Option<Self> {
        let bytes = unpack(&words);
        let version = u16::from_le_bytes([bytes[VERSION_OFFSET], bytes[VERSION_OFFSET + 1]]);
        if word(&bytes, 0) != MAGIC
            || version != VERSION
            || word(&bytes, CRC_OFFSET) != crc32fast::hash(&bytes[..CRC_OFFSET])
        {
            return None;
        }
        let cause = ResetCause::from_code(bytes[CAUSE_OFFSET])?;
        let text_length = bytes[TEXT_LENGTH_OFFSET];
        if usize::from(text_length) > TEXT_BYTES {
            return None;
        }
        let mut text = [0u8; TEXT_BYTES];
        text.copy_from_slice(&bytes[TEXT_OFFSET..TEXT_OFFSET + TEXT_BYTES]);
        Some(Self {
            cause,
            text,
            text_length,
            boot_count: word(&bytes, BOOT_COUNT_OFFSET),
        })
    }
}

/// The memory a chip keeps across a reset: the record words and a boot counter.
///
/// The C6 stores both in persistent RTC fast memory and the F407 in RTC backup registers; the host
/// tests drive a fake store with literal words.
pub trait ResetStore {
    /// The stored record words; all zero when no record was written.
    fn record_words(&self) -> [u32; RECORD_WORDS];
    /// Overwrite the stored record words.
    fn set_record_words(&mut self, words: [u32; RECORD_WORDS]);
    /// The stored boot counter.
    fn boot_count(&self) -> u32;
    /// Overwrite the stored boot counter.
    fn set_boot_count(&mut self, count: u32);
}

/// Write `cause` and `text` for the next boot, stamped with the store's boot counter.
pub fn write_record<S: ResetStore>(store: &mut S, cause: ResetCause, text: &str) {
    let count = store.boot_count();
    store.set_record_words(ResetRecord::with_text(cause, text, count).words());
}

/// Read the record the previous boot left and clear it, so a cause is reported exactly once.
pub fn take_record<S: ResetStore>(store: &mut S) -> Option<ResetRecord> {
    let record = ResetRecord::decode(store.record_words())?;
    store.set_record_words([0; RECORD_WORDS]);
    Some(record)
}

/// Count this boot and return the new boot counter; the next record is stamped with it.
pub fn note_boot<S: ResetStore>(store: &mut S) -> u32 {
    let count = store.boot_count().wrapping_add(1);
    store.set_boot_count(count);
    count
}

/// The little-endian word at `offset`, inside the record.
fn word(bytes: &[u8; RECORD_BYTES], offset: usize) -> u32 {
    u32::from_le_bytes([
        bytes[offset],
        bytes[offset + 1],
        bytes[offset + 2],
        bytes[offset + 3],
    ])
}

/// Store the CRC-32 of the covered bytes in the record's own CRC field.
fn seal(bytes: &mut [u8; RECORD_BYTES]) {
    let crc = crc32fast::hash(&bytes[..CRC_OFFSET]);
    bytes[CRC_OFFSET..].copy_from_slice(&crc.to_le_bytes());
}

/// Pack the encoded bytes into the words a chip stores.
fn pack(bytes: &[u8; RECORD_BYTES]) -> [u32; RECORD_WORDS] {
    let mut words = [0u32; RECORD_WORDS];
    for (index, slot) in words.iter_mut().enumerate() {
        *slot = word(bytes, index * 4);
    }
    words
}

/// Unpack stored words into the encoded bytes.
fn unpack(words: &[u32; RECORD_WORDS]) -> [u8; RECORD_BYTES] {
    let mut bytes = [0u8; RECORD_BYTES];
    for (index, word) in words.iter().enumerate() {
        bytes[index * 4..index * 4 + 4].copy_from_slice(&word.to_le_bytes());
    }
    bytes
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A store in ordinary RAM that outlives a test's simulated reset.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    struct FakeStore {
        record: [u32; RECORD_WORDS],
        boot_count: u32,
    }

    impl ResetStore for FakeStore {
        fn record_words(&self) -> [u32; RECORD_WORDS] {
            self.record
        }

        fn set_record_words(&mut self, words: [u32; RECORD_WORDS]) {
            self.record = words;
        }

        fn boot_count(&self) -> u32 {
            self.boot_count
        }

        fn set_boot_count(&mut self, count: u32) {
            self.boot_count = count;
        }
    }

    /// Damage `bytes` and reseal the CRC, so only the damaged field is wrong.
    fn damaged(
        valid: [u32; RECORD_WORDS],
        damage: impl FnOnce(&mut [u8; RECORD_BYTES]),
    ) -> [u32; RECORD_WORDS] {
        let mut bytes = unpack(&valid);
        damage(&mut bytes);
        seal(&mut bytes);
        pack(&bytes)
    }

    #[test]
    fn a_reset_record_survives_a_reset_and_is_read_once() {
        // R2 and design S1b §1.1: a panic or watchdog path writes the cause before the reset, and
        // the next boot reads it once with its cause and text and clears it. The wrong behaviour:
        // a record that reads back as no record, a cause, text or counter that is lost, and a
        // record that can be read twice (the previous cause reported on every boot).
        let mut store = FakeStore {
            record: [0; RECORD_WORDS],
            boot_count: 7,
        };
        write_record(&mut store, ResetCause::Panic, "main.rs:42");

        let record = take_record(&mut store).expect("a written record survives the reset");
        assert_eq!(record.cause(), ResetCause::Panic);
        assert_eq!(record.text(), &b"main.rs:42"[..]);
        assert_eq!(record.boot_count(), 7);

        assert_eq!(take_record(&mut store), None);
    }

    #[test]
    fn a_damaged_reset_record_is_no_record() {
        // R2: a wrong magic, version or CRC reads as no record, never as a cause. Each damage is
        // resealed, so only that field is wrong and the check that owns it is the one that fails.
        let valid = ResetRecord::with_text(ResetCause::Watchdog, "scan", 3).words();
        assert_eq!(
            ResetRecord::decode(valid),
            Some(ResetRecord::with_text(ResetCause::Watchdog, "scan", 3))
        );

        let wrong_magic = damaged(valid, |bytes| bytes[0] ^= 0x01);
        assert_eq!(ResetRecord::decode(wrong_magic), None);

        let wrong_version = damaged(valid, |bytes| bytes[VERSION_OFFSET] ^= 0x01);
        assert_eq!(ResetRecord::decode(wrong_version), None);

        let mut wrong_crc = valid;
        wrong_crc[RECORD_WORDS - 1] ^= 0x01;
        assert_eq!(ResetRecord::decode(wrong_crc), None);
    }

    #[test]
    fn a_record_carries_the_standard_crc_of_its_own_bytes() {
        // R2 and the reset record layout above: the last word is the CRC-32/ISO-HDLC of the 44
        // bytes before it. The literal is the standard CRC-32 of `Watchdog`/`scan`/boot 3 in that
        // layout (the algorithm's own check value for "123456789" is 0xCBF43926), so a changed
        // byte, offset, covered range or algorithm fails here; the encode/decode tests above use
        // the record's own seal, so only this literal pins the wire bytes both chips store.
        let words = ResetRecord::with_text(ResetCause::Watchdog, "scan", 3).words();
        assert_eq!(words[RECORD_WORDS - 1], 0xd817_f8d9);
    }

    #[test]
    fn the_boot_counter_increments_once_per_boot() {
        // R2: the record holds a boot counter. The wrong behaviour: a counter that does not count
        // a boot, and a record stamped with the counter before the boot rather than after it.
        let mut store = FakeStore {
            record: [0; RECORD_WORDS],
            boot_count: 0,
        };
        assert_eq!(note_boot(&mut store), 1);
        write_record(&mut store, ResetCause::Requested, "");
        let record = take_record(&mut store).expect("a written record reads back");
        assert_eq!(record.boot_count(), 1);
        assert_eq!(note_boot(&mut store), 2);
    }
}
