//! The chip's identity as the boot reads it (design §1.6 and §1.9; specification 34, D.5): DEV_ID of
//! DBGMCU_IDCODE, the flash size register and TZEN in FLASH_OPTR. A part that does not match, or
//! that has TrustZone enabled, does not run: the link serves Stopped with the reason.
use stm32u5::stm32u5a9 as pac;
use trust_platform_stm32u5::chip::CHIP;

/// The flash size register: the size in KiB in its low half-word (RM0456; ST's `FLASHSIZE_BASE`).
const FLASH_SIZE_REGISTER: usize = 0x0BFA_07A0;

/// What the boot reads from the chip.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Identity {
    /// DEV_ID, bits 11:0 of DBGMCU_IDCODE.
    pub dev_id: u16,
    /// The flash size the part reports, in KiB.
    pub flash_kib: u16,
    /// TZEN, bit 31 of FLASH_OPTR: TrustZone is enabled.
    pub tzen: bool,
}

/// Reads the identity of this part.
pub fn read(dbgmcu: &pac::DBGMCU, flash: &pac::FLASH) -> Identity {
    Identity {
        dev_id: dbgmcu.idcode().read().dev_id().bits(),
        flash_kib: flash_size_kib(),
        tzen: flash.optr().read().tzen().bit(),
    }
}

/// The flash size the part reports, in KiB. The register is read as a volatile half-word, which has no
/// effect on the chip. A register that reads 0 or 0xFFFF has no programmed size, and the chip's default
/// applies (design §1.1).
fn flash_size_kib() -> u16 {
    // SAFETY: the flash size register is a read-only word of the chip's system memory at a fixed
    // address (RM0456). A volatile half-word read has no effect on the chip.
    let raw = unsafe { core::ptr::read_volatile(FLASH_SIZE_REGISTER as *const u16) };
    match raw {
        0 | u16::MAX => CHIP.flash_kib,
        kib => kib,
    }
}

/// Whether the part is the STM32U5A9 this firmware is built for, with TrustZone off.
///
/// # Errors
///
/// The reason the part does not run: TrustZone is enabled, or DEV_ID or the flash size differ from
/// `chips/stm32u5a9.toml`.
pub fn check(identity: Identity) -> Result<(), &'static str> {
    if identity.tzen {
        return Err("TrustZone is enabled (TZEN is set), and the firmware does not run on it");
    }
    if identity.dev_id != CHIP.device_id {
        return Err("the chip's DBGMCU DEV_ID is not the STM32U5A9's");
    }
    if identity.flash_kib != CHIP.flash_kib {
        return Err("the chip's flash size register is not the STM32U5A9's 4096 KiB");
    }
    Ok(())
}
