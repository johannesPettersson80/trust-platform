//! The C6's firmware slots and the boot loader's selection in its flash (specification 34
//! TARGET-04), over a [`RawFlash`] so that the rules run on the host too.
//!
//! [`FirmwareSlots`] is the program-store shape of the two slots, `ota_0` as `Slot::A` and `ota_1` as
//! `Slot::B`, which the shared loop writes a firmware upload into. [`Selection`] reads and writes the
//! two entries of `otadata`. [`C6Firmware`] is what the loop asks of the chip: the slot it runs from,
//! the check of an uploaded image ([`check_image`]), the selection of the slot for the next boot on
//! trial, and the reset. At boot [`C6Firmware::boot`] reads the selection the boot loader left and
//! the version of a firmware it rolled back, and [`C6Firmware::confirm`] marks a firmware that runs
//! on trial as `Valid` once its boot is done.

use alloc::string::{String, ToString};
use alloc::vec;

use trust_runtime_core::ports::ProgramFlash;
use trust_runtime_core::retain::checkpoint::{CheckpointError, FlashError, FlashGeometry, Slot};
use trust_runtime_services::firmware::{FirmwareDevice, FirmwareFacts, FirmwareImage};

use crate::checkpoint::RawFlash;
use crate::firmware_slots::{
    activation, boot_selection, check_image, decode_entry, descriptor_of, encode_entry,
    entry_offset, other, slot_offset, BootSelection, ImageState, SelectEntry, DESCRIPTOR_AT,
    DESCRIPTOR_BYTES, ENTRY_BYTES, FIRMWARE_SLOT_BYTES,
};
use crate::profile::{FLASH_ERASE_BYTES, FLASH_PROGRAM_BYTES};

/// The two firmware slots over a [`RawFlash`], as the shared loop drives them through
/// [`ProgramFlash`]: 4-byte program units, 4 KiB erase units and slots of
/// [`FIRMWARE_SLOT_BYTES`].
pub struct FirmwareSlots<R> {
    raw: R,
    geometry: FlashGeometry,
}

impl<R> FirmwareSlots<R> {
    /// The firmware slots over `raw`.
    ///
    /// # Errors
    ///
    /// [`CheckpointError::Geometry`] when the flash geometry cannot hold a slot.
    pub fn new(raw: R) -> Result<Self, CheckpointError> {
        let geometry =
            FlashGeometry::new(FLASH_PROGRAM_BYTES, FLASH_ERASE_BYTES, FIRMWARE_SLOT_BYTES)?;
        Ok(Self { raw, geometry })
    }
}

/// The absolute address of `len` bytes at `offset` in firmware slot `slot`, when they are whole
/// program units inside the slot.
fn access_address(slot: Slot, offset: u32, len: usize) -> Option<u32> {
    let len = u32::try_from(len).ok()?;
    let whole_units =
        offset.is_multiple_of(FLASH_PROGRAM_BYTES) && len.is_multiple_of(FLASH_PROGRAM_BYTES);
    let inside = offset.checked_add(len)? <= FIRMWARE_SLOT_BYTES;
    (whole_units && inside).then(|| slot_offset(slot) + offset)
}

impl<R: RawFlash> ProgramFlash for FirmwareSlots<R> {
    fn geometry(&self) -> FlashGeometry {
        self.geometry
    }

    fn read(&mut self, slot: Slot, offset: u32, bytes: &mut [u8]) -> Result<(), FlashError> {
        let address = access_address(slot, offset, bytes.len()).ok_or(FlashError::Read)?;
        self.raw.read_at(address, bytes)
    }

    fn program(&mut self, slot: Slot, offset: u32, bytes: &[u8]) -> Result<(), FlashError> {
        let address = access_address(slot, offset, bytes.len()).ok_or(FlashError::Program)?;
        self.raw.program_at(address, bytes)
    }

    fn erase(&mut self, slot: Slot, offset: u32) -> Result<(), FlashError> {
        if !offset.is_multiple_of(FLASH_ERASE_BYTES) || offset >= FIRMWARE_SLOT_BYTES {
            return Err(FlashError::Erase);
        }
        self.raw.erase_at(slot_offset(slot) + offset)
    }
}

/// The boot loader's two selection entries in `otadata`.
pub struct Selection<R> {
    raw: R,
}

impl<R: RawFlash> Selection<R> {
    /// The selection over `raw`.
    pub fn new(raw: R) -> Self {
        Self { raw }
    }

    /// Reads both entries; an erased or damaged entry, or one that cannot be read, is none.
    pub fn entries(&mut self) -> [Option<SelectEntry>; 2] {
        let mut read = |index: usize| {
            let mut bytes = [0u8; ENTRY_BYTES];
            self.raw
                .read_at(entry_offset(index), &mut bytes)
                .ok()
                .and_then(|()| decode_entry(&bytes))
        };
        [read(0), read(1)]
    }

    /// Writes `entry` as entry `index`: its sector is erased, the 32 bytes programmed and read back.
    ///
    /// # Errors
    ///
    /// The [`FlashError`] of the erase, the program, or a read-back that differs.
    pub fn write(&mut self, index: usize, entry: SelectEntry) -> Result<(), FlashError> {
        let address = entry_offset(index);
        let bytes = encode_entry(entry);
        self.raw.erase_at(address)?;
        self.raw.program_at(address, &bytes)?;
        let mut read = [0u8; ENTRY_BYTES];
        self.raw.read_at(address, &mut read)?;
        if read == bytes {
            Ok(())
        } else {
            Err(FlashError::Program)
        }
    }
}

/// What the loop asks of the C6 for a firmware update, and what its boot reads of its slots.
pub struct C6Firmware<R> {
    raw: R,
    running: Slot,
}

impl<R: RawFlash + Clone> C6Firmware<R> {
    /// The firmware of a C6 that runs from `running` (read from its MMU by the hardware adapter), over
    /// `raw`.
    pub fn new(raw: R, running: Slot) -> Self {
        Self { raw, running }
    }

    /// What the boot reads of the slots: the selection the boot loader left, with this firmware's
    /// `version`, and the version of a newer firmware in the other slot that the boot loader rolled
    /// back.
    pub fn boot(&mut self, version: &str) -> (BootSelection, FirmwareFacts) {
        let entries = Selection::new(self.raw.clone()).entries();
        let selection = boot_selection(&entries, self.running);
        let rolled_back = selection
            .rolled_back
            .then(|| self.version_in(other(self.running)))
            .flatten();
        let facts = FirmwareFacts {
            slot: self.running,
            version: version.to_string(),
            confirmed: !selection.trial,
            rolled_back,
        };
        (selection, facts)
    }

    /// Marks the firmware that runs on trial as `Valid`, so that the boot loader keeps booting it,
    /// once its boot is done (specification 34 TARGET-04). A firmware not on trial writes nothing.
    ///
    /// # Errors
    ///
    /// The [`FlashError`] of the write.
    pub fn confirm(&mut self, selection: &BootSelection) -> Result<(), FlashError> {
        let Some(index) = selection.entry.filter(|_| selection.trial) else {
            return Ok(());
        };
        let mut writer = Selection::new(self.raw.clone());
        let Some(entry) = writer.entries()[index] else {
            return Ok(());
        };
        writer.write(
            index,
            SelectEntry {
                sequence: entry.sequence,
                state: ImageState::Valid,
            },
        )
    }

    /// The version of the truST firmware in `slot`, read from its application descriptor; `None` when
    /// the slot holds none.
    fn version_in(&mut self, slot: Slot) -> Option<String> {
        let mut descriptor = [0u8; DESCRIPTOR_BYTES as usize];
        self.raw
            .read_at(slot_offset(slot) + DESCRIPTOR_AT, &mut descriptor)
            .ok()?;
        descriptor_of(&descriptor)
    }
}

impl<R: RawFlash + Clone> FirmwareDevice for C6Firmware<R> {
    fn running(&self) -> Slot {
        self.running
    }

    fn check(
        &mut self,
        flash: &mut dyn ProgramFlash,
        slot: Slot,
        head: &[u8],
        bytes: usize,
    ) -> Result<FirmwareImage, String> {
        let len =
            u32::try_from(bytes).map_err(|_| String::from("the firmware image is too long"))?;
        let unit = FLASH_PROGRAM_BYTES as usize;
        let mut words = vec![0u8; 0];
        let mut read = |at: u32, buffer: &mut [u8]| -> Result<(), ()> {
            // The first bytes are the session's; the rest lie in the slot, read in whole units.
            let start = at as usize;
            let end = start + buffer.len();
            let kept = head.len().min(end).max(start);
            if start < head.len() {
                buffer[..kept - start].copy_from_slice(&head[start..kept]);
            }
            if end > kept {
                let first = kept - kept % unit;
                let last = end.next_multiple_of(unit);
                words.resize(last - first, 0);
                let offset = u32::try_from(first).map_err(|_| ())?;
                flash.read(slot, offset, &mut words).map_err(|_| ())?;
                buffer[kept - start..].copy_from_slice(&words[kept - first..end - first]);
            }
            Ok(())
        };
        check_image(len, &mut read)
            .map(|image| FirmwareImage {
                version: image.version,
            })
            .map_err(|error| error.to_string())
    }

    fn activate(&mut self, slot: Slot) -> Result<(), String> {
        let mut selection = Selection::new(self.raw.clone());
        let entries = selection.entries();
        let (index, entry) = activation(&entries, slot);
        selection
            .write(index, entry)
            .map_err(|_| String::from("the boot loader's selection could not be written"))
    }
}

#[cfg(test)]
mod tests;
