//! The C6's loader and program memory over fake hardware, for the tests of an online change
//! (specification 34 §11.3): GPIO lines that hold pin claims, a hardware watchdog that accepts the
//! C6's range, a RAM flash with the C6's units behind the real flash owner, and the records of the
//! programs. The tests call the loader as the resource loop does: `load`, then `prepare_change` and
//! `commit_change`.

#![allow(
    dead_code,
    reason = "each test file that includes this module uses a part of it"
)]

use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration as StdDuration;

use smol_str::SmolStr;
use trust_platform_esp32::checkpoint::RawFlash;
use trust_platform_esp32::loader::{CycleWatchdog, ProgramHardware};
use trust_platform_esp32::process_io::{
    check_pin, GpioPin, PinClaim, PinClaims, CHIP_GPIO_PINS, CHIP_GPIO_RESERVED,
};
use trust_platform_esp32::program_flash::FlashOwner;
use trust_platform_esp32::watchdog_rule::watchdog_timeout_ms;
use trust_runtime_core::bytecode::BytecodeVersion;
use trust_runtime_core::deployment::{
    sha256, ApplicationIdentity, Capabilities, Configuration, Contracts, DeploymentRecord,
    GpioInput, GpioOutput, IoDriver, NumericContract, ProcessImageSummary, RequestedLimits,
    SchedulingProfile, TimingEvidence,
};
use trust_runtime_core::error::RuntimeError;
use trust_runtime_core::io_address::IoAddress;
use trust_runtime_core::ports::{HardwareWatchdog, MonotonicClock, MonotonicInstant, TimeSpan};
use trust_runtime_core::retain::checkpoint::{
    CheckpointFlash, CheckpointIdentity, CheckpointLog, FlashError,
};
use trust_runtime_core::retain::RetainSnapshot;
use trust_runtime_core::value::Duration;
use trust_runtime_core::vm::PreparationLimits;
use trust_runtime_core::watchdog::RetainMode;
use trust_runtime_services::loading::select::Booted;
use trust_runtime_services::runner::engine::ProcessIo;
use trust_runtime_services::status::IoDriverStatus;

/// The A4 program: `Plant` has the retained `activations` and a 25 ms task (6,412 bytes).
pub const PROGRAM: &[u8] =
    include_bytes!("../../../trust-runtime/tests/fixtures/portability/stbc-2.0/program-v2.stbc");

/// The board probe: `led := button` every 10 ms, binding `%IX0.0` and `%QX0.0` (3,388 bytes).
pub const PROBE: &[u8] =
    include_bytes!("../../../trust-runtime/tests/fixtures/portability/f401/gpio.stbc");

/// A clock that never moves: the loader reads no time.
#[derive(Clone, Copy)]
pub struct TestClock;

impl MonotonicClock for TestClock {
    fn now(&self) -> MonotonicInstant {
        MonotonicInstant::from_origin(StdDuration::ZERO)
    }
}

#[derive(Default)]
pub struct DogState {
    pub feeds: u32,
    pub intervals: Vec<u32>,
}

/// A hardware watchdog that counts its feeds and accepts the C6's range, 1 to 60,000 ms.
#[derive(Clone)]
pub struct Dog(pub Rc<RefCell<DogState>>);

impl Default for Dog {
    /// A watchdog whose list of intervals has room for all it will be asked to keep, so that the
    /// bookkeeping of a test does not grow the heap that the test measures.
    fn default() -> Self {
        Self(Rc::new(RefCell::new(DogState {
            feeds: 0,
            intervals: Vec::with_capacity(16),
        })))
    }
}

impl HardwareWatchdog for Dog {
    fn feed_interval(&self) -> TimeSpan {
        TimeSpan::new(StdDuration::from_millis(1_000))
    }

    fn feed(&mut self) {
        self.0.borrow_mut().feeds += 1;
    }
}

impl CycleWatchdog for Dog {
    fn apply_cycle_timeout_ms(&mut self, cycle_timeout_ms: u32) -> bool {
        let interval = watchdog_timeout_ms(cycle_timeout_ms);
        let in_range = (1..=60_000).contains(&interval);
        if in_range {
            self.0.borrow_mut().intervals.push(interval);
        }
        in_range
    }
}

/// A flash from address 0 to the end of the 8 MB module, erased, with the C6's units.
pub struct RamFlash {
    pub bytes: Vec<u8>,
}

impl RamFlash {
    pub fn erased() -> Self {
        Self {
            bytes: vec![0xFF; 0x80_0000],
        }
    }
}

impl RawFlash for RamFlash {
    fn read_at(&mut self, address: u32, bytes: &mut [u8]) -> Result<(), FlashError> {
        let start = address as usize;
        let stored = self
            .bytes
            .get(start..start + bytes.len())
            .ok_or(FlashError::Read)?;
        bytes.copy_from_slice(stored);
        Ok(())
    }

    fn program_at(&mut self, address: u32, bytes: &[u8]) -> Result<(), FlashError> {
        let start = address as usize;
        let stored = self
            .bytes
            .get_mut(start..start + bytes.len())
            .ok_or(FlashError::Program)?;
        for (cell, byte) in stored.iter_mut().zip(bytes) {
            *cell &= byte;
        }
        Ok(())
    }

    fn erase_at(&mut self, address: u32) -> Result<(), FlashError> {
        let start = address as usize;
        let unit = self
            .bytes
            .get_mut(start..start + 4_096)
            .ok_or(FlashError::Erase)?;
        unit.fill(0xFF);
        Ok(())
    }
}

/// The process I/O of a program: it holds the claims on its pins, and frees them when it drops.
pub struct Lines {
    pub claims: Vec<PinClaim>,
}

impl ProcessIo for Lines {
    fn read_inputs(&mut self, _image: &mut [u8]) -> Result<(), RuntimeError> {
        Ok(())
    }

    fn write_outputs(&mut self, _image: &[u8]) -> Result<(), RuntimeError> {
        Ok(())
    }

    fn write_safe_outputs(&mut self, _image: &[u8]) -> Result<(), RuntimeError> {
        Ok(())
    }

    fn driver_health(&self) -> Vec<IoDriverStatus> {
        Vec::new()
    }
}

/// The chip's hardware as the loader sees it: the GPIO pins, claimed through the same pin checks and
/// claim set as the C6's, and the real flash owner, which hands the checkpoint slots to one writer at
/// a time.
pub struct Hardware {
    pub owner: FlashOwner<RamFlash>,
    pub claims: PinClaims,
}

impl Hardware {
    /// Hardware over an erased flash, with a read-only handle on its checkpoint slots for the loader.
    pub fn with_reader() -> (Self, Box<dyn CheckpointFlash>) {
        let owner = FlashOwner::new(RamFlash::erased());
        let reader = owner
            .checkpoint_reader()
            .expect("the checkpoint slots have a geometry");
        (
            Self {
                owner,
                claims: PinClaims::default(),
            },
            Box::new(reader),
        )
    }

    /// Writes `snapshot` as the newest committed checkpoint, through the real checkpoint log.
    pub fn save_checkpoint(&mut self, snapshot: &RetainSnapshot) {
        let writer = self
            .owner
            .take_checkpoint()
            .expect("the checkpoint slots have a geometry")
            .expect("the checkpoint writer is free");
        let mut payload = Vec::new();
        snapshot
            .encode_snapshot_payload_into(&mut payload)
            .expect("the snapshot encodes");
        CheckpointLog::open(writer)
            .expect("the checkpoint log opens")
            .append(
                CheckpointIdentity {
                    application: [0x11; 16],
                    schema: [0x22; 16],
                },
                &payload,
            )
            .expect("the checkpoint is written");
    }

    /// Whether a line holds `pin` now.
    pub fn holds(&self, pin: &str) -> bool {
        self.claims.holds(GpioPin::parse(pin).expect("a pin name"))
    }
}

impl ProgramHardware for Hardware {
    type Io = Lines;

    fn claim_lines(&mut self, config: &Configuration) -> Result<Lines, String> {
        let (inputs, outputs) =
            trust_runtime_services::chip::process_io::record_lines(config, "C6")?;
        let reserved = CHIP_GPIO_RESERVED.map(GpioPin::from_number);
        let mut claimed = Vec::new();
        let names = inputs
            .iter()
            .map(|line| line.line.as_str())
            .chain(outputs.iter().map(|line| line.line.as_str()));
        for name in names {
            let claim = check_pin(name, CHIP_GPIO_PINS, &reserved)
                .and_then(|pin| self.claims.claim(pin))
                .map_err(|error| {
                    format!("the installed application's GPIO lines cannot be claimed: {error}")
                })?;
            claimed.push(claim);
        }
        Ok(Lines { claims: claimed })
    }

    fn checkpoint_flash(&mut self) -> Result<Box<dyn CheckpointFlash>, String> {
        let writer = self
            .owner
            .take_checkpoint()
            .map_err(|error| format!("the chip's checkpoint flash is not available: {error:?}"))?
            .ok_or_else(|| {
                String::from("the chip's checkpoint flash is not available: it is still in use")
            })?;
        Ok(Box::new(writer))
    }
}

/// The `gpio` driver of a record with the given input and output pins, in `io.toml` order, at the
/// addresses `%IX0.n` and `%QX0.n`.
pub fn gpio(inputs: &[&str], outputs: &[&str]) -> IoDriver {
    let address = |text: &str| IoAddress::parse(text).expect("an address");
    IoDriver {
        name: SmolStr::new_static("gpio"),
        enabled: true,
        inputs: inputs
            .iter()
            .enumerate()
            .map(|(index, pin)| GpioInput {
                address: address(&format!("%IX0.{index}")),
                line: SmolStr::new(pin),
                invert: false,
                debounce_ms: 0,
            })
            .collect(),
        outputs: outputs
            .iter()
            .enumerate()
            .map(|(index, pin)| GpioOutput {
                address: address(&format!("%QX0.{index}")),
                line: SmolStr::new(pin),
                invert: false,
                initial: false,
            })
            .collect(),
    }
}

/// The configuration of a record: `retain_mode`, and the lines of `drivers`.
pub fn configuration(retain_mode: RetainMode, drivers: Vec<IoDriver>) -> Configuration {
    Configuration {
        retain_mode,
        retain_save_interval: Duration::ZERO,
        io_drivers: drivers,
        ..Configuration::default()
    }
}

/// The container's record of `program` with the configuration `configuration`, as a boot hands it to
/// the loader: sequence 1, the program's bytes, its decoded record and the default limits.
pub fn booted(program: &[u8], configuration: Configuration) -> Booted {
    let record = DeploymentRecord {
        application: ApplicationIdentity {
            program_sha256: sha256(program),
            program_bytes: u32::try_from(program.len()).expect("a short program"),
            bytecode_version: BytecodeVersion::SOURCE_FREE_V2,
            bytecode_flags: 1,
            configuration_sha256: [0; 32],
            retain_schema_sha256: [0; 32],
        },
        contracts: Contracts {
            scheduling_profile: SchedulingProfile::COOPERATIVE_RESOURCE_V1,
            numeric_contract: NumericContract::V1,
            timing_evidence: TimingEvidence::UNQUALIFIED,
            capabilities: Capabilities::NONE,
        },
        limits: RequestedLimits::from_preparation(&PreparationLimits::default()),
        process_image: ProcessImageSummary::default(),
        tasks: Vec::new(),
        imports: Vec::new(),
        configuration,
    };
    Booted {
        sequence: 1,
        program: program.to_vec(),
        record,
        limits: PreparationLimits::default(),
    }
}
