//! The settings a device receives inside the deployment record (specification 34 §5.1.1,
//! section 7): what a device without an operating system needs from `runtime.toml` and `io.toml`.
//!
//! A chip never parses the two configuration files; it runs from this section. A computer reads
//! the files themselves at each start and ignores it.

use crate::io_address::IoAddress;
use crate::value::{Duration, Value};
use crate::watchdog::{FaultPolicy, RetainMode, WatchdogPolicy};
use alloc::string::String;
use alloc::vec::Vec;
use smol_str::SmolStr;

/// The configuration a device without an operating system runs from (specification 34 §5.1.1,
/// section 7). Every field is the setting of the same name in `runtime.toml` or `io.toml`.
#[derive(Debug, Clone, PartialEq)]
pub struct Configuration {
    /// `resource.name`. Empty in a record whose build did not fill this section.
    pub resource_name: SmolStr,
    /// `resource.cycle_interval_ms`, in nanoseconds.
    pub cycle_interval_nanos: u64,
    /// `runtime.control.debug_enabled`.
    pub control_debug_enabled: bool,
    /// `runtime.control.mode`.
    pub control_mode: ControlMode,
    /// `runtime.log.level`.
    pub log_level: LogLevel,
    /// `runtime.events.capacity`: the newest events the store keeps; 0 is the device's default
    /// (specification 11, the event store).
    pub event_capacity: u32,
    /// `runtime.retain.mode`.
    pub retain_mode: RetainMode,
    /// `runtime.retain.save_interval_ms`.
    pub retain_save_interval: Duration,
    /// `runtime.watchdog`: whether it is enabled, its timeout and its action.
    pub watchdog: WatchdogPolicy,
    /// `runtime.watchdog.hang_action`.
    pub watchdog_hang_action: HangAction,
    /// `runtime.fault.policy`.
    pub fault_policy: FaultPolicy,
    /// `[[resource.tasks]]`, in declaration order; `None` when the resource declares no override.
    pub task_overrides: Option<Vec<TaskOverride>>,
    /// `io.drivers`, in declaration order.
    pub io_drivers: Vec<IoDriver>,
    /// `[[io.safe_state]]`, in declaration order.
    pub safe_state: Vec<SafeStateEntry>,
}

impl Default for Configuration {
    /// The empty configuration: what a build writes before it fills the section, so that a record
    /// carries a section 7 from the start. Every flag is off, every interval is 0, the event-store
    /// capacity is 0 (the device's default), every enum of this module carries its first code, the
    /// watchdog is [`WatchdogPolicy::default`] (disabled, with `safe_halt`), and there are no task
    /// overrides, no drivers and no safe state.
    fn default() -> Self {
        Self {
            resource_name: SmolStr::new_static(""),
            cycle_interval_nanos: 0,
            control_debug_enabled: false,
            control_mode: ControlMode::Production,
            log_level: LogLevel::Error,
            event_capacity: 0,
            retain_mode: RetainMode::None,
            retain_save_interval: Duration::ZERO,
            watchdog: WatchdogPolicy::default(),
            watchdog_hang_action: HangAction::Fault,
            fault_policy: FaultPolicy::Halt,
            task_overrides: None,
            io_drivers: Vec::new(),
            safe_state: Vec::new(),
        }
    }
}

/// `runtime.control.mode`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControlMode {
    /// `production`.
    Production = 1,
    /// `debug`.
    Debug = 2,
}

/// `runtime.log.level`, with the level names the runtime's configuration parser accepts
/// (`warning` is `warn`). The levels are ordered from the fewest messages to the most, so a
/// message is written when its level is at most the configured one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum LogLevel {
    /// `error`.
    Error = 1,
    /// `warn`.
    Warn = 2,
    /// `info`.
    Info = 3,
    /// `debug`.
    Debug = 4,
    /// `trace`.
    Trace = 5,
}

/// `runtime.watchdog.hang_action`: what a device does with a scan that did not return
/// (specification 34 TIME-03).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HangAction {
    /// `fault`: record the fault and keep serving.
    Fault = 1,
    /// `abort`: record the fault, then stop the runtime.
    Abort = 2,
}

impl ControlMode {
    /// The mode that `text` names, trimmed and in any ASCII case.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        named(
            text,
            &[("production", Self::Production), ("debug", Self::Debug)],
        )
    }

    /// The mode's configuration name.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Production => "production",
            Self::Debug => "debug",
        }
    }
}

impl LogLevel {
    /// The level that `text` names, trimmed and in any ASCII case; `warning` is `warn`.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        named(
            text,
            &[
                ("error", Self::Error),
                ("warn", Self::Warn),
                ("warning", Self::Warn),
                ("info", Self::Info),
                ("debug", Self::Debug),
                ("trace", Self::Trace),
            ],
        )
    }

    /// The level's configuration name.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Error => "error",
            Self::Warn => "warn",
            Self::Info => "info",
            Self::Debug => "debug",
            Self::Trace => "trace",
        }
    }
}

impl HangAction {
    /// The action that `text` names, trimmed and in any ASCII case.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        named(text, &[("fault", Self::Fault), ("abort", Self::Abort)])
    }

    /// The action's configuration name.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Fault => "fault",
            Self::Abort => "abort",
        }
    }
}

/// The value of the first of `names` that `text`, trimmed, spells in any ASCII case.
fn named<T: Copy>(text: &str, names: &[(&str, T)]) -> Option<T> {
    let text = text.trim();
    names
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case(text))
        .map(|&(_, value)| value)
}

/// One `[[resource.tasks]]` override.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskOverride {
    /// The task name.
    pub name: SmolStr,
    /// `interval_ms`.
    pub interval: Duration,
    /// `priority`; smaller values run first.
    pub priority: u8,
    /// `single`: the task's single input, when it has one.
    pub single: Option<SmolStr>,
    /// `programs`, in declaration order.
    pub programs: Vec<SmolStr>,
}

/// One `io.drivers` entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IoDriver {
    /// The driver name, such as `gpio`.
    pub name: SmolStr,
    /// Whether the driver is enabled.
    pub enabled: bool,
    /// The driver's input lines; empty for every driver but `gpio`.
    pub inputs: Vec<GpioInput>,
    /// The driver's output lines; empty for every driver but `gpio`.
    pub outputs: Vec<GpioOutput>,
    /// `params`, the driver's parameter table as JSON text (specification 34 §5.1.1, section 8),
    /// from which a chip runs the driver; `None` in a version-1 record, which carries none.
    pub params: Option<String>,
}

/// One input line of the `gpio` driver.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GpioInput {
    /// `address`: the process image the line reads into.
    pub address: IoAddress,
    /// `line`, exactly as `io.toml` writes it: a number's decimal digits, such as `17`, or a pin
    /// name, such as `PD12`.
    pub line: SmolStr,
    /// `invert`: whether the line's level is inverted.
    pub invert: bool,
    /// `debounce_ms`.
    pub debounce_ms: u32,
}

/// One output line of the `gpio` driver.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GpioOutput {
    /// `address`: the process image the line writes.
    pub address: IoAddress,
    /// `line`, exactly as `io.toml` writes it: a number's decimal digits, such as `17`, or a pin
    /// name, such as `PD12`.
    pub line: SmolStr,
    /// `invert`: whether the line's level is inverted.
    pub invert: bool,
    /// `initial`: the level the line holds before the program writes it.
    pub initial: bool,
}

/// One `[[io.safe_state]]` entry: the value an output takes when the resource stops.
#[derive(Debug, Clone, PartialEq)]
pub struct SafeStateEntry {
    /// `address`.
    pub address: IoAddress,
    /// `value`, of a type the record has a code for.
    pub value: Value,
}
