//! The C6's records of two sectors: the device settings (specification 34 TARGET-02), `device_a` at
//! 0xD000 and `device_b` at 0xE000 of chips/esp32c6.toml, and the pairings (specification 11
//! §6.9.6), `pairing_a` at 0x7F_6000 and `pairing_b` at 0x7F_7000, one 4 KiB erase unit each, over a
//! [`RawFlash`] so that the bounds are checked on the host too. Each access stays inside its sector.

use trust_runtime_core::retain::checkpoint::FlashError;
use trust_runtime_services::device_settings::SettingsFlash;

use crate::checkpoint::RawFlash;
use crate::profile::FLASH_ERASE_BYTES;

/// The flash offset of the first settings sector, `device_a`.
pub const DEVICE_A_OFFSET: u32 = 0xD000;
/// The flash offset of the second settings sector, `device_b`.
pub const DEVICE_B_OFFSET: u32 = 0xE000;
/// The flash offset of the first pairing sector, `pairing_a`.
pub const PAIRING_A_OFFSET: u32 = 0x7F_6000;
/// The flash offset of the second pairing sector, `pairing_b`.
pub const PAIRING_B_OFFSET: u32 = 0x7F_7000;

/// The two sectors of one record over a [`RawFlash`].
pub struct DeviceSectors<R> {
    raw: R,
    /// The flash offsets of sector 0 and sector 1.
    offsets: [u32; 2],
}

impl<R> DeviceSectors<R> {
    /// The device settings' sectors over `raw`.
    pub fn new(raw: R) -> Self {
        Self {
            raw,
            offsets: [DEVICE_A_OFFSET, DEVICE_B_OFFSET],
        }
    }

    /// The pairings' sectors over `raw`.
    pub fn pairings(raw: R) -> Self {
        Self {
            raw,
            offsets: [PAIRING_A_OFFSET, PAIRING_B_OFFSET],
        }
    }

    /// The flash offset of `bytes` bytes at the start of `sector`, when they fit it.
    fn address(&self, sector: usize, bytes: usize) -> Option<u32> {
        let base = *self.offsets.get(sector)?;
        (u32::try_from(bytes).ok()? <= FLASH_ERASE_BYTES).then_some(base)
    }
}

impl<R: RawFlash> SettingsFlash for DeviceSectors<R> {
    fn sector_bytes(&self) -> usize {
        FLASH_ERASE_BYTES as usize
    }

    fn read(&mut self, sector: usize, bytes: &mut [u8]) -> Result<(), FlashError> {
        let at = self.address(sector, bytes.len()).ok_or(FlashError::Read)?;
        self.raw.read_at(at, bytes)
    }

    fn erase(&mut self, sector: usize) -> Result<(), FlashError> {
        let at = self.address(sector, 0).ok_or(FlashError::Erase)?;
        self.raw.erase_at(at)
    }

    fn program(&mut self, sector: usize, bytes: &[u8]) -> Result<(), FlashError> {
        let at = self
            .address(sector, bytes.len())
            .ok_or(FlashError::Program)?;
        self.raw.program_at(at, bytes)
    }
}

#[cfg(test)]
mod tests {
    use alloc::rc::Rc;
    use alloc::vec::Vec;
    use core::cell::RefCell;

    use super::*;

    /// The accesses a flash saw: each one's kind, address and length.
    type Accesses = Rc<RefCell<Vec<(&'static str, u32, usize)>>>;

    /// A flash that records each access's address and length.
    #[derive(Clone, Default)]
    struct Recording(Accesses);

    impl RawFlash for Recording {
        fn read_at(&mut self, address: u32, bytes: &mut [u8]) -> Result<(), FlashError> {
            bytes.fill(0xFF);
            self.0.borrow_mut().push(("read", address, bytes.len()));
            Ok(())
        }

        fn program_at(&mut self, address: u32, bytes: &[u8]) -> Result<(), FlashError> {
            self.0.borrow_mut().push(("program", address, bytes.len()));
            Ok(())
        }

        fn erase_at(&mut self, address: u32) -> Result<(), FlashError> {
            self.0.borrow_mut().push(("erase", address, 4_096));
            Ok(())
        }
    }

    /// Sector 0 is `device_a` at 0xD000 and sector 1 `device_b` at 0xE000; an access past a sector's
    /// 4 KiB or to a third sector reaches no flash. Catches a settings write that would run into the
    /// first firmware slot at 0x1_0000 or the selection at 0xB000.
    #[test]
    fn each_sector_lies_where_the_chip_file_puts_it() {
        let flash = Recording::default();
        let mut sectors = DeviceSectors::new(flash.clone());
        sectors.erase(0).expect("an erase");
        sectors.program(1, &[0; 8]).expect("a program");
        assert_eq!(
            *flash.0.borrow(),
            [("erase", 0xD000, 4_096), ("program", 0xE000, 8)]
        );
        assert_eq!(sectors.program(1, &[0; 4_100]), Err(FlashError::Program));
        assert_eq!(sectors.erase(2), Err(FlashError::Erase));
        assert_eq!(
            flash.0.borrow().len(),
            2,
            "no other access reached the flash"
        );
    }

    /// Sector 0 of the pairings is `pairing_a` at 0x7F_6000 and sector 1 `pairing_b` at 0x7F_7000,
    /// after the program store and before the checkpoint slots; an access past a sector's 4 KiB
    /// reaches no flash. Catches a pairing write that lands in the device settings, the program store
    /// or `checkpoint_a` at 0x7F_8000.
    #[test]
    fn the_pairing_sectors_lie_where_the_chip_file_puts_them() {
        let flash = Recording::default();
        let mut sectors = DeviceSectors::pairings(flash.clone());
        sectors.erase(1).expect("an erase");
        sectors.program(0, &[0; 8]).expect("a program");
        assert_eq!(
            *flash.0.borrow(),
            [("erase", 0x7F_7000, 4_096), ("program", 0x7F_6000, 8)]
        );
        assert_eq!(sectors.program(0, &[0; 4_100]), Err(FlashError::Program));
        assert_eq!(
            flash.0.borrow().len(),
            2,
            "no other access reached the flash"
        );
    }
}
