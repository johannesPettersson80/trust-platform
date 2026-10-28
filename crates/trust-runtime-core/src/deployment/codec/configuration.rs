//! The bytes of the record's configuration section (specification 34 §5.1.1, section 7): the
//! settings a device without an operating system needs from `runtime.toml` and `io.toml`.

use super::Reader;
use crate::deployment::configuration::{
    Configuration, ControlMode, GpioInput, GpioOutput, HangAction, IoDriver, LogLevel,
    SafeStateEntry, TaskOverride,
};
use crate::deployment::DeploymentRecordError as Error;
use crate::io_address::{IoAddress, IoSize};
use crate::memory::IoArea;
use crate::value::{Duration, Value};
use crate::watchdog::{FaultPolicy, RetainMode, WatchdogAction, WatchdogPolicy};
use alloc::string::String;
use alloc::vec::Vec;
use smol_str::SmolStr;

/// The longest text a field holds: the maximum of its 1-byte length.
const MAX_TEXT_BYTES: usize = 255;
/// Section 7's bytes before its variable parts: the resource name's length byte, the cycle
/// interval, control, the log level, the event-store capacity, retain, the watchdog, the fault
/// policy, the task overrides' presence byte, and the counts that start the I/O drivers and the
/// safe state.
const FIXED_BYTES: usize = 1 + 8 + 2 + 1 + 4 + (1 + 8) + (1 + 8 + 1 + 1) + 1 + 1 + 2 + 2;
/// The fewest bytes one task override takes: its name, interval, priority, single input and
/// program count, each name one byte long.
const TASK_OVERRIDE_MIN_BYTES: usize = 1 + 1 + 8 + 1 + 1 + 2 + 1 + 1;
/// The fewest bytes one I/O driver takes: its name and its enabled flag.
const DRIVER_MIN_BYTES: usize = 1 + 1 + 1;
/// The fewest bytes one GPIO input takes: its address, line, invert flag and debounce interval.
const GPIO_INPUT_MIN_BYTES: usize = 7 + 1 + 1 + 1 + 4;
/// The fewest bytes one GPIO output takes: its address, line and two flags.
const GPIO_OUTPUT_MIN_BYTES: usize = 7 + 1 + 1 + 1 + 1;
/// The fewest bytes one safe-state entry takes: its address, value type and value.
const SAFE_STATE_ENTRY_MIN_BYTES: usize = 7 + 1 + 8;

impl Configuration {
    /// The length of section 7's payload in bytes, exactly what `Configuration::encode` writes;
    /// the record's own length uses it.
    #[must_use]
    pub(super) fn encoded_len(&self) -> usize {
        let mut bytes = FIXED_BYTES.saturating_add(self.resource_name.len());
        if let Some(overrides) = &self.task_overrides {
            bytes = bytes.saturating_add(2);
            for task in overrides {
                bytes = bytes
                    .saturating_add(1 + task.name.len() + 8 + 1)
                    .saturating_add(1 + task.single.as_ref().map_or(0, SmolStr::len))
                    .saturating_add(2);
                for program in &task.programs {
                    bytes = bytes.saturating_add(1 + program.len());
                }
            }
        }
        for driver in &self.io_drivers {
            bytes = bytes.saturating_add(1 + driver.name.len() + 1);
            if is_gpio(&driver.name) {
                bytes = bytes.saturating_add(2 + 2);
                for input in &driver.inputs {
                    bytes = bytes.saturating_add(7 + 1 + input.line.len() + 1 + 4);
                }
                for output in &driver.outputs {
                    bytes = bytes.saturating_add(7 + 1 + output.line.len() + 1 + 1);
                }
            }
        }
        bytes.saturating_add(self.safe_state.len().saturating_mul(7 + 1 + 8))
    }

    /// Append section 7's payload, in the order of specification 34 §5.1.1.
    ///
    /// # Errors
    ///
    /// A field the section's bytes cannot hold: an empty name where the section requires one, a
    /// text over 255 bytes, a count over 65,535, lines under a driver that is not `gpio`, an
    /// address the seven bytes cannot hold, or a safe-state value without a type code.
    pub(super) fn encode(&self, out: &mut Vec<u8>) -> Result<(), Error> {
        write_text(out, &self.resource_name, "the resource name")?;
        out.extend_from_slice(&self.cycle_interval_nanos.to_le_bytes());
        out.push(u8::from(self.control_debug_enabled));
        out.push(self.control_mode as u8);
        out.push(self.log_level as u8);
        out.extend_from_slice(&self.event_capacity.to_le_bytes());
        out.push(retain_mode_code(self.retain_mode));
        out.extend_from_slice(&self.retain_save_interval.as_nanos().to_le_bytes());
        out.push(u8::from(self.watchdog.enabled));
        out.extend_from_slice(&self.watchdog.timeout.as_nanos().to_le_bytes());
        out.push(watchdog_action_code(self.watchdog.action));
        out.push(self.watchdog_hang_action as u8);
        out.push(fault_policy_code(self.fault_policy));
        write_task_overrides(out, self.task_overrides.as_deref())?;
        write_drivers(out, &self.io_drivers)?;
        write_safe_state(out, &self.safe_state)
    }
}

/// Read section 7's payload (specification 34 §5.1.1).
///
/// # Errors
///
/// A field outside its range, a text that is not UTF-8, a missing name, or a payload that does not
/// hold the counts it declares; the reason names the field.
pub(super) fn read(reader: &mut Reader<'_>) -> Result<Configuration, Error> {
    let resource_name = read_text(reader, "the resource name")?;
    let cycle_interval_nanos = reader.u64()?;
    let control_debug_enabled = read_flag(reader, "the control debug flag")?;
    let control_mode = match reader.u8()? {
        1 => ControlMode::Production,
        2 => ControlMode::Debug,
        _ => return Err(Error::Field("a control mode")),
    };
    let log_level = match reader.u8()? {
        1 => LogLevel::Error,
        2 => LogLevel::Warn,
        3 => LogLevel::Info,
        4 => LogLevel::Debug,
        5 => LogLevel::Trace,
        _ => return Err(Error::Field("a log level")),
    };
    // Any four-byte value: 0 is the device's default and the rest is the requested capacity
    // (specification 34 §5.1.1; the device's maximum is admission's, not the decoder's).
    let event_capacity = reader.u32()?;
    let retain_mode = match reader.u8()? {
        0 => RetainMode::None,
        1 => RetainMode::File,
        _ => return Err(Error::Field("a retain mode")),
    };
    let retain_save_interval = Duration::from_nanos(reader.i64()?);
    let watchdog = WatchdogPolicy {
        enabled: read_flag(reader, "the watchdog enabled flag")?,
        timeout: Duration::from_nanos(reader.i64()?),
        action: match reader.u8()? {
            1 => WatchdogAction::Halt,
            2 => WatchdogAction::SafeHalt,
            3 => WatchdogAction::Restart,
            _ => return Err(Error::Field("a watchdog action")),
        },
    };
    let watchdog_hang_action = match reader.u8()? {
        1 => HangAction::Fault,
        2 => HangAction::Abort,
        _ => return Err(Error::Field("a watchdog hang action")),
    };
    let fault_policy = match reader.u8()? {
        1 => FaultPolicy::Halt,
        2 => FaultPolicy::SafeHalt,
        3 => FaultPolicy::Restart,
        _ => return Err(Error::Field("a fault policy")),
    };
    Ok(Configuration {
        resource_name,
        cycle_interval_nanos,
        control_debug_enabled,
        control_mode,
        log_level,
        event_capacity,
        retain_mode,
        retain_save_interval,
        watchdog,
        watchdog_hang_action,
        fault_policy,
        task_overrides: read_task_overrides(reader)?,
        io_drivers: read_drivers(reader)?,
        safe_state: read_safe_state(reader)?,
    })
}

/// A text: its 1-byte length and its UTF-8 bytes, at most 255.
fn write_text(out: &mut Vec<u8>, text: &str, field: &'static str) -> Result<(), Error> {
    if text.len() > MAX_TEXT_BYTES {
        return Err(Error::Field(field));
    }
    // Checked above: the length fits the one byte the section holds it in.
    out.push(u8::try_from(text.len()).unwrap_or(u8::MAX));
    out.extend_from_slice(text.as_bytes());
    Ok(())
}

/// A name: a text the section requires to be present.
fn write_name(out: &mut Vec<u8>, name: &str, field: &'static str) -> Result<(), Error> {
    if name.is_empty() {
        return Err(Error::Field(field));
    }
    write_text(out, name, field)
}

/// A count: two bytes, little-endian.
fn write_count(out: &mut Vec<u8>, count: usize, field: &'static str) -> Result<(), Error> {
    let count = u16::try_from(count).map_err(|_| Error::Field(field))?;
    out.extend_from_slice(&count.to_le_bytes());
    Ok(())
}

/// The task overrides: their presence byte, and then their count and each override.
fn write_task_overrides(
    out: &mut Vec<u8>,
    overrides: Option<&[TaskOverride]>,
) -> Result<(), Error> {
    let Some(overrides) = overrides else {
        out.push(0);
        return Ok(());
    };
    out.push(1);
    write_count(out, overrides.len(), "the task override count")?;
    for task in overrides {
        write_name(out, &task.name, "a task name")?;
        out.extend_from_slice(&task.interval.as_nanos().to_le_bytes());
        out.push(task.priority);
        write_single(out, task.single.as_ref())?;
        write_count(out, task.programs.len(), "a program count")?;
        for program in &task.programs {
            write_name(out, program, "an override program name")?;
        }
    }
    Ok(())
}

/// A task override's single input: a text, empty for none.
fn write_single(out: &mut Vec<u8>, single: Option<&SmolStr>) -> Result<(), Error> {
    match single {
        // A text that is present is never empty; an empty one reads back as none.
        Some(single) if single.is_empty() => Err(Error::Field("a task's single input")),
        Some(single) => write_text(out, single, "a task's single input"),
        None => {
            out.push(0);
            Ok(())
        }
    }
}

/// The I/O drivers: their count, and then each driver's name and enabled flag; the `gpio` driver is
/// followed by its input and output lines.
fn write_drivers(out: &mut Vec<u8>, drivers: &[IoDriver]) -> Result<(), Error> {
    write_count(out, drivers.len(), "the I/O driver count")?;
    for driver in drivers {
        write_name(out, &driver.name, "a driver name")?;
        out.push(u8::from(driver.enabled));
        if !is_gpio(&driver.name) {
            // Only the `gpio` driver carries lines, so another driver has none.
            if !(driver.inputs.is_empty() && driver.outputs.is_empty()) {
                return Err(Error::Field("the GPIO lines of another driver"));
            }
            continue;
        }
        write_count(out, driver.inputs.len(), "a GPIO input count")?;
        for input in &driver.inputs {
            write_address(out, &input.address)?;
            write_name(out, &input.line, "a GPIO line")?;
            out.push(u8::from(input.invert));
            out.extend_from_slice(&input.debounce_ms.to_le_bytes());
        }
        write_count(out, driver.outputs.len(), "a GPIO output count")?;
        for output in &driver.outputs {
            write_address(out, &output.address)?;
            write_name(out, &output.line, "a GPIO line")?;
            out.push(u8::from(output.invert));
            out.push(u8::from(output.initial));
        }
    }
    Ok(())
}

/// The safe state: its count, and then each entry's address, value type and value.
fn write_safe_state(out: &mut Vec<u8>, entries: &[SafeStateEntry]) -> Result<(), Error> {
    write_count(out, entries.len(), "the safe state count")?;
    for entry in entries {
        write_address(out, &entry.address)?;
        let (code, value) =
            value_encoding(&entry.value).ok_or(Error::Field("an I/O safe-state value"))?;
        out.push(code);
        out.extend_from_slice(&value);
    }
    Ok(())
}

/// An address: its area, size, byte offset and bit.
fn write_address(out: &mut Vec<u8>, address: &IoAddress) -> Result<(), Error> {
    let Some(size) = size_code(address.size) else {
        return Err(Error::Field("an I/O address"));
    };
    // A flat address is one path component naming its own byte, not a wildcard.
    let flat =
        !address.wildcard && address.path.len() == 1 && address.path.first() == Some(&address.byte);
    if !flat || address.bit > 7 || (address.bit != 0 && !matches!(address.size, IoSize::Bit)) {
        return Err(Error::Field("an I/O address"));
    }
    out.push(area_code(address.area));
    out.push(size);
    out.extend_from_slice(&address.byte.to_le_bytes());
    out.push(address.bit);
    Ok(())
}

/// Whether a driver is the `gpio` driver, the only one whose lines the section holds.
fn is_gpio(name: &str) -> bool {
    name == "gpio"
}

/// The section's code of a process-image area.
fn area_code(area: IoArea) -> u8 {
    match area {
        IoArea::Input => 1,
        IoArea::Output => 2,
        IoArea::Memory => 3,
    }
}

/// The section's code of an address's size: `None` for an explicit byte sequence, which the
/// address's seven bytes cannot hold.
fn size_code(size: IoSize) -> Option<u8> {
    Some(match size {
        IoSize::Bit => 1,
        IoSize::Byte => 2,
        IoSize::Word => 3,
        IoSize::DWord => 4,
        IoSize::LWord => 5,
        IoSize::Bytes(_) => return None,
    })
}

/// The section's code of a retain mode.
fn retain_mode_code(mode: RetainMode) -> u8 {
    match mode {
        RetainMode::None => 0,
        RetainMode::File => 1,
    }
}

/// The section's code of a watchdog action.
fn watchdog_action_code(action: WatchdogAction) -> u8 {
    match action {
        WatchdogAction::Halt => 1,
        WatchdogAction::SafeHalt => 2,
        WatchdogAction::Restart => 3,
    }
}

/// The section's code of a fault policy.
fn fault_policy_code(policy: FaultPolicy) -> u8 {
    match policy {
        FaultPolicy::Halt => 1,
        FaultPolicy::SafeHalt => 2,
        FaultPolicy::Restart => 3,
    }
}

/// The section's type code and eight bytes of a safe-state value, little-endian and zero-extended
/// above the type's width; `None` for a value the section has no code for.
fn value_encoding(value: &Value) -> Option<(u8, [u8; 8])> {
    let mut bytes = [0u8; 8];
    let code = match value {
        Value::Bool(value) => {
            bytes[0] = u8::from(*value);
            1
        }
        Value::SInt(value) => {
            bytes[0] = value.to_le_bytes()[0];
            2
        }
        Value::Int(value) => {
            bytes[..2].copy_from_slice(&value.to_le_bytes());
            3
        }
        Value::DInt(value) => {
            bytes[..4].copy_from_slice(&value.to_le_bytes());
            4
        }
        Value::LInt(value) => {
            bytes = value.to_le_bytes();
            5
        }
        Value::USInt(value) => {
            bytes[0] = *value;
            6
        }
        Value::UInt(value) => {
            bytes[..2].copy_from_slice(&value.to_le_bytes());
            7
        }
        Value::UDInt(value) => {
            bytes[..4].copy_from_slice(&value.to_le_bytes());
            8
        }
        Value::ULInt(value) => {
            bytes = value.to_le_bytes();
            9
        }
        Value::Byte(value) => {
            bytes[0] = *value;
            10
        }
        Value::Word(value) => {
            bytes[..2].copy_from_slice(&value.to_le_bytes());
            11
        }
        Value::DWord(value) => {
            bytes[..4].copy_from_slice(&value.to_le_bytes());
            12
        }
        Value::LWord(value) => {
            bytes = value.to_le_bytes();
            13
        }
        Value::Real(value) => {
            bytes[..4].copy_from_slice(&value.to_bits().to_le_bytes());
            14
        }
        Value::LReal(value) => {
            bytes = value.to_bits().to_le_bytes();
            15
        }
        _ => return None,
    };
    Some((code, bytes))
}

/// A text: its 1-byte length and its UTF-8 bytes.
fn read_text(reader: &mut Reader<'_>, field: &'static str) -> Result<SmolStr, Error> {
    let length = usize::from(reader.u8()?);
    let text = core::str::from_utf8(reader.take(length)?)
        .ok()
        .ok_or(Error::Field(field))?;
    Ok(SmolStr::new(text))
}

/// A name: a text the section requires to be present.
fn read_name(reader: &mut Reader<'_>, field: &'static str) -> Result<SmolStr, Error> {
    let name = read_text(reader, field)?;
    if name.is_empty() {
        Err(Error::Field(field))
    } else {
        Ok(name)
    }
}

/// A flag: 0 or 1.
fn read_flag(reader: &mut Reader<'_>, field: &'static str) -> Result<bool, Error> {
    match reader.u8()? {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(Error::Field(field)),
    }
}

/// A task override's single input: a text, empty for none.
fn read_single(reader: &mut Reader<'_>) -> Result<Option<SmolStr>, Error> {
    let single = read_text(reader, "a task's single input")?;
    if single.is_empty() {
        Ok(None)
    } else {
        Ok(Some(single))
    }
}

/// Reserve `count` values of at least `bytes` each, only when the payload can hold them, so that a
/// short payload cannot request a large allocation (LOAD-04).
fn reserve<T>(reader: &Reader<'_>, count: usize, bytes: usize) -> Result<Vec<T>, Error> {
    if count.saturating_mul(bytes) > reader.remaining() {
        return Err(Error::Truncated);
    }
    let mut values = Vec::new();
    values.try_reserve_exact(count).map_err(|_| Error::Memory)?;
    Ok(values)
}

/// The task overrides: their presence byte, and then their count and each override.
fn read_task_overrides(reader: &mut Reader<'_>) -> Result<Option<Vec<TaskOverride>>, Error> {
    match reader.u8()? {
        0 => Ok(None),
        1 => {
            let count = usize::from(reader.u16()?);
            let mut overrides: Vec<TaskOverride> = reserve(reader, count, TASK_OVERRIDE_MIN_BYTES)?;
            for _ in 0..count {
                let name = read_name(reader, "a task name")?;
                let interval = Duration::from_nanos(reader.i64()?);
                let priority = reader.u8()?;
                let single = read_single(reader)?;
                let programs = read_programs(reader)?;
                overrides.push(TaskOverride {
                    name,
                    interval,
                    priority,
                    single,
                    programs,
                });
            }
            Ok(Some(overrides))
        }
        _ => Err(Error::Field("the task overrides")),
    }
}

/// A task override's programs: their count, then each program name.
fn read_programs(reader: &mut Reader<'_>) -> Result<Vec<SmolStr>, Error> {
    let count = usize::from(reader.u16()?);
    // A program name takes its length byte and at least one byte.
    let mut programs: Vec<SmolStr> = reserve(reader, count, 2)?;
    for _ in 0..count {
        programs.push(read_name(reader, "an override program name")?);
    }
    Ok(programs)
}

/// The I/O drivers: their count, then each driver's name and enabled flag; the `gpio` driver is
/// followed by its input and output lines.
fn read_drivers(reader: &mut Reader<'_>) -> Result<Vec<IoDriver>, Error> {
    let count = usize::from(reader.u16()?);
    let mut drivers: Vec<IoDriver> = reserve(reader, count, DRIVER_MIN_BYTES)?;
    for _ in 0..count {
        let name = read_name(reader, "a driver name")?;
        let enabled = read_flag(reader, "the driver enabled flag")?;
        let mut inputs = Vec::new();
        let mut outputs = Vec::new();
        if is_gpio(&name) {
            inputs = read_gpio_inputs(reader)?;
            outputs = read_gpio_outputs(reader)?;
        }
        drivers.push(IoDriver {
            name,
            enabled,
            inputs,
            outputs,
            params: None,
        });
    }
    Ok(drivers)
}

/// The `gpio` driver's inputs: their count, then each input's address, line, invert flag and
/// debounce interval.
fn read_gpio_inputs(reader: &mut Reader<'_>) -> Result<Vec<GpioInput>, Error> {
    let count = usize::from(reader.u16()?);
    let mut inputs: Vec<GpioInput> = reserve(reader, count, GPIO_INPUT_MIN_BYTES)?;
    for _ in 0..count {
        let address = read_address(reader)?;
        let line = read_name(reader, "a GPIO line")?;
        let invert = read_flag(reader, "the GPIO input invert flag")?;
        let debounce_ms = reader.u32()?;
        inputs.push(GpioInput {
            address,
            line,
            invert,
            debounce_ms,
        });
    }
    Ok(inputs)
}

/// The `gpio` driver's outputs: their count, then each output's address, line, invert flag and
/// initial level.
fn read_gpio_outputs(reader: &mut Reader<'_>) -> Result<Vec<GpioOutput>, Error> {
    let count = usize::from(reader.u16()?);
    let mut outputs: Vec<GpioOutput> = reserve(reader, count, GPIO_OUTPUT_MIN_BYTES)?;
    for _ in 0..count {
        let address = read_address(reader)?;
        let line = read_name(reader, "a GPIO line")?;
        let invert = read_flag(reader, "the GPIO output invert flag")?;
        let initial = read_flag(reader, "the GPIO output initial flag")?;
        outputs.push(GpioOutput {
            address,
            line,
            invert,
            initial,
        });
    }
    Ok(outputs)
}

/// The safe state: its count, then each entry's address, value type and value.
fn read_safe_state(reader: &mut Reader<'_>) -> Result<Vec<SafeStateEntry>, Error> {
    let count = usize::from(reader.u16()?);
    let mut entries: Vec<SafeStateEntry> = reserve(reader, count, SAFE_STATE_ENTRY_MIN_BYTES)?;
    for _ in 0..count {
        let address = read_address(reader)?;
        let code = reader.u8()?;
        let value = read_value(code, reader.array::<8>()?)?;
        entries.push(SafeStateEntry { address, value });
    }
    Ok(entries)
}

/// An address: its area, size, byte offset and bit.
fn read_address(reader: &mut Reader<'_>) -> Result<IoAddress, Error> {
    let area = match reader.u8()? {
        1 => IoArea::Input,
        2 => IoArea::Output,
        3 => IoArea::Memory,
        _ => return Err(Error::Field("an I/O address")),
    };
    let size = match reader.u8()? {
        1 => IoSize::Bit,
        2 => IoSize::Byte,
        3 => IoSize::Word,
        4 => IoSize::DWord,
        5 => IoSize::LWord,
        _ => return Err(Error::Field("an I/O address")),
    };
    let byte = reader.u32()?;
    let bit = reader.u8()?;
    if bit > 7 || (bit != 0 && !matches!(size, IoSize::Bit)) {
        return Err(Error::Field("an I/O address"));
    }
    Ok(IoAddress {
        area,
        size,
        byte,
        bit,
        path: alloc::vec![byte],
        wildcard: false,
    })
}

/// The value of a type code and its eight bytes; a code outside the list, or a byte above the
/// type's width that is not 0, is refused.
fn read_value(code: u8, bytes: [u8; 8]) -> Result<Value, Error> {
    let zero_extended = |width: usize| bytes[width..].iter().all(|byte| *byte == 0);
    Ok(match code {
        1 if bytes[0] <= 1 && zero_extended(1) => Value::Bool(bytes[0] == 1),
        2 if zero_extended(1) => Value::SInt(i8::from_le_bytes([bytes[0]])),
        3 if zero_extended(2) => Value::Int(i16::from_le_bytes(first_two(&bytes))),
        4 if zero_extended(4) => Value::DInt(i32::from_le_bytes(first_four(&bytes))),
        5 => Value::LInt(i64::from_le_bytes(bytes)),
        6 if zero_extended(1) => Value::USInt(bytes[0]),
        7 if zero_extended(2) => Value::UInt(u16::from_le_bytes(first_two(&bytes))),
        8 if zero_extended(4) => Value::UDInt(u32::from_le_bytes(first_four(&bytes))),
        9 => Value::ULInt(u64::from_le_bytes(bytes)),
        10 if zero_extended(1) => Value::Byte(bytes[0]),
        11 if zero_extended(2) => Value::Word(u16::from_le_bytes(first_two(&bytes))),
        12 if zero_extended(4) => Value::DWord(u32::from_le_bytes(first_four(&bytes))),
        13 => Value::LWord(u64::from_le_bytes(bytes)),
        14 if zero_extended(4) => {
            Value::Real(f32::from_bits(u32::from_le_bytes(first_four(&bytes))))
        }
        15 => Value::LReal(f64::from_bits(u64::from_le_bytes(bytes))),
        _ => return Err(Error::Field("an I/O safe-state value")),
    })
}

/// The first two of a safe-state value's eight bytes.
fn first_two(bytes: &[u8; 8]) -> [u8; 2] {
    [bytes[0], bytes[1]]
}

/// The first four of a safe-state value's eight bytes.
fn first_four(bytes: &[u8; 8]) -> [u8; 4] {
    [bytes[0], bytes[1], bytes[2], bytes[3]]
}

/// Section 8 (version 2): the parameters of the I/O drivers of `configuration`, in section 7's
/// order: their count, then each driver's text with its four-byte length (specification 34 §5.1.1).
/// A driver without parameters writes `{}`.
pub(super) fn encode_parameters(
    configuration: &Configuration,
    out: &mut Vec<u8>,
) -> Result<(), Error> {
    write_count(out, configuration.io_drivers.len(), "the I/O driver count")?;
    for driver in &configuration.io_drivers {
        let text = driver.params.as_deref().unwrap_or(NO_PARAMETERS);
        let length =
            u32::try_from(text.len()).map_err(|_| Error::Field("a driver's parameters"))?;
        out.extend_from_slice(&length.to_le_bytes());
        out.extend_from_slice(text.as_bytes());
    }
    Ok(())
}

/// The length of section 8's payload in bytes, exactly what [`encode_parameters`] writes.
pub(super) fn parameters_len(configuration: &Configuration) -> usize {
    configuration
        .io_drivers
        .iter()
        .fold(2usize, |bytes, driver| {
            bytes.saturating_add(4 + driver.params.as_deref().unwrap_or(NO_PARAMETERS).len())
        })
}

/// Reads section 8 into `drivers`, the drivers section 7 read: the count must be theirs, and each
/// text UTF-8. Whether a text is a JSON object of the driver's keys is the driver's to judge.
pub(super) fn read_parameters(
    reader: &mut Reader<'_>,
    drivers: &mut [IoDriver],
) -> Result<(), Error> {
    if usize::from(reader.u16()?) != drivers.len() {
        return Err(Error::Field("the driver parameters' count"));
    }
    for driver in drivers {
        let length = reader.u32()? as usize;
        let text = core::str::from_utf8(reader.take(length)?)
            .map_err(|_| Error::Field("a driver's parameters"))?;
        driver.params = Some(String::from(text));
    }
    Ok(())
}

/// The parameters of a driver that has none.
const NO_PARAMETERS: &str = "{}";
