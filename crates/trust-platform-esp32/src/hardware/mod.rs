//! Bare-metal ownership and initialization of the fixed ESP32-C6 board peripherals.
use crate::{
    image::{decode_image_header, IMAGE_HEADER_BYTES},
    profile::{CPU_HZ, FACTORY_OFFSET, STBC_BYTES, STBC_OFFSET},
};
use core::sync::atomic::{AtomicBool, Ordering};
use esp_hal::{
    clock::CpuClock, efuse, timer::systimer::SystemTimer, usb::usb_serial_jtag::UsbSerialJtag,
    Config,
};
use esp_storage::FlashStorage;

mod clock;
mod console;
mod emergency;
mod io;
mod reset;
pub use clock::Monotonic;
pub use console::Console;
pub use emergency::emergency_halt;
pub use io::DigitalIo;
pub use reset::{ResetControl, Watchdog};

/// Set by the first `Board::take`. `esp_hal::init` panics when it runs twice, so this guard
/// turns a second call into `PeripheralsTaken` before the HAL is touched.
static TAKEN: AtomicBool = AtomicBool::new(false);

/// Initialization or explicit peripheral-service failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoardError {
    /// The board was already taken; a second `Board::take` is refused.
    PeripheralsTaken,
    /// Watchdog interval must be between 1 and 60,000 nominal milliseconds.
    WatchdogInterval,
    /// Phase zero is invalid, or the RTC-memory readback did not match the written cookie.
    ResetCookie,
    /// The flash driver rejected a read.
    FlashRead,
    /// The running image header lacks the ESP magic or names an unknown flash size.
    ImageHeader,
    /// The 1 kHz tick alarm could not be configured.
    TickTimer,
    /// The caller's buffer is larger than the `stbc` partition.
    BufferTooLarge,
}

/// Chip, MAC, ISA and clock identity captured at boot. The reset reason is
/// available from [`ResetControl::reason`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Identity {
    /// Chip ID from the running image header (13 on the owned board).
    pub chip_id: u16,
    /// Flash size in KiB from the running image header.
    pub flash_kib: u16,
    /// Chip revision as major·100 + minor from the eFuse block.
    pub revision: u16,
    /// Base MAC address from eFuse, in the printed order (`20:6e:f1:17:04:c0`).
    pub mac: [u8; 6],
    /// Machine ISA CSR (`misa`) read at boot.
    pub misa: u32,
    /// Configured CPU clock in Hz; this is not a measured clock accuracy.
    pub cpu_hz: u32,
}

/// Separately owned peripherals: the engine borrows the clock while the firmware samples and
/// publishes process images and writes bounded trace records.
pub struct Board {
    /// SYSTIMER time and the optional 1 kHz tick instrumentation.
    pub clock: Monotonic,
    /// BOOT input on GPIO9 and the plain output on GPIO18.
    pub io: DigitalIo,
    /// Native USB-Serial/JTAG trace transmitter.
    pub console: Console,
    /// MWDT0 supervisor: configured by `take`, enabled only by `start_ms`.
    pub watchdog: Watchdog,
    /// Boot reset reason and the RTC-memory reset cookie.
    pub reset: ResetControl,
    /// Identity captured at boot.
    pub identity: Identity,
    /// Flash driver for the factory header and the `stbc` partition.
    flash: FlashStorage<'static>,
}

impl Board {
    /// Take the board once and configure the stock DevKitC-1 connection. GPIO18 is driven low
    /// before anything else, then the console, clock, watchdog (configured, not enabled), reset
    /// control and identity are set up. A second call returns `PeripheralsTaken`.
    #[inline(never)]
    pub fn take() -> Result<Self, BoardError> {
        if TAKEN.swap(true, Ordering::AcqRel) {
            return Err(BoardError::PeripheralsTaken);
        }
        let peripherals = esp_hal::init(Config::default().with_cpu_clock(CpuClock::max()));
        let io = DigitalIo::new(peripherals.GPIO9, peripherals.GPIO18);
        let console = Console::new(UsbSerialJtag::new(peripherals.USB_DEVICE));
        let clock = Monotonic::new(SystemTimer::new(peripherals.SYSTIMER).alarm0);
        let watchdog = Watchdog::new(peripherals.TIMG0);
        let reset = ResetControl::new();
        let mut flash = FlashStorage::new(peripherals.FLASH);
        let identity = read_identity(&mut flash)?;
        Ok(Self {
            clock,
            io,
            console,
            watchdog,
            reset,
            identity,
            flash,
        })
    }

    /// Read the first `buffer.len()` bytes of the `stbc` partition. Call only while STOPPED,
    /// before RUN; this reads flash and never writes or erases it.
    pub fn read_application(&mut self, buffer: &mut [u8]) -> Result<(), BoardError> {
        if buffer.len() > STBC_BYTES {
            return Err(BoardError::BufferTooLarge);
        }
        self.flash
            .read(STBC_OFFSET, buffer)
            .map_err(|_| BoardError::FlashRead)
    }
}

fn read_identity(flash: &mut FlashStorage<'_>) -> Result<Identity, BoardError> {
    let mut header = [0u8; IMAGE_HEADER_BYTES];
    flash
        .read(FACTORY_OFFSET, &mut header)
        .map_err(|_| BoardError::FlashRead)?;
    let image = decode_image_header(&header).ok_or(BoardError::ImageHeader)?;
    let mut mac = [0u8; 6];
    // `MacAddress` always holds exactly six bytes, so the copy cannot fail.
    mac.copy_from_slice(efuse::base_mac_address().as_bytes());
    Ok(Identity {
        chip_id: image.chip_id,
        flash_kib: image.flash_kib,
        revision: efuse::chip_revision().combined(),
        mac,
        misa: misa(),
        cpu_hz: CPU_HZ,
    })
}

/// Read the machine ISA CSR; the read touches no memory and changes no state.
fn misa() -> u32 {
    let value: u32;
    // SAFETY: `csrr` copies the read-only `misa` CSR into a register. It has no memory operand
    // and no side effect, and the value is only used as an ISA description.
    unsafe { core::arch::asm!("csrr {0}, misa", out(reg) value, options(nomem, nostack)) };
    value
}
