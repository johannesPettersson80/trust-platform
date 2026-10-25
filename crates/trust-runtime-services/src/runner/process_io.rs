//! The drivers of a device's process image (specification 34, §7.2, PORT-04 and STATE-04): the
//! port that a platform adapter implements over the I/O the deployment configures, for example
//! the `gpio` lines of `io.toml`.

use alloc::vec::Vec;

use trust_runtime_core::error::RuntimeError;

use crate::status::IoDriverStatus;

/// The drivers of the installed program's process image, as the program's cycle calls them.
///
/// The images are the program's flat `%I` and `%Q` images. A driver never substitutes a value
/// for an input it cannot read: it fails, and the scan fails with it (§7.2). What a successful
/// write establishes is the adapter's to state (PORT-04); a write to a GPIO line establishes
/// driver acceptance.
pub trait ProcessIo {
    /// Reads every input into `image`, the input image, before the program samples it into its
    /// variables.
    ///
    /// # Errors
    ///
    /// The [`RuntimeError`] of an input that cannot be read. The scan fails without running the
    /// program, and the fault policy follows.
    fn read_inputs(&mut self, image: &mut [u8]) -> Result<(), RuntimeError>;

    /// Drives the outputs from `image`, the output image of a scan, after the program has run and
    /// the scan's deadline has been checked.
    ///
    /// # Errors
    ///
    /// The [`RuntimeError`] of an output that cannot be driven. The scan fails; an output driven
    /// before the failure keeps its new value (§8.2).
    fn write_outputs(&mut self, image: &[u8]) -> Result<(), RuntimeError>;

    /// Drives the outputs from `image` after a stop or a fault, when the output image holds the
    /// safe values of `[[io.safe_state]]` (STATE-02). Every driver is attempted even when one
    /// fails (STATE-04).
    ///
    /// # Errors
    ///
    /// The first failure among the drivers.
    fn write_safe_outputs(&mut self, image: &[u8]) -> Result<(), RuntimeError>;

    /// The health of each driver, in `io.toml` order, as `status` and `health` report it.
    fn driver_health(&self) -> Vec<IoDriverStatus>;
}
