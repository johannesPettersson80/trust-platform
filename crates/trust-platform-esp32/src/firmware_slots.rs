//! The C6's two firmware slots and the boot loader's choice between them (specification 34
//! TARGET-04): truST is updated over the control connection by writing the new firmware into the
//! slot that does not run, checking it, and selecting it for the next boot on trial.
//!
//! The boot loader is ESP-IDF v5.5.1's, built with application rollback
//! (`firmware/trust-esp32c6/bootloader`). It keeps its choice in `otadata`, two 4 KiB sectors with one
//! 32-byte entry each: a sequence number, a state and the CRC of the sequence. It boots the slot that
//! the valid entry with the highest sequence names, `(sequence - 1) % 2`, and slot A when no entry is
//! valid. An entry is valid when its sequence is not erased, its CRC is right and its state is neither
//! `Invalid` nor `Aborted` (`bootloader_common_ota_select_valid`). It turns a `New` entry it boots into
//! `PendingVerify`, and at the next boot it turns a `PendingVerify` entry into `Aborted`, so a firmware
//! that does not confirm itself before a reset is replaced by the one before it. The rules here are
//! the boot loader's own (`bootloader_utility.c`, `esp_ota_ops.c`), so that the firmware reads and
//! writes the selection as the boot loader and ESP-IDF do; nothing here touches the flash.
//!
//! A firmware upload is an ESP-IDF application image, as `espflash save-image` writes it: a 24-byte
//! header, the segments, the checksum and the appended SHA-256, with the application descriptor of
//! `esp_app_desc!` at offset 32 ([`check_image`]).

use alloc::string::String;

use sha2::{Digest, Sha256};
use trust_runtime_core::retain::checkpoint::Slot;

/// The flash offset of firmware slot A, the partition `ota_0` (`firmware_a` of `chips/esp32c6.toml`).
pub const FIRMWARE_A_OFFSET: u32 = 0x1_0000;
/// The flash offset of firmware slot B, the partition `ota_1` (`firmware_b`).
pub const FIRMWARE_B_OFFSET: u32 = 0x20_0000;
/// The bytes of each firmware slot: 1,984 KiB.
pub const FIRMWARE_SLOT_BYTES: u32 = 1_984 * 1024;
/// The flash offset of the boot loader's selection, the partition `otadata` (`firmware_select`): two
/// erase units, the first entry at the start of each.
pub const SELECT_OFFSET: u32 = 0xB000;
/// The bytes of one selection entry.
pub const ENTRY_BYTES: usize = 32;
/// The project name the firmware's application descriptor carries: an image of another project is
/// refused, so no other application replaces truST by accident.
pub const PROJECT: &str = "trust-esp32c6";
/// The C6's chip id in an image header (`ESP_CHIP_ID_ESP32C6`).
pub const IMAGE_CHIP_ID: u16 = 13;

/// The flash offset of firmware slot `slot`.
#[must_use]
pub const fn slot_offset(slot: Slot) -> u32 {
    match slot {
        Slot::A => FIRMWARE_A_OFFSET,
        Slot::B => FIRMWARE_B_OFFSET,
    }
}

/// The firmware slot that holds the flash byte at `address`, or `None` outside both slots.
#[must_use]
pub fn slot_at(address: u32) -> Option<Slot> {
    [Slot::A, Slot::B].into_iter().find(|slot| {
        let start = slot_offset(*slot);
        address >= start && address - start < FIRMWARE_SLOT_BYTES
    })
}

/// The flash offset of selection entry `index` (0 or 1).
#[must_use]
pub const fn entry_offset(index: usize) -> u32 {
    if index == 0 {
        SELECT_OFFSET
    } else {
        SELECT_OFFSET + 0x1000
    }
}

/// The state of a firmware the boot loader selected (`esp_ota_img_states_t`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageState {
    /// Selected and not booted yet.
    New,
    /// Booted once and not confirmed yet.
    PendingVerify,
    /// Confirmed by the firmware.
    Valid,
    /// Refused by the firmware.
    Invalid,
    /// Not confirmed before a reset: the boot loader no longer boots it.
    Aborted,
    /// No state: the boot loader makes no assumption about the firmware.
    Undefined,
}

impl ImageState {
    /// The state's code in an entry.
    #[must_use]
    pub const fn code(self) -> u32 {
        match self {
            Self::New => 0,
            Self::PendingVerify => 1,
            Self::Valid => 2,
            Self::Invalid => 3,
            Self::Aborted => 4,
            Self::Undefined => u32::MAX,
        }
    }

    /// The state of `code`. A code the boot loader does not know is a state it makes no assumption
    /// about, as it treats it.
    #[must_use]
    pub const fn from_code(code: u32) -> Self {
        match code {
            0 => Self::New,
            1 => Self::PendingVerify,
            2 => Self::Valid,
            3 => Self::Invalid,
            4 => Self::Aborted,
            _ => Self::Undefined,
        }
    }
}

/// One selection entry that is not erased and whose CRC is right.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SelectEntry {
    /// The sequence: the entry names slot `(sequence - 1) % 2`.
    pub sequence: u32,
    /// The state of the firmware it selects.
    pub state: ImageState,
}

impl SelectEntry {
    /// The slot the entry names.
    #[must_use]
    pub const fn slot(&self) -> Slot {
        if self.sequence.wrapping_sub(1).is_multiple_of(2) {
            Slot::A
        } else {
            Slot::B
        }
    }

    /// Whether the boot loader boots from the entry: its state is neither `Invalid` nor `Aborted`.
    #[must_use]
    pub const fn bootable(&self) -> bool {
        !matches!(self.state, ImageState::Invalid | ImageState::Aborted)
    }
}

/// The CRC the boot loader keeps of a sequence (`bootloader_common_ota_select_crc`): the CRC-32 of
/// its four little-endian bytes, reflected with the polynomial 0x04C11DB7, from a register of 0 and
/// inverted at the end, which is the ROM's `esp_rom_crc32_le(UINT32_MAX, ..)`.
#[must_use]
pub fn sequence_crc(sequence: u32) -> u32 {
    let mut crc: u32 = 0;
    for byte in sequence.to_le_bytes() {
        crc ^= u32::from(byte);
        for _ in 0..8 {
            crc = if crc & 1 == 1 {
                (crc >> 1) ^ 0xEDB8_8320
            } else {
                crc >> 1
            };
        }
    }
    !crc
}

/// Reads an entry: `None` when its sequence is erased or its CRC is wrong, as the boot loader reads
/// such an entry.
#[must_use]
pub fn decode_entry(bytes: &[u8; ENTRY_BYTES]) -> Option<SelectEntry> {
    let word =
        |at: usize| u32::from_le_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]]);
    let sequence = word(0);
    if sequence == u32::MAX || word(28) != sequence_crc(sequence) {
        return None;
    }
    Some(SelectEntry {
        sequence,
        state: ImageState::from_code(word(24)),
    })
}

/// The 32 bytes of `entry`: the sequence, a label of erased bytes, the state and the CRC, as ESP-IDF
/// writes an entry into an erased sector.
#[must_use]
pub fn encode_entry(entry: SelectEntry) -> [u8; ENTRY_BYTES] {
    let mut bytes = [0xFF; ENTRY_BYTES];
    bytes[0..4].copy_from_slice(&entry.sequence.to_le_bytes());
    bytes[24..28].copy_from_slice(&entry.state.code().to_le_bytes());
    bytes[28..32].copy_from_slice(&sequence_crc(entry.sequence).to_le_bytes());
    bytes
}

/// The entry the boot loader boots from, by its index: the bootable entry with the highest sequence
/// (the first on a tie, as `bootloader_common_select_otadata` takes it), `None` when neither is.
#[must_use]
pub fn active_entry(entries: &[Option<SelectEntry>; 2]) -> Option<usize> {
    match (
        entries[0].filter(SelectEntry::bootable),
        entries[1].filter(SelectEntry::bootable),
    ) {
        (Some(first), Some(second)) => Some(usize::from(second.sequence > first.sequence)),
        (Some(_), None) => Some(0),
        (None, Some(_)) => Some(1),
        (None, None) => None,
    }
}

/// The slot the boot loader boots: the one the active entry names, and slot A, `ota_0`, when no entry
/// is bootable.
#[must_use]
pub fn selected_slot(entries: &[Option<SelectEntry>; 2]) -> Slot {
    active_entry(entries)
        .and_then(|index| entries[index])
        .map_or(Slot::A, |entry| entry.slot())
}

/// The entry that selects `target` for the next boot on trial, and its index, as ESP-IDF's
/// `esp_ota_set_boot_partition` writes it with rollback: the other entry than the active one (entry 0
/// when none is active), in the state `New`, with the smallest sequence from the active entry's on
/// that names `target`.
#[must_use]
pub fn activation(entries: &[Option<SelectEntry>; 2], target: Slot) -> (usize, SelectEntry) {
    let target_number: u32 = match target {
        Slot::A => 0,
        Slot::B => 1,
    };
    let Some(active) = active_entry(entries) else {
        return (
            0,
            SelectEntry {
                sequence: target_number + 1,
                state: ImageState::New,
            },
        );
    };
    let current = entries[active].map_or(0, |entry| entry.sequence);
    // The sequences that name `target` are `(target + 1) % 2 + 2i`; the first from `current` on.
    let base = (target_number + 1) % 2;
    let mut sequence = base;
    while current > sequence {
        sequence = sequence.saturating_add(2);
    }
    (
        1 - active,
        SelectEntry {
            sequence,
            state: ImageState::New,
        },
    )
}

/// What a firmware learns of its slots at boot, from the selection that the boot loader left.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BootSelection {
    /// The slot the firmware runs from.
    pub running: Slot,
    /// The entry that selected it, by its index, when one did.
    pub entry: Option<usize>,
    /// Whether the firmware runs on trial: the boot loader booted it once and waits for it to confirm
    /// itself (`PendingVerify`).
    pub trial: bool,
    /// Whether the other slot holds a newer firmware that the boot loader stopped booting: its entry
    /// is `Aborted` with a higher sequence than the running one's.
    pub rolled_back: bool,
}

/// The selection the firmware running from `running` reads: its own entry is the bootable one that
/// names `running` with the highest sequence.
#[must_use]
pub fn boot_selection(entries: &[Option<SelectEntry>; 2], running: Slot) -> BootSelection {
    let own = (0..2)
        .filter(|&index| {
            entries[index].is_some_and(|entry| entry.bootable() && entry.slot() == running)
        })
        .max_by_key(|&index| entries[index].map_or(0, |entry| entry.sequence));
    let own_sequence = own
        .and_then(|index| entries[index])
        .map_or(0, |entry| entry.sequence);
    let trial = own
        .and_then(|index| entries[index])
        .is_some_and(|entry| entry.state == ImageState::PendingVerify);
    let rolled_back = entries.iter().flatten().any(|entry| {
        entry.state == ImageState::Aborted
            && entry.slot() != running
            && entry.sequence > own_sequence
    });
    BootSelection {
        running,
        entry: own,
        trial,
        rolled_back,
    }
}

/// The other slot.
#[must_use]
pub const fn other(slot: Slot) -> Slot {
    match slot {
        Slot::A => Slot::B,
        Slot::B => Slot::A,
    }
}

/// What a firmware image carries, as its check read it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppImage {
    /// The version of its application descriptor: the truST release.
    pub version: String,
    /// The image's length, in bytes, up to its appended SHA-256.
    pub bytes: u32,
}

/// Why a firmware image is refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImageError {
    /// The first byte is not the image magic 0xE9.
    Magic,
    /// The header names another chip.
    Chip(u16),
    /// More segments than an image has, or a segment that runs past the upload.
    Segments,
    /// The image ends before its checksum or its SHA-256.
    Truncated,
    /// The upload holds more bytes than the image.
    Trailing {
        /// The image's length.
        image: u32,
        /// The upload's length.
        upload: u32,
    },
    /// The checksum over the segments' bytes is wrong.
    Checksum,
    /// The image carries no SHA-256, which the boot loader checks.
    NoDigest,
    /// The appended SHA-256 is not that of the image.
    Digest,
    /// No application descriptor at offset 32.
    Descriptor,
    /// The descriptor names another project.
    Project(String),
    /// The flash could not be read.
    Read,
}

impl core::fmt::Display for ImageError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Magic => write!(formatter, "the firmware image does not begin with the ESP image magic 0xE9"),
            Self::Chip(chip) => write!(
                formatter,
                "the firmware image is for chip id {chip}, and this chip is an ESP32-C6 (chip id {IMAGE_CHIP_ID})"
            ),
            Self::Segments => write!(formatter, "the firmware image's segments do not fit the upload"),
            Self::Truncated => write!(formatter, "the firmware image ends before its checksum and SHA-256"),
            Self::Trailing { image, upload } => write!(
                formatter,
                "the firmware image is {image} bytes and the upload {upload} bytes"
            ),
            Self::Checksum => write!(formatter, "the firmware image's checksum is wrong"),
            Self::NoDigest => write!(formatter, "the firmware image carries no SHA-256"),
            Self::Digest => write!(formatter, "the firmware image's SHA-256 is wrong"),
            Self::Descriptor => write!(formatter, "the firmware image has no application descriptor"),
            Self::Project(project) => write!(
                formatter,
                "the firmware image is the application '{project}', not truST's '{PROJECT}'"
            ),
            Self::Read => write!(formatter, "the firmware slot's flash could not be read"),
        }
    }
}

/// A reader of an image's bytes: the offset and the buffer to fill.
pub type ReadAt<'a> = dyn FnMut(u32, &mut [u8]) -> Result<(), ()> + 'a;

/// The application descriptor's magic word (`ESP_APP_DESC_MAGIC_WORD`).
const DESCRIPTOR_MAGIC: u32 = 0xABCD_5432;
/// Where the descriptor begins: after the image header and the first segment's header.
const DESCRIPTOR_OFFSET: u32 = 32;
/// The bytes of the descriptor this check reads: the magic word, the secure version, two reserved
/// words, the version (32 bytes) and the project name (32 bytes).
pub const DESCRIPTOR_BYTES: u32 = 80;
/// The most segments an image has (`ESP_IMAGE_MAX_SEGMENTS`).
const MAX_SEGMENTS: u8 = 16;
/// The checksum's starting value (`ESP_ROM_CHECKSUM_INITIAL`).
const CHECKSUM_INITIAL: u8 = 0xEF;
/// The bytes the check reads at a time.
const CHUNK: u32 = 256;

/// Checks the ESP-IDF application image of `len` bytes that `read` reads (offset, buffer), as the boot
/// loader checks it before it boots it and more: the magic, the chip, each segment inside the upload,
/// the checksum, the appended SHA-256, an upload that holds nothing after the image, and the
/// application descriptor of truST's firmware. Returns the descriptor's version.
///
/// # Errors
///
/// The [`ImageError`] of the first check that fails.
pub fn check_image(len: u32, read: &mut ReadAt<'_>) -> Result<AppImage, ImageError> {
    let mut reader = Reader {
        read,
        at: 0,
        len,
        digest: Sha256::new(),
    };
    let mut header = [0u8; 24];
    reader.take(&mut header)?;
    if header[0] != 0xE9 {
        return Err(ImageError::Magic);
    }
    let chip = u16::from_le_bytes([header[12], header[13]]);
    if chip != IMAGE_CHIP_ID {
        return Err(ImageError::Chip(chip));
    }
    let segments = header[1];
    if segments == 0 || segments > MAX_SEGMENTS {
        return Err(ImageError::Segments);
    }
    let hash_appended = header[23] == 1;
    let mut checksum = CHECKSUM_INITIAL;
    for _ in 0..segments {
        let mut segment = [0u8; 8];
        reader.take(&mut segment)?;
        let data_len = u32::from_le_bytes([segment[4], segment[5], segment[6], segment[7]]);
        if data_len > len.saturating_sub(reader.at) {
            return Err(ImageError::Segments);
        }
        let mut left = data_len;
        let mut chunk = [0u8; CHUNK as usize];
        while left > 0 {
            let take = left.min(CHUNK);
            let bytes = &mut chunk[..take as usize];
            reader.take(bytes)?;
            checksum = bytes.iter().fold(checksum, |sum, byte| sum ^ byte);
            left -= take;
        }
    }
    // The checksum is the last byte of the image's next 16-byte boundary.
    let padding = 15 - (reader.at % 16);
    let mut tail = [0u8; 16];
    reader.take(&mut tail[..=padding as usize])?;
    if tail[padding as usize] != checksum {
        return Err(ImageError::Checksum);
    }
    if !hash_appended {
        return Err(ImageError::NoDigest);
    }
    let image = reader.at;
    let computed = reader.digest.clone().finalize();
    let mut appended = [0u8; 32];
    reader.take(&mut appended)?;
    if computed.as_slice() != appended {
        return Err(ImageError::Digest);
    }
    if reader.at != len {
        return Err(ImageError::Trailing {
            image: reader.at,
            upload: len,
        });
    }
    let version = descriptor_version(image, reader.read)?;
    Ok(AppImage {
        version,
        bytes: image,
    })
}

/// The version of the application descriptor at offset 32 of the image whose first `image` bytes
/// `read` reads.
fn descriptor_version(image: u32, read: &mut ReadAt<'_>) -> Result<String, ImageError> {
    if image < DESCRIPTOR_OFFSET + DESCRIPTOR_BYTES {
        return Err(ImageError::Descriptor);
    }
    let mut descriptor = [0u8; DESCRIPTOR_BYTES as usize];
    read(DESCRIPTOR_OFFSET, &mut descriptor).map_err(|()| ImageError::Read)?;
    let magic = u32::from_le_bytes([descriptor[0], descriptor[1], descriptor[2], descriptor[3]]);
    if magic != DESCRIPTOR_MAGIC {
        return Err(ImageError::Descriptor);
    }
    let project = text(&descriptor[48..80]);
    if project != PROJECT {
        return Err(ImageError::Project(project));
    }
    Ok(text(&descriptor[16..48]))
}

/// The version an application descriptor's 80 bytes from offset 32 carry, `None` when they are not
/// the descriptor of truST's firmware.
#[must_use]
pub fn descriptor_of(descriptor: &[u8; DESCRIPTOR_BYTES as usize]) -> Option<String> {
    let magic = u32::from_le_bytes([descriptor[0], descriptor[1], descriptor[2], descriptor[3]]);
    (magic == DESCRIPTOR_MAGIC && text(&descriptor[48..80]) == PROJECT)
        .then(|| text(&descriptor[16..48]))
}

/// Where in an image [`descriptor_of`]'s bytes begin.
pub const DESCRIPTOR_AT: u32 = DESCRIPTOR_OFFSET;

/// A NUL-terminated text of a descriptor field.
fn text(field: &[u8]) -> String {
    let end = field
        .iter()
        .position(|byte| *byte == 0)
        .unwrap_or(field.len());
    String::from_utf8_lossy(&field[..end]).into_owned()
}

/// Reads an image from its start in order, feeding every byte read to the SHA-256.
struct Reader<'a> {
    read: &'a mut ReadAt<'a>,
    at: u32,
    len: u32,
    digest: Sha256,
}

impl Reader<'_> {
    fn take(&mut self, buffer: &mut [u8]) -> Result<(), ImageError> {
        let count = u32::try_from(buffer.len()).map_err(|_| ImageError::Truncated)?;
        if count > self.len.saturating_sub(self.at) {
            return Err(ImageError::Truncated);
        }
        (self.read)(self.at, buffer).map_err(|()| ImageError::Read)?;
        self.digest.update(&*buffer);
        self.at += count;
        Ok(())
    }
}

#[cfg(test)]
pub(crate) mod tests;
