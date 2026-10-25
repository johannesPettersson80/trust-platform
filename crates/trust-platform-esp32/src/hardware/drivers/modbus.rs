//! The `modbus-tcp` driver on the C6 (specification 11 §6.5, built-in driver 1; specification 34
//! TARGET-01): the shared exchange of `trust_runtime_services::io_drivers::modbus` over a TCP
//! connection of the chip's network (`trust_platform_net::ClientLink`). The exchange decides every
//! request, check and error; this module only carries its bytes.
//!
//! As on a computer, the scan side has a fixed deadline: at the cycle start the driver asks for a
//! fresh input snapshot and drives the network until the snapshot is settled or the deadline
//! passes, and after the tasks it hands the outputs off and drives the network until they are
//! settled or the deadline passes. A computer's worker thread makes the round trips meanwhile; the
//! chip, with one core and no threads, polls its network in that time. Between the scans the
//! network's idle work moves the bytes of any exchange still on its way.

use alloc::boxed::Box;
use alloc::format;
use alloc::rc::Rc;
use alloc::string::String;
use alloc::vec::Vec;
use core::cell::RefCell;

use trust_platform_net::smoltcp::wire::Ipv4Address;
use trust_platform_net::{ClientLink, LinkStatus};
use trust_runtime_core::error::RuntimeError;
use trust_runtime_services::chip::network::ChipNetwork;
use trust_runtime_services::chip::process_io::ChipDriver;
use trust_runtime_services::io_drivers::modbus::{ModbusExchange, ModbusPoll, ModbusTcpParams};
use trust_runtime_services::io_drivers::OnError;
use trust_runtime_services::status::IoDriverHealth;

use crate::hardware::clock::now_us;
use crate::hardware::C6Net;

/// The driver's name in `io.toml`.
pub(super) const NAME: &str = "modbus-tcp";

/// The bytes of the largest Modbus TCP frame, `trust_modbus::tcp::MAX_ADU_LEN`.
const FRAME_BYTES: usize = 260;

/// A `modbus-tcp` driver: the exchange, its connection and the network that carries it.
struct ModbusDriver {
    exchange: ModbusExchange,
    link: ClientLink,
    network: Rc<RefCell<C6Net>>,
    /// The send buffer the exchange writes its requests into.
    frame: Box<[u8; FRAME_BYTES]>,
    /// The bytes the link received, handed to the exchange.
    received: Vec<u8>,
    /// Whether the exchange has been told the connection is open.
    open: bool,
}

/// The `modbus-tcp` driver of `params`, its connection carried by `network`.
///
/// # Errors
///
/// `modbus-tcp: <key>: <reason>` for parameters the shared rules refuse, and
/// `modbus-tcp: the device serves no network: name one in device.toml` without a network.
pub(super) fn claim(
    params: &str,
    network: Option<&Rc<RefCell<C6Net>>>,
) -> Result<Box<dyn ChipDriver>, String> {
    let params = ModbusTcpParams::from_json(params).map_err(|error| format!("{NAME}: {error}"))?;
    let network = network
        .ok_or_else(|| format!("{NAME}: the device serves no network: name one in device.toml"))?;
    let [a, b, c, d] = params.address.ip;
    let link = network
        .borrow_mut()
        .client((Ipv4Address::new(a, b, c, d), params.address.port));
    Ok(Box::new(ModbusDriver {
        exchange: ModbusExchange::new(params, now_ms()),
        link,
        network: Rc::clone(network),
        frame: Box::new([0; FRAME_BYTES]),
        received: Vec::new(),
        open: false,
    }))
}

impl ModbusDriver {
    /// Carries the exchange's steps over the link at `now`: what became of the connection, the
    /// bytes that came, and the requests and closes the exchange asks for.
    fn carry(&mut self, now: u64) {
        match self.link.status() {
            LinkStatus::Open if !self.open => {
                self.open = true;
                self.exchange.connected();
            }
            LinkStatus::Closed(reason) => {
                self.open = false;
                self.exchange.failed(now, reason);
            }
            _ => {}
        }
        self.link.receive(&mut self.received);
        if !self.received.is_empty() {
            self.exchange.received(now, &self.received);
            self.received.clear();
        }
        loop {
            match self.exchange.poll(now, &mut self.frame) {
                ModbusPoll::Connect(_) => {
                    self.open = false;
                    self.link.open();
                }
                ModbusPoll::Send(len) => {
                    let frame = self.frame.get(..len).unwrap_or_default();
                    if self.link.send(frame).is_err() {
                        self.exchange
                            .failed(now, "the connection took no more bytes to send");
                    }
                }
                ModbusPoll::Close => {
                    self.open = false;
                    self.link.close();
                }
                ModbusPoll::Wait(_) => break,
            }
        }
    }

    /// Drives the network and the exchange until `settled` says the scan side's work is done or
    /// `deadline` passes, in milliseconds of the chip's clock.
    fn drive_until(&mut self, deadline: u64, settled: fn(&ModbusExchange) -> bool) {
        loop {
            let now = now_ms();
            self.carry(now);
            if settled(&self.exchange) || now >= deadline {
                return;
            }
            self.network.borrow_mut().poll();
        }
    }
}

impl ChipDriver for ModbusDriver {
    fn name(&self) -> &str {
        NAME
    }

    /// Asks for a fresh input snapshot, drives the exchange until it is settled or the scan-side
    /// deadline passes, and copies it or applies `on_error` (specification 11, "Modbus
    /// default-fault and deadline integrity").
    fn read_inputs(&mut self, image: &mut [u8]) -> Result<(), RuntimeError> {
        let deadline = self.exchange.request_inputs(now_ms(), image.len());
        self.drive_until(deadline, ModbusExchange::inputs_settled);
        self.exchange.read_inputs(now_ms(), image)
    }

    /// Hands the output image off, drives the exchange until it is settled or the scan-side
    /// deadline passes, and gives its result under `on_error`.
    fn write_outputs(&mut self, image: &[u8]) -> Result<(), RuntimeError> {
        let deadline = self.exchange.write_outputs(now_ms(), image);
        self.drive_until(deadline, ModbusExchange::outputs_settled);
        self.exchange.output_result(now_ms())
    }

    fn health(&self) -> IoDriverHealth {
        self.exchange.health()
    }

    fn output_health(&self) -> IoDriverHealth {
        self.exchange.output_health()
    }

    fn on_error(&self) -> OnError {
        self.exchange.params().on_error
    }
}

impl Drop for ModbusDriver {
    /// Closes the connection; the network gives its socket back once no driver holds the link.
    fn drop(&mut self) {
        self.link.close();
    }
}

/// The chip's clock in milliseconds.
fn now_ms() -> u64 {
    now_us() / 1_000
}
