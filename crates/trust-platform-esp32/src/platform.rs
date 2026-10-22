//! The ESP32-C6's platform for the resource loop (specification 34, ARCH-11 and §3.3): the shared chip
//! platform of `trust_runtime_services::chip::platform`, over the C6's profile. The profile supplies the
//! chip's name, its descriptor, the reason of its watchdog reset and its checkpoint step (specification
//! 34 §10). The firmware supplies the C6's watchdog, runtime ID and settings to the platform, and the
//! adapter's own peripherals stay in `hardware/`.

use trust_runtime_core::deployment::DeviceDescriptor;
use trust_runtime_services::chip::platform::{ChipPlatform, ChipProfile};

#[cfg(feature = "native")]
use crate::profile::NATIVE_DEVICE_DESCRIPTOR;
use crate::profile::{CHECKPOINT_STEP_BYTES, DEVICE_DESCRIPTOR, PLATFORM, RESET_WATCHDOG};

/// The ESP32-C6's profile for the shared platform (specification 34, ARCH-11): the values the chip
/// names and describes itself with.
pub struct Esp32C6;

impl ChipProfile for Esp32C6 {
    type Reason = u8;

    /// The C6's watchdog reset is the MWDT0 reason (specification 34, the C6's supervision).
    fn watchdog_reset() -> u8 {
        RESET_WATCHDOG
    }

    /// The C6's name (specification 11, `status`).
    fn platform_name() -> &'static str {
        PLATFORM
    }

    /// The C6's descriptor (specification 34 §5.1.1, COMPAT-02 and TARGET-03).
    fn device_descriptor() -> DeviceDescriptor {
        DEVICE_DESCRIPTOR
    }

    /// The C6's checkpoint step: one 256-byte flash page between two scans (specification 34 §10).
    fn checkpoint_step_bytes() -> Option<usize> {
        Some(CHECKPOINT_STEP_BYTES)
    }
}

/// The C6's platform for the single-thread resource loop, over the watchdog `W` the adapter supplies
/// (specification 34, ARCH-11).
pub type Platform<W> = ChipPlatform<Esp32C6, W>;

/// The ESP32-C6's profile when its firmware runs compiled program images (specification 44 §15): the
/// values of [`Esp32C6`] with [`NATIVE_DEVICE_DESCRIPTOR`].
#[cfg(feature = "native")]
pub struct Esp32C6Native;

#[cfg(feature = "native")]
impl ChipProfile for Esp32C6Native {
    type Reason = u8;

    fn watchdog_reset() -> u8 {
        Esp32C6::watchdog_reset()
    }

    fn platform_name() -> &'static str {
        Esp32C6::platform_name()
    }

    /// The C6's descriptor for compiled program images.
    fn device_descriptor() -> DeviceDescriptor {
        NATIVE_DEVICE_DESCRIPTOR
    }

    fn checkpoint_step_bytes() -> Option<usize> {
        Esp32C6::checkpoint_step_bytes()
    }
}

/// The C6's platform for the resource loop of a firmware that runs compiled program images.
#[cfg(feature = "native")]
pub type NativePlatform<W> = ChipPlatform<Esp32C6Native, W>;
