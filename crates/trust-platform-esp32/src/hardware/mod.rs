//! Bare-metal ownership and initialization of the fixed ESP32-C6 board peripherals.
use crate::{
    image::{decode_image_header, IMAGE_HEADER_BYTES},
    process_io::{claim_lines, ClaimedLines, GpioBank, PinError},
    profile::{CPU_HZ, FACTORY_OFFSET, STBC_BYTES},
    program_flash::FlashOwner,
};
use alloc::boxed::Box;
use core::sync::atomic::{AtomicBool, Ordering};
use esp_hal::{
    clock::CpuClock, efuse, gpio::Pin, timer::systimer::Alarm, usb::usb_serial_jtag::UsbSerialJtag,
    Config,
};
use esp_storage::FlashStorage;
use trust_runtime_core::deployment::{GpioInput, GpioOutput};
use trust_runtime_core::ports::ProgramFlash;
use trust_runtime_core::retain::checkpoint::{CheckpointError, CheckpointFlash};

mod checkpoint_flash;
mod clock;
mod console;
pub(crate) mod emergency;
mod reset;
mod usb_link;
mod wait;
pub use clock::Monotonic;
pub use console::Console;
pub use emergency::emergency_halt;
pub use reset::{ResetControl, Watchdog};
pub use usb_link::UsbLink;
pub use wait::Wait;

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
    /// The checkpoint flash or the program slots were handed out already; each is handed out once.
    FlashTaken,
    /// The checkpoint slots' geometry cannot hold the checkpoint log.
    Checkpoint(CheckpointError),
    /// The program slots' geometry cannot be used.
    ProgramSlots(CheckpointError),
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

/// Separately owned peripherals: the resource loop borrows the clock and the wait, claims the
/// record's GPIO lines, reads the installed application from flash, and feeds the watchdog.
pub struct Board {
    /// SYSTIMER time, read by the resource loop's clock.
    pub clock: Monotonic,
    /// Native USB-Serial/JTAG peripheral; [`UsbLink::bind`] takes the same port's registers.
    pub console: Console,
    /// MWDT0 supervisor: configured by `take`, enabled by the firmware from the record.
    pub watchdog: Watchdog,
    /// Boot reset reason and the persistent reset record.
    pub reset: ResetControl,
    /// Identity captured at boot.
    pub identity: Identity,
    /// The flash, owned once: the program slots, the checkpoint slots and the installed application
    /// read share it (specification 34 §5.1 and §10.1).
    flash: FlashOwner<FlashStorage<'static>>,
    /// The chip's GPIO bank, from which the record's lines are claimed by pin name.
    gpio: GpioBank,
    /// The SYSTIMER alarm the resource loop's wait arms at its deadline.
    wait_alarm: Option<Alarm<'static>>,
}

impl Board {
    /// Take the board once and configure the stock DevKitC-1 connection: the console and the
    /// resource loop's clock and deadline alarm, the watchdog (configured, not enabled), the
    /// reset control and record, the flash and the identity, and the GPIO bank. A second call
    /// returns `PeripheralsTaken`.
    #[inline(never)]
    pub fn take() -> Result<Self, BoardError> {
        if TAKEN.swap(true, Ordering::AcqRel) {
            return Err(BoardError::PeripheralsTaken);
        }
        let peripherals = esp_hal::init(Config::default().with_cpu_clock(CpuClock::max()));
        // The GPIO bank takes every pin first, so the record's lines can be claimed by name. The
        // other peripherals are moved out of the same `Peripherals` value, one field each.
        let gpio = GpioBank::new([
            Some(peripherals.GPIO0.degrade()),
            Some(peripherals.GPIO1.degrade()),
            Some(peripherals.GPIO2.degrade()),
            Some(peripherals.GPIO3.degrade()),
            Some(peripherals.GPIO4.degrade()),
            Some(peripherals.GPIO5.degrade()),
            Some(peripherals.GPIO6.degrade()),
            Some(peripherals.GPIO7.degrade()),
            Some(peripherals.GPIO8.degrade()),
            Some(peripherals.GPIO9.degrade()),
            Some(peripherals.GPIO10.degrade()),
            Some(peripherals.GPIO11.degrade()),
            Some(peripherals.GPIO12.degrade()),
            Some(peripherals.GPIO13.degrade()),
            Some(peripherals.GPIO14.degrade()),
            Some(peripherals.GPIO15.degrade()),
            Some(peripherals.GPIO16.degrade()),
            Some(peripherals.GPIO17.degrade()),
            Some(peripherals.GPIO18.degrade()),
            Some(peripherals.GPIO19.degrade()),
            Some(peripherals.GPIO20.degrade()),
            Some(peripherals.GPIO21.degrade()),
            Some(peripherals.GPIO22.degrade()),
            Some(peripherals.GPIO23.degrade()),
            Some(peripherals.GPIO24.degrade()),
            Some(peripherals.GPIO25.degrade()),
            Some(peripherals.GPIO26.degrade()),
            Some(peripherals.GPIO27.degrade()),
            Some(peripherals.GPIO28.degrade()),
            Some(peripherals.GPIO29.degrade()),
            Some(peripherals.GPIO30.degrade()),
        ]);
        let console = Console::new(UsbSerialJtag::new(peripherals.USB_DEVICE));
        let systimer = esp_hal::timer::systimer::SystemTimer::new(peripherals.SYSTIMER);
        let clock = Monotonic::new(systimer.alarm0);
        let wait_alarm = Some(systimer.alarm1);
        let watchdog = Watchdog::new(peripherals.TIMG0);
        let reset = ResetControl::new();
        let mut flash = FlashStorage::new(peripherals.FLASH);
        let identity = read_identity(&mut flash)?;
        Ok(Self {
            clock,
            console,
            watchdog,
            reset,
            identity,
            flash: FlashOwner::new(flash),
            gpio,
            wait_alarm,
        })
    }

    /// Read the first `buffer.len()` bytes of the installed application, which begin at slot A.
    /// Call only while STOPPED, before RUN; this reads flash and never writes or erases it.
    pub fn read_application(&mut self, buffer: &mut [u8]) -> Result<(), BoardError> {
        if buffer.len() > STBC_BYTES {
            return Err(BoardError::BufferTooLarge);
        }
        self.flash
            .read_application(buffer)
            .map_err(|_| BoardError::FlashRead)
    }

    /// Claims the record's GPIO lines by pin name, in `io.toml` order, and drives each output to
    /// its initial level before it becomes an output (design §1.1, specification 34 STATE-01 and
    /// TARGET-01). A pin the chip lacks or keeps, or one named by two lines, is refused.
    ///
    /// # Errors
    ///
    /// The [`PinError`] of the first line that cannot be claimed.
    pub fn claim_lines(
        &mut self,
        inputs: &[GpioInput],
        outputs: &[GpioOutput],
    ) -> Result<ClaimedLines, PinError> {
        claim_lines(&mut self.gpio, inputs, outputs)
    }

    /// The SYSTIMER alarm the resource loop's wait arms; taken once, so the wait owns it.
    pub fn take_wait_alarm(&mut self) -> Option<Alarm<'static>> {
        self.wait_alarm.take()
    }

    /// The chip's checkpoint flash (specification 34 §10.1), handed out once. It shares the flash
    /// with the program slots and with [`Board::read_application`], so it may be taken at any point
    /// after the board is taken.
    ///
    /// # Errors
    ///
    /// [`BoardError::FlashTaken`] when the checkpoint flash was handed out already, and
    /// [`BoardError::Checkpoint`] when the slots' geometry cannot hold the checkpoint log.
    pub fn take_checkpoint_flash(&mut self) -> Result<Box<dyn CheckpointFlash>, BoardError> {
        let checkpoint = self
            .flash
            .take_checkpoint()
            .map_err(BoardError::Checkpoint)?
            .ok_or(BoardError::FlashTaken)?;
        let checkpoint: Box<dyn CheckpointFlash> = Box::new(checkpoint);
        Ok(checkpoint)
    }

    /// The chip's program slots (specification 34 §5.1), handed out once. They share the flash with
    /// the checkpoint slots and with [`Board::read_application`].
    ///
    /// # Errors
    ///
    /// [`BoardError::FlashTaken`] when the program slots were handed out already, and
    /// [`BoardError::ProgramSlots`] when their geometry cannot be used.
    pub fn take_program_flash(&mut self) -> Result<Box<dyn ProgramFlash>, BoardError> {
        let program = self
            .flash
            .take_program()
            .map_err(BoardError::ProgramSlots)?
            .ok_or(BoardError::FlashTaken)?;
        let program: Box<dyn ProgramFlash> = Box::new(program);
        Ok(program)
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
