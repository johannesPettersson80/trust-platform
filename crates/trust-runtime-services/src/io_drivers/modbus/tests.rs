//! Tests of the `modbus-tcp` driver against specification 11 §6.5 and the Modbus specifications
//! (Modbus Application Protocol V1.1b3, Modbus Messaging on TCP/IP V1.0b). Every expected value is
//! worked out by hand from them, in the comment beside it, and each test names the wrong behaviour
//! it catches.

mod conversions;
mod exchange;
mod params;
mod plan;
mod points;
mod wire;

use alloc::format;
use alloc::vec::Vec;

use smol_str::SmolStr;
use trust_modbus::tcp::MAX_ADU_LEN;
use trust_runtime_core::error::RuntimeError;

use super::{ModbusExchange, ModbusPoll, ModbusTcpAddress, ModbusTcpParams};
use crate::status::IoDriverHealth;

/// The server of every exchange test.
const SERVER: ModbusTcpAddress = ModbusTcpAddress {
    ip: [10, 0, 0, 5],
    port: 502,
};

/// The error and the health of an input refresh that is not complete at its deadline
/// (specification 11, "Modbus default-fault and deadline integrity").
const INPUT_PENDING: &str = "modbus input refresh pending";

/// The error and the health of an output handoff that is not complete at its deadline.
const OUTPUT_PENDING: &str = "modbus output handoff pending";

/// An exchange with the server, and the transport's send buffer, which the tests drive as a
/// transport would.
struct Bench {
    /// The exchange.
    exchange: ModbusExchange,
    /// The send buffer.
    frame: [u8; MAX_ADU_LEN],
}

impl Bench {
    /// The exchange of the server with `fields`, more members of the JSON table, started at 0 ms.
    fn new(fields: &str) -> Self {
        let separator = if fields.is_empty() { "" } else { "," };
        let text = format!(r#"{{"address":"10.0.0.5:502"{separator}{fields}}}"#);
        let params = ModbusTcpParams::from_json(&text).expect("valid parameters");
        Self {
            exchange: ModbusExchange::new(params, 0),
            frame: [0; MAX_ADU_LEN],
        }
    }

    /// What the transport does at `now_ms`.
    fn poll(&mut self, now_ms: u64) -> ModbusPoll {
        self.exchange.poll(now_ms, &mut self.frame)
    }

    /// The request that the transport sends at `now_ms`.
    fn sent(&mut self, now_ms: u64) -> Vec<u8> {
        match self.poll(now_ms) {
            ModbusPoll::Send(len) => self.frame[..len].to_vec(),
            other => panic!("expected a request at {now_ms} ms, got {other:?}"),
        }
    }

    /// Opens the connection at `now_ms`: the exchange asks for the server, which accepts.
    fn connect(&mut self, now_ms: u64) {
        assert_eq!(self.poll(now_ms), ModbusPoll::Connect(SERVER));
        self.exchange.connected();
    }
}

/// A degraded health with `error`.
fn degraded(error: &str) -> IoDriverHealth {
    IoDriverHealth::Degraded {
        error: SmolStr::new(error),
    }
}

/// A faulted health with `error`.
fn faulted(error: &str) -> IoDriverHealth {
    IoDriverHealth::Faulted {
        error: SmolStr::new(error),
    }
}

/// The transport error `text` of a scan call under `on_error = "fault"`.
fn transport(text: &str) -> Result<(), RuntimeError> {
    Err(RuntimeError::IoTransport(text.into()))
}
