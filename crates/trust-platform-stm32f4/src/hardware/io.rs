//! Stock MB1136 PC13 button and PA5 green LED, sampled through IDR.
use crate::InputSnapshot;
use stm32f4xx_hal::gpio::{gpioa::PA5, gpioc::PC13, Input, Output, PushPull};

/// Pin owner for the physical one-bit process-image endpoint.
pub struct DigitalIo {
    led: PA5<Output<PushPull>>,
    button: PC13<Input>,
}
impl DigitalIo {
    pub(super) fn new(led: PA5<Output<PushPull>>, button: PC13<Input>) -> Self {
        Self { led, button }
    }
    /// Sample the physical pin once; no synthetic test value is substituted.
    pub fn sample(&self) -> InputSnapshot {
        InputSnapshot::from_pc13(self.button.is_high())
    }
    /// Publish the output bit: high lights LD2, low is the controlled safe state.
    pub fn set_output(&mut self, on: bool) {
        if on {
            self.led.set_high();
        } else {
            self.led.set_low();
        }
    }
    /// Force the physical output low without consulting runtime state.
    pub fn safe_off(&mut self) {
        self.led.set_low();
    }
    /// Read PA5 IDR, not the output latch; proves an electrical MCU pin level,
    /// not an observed LED or external actuator response.
    pub fn output_readback(&self) -> bool {
        // SAFETY: this object owns PA5 for its lifetime. The PAC register block
        // uses volatile interior-mutability access; this shared read touches only
        // GPIOA.IDR and cannot reconfigure or race a write to another pin.
        unsafe { &*stm32f4::stm32f401::GPIOA::ptr() }
            .idr()
            .read()
            .idr5()
            .bit_is_set()
    }
}
