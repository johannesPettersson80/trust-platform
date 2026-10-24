//! The process I/O of a compiled program on a computer (specification 44 §8.8): the drivers of
//! `io.toml` read the input image and drive the output image of the compiled cycle, in their order,
//! as they do for the bytecode engine (`io_subsystem.rs`), with the bytes past the program's own
//! image that the MQTT tags bind (§13.3) after it.

use std::sync::{Arc, Mutex};

use smol_str::SmolStr;
use trust_runtime_services::runner::engine::ProcessIo;

use super::super::io_subsystem::{publish_health, write_safe_outputs, IoDriverEntry};
use super::tags::{lock, SharedExtraImages};
use crate::error::RuntimeError;
use crate::io::{IoDriver, IoDriverStatus};

/// The `io.toml` drivers of a compiled program, in their order, and where their health is
/// published.
#[derive(Default)]
pub(crate) struct HostIo {
    drivers: Vec<IoDriverEntry>,
    health_sink: Option<Arc<Mutex<Vec<IoDriverStatus>>>>,
    /// The bytes past the program's own image that the MQTT tags bind.
    extra: SharedExtraImages,
    /// The program's image with the extra bytes after it, as the drivers see the image.
    joined: Vec<u8>,
}

impl HostIo {
    /// Adds the driver `driver` named `name` after the others.
    pub(crate) fn add_driver(&mut self, name: impl Into<SmolStr>, driver: Box<dyn IoDriver>) {
        self.drivers.push(IoDriverEntry {
            name: name.into(),
            driver,
        });
    }

    /// Stops using every driver.
    pub(crate) fn clear_drivers(&mut self) {
        self.drivers.clear();
    }

    /// Publishes the drivers' health into `sink` from now on.
    pub(crate) fn set_health_sink(&mut self, sink: Option<Arc<Mutex<Vec<IoDriverStatus>>>>) {
        self.health_sink = sink;
    }

    /// Publishes the drivers' health without waiting for a reader (specification 34 HOST-05).
    pub(crate) fn update_health(&self) {
        publish_health(&self.drivers, self.health_sink.as_ref());
    }

    /// The bytes past the program's own image, which the boundaries of the scan share.
    pub(crate) fn extra_images(&self) -> SharedExtraImages {
        Arc::clone(&self.extra)
    }

    /// `image` with the extra `bytes` after it, in the buffer the drivers see.
    fn join(&mut self, image: &[u8], bytes: &[u8]) {
        self.joined.clear();
        self.joined.extend_from_slice(image);
        self.joined.extend_from_slice(bytes);
    }
}

impl ProcessIo for HostIo {
    /// Each driver reads its inputs into the image, and the extra input bytes after it, in turn;
    /// the first failure ends the read.
    fn read_inputs(&mut self, image: &mut [u8]) -> Result<(), RuntimeError> {
        let extra = Arc::clone(&self.extra);
        let mut extra = lock(&extra);
        if extra.inputs.is_empty() {
            return self
                .drivers
                .iter_mut()
                .try_for_each(|entry| entry.driver.read_inputs(image));
        }
        self.join(image, &extra.inputs);
        let result = self
            .drivers
            .iter_mut()
            .try_for_each(|entry| entry.driver.read_inputs(&mut self.joined));
        let (own, past) = self.joined.split_at(image.len());
        image.copy_from_slice(own);
        extra.inputs.copy_from_slice(past);
        result
    }

    /// Each driver drives its outputs from the image, and the extra output bytes after it, in
    /// turn; the first failure ends the write.
    fn write_outputs(&mut self, image: &[u8]) -> Result<(), RuntimeError> {
        let extra = Arc::clone(&self.extra);
        let extra = lock(&extra);
        let result = if extra.outputs.is_empty() {
            self.drivers
                .iter_mut()
                .try_for_each(|entry| entry.driver.write_outputs(image))
        } else {
            self.join(image, &extra.outputs);
            self.drivers
                .iter_mut()
                .try_for_each(|entry| entry.driver.write_outputs(&self.joined))
        };
        drop(extra);
        self.update_health();
        result
    }

    /// Every driver is attempted with the safe image and the extra output bytes after it, and the
    /// first failure is reported.
    fn write_safe_outputs(&mut self, image: &[u8]) -> Result<(), RuntimeError> {
        let extra = Arc::clone(&self.extra);
        let extra = lock(&extra);
        let result = if extra.outputs.is_empty() {
            write_safe_outputs(&mut self.drivers, image)
        } else {
            self.join(image, &extra.outputs);
            write_safe_outputs(&mut self.drivers, &self.joined)
        };
        drop(extra);
        self.update_health();
        result
    }

    fn driver_health(&self) -> Vec<IoDriverStatus> {
        self.drivers
            .iter()
            .map(|entry| IoDriverStatus {
                name: entry.name.clone(),
                health: entry.driver.health(),
            })
            .collect()
    }
}
