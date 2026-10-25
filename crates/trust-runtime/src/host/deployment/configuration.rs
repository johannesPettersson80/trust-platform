//! The deployment record's configuration section, filled from a project (specification 34
//! §5.1.1, section 7): the settings a device without an operating system runs from, read from
//! the project's `runtime.toml` and `io.toml` with the runtime's own parsers, so a chip receives
//! the settings a computer reads from those files.

use std::path::Path;

use smol_str::SmolStr;
use trust_runtime_core::deployment::{
    Configuration, ControlMode, HangAction, IoDriver, LogLevel, SafeStateEntry, TaskOverride,
};
use trust_runtime_core::io_address::{IoAddress, IoSize};
use trust_runtime_core::memory::IoArea;
use trust_runtime_core::value::{format_user_value, Duration, Value};

use crate::config::{IoConfig, IoDriverConfig, RuntimeConfig};
use crate::error::RuntimeError;
use crate::io::{GpioDriver, IoSafeState};

/// The longest text section 7 holds: a one-byte length (specification 34 §5.1.1).
const MAX_TEXT_BYTES: usize = 255;

/// Section 7 of `project`'s record: its `runtime.toml` and `io.toml` read with the runtime's own
/// loaders, so every value has the meaning and the defaults it has on a computer (specification
/// 34 §5.1.1, R1). A project without `io.toml` has no drivers and no safe state; a project
/// without `runtime.toml` keeps the section's defaults.
///
/// # Errors
///
/// A file exists but cannot be read or parsed, or holds a value section 7 cannot carry (R2).
pub(crate) fn from_project(project: &Path) -> Result<Configuration, RuntimeError> {
    let runtime = load_runtime(project)?;
    let io = load_io(project)?;
    let (io_drivers, safe_state) = match &io {
        Some(io) => (drivers(&io.drivers)?, safe_state(&io.safe_state)?),
        None => (Vec::new(), Vec::new()),
    };
    match &runtime {
        Some(runtime) => from_runtime(runtime, io_drivers, safe_state),
        None => Ok(Configuration {
            io_drivers,
            safe_state,
            ..Configuration::default()
        }),
    }
}

/// The project's `runtime.toml`, when it has one.
fn load_runtime(project: &Path) -> Result<Option<RuntimeConfig>, RuntimeError> {
    let path = project.join("runtime.toml");
    if path.is_file() {
        Ok(Some(RuntimeConfig::load(&path)?))
    } else {
        Ok(None)
    }
}

/// The project's `io.toml`, when it has one.
fn load_io(project: &Path) -> Result<Option<IoConfig>, RuntimeError> {
    let path = project.join("io.toml");
    if path.is_file() {
        Ok(Some(IoConfig::load(&path)?))
    } else {
        Ok(None)
    }
}

/// Section 7's runtime fields, from `runtime.toml`.
fn from_runtime(
    runtime: &RuntimeConfig,
    io_drivers: Vec<IoDriver>,
    safe_state: Vec<SafeStateEntry>,
) -> Result<Configuration, RuntimeError> {
    Ok(Configuration {
        resource_name: text(&runtime.resource_name, "runtime.toml", "resource.name")?,
        cycle_interval_nanos: cycle_interval(runtime.cycle_interval)?,
        control_debug_enabled: runtime.control_debug_enabled,
        control_mode: match runtime.control_mode {
            crate::config::ControlMode::Production => ControlMode::Production,
            crate::config::ControlMode::Debug => ControlMode::Debug,
        },
        log_level: log_level(&runtime.log_level)?,
        event_capacity: event_capacity(runtime.event_capacity)?,
        retain_mode: runtime.retain_mode,
        retain_save_interval: runtime.retain_save_interval,
        watchdog: runtime.watchdog,
        watchdog_hang_action: match runtime.watchdog_hang_action {
            crate::watchdog::HangAction::Fault => HangAction::Fault,
            crate::watchdog::HangAction::Abort => HangAction::Abort,
        },
        fault_policy: runtime.fault_policy,
        task_overrides: match &runtime.tasks {
            Some(tasks) => Some(task_overrides(tasks)?),
            None => None,
        },
        io_drivers,
        safe_state,
    })
}

/// `resource.cycle_interval_ms` in the nanoseconds section 7 holds. The parser guarantees a
/// positive interval; anything else is refused rather than truncated.
fn cycle_interval(interval: Duration) -> Result<u64, RuntimeError> {
    u64::try_from(interval.as_nanos()).map_err(|_| {
        invalid(
            "runtime.toml: resource.cycle_interval_ms is outside the nanoseconds the deployment \
             record's configuration section holds",
        )
    })
}

/// `runtime.log.level` as section 7's code, where `warning` is `warn` (specification 34 §5.1.1).
fn log_level(level: &str) -> Result<LogLevel, RuntimeError> {
    LogLevel::parse(level).ok_or_else(|| {
        invalid(format!(
            "runtime.toml: runtime.log.level '{level}' is not a level the deployment record's \
             configuration section holds"
        ))
    })
}

/// `runtime.events.capacity` as section 7's four bytes: 0 when the project sets none, so the
/// device keeps its own default (specification 34 §5.1.1).
fn event_capacity(capacity: Option<usize>) -> Result<u32, RuntimeError> {
    match capacity {
        Some(capacity) => u32::try_from(capacity).map_err(|_| {
            invalid(format!(
                "runtime.toml: runtime.events.capacity {capacity} is outside the four bytes the \
                 deployment record's configuration section holds"
            ))
        }),
        None => Ok(0),
    }
}

/// The task overrides, each text checked against section 7's 255-byte limit.
fn task_overrides(
    tasks: &[crate::config::TaskOverride],
) -> Result<Vec<TaskOverride>, RuntimeError> {
    tasks
        .iter()
        .map(|task| {
            Ok(TaskOverride {
                name: text(&task.name, "runtime.toml", "resource.tasks[].name")?,
                interval: task.interval,
                priority: task.priority,
                single: match &task.single {
                    Some(single) => Some(text(single, "runtime.toml", "resource.tasks[].single")?),
                    None => None,
                },
                programs: task
                    .programs
                    .iter()
                    .map(|program| text(program, "runtime.toml", "resource.tasks[].programs[]"))
                    .collect::<Result<Vec<_>, _>>()?,
            })
        })
        .collect()
}

/// The I/O drivers, each name checked against section 7's 255-byte limit; the `gpio` driver
/// carries its input and output lines (specification 34 §5.1.1).
fn drivers(drivers: &[IoDriverConfig]) -> Result<Vec<IoDriver>, RuntimeError> {
    drivers
        .iter()
        .enumerate()
        .map(|(index, driver)| {
            let name = text(
                &driver.name,
                "io.toml",
                &format!("io.drivers[{index}].name"),
            )?;
            let (inputs, outputs) = if name.as_str() == "gpio" {
                GpioDriver::configuration_lines(&driver.params)?
            } else {
                (Vec::new(), Vec::new())
            };
            for (position, input) in inputs.iter().enumerate() {
                text(
                    &input.line,
                    "io.toml",
                    &format!("io.drivers[{index}].inputs[{position}].line"),
                )?;
            }
            for (position, output) in outputs.iter().enumerate() {
                text(
                    &output.line,
                    "io.toml",
                    &format!("io.drivers[{index}].outputs[{position}].line"),
                )?;
            }
            Ok(IoDriver {
                name,
                enabled: driver.enabled,
                inputs,
                outputs,
            })
        })
        .collect()
}

/// The safe state, each address and value checked against what section 7 holds.
fn safe_state(state: &IoSafeState) -> Result<Vec<SafeStateEntry>, RuntimeError> {
    state
        .outputs
        .iter()
        .enumerate()
        .map(|(index, (address, value))| {
            flat_address(address, &format!("io.safe_state[{index}].address"))?;
            if !has_type_code(value) {
                return Err(invalid(format!(
                    "io.toml: io.safe_state[{index}].value '{}' has no type code in the \
                     deployment record's configuration section",
                    format_user_value(value)
                )));
            }
            Ok(SafeStateEntry {
                address: address.clone(),
                value: value.clone(),
            })
        })
        .collect()
}

/// A text section 7 holds: a one-byte length of at most 255 bytes. The reason names the file,
/// the key and the value (R2).
fn text(value: &str, file: &str, key: &str) -> Result<SmolStr, RuntimeError> {
    if value.len() > MAX_TEXT_BYTES {
        return Err(invalid(format!(
            "{file}: {key} '{value}' is {} bytes; the deployment record's configuration section \
             holds at most {MAX_TEXT_BYTES}",
            value.len()
        )));
    }
    Ok(SmolStr::new(value))
}

/// A safe-state address section 7 holds: one flat, non-wildcard process-image address with a
/// size code (specification 34 §5.1.1).
fn flat_address(address: &IoAddress, key: &str) -> Result<(), RuntimeError> {
    let flat =
        !address.wildcard && address.path.len() == 1 && address.path.first() == Some(&address.byte);
    if flat && !matches!(address.size, IoSize::Bytes(_)) {
        return Ok(());
    }
    Err(invalid(format!(
        "io.toml: {key} '{}' is not a flat process-image address the deployment record's \
         configuration section holds",
        address_text(address)
    )))
}

/// An address as `io.toml` writes it, for a refusal reason.
fn address_text(address: &IoAddress) -> String {
    let area = match address.area {
        IoArea::Input => 'I',
        IoArea::Output => 'Q',
        IoArea::Memory => 'M',
    };
    let size = match address.size {
        IoSize::Bit => "X",
        IoSize::Byte => "B",
        IoSize::Word => "W",
        IoSize::DWord => "D",
        IoSize::LWord => "L",
        IoSize::Bytes(_) => "B",
    };
    if address.wildcard {
        return format!("%{area}{size}*");
    }
    let path = address
        .path
        .iter()
        .map(u32::to_string)
        .collect::<Vec<_>>()
        .join(".");
    if matches!(address.size, IoSize::Bit) {
        format!("%{area}{size}{path}.{}", address.bit)
    } else {
        format!("%{area}{size}{path}")
    }
}

/// Whether section 7 has a type code for this safe-state value (specification 34 §5.1.1).
fn has_type_code(value: &Value) -> bool {
    matches!(
        value,
        Value::Bool(_)
            | Value::SInt(_)
            | Value::Int(_)
            | Value::DInt(_)
            | Value::LInt(_)
            | Value::USInt(_)
            | Value::UInt(_)
            | Value::UDInt(_)
            | Value::ULInt(_)
            | Value::Byte(_)
            | Value::Word(_)
            | Value::DWord(_)
            | Value::LWord(_)
            | Value::Real(_)
            | Value::LReal(_)
    )
}

/// A configuration refusal with a reason that names the file, the key and the value (R2).
fn invalid(message: impl Into<SmolStr>) -> RuntimeError {
    RuntimeError::InvalidConfig(message.into())
}
