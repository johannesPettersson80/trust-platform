//! Bare-metal ownership and initialization of the fixed ESP32-C6 board peripherals.
use crate::{
    boot_counter::{BootCounter, BootCounterError},
    device_flash::DeviceSectors,
    firmware_flash::{C6Firmware, FirmwareSlots},
    firmware_slots::{slot_at, slot_offset},
    image::{decode_image_header, IMAGE_HEADER_BYTES},
    loader::ProgramHardware,
    process_io::{bind_lines, claim_lines, GpioBank, GpioIo},
    profile::{CPU_HZ, STBC_BYTES},
    program_flash::{FlashOwner, SharedFlash},
};
use alloc::boxed::Box;
use alloc::format;
use alloc::string::String;
use core::sync::atomic::{AtomicBool, Ordering};
use esp_hal::{
    clock::CpuClock, efuse, gpio::Pin, timer::systimer::Alarm, usb::usb_serial_jtag::UsbSerialJtag,
    Config,
};
use esp_storage::FlashStorage;
use trust_platform_memory::reset::{ResetCause, ResetStore};
use trust_runtime_core::deployment::Configuration;
use trust_runtime_core::ports::ProgramFlash;
use trust_runtime_core::retain::checkpoint::{CheckpointError, CheckpointFlash, Slot};
use trust_runtime_services::chip::process_io::record_lines;
use trust_runtime_services::device_settings::SettingsFlash;

pub(crate) mod calendar;
mod checkpoint_flash;
mod clock;
mod code_region;
mod console;
pub(crate) mod emergency;
mod image_window;
mod reset;
mod usb_link;
mod wait;
mod wifi;
pub use calendar::KERNEL_FUNCTIONS;
pub use clock::Monotonic;
pub use code_region::{program_region, C6Code, Line, ProgramCode};
pub use console::Console;
pub use emergency::emergency_halt;
pub use reset::{end_program_load, mark_program_load, ResetControl, Watchdog};
pub use usb_link::UsbLink;
pub use wait::Wait;
pub use wifi::{bring_up, start_scheduler, C6Net, NetLink, RadioFailure, RadioParts};

/// Set by the first `Board::take`. `esp_hal::init` panics when it runs twice, so this guard
/// turns a second call into `PeripheralsTaken` before the HAL is touched.
static TAKEN: AtomicBool = AtomicBool::new(false);

/// Initialization or explicit peripheral-service failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoardError {
    /// The board was already taken; a second `Board::take` is refused.
    PeripheralsTaken,
    /// Phase zero is invalid, or the RTC-memory readback did not match the written cookie.
    ResetCookie,
    /// The flash driver rejected a read.
    FlashRead,
    /// The program slots were handed out already (once), or a writer on the checkpoint flash exists.
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
    /// The boot could not be counted in the flash (specification 34 PORT-05).
    BootCounter(BootCounterError),
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

/// Separately owned peripherals: the boot counts itself in the flash and reads the program store,
/// and [`Board::into_parts`] hands the resource loop its clock, wait and watchdog and the hardware
/// that each program claims its lines and its checkpoint flash from.
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
    /// The flash, owned once: the program slots, the checkpoint slots, the boot counter and the
    /// installed application read share it (specification 34 §5.1 and §10.1).
    flash: FlashOwner<FlashStorage<'static>>,
    /// The chip's GPIO bank, from which the record's lines are claimed by pin name.
    gpio: GpioBank,
    /// The SYSTIMER alarm the resource loop's wait arms at its deadline.
    wait_alarm: Option<Alarm<'static>>,
    /// What the Wi-Fi station takes (checklist L3).
    radio: Option<RadioParts>,
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
        // The wall clock reads SYSTIMER, so it starts after the clock (specification 34 TARGET-06).
        calendar::start(peripherals.RTC_TIMER);
        let wait_alarm = Some(systimer.alarm1);
        let radio = Some(RadioParts {
            scheduler_alarm: systimer.alarm2,
            scheduler_interrupt: peripherals.FROM_CPU_INTR0,
            wifi: peripherals.WIFI,
        });
        let watchdog = Watchdog::new(peripherals.TIMG0);
        let reset = ResetControl::new();
        let mut flash = FlashStorage::new(peripherals.FLASH);
        let identity = read_identity(&mut flash, running_slot())?;
        Ok(Self {
            clock,
            console,
            watchdog,
            reset,
            identity,
            flash: FlashOwner::new(flash),
            gpio,
            wait_alarm,
            radio,
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

    /// Counts this boot in the flash and returns the new count, which is the boot number of the transfer
    /// ids (specification 34 PORT-05). The count survives a loss of power; the reset record's copy in
    /// RTC fast memory is set to it, so a record the next panic writes carries the same count.
    ///
    /// # Errors
    ///
    /// [`BoardError::BootCounter`] when the flash fails or the record does not read back.
    pub fn count_boot(&mut self) -> Result<u32, BoardError> {
        let count = BootCounter::new(self.flash.raw())
            .count_boot()
            .map_err(BoardError::BootCounter)?;
        self.reset.set_boot_count(count);
        Ok(count)
    }

    /// The C6's firmware slots as the program-store shape the loop writes a firmware upload into
    /// (specification 34 TARGET-04).
    ///
    /// # Errors
    ///
    /// [`BoardError::ProgramSlots`] when their geometry cannot be used.
    pub fn firmware_slots(&self) -> Result<Box<dyn ProgramFlash>, BoardError> {
        let slots = FirmwareSlots::new(self.flash.raw()).map_err(BoardError::ProgramSlots)?;
        let slots: Box<dyn ProgramFlash> = Box::new(slots);
        Ok(slots)
    }

    /// What the loop asks of the C6 for a firmware update, and what the boot reads of its slots: the
    /// slot it runs from, found in its MMU, and the boot loader's selection (specification 34
    /// TARGET-04).
    #[must_use]
    pub fn firmware(&self) -> C6Firmware<SharedFlash<FlashStorage<'static>>> {
        C6Firmware::new(self.flash.raw(), running_slot())
    }

    /// The flash of the C6's two device settings sectors (specification 34 TARGET-02).
    #[must_use]
    pub fn settings_flash(&self) -> Box<dyn SettingsFlash> {
        Box::new(DeviceSectors::new(self.flash.raw()))
    }

    /// The SYSTIMER alarm the resource loop's wait arms; taken once, so the wait owns it.
    pub fn take_wait_alarm(&mut self) -> Option<Alarm<'static>> {
        self.wait_alarm.take()
    }

    /// A second handle on the checkpoint slots, which the resource loop only reads: the retained-value
    /// check of an install reads the newest checkpoint through it (specification 11 §6.7.1, decision
    /// G13), while the writer that [`BoardHardware`] hands out belongs to the installed program's
    /// cycle. It may be taken at any point after the board is taken.
    ///
    /// # Errors
    ///
    /// [`BoardError::Checkpoint`] when the slots' geometry cannot hold the checkpoint log.
    pub fn checkpoint_reader(&self) -> Result<Box<dyn CheckpointFlash>, BoardError> {
        let reader = self
            .flash
            .checkpoint_reader()
            .map_err(BoardError::Checkpoint)?;
        let reader: Box<dyn CheckpointFlash> = Box::new(reader);
        Ok(reader)
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

    /// Splits the board into what the resource loop owns and what each program claims its hardware
    /// from. The console and the reset control are dropped, which does not stop the USB-Serial/JTAG
    /// controller that the control link uses.
    #[must_use]
    pub fn into_parts(self) -> BoardParts {
        BoardParts {
            watchdog: self.watchdog,
            clock: self.clock,
            identity: self.identity,
            wait_alarm: self.wait_alarm,
            radio: self.radio,
            hardware: BoardHardware {
                gpio: self.gpio,
                flash: self.flash,
            },
        }
    }
}

/// What the resource loop owns after the boot has counted itself and read its program store.
pub struct BoardParts {
    /// MWDT0 supervisor: configured, and started by the firmware once the program is built.
    pub watchdog: Watchdog,
    /// SYSTIMER time, read by the resource loop's clock.
    pub clock: Monotonic,
    /// Identity captured at boot.
    pub identity: Identity,
    /// The SYSTIMER alarm the resource loop's wait arms at its deadline.
    pub wait_alarm: Option<Alarm<'static>>,
    /// What the Wi-Fi station takes (checklist L3).
    pub radio: Option<RadioParts>,
    /// What each program claims its lines and its checkpoint flash from.
    pub hardware: BoardHardware,
}

/// The hardware that each program claims from, kept for the life of the boot so that a program loaded in
/// place claims the pins and the checkpoint flash that the program before it released (specification 34
/// STATE-08): the GPIO bank, which lends a pin to one line at a time, and the flash owner, which hands
/// the checkpoint slots to one writer at a time.
pub struct BoardHardware {
    gpio: GpioBank,
    flash: FlashOwner<FlashStorage<'static>>,
}

impl ProgramHardware for BoardHardware {
    type Io = GpioIo;

    /// Claims the record's GPIO lines by pin name, in `io.toml` order, and drives each output to its
    /// initial level before it becomes an output (design §1.1, specification 34 STATE-01 and
    /// TARGET-01), recording its `[[io.safe_state]]` level for the terminal fault path (TARGET-08). A
    /// pin the chip lacks or keeps, or one named by two lines, is refused.
    fn claim_lines(&mut self, config: &Configuration) -> Result<GpioIo, String> {
        let (inputs, outputs) = record_lines(config, "C6")?;
        let (inputs, outputs) =
            claim_lines(&self.gpio, &inputs, &outputs, config).map_err(|error| {
                format!("the installed application's GPIO lines cannot be claimed: {error}")
            })?;
        Ok(bind_lines(inputs, outputs))
    }

    /// The chip's checkpoint flash (specification 34 §10.1), for one program at a time: it is free again
    /// when the writer of the program before it is dropped.
    fn checkpoint_flash(&mut self) -> Result<Box<dyn CheckpointFlash>, String> {
        let writer = self
            .flash
            .take_checkpoint()
            .map_err(|error| format!("the chip's checkpoint flash is not available: {error:?}"))?
            .ok_or_else(|| {
                String::from("the chip's checkpoint flash is not available: it is still in use")
            })?;
        Ok(Box::new(writer))
    }
}

/// The firmware slot the boot loader booted, from the MMU's first entry; slot A when the entry lies
/// in neither slot, which only a firmware flashed outside the slots would see.
fn running_slot() -> Slot {
    slot_at(image_window::running_firmware_address()).unwrap_or(Slot::A)
}

/// Resets the chip into the firmware the boot loader selects next (specification 34 TARGET-04,
/// TARGET-02). The reset record says that the reset was requested, so the next boot reports it as
/// `requested` and not as a loss of power (specification 11, the runtime device capabilities
/// contract).
pub(crate) fn reset_chip() {
    ResetControl::new().write_reset_record(ResetCause::Requested, "");
    esp_hal::system::software_reset();
}

fn read_identity(flash: &mut FlashStorage<'_>, running: Slot) -> Result<Identity, BoardError> {
    let mut header = [0u8; IMAGE_HEADER_BYTES];
    flash
        .read(slot_offset(running), &mut header)
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
