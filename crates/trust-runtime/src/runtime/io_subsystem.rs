//! The I/O drivers of a computer's program (specification 44 §8.8): their health, the safe
//! outputs of a stop or a fault, and their links (specification 42): each `io.toml` driver's link
//! name, whether the program switched the link off, how long its last read and write took, and a
//! test read.

use std::sync::{Arc, Mutex};

use smol_str::SmolStr;

use crate::error::RuntimeError;
use crate::io::{IoDriver, IoDriverHealth, IoDriverStatus};

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
    /// The driver `driver` named `name`, of the link `link` for a driver of `io.toml`, switched
    /// on, before its first read and write.
    pub(super) fn new(name: SmolStr, link: Option<SmolStr>, driver: Box<dyn IoDriver>) -> Self {
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

/// A linked driver as its link reports it now (specification 42, §3 item 1, §4): what the
/// `Comm.Link` blocks show and what the HMI marks the values of a link without data by (§5 item 4).
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

/// Reads the inputs of every driver into `inputs`, in their order, and times each read; a link
/// switched off from the program is not read (specification 42, §4 item 6). The first failure ends
/// the read.
pub(super) fn read_inputs(
    drivers: &mut [IoDriverEntry],
    inputs: &mut [u8],
) -> Result<(), RuntimeError> {
    for entry in drivers.iter_mut().filter(|entry| entry.enabled) {
        let started = std::time::Instant::now();
        entry.driver.read_inputs(inputs)?;
        entry.read_time = started.elapsed();
    }
    Ok(())
}

/// Drives the outputs of every driver from `outputs`, in their order, and times each write; a link
/// switched off from the program is not written (specification 42, §4 item 6). The first failure
/// ends the write.
pub(super) fn write_outputs(
    drivers: &mut [IoDriverEntry],
    outputs: &[u8],
) -> Result<(), RuntimeError> {
    for entry in drivers.iter_mut().filter(|entry| entry.enabled) {
        let started = std::time::Instant::now();
        entry.driver.write_outputs(outputs)?;
        entry.write_time = started.elapsed();
    }
    Ok(())
}

/// Whether every driver is an in-process simulator with no physical I/O side effects, so that the
/// target is simulated while simulation is enabled (specification 35, A2.4).
pub(super) fn all_drivers_simulated(drivers: &[IoDriverEntry]) -> bool {
    drivers.iter().all(|entry| entry.driver.is_simulated())
}

/// Each linked driver as its link reports it.
pub(super) fn link_ios(drivers: &[IoDriverEntry]) -> impl Iterator<Item = LinkIo<'_>> + '_ {
    drivers.iter().filter_map(|entry| {
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
pub(super) fn set_link_enabled(drivers: &mut [IoDriverEntry], link: &str, enabled: bool) {
    for entry in drivers {
        if entry.link.as_deref() == Some(link) {
            entry.enabled = enabled;
        }
    }
}

/// Tests a linked driver (specification 42, §4 item 7): one read into a spare image of `size`
/// bytes, which the process image does not take; nothing when no driver has that link.
pub(super) fn test_link(
    drivers: &mut [IoDriverEntry],
    link: &str,
    size: usize,
) -> Option<Result<std::time::Duration, String>> {
    let entry = drivers
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
        IoDriverEntry::new(name.into(), None, Box::new(driver))
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

    /// Specification 42, §4 item 6: a link switched off from the program is neither read nor
    /// written, while the other drivers still are; switched on again, it is read and written again.
    /// Catches a switch that stops every driver, or one that stops the reads but not the writes.
    #[test]
    fn a_link_switched_off_is_neither_read_nor_written() {
        let field = CountingDriver::default();
        let other = CountingDriver::default();
        let mut drivers = vec![
            linked("modbus-tcp", "modbus_tcp:0", field.clone()),
            linked("simulated", "simulated:1", other.clone()),
        ];
        let mut inputs = [0u8; 2];
        set_link_enabled(&mut drivers, "modbus_tcp:0", false);
        read_inputs(&mut drivers, &mut inputs).unwrap();
        write_outputs(&mut drivers, &[0]).unwrap();
        assert_eq!(field.counts(), (0, 0), "the switched-off driver rests");
        assert_eq!(
            other.counts(),
            (1, 1),
            "the other driver is read and written"
        );
        let facts: Vec<(SmolStr, bool)> = link_ios(&drivers)
            .map(|io| (io.link.clone(), io.enabled))
            .collect();
        assert_eq!(
            facts,
            [
                (SmolStr::new("modbus_tcp:0"), false),
                (SmolStr::new("simulated:1"), true)
            ]
        );
        set_link_enabled(&mut drivers, "modbus_tcp:0", true);
        read_inputs(&mut drivers, &mut inputs).unwrap();
        write_outputs(&mut drivers, &[0]).unwrap();
        assert_eq!(field.counts(), (1, 1), "switched on again");
    }

    /// Specification 42, §4 item 7: a test reads its link's driver once, into a spare image of the
    /// size it is given, which the process image does not take, and gives the driver's error when
    /// the read fails; a link that no driver has gives nothing. Catches a test that reads into the
    /// process image, reads another link's driver, writes, or hides the error.
    #[test]
    fn a_test_reads_its_driver_once_into_a_spare_image() {
        let field = CountingDriver::default();
        let refusing = CountingDriver {
            refuse: Some("no answer from 10.0.0.5"),
            ..CountingDriver::default()
        };
        let mut drivers = vec![
            linked("modbus-tcp", "modbus_tcp:0", field.clone()),
            linked("mqtt", "mqtt:1", refusing.clone()),
        ];
        assert!(matches!(
            test_link(&mut drivers, "modbus_tcp:0", 4),
            Some(Ok(_))
        ));
        assert_eq!(field.counts(), (1, 0), "one read and no write");
        assert_eq!(field.state.lock().unwrap().read_len, 4, "a spare image");
        assert_eq!(refusing.counts(), (0, 0), "another link's driver rests");
        match test_link(&mut drivers, "mqtt:1", 4) {
            Some(Err(error)) => assert!(error.contains("no answer from 10.0.0.5"), "{error}"),
            other => panic!("the driver's error: {other:?}"),
        }
        assert!(test_link(&mut drivers, "mqtt:7", 4).is_none());
    }

    /// Specification 35, A2.4: the I/O is simulated when all attached I/O is, which no driver at
    /// all satisfies too; one physical driver among simulated ones makes it physical. Catches a
    /// rule that reads only the first driver.
    #[test]
    fn the_io_is_simulated_only_when_every_driver_is() {
        let simulated = || CountingDriver {
            simulated: true,
            ..CountingDriver::default()
        };
        assert!(all_drivers_simulated(&[]));
        let mut drivers = vec![
            entry("simulated", simulated()),
            entry("loopback", simulated()),
        ];
        assert!(all_drivers_simulated(&drivers));
        drivers.push(entry("modbus-tcp", CountingDriver::default()));
        assert!(!all_drivers_simulated(&drivers));
    }

    fn linked(name: &str, link: &str, driver: impl IoDriver + 'static) -> IoDriverEntry {
        IoDriverEntry::new(name.into(), Some(link.into()), Box::new(driver))
    }

    /// What a [`CountingDriver`] was asked.
    #[derive(Default)]
    struct Counted {
        reads: usize,
        writes: usize,
        /// The length of the image of the last read.
        read_len: usize,
    }

    /// A driver that counts its reads and writes, fills the image it reads, and refuses its reads
    /// with `refuse`.
    #[derive(Clone, Default)]
    struct CountingDriver {
        state: Arc<Mutex<Counted>>,
        refuse: Option<&'static str>,
        simulated: bool,
    }

    impl CountingDriver {
        fn counts(&self) -> (usize, usize) {
            let state = self.state.lock().unwrap();
            (state.reads, state.writes)
        }
    }

    impl IoDriver for CountingDriver {
        fn read_inputs(&mut self, inputs: &mut [u8]) -> Result<(), RuntimeError> {
            let mut state = self.state.lock().unwrap();
            state.reads += 1;
            state.read_len = inputs.len();
            if let Some(error) = self.refuse {
                return Err(RuntimeError::IoDriver(error.into()));
            }
            inputs.fill(0xFF);
            Ok(())
        }

        fn write_outputs(&mut self, _outputs: &[u8]) -> Result<(), RuntimeError> {
            self.state.lock().unwrap().writes += 1;
            Ok(())
        }

        fn is_simulated(&self) -> bool {
            self.simulated
        }
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
