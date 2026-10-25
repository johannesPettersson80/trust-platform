//! The `modbus-tcp` driver's parameter table (specification 11 §6.5): the keys, the defaults and the
//! function aliases of the computer's driver, read from the JSON text of the table (specification
//! 34 §5.1.1, section 8) and checked fail-closed, with the key and the reason.

use alloc::format;
use alloc::vec::Vec;
use core::fmt;
use core::net::SocketAddrV4;

use trust_modbus::FunctionCode;

use super::plan::RawPlan;
use super::point_map::{self, InputPoint, OutputPoint};
use super::word;
use crate::io_drivers::{parse_root, OnError, ParamsError, Table};

/// The keys of the parameter table.
const KEYS: [&str; 10] = [
    "address",
    "input_function",
    "input_points",
    "input_start",
    "on_error",
    "output_function",
    "output_points",
    "output_start",
    "timeout_ms",
    "unit_id",
];

/// What `address` must be.
const ADDRESS: &str = "an IPv4 address with a port from 1 to 65535, such as \"192.168.1.10:502\"";

/// `unit_id` when the table has none.
const DEFAULT_UNIT_ID: u8 = 1;

/// `timeout_ms` when the table has none.
const DEFAULT_TIMEOUT_MS: u32 = 500;

/// The words that name an input function, read as `word` reads them: the names of specification 11
/// and the aliases of the computer's driver.
pub(super) const INPUT_WORDS: [(&str, InputFunction); 20] = [
    ("read_coils", InputFunction::Coils),
    ("coil", InputFunction::Coils),
    ("coils", InputFunction::Coils),
    ("fc01", InputFunction::Coils),
    ("01", InputFunction::Coils),
    ("read_discrete_inputs", InputFunction::DiscreteInputs),
    ("discrete_inputs", InputFunction::DiscreteInputs),
    ("discrete", InputFunction::DiscreteInputs),
    ("fc02", InputFunction::DiscreteInputs),
    ("02", InputFunction::DiscreteInputs),
    ("read_holding_registers", InputFunction::HoldingRegisters),
    ("holding_registers", InputFunction::HoldingRegisters),
    ("holding", InputFunction::HoldingRegisters),
    ("fc03", InputFunction::HoldingRegisters),
    ("03", InputFunction::HoldingRegisters),
    ("read_input_registers", InputFunction::InputRegisters),
    ("input_registers", InputFunction::InputRegisters),
    ("input", InputFunction::InputRegisters),
    ("fc04", InputFunction::InputRegisters),
    ("04", InputFunction::InputRegisters),
];

/// The input functions a refusal lists.
pub(super) const INPUT_WHAT: &str =
    "read_coils, read_discrete_inputs, read_holding_registers or read_input_registers";

/// The words that name an output function, read as `word` reads them: the names of specification 11
/// and the aliases of the computer's driver, with `0f` and `10`, the hexadecimal codes of FC15 and
/// FC16.
pub(super) const OUTPUT_WORDS: [(&str, OutputFunction); 22] = [
    ("write_single_coil", OutputFunction::SingleCoil),
    ("single_coil", OutputFunction::SingleCoil),
    ("coil", OutputFunction::SingleCoil),
    ("fc05", OutputFunction::SingleCoil),
    ("05", OutputFunction::SingleCoil),
    ("write_single_register", OutputFunction::SingleRegister),
    ("single_register", OutputFunction::SingleRegister),
    ("register", OutputFunction::SingleRegister),
    ("fc06", OutputFunction::SingleRegister),
    ("06", OutputFunction::SingleRegister),
    ("write_multiple_coils", OutputFunction::MultipleCoils),
    ("multiple_coils", OutputFunction::MultipleCoils),
    ("coils", OutputFunction::MultipleCoils),
    ("fc15", OutputFunction::MultipleCoils),
    ("15", OutputFunction::MultipleCoils),
    ("0f", OutputFunction::MultipleCoils),
    (
        "write_multiple_registers",
        OutputFunction::MultipleRegisters,
    ),
    ("multiple_registers", OutputFunction::MultipleRegisters),
    ("registers", OutputFunction::MultipleRegisters),
    ("fc16", OutputFunction::MultipleRegisters),
    ("16", OutputFunction::MultipleRegisters),
    ("10", OutputFunction::MultipleRegisters),
];

/// The output functions a refusal lists.
pub(super) const OUTPUT_WHAT: &str =
    "write_single_coil, write_single_register, write_multiple_coils or write_multiple_registers";

/// The address of a Modbus TCP server: an IPv4 address and a port.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ModbusTcpAddress {
    /// The IPv4 address, its first number first: `192.168.1.10` is `[192, 168, 1, 10]`.
    pub ip: [u8; 4],
    /// The TCP port, 1 to 65535; 502 is the port of Modbus.
    pub port: u16,
}

impl fmt::Display for ModbusTcpAddress {
    /// The address as `io.toml` writes it, such as `192.168.1.10:502`.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{}",
            SocketAddrV4::new(self.ip.into(), self.port)
        )
    }
}

/// The function that reads the input image (specification 11 §6.5): FC01 to FC04.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputFunction {
    /// FC01 Read Coils, `read_coils`.
    Coils,
    /// FC02 Read Discrete Inputs, `read_discrete_inputs`.
    DiscreteInputs,
    /// FC03 Read Holding Registers, `read_holding_registers`.
    HoldingRegisters,
    /// FC04 Read Input Registers, `read_input_registers`, the default.
    InputRegisters,
}

impl InputFunction {
    /// The function's code in the `trust-modbus` crate.
    #[must_use]
    pub const fn code(self) -> FunctionCode {
        match self {
            Self::Coils => FunctionCode::ReadCoils,
            Self::DiscreteInputs => FunctionCode::ReadDiscreteInputs,
            Self::HoldingRegisters => FunctionCode::ReadHoldingRegisters,
            Self::InputRegisters => FunctionCode::ReadInputRegisters,
        }
    }

    /// Whether the function reads coils or discrete inputs, one bit each, rather than registers.
    #[must_use]
    pub const fn reads_bits(self) -> bool {
        matches!(self, Self::Coils | Self::DiscreteInputs)
    }
}

/// The function that writes the output image (specification 11 §6.5): FC05, FC06, FC15 or FC16.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFunction {
    /// FC05 Write Single Coil, `write_single_coil`.
    SingleCoil,
    /// FC06 Write Single Register, `write_single_register`.
    SingleRegister,
    /// FC15 Write Multiple Coils, `write_multiple_coils`.
    MultipleCoils,
    /// FC16 Write Multiple Registers, `write_multiple_registers`, the default.
    MultipleRegisters,
}

impl OutputFunction {
    /// The function's code in the `trust-modbus` crate.
    #[must_use]
    pub const fn code(self) -> FunctionCode {
        match self {
            Self::SingleCoil => FunctionCode::WriteSingleCoil,
            Self::SingleRegister => FunctionCode::WriteSingleRegister,
            Self::MultipleCoils => FunctionCode::WriteMultipleCoils,
            Self::MultipleRegisters => FunctionCode::WriteMultipleRegisters,
        }
    }

    /// Whether the function writes coils, one bit each, rather than registers.
    #[must_use]
    pub const fn writes_bits(self) -> bool {
        matches!(self, Self::SingleCoil | Self::MultipleCoils)
    }
}

/// The `modbus-tcp` driver's parameters.
#[derive(Debug, Clone, PartialEq)]
pub struct ModbusTcpParams {
    /// `address`: the server's IPv4 address and port; required.
    pub address: ModbusTcpAddress,
    /// `unit_id`: the unit identifier of every request, 0 to 255; default 1.
    pub unit_id: u8,
    /// `input_start`: the first address of a raw input read, 0 to 65535; default 0.
    pub input_start: u16,
    /// `output_start`: the first address of a raw output write, 0 to 65535; default 0.
    pub output_start: u16,
    /// `timeout_ms`: how long a connection and each response may take, from 1 ms; default 500.
    pub timeout_ms: u32,
    /// `on_error`; default `fault`.
    pub on_error: OnError,
    /// `input_function`; default [`InputFunction::InputRegisters`].
    pub input_function: InputFunction,
    /// `output_function`; default [`OutputFunction::MultipleRegisters`].
    pub output_function: OutputFunction,
    /// `input_points`, in `io.toml` order; empty for a raw input image.
    pub input_points: Vec<InputPoint>,
    /// `output_points`, in `io.toml` order; empty for a raw output image.
    pub output_points: Vec<OutputPoint>,
}

impl ModbusTcpParams {
    /// The parameters of the JSON text `text`, the driver's `params` table (specification 34
    /// §5.1.1, section 8), checked by the rules of specification 11 §6.5: an unknown key is
    /// refused, integers are JSON integers in their ranges, functions and types are the words of
    /// the computer's driver, and the points of one list neither share an address of one Modbus
    /// table nor overlap in the image.
    ///
    /// # Errors
    ///
    /// The first key that breaks a rule, with the reason.
    pub fn from_json(text: &str) -> Result<Self, ParamsError> {
        let map = parse_root(text)?;
        let root = Table::root(&map);
        root.only(&KEYS, "the modbus-tcp driver")?;
        let address = address(&root)?;
        let unit_id = root
            .integer("unit_id", 0, u8::MAX)?
            .unwrap_or(DEFAULT_UNIT_ID);
        let input_start = root.integer("input_start", 0, u16::MAX)?.unwrap_or(0);
        let output_start = root.integer("output_start", 0, u16::MAX)?.unwrap_or(0);
        let timeout_ms = root
            .integer("timeout_ms", 1, u32::MAX)?
            .unwrap_or(DEFAULT_TIMEOUT_MS);
        let on_error = on_error(&root)?;
        let input_function = word(&root, "input_function", &INPUT_WORDS, INPUT_WHAT)?
            .unwrap_or(InputFunction::InputRegisters);
        let output_function = word(&root, "output_function", &OUTPUT_WORDS, OUTPUT_WHAT)?
            .unwrap_or(OutputFunction::MultipleRegisters);
        let input_points = point_map::points(&root, input_function)?;
        let output_points = point_map::points(&root, output_function)?;
        Ok(Self {
            address,
            unit_id,
            input_start,
            output_start,
            timeout_ms,
            on_error,
            input_function,
            output_function,
            input_points,
            output_points,
        })
    }

    /// Checks the parameters against an input image of `input_len` bytes and an output image of
    /// `output_len` bytes, which a runtime knows once it has its program (specification 11 §6.5).
    /// Without a point map, the coils or registers that the image takes from `input_start` or
    /// `output_start` must end at address 65535; with one, the bytes of every point must lie in
    /// the image.
    ///
    /// # Errors
    ///
    /// The refusal of `input_start` or `output_start`, or of the `image_offset` of the first point
    /// outside its image.
    pub fn check_images(&self, input_len: usize, output_len: usize) -> Result<(), ParamsError> {
        if self.input_points.is_empty() {
            let function = self.input_function.code();
            RawPlan::new(function, self.input_start, input_len, "input")
                .map_err(|reason| ParamsError::new("input_start", reason))?;
        } else {
            point_map::check_image(&self.input_points, input_len)?;
        }
        if self.output_points.is_empty() {
            let function = self.output_function.code();
            RawPlan::new(function, self.output_start, output_len, "output")
                .map_err(|reason| ParamsError::new("output_start", reason))?;
        } else {
            point_map::check_image(&self.output_points, output_len)?;
        }
        Ok(())
    }
}

/// `address`: an IPv4 address and a port other than 0, as the computer's socket address reads it;
/// required.
fn address(root: &Table<'_>) -> Result<ModbusTcpAddress, ParamsError> {
    let Some(value) = root.get("address") else {
        return Err(root.refuse("address", format!("is required, {ADDRESS}")));
    };
    value
        .as_str()
        .and_then(|text| text.parse::<SocketAddrV4>().ok())
        .filter(|socket| socket.port() != 0)
        .map(|socket| ModbusTcpAddress {
            ip: socket.ip().octets(),
            port: socket.port(),
        })
        .ok_or_else(|| root.refuse("address", format!("must be {ADDRESS}, not {value}")))
}

/// `on_error`, read as the computer's driver reads it: trimmed, in any case, with `warning` for
/// `warn`; `fault` when absent (specification 11 §6.5). The `can` driver reads it the same way.
fn on_error(root: &Table<'_>) -> Result<OnError, ParamsError> {
    let Some(value) = root.get("on_error") else {
        return Ok(OnError::default());
    };
    match value
        .as_str()
        .map(|text| text.trim().to_ascii_lowercase())
        .as_deref()
    {
        Some("fault") => Ok(OnError::Fault),
        Some("warn" | "warning") => Ok(OnError::Warn),
        Some("ignore") => Ok(OnError::Ignore),
        _ => Err(root.refuse(
            "on_error",
            format!("must be \"fault\", \"warn\" or \"ignore\", not {value}"),
        )),
    }
}
