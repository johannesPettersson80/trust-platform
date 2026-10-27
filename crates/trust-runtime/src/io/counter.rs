//! The `counter` driver of a computer (specification 11 §6.5). On Linux an input is a count of the
//! counter subsystem, such as `counter0/count0`. For a pulse input the driver sets its `function`
//! to `increase` and the action of its signal to the input's `edge`, and reads its `count` at each
//! cycle start as the edges since its last read, which add up to the `UDINT` of a `%ID` double
//! word. For a quadrature input it sets `quadrature x4`, whose signals are the encoder's A and B,
//! and reads the count's change either way round, which adds up to a `DINT`. The
//! parameters are the shared ones of `trust_runtime_services::io_drivers::counter`; this module
//! sets and reads the count's files, as the counter ABI names them
//! (`Documentation/ABI/testing/sysfs-bus-counter`): `function` and `function_available`,
//! `signal<Z>_action` and `signal<Z>_action_available`, `enable`, `ceiling` and `count`.

use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use smol_str::SmolStr;
use trust_runtime_services::io_drivers::counter::{CounterInput, CounterMode, CounterParams, Edge};

use crate::error::RuntimeError;
use crate::io::{IoDriver, IoDriverHealth};

/// The driver's name, which starts each of its errors.
const NAME: &str = "counter";

/// The folder of the counter devices under the sysfs root.
const COUNTERS: &str = "bus/counter/devices";

/// The `counter` driver: its counts, in `io.toml` order, and its health.
pub(super) struct CounterDriver {
    inputs: Vec<CountChannel>,
    health: IoDriverHealth,
}

impl CounterDriver {
    /// The driver of `params` on this computer, whose sysfs is at `/sys`.
    ///
    /// # Errors
    ///
    /// The refusal of a line this computer has no count for, of a quadrature input's `line_b`, or
    /// of an edge or a function the count does not offer, and the failure to set a count up, each
    /// after the driver's name.
    pub(super) fn start(params: CounterParams) -> Result<Self, RuntimeError> {
        Self::open(params, Path::new("/sys"))
    }

    /// The driver of `params` over the sysfs tree at `sysfs`, each count set up and read once, so
    /// that its edges count from now.
    fn open(params: CounterParams, sysfs: &Path) -> Result<Self, RuntimeError> {
        let inputs = params
            .inputs
            .iter()
            .enumerate()
            .map(|(index, input)| CountChannel::open(sysfs, index, input))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self {
            inputs,
            health: IoDriverHealth::Ok,
        })
    }
}

impl IoDriver for CounterDriver {
    /// Reads each count into its double word. A failure faults the driver with its error, and a
    /// complete handoff makes it healthy again, as for GPIO.
    fn read_inputs(&mut self, inputs: &mut [u8]) -> Result<(), RuntimeError> {
        let result = self
            .inputs
            .iter_mut()
            .try_for_each(|channel| channel.read_into(inputs));
        self.health = match &result {
            Ok(()) => IoDriverHealth::Ok,
            Err(error) => IoDriverHealth::Faulted {
                error: SmolStr::new(error.to_string()),
            },
        };
        result
    }

    /// A counter driver has no outputs.
    fn write_outputs(&mut self, _outputs: &[u8]) -> Result<(), RuntimeError> {
        Ok(())
    }

    fn health(&self) -> IoDriverHealth {
        self.health.clone()
    }

    /// A counter driver has no output path, so a failed reading never leaves a safe-state
    /// handoff unconfirmed (specification 11, safe-state handoff confirmation).
    fn output_health(&self) -> IoDriverHealth {
        IoDriverHealth::Ok
    }
}

/// A count and the `%ID` double word it reads into.
struct CountChannel {
    /// The count's `count` file.
    count: PathBuf,
    /// The number of values the count takes before it wraps to 0: its ceiling + 1, or 2^64
    /// without a ceiling.
    span: u128,
    /// The count at the last read.
    last: u64,
    /// The edges since the driver started, a `UDINT` that wraps, or for a quadrature input the
    /// steps, the bits of a `DINT` that wraps.
    total: u32,
    /// Whether the count is an encoder's: its change is then read either way round.
    quadrature: bool,
    /// The double word's first byte in the input image.
    byte: usize,
}

impl CountChannel {
    /// The count that `input`, entry `index` of `inputs`, names under `sysfs`, set to count the
    /// edges of its `edge` and read once.
    fn open(sysfs: &Path, index: usize, input: &CounterInput) -> Result<Self, RuntimeError> {
        let line = input.line.as_str();
        let edge = match &input.mode {
            CounterMode::Pulse { edge } => Some(*edge),
            CounterMode::Quadrature { line_b: Some(_) } => {
                return Err(refused(format!(
                    "inputs[{index}].line_b is refused on Linux, where the encoder's A and B are \
                     the signals of {line}"
                )));
            }
            CounterMode::Quadrature { line_b: None } => None,
        };
        let parts = line
            .split_once('/')
            .filter(|(device, count)| numbered(device, "counter") && numbered(count, "count"));
        let Some((device, count)) = parts else {
            return Err(refused(format!(
                "inputs[{index}].line '{line}' is not a count such as counter0/count0"
            )));
        };
        let device_folder = sysfs.join(COUNTERS).join(device);
        if !device_folder.is_dir() {
            return Err(refused(format!(
                "this computer has no counter device {device}"
            )));
        }
        let folder = device_folder.join(count);
        if !folder.is_dir() {
            return Err(refused(format!(
                "counter device {device} has no count {count}"
            )));
        }
        match edge {
            Some(edge) => {
                choose(line, &folder, "function", "increase")?;
                let Some(signal) = first_signal(&folder)? else {
                    return Err(refused(format!("{line} has no signal to count")));
                };
                choose(
                    line,
                    &folder,
                    &format!("signal{signal}_action"),
                    action(edge),
                )?;
            }
            // The function sets the actions of both signals: every edge of A and of B.
            None => choose(line, &folder, "function", "quadrature x4")?,
        }
        enable(&folder)?;
        let span = count_span(line, &folder)?;
        let count = folder.join("count");
        let last = read_count(&count)?;
        Ok(Self {
            count,
            span,
            last,
            total: 0,
            quadrature: edge.is_none(),
            byte: input.address.byte as usize,
        })
    }

    /// Reads the count, adds the edges since the last read to the total, and writes the total to
    /// its double word of `image`.
    fn read_into(&mut self, image: &mut [u8]) -> Result<(), RuntimeError> {
        let now = read_count(&self.count)?;
        let change = if self.quadrature {
            steps(self.last, now, self.span)
        } else {
            edges(self.last, now, self.span)
        };
        self.total = self.total.wrapping_add(change);
        self.last = now;
        let len = image.len();
        let word = image
            .get_mut(self.byte..)
            .and_then(|rest| rest.first_chunk_mut::<4>())
            .ok_or_else(|| {
                RuntimeError::IoDriver(
                    format!(
                        "{NAME}: %ID{} is outside the input image of {len} bytes",
                        self.byte
                    )
                    .into(),
                )
            })?;
        *word = self.total.to_le_bytes();
        Ok(())
    }
}

/// The edges a count shows from `last` to `now` when it takes `span` values and wraps from the
/// last to 0, as the low 32 bits a `UDINT` adds.
fn edges(last: u64, now: u64, span: u128) -> u32 {
    let edges = (u128::from(now) + span - u128::from(last) % span) % span;
    // The total is a `UDINT` that wraps, so only the low 32 bits of the edges add to it.
    edges as u32
}

/// The steps an encoder's count shows from `last` to `now` when it takes `span` values and wraps
/// either way, taken the shorter way round, as the low 32 bits of the signed change that a `DINT`
/// adds: a count that went down by 3 adds `0xFFFF_FFFD`.
fn steps(last: u64, now: u64, span: u128) -> u32 {
    let forward = (u128::from(now) + span - u128::from(last) % span) % span;
    let signed = if forward > span / 2 {
        forward.cast_signed() - span.cast_signed()
    } else {
        forward.cast_signed()
    };
    // The total is a `DINT` that wraps, so only the low 32 bits of the steps add to it.
    signed as u32
}

/// The action of a signal that counts `edge`, in the counter ABI's words.
fn action(edge: Edge) -> &'static str {
    match edge {
        Edge::Rising => "rising edge",
        Edge::Falling => "falling edge",
        Edge::Both => "both edges",
    }
}

/// Sets the count's `attribute` in `folder` to `value` unless it holds it already, as a counter
/// with a single choice may not take a write. A value that its `<attribute>_available` does not
/// list is refused with the values that it lists.
fn choose(line: &str, folder: &Path, attribute: &str, value: &str) -> Result<(), RuntimeError> {
    let path = folder.join(attribute);
    let current = fs::read_to_string(&path).map_err(|error| failed("read", &path, &error))?;
    if current.trim() == value {
        return Ok(());
    }
    let available = folder.join(format!("{attribute}_available"));
    match fs::read_to_string(&available) {
        Ok(listed) if !listed.lines().any(|offered| offered.trim() == value) => {
            let offered: Vec<&str> = listed
                .lines()
                .map(str::trim)
                .filter(|offered| !offered.is_empty())
                .collect();
            return Err(refused(format!(
                "{line} cannot set {attribute} to '{value}'; it offers: {}",
                offered.join(", ")
            )));
        }
        Ok(_) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(failed("read", &available, &error)),
    }
    write_attribute(&path, value)
}

/// The lowest `Z` of the count's `signal<Z>_action` files in `folder`: the signal whose edges it
/// counts. `None` for a count without one.
fn first_signal(folder: &Path) -> Result<Option<u32>, RuntimeError> {
    let entries = fs::read_dir(folder).map_err(|error| failed("read", folder, &error))?;
    let mut signals = Vec::new();
    for entry in entries {
        let name = entry
            .map_err(|error| failed("read", folder, &error))?
            .file_name();
        if let Some(signal) = name.to_str().and_then(signal_of_action) {
            signals.push(signal);
        }
    }
    Ok(signals.into_iter().min())
}

/// The signal number `Z` of a file named `signal<Z>_action`.
fn signal_of_action(name: &str) -> Option<u32> {
    let digits = name.strip_prefix("signal")?.strip_suffix("_action")?;
    if is_decimal(digits) {
        digits.parse().ok()
    } else {
        None
    }
}

/// Enables the count in `folder` when it has an `enable` file that does not read 1: some counters,
/// the GPIO interrupt counter among them, start disabled.
fn enable(folder: &Path) -> Result<(), RuntimeError> {
    let path = folder.join("enable");
    match fs::read_to_string(&path) {
        Ok(state) if state.trim() == "1" => Ok(()),
        Ok(_) => write_attribute(&path, "1"),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(failed("read", &path, &error)),
    }
}

/// The number of values the count in `folder` takes before it wraps to 0: its `ceiling` + 1, or
/// 2^64 without a `ceiling` file. A count whose ceiling is 0 cannot count and is refused.
fn count_span(line: &str, folder: &Path) -> Result<u128, RuntimeError> {
    let path = folder.join("ceiling");
    match fs::read_to_string(&path) {
        Ok(text) => match text.trim().parse::<u64>() {
            Ok(0) => Err(refused(format!(
                "{line} has the ceiling 0, so it cannot count"
            ))),
            Ok(ceiling) => Ok(u128::from(ceiling) + 1),
            Err(_) => Err(malformed(&path, &text)),
        },
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(1_u128 << 64),
        Err(error) => Err(failed("read", &path, &error)),
    }
}

/// The count in the file at `path`.
fn read_count(path: &Path) -> Result<u64, RuntimeError> {
    let text = fs::read_to_string(path).map_err(|error| failed("read", path, &error))?;
    text.trim().parse().map_err(|_| malformed(path, &text))
}

/// Whether `text` is `prefix` followed by a decimal number, as `count0`.
fn numbered(text: &str, prefix: &str) -> bool {
    text.strip_prefix(prefix).is_some_and(is_decimal)
}

/// Whether `text` is a decimal number: digits only, at least one.
fn is_decimal(text: &str) -> bool {
    !text.is_empty() && text.bytes().all(|byte| byte.is_ascii_digit())
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

/// The refusal of `text`, read from the file at `path`, which should hold a number.
fn malformed(path: &Path, text: &str) -> RuntimeError {
    RuntimeError::IoDriver(
        format!(
            "{NAME}: {} holds '{}', not a number",
            path.display(),
            text.trim()
        )
        .into(),
    )
}

#[cfg(test)]
mod tests {
    use smol_str::SmolStr;
    use trust_runtime_services::io_drivers::counter::{
        CounterInput, CounterMode, CounterParams, Edge,
    };

    use super::CounterDriver;
    use crate::io::fake_sysfs::FakeSysfs;
    use crate::io::{IoAddress, IoDriver, IoDriverHealth};

    /// The fake count `counter0/count0`.
    const COUNT: &str = "bus/counter/devices/counter0/count0";

    fn attribute(name: &str) -> String {
        format!("{COUNT}/{name}")
    }

    /// A count that offers both functions and every edge, set to quadrature on both edges,
    /// disabled, at `count`.
    fn count(name: &str, count: &str) -> FakeSysfs {
        let sysfs = FakeSysfs::new(name);
        sysfs.set(&attribute("function"), "quadrature x4\n");
        sysfs.set(
            &attribute("function_available"),
            "increase\nquadrature x4\n",
        );
        sysfs.set(&attribute("signal0_action"), "both edges\n");
        sysfs.set(
            &attribute("signal0_action_available"),
            "none\nrising edge\nfalling edge\nboth edges\n",
        );
        sysfs.set(&attribute("enable"), "0\n");
        sysfs.set(&attribute("count"), count);
        sysfs
    }

    fn params(line: &str, mode: CounterMode) -> CounterParams {
        CounterParams {
            inputs: vec![CounterInput {
                address: IoAddress::parse("%ID4").expect("input address"),
                line: SmolStr::new(line),
                mode,
            }],
        }
    }

    fn pulse(edge: Edge) -> CounterMode {
        CounterMode::Pulse { edge }
    }

    fn open(sysfs: &FakeSysfs, edge: Edge) -> CounterDriver {
        CounterDriver::open(params("counter0/count0", pulse(edge)), sysfs.root())
            .expect("open the counter input")
    }

    /// The `%ID4` double word of an 8-byte input image after one read.
    fn read(driver: &mut CounterDriver) -> [u8; 4] {
        let mut image = [0; 8];
        driver.read_inputs(&mut image).expect("read the counter");
        [image[4], image[5], image[6], image[7]]
    }

    fn refusal(params: CounterParams, sysfs: &FakeSysfs) -> String {
        match CounterDriver::open(params, sysfs.root()) {
            Ok(_) => panic!("the counter driver must be refused"),
            Err(error) => error.to_string(),
        }
    }

    /// Specification 11 §6.5: on Linux the driver sets the count's `function` to `increase` with
    /// the synapse action of `edge`, here `falling edge` on its signal 0, and the count runs.
    /// Catches the quadrature function left in force, another edge counted, and a count left
    /// disabled, as the GPIO interrupt counter starts.
    #[test]
    fn a_pulse_input_sets_increase_and_the_action_of_its_edge() {
        let sysfs = count("counter-setup", "0\n");

        open(&sysfs, Edge::Falling);

        assert_eq!(sysfs.get(&attribute("function")), "increase");
        assert_eq!(sysfs.get(&attribute("signal0_action")), "falling edge");
        assert_eq!(sysfs.get(&attribute("enable")), "1");
    }

    /// Specification 11 §6.5: the input is the number of edges since the driver started, read as
    /// the count's change since its last read. A count at 1,000 when the driver starts reads 0,
    /// and 5 = `05 00 00 00` after it moves to 1,005. Catches the count's own value (1,000 and
    /// 1,005) taken as the input.
    #[test]
    fn the_input_is_the_edges_since_the_driver_started() {
        let sysfs = count("counter-edges", "1000\n");
        let mut driver = open(&sysfs, Edge::Rising);

        assert_eq!(read(&mut driver), [0, 0, 0, 0]);
        sysfs.set(&attribute("count"), "1005\n");
        assert_eq!(read(&mut driver), [5, 0, 0, 0]);
    }

    /// The counter ABI: a count wraps from its `ceiling` to 0. With the ceiling 65,535, a count
    /// that moves from 65,530 to 4 has seen 6 edges to 0 and 4 more, 10. Catches a wrap taken as a
    /// jump of 2^64 − 65,526 edges.
    #[test]
    fn a_count_that_wraps_at_its_ceiling_adds_the_edges_across_the_wrap() {
        let sysfs = count("counter-ceiling", "65530\n");
        sysfs.set(&attribute("ceiling"), "65535\n");
        let mut driver = open(&sysfs, Edge::Rising);

        sysfs.set(&attribute("count"), "4\n");
        assert_eq!(read(&mut driver), [10, 0, 0, 0]);
    }

    /// Specification 11 §6.5, as for GPIO: a failed read faults the driver's health with the
    /// error, and the next complete read makes it healthy again, with the edges that came in the
    /// meantime: from 0, a count of 7 after the failure reads 7. A counter has no output path, so
    /// its fault never leaves a safe-state handoff unconfirmed (specification 11, safe-state
    /// handoff confirmation). Catches a garbled count taken as edges, edges lost across a failed
    /// read, and a safe state refused because of an input.
    #[test]
    fn a_count_that_is_not_a_number_faults_the_driver_until_a_good_one() {
        let sysfs = count("counter-fault", "0\n");
        let mut driver = open(&sysfs, Edge::Rising);
        let mut image = [0; 8];

        sysfs.set(&attribute("count"), "busy\n");
        let error = driver
            .read_inputs(&mut image)
            .expect_err("a garbled count must fail");
        assert!(
            error.to_string().contains("holds 'busy', not a number"),
            "{error}"
        );
        assert!(matches!(driver.health(), IoDriverHealth::Faulted { .. }));
        assert_eq!(driver.output_health(), IoDriverHealth::Ok);

        sysfs.set(&attribute("count"), "7\n");
        assert_eq!(read(&mut driver), [7, 0, 0, 0]);
        assert_eq!(driver.health(), IoDriverHealth::Ok);
    }

    /// Specification 11 §6.5: the input is a `UDINT` that wraps from 4,294,967,295 to 0. From 0,
    /// 4,294,967,295 edges read `FF FF FF FF`, and 2 more wrap to 1. Catches a total that
    /// saturates or faults at the top.
    #[test]
    fn the_udint_wraps_from_4294967295_to_0() {
        let sysfs = count("counter-wrap", "0\n");
        let mut driver = open(&sysfs, Edge::Rising);

        sysfs.set(&attribute("count"), "4294967295\n");
        assert_eq!(read(&mut driver), [0xFF, 0xFF, 0xFF, 0xFF]);
        sysfs.set(&attribute("count"), "4294967297\n");
        assert_eq!(read(&mut driver), [1, 0, 0, 0]);
    }

    /// The counter ABI: a count with one function or one action, as the GPIO interrupt counter,
    /// may not take a write, so a value already in force is not written. The newline the fake
    /// files keep shows that the driver did not write them. Catches a write that such a counter
    /// refuses, which would refuse the driver.
    #[test]
    fn a_value_already_in_force_is_not_written() {
        let sysfs = count("counter-in-force", "0\n");
        sysfs.set(&attribute("function"), "increase\n");
        sysfs.set(&attribute("signal0_action"), "rising edge\n");
        sysfs.set(&attribute("enable"), "1\n");

        open(&sysfs, Edge::Rising);

        assert_eq!(sysfs.get(&attribute("function")), "increase\n");
        assert_eq!(sysfs.get(&attribute("signal0_action")), "rising edge\n");
        assert_eq!(sysfs.get(&attribute("enable")), "1\n");
    }

    /// Specification 11 §6.5 and 34 ARCH-13: an edge or a function the count does not offer is
    /// refused when the driver starts, with what it offers. The GPIO interrupt counter counts only
    /// rising edges. Catches a refusal deferred to the kernel's bare "Invalid argument".
    #[test]
    fn an_edge_or_a_function_the_count_does_not_offer_is_refused_with_what_it_offers() {
        let sysfs = count("counter-offers", "0\n");
        sysfs.set(&attribute("signal0_action"), "rising edge\n");
        sysfs.set(&attribute("signal0_action_available"), "rising edge\n");
        assert_eq!(
            refusal(params("counter0/count0", pulse(Edge::Falling)), &sysfs),
            "invalid config 'counter: counter0/count0 cannot set signal0_action to 'falling \
             edge'; it offers: rising edge'"
        );

        // The refused start above set the function already, so it is put back.
        sysfs.set(&attribute("function"), "quadrature x4\n");
        sysfs.set(
            &attribute("function_available"),
            "quadrature x4\npulse-direction\n",
        );
        assert_eq!(
            refusal(params("counter0/count0", pulse(Edge::Rising)), &sysfs),
            "invalid config 'counter: counter0/count0 cannot set function to 'increase'; it \
             offers: quadrature x4, pulse-direction'"
        );
    }

    /// Specification 11 §6.5: on Linux `line_b` is refused, as the counter device's own signals
    /// are the encoder's lines; a line that is not a count, a missing device and a missing count
    /// are refused when the driver starts, with the driver's name and the reason (34 TARGET-01).
    /// Catches a chip's second pin accepted on Linux and a line that fails later or without saying
    /// why.
    #[test]
    fn line_b_and_a_line_this_computer_has_no_count_for_are_refused() {
        let sysfs = count("counter-refusals", "0\n");

        assert_eq!(
            refusal(
                params(
                    "counter0/count0",
                    CounterMode::Quadrature {
                        line_b: Some(SmolStr::new("counter0/count1")),
                    },
                ),
                &sysfs
            ),
            "invalid config 'counter: inputs[0].line_b is refused on Linux, where the encoder's \
             A and B are the signals of counter0/count0'"
        );
        assert_eq!(
            refusal(params("GPIO5", pulse(Edge::Rising)), &sysfs),
            "invalid config 'counter: inputs[0].line 'GPIO5' is not a count such as \
             counter0/count0'"
        );
        assert_eq!(
            refusal(params("counter7/count0", pulse(Edge::Rising)), &sysfs),
            "invalid config 'counter: this computer has no counter device counter7'"
        );
        assert_eq!(
            refusal(params("counter0/count3", pulse(Edge::Rising)), &sysfs),
            "invalid config 'counter: counter device counter0 has no count count3'"
        );
    }

    /// Specification 11 §6.5: on Linux a quadrature input sets the count's `function` to
    /// `quadrature x4` and reads its change either way round as a `DINT`. With the ceiling 65,535
    /// and the count at 10 when the driver starts, a count of 7 is 3 steps back, -3 =
    /// `FD FF FF FF`; then 65,534 is 9 more back across the wrap (7 down to 0 and 65,535 down to
    /// 65,534), -12 = `F4 FF FF FF`; then 2 is 4 forward across the wrap, -8 = `F8 FF FF FF`.
    /// Catches the pulse function left in force, a step back read as 65,533 steps forward, and a
    /// total that does not go below 0.
    #[test]
    fn a_quadrature_input_counts_both_ways_as_a_dint() {
        let sysfs = count("counter-quadrature", "10\n");
        sysfs.set(&attribute("function"), "increase\n");
        sysfs.set(&attribute("ceiling"), "65535\n");
        let mut driver = CounterDriver::open(
            params("counter0/count0", CounterMode::Quadrature { line_b: None }),
            sysfs.root(),
        )
        .expect("open the quadrature input");

        assert_eq!(sysfs.get(&attribute("function")), "quadrature x4");
        assert_eq!(sysfs.get(&attribute("enable")), "1");
        sysfs.set(&attribute("count"), "7\n");
        assert_eq!(read(&mut driver), [0xFD, 0xFF, 0xFF, 0xFF]);
        sysfs.set(&attribute("count"), "65534\n");
        assert_eq!(read(&mut driver), [0xF4, 0xFF, 0xFF, 0xFF]);
        sysfs.set(&attribute("count"), "2\n");
        assert_eq!(read(&mut driver), [0xF8, 0xFF, 0xFF, 0xFF]);
    }
}
