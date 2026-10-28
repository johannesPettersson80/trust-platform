//! The STM32U5A9's platform for the resource loop (specification 34, ARCH-11 and §3.3): the shared chip
//! platform of `trust_runtime_services::chip::platform`, over this chip's profile. The profile supplies
//! the chip's name, its descriptor and the reason of its watchdog reset; the chip has no checkpoint store
//! until its checkpoint driver exists (design §4.4). The firmware supplies the watchdog, the runtime ID and
//! the settings, and this file keeps the chip's identity reader.

use smol_str::SmolStr;
use trust_runtime_core::deployment::DeviceDescriptor;
use trust_runtime_services::chip::platform::{ChipPlatform, ChipProfile};

use crate::profile::DEVICE_DESCRIPTOR;
use crate::watchdog::Watchdog;

/// The STM32U5A9's profile for the shared platform (specification 34, ARCH-11).
pub struct Stm32u5a9;

impl ChipProfile for Stm32u5a9 {
    /// The chip's reset reason is the independent watchdog flag of the reset control.
    type Reason = bool;

    /// The independent watchdog reset is the flag `true` (specification 34, the chip's supervision).
    fn watchdog_reset() -> bool {
        true
    }

    /// The chip's name (specification 11, `status`).
    fn platform_name() -> &'static str {
        trust_platform_stm32u5::usb::PLATFORM
    }

    /// The chip's descriptor (specification 34 §5.1.1, COMPAT-02 and TARGET-03).
    fn device_descriptor() -> DeviceDescriptor {
        DEVICE_DESCRIPTOR
    }

    /// No checkpoint store until the chip's checkpoint driver exists (design §4.4); the firmware refuses
    /// a record that asks for a retain mode.
    fn checkpoint_step_bytes() -> Option<usize> {
        None
    }
}

/// The STM32U5A9's platform for the single-thread resource loop, over its watchdog (specification 34,
/// ARCH-11).
pub type Platform = ChipPlatform<Stm32u5a9, Watchdog>;

/// The chip's stable runtime ID: its chip name and the 24 digits of its unique ID, the serial number
/// its USB device reports (specification 11, `status`).
pub fn device_runtime_id() -> SmolStr {
    let text =
        trust_platform_stm32u5::usb::runtime_id_text(trust_platform_stm32u5::usb_link::read_uid());
    // The text is ASCII by construction: the chip name, a hyphen and hexadecimal digits.
    core::str::from_utf8(&text).map_or_else(
        |_| SmolStr::new_static(trust_platform_stm32u5::usb::PLATFORM),
        SmolStr::new,
    )
}
