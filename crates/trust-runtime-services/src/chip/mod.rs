//! The runtime every chip shares (specification 34, ARCH-11 and §3.3): the platform the resource loop
//! runs over ([`platform`]), the GPIO process image of the installed program's lines ([`process_io`])
//! and the hardware watchdog's timeout rule ([`watchdog`]). A chip supplies only its own peripheral
//! drivers, its identity and its profile; the rules it runs by are written here once.

pub mod platform;
pub mod process_io;
pub mod watchdog;
