//! Thin local wrappers that give the STM32F401 adapter types the shared fixture
//! interfaces. The fixture traits and the adapter types are both foreign to this
//! crate, so each adapter type is wrapped in a local newtype.
use trust_board_fixture::board as fixture;
use trust_platform_stm32f4::{DigitalIo as Pins, Monotonic, Watchdog as HardwareWatchdog};

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

/// PC13 input and PA5 output, borrowed for one run.
pub struct Io<'a>(pub &'a mut Pins);

impl fixture::DigitalIo for Io<'_> {
    fn sample(&self) -> fixture::InputSnapshot {
        let physical = self.0.sample();
        fixture::InputSnapshot {
            pin_high: physical.pc13_high,
            pressed: physical.button_pressed,
        }
    }
    fn set_output(&mut self, on: bool) {
        self.0.set_output(on);
    }
    fn safe_off(&mut self) {
        self.0.safe_off();
    }
    fn output_readback(&self) -> bool {
        self.0.output_readback()
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
