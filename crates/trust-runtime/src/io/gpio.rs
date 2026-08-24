//! Raspberry Pi GPIO driver (configurable backend).

use std::collections::HashSet;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Instant;

use smol_str::SmolStr;
#[cfg(target_os = "linux")]
use std::collections::HashMap;

use crate::error::RuntimeError;
use crate::io::{IoAddress, IoDriver, IoDriverHealth, IoSize};

pub struct GpioDriver {
    backend: Box<dyn GpioBackend>,
    inputs: Vec<GpioInput>,
    outputs: Vec<GpioOutput>,
    health: IoDriverHealth,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GpioLineInfo {
    pub chip: String,
    pub label: Option<String>,
    pub line: u32,
    pub direction: Option<String>,
}

impl std::fmt::Debug for GpioDriver {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GpioDriver")
            .field("inputs", &self.inputs.len())
            .field("outputs", &self.outputs.len())
            .finish()
    }
}

impl GpioDriver {
    pub fn from_params(params: &toml::Value) -> Result<Self, RuntimeError> {
        let config = GpioConfig::parse(params)?;
        let mut backend: Box<dyn GpioBackend> = match config.backend {
            GpioBackendKind::Sysfs => Box::new(SysfsBackend::new(config.sysfs_base)),
            GpioBackendKind::Chardev => make_chardev_backend(config.chip_path),
        };

        let mut inputs = Vec::new();
        let mut outputs = Vec::new();
        for entry in config.inputs {
            backend.configure_input(entry.line)?;
            inputs.push(GpioInput::from_entry(entry)?);
        }
        for entry in config.outputs {
            backend.configure_output(entry.line, entry.initial)?;
            outputs.push(GpioOutput::from_entry(entry)?);
        }

        Ok(Self {
            backend,
            inputs,
            outputs,
            health: IoDriverHealth::Ok,
        })
    }

    pub fn validate_params(params: &toml::Value) -> Result<(), RuntimeError> {
        let _ = GpioConfig::parse(params)?;
        Ok(())
    }
}

pub fn discover_gpio_lines() -> Result<Vec<GpioLineInfo>, RuntimeError> {
    let mut lines = Vec::new();
    collect_gpiochip_lines(Path::new("/sys/bus/gpio/devices"), &mut lines)?;
    if lines.is_empty() {
        collect_gpiochip_lines(Path::new("/sys/class/gpio"), &mut lines)?;
    }
    lines.sort_by(|left, right| left.chip.cmp(&right.chip).then(left.line.cmp(&right.line)));
    lines.dedup_by(|left, right| left.chip == right.chip && left.line == right.line);
    Ok(lines)
}

impl IoDriver for GpioDriver {
    fn read_inputs(&mut self, inputs: &mut [u8]) -> Result<(), RuntimeError> {
        let now = Instant::now();
        for entry in &mut self.inputs {
            let raw = match self.backend.read(entry.line) {
                Ok(value) => value,
                Err(err) => {
                    self.health = IoDriverHealth::Faulted {
                        error: SmolStr::new(err.to_string()),
                    };
                    return Err(err);
                }
            };
            let mut value = if entry.invert { !raw } else { raw };
            if entry.debounce_ms > 0 {
                match entry.last_change {
                    None => {
                        entry.last_change = Some(now);
                        entry.last_state = value;
                    }
                    Some(last) => {
                        if value != entry.last_state {
                            let elapsed = now.duration_since(last);
                            if elapsed.as_millis() >= entry.debounce_ms as u128 {
                                entry.last_state = value;
                                entry.last_change = Some(now);
                            } else {
                                value = entry.last_state;
                            }
                        }
                    }
                }
            }
            if let Err(err) = write_bit(inputs, entry.byte, entry.bit, value) {
                self.health = IoDriverHealth::Faulted {
                    error: SmolStr::new(err.to_string()),
                };
                return Err(err);
            }
        }
        self.health = IoDriverHealth::Ok;
        Ok(())
    }

    fn write_outputs(&mut self, outputs: &[u8]) -> Result<(), RuntimeError> {
        for entry in &mut self.outputs {
            let raw = match read_bit(outputs, entry.byte, entry.bit) {
                Ok(value) => value,
                Err(err) => {
                    self.health = IoDriverHealth::Faulted {
                        error: SmolStr::new(err.to_string()),
                    };
                    return Err(err);
                }
            };
            let value = if entry.invert { !raw } else { raw };
            if entry.last_written != Some(value) {
                if let Err(err) = self.backend.write(entry.line, value) {
                    self.health = IoDriverHealth::Faulted {
                        error: SmolStr::new(err.to_string()),
                    };
                    return Err(err);
                }
                entry.last_written = Some(value);
            }
        }
        self.health = IoDriverHealth::Ok;
        Ok(())
    }

    fn health(&self) -> IoDriverHealth {
        self.health.clone()
    }
}

#[derive(Debug)]
struct GpioInput {
    line: u32,
    byte: usize,
    bit: u8,
    invert: bool,
    debounce_ms: u64,
    last_state: bool,
    last_change: Option<Instant>,
}

impl GpioInput {
    fn from_entry(entry: GpioInputEntry) -> Result<Self, RuntimeError> {
        let (byte, bit) = map_bit(&entry.address)?;
        Ok(Self {
            line: entry.line,
            byte,
            bit,
            invert: entry.invert,
            debounce_ms: entry.debounce_ms,
            last_state: false,
            last_change: None,
        })
    }
}

#[derive(Debug)]
struct GpioOutput {
    line: u32,
    byte: usize,
    bit: u8,
    invert: bool,
    last_written: Option<bool>,
}

impl GpioOutput {
    fn from_entry(entry: GpioOutputEntry) -> Result<Self, RuntimeError> {
        let (byte, bit) = map_bit(&entry.address)?;
        Ok(Self {
            line: entry.line,
            byte,
            bit,
            invert: entry.invert,
            last_written: None,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GpioBackendKind {
    Sysfs,
    Chardev,
}

#[derive(Debug)]
struct GpioConfig {
    backend: GpioBackendKind,
    sysfs_base: PathBuf,
    chip_path: PathBuf,
    inputs: Vec<GpioInputEntry>,
    outputs: Vec<GpioOutputEntry>,
}

#[derive(Debug)]
struct GpioInputEntry {
    address: IoAddress,
    line: u32,
    invert: bool,
    debounce_ms: u64,
}

#[derive(Debug)]
struct GpioOutputEntry {
    address: IoAddress,
    line: u32,
    invert: bool,
    initial: bool,
}

impl GpioConfig {
    fn parse(params: &toml::Value) -> Result<Self, RuntimeError> {
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
    if let Some(value) = table.get("line") {
        let line = value
            .as_integer()
            .and_then(|line| u32::try_from(line).ok())
            .ok_or_else(|| invalid_gpio("gpio entry requires 'line' (BCM) as a nonnegative u32"))?;
        return Ok(line);
    }
    if let Some(value) = table.get("pin") {
        let line = value
            .as_integer()
            .and_then(|line| u32::try_from(line).ok())
            .ok_or_else(|| invalid_gpio("gpio entry requires 'pin' alias as a nonnegative u32"))?;
        return Ok(line);
    }
    Err(invalid_gpio("gpio entry requires 'line' (BCM)"))
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

fn map_bit(address: &IoAddress) -> Result<(usize, u8), RuntimeError> {
    if address.path.len() != 1 {
        return Err(invalid_gpio(
            "gpio address must be a simple bit address (no nested path)",
        ));
    }
    Ok((address.byte as usize, address.bit))
}

fn read_bit(buffer: &[u8], byte: usize, bit: u8) -> Result<bool, RuntimeError> {
    if bit > 7 {
        return Err(invalid_gpio("gpio bit index must be between 0 and 7"));
    }
    let Some(byte_val) = buffer.get(byte) else {
        return Err(invalid_gpio("gpio mapping outside output buffer"));
    };
    Ok((byte_val & (1 << bit)) != 0)
}

fn write_bit(buffer: &mut [u8], byte: usize, bit: u8, value: bool) -> Result<(), RuntimeError> {
    if bit > 7 {
        return Err(invalid_gpio("gpio bit index must be between 0 and 7"));
    }
    let Some(byte_val) = buffer.get_mut(byte) else {
        return Err(invalid_gpio("gpio mapping outside input buffer"));
    };
    if value {
        *byte_val |= 1 << bit;
    } else {
        *byte_val &= !(1 << bit);
    }
    Ok(())
}

fn invalid_gpio(msg: impl Into<String>) -> RuntimeError {
    RuntimeError::InvalidConfig(SmolStr::new(msg.into()))
}

fn collect_gpiochip_lines(root: &Path, lines: &mut Vec<GpioLineInfo>) -> Result<(), RuntimeError> {
    let entries = match fs::read_dir(root) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => {
            return Err(RuntimeError::IoDriver(SmolStr::new(format!(
                "gpio enumerate {} failed: {error}",
                root.display()
            ))))
        }
    };
    for entry in entries {
        let entry = entry.map_err(|error| {
            RuntimeError::IoDriver(SmolStr::new(format!(
                "gpio enumerate {} failed: {error}",
                root.display()
            )))
        })?;
        let path = entry.path();
        let Some(file_name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if !file_name.starts_with("gpiochip") {
            continue;
        }
        let base = read_u32(path.join("base").as_path()).unwrap_or(0);
        let ngpio = read_u32(path.join("ngpio").as_path()).unwrap_or(0);
        if ngpio == 0 {
            continue;
        }
        let label = read_optional_trimmed(path.join("label").as_path());
        for offset in 0..ngpio.min(512) {
            let line = base.saturating_add(offset);
            lines.push(GpioLineInfo {
                chip: file_name.to_string(),
                label: label.clone(),
                line,
                direction: read_optional_trimmed(
                    Path::new("/sys/class/gpio")
                        .join(format!("gpio{line}"))
                        .join("direction")
                        .as_path(),
                ),
            });
        }
    }
    Ok(())
}

fn read_u32(path: &Path) -> Option<u32> {
    fs::read_to_string(path).ok()?.trim().parse().ok()
}

fn read_optional_trimmed(path: &Path) -> Option<String> {
    fs::read_to_string(path)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

trait GpioBackend: Send {
    fn configure_input(&mut self, line: u32) -> Result<(), RuntimeError>;
    fn configure_output(&mut self, line: u32, initial: bool) -> Result<(), RuntimeError>;
    fn read(&mut self, line: u32) -> Result<bool, RuntimeError>;
    fn write(&mut self, line: u32, value: bool) -> Result<(), RuntimeError>;
}

#[derive(Debug)]
struct SysfsBackend {
    base: PathBuf,
}

impl SysfsBackend {
    fn new(base: PathBuf) -> Self {
        Self { base }
    }

    fn gpio_path(&self, line: u32, leaf: &str) -> PathBuf {
        self.base.join(format!("gpio{line}")).join(leaf)
    }

    fn ensure_exported(&self, line: u32) -> Result<(), RuntimeError> {
        let line_path = self.base.join(format!("gpio{line}"));
        if line_path.exists() {
            return Ok(());
        }
        let export_path = self.base.join("export");
        fs::write(&export_path, line.to_string()).map_err(|err| {
            RuntimeError::IoDriver(SmolStr::new(format!("gpio export {line} failed: {err}")))
        })?;
        Ok(())
    }

    fn write_path(&self, path: &Path, value: &str) -> Result<(), RuntimeError> {
        let mut file = fs::OpenOptions::new()
            .write(true)
            .open(path)
            .map_err(|err| {
                RuntimeError::IoDriver(SmolStr::new(format!("gpio write {path:?} failed: {err}")))
            })?;
        file.write_all(value.as_bytes()).map_err(|err| {
            RuntimeError::IoDriver(SmolStr::new(format!("gpio write {path:?} failed: {err}")))
        })
    }

    fn read_path(&self, path: &Path) -> Result<String, RuntimeError> {
        fs::read_to_string(path).map_err(|err| {
            RuntimeError::IoDriver(SmolStr::new(format!("gpio read {path:?} failed: {err}")))
        })
    }
}

impl GpioBackend for SysfsBackend {
    fn configure_input(&mut self, line: u32) -> Result<(), RuntimeError> {
        self.ensure_exported(line)?;
        let dir_path = self.gpio_path(line, "direction");
        self.write_path(&dir_path, "in")?;
        Ok(())
    }

    fn configure_output(&mut self, line: u32, initial: bool) -> Result<(), RuntimeError> {
        self.ensure_exported(line)?;
        let dir_path = self.gpio_path(line, "direction");
        self.write_path(&dir_path, "out")?;
        let value_path = self.gpio_path(line, "value");
        self.write_path(&value_path, if initial { "1" } else { "0" })?;
        Ok(())
    }

    fn read(&mut self, line: u32) -> Result<bool, RuntimeError> {
        let value_path = self.gpio_path(line, "value");
        let text = self.read_path(&value_path)?;
        match text.trim() {
            "0" => Ok(false),
            "1" => Ok(true),
            value => Err(RuntimeError::IoDriver(SmolStr::new(format!(
                "invalid gpio value '{value}' for line {line}; expected 0 or 1"
            )))),
        }
    }

    fn write(&mut self, line: u32, value: bool) -> Result<(), RuntimeError> {
        let value_path = self.gpio_path(line, "value");
        self.write_path(&value_path, if value { "1" } else { "0" })
    }
}

#[cfg(target_os = "linux")]
type ChardevLineHandle = gpio_cdev::LineHandle;

#[cfg(target_os = "linux")]
fn make_chardev_backend(chip_path: PathBuf) -> Box<dyn GpioBackend> {
    Box::new(ChardevBackend::new(chip_path))
}

#[cfg(not(target_os = "linux"))]
fn make_chardev_backend(chip_path: PathBuf) -> Box<dyn GpioBackend> {
    Box::new(UnsupportedChardevBackend { chip_path })
}

#[cfg(target_os = "linux")]
#[derive(Debug)]
struct ChardevBackend {
    chip_path: PathBuf,
    inputs: HashMap<u32, ChardevLineHandle>,
    outputs: HashMap<u32, ChardevLineHandle>,
}

#[cfg(target_os = "linux")]
impl ChardevBackend {
    fn new(chip_path: PathBuf) -> Self {
        Self {
            chip_path,
            inputs: HashMap::new(),
            outputs: HashMap::new(),
        }
    }

    fn request(
        &self,
        line: u32,
        flags: gpio_cdev::LineRequestFlags,
        initial: bool,
    ) -> Result<ChardevLineHandle, RuntimeError> {
        let mut chip = gpio_cdev::Chip::new(&self.chip_path).map_err(|err| {
            RuntimeError::IoDriver(SmolStr::new(format!(
                "gpio chardev open {} failed: {err}",
                self.chip_path.display()
            )))
        })?;
        chip.get_line(line)
            .and_then(|line_handle| line_handle.request(flags, u8::from(initial), "trust-runtime"))
            .map_err(|err| {
                RuntimeError::IoDriver(SmolStr::new(format!(
                    "gpio chardev request line {line} on {} failed: {err}",
                    self.chip_path.display()
                )))
            })
    }
}

#[cfg(target_os = "linux")]
impl GpioBackend for ChardevBackend {
    fn configure_input(&mut self, line: u32) -> Result<(), RuntimeError> {
        if self.inputs.contains_key(&line) {
            return Ok(());
        }
        let handle = self.request(line, gpio_cdev::LineRequestFlags::INPUT, false)?;
        self.inputs.insert(line, handle);
        Ok(())
    }

    fn configure_output(&mut self, line: u32, initial: bool) -> Result<(), RuntimeError> {
        if self.outputs.contains_key(&line) {
            return Ok(());
        }
        let handle = self.request(line, gpio_cdev::LineRequestFlags::OUTPUT, initial)?;
        self.outputs.insert(line, handle);
        Ok(())
    }

    fn read(&mut self, line: u32) -> Result<bool, RuntimeError> {
        let handle = self
            .inputs
            .get(&line)
            .or_else(|| self.outputs.get(&line))
            .ok_or_else(|| {
                RuntimeError::IoDriver(SmolStr::new(format!(
                    "gpio chardev line {line} was not requested"
                )))
            })?;
        handle.get_value().map(|value| value != 0).map_err(|err| {
            RuntimeError::IoDriver(SmolStr::new(format!(
                "gpio chardev read line {line} failed: {err}"
            )))
        })
    }

    fn write(&mut self, line: u32, value: bool) -> Result<(), RuntimeError> {
        let handle = self.outputs.get(&line).ok_or_else(|| {
            RuntimeError::IoDriver(SmolStr::new(format!(
                "gpio chardev output line {line} was not requested"
            )))
        })?;
        handle.set_value(u8::from(value)).map_err(|err| {
            RuntimeError::IoDriver(SmolStr::new(format!(
                "gpio chardev write line {line} failed: {err}"
            )))
        })
    }
}

#[cfg(not(target_os = "linux"))]
#[derive(Debug)]
struct UnsupportedChardevBackend {
    chip_path: PathBuf,
}

#[cfg(not(target_os = "linux"))]
impl GpioBackend for UnsupportedChardevBackend {
    fn configure_input(&mut self, _line: u32) -> Result<(), RuntimeError> {
        Err(chardev_unsupported(&self.chip_path))
    }

    fn configure_output(&mut self, _line: u32, _initial: bool) -> Result<(), RuntimeError> {
        Err(chardev_unsupported(&self.chip_path))
    }

    fn read(&mut self, _line: u32) -> Result<bool, RuntimeError> {
        Err(chardev_unsupported(&self.chip_path))
    }

    fn write(&mut self, _line: u32, _value: bool) -> Result<(), RuntimeError> {
        Err(chardev_unsupported(&self.chip_path))
    }
}

#[cfg(not(target_os = "linux"))]
fn chardev_unsupported(chip_path: &Path) -> RuntimeError {
    RuntimeError::IoDriver(SmolStr::new(format!(
        "gpio libgpiod/chardev backend requires Linux GPIO character devices; {} is not available on this platform",
        chip_path.display()
    )))
}

#[cfg(test)]
mod tests {
    use super::*;
    use smol_str::SmolStr;
    use std::sync::{Arc, Mutex};

    #[test]
    fn parse_gpio_config_accepts_basic_inputs() {
        let params: toml::Value = toml::from_str(
            r#"
backend = "libgpiod"
chip = "/dev/gpiochip0"
inputs = [ { address = "%IX0.0", line = 17, invert = true, debounce_ms = 10 } ]
outputs = [ { address = "%QX0.1", line = 27, invert = false, initial = true } ]
"#,
        )
        .unwrap();
        let config = GpioConfig::parse(&params).expect("config");
        assert_eq!(config.inputs.len(), 1);
        assert_eq!(config.outputs.len(), 1);
        assert_eq!(config.backend, GpioBackendKind::Chardev);
        assert_eq!(config.chip_path, PathBuf::from("/dev/gpiochip0"));
    }

    #[test]
    fn parse_gpio_config_keeps_sysfs_selectable_for_legacy_hosts() {
        let params: toml::Value = toml::from_str(
            r#"
backend = "sysfs"
sysfs_base = "/tmp/gpio"
"#,
        )
        .unwrap();
        let config = GpioConfig::parse(&params).expect("config");
        assert_eq!(config.backend, GpioBackendKind::Sysfs);
        assert_eq!(config.sysfs_base, PathBuf::from("/tmp/gpio"));
    }

    #[test]
    fn rejects_non_bit_addresses() {
        let params: toml::Value =
            toml::from_str(r#"inputs = [ { address = "%IW0", line = 5 } ]"#).unwrap();
        let err = GpioConfig::parse(&params).unwrap_err();
        assert!(format!("{err}").contains("bit"));
    }

    #[derive(Default)]
    struct FailingBackend {
        fail_read: bool,
        fail_write: bool,
    }

    impl GpioBackend for FailingBackend {
        fn configure_input(&mut self, _line: u32) -> Result<(), RuntimeError> {
            Ok(())
        }

        fn configure_output(&mut self, _line: u32, _initial: bool) -> Result<(), RuntimeError> {
            Ok(())
        }

        fn read(&mut self, _line: u32) -> Result<bool, RuntimeError> {
            if self.fail_read {
                return Err(RuntimeError::IoDriver(SmolStr::new("gpio read failed")));
            }
            Ok(true)
        }

        fn write(&mut self, _line: u32, _value: bool) -> Result<(), RuntimeError> {
            if self.fail_write {
                return Err(RuntimeError::IoDriver(SmolStr::new("gpio write failed")));
            }
            Ok(())
        }
    }

    fn driver_with_backend(backend: FailingBackend) -> GpioDriver {
        GpioDriver {
            backend: Box::new(backend),
            inputs: vec![GpioInput {
                line: 17,
                byte: 0,
                bit: 0,
                invert: false,
                debounce_ms: 0,
                last_state: false,
                last_change: None,
            }],
            outputs: vec![GpioOutput {
                line: 27,
                byte: 0,
                bit: 0,
                invert: false,
                last_written: None,
            }],
            health: IoDriverHealth::Ok,
        }
    }

    #[test]
    fn gpio_read_failure_updates_driver_health() {
        let mut driver = driver_with_backend(FailingBackend {
            fail_read: true,
            fail_write: false,
        });
        let mut inputs = [0u8; 1];

        let err = driver
            .read_inputs(&mut inputs)
            .expect_err("GPIO read failure must be returned");
        assert!(err.to_string().contains("gpio read"));
        assert!(
            !matches!(driver.health(), IoDriverHealth::Ok),
            "GPIO health must report the last read failure"
        );
    }

    #[test]
    fn gpio_write_failure_updates_driver_health() {
        let mut driver = driver_with_backend(FailingBackend {
            fail_read: false,
            fail_write: true,
        });

        let err = driver
            .write_outputs(&[1])
            .expect_err("GPIO write failure must be returned");
        assert!(err.to_string().contains("gpio write"));
        assert!(
            !matches!(driver.health(), IoDriverHealth::Ok),
            "GPIO health must report the last write failure"
        );
    }

    struct RecordingBackend {
        writes: Arc<Mutex<Vec<(u32, bool)>>>,
    }

    impl GpioBackend for RecordingBackend {
        fn configure_input(&mut self, _line: u32) -> Result<(), RuntimeError> {
            Ok(())
        }

        fn configure_output(&mut self, _line: u32, _initial: bool) -> Result<(), RuntimeError> {
            Ok(())
        }

        fn read(&mut self, _line: u32) -> Result<bool, RuntimeError> {
            Ok(false)
        }

        fn write(&mut self, line: u32, value: bool) -> Result<(), RuntimeError> {
            self.writes
                .lock()
                .expect("recording backend lock")
                .push((line, value));
            Ok(())
        }
    }

    #[test]
    fn gpio_safe_state_handoff_writes_and_confirms_backend_output() {
        let writes = Arc::new(Mutex::new(Vec::new()));
        let driver = GpioDriver {
            backend: Box::new(RecordingBackend {
                writes: Arc::clone(&writes),
            }),
            inputs: Vec::new(),
            outputs: vec![GpioOutput {
                line: 27,
                byte: 0,
                bit: 0,
                invert: false,
                last_written: None,
            }],
            health: IoDriverHealth::Ok,
        };
        let mut runtime = crate::Runtime::new();
        runtime.io_mut().resize(0, 1, 0);
        runtime.add_io_driver("gpio", Box::new(driver));
        runtime.set_io_safe_state(crate::io::IoSafeState {
            outputs: vec![(
                IoAddress::parse("%QX0.0").expect("safe output address"),
                crate::value::Value::Bool(true),
            )],
        });

        runtime
            .apply_io_safe_state()
            .expect("GPIO safe-state handoff must be confirmed");

        assert_eq!(
            writes.lock().expect("recording backend lock").as_slice(),
            &[(27, true)]
        );
        assert_eq!(runtime.io().outputs(), &[1]);
    }
}

#[cfg(test)]
#[path = "gpio_contract_tests.rs"]
mod contract_tests;
