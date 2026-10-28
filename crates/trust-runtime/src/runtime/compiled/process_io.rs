//! The process I/O of a compiled program on a computer (specification 44 §8.8): the drivers of
//! `io.toml` read the input image and drive the output image of the compiled cycle, in their order,
//! as they do for the bytecode engine (`io_subsystem.rs`), with the bytes past the program's own
//! image that the MQTT tags bind (§13.3) after it. Each driver of `io.toml` runs its link
//! (specification 42, §2): a link the program switched off is neither read nor written, and each
//! read and write is timed (§4 items 6 and 8). The `Comm.Link` blocks reach the drivers at the
//! access point after the inputs (§4 item 1), while the cycle holds the process I/O, through the
//! drivers' shared list.

use std::sync::{Arc, Mutex, MutexGuard};

use smol_str::SmolStr;
use trust_runtime_services::runner::ProcessIo;

use super::super::io_subsystem::{self, publish_health, write_safe_outputs, IoDriverEntry};
use super::tags::{lock, SharedExtraImages};
use crate::error::RuntimeError;
use crate::io::{IdentifyHandle, ImageBit, IoDriver, IoDriverStatus};

/// The drivers of `io.toml`, in their order, which the process I/O reads and drives and the
/// `Comm.Link` blocks switch off and test at the access point after the inputs (specification 42,
/// §4 items 6 and 7); only the scan thread takes them.
pub(super) type SharedDrivers = Arc<Mutex<Vec<IoDriverEntry>>>;

/// The drivers, also after a thread that held them panicked: each entry stays a whole driver.
pub(super) fn lock_drivers(
    drivers: &Mutex<Vec<IoDriverEntry>>,
) -> MutexGuard<'_, Vec<IoDriverEntry>> {
    drivers
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// The `io.toml` drivers of a compiled program, in their order, and where their health is
/// published.
#[derive(Default)]
pub(crate) struct HostIo {
    drivers: SharedDrivers,
    health_sink: Option<Arc<Mutex<Vec<IoDriverStatus>>>>,
    /// The bytes past the program's own image that the MQTT tags bind.
    extra: SharedExtraImages,
    /// The program's image with the extra bytes after it, as the drivers see the image.
    joined: Vec<u8>,
    /// The identify state the runtime shares with its control state (specification 11,
    /// "Identify"): its blink is set in the copy of the output image the drivers get.
    identify: IdentifyHandle,
    /// The output image with the identify bit set, as the drivers get it while the bit is on.
    with_identify: Vec<u8>,
}

impl HostIo {
    /// Adds the driver `driver` named `name` after the others, a driver without a link.
    pub(crate) fn add_driver(&mut self, name: impl Into<SmolStr>, driver: Box<dyn IoDriver>) {
        lock_drivers(&self.drivers).push(IoDriverEntry::new(name.into(), None, driver));
    }

    /// Adds the `io.toml` driver `driver` named `name`, which runs the link `link` (specification
    /// 42, §2), after the others, switched on.
    pub(crate) fn add_driver_with_link(
        &mut self,
        name: impl Into<SmolStr>,
        link: impl Into<SmolStr>,
        driver: Box<dyn IoDriver>,
    ) {
        lock_drivers(&self.drivers).push(IoDriverEntry::new(
            name.into(),
            Some(link.into()),
            driver,
        ));
    }

    /// Stops using every driver.
    pub(crate) fn clear_drivers(&mut self) {
        lock_drivers(&self.drivers).clear();
    }

    /// Publishes the drivers' health into `sink` from now on.
    pub(crate) fn set_health_sink(&mut self, sink: Option<Arc<Mutex<Vec<IoDriverStatus>>>>) {
        self.health_sink = sink;
    }

    /// Publishes the drivers' health without waiting for a reader (specification 34 HOST-05).
    pub(crate) fn update_health(&self) {
        publish_health(&lock_drivers(&self.drivers), self.health_sink.as_ref());
    }

    /// The bytes past the program's own image, which the boundaries of the scan share.
    pub(crate) fn extra_images(&self) -> SharedExtraImages {
        Arc::clone(&self.extra)
    }

    /// The drivers' list, which the access point after the inputs shares while the cycle holds
    /// the process I/O.
    pub(super) fn shared_drivers(&self) -> SharedDrivers {
        Arc::clone(&self.drivers)
    }

    /// Whether every driver is an in-process simulator with no physical I/O side effects, as no
    /// driver at all is: the I/O is simulated when all attached I/O is (specification 35, A2.4).
    pub(crate) fn all_drivers_simulated(&self) -> bool {
        io_subsystem::all_drivers_simulated(&lock_drivers(&self.drivers))
    }

    /// `image` with the extra `bytes` after it, in the buffer the drivers see.
    fn join(&mut self, image: &[u8], bytes: &[u8]) {
        self.joined.clear();
        self.joined.extend_from_slice(image);
        self.joined.extend_from_slice(bytes);
    }
}

impl ProcessIo for HostIo {
    /// Each driver whose link is not switched off reads its inputs into the image, and the extra
    /// input bytes after it, in turn, timed; the first failure ends the read.
    fn read_inputs(&mut self, image: &mut [u8]) -> Result<(), RuntimeError> {
        let drivers = Arc::clone(&self.drivers);
        let mut drivers = lock_drivers(&drivers);
        let extra = Arc::clone(&self.extra);
        let mut extra = lock(&extra);
        if extra.inputs.is_empty() {
            return io_subsystem::read_inputs(&mut drivers, image);
        }
        self.join(image, &extra.inputs);
        let result = io_subsystem::read_inputs(&mut drivers, &mut self.joined);
        let (own, past) = self.joined.split_at(image.len());
        image.copy_from_slice(own);
        extra.inputs.copy_from_slice(past);
        result
    }

    /// Each driver whose link is not switched off drives its outputs from the image, and the
    /// extra output bytes after it, in turn, timed; the first failure ends the write. While the
    /// blink of identify has its image bit on, the drivers get a copy of the image with that bit
    /// set; the program's own image never holds it (specification 11, "Identify").
    fn write_outputs(&mut self, image: &[u8]) -> Result<(), RuntimeError> {
        if let Some(bit) = self.identify.publication() {
            let mut with_identify = std::mem::take(&mut self.with_identify);
            with_identify.clear();
            with_identify.extend_from_slice(image);
            bit.set_bit(&mut with_identify);
            let result = self.write_image(&with_identify);
            self.with_identify = with_identify;
            return result;
        }
        self.write_image(image)
    }

    /// Every driver, a switched-off link's too, is attempted with the safe image and the extra
    /// output bytes after it, and the first failure is reported.
    fn write_safe_outputs(&mut self, image: &[u8]) -> Result<(), RuntimeError> {
        self.write_safe_image(image)
    }

    fn driver_health(&self) -> Vec<IoDriverStatus> {
        lock_drivers(&self.drivers)
            .iter()
            .map(|entry| IoDriverStatus {
                name: entry.name.clone(),
                health: entry.driver.health(),
            })
            .collect()
    }
}

impl HostIo {
    /// Gives the process I/O the identify state the runtime shares with its control state.
    pub(crate) fn set_identify(&mut self, identify: IdentifyHandle) {
        self.identify = identify;
    }

    /// Hands the committed output image `image` to every driver once more, with the identify `bit`
    /// set when there is one, as identify does at an edge of its blink while no scan runs, and
    /// without it when identify ends (specification 11, "Identify"). A failure is the driver's own
    /// health, as for every output; each driver is attempted.
    pub(crate) fn republish_outputs(&mut self, image: &[u8], bit: Option<ImageBit>) {
        let mut outputs = image.to_vec();
        if let Some(bit) = bit {
            bit.set_bit(&mut outputs);
        }
        if let Err(error) = self.write_safe_image(&outputs) {
            tracing::warn!("a driver did not take the output image of identify: {error}");
        }
    }

    /// Each driver whose link is not switched off drives `image` and the extra output bytes after
    /// it, in turn, timed; the first failure ends the write.
    fn write_image(&mut self, image: &[u8]) -> Result<(), RuntimeError> {
        let result = {
            let drivers = Arc::clone(&self.drivers);
            let mut drivers = lock_drivers(&drivers);
            let extra = Arc::clone(&self.extra);
            let extra = lock(&extra);
            if extra.outputs.is_empty() {
                io_subsystem::write_outputs(&mut drivers, image)
            } else {
                self.join(image, &extra.outputs);
                io_subsystem::write_outputs(&mut drivers, &self.joined)
            }
        };
        self.update_health();
        result
    }

    /// Every driver, a switched-off link's too, is attempted with `image` and the extra output
    /// bytes after it, and the first failure is reported.
    fn write_safe_image(&mut self, image: &[u8]) -> Result<(), RuntimeError> {
        let result = {
            let drivers = Arc::clone(&self.drivers);
            let mut drivers = lock_drivers(&drivers);
            let extra = Arc::clone(&self.extra);
            let extra = lock(&extra);
            if extra.outputs.is_empty() {
                write_safe_outputs(&mut drivers, image)
            } else {
                self.join(image, &extra.outputs);
                write_safe_outputs(&mut drivers, &self.joined)
            }
        };
        self.update_health();
        result
    }
}
