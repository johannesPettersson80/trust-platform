//! Pin names of the STM32F4 family. The grammar and the pin errors are the shared STM32 rules of
//! `trust-platform-stm32`; which pins the F407 has, and which it keeps, is chip data in
//! [`chip`](crate::chip).

pub(crate) use trust_platform_stm32::pins::at;
pub use trust_platform_stm32::pins::{
    Claims, Pin, PinError, PinNameError, Port, Pull, Reserved, Wiring, PINS_PER_PORT, PORT_COUNT,
};
