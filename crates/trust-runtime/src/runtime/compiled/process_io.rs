//! The process I/O of a compiled program on a computer (specification 44 §8.8): the drivers of
//! `io.toml` read the input image and drive the output image of the compiled cycle, in their order,
//! as they do for the bytecode engine (`io_subsystem.rs`).

use std::sync::{Arc, Mutex};

use smol_str::SmolStr;
use trust_runtime_services::runner::engine::ProcessIo;

use super::super::io_subsystem::{publish_health, write_safe_outputs, IoDriverEntry};
use crate::error::RuntimeError;
use crate::io::{IoDriver, IoDriverStatus};

/// The `io.toml` drivers of a compiled program, in their order, and where their health is
/// published.
#[derive(Default)]
pub(crate) struct HostIo {
    drivers: Vec<IoDriverEntry>,
    health_sink: Option<Arc<Mutex<Vec<IoDriverStatus>>>>,
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
}

impl ProcessIo for HostIo {
    /// Each driver reads its inputs into the image in turn; the first failure ends the read.
    fn read_inputs(&mut self, image: &mut [u8]) -> Result<(), RuntimeError> {
        for entry in &mut self.drivers {
            entry.driver.read_inputs(image)?;
        }
        Ok(())
    }

    /// Each driver drives its outputs from the image in turn; the first failure ends the write.
    fn write_outputs(&mut self, image: &[u8]) -> Result<(), RuntimeError> {
        let result = self
            .drivers
            .iter_mut()
            .try_for_each(|entry| entry.driver.write_outputs(image));
        self.update_health();
        result
    }

    /// Every driver is attempted, and the first failure is reported.
    fn write_safe_outputs(&mut self, image: &[u8]) -> Result<(), RuntimeError> {
        let result = write_safe_outputs(&mut self.drivers, image);
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
