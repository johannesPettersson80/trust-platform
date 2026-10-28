//! The `analog` driver of a computer (specification 11 §6.5). On Linux an input is an IIO input
//! channel, such as `iio:device0/in_voltage1`, read at each cycle start into a `%IW` word as the
//! millivolts `(raw + offset) × scale`; an output is an IIO output channel, such as
//! `iio:device0/out_voltage0`, driven to the millivolts of a `%QW` word with the code
//! `round(mV / scale) − offset`. The parameters and the value rules are the shared ones of
//! `trust_runtime_services::io_drivers::analog`; this module finds the channels' files under the
//! sysfs root and reads and writes them, as the IIO ABI names them
//! (`Documentation/ABI/testing/sysfs-bus-iio`).

use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use smol_str::SmolStr;
use trust_runtime_services::io_drivers::analog::{self, AnalogInput, AnalogOutput, AnalogParams};

use crate::error::RuntimeError;
use crate::io::{IoDriver, IoDriverHealth};

/// The driver's name, which starts each of its errors.
const NAME: &str = "analog";

/// The folder of the IIO devices under the sysfs root.
const IIO_DEVICES: &str = "bus/iio/devices";

/// The `analog` driver: its channels, in `io.toml` order, and the health of its input and its
/// output path.
pub(super) struct AnalogDriver {
    inputs: Vec<InputChannel>,
    outputs: Vec<OutputChannel>,
    /// The outcome of the last input handoff.
    input: IoDriverHealth,
    /// The outcome of the last output handoff.
    output: IoDriverHealth,
}

impl AnalogDriver {
    /// The driver of `params` on this computer, whose sysfs is at `/sys`.
    ///
    /// # Errors
    ///
    /// The refusal of a line this computer has no channel for, and the failure to read a channel
    /// or to drive an output to its `initial` millivolts, each after the driver's name.
    pub(super) fn start(params: AnalogParams) -> Result<Self, RuntimeError> {
        Self::open(params, Path::new("/sys"))
    }

    /// The driver of `params` over the sysfs tree at `sysfs`: each channel found with its offset
    /// and scale, and each output driven to its `initial` millivolts.
    fn open(params: AnalogParams, sysfs: &Path) -> Result<Self, RuntimeError> {
        let inputs = params
            .inputs
            .iter()
            .enumerate()
            .map(|(index, input)| InputChannel::open(sysfs, index, input))
            .collect::<Result<Vec<_>, _>>()?;
        let outputs = params
            .outputs
            .iter()
            .enumerate()
            .map(|(index, output)| OutputChannel::open(sysfs, index, output))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self {
            inputs,
            outputs,
            input: IoDriverHealth::Ok,
            output: IoDriverHealth::Ok,
        })
    }
}

impl IoDriver for AnalogDriver {
    fn read_inputs(&mut self, inputs: &mut [u8]) -> Result<(), RuntimeError> {
        let result = self
            .inputs
            .iter()
            .try_for_each(|channel| channel.read_into(inputs));
        self.input = health_after(&result);
        result
    }

    fn write_outputs(&mut self, outputs: &[u8]) -> Result<(), RuntimeError> {
        let result = self
            .outputs
            .iter_mut()
            .try_for_each(|channel| channel.write_from(outputs));
        self.output = health_after(&result);
        result
    }

    /// The input path's health when it is faulted, otherwise the output path's.
    fn health(&self) -> IoDriverHealth {
        match self.input {
            IoDriverHealth::Faulted { .. } => self.input.clone(),
            _ => self.output.clone(),
        }
    }

    /// The output path's health: a failed reading concerns the inputs alone, so it never leaves
    /// a safe-state handoff unconfirmed (specification 11, safe-state handoff confirmation).
    fn output_health(&self) -> IoDriverHealth {
        self.output.clone()
    }
}

/// The health of a path after a handoff with `result`: a failure faults it with its error, and a
/// complete handoff makes it healthy again, as for GPIO.
fn health_after(result: &Result<(), RuntimeError>) -> IoDriverHealth {
    match result {
        Ok(()) => IoDriverHealth::Ok,
        Err(error) => IoDriverHealth::Faulted {
            error: SmolStr::new(error.to_string()),
        },
    }
}

/// An input channel and the `%IW` word it reads into.
struct InputChannel {
    /// The channel's `_raw` file.
    raw: PathBuf,
    /// The offset in counts, added to a reading before it is scaled.
    offset: f64,
    /// The scale in millivolts per count.
    scale: f64,
    /// The word's first byte in the input image.
    byte: usize,
}

impl InputChannel {
    /// The input `input`, entry `index` of `inputs`, under `sysfs`.
    fn open(sysfs: &Path, index: usize, input: &AnalogInput) -> Result<Self, RuntimeError> {
        let key = format!("inputs[{index}]");
        let channel = Channel::find(sysfs, &key, &input.line, Direction::Input)?;
        let raw = channel.file("raw");
        // Opened once here, so that a channel this process may not read is refused at the start
        // rather than at the first cycle.
        fs::File::open(&raw).map_err(|error| failed("read", &raw, &error))?;
        Ok(Self {
            offset: channel.offset()?,
            scale: channel.scale()?,
            raw,
            byte: input.address.byte as usize,
        })
    }

    /// Reads the channel and writes its millivolts, limited to an `INT`, to its word of `image`.
    fn read_into(&self, image: &mut [u8]) -> Result<(), RuntimeError> {
        let text =
            fs::read_to_string(&self.raw).map_err(|error| failed("read", &self.raw, &error))?;
        let raw: i64 = text
            .trim()
            .parse()
            .map_err(|_| malformed(&self.raw, &text, "a count"))?;
        let millivolts = analog::input_value(analog::iio_millivolts(raw, self.offset, self.scale));
        let len = image.len();
        let word = image
            .get_mut(self.byte..)
            .and_then(|rest| rest.first_chunk_mut::<2>())
            .ok_or_else(|| outside("%IW", self.byte, "input", len))?;
        *word = millivolts.to_le_bytes();
        Ok(())
    }
}

/// An output channel and the `%QW` word it is driven from.
struct OutputChannel {
    /// The channel's `_raw` file.
    raw: PathBuf,
    /// The offset in counts, taken from a code after it is scaled.
    offset: f64,
    /// The scale in millivolts per count.
    scale: f64,
    /// The lowest and the highest code the channel converts.
    codes: (i64, i64),
    /// The word's first byte in the output image.
    byte: usize,
    /// The code last written, which is not written again.
    written: Option<i64>,
}

impl OutputChannel {
    /// The output `output`, entry `index` of `outputs`, under `sysfs`, driven to its `initial`
    /// millivolts.
    fn open(sysfs: &Path, index: usize, output: &AnalogOutput) -> Result<Self, RuntimeError> {
        let key = format!("outputs[{index}]");
        let channel = Channel::find(sysfs, &key, &output.line, Direction::Output)?;
        let mut opened = Self {
            raw: channel.file("raw"),
            offset: channel.offset()?,
            scale: channel.scale()?,
            codes: channel.codes()?,
            byte: output.address.byte as usize,
            written: None,
        };
        opened.drive(output.initial)?;
        Ok(opened)
    }

    /// Drives the channel to the millivolts of its word of `image`.
    fn write_from(&mut self, image: &[u8]) -> Result<(), RuntimeError> {
        let word = image
            .get(self.byte..)
            .and_then(|rest| rest.first_chunk::<2>())
            .ok_or_else(|| outside("%QW", self.byte, "output", image.len()))?;
        self.drive(i16::from_le_bytes(*word))
    }

    /// Writes the code of `millivolts` unless it is the code last written. A failed write is not
    /// remembered, so the next cycle writes the code again.
    fn drive(&mut self, millivolts: i16) -> Result<(), RuntimeError> {
        let code = output_code(millivolts, self.scale, self.offset, self.codes);
        if self.written != Some(code) {
            write_attribute(&self.raw, &code.to_string())?;
            self.written = Some(code);
        }
        Ok(())
    }
}

/// Whether a channel converts a voltage in or out.
#[derive(Debug, Clone, Copy)]
enum Direction {
    /// An input, `in_voltage<N>`.
    Input,
    /// An output, `out_voltage<N>`.
    Output,
}

impl Direction {
    /// The type of the channel's files, after which its device's shared files are named.
    fn prefix(self) -> &'static str {
        match self {
            Self::Input => "in_voltage",
            Self::Output => "out_voltage",
        }
    }

    /// A line of this direction, which a refused line is shown with.
    fn example(self) -> &'static str {
        match self {
            Self::Input => "iio:device0/in_voltage1",
            Self::Output => "iio:device0/out_voltage0",
        }
    }
}

/// The IIO channel that a line names.
struct Channel<'a> {
    /// The line, as `iio:device0/in_voltage1`.
    line: &'a str,
    /// The device's folder.
    folder: PathBuf,
    /// The channel's name, as `in_voltage1`.
    name: &'a str,
    /// Whether the channel converts in or out.
    direction: Direction,
}

impl<'a> Channel<'a> {
    /// The channel that `line`, the line of the entry `key`, names under `sysfs`.
    fn find(
        sysfs: &Path,
        key: &str,
        line: &'a str,
        direction: Direction,
    ) -> Result<Self, RuntimeError> {
        let parts = line.split_once('/').filter(|(device, name)| {
            numbered(device, "iio:device") && numbered(name, direction.prefix())
        });
        let Some((device, name)) = parts else {
            return Err(refused(format!(
                "{key}.line '{line}' is not an IIO channel such as {}",
                direction.example()
            )));
        };
        let folder = sysfs.join(IIO_DEVICES).join(device);
        if !folder.is_dir() {
            return Err(refused(format!("this computer has no IIO device {device}")));
        }
        let channel = Self {
            line,
            folder,
            name,
            direction,
        };
        if !channel.file("raw").is_file() {
            return Err(refused(format!(
                "IIO device {device} has no channel {name}"
            )));
        }
        Ok(channel)
    }

    /// The channel's own file `<name>_<attribute>`.
    fn file(&self, attribute: &str) -> PathBuf {
        self.folder.join(format!("{}_{attribute}", self.name))
    }

    /// The value of `attribute`: the channel's own, otherwise the one its device shares among the
    /// channels of its type, `<type>_<attribute>` (IIO ABI); `None` without either.
    fn number(&self, attribute: &str) -> Result<Option<f64>, RuntimeError> {
        if let Some(own) = optional_number(&self.file(attribute))? {
            return Ok(Some(own));
        }
        let shared = format!("{}_{attribute}", self.direction.prefix());
        optional_number(&self.folder.join(shared))
    }

    /// The offset in counts, 0 when neither the channel nor its device has one.
    fn offset(&self) -> Result<f64, RuntimeError> {
        match self.number("offset")? {
            None => Ok(0.0),
            Some(offset) if offset.is_finite() => Ok(offset),
            Some(offset) => Err(refused(format!(
                "IIO channel {} has the offset {offset}",
                self.line
            ))),
        }
    }

    /// The scale in millivolts per count, which every channel needs.
    fn scale(&self) -> Result<f64, RuntimeError> {
        match self.number("scale")? {
            Some(scale) if scale.is_finite() && scale > 0.0 => Ok(scale),
            Some(scale) => Err(refused(format!(
                "IIO channel {} has the scale {scale}, which converts nothing",
                self.line
            ))),
            None => Err(refused(format!(
                "IIO channel {} has no scale: neither {}_scale nor {}_scale",
                self.line,
                self.name,
                self.direction.prefix()
            ))),
        }
    }

    /// The codes an output channel converts, so that a value outside them is limited to them
    /// (specification 11 §6.5): those its `_raw_available` lists, as the range `[min step max]` or
    /// as codes. Without that file every code from 0 is taken, as an IIO converter's codes are
    /// unsigned and a bipolar converter's sign is its `_offset`; the kernel refuses a code above
    /// its converter's range.
    fn codes(&self) -> Result<(i64, i64), RuntimeError> {
        let path = self.file("raw_available");
        match fs::read_to_string(&path) {
            Ok(text) => parse_codes(&text).ok_or_else(|| malformed(&path, &text, "codes")),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok((0, i64::MAX)),
            Err(error) => Err(failed("read", &path, &error)),
        }
    }
}

/// The lowest and the highest code of a `_raw_available` text: the range `[min step max]`, or a
/// list of codes. `None` for any other text.
fn parse_codes(text: &str) -> Option<(i64, i64)> {
    let text = text.trim();
    let range = text
        .strip_prefix('[')
        .and_then(|rest| rest.strip_suffix(']'));
    let codes = range
        .unwrap_or(text)
        .split_whitespace()
        .map(str::parse::<i64>)
        .collect::<Result<Vec<_>, _>>()
        .ok()?;
    let (low, high) = match (range, codes.as_slice()) {
        (Some(_), [low, _step, high]) => (*low, *high),
        (None, [_, ..]) => (*codes.iter().min()?, *codes.iter().max()?),
        _ => return None,
    };
    (low <= high).then_some((low, high))
}

/// The code that drives an output to `millivolts`: `round(mV / scale) − offset` (specification 11
/// §6.5), limited to `codes`, the lowest and the highest code the channel converts.
fn output_code(millivolts: i16, scale: f64, offset: f64, codes: (i64, i64)) -> i64 {
    let code = ((f64::from(millivolts) / scale).round() - offset).round();
    // A float beyond the integers saturates in the cast, and the codes limit it either way.
    (code as i64).clamp(codes.0, codes.1)
}

/// Whether `text` is `prefix` followed by a decimal number, as `in_voltage1`.
fn numbered(text: &str, prefix: &str) -> bool {
    text.strip_prefix(prefix).is_some_and(|number| {
        !number.is_empty() && number.bytes().all(|byte| byte.is_ascii_digit())
    })
}

/// The number in the file at `path`, `None` when there is no such file.
fn optional_number(path: &Path) -> Result<Option<f64>, RuntimeError> {
    match fs::read_to_string(path) {
        Ok(text) => text
            .trim()
            .parse()
            .map(Some)
            .map_err(|_| malformed(path, &text, "a number")),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(failed("read", path, &error)),
    }
}

/// Writes `text` to the sysfs file at `path` in one write, as sysfs takes a value.
fn write_attribute(path: &Path, text: &str) -> Result<(), RuntimeError> {
    OpenOptions::new()
        .write(true)
        .truncate(true)
        .open(path)
        .and_then(|mut file| file.write_all(text.as_bytes()))
        .map_err(|error| failed("write", path, &error))
}

/// The driver's refusal when it starts: its name and `reason`.
fn refused(reason: String) -> RuntimeError {
    RuntimeError::InvalidConfig(format!("{NAME}: {reason}").into())
}

/// The failure to `verb` the file at `path`.
fn failed(verb: &str, path: &Path, error: &io::Error) -> RuntimeError {
    RuntimeError::IoDriver(format!("{NAME}: {verb} {} failed: {error}", path.display()).into())
}

/// The refusal of `text`, read from the file at `path`, which should hold `expected`.
fn malformed(path: &Path, text: &str, expected: &str) -> RuntimeError {
    RuntimeError::IoDriver(
        format!(
            "{NAME}: {} holds '{}', not {expected}",
            path.display(),
            text.trim()
        )
        .into(),
    )
}

/// The failure of the word `area` at `byte`, past the end of an image of `len` bytes.
fn outside(area: &str, byte: usize, image: &str, len: usize) -> RuntimeError {
    RuntimeError::IoDriver(
        format!("{NAME}: {area}{byte} is outside the {image} image of {len} bytes").into(),
    )
}

#[cfg(test)]
mod tests {
    use smol_str::SmolStr;
    use trust_runtime_services::io_drivers::analog::{AnalogInput, AnalogOutput, AnalogParams};

    use super::{parse_codes, AnalogDriver};
    use crate::io::fake_sysfs::FakeSysfs;
    use crate::io::{IoAddress, IoDriver, IoDriverHealth};

    /// The fake device `iio:device0`.
    const DEVICE: &str = "bus/iio/devices/iio:device0";

    /// The scale of a 12-bit converter over 3,300 mV, in millivolts per count: 3,300 / 4,096.
    const SCALE_12_BIT: &str = "0.805664062\n";

    fn input(line: &str, address: &str) -> AnalogInput {
        AnalogInput {
            address: IoAddress::parse(address).expect("input address"),
            line: SmolStr::new(line),
        }
    }

    fn output(line: &str, address: &str, initial: i16) -> AnalogOutput {
        AnalogOutput {
            address: IoAddress::parse(address).expect("output address"),
            line: SmolStr::new(line),
            initial,
        }
    }

    fn reading(inputs: Vec<AnalogInput>) -> AnalogParams {
        AnalogParams {
            inputs,
            outputs: Vec::new(),
        }
    }

    fn driving(outputs: Vec<AnalogOutput>) -> AnalogParams {
        AnalogParams {
            inputs: Vec::new(),
            outputs,
        }
    }

    fn attribute(name: &str) -> String {
        format!("{DEVICE}/{name}")
    }

    /// The `%IW2` word an input channel reads into an image of 4 bytes filled with `0xFF`.
    fn read_word(sysfs: &FakeSysfs) -> [u8; 4] {
        let mut driver = AnalogDriver::open(
            reading(vec![input("iio:device0/in_voltage1", "%IW2")]),
            sysfs.root(),
        )
        .expect("open the analog input");
        let mut image = [0xFF; 4];
        driver.read_inputs(&mut image).expect("read the input");
        image
    }

    fn refusal(params: AnalogParams, sysfs: &FakeSysfs) -> String {
        match AnalogDriver::open(params, sysfs.root()) {
            Ok(_) => panic!("the analog driver must be refused"),
            Err(error) => error.to_string(),
        }
    }

    /// Specification 11 §6.5: an input is `(raw + offset) × scale`, rounded to the nearest
    /// millivolt, with `_offset` 0 when absent and the device's shared `in_voltage_scale` when the
    /// channel has no `_scale`. Raw 2048 at 0.805664062 mV per count is 2048 × 0.805664062 =
    /// 1,649.999998976 mV, so 1,650 = 0x0672, written little-endian as `72 06` at `%IW2`. Catches
    /// a truncated reading (1,649), a missed shared scale, and a word written to other bytes or in
    /// the other byte order.
    #[test]
    fn an_iio_input_of_raw_2048_at_0_805664062_mv_per_count_reads_1650_mv() {
        let sysfs = FakeSysfs::new("analog-1650");
        sysfs.set(&attribute("in_voltage1_raw"), "2048\n");
        sysfs.set(&attribute("in_voltage_scale"), SCALE_12_BIT);

        assert_eq!(read_word(&sysfs), [0xFF, 0xFF, 0x72, 0x06]);
    }

    /// Specification 11 §6.5: the channel's own `_scale` is the scale, the shared one only stands
    /// in for it. Raw 2048 at the channel's 0.5 mV per count is 1,024 mV = 0x0400. Catches the
    /// shared scale taken over the channel's own (1,650 mV).
    #[test]
    fn a_channel_scale_comes_before_the_device_shared_scale() {
        let sysfs = FakeSysfs::new("analog-own-scale");
        sysfs.set(&attribute("in_voltage1_raw"), "2048\n");
        sysfs.set(&attribute("in_voltage1_scale"), "0.5\n");
        sysfs.set(&attribute("in_voltage_scale"), SCALE_12_BIT);

        assert_eq!(read_word(&sysfs), [0xFF, 0xFF, 0x00, 0x04]);
    }

    /// Specification 11 §6.5 and the IIO ABI: the offset is added to the raw count before it is
    /// scaled. Raw 3000 with `in_voltage1_offset` -2048 at 1 mV per count is (3000 − 2048) × 1 =
    /// 952 mV = 0x03B8. Catches an offset subtracted (5,048 mV) or ignored (3,000 mV).
    #[test]
    fn the_channel_offset_is_added_before_the_scale() {
        let sysfs = FakeSysfs::new("analog-offset");
        sysfs.set(&attribute("in_voltage1_raw"), "3000\n");
        sysfs.set(&attribute("in_voltage1_offset"), "-2048\n");
        sysfs.set(&attribute("in_voltage1_scale"), "1\n");

        assert_eq!(read_word(&sysfs), [0xFF, 0xFF, 0xB8, 0x03]);
    }

    /// The IIO ABI: an `in_voltage_offset` without a channel number applies to every voltage input
    /// of the device. Raw 2000 with the shared offset 100 at 0.5 mV per count is (2000 + 100) ×
    /// 0.5 = 1,050 mV = 0x041A. Catches a shared offset ignored (1,000 mV).
    #[test]
    fn the_device_shared_offset_applies_to_a_channel_without_its_own() {
        let sysfs = FakeSysfs::new("analog-shared-offset");
        sysfs.set(&attribute("in_voltage1_raw"), "2000\n");
        sysfs.set(&attribute("in_voltage_offset"), "100\n");
        sysfs.set(&attribute("in_voltage_scale"), "0.5\n");

        assert_eq!(read_word(&sysfs), [0xFF, 0xFF, 0x1A, 0x04]);
    }

    /// Specification 11 §6.5: the input is an `INT`, and a value outside what is converted is
    /// limited to its range. Raw 50,000 at 1 mV per count is 50,000 mV, limited to 32,767 =
    /// 0x7FFF; raw -50,000 to -32,768 = 0x8000. Catches a wrapped value (50,000 as an `INT` is
    /// -15,536).
    #[test]
    fn an_input_beyond_an_int_is_limited_to_its_range() {
        let sysfs = FakeSysfs::new("analog-limited");
        sysfs.set(&attribute("in_voltage1_scale"), "1\n");
        sysfs.set(&attribute("in_voltage1_raw"), "50000\n");
        assert_eq!(read_word(&sysfs), [0xFF, 0xFF, 0xFF, 0x7F]);

        sysfs.set(&attribute("in_voltage1_raw"), "-50000\n");
        assert_eq!(read_word(&sysfs), [0xFF, 0xFF, 0x00, 0x80]);
    }

    /// Specification 11 §6.5: an output is written as `round(mV / scale) − offset`. 1,002 mV at
    /// 0.805664062 mV per count is 1,243.69… counts, so the code is 1,244; with
    /// `out_voltage0_offset` 100 it is 1,244 − 100 = 1,144. `%QW0` = 1,002 = 0x03EA. Catches a
    /// truncated code (1,243) and an offset added (1,344).
    #[test]
    fn an_output_writes_the_rounded_code_less_its_offset() {
        let sysfs = FakeSysfs::new("analog-output");
        sysfs.set(&attribute("out_voltage0_raw"), "0\n");
        sysfs.set(&attribute("out_voltage_scale"), SCALE_12_BIT);
        let mut driver = AnalogDriver::open(
            driving(vec![output("iio:device0/out_voltage0", "%QW0", 0)]),
            sysfs.root(),
        )
        .expect("open the analog output");

        driver.write_outputs(&[0xEA, 0x03]).expect("drive 1,002 mV");
        assert_eq!(sysfs.get(&attribute("out_voltage0_raw")), "1244");

        let sysfs = FakeSysfs::new("analog-output-offset");
        sysfs.set(&attribute("out_voltage0_raw"), "0\n");
        sysfs.set(&attribute("out_voltage_scale"), SCALE_12_BIT);
        sysfs.set(&attribute("out_voltage0_offset"), "100\n");
        let mut driver = AnalogDriver::open(
            driving(vec![output("iio:device0/out_voltage0", "%QW0", 0)]),
            sysfs.root(),
        )
        .expect("open the analog output");

        driver.write_outputs(&[0xEA, 0x03]).expect("drive 1,002 mV");
        assert_eq!(sysfs.get(&attribute("out_voltage0_raw")), "1144");
    }

    /// Specification 11 §6.5: an output is driven from `initial` until the program writes it, so
    /// the driver writes the code of `initial` when it starts. 1,650 mV at 0.805664062 mV per
    /// count is 2,048.0000026 counts, code 2,048. Catches an output left at whatever it held
    /// before the start.
    #[test]
    fn an_output_is_driven_to_its_initial_millivolts_when_the_driver_starts() {
        let sysfs = FakeSysfs::new("analog-initial");
        sysfs.set(&attribute("out_voltage0_raw"), "0\n");
        sysfs.set(&attribute("out_voltage_scale"), SCALE_12_BIT);

        AnalogDriver::open(
            driving(vec![output("iio:device0/out_voltage0", "%QW0", 1650)]),
            sysfs.root(),
        )
        .expect("open the analog output");

        assert_eq!(sysfs.get(&attribute("out_voltage0_raw")), "2048");
    }

    /// Specification 11 §6.5: a value outside what the hardware converts is limited to its range,
    /// which a converter publishes as `out_voltage0_raw_available` = `[0 1 4095]`. 32,767 mV is
    /// 40,671 counts, limited to 4,095; -100 mV is -124 counts, limited to 0. `%QW0` = 32,767 =
    /// `FF 7F`, -100 = 0xFF9C = `9C FF`. Catches a code the converter refuses written, which
    /// would fault the driver instead.
    #[test]
    fn an_output_beyond_the_converter_range_is_limited_to_it() {
        let sysfs = FakeSysfs::new("analog-output-range");
        sysfs.set(&attribute("out_voltage0_raw"), "0\n");
        sysfs.set(&attribute("out_voltage_scale"), SCALE_12_BIT);
        sysfs.set(&attribute("out_voltage0_raw_available"), "[0 1 4095]\n");
        let mut driver = AnalogDriver::open(
            driving(vec![output("iio:device0/out_voltage0", "%QW0", 0)]),
            sysfs.root(),
        )
        .expect("open the analog output");

        driver
            .write_outputs(&[0xFF, 0x7F])
            .expect("drive 32,767 mV");
        assert_eq!(sysfs.get(&attribute("out_voltage0_raw")), "4095");
        driver.write_outputs(&[0x9C, 0xFF]).expect("drive -100 mV");
        assert_eq!(sysfs.get(&attribute("out_voltage0_raw")), "0");
    }

    /// The IIO ABI writes an available range as `[min step max]` and a set as a list. Catches the
    /// step taken for the highest code, and a list without brackets refused.
    #[test]
    fn available_codes_read_as_a_range_or_a_list() {
        assert_eq!(parse_codes("[0 1 4095]\n"), Some((0, 4095)));
        assert_eq!(parse_codes("[-2048 16 2047]"), Some((-2048, 2047)));
        assert_eq!(parse_codes("0 255 1023\n"), Some((0, 1023)));
        assert_eq!(parse_codes("[0 4095]"), None);
        assert_eq!(parse_codes("low high"), None);
    }

    /// Specification 11 §6.5 and 34 TARGET-01: a computer refuses, when the driver starts, a line
    /// that is not one of its IIO channels, with the driver's name and the reason. Catches a chip's
    /// pin name or a missing device that fails later or without saying why.
    #[test]
    fn a_line_this_computer_has_no_channel_for_is_refused_with_the_reason() {
        let sysfs = FakeSysfs::new("analog-refusals");
        sysfs.set(&attribute("in_voltage1_raw"), "0\n");

        assert_eq!(
            refusal(reading(vec![input("GPIO2", "%IW0")]), &sysfs),
            "invalid config 'analog: inputs[0].line 'GPIO2' is not an IIO channel such as \
             iio:device0/in_voltage1'"
        );
        assert_eq!(
            refusal(
                reading(vec![input("iio:device3/in_voltage1", "%IW0")]),
                &sysfs
            ),
            "invalid config 'analog: this computer has no IIO device iio:device3'"
        );
        assert_eq!(
            refusal(
                reading(vec![input("iio:device0/in_voltage7", "%IW0")]),
                &sysfs
            ),
            "invalid config 'analog: IIO device iio:device0 has no channel in_voltage7'"
        );
        assert_eq!(
            refusal(
                reading(vec![input("iio:device0/in_voltage1", "%IW0")]),
                &sysfs
            ),
            "invalid config 'analog: IIO channel iio:device0/in_voltage1 has no scale: neither \
             in_voltage1_scale nor in_voltage_scale'"
        );
        assert_eq!(
            refusal(
                driving(vec![output("iio:device0/in_voltage1", "%QW0", 0)]),
                &sysfs
            ),
            "invalid config 'analog: outputs[0].line 'iio:device0/in_voltage1' is not an IIO \
             channel such as iio:device0/out_voltage0'"
        );
    }

    /// Specification 11 §6.5, as for GPIO: a failed read faults the driver's health with the
    /// error, and a later complete read makes it healthy again. A failed reading is an input-path
    /// condition, so an output handoff neither clears it nor leaves a safe-state handoff
    /// unconfirmed (specification 11, safe-state handoff confirmation). Catches a garbled reading
    /// taken as a value, a fault hidden by the next output handoff, and a fault that stays after
    /// the channel reads again.
    #[test]
    fn a_reading_that_is_not_a_count_faults_the_driver_until_a_good_one() {
        let sysfs = FakeSysfs::new("analog-fault");
        sysfs.set(&attribute("in_voltage1_raw"), "2048\n");
        sysfs.set(&attribute("in_voltage_scale"), SCALE_12_BIT);
        let mut driver = AnalogDriver::open(
            reading(vec![input("iio:device0/in_voltage1", "%IW0")]),
            sysfs.root(),
        )
        .expect("open the analog input");
        let mut image = [0; 2];

        sysfs.set(&attribute("in_voltage1_raw"), "busy\n");
        let error = driver
            .read_inputs(&mut image)
            .expect_err("a garbled reading must fail");
        assert!(
            error.to_string().contains("holds 'busy', not a count"),
            "{error}"
        );
        assert!(matches!(driver.health(), IoDriverHealth::Faulted { .. }));
        driver.write_outputs(&[]).expect("no output to drive");
        assert!(matches!(driver.health(), IoDriverHealth::Faulted { .. }));
        assert_eq!(driver.output_health(), IoDriverHealth::Ok);

        sysfs.set(&attribute("in_voltage1_raw"), "2048\n");
        driver.read_inputs(&mut image).expect("read again");
        assert_eq!(image, [0x72, 0x06]);
        assert_eq!(driver.health(), IoDriverHealth::Ok);
    }
}
