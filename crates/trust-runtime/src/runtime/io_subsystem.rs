//! I/O subsystem composition for the runtime.

use std::sync::{Arc, Mutex};

use smol_str::SmolStr;

use crate::error::RuntimeError;
use crate::io::{IoDriver, IoDriverHealth, IoDriverStatus, IoInterface, IoSafeState, IoSnapshot};

pub(super) struct IoSubsystem {
    interface: IoInterface,
    drivers: Vec<IoDriverEntry>,
    health_sink: Option<Arc<Mutex<Vec<IoDriverStatus>>>>,
    safe_state: IoSafeState,
}

pub(super) struct IoDriverEntry {
    pub(super) name: SmolStr,
    pub(super) driver: Box<dyn IoDriver>,
}

impl IoSubsystem {
    pub(super) fn new() -> Self {
        Self {
            interface: IoInterface::new(),
            drivers: Vec::new(),
            health_sink: None,
            safe_state: IoSafeState::default(),
        }
    }

    pub(super) fn interface(&self) -> &IoInterface {
        &self.interface
    }

    pub(super) fn interface_mut(&mut self) -> &mut IoInterface {
        &mut self.interface
    }

    pub(super) fn interface_and_drivers_mut(&mut self) -> (&mut IoInterface, &mut [IoDriverEntry]) {
        let Self {
            interface, drivers, ..
        } = self;
        (interface, drivers)
    }

    pub(super) fn try_resize(
        &mut self,
        inputs: usize,
        outputs: usize,
        memory: usize,
    ) -> Result<(), RuntimeError> {
        self.interface.try_resize(inputs, outputs, memory)
    }

    pub(super) fn add_driver(&mut self, name: impl Into<SmolStr>, driver: Box<dyn IoDriver>) {
        self.drivers.push(IoDriverEntry {
            name: name.into(),
            driver,
        });
    }

    pub(super) fn clear_drivers(&mut self) {
        self.drivers.clear();
    }

    pub(super) fn set_health_sink(&mut self, sink: Option<Arc<Mutex<Vec<IoDriverStatus>>>>) {
        self.health_sink = sink;
    }

    pub(super) fn update_health(&self) {
        let Some(sink) = &self.health_sink else {
            return;
        };
        if let Ok(mut guard) = sink.lock() {
            guard.clear();
            for entry in &self.drivers {
                guard.push(IoDriverStatus {
                    name: entry.name.clone(),
                    health: entry.driver.health(),
                });
            }
        }
    }

    pub(super) fn set_safe_state(&mut self, safe_state: IoSafeState) {
        self.safe_state = safe_state;
    }

    pub(super) fn apply_safe_state(&mut self) -> Result<(), RuntimeError> {
        if self.safe_state.is_empty() {
            self.update_health();
            return Ok(());
        }
        self.safe_state.apply(&mut self.interface)?;
        let mut first_failure = None;
        for entry in &mut self.drivers {
            let failure = match entry.driver.write_outputs(self.interface.outputs()) {
                Err(error) => Some(RuntimeError::IoDriver(
                    format!(
                        "safe-state output write failed for driver '{}': {error}",
                        entry.name
                    )
                    .into(),
                )),
                Ok(()) => match entry.driver.health() {
                    IoDriverHealth::Ok => None,
                    IoDriverHealth::Degraded { error } | IoDriverHealth::Faulted { error } => {
                        Some(RuntimeError::IoDriver(
                            format!(
                                "safe-state output unconfirmed for driver '{}': {error}",
                                entry.name
                            )
                            .into(),
                        ))
                    }
                },
            };
            if first_failure.is_none() {
                first_failure = failure;
            }
        }
        self.update_health();
        match first_failure {
            Some(error) => Err(error),
            None => Ok(()),
        }
    }

    pub(super) fn snapshot(&self) -> IoSnapshot {
        self.interface.snapshot()
    }
}
