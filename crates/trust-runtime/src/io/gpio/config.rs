//! The `gpio` driver's configuration: its backend selection and its input and output entries,
//! parsed from the driver's `io.params` (specification 11 §6.8.1d). The deployment record's
//! configuration section reads the same parse, so a chip receives the settings a computer reads
//! (specification 34 §5.1.1, TARGET-01).

use std::collections::HashSet;
use std::path::PathBuf;

use smol_str::SmolStr;
use trust_runtime_core::deployment::{GpioInput as RecordInput, GpioOutput as RecordOutput};

use super::{invalid_gpio, map_bit, GpioDriver};

use crate::error::RuntimeError;
use crate::io::{IoAddress, IoSize};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum GpioBackendKind {
    Sysfs,
    Chardev,
}

#[derive(Debug)]
pub(super) struct GpioConfig {
    pub(super) backend: GpioBackendKind,
    pub(super) sysfs_base: PathBuf,
    pub(super) chip_path: PathBuf,
    pub(super) inputs: Vec<GpioInputEntry>,
    pub(super) outputs: Vec<GpioOutputEntry>,
}

#[derive(Debug)]
pub(super) struct GpioInputEntry {
    pub(super) address: IoAddress,
    pub(super) line: u32,
    pub(super) invert: bool,
    pub(super) debounce_ms: u64,
}

#[derive(Debug)]
pub(super) struct GpioOutputEntry {
    pub(super) address: IoAddress,
    pub(super) line: u32,
    pub(super) invert: bool,
    pub(super) initial: bool,
}

impl GpioConfig {
    pub(super) fn parse(params: &toml::Value) -> Result<Self, RuntimeError> {
        let table = params
            .as_table()
            .ok_or_else(|| invalid_gpio("io.params must be a table"))?;
        let backend = match table.get("backend") {
            None => GpioBackendKind::Chardev,
            Some(toml::Value::String(name)) if name.eq_ignore_ascii_case("sysfs") => {
                GpioBackendKind::Sysfs
            }
            Some(toml::Value::String(name))
                if name.eq_ignore_ascii_case("libgpiod")
                    || name.eq_ignore_ascii_case("chardev") =>
            {
                GpioBackendKind::Chardev
            }
            Some(toml::Value::String(name)) => {
                return Err(invalid_gpio(format!("unsupported gpio backend '{name}'")))
            }
            Some(_) => {
                return Err(invalid_gpio(
                    "unsupported gpio backend value; expected a string",
                ))
            }
        };
        let sysfs_base = table
            .get("sysfs_base")
            .and_then(|v| v.as_str())
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("/sys/class/gpio"));
        let chip_path = table
            .get("chip")
            .and_then(|v| v.as_str())
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("/dev/gpiochip0"));

        let inputs = parse_gpio_inputs(table.get("inputs"))?;
        let outputs = parse_gpio_outputs(table.get("outputs"))?;
        ensure_unique_gpio_mappings(&inputs, &outputs)?;

        Ok(Self {
            backend,
            sysfs_base,
            chip_path,
            inputs,
            outputs,
        })
    }
}

impl GpioDriver {
    /// The `gpio` driver's inputs and outputs as the deployment record's configuration section
    /// carries them (specification 34 §5.1.1, section 7): the address, the line exactly as
    /// `io.toml` writes it — a number's decimal digits, such as `17`, or a pin name, such as
    /// `PD12` (TARGET-01) — the invert flag, and the debounce interval or the initial level.
    ///
    /// The computer's own driver refuses a pin name; the record keeps it for a chip, so this
    /// parse accepts both forms. The address checks are the driver's.
    ///
    /// # Errors
    ///
    /// The parameters are not the shape the driver accepts, or an address is not a concrete
    /// `%IX`/`%QX` bit.
    pub(crate) fn configuration_lines(
        params: &toml::Value,
    ) -> Result<(Vec<RecordInput>, Vec<RecordOutput>), RuntimeError> {
        let table = params
            .as_table()
            .ok_or_else(|| invalid_gpio("io.params must be a table"))?;
        Ok((
            configuration_inputs(table.get("inputs"))?,
            configuration_outputs(table.get("outputs"))?,
        ))
    }
}

fn parse_gpio_inputs(value: Option<&toml::Value>) -> Result<Vec<GpioInputEntry>, RuntimeError> {
    let Some(value) = value else {
        return Ok(Vec::new());
    };
    let list = value
        .as_array()
        .ok_or_else(|| invalid_gpio("io.params.inputs must be an array"))?;
    let mut entries = Vec::new();
    for entry in list {
        let table = entry
            .as_table()
            .ok_or_else(|| invalid_gpio("gpio input entry must be a table"))?;
        let address = parse_address(table, "address")?;
        ensure_input_address(&address)?;
        let line = parse_line(table)?;
        let invert = parse_bool(table, "invert")?.unwrap_or(false);
        let debounce_ms = parse_u64(table, "debounce_ms")?.unwrap_or(0);
        entries.push(GpioInputEntry {
            address,
            line,
            invert,
            debounce_ms,
        });
    }
    Ok(entries)
}

fn parse_gpio_outputs(value: Option<&toml::Value>) -> Result<Vec<GpioOutputEntry>, RuntimeError> {
    let Some(value) = value else {
        return Ok(Vec::new());
    };
    let list = value
        .as_array()
        .ok_or_else(|| invalid_gpio("io.params.outputs must be an array"))?;
    let mut entries = Vec::new();
    for entry in list {
        let table = entry
            .as_table()
            .ok_or_else(|| invalid_gpio("gpio output entry must be a table"))?;
        let address = parse_address(table, "address")?;
        ensure_output_address(&address)?;
        let line = parse_line(table)?;
        let invert = parse_bool(table, "invert")?.unwrap_or(false);
        let initial = parse_bool(table, "initial")?.unwrap_or(false);
        entries.push(GpioOutputEntry {
            address,
            line,
            invert,
            initial,
        });
    }
    Ok(entries)
}

fn parse_address(table: &toml::Table, key: &str) -> Result<IoAddress, RuntimeError> {
    let text = table
        .get(key)
        .and_then(|v| v.as_str())
        .ok_or_else(|| invalid_gpio(format!("gpio entry missing '{key}'")))?;
    IoAddress::parse(text)
}

fn parse_line(table: &toml::Table) -> Result<u32, RuntimeError> {
    let (key, value) = line_value(table)?;
    match value {
        toml::Value::Integer(line) => u32::try_from(*line).map_err(|_| line_error(key)),
        toml::Value::String(name) => Err(invalid_gpio(format!(
            "gpio entry requires 'line' (BCM) as a nonnegative u32: on this computer a line is a \
             number, and '{name}' names a chip's pin"
        ))),
        _ => Err(line_error(key)),
    }
}

/// The refusal of a `gpio` entry whose `line` is not a nonnegative number.
fn line_error(key: &str) -> RuntimeError {
    invalid_gpio(format!(
        "gpio entry requires '{key}' (BCM) as a nonnegative u32"
    ))
}

/// A GPIO entry's `line`, or the `pin` alias used only when `line` is absent (specification 11
/// §6.8.1d).
fn line_value(table: &toml::Table) -> Result<(&'static str, &toml::Value), RuntimeError> {
    if let Some(value) = table.get("line") {
        return Ok(("line", value));
    }
    if let Some(value) = table.get("pin") {
        return Ok(("pin", value));
    }
    Err(invalid_gpio("gpio entry requires 'line' (BCM)"))
}

/// A GPIO entry's line as the deployment record's configuration section carries it
/// (specification 34 §5.1.1, TARGET-01): a number's decimal digits, such as `17`, or a pin
/// name, such as `PD12` or `GPIO18`.
fn parse_line_text(table: &toml::Table) -> Result<SmolStr, RuntimeError> {
    let (key, value) = line_value(table)?;
    match value {
        toml::Value::Integer(line) => u32::try_from(*line)
            .map(|line| SmolStr::new(line.to_string()))
            .map_err(|_| line_error(key)),
        toml::Value::String(name) if !name.is_empty() => Ok(SmolStr::new(name.as_str())),
        _ => Err(line_error(key)),
    }
}

/// The `gpio` driver's inputs as section 7 carries them (specification 34 §5.1.1): each entry's
/// address, line, invert flag and debounce interval. The line keeps the pin name the computer's
/// driver refuses, so a chip can receive it (TARGET-01).
fn configuration_inputs(value: Option<&toml::Value>) -> Result<Vec<RecordInput>, RuntimeError> {
    let Some(value) = value else {
        return Ok(Vec::new());
    };
    let list = value
        .as_array()
        .ok_or_else(|| invalid_gpio("io.params.inputs must be an array"))?;
    let mut entries = Vec::new();
    for entry in list {
        let table = entry
            .as_table()
            .ok_or_else(|| invalid_gpio("gpio input entry must be a table"))?;
        let address = parse_address(table, "address")?;
        ensure_input_address(&address)?;
        let line = parse_line_text(table)?;
        let invert = parse_bool(table, "invert")?.unwrap_or(false);
        let debounce_ms = u32::try_from(parse_u64(table, "debounce_ms")?.unwrap_or(0))
            .map_err(|_| invalid_gpio("gpio debounce_ms must fit a u32"))?;
        entries.push(RecordInput {
            address,
            line,
            invert,
            debounce_ms,
        });
    }
    Ok(entries)
}

/// The `gpio` driver's outputs as section 7 carries them (specification 34 §5.1.1): each entry's
/// address, line, invert flag and initial level. The line keeps the pin name the computer's
/// driver refuses, so a chip can receive it (TARGET-01).
fn configuration_outputs(value: Option<&toml::Value>) -> Result<Vec<RecordOutput>, RuntimeError> {
    let Some(value) = value else {
        return Ok(Vec::new());
    };
    let list = value
        .as_array()
        .ok_or_else(|| invalid_gpio("io.params.outputs must be an array"))?;
    let mut entries = Vec::new();
    for entry in list {
        let table = entry
            .as_table()
            .ok_or_else(|| invalid_gpio("gpio output entry must be a table"))?;
        let address = parse_address(table, "address")?;
        ensure_output_address(&address)?;
        let line = parse_line_text(table)?;
        let invert = parse_bool(table, "invert")?.unwrap_or(false);
        let initial = parse_bool(table, "initial")?.unwrap_or(false);
        entries.push(RecordOutput {
            address,
            line,
            invert,
            initial,
        });
    }
    Ok(entries)
}

fn parse_bool(table: &toml::Table, key: &str) -> Result<Option<bool>, RuntimeError> {
    match table.get(key) {
        None => Ok(None),
        Some(value) => match value {
            toml::Value::Boolean(flag) => Ok(Some(*flag)),
            toml::Value::Integer(num) => Ok(Some(*num != 0)),
            toml::Value::String(text) => match text.trim().to_ascii_lowercase().as_str() {
                "true" | "1" => Ok(Some(true)),
                "false" | "0" => Ok(Some(false)),
                _ => Err(invalid_gpio(format!("invalid bool '{text}' for {key}"))),
            },
            _ => Err(invalid_gpio(format!("invalid type for {key}"))),
        },
    }
}

fn parse_u64(table: &toml::Table, key: &str) -> Result<Option<u64>, RuntimeError> {
    match table.get(key) {
        None => Ok(None),
        Some(value) => match value {
            toml::Value::Integer(num) if *num >= 0 => Ok(Some(*num as u64)),
            toml::Value::String(text) => text
                .trim()
                .parse::<u64>()
                .map(Some)
                .map_err(|_| invalid_gpio(format!("invalid numeric '{text}' for {key}"))),
            _ => Err(invalid_gpio(format!("invalid type for {key}"))),
        },
    }
}

fn ensure_input_address(address: &IoAddress) -> Result<(), RuntimeError> {
    if address.wildcard {
        return Err(invalid_gpio("gpio input address cannot be wildcard"));
    }
    if address.size != IoSize::Bit {
        return Err(invalid_gpio("gpio input address must be bit (%IX...)"));
    }
    if !matches!(address.area, crate::memory::IoArea::Input) {
        return Err(invalid_gpio("gpio input address must be %I"));
    }
    map_bit(address)?;
    Ok(())
}

fn ensure_output_address(address: &IoAddress) -> Result<(), RuntimeError> {
    if address.wildcard {
        return Err(invalid_gpio("gpio output address cannot be wildcard"));
    }
    if address.size != IoSize::Bit {
        return Err(invalid_gpio("gpio output address must be bit (%QX...)"));
    }
    if !matches!(address.area, crate::memory::IoArea::Output) {
        return Err(invalid_gpio("gpio output address must be %Q"));
    }
    map_bit(address)?;
    Ok(())
}

fn ensure_unique_gpio_mappings(
    inputs: &[GpioInputEntry],
    outputs: &[GpioOutputEntry],
) -> Result<(), RuntimeError> {
    let mut physical_lines = HashSet::new();
    let mut input_bits = HashSet::new();
    let mut output_bits = HashSet::new();

    for entry in inputs {
        if !physical_lines.insert(entry.line) {
            return Err(invalid_gpio(format!(
                "gpio physical line {} is mapped more than once",
                entry.line
            )));
        }
        if !input_bits.insert((entry.address.byte, entry.address.bit)) {
            return Err(invalid_gpio(format!(
                "gpio input process-image bit %IX{}.{} is mapped more than once",
                entry.address.byte, entry.address.bit
            )));
        }
    }
    for entry in outputs {
        if !physical_lines.insert(entry.line) {
            return Err(invalid_gpio(format!(
                "gpio physical line {} is mapped more than once",
                entry.line
            )));
        }
        if !output_bits.insert((entry.address.byte, entry.address.bit)) {
            return Err(invalid_gpio(format!(
                "gpio output process-image bit %QX{}.{} is mapped more than once",
                entry.address.byte, entry.address.bit
            )));
        }
    }
    Ok(())
}
