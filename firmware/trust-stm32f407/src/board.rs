//! The bench wiring and the thin wrappers that give the STM32F407 adapter types the shared
//! fixture interfaces.
//!
//! This firmware is for the chip, not for a board (specification 34, ARCH-10). What a board
//! wires is configuration: [`WIRING`] names the pins in the notation of `io.toml`, and the two
//! levels below say what a pressed button and a lit LED look like on the pins. The bench is the
//! STM32F407G-DISC1; another board with the F407 changes only this block.
use trust_board_fixture::board as fixture;
use trust_platform_stm32f4::{
    DigitalIo as Pins, Input, Monotonic, Output, Process, Pull, Watchdog as HardwareWatchdog,
    Wiring, CHIP,
};

/// STM32F407G-DISC1: B1 (USER) connects PA0 to VDD, so the pin reads high while it is pressed.
const BUTTON_PRESSED_HIGH: bool = true;
/// STM32F407G-DISC1: LD4 (green) lights when PD12 is high.
const LED_ON_HIGH: bool = true;

/// The pins of the process image's one input bit and one output bit on the STM32F407G-DISC1.
/// B1 has a pull-down on the board; the internal one adds nothing but keeps the input defined
/// without it. The output is safe when the LED is off.
pub const WIRING: Wiring<'static> = Wiring {
    input: "PA0",
    input_pull: Pull::Down,
    output: "PD12",
    output_safe_high: !LED_ON_HIGH,
};

// A line that is no pin, a pin the chip lacks or the firmware keeps, or one pin on both lines
// fails the build here instead of the boot, where the console is not up yet to say why.
const _: () = assert!(CHIP.check_wiring(&WIRING).is_ok());

/// TIM2 monotonic time and the cycle counter, borrowed for one run.
pub struct Clock<'a>(pub &'a Monotonic);

impl fixture::Clock for Clock<'_> {
    fn micros(&self) -> u64 {
        self.0.micros()
    }
    fn wait_until_us(&self, deadline_us: u64) {
        self.0.wait_until_us(deadline_us);
    }
    fn cycle_count(&self) -> u32 {
        self.0.cycle_count()
    }
    fn irq_count(&self) -> u32 {
        self.0.irq_count()
    }
}

/// The wired input and output, borrowed for one run.
pub struct Io<'a> {
    pins: &'a mut Pins,
    input: Input,
    output: Output,
}

impl<'a> Io<'a> {
    /// The pins the adapter claimed for the wiring.
    pub fn new(pins: &'a mut Pins, process: Process) -> Self {
        Self {
            pins,
            input: process.input,
            output: process.output,
        }
    }
}

impl fixture::DigitalIo for Io<'_> {
    fn sample(&self) -> fixture::InputSnapshot {
        let pin_high = self.pins.read(self.input);
        fixture::InputSnapshot {
            pin_high,
            pressed: pin_high == BUTTON_PRESSED_HIGH,
        }
    }
    fn set_output(&mut self, on: bool) {
        self.pins.drive(self.output, on == LED_ON_HIGH);
    }
    fn safe_off(&mut self) {
        self.pins.drive_safe(self.output);
    }
    fn output_readback(&self) -> bool {
        self.pins.readback(self.output) == LED_ON_HIGH
    }
}

/// Independent watchdog, borrowed for one run. The fixture feeds it; the board
/// starts it in the supervision step that `main` passes to the runner.
pub struct Watchdog<'a>(pub &'a mut HardwareWatchdog);

impl fixture::Watchdog for Watchdog<'_> {
    fn feed(&mut self) {
        self.0.feed();
    }
}
