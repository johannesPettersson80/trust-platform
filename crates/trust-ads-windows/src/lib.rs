//! Safe ownership boundary for Beckhoff's native Windows `TcAdsDll` API.
//!
//! The crate dynamically loads only trusted Windows installation locations and
//! keeps the library alive while any ADS port can call its exported functions.
//! Raw pointers and foreign calls are confined to the private FFI module;
//! consumers receive a safe, byte-oriented, RAII API.

#![deny(unsafe_op_in_unsafe_fn)]
#![warn(missing_docs)]

mod api;
mod error;
mod ffi;
#[cfg(any(windows, test))]
mod install_paths;
#[cfg(any(windows, test))]
mod native;
#[cfg(windows)]
mod windows_paths;

pub use api::{
    trusted_program_data_root, AdsDeviceState, AdsPort, AmsAddress, AmsNetId, ParseAmsNetIdError,
    TcAdsDll,
};
pub use error::AdsError;

#[cfg(test)]
mod tests;
