//! Pin names of the STM32U5A9. The grammar and the pin errors are the STM32 rules that the adapters
//! share (`trust-platform-stm32`); which pins the U5A9 has, and which the firmware keeps, is chip
//! data in [`chip`](crate::chip).

pub use trust_platform_stm32::pins::{
    Claims, Pin, PinError, PinNameError, Port, Pull, Reserved, Wiring, PINS_PER_PORT, PORT_COUNT,
};
