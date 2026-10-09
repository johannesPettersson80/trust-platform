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
    /// The name of its link, for a driver of `io.toml` (specification 42, §2).
    pub(super) link: Option<SmolStr>,
    pub(super) driver: Box<dyn IoDriver>,
    /// Switched off from the program (specification 42, §4 item 6): neither read nor written.
    pub(super) enabled: bool,
    /// How long its last read and its last write took (§4).
    pub(super) read_time: std::time::Duration,
    pub(super) write_time: std::time::Duration,
}

impl IoDriverEntry {
    fn new(name: SmolStr, link: Option<SmolStr>, driver: Box<dyn IoDriver>) -> Self {
        Self {
            name,
            link,
            driver,
            enabled: true,
            read_time: std::time::Duration::ZERO,
            write_time: std::time::Duration::ZERO,
        }
    }
}

/// A linked driver as its link reports it this scan (specification 42, §3 item 1, §4).
pub(crate) struct LinkIo<'a> {
    pub(crate) link: &'a SmolStr,
    pub(crate) health: IoDriverHealth,
    pub(crate) enabled: bool,
    /// Its last read and write together.
    pub(crate) response: std::time::Duration,
    /// The protocol's own state in words, when the driver gives it.
    pub(crate) detail: Option<SmolStr>,
    /// The bytes of the input image it fills (specification 42, §5 item 4).
    pub(crate) input_bytes: Vec<std::ops::Range<usize>>,
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

    pub(super) fn all_drivers_simulated(&self) -> bool {
        self.drivers.iter().all(|entry| entry.driver.is_simulated())
    }

    pub(super) fn add_driver(&mut self, name: impl Into<SmolStr>, driver: Box<dyn IoDriver>) {
        self.drivers
            .push(IoDriverEntry::new(name.into(), None, driver));
    }

    pub(super) fn add_driver_with_link(
        &mut self,
        name: impl Into<SmolStr>,
        link: impl Into<SmolStr>,
        driver: Box<dyn IoDriver>,
    ) {
        self.drivers
            .push(IoDriverEntry::new(name.into(), Some(link.into()), driver));
    }

    /// Each linked driver as its link reports it.
    pub(super) fn link_ios(&self) -> impl Iterator<Item = LinkIo<'_>> + '_ {
        self.drivers.iter().filter_map(|entry| {
            entry.link.as_ref().map(|link| LinkIo {
                link,
                health: entry.driver.health(),
                enabled: entry.enabled,
                response: entry.read_time + entry.write_time,
                detail: entry.driver.detail(),
                input_bytes: entry.driver.input_bytes(),
            })
        })
    }

    /// Switches a linked driver off or on (specification 42, §4 item 6).
    pub(super) fn set_link_enabled(&mut self, link: &str, enabled: bool) {
        for entry in &mut self.drivers {
            if entry.link.as_deref() == Some(link) {
                entry.enabled = enabled;
            }
        }
    }

    /// Tests a linked driver (§4 item 7): one read into a spare image, which the process image
    /// does not take; nothing when no driver has that link.
    pub(super) fn test_link(&mut self, link: &str) -> Option<Result<std::time::Duration, String>> {
        let size = self.interface.inputs().len();
        let entry = self
            .drivers
            .iter_mut()
            .find(|entry| entry.link.as_deref() == Some(link))?;
        let mut spare = vec![0u8; size];
        let started = std::time::Instant::now();
        Some(
            entry
                .driver
                .read_inputs(&mut spare)
                .map(|()| started.elapsed())
                .map_err(|error| error.to_string()),
        )
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

#[cfg(test)]
mod tests {
    use super::*;

    use crate::io::IoAddress;

    #[test]
    fn io_subsystem_health_snapshot_replaces_stale_entries_in_driver_order() {
        let mut io = IoSubsystem::new();
        let sink = Arc::new(Mutex::new(vec![IoDriverStatus {
            name: "Stale".into(),
            health: IoDriverHealth::Faulted {
                error: "old".into(),
            },
        }]));
        io.add_driver(
            "First",
            Box::new(FakeDriver::new(
                IoDriverHealth::Degraded {
                    error: "warming".into(),
                },
                false,
            )),
        );
        io.add_driver(
            "Second",
            Box::new(FakeDriver::new(IoDriverHealth::Ok, false)),
        );
        io.set_health_sink(Some(sink.clone()));

        io.update_health();

        let health = sink.lock().unwrap();
        assert_eq!(health.len(), 2);
        assert_eq!(health[0].name, "First");
        assert_eq!(
            health[0].health,
            IoDriverHealth::Degraded {
                error: "warming".into()
            }
        );
        assert_eq!(health[1].name, "Second");
        assert_eq!(health[1].health, IoDriverHealth::Ok);
    }

    #[test]
    fn io_subsystem_safe_state_attempts_every_driver_and_returns_first_failure() {
        let mut io = IoSubsystem::new();
        io.try_resize(0, 1, 0).unwrap();
        let first_writes = Arc::new(Mutex::new(Vec::new()));
        let second_writes = Arc::new(Mutex::new(Vec::new()));
        io.add_driver(
            "First",
            Box::new(FakeDriver {
                health: IoDriverHealth::Ok,
                fail_write: true,
                writes: first_writes.clone(),
            }),
        );
        io.add_driver(
            "Second",
            Box::new(FakeDriver {
                health: IoDriverHealth::Ok,
                fail_write: false,
                writes: second_writes.clone(),
            }),
        );
        io.set_safe_state(IoSafeState {
            outputs: vec![(
                IoAddress::parse("%QX0.2").unwrap(),
                crate::value::Value::Bool(true),
            )],
        });

        let error = io
            .apply_safe_state()
            .expect_err("first driver failure must be reported");

        assert!(error.to_string().contains("driver 'First'"), "{error}");
        assert_eq!(io.interface().outputs(), &[0b0000_0100]);
        assert_eq!(first_writes.lock().unwrap().as_slice(), &[vec![4]]);
        assert_eq!(second_writes.lock().unwrap().as_slice(), &[vec![4]]);
    }

    #[test]
    fn io_subsystem_empty_safe_state_only_refreshes_health() {
        let mut io = IoSubsystem::new();
        let writes = Arc::new(Mutex::new(Vec::new()));
        let sink = Arc::new(Mutex::new(Vec::new()));
        io.add_driver(
            "Driver",
            Box::new(FakeDriver {
                health: IoDriverHealth::Ok,
                fail_write: false,
                writes: writes.clone(),
            }),
        );
        io.set_health_sink(Some(sink.clone()));

        io.apply_safe_state().unwrap();

        assert!(writes.lock().unwrap().is_empty());
        assert_eq!(sink.lock().unwrap().len(), 1);
    }

    struct FakeDriver {
        health: IoDriverHealth,
        fail_write: bool,
        writes: Arc<Mutex<Vec<Vec<u8>>>>,
    }

    impl FakeDriver {
        fn new(health: IoDriverHealth, fail_write: bool) -> Self {
            Self {
                health,
                fail_write,
                writes: Arc::new(Mutex::new(Vec::new())),
            }
        }
    }

    impl IoDriver for FakeDriver {
        fn read_inputs(&mut self, _inputs: &mut [u8]) -> Result<(), RuntimeError> {
            Ok(())
        }

        fn write_outputs(&mut self, outputs: &[u8]) -> Result<(), RuntimeError> {
            self.writes.lock().unwrap().push(outputs.to_vec());
            if self.fail_write {
                Err(RuntimeError::IoDriver("write failed".into()))
            } else {
                Ok(())
            }
        }

        fn health(&self) -> IoDriverHealth {
            self.health.clone()
        }
    }
}
