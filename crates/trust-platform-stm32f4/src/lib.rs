//! NUCLEO-F401RE platform services; no scheduler, compiler or VM implementation.
//!
//! Hardware is available only on bare-metal ARM. Host-visible arithmetic and
//! profiles describe the same timer, input polarity and reset-cookie contract.
#![no_std]
#![warn(missing_docs)]
#![deny(unsafe_op_in_unsafe_fn)]

mod console_budget;
pub use console_budget::ConsoleError;
mod profile;
mod time;
pub use profile::{InputSnapshot, ResetCookie, ResetReason, SCAN_PERIOD_US, SYSCLK_HZ};
pub use time::CounterExtension;

#[cfg(all(target_arch = "arm", target_os = "none"))]
mod hardware;
#[cfg(all(target_arch = "arm", target_os = "none"))]
pub use hardware::{
    emergency_halt, systick_interrupt, Board, BoardError, Console, DigitalIo, Identity, Monotonic,
    ResetControl, Watchdog,
};
