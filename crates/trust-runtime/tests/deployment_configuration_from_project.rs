//! Section 7 of the deployment record filled from a project (specification 34 §5.1.1, S1b-3):
//! `trust-runtime build` reads the project's `runtime.toml` and `io.toml` with the runtime's own
//! parsers, so a chip receives the settings a computer reads from those files.
//!
//! The tests need the toolchain of specification 44 §4.6 at TRUST_TOOLCHAIN.

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use smol_str::SmolStr;
use trust_runtime::bundle_builder::{build_program_image, BundleBuildReport};
use trust_runtime::deployment::DEPLOYMENT_RECORD_FILE;
use trust_runtime::io::{GpioDriver, IoAddress, IoSize};
use trust_runtime::memory::IoArea;
use trust_runtime::native::{Target, Toolchain};
use trust_runtime::runtime_core::deployment::{
    Configuration, ControlMode, DeploymentRecord, GpioInput, GpioOutput, HangAction, IoDriver,
    LogLevel, SafeStateEntry, TaskOverride,
};
use trust_runtime::value::{Duration, Value};
use trust_runtime::watchdog::{FaultPolicy, RetainMode, WatchdogAction, WatchdogPolicy};

static COUNTER: AtomicU64 = AtomicU64::new(1);

const PROGRAM: &str =
    "PROGRAM Main\nVAR\n    Counter : DINT;\nEND_VAR\nCounter := Counter + 1;\nEND_PROGRAM\n";

/// A `runtime.toml` with every setting section 7 carries, each value chosen by hand.
const RUNTIME_SETTINGS: &str = r#"
[bundle]
version = 1

[resource]
name = "s1b-configuration"
cycle_interval_ms = 10

[[resource.tasks]]
name = "fast"
interval_ms = 2
priority = 1
programs = ["Main"]

[runtime.control]
endpoint = "unix:///tmp/trust-runtime.sock"
debug_enabled = true
mode = "debug"

[runtime.log]
level = "debug"

[runtime.events]
capacity = 200

[runtime.retain]
mode = "file"
path = "retain.dat"
save_interval_ms = 5000

[runtime.watchdog]
enabled = true
timeout_ms = 50
action = "safe_halt"
hang_action = "abort"

[runtime.fault]
policy = "restart"
"#;

/// An `io.toml` with a `gpio` driver carrying pin names, a disabled `modbus-tcp` driver and one
/// safe-state entry, each value chosen by hand.
const IO_SETTINGS: &str = r#"
[io]
drivers = [
  { name = "gpio", enabled = true, params = { inputs = [{ address = "%IX0.0", line = "GPIO9", invert = true, debounce_ms = 20 }], outputs = [{ address = "%QX0.0", line = "GPIO18", initial = true }] } },
  { name = "modbus-tcp", enabled = false },
]

[[io.safe_state]]
address = "%QX0.0"
value = "FALSE"
"#;

/// A `runtime.toml` with only the sections and keys the parser requires, so the settings section
/// 7 carries take their defaults.
const RUNTIME_REQUIRED_ONLY: &str = r#"
[bundle]
version = 1

[resource]
name = "defaults"
cycle_interval_ms = 20

[runtime.control]
endpoint = "unix:///tmp/trust-runtime.sock"

[runtime.log]
level = "warn"

[runtime.retain]
mode = "none"
save_interval_ms = 1000

[runtime.watchdog]
enabled = false
timeout_ms = 1000
action = "halt"

[runtime.fault]
policy = "halt"
"#;

/// A built project with `runtime` as its `runtime.toml` and, when given, `io` as its `io.toml`.
fn project(name: &str, runtime: Option<&str>, io: Option<&str>) -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "trust-deployment-configuration-{name}-{}-{}",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join("src")).expect("create the project");
    if let Some(runtime) = runtime {
        std::fs::write(root.join("runtime.toml"), runtime).expect("write runtime.toml");
    }
    if let Some(io) = io {
        std::fs::write(root.join("io.toml"), io).expect("write io.toml");
    }
    std::fs::write(root.join("src/main.st"), PROGRAM).expect("write the program");
    root
}

/// `trust-runtime build` of `root` for this computer.
fn build(root: &std::path::Path) -> anyhow::Result<BundleBuildReport> {
    let toolchain =
        Toolchain::find().expect("TRUST_TOOLCHAIN names the toolchain of specification 44 §4.6");
    let target = Target::this_cpu().expect("a computer target");
    build_program_image(root, None, target, &toolchain, false, true)
}

/// The configuration the build wrote into `root`'s record.
fn built_configuration(root: &std::path::Path) -> Configuration {
    let report = build(root).expect("build the project");
    DeploymentRecord::decode(&std::fs::read(&report.record).expect("read the record"))
        .expect("the build writes a readable record")
        .configuration
}

/// `%IX0.0`, written out by hand rather than parsed.
fn input_bit() -> IoAddress {
    IoAddress {
        area: IoArea::Input,
        size: IoSize::Bit,
        byte: 0,
        bit: 0,
        path: vec![0],
        wildcard: false,
    }
}

/// `%QX0.0`, written out by hand rather than parsed.
fn output_bit() -> IoAddress {
    IoAddress {
        area: IoArea::Output,
        size: IoSize::Bit,
        byte: 0,
        bit: 0,
        path: vec![0],
        wildcard: false,
    }
}

#[test]
fn the_configuration_section_carries_runtime_and_io_settings() {
    let root = project("settings", Some(RUNTIME_SETTINGS), Some(IO_SETTINGS));
    let expected = Configuration {
        resource_name: SmolStr::new("s1b-configuration"),
        // resource.cycle_interval_ms = 10
        cycle_interval_nanos: 10_000_000,
        control_debug_enabled: true,
        control_mode: ControlMode::Debug,
        log_level: LogLevel::Debug,
        // runtime.events.capacity = 200
        event_capacity: 200,
        retain_mode: RetainMode::File,
        // runtime.retain.save_interval_ms = 5000
        retain_save_interval: Duration::from_millis(5_000),
        watchdog: WatchdogPolicy {
            enabled: true,
            // runtime.watchdog.timeout_ms = 50
            timeout: Duration::from_millis(50),
            action: WatchdogAction::SafeHalt,
        },
        watchdog_hang_action: HangAction::Abort,
        fault_policy: FaultPolicy::Restart,
        task_overrides: Some(vec![TaskOverride {
            name: SmolStr::new("fast"),
            // resource.tasks[].interval_ms = 2
            interval: Duration::from_millis(2),
            priority: 1,
            single: None,
            programs: vec![SmolStr::new("Main")],
        }]),
        io_drivers: vec![
            IoDriver {
                name: SmolStr::new("gpio"),
                enabled: true,
                inputs: vec![GpioInput {
                    address: input_bit(),
                    line: SmolStr::new("GPIO9"),
                    invert: true,
                    debounce_ms: 20,
                }],
                outputs: vec![GpioOutput {
                    address: output_bit(),
                    line: SmolStr::new("GPIO18"),
                    invert: false,
                    initial: true,
                }],
            },
            IoDriver {
                name: SmolStr::new("modbus-tcp"),
                enabled: false,
                inputs: Vec::new(),
                outputs: Vec::new(),
            },
        ],
        safe_state: vec![SafeStateEntry {
            address: output_bit(),
            value: Value::Bool(false),
        }],
    };
    assert_eq!(built_configuration(&root), expected);
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn defaults_fill_the_section_when_settings_are_absent() {
    // A project without runtime.toml and io.toml: every field is the section's default.
    let bare = project("absent", None, None);
    assert_eq!(built_configuration(&bare), Configuration::default());
    let _ = std::fs::remove_dir_all(bare);

    // A runtime.toml with only the required keys: the optional settings take their parser
    // defaults and a project without io.toml has no drivers and no safe state.
    let root = project("required-only", Some(RUNTIME_REQUIRED_ONLY), None);
    let expected = Configuration {
        resource_name: SmolStr::new("defaults"),
        // resource.cycle_interval_ms = 20
        cycle_interval_nanos: 20_000_000,
        // runtime.control.debug_enabled is absent and the mode defaults to production.
        control_debug_enabled: false,
        control_mode: ControlMode::Production,
        log_level: LogLevel::Warn,
        // runtime.events is absent: the section carries 0, the device's default.
        event_capacity: 0,
        retain_mode: RetainMode::None,
        // runtime.retain.save_interval_ms = 1000
        retain_save_interval: Duration::from_millis(1_000),
        watchdog: WatchdogPolicy {
            enabled: false,
            // runtime.watchdog.timeout_ms = 1000
            timeout: Duration::from_millis(1_000),
            action: WatchdogAction::Halt,
        },
        // runtime.watchdog.hang_action is absent and defaults to fault.
        watchdog_hang_action: HangAction::Fault,
        fault_policy: FaultPolicy::Halt,
        // resource.tasks is absent.
        task_overrides: None,
        io_drivers: Vec::new(),
        safe_state: Vec::new(),
    };
    assert_eq!(built_configuration(&root), expected);
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn a_setting_the_section_cannot_hold_is_refused_with_its_key() {
    // A resource name of 256 bytes: section 7's texts hold at most 255.
    let name = "n".repeat(256);
    let runtime = format!(
        "[bundle]\nversion = 1\n\n[resource]\nname = \"{name}\"\ncycle_interval_ms = 10\n\n\
         [runtime.control]\nendpoint = \"unix:///tmp/trust-runtime.sock\"\n\n\
         [runtime.log]\nlevel = \"info\"\n\n[runtime.retain]\nmode = \"none\"\nsave_interval_ms = 1000\n\n\
         [runtime.watchdog]\nenabled = false\ntimeout_ms = 1000\naction = \"halt\"\n\n\
         [runtime.fault]\npolicy = \"halt\"\n"
    );
    let root = project("too-long", Some(&runtime), None);
    let error = build(&root).expect_err("the build refuses a value section 7 cannot hold");
    let message = format!("{error:#}");
    assert!(message.contains("runtime.toml"), "{message}");
    assert!(message.contains("resource.name"), "{message}");
    assert!(message.contains(&name), "{message}");
    assert!(
        !root.join(DEPLOYMENT_RECORD_FILE).exists(),
        "no record is written for a value section 7 cannot hold"
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn a_hierarchical_safe_state_address_is_refused_with_its_key() {
    let io = r#"
[io]
drivers = [{ name = "loopback", enabled = true }]

[[io.safe_state]]
address = "%IX0.1.2"
value = "TRUE"
"#;
    let root = project("hierarchical", Some(RUNTIME_REQUIRED_ONLY), Some(io));
    let error = build(&root).expect_err("the build refuses a hierarchical address");
    let message = format!("{error:#}");
    assert!(message.contains("io.toml"), "{message}");
    assert!(message.contains("io.safe_state[0].address"), "{message}");
    assert!(message.contains("%IX0.1.2"), "{message}");
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn a_pin_name_line_is_refused_by_the_computer_gpio_driver() {
    let params: toml::Value = toml::from_str(r#"inputs = [{ address = "%IX0.0", line = "PD12" }]"#)
        .expect("valid GPIO parameters");
    let error = GpioDriver::validate_params(&params).expect_err("a computer needs a number");
    let message = error.to_string();
    assert!(
        message.contains("on this computer a line is a number"),
        "{message}"
    );
    assert!(message.contains("PD12"), "{message}");
}

#[test]
fn a_number_line_is_carried_as_its_decimal_digits() {
    let io = r#"
[io]
drivers = [
  { name = "gpio", enabled = true, params = { inputs = [{ address = "%IX0.0", line = 17 }] } },
]
"#;
    let root = project("number-line", Some(RUNTIME_REQUIRED_ONLY), Some(io));
    let configuration = built_configuration(&root);
    assert_eq!(
        configuration.io_drivers,
        vec![IoDriver {
            name: SmolStr::new("gpio"),
            enabled: true,
            inputs: vec![GpioInput {
                address: input_bit(),
                line: SmolStr::new("17"),
                invert: false,
                debounce_ms: 0,
            }],
            outputs: Vec::new(),
        }]
    );
    let _ = std::fs::remove_dir_all(root);
}
