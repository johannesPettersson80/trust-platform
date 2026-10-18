//! Shared truST runtime services (specification 34, ARCH-11 and ARCH-14): the runtime components
//! above the engine that every target runs, built without an operating system.
//!
//! [`protocol`] holds the control connection's request and response envelope, [`access`] its
//! access rights and [`dispatch`] the request pipeline. [`status`], [`events`], [`metrics`] and
//! [`resource`] answer the status, event and resource operations and hold the supervisor's
//! decisions. [`live_values`] holds the queues of pending writes and active forces that a control
//! surface applies at the next cycle boundary. [`ports`] names what a platform supplies.
//! [`runner`] is the single-thread resource loop of a device without an operating system, and
//! [`link`] the framing and reply handling of its control link over a byte stream.
//! [`installation`] checks the `TRSTA001` container a chip holds its installed program in before
//! any payload is exposed. Transports, threads, files, clocks and host state stay in the platform
//! that composes these services.

#![no_std]
#![forbid(unsafe_code)]
#![warn(missing_docs)]
#![allow(clippy::module_name_repetitions)]

extern crate alloc;

pub mod access;
pub mod dispatch;
pub mod events;
pub mod execution_backend;
pub mod installation;
pub mod link;
pub mod live_values;
pub mod metrics;
pub mod ports;
pub mod protocol;
pub mod resource;
pub mod runner;
pub mod status;

#[cfg(test)]
mod tests;
