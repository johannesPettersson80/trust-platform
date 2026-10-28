//! The `counter` driver's parameters and count rules (specification 11 §6.5): inputs that count the
//! edges of a line into a `%ID` double word, or an encoder's A and B lines up and down.

use alloc::vec::Vec;

use smol_str::SmolStr;
use trust_runtime_core::io_address::{IoAddress, IoSize};
use trust_runtime_core::memory::IoArea;

use super::{parse_root, AddressKind, Once, ParamsError, Ranges, Table, LINE, LINE_ONCE};

/// An input's address: a `%ID` double word.
const ADDRESS: AddressKind = AddressKind {
    area: IoArea::Input,
    size: IoSize::DWord,
    prefix: "%ID",
    noun: "double word",
};

/// The `counter` driver's parameters.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CounterParams {
    /// `inputs`, in `io.toml` order.
    pub inputs: Vec<CounterInput>,
}

/// One counter input.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CounterInput {
    /// `address`, a concrete `%ID` double word.
    pub address: IoAddress,
    /// `line`: a chip's pin such as `GPIO5` (an encoder's A), or a Linux counter's count such as
    /// `counter0/count0`.
    pub line: SmolStr,
    /// `mode`, with its own keys.
    pub mode: CounterMode,
}

/// What a counter counts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CounterMode {
    /// `mode = "pulse"`, the default: the `edge`s of `line`, as a `UDINT` that wraps.
    Pulse {
        /// `edge`: `rising`, the default, `falling` or `both`.
        edge: Edge,
    },
    /// `mode = "quadrature"`: an encoder's A (`line`) and B (`line_b`), four counts per cycle of A,
    /// up while A leads B, as a `DINT` that wraps.
    Quadrature {
        /// `line_b`, the encoder's B: a chip counts it on a pin of its own and requires it, and
        /// Linux refuses it, as a counter device's own signals are the encoder's lines
        /// (specification 11 §6.5).
        line_b: Option<SmolStr>,
    },
}

/// The edges a pulse counter counts.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Edge {
    /// `rising`, the default.
    #[default]
    Rising,
    /// `falling`.
    Falling,
    /// `both`.
    Both,
}

/// The two words of `mode`.
#[derive(Clone, Copy)]
enum Counting {
    /// `pulse`.
    Pulse,
    /// `quadrature`.
    Quadrature,
}

impl CounterParams {
    /// The parameters of the JSON text `text`, the driver's `params` table (specification 34
    /// §5.1.1, section 8), checked by the rules of specification 11 §6.5.
    ///
    /// # Errors
    ///
    /// The first key that breaks a rule, with the reason.
    pub fn from_json(text: &str) -> Result<Self, ParamsError> {
        let map = parse_root(text)?;
        let root = Table::root(&map);
        root.only(&["inputs"], "the counter driver")?;
        let mut lines = Once::new(LINE_ONCE);
        let mut bytes = Ranges::new();
        let mut inputs = Vec::new();
        for entry in root.list("inputs")? {
            entry.only(
                &["address", "edge", "line", "line_b", "mode"],
                "a counter input",
            )?;
            let address = entry.address(&ADDRESS)?;
            bytes.claim(&entry, &address, 4, ADDRESS.prefix)?;
            let line = entry.required_line("line", LINE, &mut lines)?;
            let mode = counter_mode(&entry, &mut lines)?;
            inputs.push(CounterInput {
                address,
                line,
                mode,
            });
        }
        Ok(Self { inputs })
    }
}

/// An entry's `mode` with the key of its own: a pulse counter's `edge`, which a quadrature counter
/// refuses, and a quadrature counter's `line_b`, which a pulse counter refuses and which no other
/// key of the driver names (`lines`). Whether a quadrature counter needs `line_b` is the runtime's
/// hardware: a chip requires it and Linux refuses it.
fn counter_mode(entry: &Table<'_>, lines: &mut Once<SmolStr>) -> Result<CounterMode, ParamsError> {
    let counting = entry
        .choice(
            "mode",
            &[
                ("pulse", Counting::Pulse),
                ("quadrature", Counting::Quadrature),
            ],
        )?
        .unwrap_or(Counting::Pulse);
    match counting {
        Counting::Pulse => {
            let edge = entry
                .choice(
                    "edge",
                    &[
                        ("rising", Edge::Rising),
                        ("falling", Edge::Falling),
                        ("both", Edge::Both),
                    ],
                )?
                .unwrap_or_default();
            if entry.get("line_b").is_some() {
                return Err(entry.refuse(
                    "line_b",
                    "is refused with mode = \"pulse\": a pulse counter counts one line",
                ));
            }
            Ok(CounterMode::Pulse { edge })
        }
        Counting::Quadrature => {
            if entry.get("edge").is_some() {
                return Err(entry.refuse(
                    "edge",
                    "is refused with mode = \"quadrature\": a quadrature counter counts every \
                     edge of both lines",
                ));
            }
            let line_b = entry.line("line_b", LINE, lines)?;
            Ok(CounterMode::Quadrature { line_b })
        }
    }
}

/// A 32-bit count made from a 16-bit hardware counter that the driver reads once per cycle: each
/// read adds the change since the last read, taken as a signed 16-bit difference, so at most 32,767
/// counts may arrive between two reads (specification 11 §6.5).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WideCount {
    /// The hardware counter's value at the last read.
    last: i16,
    /// The count so far, which wraps.
    total: u32,
}

impl WideCount {
    /// A count of zero over a hardware counter that reads `start` now.
    #[must_use]
    pub fn new(start: i16) -> Self {
        Self {
            last: start,
            total: 0,
        }
    }

    /// Takes the hardware counter's value `raw` and returns the count: the total plus
    /// `raw − last` as a wrapping signed 16-bit difference, wrapping at 2^32.
    pub fn update(&mut self, raw: i16) -> u32 {
        let change = raw.wrapping_sub(self.last);
        self.last = raw;
        self.total = self.total.wrapping_add_signed(i32::from(change));
        self.total
    }

    /// The count as a `DINT` (a quadrature counter), the same 32 bits.
    #[must_use]
    pub fn signed(self) -> i32 {
        self.total.cast_signed()
    }
}

#[cfg(test)]
mod tests;
