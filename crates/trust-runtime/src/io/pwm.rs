//! The `pwm` driver of a computer (specification 11 §6.5). On Linux an output is a sysfs PWM
//! channel, such as `pwmchip0/pwm1`, which the driver exports when it is not, runs at the period of
//! its `frequency_hz` and drives with the duty cycle of a `%QW` word in hundredths of a percent.
//! The parameters and the duty rule are the shared ones of
//! `trust_runtime_services::io_drivers::pwm`; this module writes the channel's files, as the PWM
//! ABI names them (`Documentation/ABI/testing/sysfs-class-pwm`).

use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use smol_str::SmolStr;
use trust_runtime_services::io_drivers::pwm::{self, PwmOutput, PwmParams};

use crate::error::RuntimeError;
use crate::io::{IoDriver, IoDriverHealth};

/// The driver's name, which starts each of its errors.
const NAME: &str = "pwm";

/// The folder of the PWM chips under the sysfs root.
const PWM_CHIPS: &str = "class/pwm";

/// Nanoseconds in a second, the period of 1 Hz.
const NANOS_PER_SECOND: u64 = 1_000_000_000;

/// The `pwm` driver: its channels, in `io.toml` order, and its health.
pub(super) struct PwmDriver {
    outputs: Vec<PwmChannel>,
    health: IoDriverHealth,
}

impl PwmDriver {
    /// The driver of `params` on this computer, whose sysfs is at `/sys`.
    ///
    /// # Errors
    ///
    /// The refusal of a line this computer has no PWM channel for or of a frequency its chip
    /// cannot make, and the failure to set a channel up, each after the driver's name.
    pub(super) fn start(params: PwmParams) -> Result<Self, RuntimeError> {
        Self::open(params, Path::new("/sys"))
    }

    /// The driver of `params` over the sysfs tree at `sysfs`, each channel set up and running at
    /// its `initial` duty cycle.
    fn open(params: PwmParams, sysfs: &Path) -> Result<Self, RuntimeError> {
        let outputs = params
            .outputs
            .iter()
            .enumerate()
            .map(|(index, output)| PwmChannel::open(sysfs, index, output))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self {
            outputs,
            health: IoDriverHealth::Ok,
        })
    }
}

impl IoDriver for PwmDriver {
    /// A PWM driver has no inputs.
    fn read_inputs(&mut self, _inputs: &mut [u8]) -> Result<(), RuntimeError> {
        Ok(())
    }

    /// Drives each channel with the duty cycle of its word. A failure faults the driver with its
    /// error, and a complete handoff makes it healthy again, as for GPIO.
    fn write_outputs(&mut self, outputs: &[u8]) -> Result<(), RuntimeError> {
        let result = self
            .outputs
            .iter_mut()
            .try_for_each(|channel| channel.write_from(outputs));
        self.health = match &result {
            Ok(()) => IoDriverHealth::Ok,
            Err(error) => IoDriverHealth::Faulted {
                error: SmolStr::new(error.to_string()),
            },
        };
        result
    }

    fn health(&self) -> IoDriverHealth {
        self.health.clone()
    }
}

/// A PWM channel and the `%QW` word it is driven from.
struct PwmChannel {
    /// The channel's `duty_cycle` file.
    duty_cycle: PathBuf,
    /// The period in nanoseconds.
    period: u64,
    /// The word's first byte in the output image.
    byte: usize,
    /// The duty cycle last written, in nanoseconds, which is not written again.
    written: Option<u64>,
}

impl PwmChannel {
    /// The channel that `output`, entry `index` of `outputs`, names under `sysfs`: exported when
    /// it is not, at the period of its frequency, at its `initial` duty cycle, and then enabled.
    fn open(sysfs: &Path, index: usize, output: &PwmOutput) -> Result<Self, RuntimeError> {
        let line = output.line.as_str();
        let Some((chip, number)) = parse_line(line) else {
            return Err(refused(format!(
                "outputs[{index}].line '{line}' is not a PWM channel such as pwmchip0/pwm1"
            )));
        };
        let chip_folder = sysfs.join(PWM_CHIPS).join(chip);
        if !chip_folder.is_dir() {
            return Err(refused(format!("this computer has no PWM chip {chip}")));
        }
        let folder = chip_folder.join(format!("pwm{number}"));
        if !folder.is_dir() {
            export(&chip_folder, chip, number)?;
        }
        if !folder.is_dir() {
            return Err(refused(format!(
                "PWM chip {chip} did not export pwm{number}"
            )));
        }
        let frequency = output.frequency_hz;
        let Some(period) = period_nanos(frequency) else {
            return Err(refused(format!(
                "outputs[{index}].frequency_hz {frequency} has no period in nanoseconds"
            )));
        };
        let duty_cycle = folder.join("duty_cycle");
        // The kernel refuses a period below the duty cycle in force, so a longer duty cycle left
        // by an earlier user is cleared first.
        if read_number(&duty_cycle)? > period {
            write_attribute(&duty_cycle, "0")?;
        }
        let period_file = folder.join("period");
        write_text(&period_file, &period.to_string()).map_err(|error| {
            refused(format!(
                "{line} cannot run at {frequency} Hz, a period of {period} ns: {error}"
            ))
        })?;
        // The initial duty cycle is in force before the channel is enabled.
        let initial = pwm::duty_nanos(output.initial, period);
        write_attribute(&duty_cycle, &initial.to_string())?;
        write_attribute(&folder.join("enable"), "1")?;
        Ok(Self {
            duty_cycle,
            period,
            byte: output.address.byte as usize,
            written: Some(initial),
        })
    }

    /// Drives the channel with the duty cycle of its word of `image`, unless that is the duty
    /// cycle last written. A failed write is not remembered, so the next cycle writes it again.
    fn write_from(&mut self, image: &[u8]) -> Result<(), RuntimeError> {
        let word = image
            .get(self.byte..)
            .and_then(|rest| rest.first_chunk::<2>())
            .ok_or_else(|| {
                RuntimeError::IoDriver(
                    format!(
                        "{NAME}: %QW{} is outside the output image of {} bytes",
                        self.byte,
                        image.len()
                    )
                    .into(),
                )
            })?;
        let nanos = pwm::duty_nanos(u16::from_le_bytes(*word), self.period);
        if self.written != Some(nanos) {
            write_attribute(&self.duty_cycle, &nanos.to_string())?;
            self.written = Some(nanos);
        }
        Ok(())
    }
}

/// The period of `frequency_hz` in nanoseconds, 1,000,000,000 / `frequency_hz` rounded down
/// (specification 11 §6.5); `None` for a frequency without a whole nanosecond of period.
fn period_nanos(frequency_hz: u32) -> Option<u64> {
    NANOS_PER_SECOND
        .checked_div(u64::from(frequency_hz))
        .filter(|period| *period > 0)
}

/// The chip and the channel number of a line such as `pwmchip0/pwm1`.
fn parse_line(line: &str) -> Option<(&str, u32)> {
    let (chip, channel) = line.split_once('/')?;
    let number = channel.strip_prefix("pwm")?;
    let chip_number = chip.strip_prefix("pwmchip")?;
    if !is_decimal(chip_number) || !is_decimal(number) {
        return None;
    }
    Some((chip, number.parse().ok()?))
}

/// Whether `text` is a decimal number: digits only, at least one.
fn is_decimal(text: &str) -> bool {
    !text.is_empty() && text.bytes().all(|byte| byte.is_ascii_digit())
}

/// Exports channel `number` of the chip `chip`, whose folder is `chip_folder`, after checking
/// with its `npwm` that the chip has it.
fn export(chip_folder: &Path, chip: &str, number: u32) -> Result<(), RuntimeError> {
    let channels = read_number(&chip_folder.join("npwm"))?;
    if u64::from(number) >= channels {
        return Err(refused(format!(
            "PWM chip {chip} has no channel pwm{number}: it has {channels} channels"
        )));
    }
    write_attribute(&chip_folder.join("export"), &number.to_string())
}

/// The number in the file at `path`.
fn read_number(path: &Path) -> Result<u64, RuntimeError> {
    let text = fs::read_to_string(path).map_err(|error| failed("read", path, &error))?;
    text.trim().parse().map_err(|_| {
        RuntimeError::IoDriver(
            format!(
                "{NAME}: {} holds '{}', not a number",
                path.display(),
                text.trim()
            )
            .into(),
        )
    })
}

/// Writes `text` to the sysfs file at `path`, failing with the path.
fn write_attribute(path: &Path, text: &str) -> Result<(), RuntimeError> {
    write_text(path, text).map_err(|error| failed("write", path, &error))
}

/// Writes `text` to the sysfs file at `path` in one write, as sysfs takes a value.
fn write_text(path: &Path, text: &str) -> io::Result<()> {
    OpenOptions::new()
        .write(true)
        .truncate(true)
        .open(path)
        .and_then(|mut file| file.write_all(text.as_bytes()))
}

/// The driver's refusal when it starts: its name and `reason`.
fn refused(reason: String) -> RuntimeError {
    RuntimeError::InvalidConfig(format!("{NAME}: {reason}").into())
}

/// The failure to `verb` the file at `path`.
fn failed(verb: &str, path: &Path, error: &io::Error) -> RuntimeError {
    RuntimeError::IoDriver(format!("{NAME}: {verb} {} failed: {error}", path.display()).into())
}

#[cfg(test)]
mod tests {
    use smol_str::SmolStr;
    use trust_runtime_services::io_drivers::pwm::{PwmOutput, PwmParams};

    use super::{period_nanos, PwmDriver};
    use crate::io::fake_sysfs::FakeSysfs;
    use crate::io::{IoAddress, IoDriver, IoDriverHealth};

    /// The fake chip `pwmchip0`.
    const CHIP: &str = "class/pwm/pwmchip0";

    /// A chip with two channels whose `pwm1` is exported, disabled, with no period yet.
    fn chip_with_exported_pwm1(name: &str) -> FakeSysfs {
        let sysfs = FakeSysfs::new(name);
        sysfs.set(&format!("{CHIP}/npwm"), "2\n");
        sysfs.set(&format!("{CHIP}/export"), "");
        sysfs.set(&format!("{CHIP}/pwm1/period"), "0\n");
        sysfs.set(&format!("{CHIP}/pwm1/duty_cycle"), "0\n");
        sysfs.set(&format!("{CHIP}/pwm1/enable"), "0\n");
        sysfs
    }

    fn params(line: &str, frequency_hz: u32, initial: u16) -> PwmParams {
        PwmParams {
            outputs: vec![PwmOutput {
                address: IoAddress::parse("%QW0").expect("output address"),
                line: SmolStr::new(line),
                frequency_hz,
                initial,
            }],
        }
    }

    fn refusal(params: PwmParams, sysfs: &FakeSysfs) -> String {
        match PwmDriver::open(params, sysfs.root()) {
            Ok(_) => panic!("the pwm driver must be refused"),
            Err(error) => error.to_string(),
        }
    }

    /// Specification 11 §6.5: the period is 1,000,000,000 / `frequency_hz` ns, so 1,000 Hz is
    /// 1,000,000 ns; the `initial` duty cycle 2,500 (25.00 %) is 1,000,000 × 2,500 / 10,000 =
    /// 250,000 ns; the channel is enabled; a channel already exported is not exported again.
    /// Catches a period in another unit, a duty cycle that is not the initial one at the start, a
    /// channel left disabled, and a second export (which the kernel refuses with EBUSY).
    #[test]
    fn a_1000_hz_output_runs_at_1000000_ns_with_its_initial_duty_cycle() {
        let sysfs = chip_with_exported_pwm1("pwm-start");

        PwmDriver::open(params("pwmchip0/pwm1", 1_000, 2_500), sysfs.root())
            .expect("open the pwm output");

        assert_eq!(sysfs.get(&format!("{CHIP}/pwm1/period")), "1000000");
        assert_eq!(sysfs.get(&format!("{CHIP}/pwm1/duty_cycle")), "250000");
        assert_eq!(sysfs.get(&format!("{CHIP}/pwm1/enable")), "1");
        assert_eq!(sysfs.get(&format!("{CHIP}/export")), "");
    }

    /// Specification 11 §6.5: the period is rounded down, so 3 Hz is 333,333,333 ns. Catches a
    /// period rounded up (333,333,334 ns) or to the nearest.
    #[test]
    fn the_period_of_a_frequency_is_rounded_down() {
        assert_eq!(period_nanos(3), Some(333_333_333));
        assert_eq!(period_nanos(1_000_000), Some(1_000));
        assert_eq!(period_nanos(0), None);
    }

    /// Specification 11 §6.5: the duty cycle is the `%QW` word in hundredths of a percent, and a
    /// value above 10,000 drives 100 %. At 1,000 Hz, 5,000 (0x1388) is 500,000 ns and 12,000
    /// (0x2EE0) is the whole period, 1,000,000 ns. Catches a duty cycle longer than the period
    /// (1,200,000 ns, which the kernel refuses) and a word read in the other byte order.
    #[test]
    fn the_duty_cycle_follows_the_word_and_a_value_above_10000_is_100_percent() {
        let sysfs = chip_with_exported_pwm1("pwm-duty");
        let mut driver = PwmDriver::open(params("pwmchip0/pwm1", 1_000, 0), sysfs.root())
            .expect("open the pwm output");

        driver.write_outputs(&[0x88, 0x13]).expect("drive 50 %");
        assert_eq!(sysfs.get(&format!("{CHIP}/pwm1/duty_cycle")), "500000");
        driver.write_outputs(&[0xE0, 0x2E]).expect("drive 120 %");
        assert_eq!(sysfs.get(&format!("{CHIP}/pwm1/duty_cycle")), "1000000");
        assert_eq!(driver.health(), IoDriverHealth::Ok);
    }

    /// Specification 11 §6.5 with the GPIO output rule: a failed write faults the driver and is
    /// not remembered, so the next cycle writes the same duty cycle again. Catches a duty cycle
    /// counted as written although the write failed, which would leave the channel at its old
    /// duty cycle for good.
    #[test]
    fn a_failed_duty_cycle_write_faults_the_driver_and_is_written_again() {
        let sysfs = chip_with_exported_pwm1("pwm-retry");
        let mut driver = PwmDriver::open(params("pwmchip0/pwm1", 1_000, 0), sysfs.root())
            .expect("open the pwm output");

        sysfs.remove(&format!("{CHIP}/pwm1/duty_cycle"));
        let error = driver
            .write_outputs(&[0x88, 0x13])
            .expect_err("a missing duty_cycle must fail");
        assert!(error.to_string().contains("duty_cycle failed"), "{error}");
        assert!(matches!(driver.health(), IoDriverHealth::Faulted { .. }));

        sysfs.set(&format!("{CHIP}/pwm1/duty_cycle"), "0\n");
        driver
            .write_outputs(&[0x88, 0x13])
            .expect("drive 50 % again");
        assert_eq!(sysfs.get(&format!("{CHIP}/pwm1/duty_cycle")), "500000");
        assert_eq!(driver.health(), IoDriverHealth::Ok);
    }

    /// The PWM ABI: a channel that is not exported is exported by writing its number to the
    /// chip's `export`. The fake tree has no kernel to create `pwm1`, so the driver then reports
    /// the channel missing. Catches a channel used without being exported.
    #[test]
    fn a_channel_that_is_not_exported_is_exported() {
        let sysfs = FakeSysfs::new("pwm-export");
        sysfs.set(&format!("{CHIP}/npwm"), "2\n");
        sysfs.set(&format!("{CHIP}/export"), "");

        assert_eq!(
            refusal(params("pwmchip0/pwm1", 1_000, 0), &sysfs),
            "invalid config 'pwm: PWM chip pwmchip0 did not export pwm1'"
        );
        assert_eq!(sysfs.get(&format!("{CHIP}/export")), "1");
    }

    /// Specification 11 §6.5 and 34 TARGET-01: a computer refuses, when the driver starts, a line
    /// that is not one of its PWM channels, with the driver's name and the reason. Catches a chip's
    /// pin name, a missing chip or a channel past the chip's `npwm` that fails later or without
    /// saying why.
    #[test]
    fn a_line_this_computer_has_no_channel_for_is_refused_with_the_reason() {
        let sysfs = chip_with_exported_pwm1("pwm-refusals");

        assert_eq!(
            refusal(params("GPIO4", 1_000, 0), &sysfs),
            "invalid config 'pwm: outputs[0].line 'GPIO4' is not a PWM channel such as \
             pwmchip0/pwm1'"
        );
        assert_eq!(
            refusal(params("pwmchip3/pwm0", 1_000, 0), &sysfs),
            "invalid config 'pwm: this computer has no PWM chip pwmchip3'"
        );
        assert_eq!(
            refusal(params("pwmchip0/pwm5", 1_000, 0), &sysfs),
            "invalid config 'pwm: PWM chip pwmchip0 has no channel pwm5: it has 2 channels'"
        );
    }
}
