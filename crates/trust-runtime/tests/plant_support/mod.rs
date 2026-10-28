//! Shared harness for the native tests of the truST Plant library (specification
//! `docs/specs/trust-plant-library.md`, §14). A test compiles the real library sources, with the
//! Alarm and Control libraries they depend on, together with one test program `Main`, for this
//! computer (the toolchain of specification 44 §4.6 at `TRUST_TOOLCHAIN`), and scans it with an
//! explicit clock. Methods are called from PLC code, so an operator command is a one-shot BOOL
//! input the test program executes and clears in the next scan.
#![allow(dead_code)]

use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

use trust_runtime::harness::TestHarness;
use trust_runtime::retain::FileRetainStore;
use trust_runtime::value::{Duration, Value};
use trust_runtime::RestartMode;

macro_rules! library_sources {
    ($($file:literal),+ $(,)?) => {
        /// The Plant library's source files, by name, as `libraries/plant/src` holds them.
        pub const PLANT_SOURCE_FILES: &[&str] = &[$($file),+];

        /// The Alarm and Control libraries the Plant library depends on, then every Plant source
        /// file.
        pub const SOURCES: &[&str] = &[
            include_str!("../../../../libraries/alarm/src/alarm.st"),
            include_str!("../../../../libraries/control/src/control.st"),
            $(include_str!(concat!("../../../../libraries/plant/src/", $file)),)+
        ];
    };
}

library_sources!(
    "types.st",
    "functions.st",
    "interfaces.st",
    "device.st",
    "end_position.st",
    "motor.st",
    "drive.st",
    "heater.st",
    "two_output_motor.st",
    "valve.st",
    "motor_valve.st",
    "cylinder.st",
    "control_valve.st",
    "totals.st",
    "analog_input.st",
    "digital_input.st",
    "scale_and_tank.st",
    "controllers.st",
    "conditions.st",
    "groups.st",
    "managers.st",
    "sequence.st",
);

/// A program without a configuration runs as an instance of its own name.
const PROGRAM_INSTANCE: &str = "Main";

/// The scan period of [`Rig::scan`].
pub const SCAN_MS: i64 = 100;

/// The library and one test program, compiled.
pub struct Rig {
    harness: TestHarness,
    programs: Vec<String>,
}

impl Rig {
    /// Compiles the library with the test program; nothing has run yet.
    pub fn new(program: &str) -> Self {
        let programs = vec![program.to_owned()];
        let harness = compile(&programs);
        Self { harness, programs }
    }

    pub fn harness(&mut self) -> &mut TestHarness {
        &mut self.harness
    }

    /// Advances the clock by one scan period and runs one scan.
    pub fn scan(&mut self) {
        self.step_ms(SCAN_MS);
    }

    pub fn scans(&mut self, count: usize) {
        for _ in 0..count {
            self.scan();
        }
    }

    /// Advances the clock by `millis` and runs one scan, which must not fail.
    pub fn step_ms(&mut self, millis: i64) {
        self.harness.advance_time(Duration::from_millis(millis));
        let result = self.harness.cycle();
        assert!(
            result.errors.is_empty(),
            "scan {} at {} ms runs without a runtime error: {:?}",
            result.cycle_number,
            result.elapsed_time.as_millis(),
            result.errors
        );
    }

    /// Writes a variable of the test program.
    pub fn set(&mut self, name: &str, value: Value) {
        if let Err(error) = self.harness.try_set_input(name, value) {
            panic!("the test program declares {name}: {error}");
        }
    }

    pub fn set_bool(&mut self, name: &str, value: bool) {
        self.set(name, Value::Bool(value));
    }

    pub fn set_real(&mut self, name: &str, value: f64) {
        self.set(name, Value::LReal(value));
    }

    pub fn set_int(&mut self, name: &str, value: i16) {
        self.set(name, Value::Int(value));
    }

    /// Presses a one-shot command: the program runs it in this scan and clears it.
    pub fn press(&mut self, command: &str) {
        self.set_bool(command, true);
        self.scan();
    }

    /// Reads a member path below the test program, for example `P101.RunCommand`, as the last
    /// scan left it.
    pub fn value(&self, path: &str) -> Value {
        let full = format!("{PROGRAM_INSTANCE}.{path}");
        self.harness
            .runtime()
            .with_program_view(|program| program.value(&full))
            .unwrap_or_else(|| panic!("{full} exists"))
    }

    pub fn flag(&self, path: &str) -> bool {
        match self.value(path) {
            Value::Bool(value) => value,
            other => panic!("{path} is a BOOL: {other:?}"),
        }
    }

    pub fn real(&self, path: &str) -> f64 {
        match self.value(path) {
            Value::LReal(value) => value,
            Value::Real(value) => f64::from(value),
            other => panic!("{path} is a REAL or LREAL: {other:?}"),
        }
    }

    pub fn count(&self, path: &str) -> u64 {
        match self.value(path) {
            Value::USInt(value) => u64::from(value),
            Value::UInt(value) => u64::from(value),
            Value::UDInt(value) => u64::from(value),
            Value::ULInt(value) => value,
            other => panic!("{path} is an unsigned integer: {other:?}"),
        }
    }

    pub fn int(&self, path: &str) -> i64 {
        match self.value(path) {
            Value::SInt(value) => i64::from(value),
            Value::Int(value) => i64::from(value),
            Value::DInt(value) => i64::from(value),
            Value::LInt(value) => value,
            other => panic!("{path} is a signed integer: {other:?}"),
        }
    }

    pub fn text(&self, path: &str) -> String {
        match self.value(path) {
            Value::String(value) => value.to_string(),
            Value::WString(value) => value,
            other => panic!("{path} is a STRING: {other:?}"),
        }
    }

    /// The variant name of an enumeration, for example `Manual` of `E_Mode#Manual`.
    pub fn state(&self, path: &str) -> String {
        match self.value(path) {
            Value::Enum(value) => value.variant_name().to_string(),
            other => panic!("{path} is an enumeration: {other:?}"),
        }
    }

    pub fn millis(&self, path: &str) -> i64 {
        match self.value(path) {
            Value::Time(value) | Value::LTime(value) => value.as_millis(),
            other => panic!("{path} is a TIME: {other:?}"),
        }
    }

    /// A warm restart (specification 10 §6.7, Plant §4.8): RETAIN members stay, the rest starts
    /// from its declarations.
    pub fn warm_restart(&mut self) {
        self.harness
            .restart(RestartMode::Warm)
            .expect("a warm restart");
    }

    /// A power loss (Plant §4.8.2, §11): the retain store is saved, the PLC goes off, and a new
    /// runtime of the same project loads the store at start, a warm start from the retain file
    /// (specification 44 §11). Nothing has run after the start.
    pub fn power_loss(self) -> Self {
        let path = store_path();
        let _ = std::fs::remove_file(&path);
        let programs = {
            let Self {
                mut harness,
                programs,
            } = self;
            let runtime = harness.runtime_mut();
            runtime
                .set_retain_store(
                    Some(Box::new(FileRetainStore::new(&path))),
                    Some(Duration::from_millis(1)),
                )
                .expect("the retain store is set before the power loss");
            runtime
                .save_retain_store()
                .expect("the retain store is saved before the power loss");
            programs
        };
        let mut harness = compile(&programs);
        harness
            .runtime_mut()
            .set_retain_store(
                Some(Box::new(FileRetainStore::new(&path))),
                Some(Duration::from_millis(1)),
            )
            .expect("the retain store is set at start");
        harness
            .restart_with_retain(RestartMode::Warm)
            .expect("the retain store is loaded at start");
        harness
            .runtime_mut()
            .set_retain_store(None, None)
            .expect("the retain store is detached");
        let _ = std::fs::remove_file(&path);
        Self { harness, programs }
    }
}

/// Asserts that two numbers agree to 1e-9.
pub fn assert_close(actual: f64, expected: f64, what: &str) {
    assert!(
        (actual - expected).abs() < 1e-9,
        "{what}: expected {expected}, got {actual}"
    );
}

fn compile(programs: &[String]) -> TestHarness {
    let mut sources: Vec<&str> = SOURCES.to_vec();
    sources.extend(programs.iter().map(String::as_str));
    TestHarness::from_sources(&sources)
        .unwrap_or_else(|error| panic!("the Plant library and the test program compile: {error}"))
}

/// A retain store file no other test of any test binary uses.
fn store_path() -> PathBuf {
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    std::env::temp_dir().join(format!(
        "trust_plant_library_retain_{}_{}.bin",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ))
}
