//! The I/O drivers of a computer's program (specification 44 §8.8): their health and the safe
//! outputs of a stop or a fault.

use std::sync::{Arc, Mutex};

use smol_str::SmolStr;

use crate::error::RuntimeError;
use crate::io::{IoDriver, IoDriverHealth, IoDriverStatus};

pub(super) struct IoDriverEntry {
    pub(super) name: SmolStr,
    pub(super) driver: Box<dyn IoDriver>,
}

/// Publishes the health of `drivers` into `sink` without waiting for a reader (spec 34 HOST-05).
/// While a reader holds the sink nothing is published; the next call that acquires it publishes the
/// health current then, so readers may see a snapshot one cycle late.
pub(super) fn publish_health(
    drivers: &[IoDriverEntry],
    sink: Option<&Arc<Mutex<Vec<IoDriverStatus>>>>,
) {
    let Some(sink) = sink else {
        return;
    };
    let Ok(mut guard) = sink.try_lock() else {
        return;
    };
    guard.clear();
    for entry in drivers {
        guard.push(IoDriverStatus {
            name: entry.name.clone(),
            health: entry.driver.health(),
        });
    }
}

/// Drives every driver's outputs from `outputs`, the safe output image, attempting all of them and
/// reporting the first failure.
pub(super) fn write_safe_outputs(
    drivers: &mut [IoDriverEntry],
    outputs: &[u8],
) -> Result<(), RuntimeError> {
    let mut first_failure = None;
    for entry in drivers {
        let failure = match entry.driver.write_outputs(outputs) {
            Err(error) => Some(RuntimeError::IoDriver(
                format!(
                    "safe-state output write failed for driver '{}': {error}",
                    entry.name
                )
                .into(),
            )),
            Ok(()) => match entry.driver.output_health() {
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
    match first_failure {
        Some(error) => Err(error),
        None => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(name: &str, driver: impl IoDriver + 'static) -> IoDriverEntry {
        IoDriverEntry {
            name: name.into(),
            driver: Box::new(driver),
        }
    }

    /// The safe output image of `%QX0.2 := TRUE` in a one-byte output image: bit 2 set.
    const SAFE_IMAGE: [u8; 1] = [0b0000_0100];

    #[test]
    fn io_subsystem_health_snapshot_replaces_stale_entries_in_driver_order() {
        let sink = Arc::new(Mutex::new(vec![IoDriverStatus {
            name: "Stale".into(),
            health: IoDriverHealth::Faulted {
                error: "old".into(),
            },
        }]));
        let drivers = vec![
            entry(
                "First",
                FakeDriver::new(
                    IoDriverHealth::Degraded {
                        error: "warming".into(),
                    },
                    false,
                ),
            ),
            entry("Second", FakeDriver::new(IoDriverHealth::Ok, false)),
        ];

        publish_health(&drivers, Some(&sink));

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
        let first_writes = Arc::new(Mutex::new(Vec::new()));
        let second_writes = Arc::new(Mutex::new(Vec::new()));
        let mut drivers = vec![
            entry(
                "First",
                FakeDriver {
                    health: IoDriverHealth::Ok,
                    fail_write: true,
                    writes: first_writes.clone(),
                },
            ),
            entry(
                "Second",
                FakeDriver {
                    health: IoDriverHealth::Ok,
                    fail_write: false,
                    writes: second_writes.clone(),
                },
            ),
        ];

        let error = write_safe_outputs(&mut drivers, &SAFE_IMAGE)
            .expect_err("first driver failure must be reported");

        assert!(error.to_string().contains("driver 'First'"), "{error}");
        assert_eq!(first_writes.lock().unwrap().as_slice(), &[vec![4]]);
        assert_eq!(second_writes.lock().unwrap().as_slice(), &[vec![4]]);
    }

    #[test]
    fn safe_state_is_confirmed_by_output_path_health_when_reported_health_is_degraded() {
        let writes = Arc::new(Mutex::new(Vec::new()));
        let sink: Arc<Mutex<Vec<IoDriverStatus>>> = Arc::new(Mutex::new(Vec::new()));
        let overflow = IoDriverHealth::Degraded {
            error: "incoming queue overflow: 1 messages dropped".into(),
        };
        let mut drivers = vec![entry(
            "Mqtt",
            SplitHealthDriver {
                health: overflow.clone(),
                output_health: IoDriverHealth::Ok,
                writes: writes.clone(),
            },
        )];

        write_safe_outputs(&mut drivers, &SAFE_IMAGE)
            .expect("output-path health is Ok, so the safe state is confirmed");
        publish_health(&drivers, Some(&sink));

        assert_eq!(writes.lock().unwrap().as_slice(), &[vec![4]]);
        // Ordinary health reporting still shows the degraded driver.
        assert_eq!(sink.lock().unwrap()[0].health, overflow);
    }

    #[test]
    fn safe_state_is_unconfirmed_by_output_path_health_alone() {
        let mut drivers = vec![entry(
            "Output",
            SplitHealthDriver {
                health: IoDriverHealth::Ok,
                output_health: IoDriverHealth::Degraded {
                    error: "mqtt output handoff pending".into(),
                },
                writes: Arc::new(Mutex::new(Vec::new())),
            },
        )];

        let error = write_safe_outputs(&mut drivers, &SAFE_IMAGE)
            .expect_err("a degraded output path leaves the safe state unconfirmed");

        assert!(
            error
                .to_string()
                .contains("unconfirmed for driver 'Output'"),
            "{error}"
        );
    }

    #[test]
    fn safe_state_stays_unconfirmed_when_reported_health_is_degraded_for_another_reason() {
        let mut drivers = vec![entry(
            "Mqtt",
            FakeDriver::new(
                IoDriverHealth::Degraded {
                    error: "mqtt disconnected: broker closed".into(),
                },
                false,
            ),
        )];

        let error = write_safe_outputs(&mut drivers, &SAFE_IMAGE)
            .expect_err("a degraded connection leaves the safe state unconfirmed");

        assert!(
            error.to_string().contains("unconfirmed for driver 'Mqtt'"),
            "{error}"
        );
    }

    /// Reports ordinary and output-path health separately, as an MQTT driver does after overflow.
    struct SplitHealthDriver {
        health: IoDriverHealth,
        output_health: IoDriverHealth,
        writes: Arc<Mutex<Vec<Vec<u8>>>>,
    }

    impl IoDriver for SplitHealthDriver {
        fn read_inputs(&mut self, _inputs: &mut [u8]) -> Result<(), RuntimeError> {
            Ok(())
        }

        fn write_outputs(&mut self, outputs: &[u8]) -> Result<(), RuntimeError> {
            self.writes.lock().unwrap().push(outputs.to_vec());
            Ok(())
        }

        fn health(&self) -> IoDriverHealth {
            self.health.clone()
        }

        fn output_health(&self) -> IoDriverHealth {
            self.output_health.clone()
        }
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
