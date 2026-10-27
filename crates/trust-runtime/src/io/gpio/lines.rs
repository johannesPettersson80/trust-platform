//! Listing the GPIO lines of this computer (specification 11, "Sources" and "Identity"): the lines
//! are read through the GPIO character devices of Linux, and the sysfs interface is only the
//! fallback for a kernel without them.

use std::path::Path;

use super::{collect_gpiochip_lines, GpioLineInfo};
use crate::error::RuntimeError;
#[cfg(target_os = "linux")]
use smol_str::SmolStr;

/// One GPIO line as the operating system lists it (specification 11, "Identity"): the chip that
/// exposes it, the chip's label, the line's offset within that chip, its name, its consumer,
/// whether it is in use, and its direction. A line from the sysfs fallback is marked, so the
/// search says so in a warning.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GpioLine {
    /// The chip's device name, for example `gpiochip0`.
    pub chip: String,
    /// The chip's label, for example `pinctrl-bcm2835`.
    pub label: Option<String>,
    /// The line's offset within its chip.
    pub line: u32,
    /// The line's name, when the kernel gives it one.
    pub name: Option<String>,
    /// The consumer that holds the line, when one does.
    pub consumer: Option<String>,
    /// Whether the line is in use.
    pub used: bool,
    /// The line's direction (`in` or `out`), when the kernel reports one.
    pub direction: Option<String>,
    /// Whether the sysfs fallback listed this line, rather than a GPIO character device.
    pub from_sysfs: bool,
}

impl GpioLine {
    /// The line as the sysfs fallback lists it (specification 11, "Sources"): the chip, its label,
    /// the sysfs line number and the direction the kernel reports. The sysfs interface names no
    /// line, consumer or use.
    pub(super) fn from_sysfs(info: GpioLineInfo) -> Self {
        Self {
            chip: info.chip,
            label: info.label,
            line: info.line,
            name: None,
            consumer: None,
            used: false,
            direction: info.direction,
            from_sysfs: true,
        }
    }
}

/// Lists every GPIO line of this computer (specification 11, "Sources"): through the GPIO
/// character devices, or the sysfs fallback for a kernel without them. A line from the fallback
/// says so, so the search warns.
pub(super) fn discover() -> Result<Vec<GpioLine>, RuntimeError> {
    #[cfg(target_os = "linux")]
    let mut lines = character_device_lines()?;
    #[cfg(not(target_os = "linux"))]
    let mut lines = Vec::new();
    if lines.is_empty() {
        let mut sysfs = Vec::new();
        collect_gpiochip_lines(Path::new("/sys/bus/gpio/devices"), &mut sysfs)?;
        if sysfs.is_empty() {
            collect_gpiochip_lines(Path::new("/sys/class/gpio"), &mut sysfs)?;
        }
        lines = sysfs.into_iter().map(GpioLine::from_sysfs).collect();
    }
    lines.sort_by(|left, right| left.chip.cmp(&right.chip).then(left.line.cmp(&right.line)));
    lines.dedup_by(|left, right| left.chip == right.chip && left.line == right.line);
    Ok(lines)
}

/// Reads the lines of every GPIO chip of this computer through the GPIO character devices.
#[cfg(target_os = "linux")]
fn character_device_lines() -> Result<Vec<GpioLine>, RuntimeError> {
    let mut lines = Vec::new();
    for chip in gpio_cdev::chips().map_err(|error| chardev_error("list chips", error))? {
        let chip = chip.map_err(|error| chardev_error("open chip", error))?;
        let name = chip.name().to_string();
        let label = nonempty(chip.label());
        let described = chip
            .lines()
            .map(|line| chip_line(&name, line))
            .collect::<Result<Vec<_>, _>>()?;
        lines.extend(chip_lines(&name, label.as_deref(), described));
    }
    Ok(lines)
}

/// Reads one line's info from the kernel.
#[cfg(target_os = "linux")]
fn chip_line(chip: &str, line: gpio_cdev::Line) -> Result<ChipLine, RuntimeError> {
    let offset = line.offset();
    let info = line.info().map_err(|error| {
        RuntimeError::IoDriver(SmolStr::new(format!(
            "gpio line {chip}:{offset} info failed: {error}"
        )))
    })?;
    Ok(ChipLine {
        offset,
        name: info.name().map(str::to_string),
        consumer: info.consumer().map(str::to_string),
        used: info.is_used(),
        direction: Some(direction_word(info.direction()).to_string()),
    })
}

/// One line as the kernel's GPIO line info describes it.
#[cfg(target_os = "linux")]
struct ChipLine {
    offset: u32,
    name: Option<String>,
    consumer: Option<String>,
    used: bool,
    direction: Option<String>,
}

/// Puts the chip's name and label on each of its lines. This is the seam that lets a test give a
/// literal chip and its lines.
#[cfg(target_os = "linux")]
fn chip_lines(chip: &str, label: Option<&str>, lines: Vec<ChipLine>) -> Vec<GpioLine> {
    lines
        .into_iter()
        .map(|line| GpioLine {
            chip: chip.to_string(),
            label: label.map(str::to_string),
            line: line.offset,
            name: line.name,
            consumer: line.consumer,
            used: line.used,
            direction: line.direction,
            from_sysfs: false,
        })
        .collect()
}

#[cfg(target_os = "linux")]
fn direction_word(direction: gpio_cdev::LineDirection) -> &'static str {
    match direction {
        gpio_cdev::LineDirection::Out => "out",
        gpio_cdev::LineDirection::In => "in",
    }
}

#[cfg(target_os = "linux")]
fn nonempty(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

#[cfg(target_os = "linux")]
fn chardev_error(context: &str, error: gpio_cdev::Error) -> RuntimeError {
    RuntimeError::IoDriver(SmolStr::new(format!("gpio {context} failed: {error}")))
}

#[cfg(test)]
#[path = "lines_tests.rs"]
mod tests;
