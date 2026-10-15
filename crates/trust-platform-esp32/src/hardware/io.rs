//! BOOT button on GPIO9 and the plain output on GPIO18, both read through their pads.
use crate::profile::InputSnapshot;
use esp_hal::{
    gpio::{Flex, Input, InputConfig, Level, Pull},
    peripherals::{GPIO18, GPIO9},
};

/// Pin owner for the physical one-bit process-image endpoint.
pub struct DigitalIo {
    button: Input<'static>,
    output: Flex<'static>,
}

impl DigitalIo {
    /// Drive GPIO18 low before its output stage is enabled, then enable its input and output.
    pub(super) fn new(button: GPIO9<'static>, output: GPIO18<'static>) -> Self {
        let button = Input::new(button, InputConfig::default().with_pull(Pull::Up));
        let mut output = Flex::new(output);
        output.set_level(Level::Low);
        output.set_input_enable(true);
        output.set_output_enable(true);
        Self { button, output }
    }

    /// Sample the physical BOOT level once; no synthetic test value is substituted.
    pub fn sample(&self) -> InputSnapshot {
        InputSnapshot::from_gpio9(self.button.is_high())
    }

    /// Publish the output bit: high drives GPIO18 high, low is the controlled safe state.
    pub fn set_output(&mut self, on: bool) {
        let level = if on { Level::High } else { Level::Low };
        self.output.set_level(level);
    }

    /// Force GPIO18 low without consulting runtime state.
    pub fn safe_off(&mut self) {
        self.output.set_low();
    }

    /// Read GPIO18 through its pad input, not the output latch. This proves an electrical pin
    /// level, not an observed actuator response.
    pub fn output_readback(&self) -> bool {
        self.output.is_high()
    }
}
