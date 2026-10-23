//! The HMI tank-level example (specification 39, §17.2 and §18 1D; specification 40): the actual
//! Control.PID holds the level of a gravity-drained tank, and the Alarm library's alarms follow
//! the process: a high level, a high-high trip and a pump overload trip that mark the first out,
//! and a fast rise detected by a block that extends the base.

use trust_runtime::debug::DebugControl;
use trust_runtime::harness::{CompileSession, SourceFile};
use trust_runtime::hmi::alarms::{read_alarms, AlarmStatus, Priority};
use trust_runtime::hmi::member_paths::{resolve_member, MemberPath};
use trust_runtime::io::IoAddress;
use trust_runtime::value::{Duration, EnumValue, Value};
use trust_runtime::Runtime;

/// The tank of `simulation.toml`: 0.2 m² with an outlet of c = 0.02 m^(5/2)/s.
const AREA: f64 = 0.2;
const OUTLET: f64 = 0.02;

struct Process {
    runtime: Runtime,
    debug: DebugControl,
    level: f64,
}

impl Process {
    fn new(level: f64) -> Self {
        let mut runtime = CompileSession::from_sources(vec![
            SourceFile::with_path(
                "Control/control.st",
                include_str!("../../../examples/hmi_tank_level/libraries/control/src/control.st"),
            ),
            SourceFile::with_path(
                "Alarm/alarm.st",
                include_str!("../../../examples/hmi_tank_level/libraries/alarm/src/alarm.st"),
            ),
            SourceFile::with_path(
                "Hmi/hmi.st",
                include_str!("../../../examples/hmi_tank_level/libraries/hmi/src/hmi.st"),
            ),
            SourceFile::with_path(
                "src/main.st",
                include_str!("../../../examples/hmi_tank_level/src/Main.st"),
            ),
        ])
        .build_runtime()
        .expect("the example compiles with its libraries");
        let debug = runtime.enable_debug();
        Self {
            runtime,
            debug,
            level,
        }
    }

    /// Runs the process for `seconds`: each 100 ms cycle reads the level, and the tank takes the
    /// inflow the program gives it.
    fn run(&mut self, seconds: f64) {
        for _ in 0..(seconds * 10.0).round() as usize {
            self.debug.enqueue_io_write(
                IoAddress::parse("%IL0").unwrap(),
                Value::LWord(self.level.to_bits()),
            );
            self.runtime.advance_time(Duration::from_millis(100));
            self.runtime.execute_cycle().expect("a cycle");
            let inflow = self.real("Tank.Inflow");
            self.level =
                (self.level + 0.1 * (inflow - OUTLET * self.level.max(0.0).sqrt()) / AREA).max(0.0);
        }
    }

    /// Runs until `done` holds, at most `seconds`; whether it did.
    fn run_until(&mut self, seconds: f64, done: impl Fn(&Self) -> bool) -> bool {
        for _ in 0..(seconds * 10.0).round() as usize {
            if done(self) {
                return true;
            }
            self.run(0.1);
        }
        done(self)
    }

    fn value(&self, path: &str) -> Value {
        resolve_member(&MemberPath::parse(path).unwrap(), self.runtime.storage())
            .cloned()
            .unwrap_or_else(|| panic!("{path} is in the program"))
    }

    fn real(&self, path: &str) -> f64 {
        match self.value(path) {
            Value::LReal(value) => value,
            Value::Real(value) => f64::from(value),
            other => panic!("{path} is {other:?}"),
        }
    }

    fn flag(&self, path: &str) -> bool {
        match self.value(path) {
            Value::Bool(value) => value,
            other => panic!("{path} is {other:?}"),
        }
    }

    /// Sets a variable of the program, as a write from the runtime display does.
    fn set(&mut self, name: &str, value: Value) {
        let Some(Value::Instance(program)) = self.runtime.storage().get_global("Tank").cloned()
        else {
            panic!("the program instance Tank");
        };
        self.runtime
            .write_instance(program, name, value)
            .expect("write the instance variable");
    }

    /// Sets the controller's mode by name, as the mode selector does.
    fn set_mode(&mut self, name: &str, number: i64) {
        let Value::Enum(current) = self.value("Tank.Mode") else {
            panic!("Mode is an enumeration");
        };
        let mode = EnumValue::from_serialized_parts(
            self.runtime.registry(),
            current.type_name(),
            name,
            number,
        )
        .expect("a PID mode");
        self.set("Mode", Value::Enum(Box::new(mode)));
    }

    fn alarm(&self, path: &str) -> AlarmStatus {
        read_alarms(&self.runtime)
            .into_iter()
            .find(|alarm| alarm.path.eq_ignore_ascii_case(path))
            .unwrap_or_else(|| panic!("the alarm {path}"))
    }
}

#[test]
fn the_actual_pid_holds_the_tank_level_at_its_setpoint() {
    let mut process = Process::new(1.5);
    process.run(60.0);
    assert!(
        (process.level - 1.5).abs() < 0.02,
        "held at 1.5 m: {}",
        process.level
    );
    process.set("Setpoint", Value::LReal(2.0));
    process.run(90.0);
    assert!(
        (process.level - 2.0).abs() < 0.02,
        "follows to 2.0 m: {}",
        process.level
    );
    let opening = process.real("Tank.ValveOpening");
    assert!(
        (0.0..=100.0).contains(&opening),
        "the valve within 0 to 100 %: {opening}"
    );
}

#[test]
fn a_setpoint_above_the_high_limit_raises_the_high_level_alarm() {
    let mut process = Process::new(1.5);
    process.set("Setpoint", Value::LReal(2.6));
    process.run(90.0);
    let high = process.alarm("Tank.Vessel.High");
    assert!(high.active && high.unacknowledged, "{high:?}");
    assert_eq!(high.priority, Priority::Medium);
    assert!(
        !process.alarm("Tank.Vessel.HighHigh").active,
        "below the trip at 2.8 m: {}",
        process.level
    );
    process.set("Setpoint", Value::LReal(1.5));
    process.run(90.0);
    let high = process.alarm("Tank.Vessel.High");
    assert!(
        !high.active && high.unacknowledged,
        "cleared, still to acknowledge: {high:?}"
    );
}

#[test]
fn a_high_high_level_trips_the_pump_and_is_the_first_out_until_reset() {
    let mut process = Process::new(1.5);
    process.set_mode("Manual", 1);
    process.set("ManualOpening", Value::LReal(100.0));
    assert!(
        process.run_until(60.0, |p| p.flag("Tank.Pump.Tripped")),
        "tripped at 2.8 m: {}",
        process.level
    );
    let trip = process.alarm("Tank.Vessel.HighHigh");
    assert!(trip.active && trip.first_out, "{trip:?}");
    assert_eq!(trip.priority, Priority::High);
    assert_eq!(process.real("Tank.Inflow"), 0.0, "no inflow while tripped");
    process.run(30.0);
    assert!(process.level < 2.7, "the level falls: {}", process.level);
    assert!(
        process.alarm("Tank.Vessel.HighHigh").active,
        "latched until reset"
    );
    process.set("TripReset", Value::Bool(true));
    process.run(0.2);
    process.set("TripReset", Value::Bool(false));
    process.set_mode("Auto", 0);
    process.run(1.0);
    assert!(
        !process.flag("Tank.Pump.Tripped"),
        "running again after the reset"
    );
    assert!(!process.alarm("Tank.Vessel.HighHigh").active);
}

#[test]
fn a_pump_overload_trips_the_pump_and_is_the_first_out() {
    let mut process = Process::new(1.5);
    process.set("OverloadTest", Value::Bool(true));
    process.run(1.0);
    let overload = process.alarm("Tank.Pump.OverloadTrip");
    assert!(overload.active && overload.first_out, "{overload:?}");
    assert!(process.flag("Tank.Pump.Tripped"));
    assert!(
        process.run_until(120.0, |p| p.alarm("Tank.Vessel.Low").active),
        "the level falls to low: {}",
        process.level
    );
    assert!(
        !process.alarm("Tank.Vessel.Low").first_out,
        "the low level follows; it is not the cause"
    );
}

#[test]
fn a_fast_rise_raises_the_level_rise_alarm_the_example_defines() {
    let mut process = Process::new(1.0);
    process.set_mode("Manual", 1);
    process.set("ManualOpening", Value::LReal(100.0));
    assert!(
        process.run_until(5.0, |p| p.alarm("Tank.Vessel.Rising").active),
        "rising at 0.15 m/s"
    );
    assert_eq!(process.alarm("Tank.Vessel.Rising").priority, Priority::Low);
}

#[test]
fn each_alarm_belongs_to_its_equipment() {
    let process = Process::new(1.5);
    for (alarm, equipment) in [
        ("Tank.Vessel.High", "Tank.Vessel"),
        ("Tank.Vessel.Rising", "Tank.Vessel"),
        ("Tank.Pump.OverloadTrip", "Tank.Pump"),
    ] {
        assert_eq!(
            process.alarm(alarm).equipment.as_deref(),
            Some(equipment),
            "{alarm}"
        );
    }
}

#[test]
fn the_level_rise_alarm_is_suppressed_by_design_while_the_controller_is_in_auto() {
    // A setpoint step makes the controller open the valve wide: a fast rise it handles itself,
    // which must not alarm (EEMUA 191: no alarm without an operator action to take).
    let mut process = Process::new(1.0);
    process.set("Setpoint", Value::LReal(2.2));
    process.run(30.0);
    let rising = process.alarm("Tank.Vessel.Rising");
    assert!(
        rising.suppressed && !rising.active && !rising.unacknowledged,
        "{rising:?}"
    );
}

#[test]
fn the_example_screens_pass_the_deployment_checks_against_its_program() {
    // The screens were built in the HMI Builder (the 1D acceptance, step 1); a deployment
    // checks them against the running program and the runtime's write policy (§16.4).
    use trust_runtime::hmi::access::config::hmi_access_from_runtime_toml;
    use trust_runtime::hmi::bundle::checks::{check_bundle, Program};
    use trust_runtime::hmi::bundle::HmiBundle;
    use trust_runtime::hmi::catalogue::Catalogue;
    let process = Process::new(1.5);
    let root =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/hmi_tank_level");
    let bundle = HmiBundle::read(&root).expect("the example's screens");
    for file in [
        "hmi/screen.toml",
        "hmi/pages/overview.toml",
        "hmi/pages/alarms.toml",
    ] {
        assert!(bundle.files.contains_key(file), "{file}");
    }
    let access = hmi_access_from_runtime_toml(
        &std::fs::read_to_string(root.join("runtime.toml")).expect("runtime.toml"),
    )
    .expect("the write policy");
    let metadata = process.runtime.metadata_snapshot();
    let problems = check_bundle(
        &bundle,
        Catalogue::shipped(),
        &Program {
            metadata: &metadata,
            storage: process.runtime.storage(),
        },
        &access.write,
    );
    assert_eq!(problems, vec![]);
}

#[test]
fn the_example_carries_the_libraries_it_uses_unchanged() {
    // The example gallery copies an example on its own, so it brings its libraries with it, as
    // the library management vendors them; each copy equals the shipped library.
    let repo = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let example = repo.join("examples/hmi_tank_level");
    let manifest = std::fs::read_to_string(example.join("trust-lsp.toml")).expect("the manifest");
    for (library, files) in [
        ("alarm", &["alarm.st"][..]),
        ("control", &["control.st", "advanced_control.st"][..]),
        ("hmi", &["hmi.st"][..]),
    ] {
        assert!(
            manifest.contains(&format!("path = \"libraries/{library}\"")),
            "{library} from the example's own folder: {manifest}"
        );
        for file in files {
            let shipped =
                std::fs::read(repo.join("libraries").join(library).join("src").join(file))
                    .expect("the shipped library");
            let copied = std::fs::read(
                example
                    .join("libraries")
                    .join(library)
                    .join("src")
                    .join(file),
            )
            .unwrap_or_default();
            assert!(copied == shipped, "examples/hmi_tank_level/libraries/{library}/src/{file} equals libraries/{library}/src/{file}");
        }
    }
}

#[test]
fn the_alarm_tutorial_quotes_the_example_program_as_it_is() {
    // Tutorial 24 (specification 39, §13.9) shows excerpts of the example's program; each line of
    // each excerpt must still be in it.
    let repo = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let tutorial = std::fs::read_to_string(repo.join("examples/tutorials/24_hmi_alarms/README.md"))
        .expect("the tutorial");
    let program: std::collections::HashSet<String> =
        std::fs::read_to_string(repo.join("examples/hmi_tank_level/src/Main.st"))
            .expect("the program")
            .lines()
            .map(|line| line.trim().to_string())
            .collect();
    let excerpts: Vec<&str> = tutorial
        .split("```iecst\n")
        .skip(1)
        .map(|rest| rest.split("```").next().unwrap_or_default())
        .collect();
    assert!(excerpts.len() >= 3, "the tutorial's excerpts");
    let missing: Vec<&str> = excerpts
        .iter()
        .flat_map(|excerpt| excerpt.lines())
        .map(str::trim)
        .filter(|line| !line.is_empty() && !program.contains(*line))
        .collect();
    assert_eq!(
        missing,
        Vec::<&str>::new(),
        "lines of the tutorial that are not in src/main.st"
    );
}
