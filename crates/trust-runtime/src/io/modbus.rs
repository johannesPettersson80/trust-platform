//! The `modbus-tcp` driver of a computer (specification 11 §6.5, built-in driver 1). Its logic is
//! the shared exchange of `trust_runtime_services::io_drivers::modbus`, the one implementation that
//! every runtime calls (specification 34 ARCH-11): the parameters, the point maps, the requests,
//! the checks of each response, the scan-side deadline, `on_error`, the reconnect backoff and the
//! health. This module is the driver's `IoDriver` side, and its worker thread (`worker`) is the
//! transport that carries the exchange's bytes over a TCP connection.
//!
//! A scan boundary asks the exchange for the work and waits for the worker until the exchange has
//! settled it or the fixed scan-side deadline that the exchange gives has passed, then takes the
//! exchange's result (specification 11, "Modbus default-fault and deadline integrity"). The scan
//! never waits on the network, only on the exchange, which the worker changes under the shared lock
//! and never holds while it waits for the network.

mod worker;

#[cfg(test)]
mod tests;

use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};
use std::thread;
use std::time::{Duration, Instant};

use smol_str::SmolStr;
use trust_runtime_services::io_drivers::modbus::{ModbusExchange, ModbusTcpParams};

use super::registry::shared_params;
use crate::error::RuntimeError;
use crate::io::{IoDriver, IoDriverHealth};

/// The driver's name in `io.toml`, which starts each refusal of its parameters.
const NAME: &str = "modbus-tcp";

/// A `modbus-tcp` driver: the exchange that it shares with the worker thread carrying it.
///
/// Dropping the driver asks the worker to stop and returns without waiting for it (specification
/// 11 §6.5: the drop returns within its bounded deadline even while the worker awaits a response;
/// the detached worker ends at its next step and closes the connection).
#[derive(Debug)]
pub struct ModbusTcpDriver {
    /// The exchange and the signals between the scan and the worker.
    shared: Arc<Shared>,
}

/// What the scan side and the worker share.
#[derive(Debug)]
struct Shared {
    /// The exchange, and whether the driver asked the worker to stop.
    state: Mutex<State>,
    /// Wakes the worker: the scan gave the exchange work, or the driver asks the worker to stop.
    to_worker: Condvar,
    /// Wakes the scan: the worker told the exchange what became of the connection or a request.
    to_scan: Condvar,
    /// The start of the exchange's clock, from which its milliseconds count.
    started: Instant,
}

/// The state behind the shared lock.
#[derive(Debug)]
struct State {
    /// The exchange.
    exchange: ModbusExchange,
    /// Whether the driver was dropped, so the worker is to stop.
    stop: bool,
}

impl Shared {
    /// The milliseconds of the exchange's clock, a monotonic clock that starts with the driver.
    fn now_ms(&self) -> u64 {
        u64::try_from(self.started.elapsed().as_millis()).unwrap_or(u64::MAX)
    }

    /// The state, also after a thread panicked while it held the lock: the exchange keeps its own
    /// state consistent between its calls.
    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Waits, from the scan side, until `settled` says the exchange has settled the scan's work or
    /// `deadline_ms` of the exchange's clock has passed, and gives the state back.
    fn wait_for_scan<'a>(
        &'a self,
        mut state: MutexGuard<'a, State>,
        deadline_ms: u64,
        settled: fn(&ModbusExchange) -> bool,
    ) -> MutexGuard<'a, State> {
        loop {
            let now = self.now_ms();
            if settled(&state.exchange) || now >= deadline_ms {
                return state;
            }
            state = self
                .to_scan
                .wait_timeout(state, Duration::from_millis(deadline_ms - now))
                .unwrap_or_else(PoisonError::into_inner)
                .0;
        }
    }
}

impl ModbusTcpDriver {
    /// Checks `value`, the driver's `params` table in `io.toml`, by the shared rules of
    /// specification 11 §6.5, as a chip checks it (specification 34 §5.1.1, section 8). It opens
    /// no connection and starts no worker (specification 11 §6.5: validation constructs no driver).
    ///
    /// # Errors
    ///
    /// `modbus-tcp: <key>: <reason>` for the first key that the rules refuse.
    pub fn validate_params(value: &toml::Value) -> Result<(), RuntimeError> {
        Self::read_params(value).map(|_| ())
    }

    /// The driver of `value`, the driver's `params` table in `io.toml`, its worker started.
    ///
    /// # Errors
    ///
    /// The refusal of [`ModbusTcpDriver::validate_params`], and the failure to start the worker.
    pub fn from_params(value: &toml::Value) -> Result<Self, RuntimeError> {
        Self::start(Self::read_params(value)?)
    }

    /// The driver of `params`: the exchange, started now with no connection, and the worker thread
    /// that carries it. The worker connects when the first scan boundary has work for it.
    ///
    /// # Errors
    ///
    /// `modbus-tcp: start the worker: <reason>` when the operating system starts no thread.
    fn start(params: ModbusTcpParams) -> Result<Self, RuntimeError> {
        let shared = Arc::new(Shared {
            state: Mutex::new(State {
                exchange: ModbusExchange::new(params, 0),
                stop: false,
            }),
            to_worker: Condvar::new(),
            to_scan: Condvar::new(),
            started: Instant::now(),
        });
        let carried = Arc::clone(&shared);
        thread::Builder::new()
            .name(String::from(NAME))
            .spawn(move || worker::run(&carried))
            .map_err(|error| {
                RuntimeError::IoDriver(format!("{NAME}: start the worker: {error}").into())
            })?;
        Ok(Self { shared })
    }

    /// The parameters of `value` by the shared rules, read from the JSON text of the table as a
    /// chip reads them.
    fn read_params(value: &toml::Value) -> Result<ModbusTcpParams, RuntimeError> {
        shared_params(NAME, value, ModbusTcpParams::from_json)
    }
}

impl IoDriver for ModbusTcpDriver {
    /// Asks the exchange at the cycle start for a fresh input snapshot, waits for the worker until
    /// the refresh settles or the scan-side deadline passes, and copies the snapshot or applies
    /// `on_error` (specification 11, "Modbus default-fault and deadline integrity").
    fn read_inputs(&mut self, inputs: &mut [u8]) -> Result<(), RuntimeError> {
        let shared = &self.shared;
        let mut state = shared.lock();
        let deadline = state.exchange.request_inputs(shared.now_ms(), inputs.len());
        shared.to_worker.notify_one();
        let mut state = shared.wait_for_scan(state, deadline, ModbusExchange::inputs_settled);
        state.exchange.read_inputs(shared.now_ms(), inputs)
    }

    /// Hands the output image off after the tasks, waits for the worker until the handoff settles
    /// or the scan-side deadline passes, and gives its result under `on_error`. A handoff pending
    /// at the deadline stays accepted and is written when the server answers.
    fn write_outputs(&mut self, outputs: &[u8]) -> Result<(), RuntimeError> {
        let shared = &self.shared;
        let mut state = shared.lock();
        let deadline = state.exchange.write_outputs(shared.now_ms(), outputs);
        shared.to_worker.notify_one();
        let mut state = shared.wait_for_scan(state, deadline, ModbusExchange::outputs_settled);
        state.exchange.output_result(shared.now_ms())
    }

    /// The more severe of the input and the output path's health, the exchange's.
    fn health(&self) -> IoDriverHealth {
        self.shared.lock().exchange.health()
    }

    /// The output path's health, on which a safe-state handoff's confirmation depends: a failed or
    /// pending input refresh does not change it (specification 11, "Safe-state handoff
    /// confirmation").
    fn output_health(&self) -> IoDriverHealth {
        self.shared.lock().exchange.output_health()
    }

    /// The bytes of the input image its configuration fills (specification 42, §5 item 4): each
    /// input point its own, and a block the image from its start.
    fn input_bytes(&self) -> Vec<std::ops::Range<usize>> {
        let state = self.shared.lock();
        let points = &state.exchange.params().input_points;
        if points.is_empty() {
            return std::iter::once(0..usize::MAX).collect();
        }
        crate::io::joined_bytes(
            points
                .iter()
                .map(|point| {
                    let start = usize::try_from(point.image_offset).unwrap_or(usize::MAX);
                    let len = usize::try_from(point.data_type.image_bytes()).unwrap_or(usize::MAX);
                    start..start.saturating_add(len)
                })
                .collect(),
        )
    }

    /// The server and unit it reads and, after an exception reply, its code and name
    /// (specification 42, §4 item 9): "10.0.0.5:502 unit 1, exception 02 illegal data address".
    fn detail(&self) -> Option<SmolStr> {
        let state = self.shared.lock();
        let exchange = &state.exchange;
        let params = exchange.params();
        Some(detail_words(
            &params.address,
            params.unit_id,
            exchange.exception_code(),
        ))
    }
}

/// The driver's words for the server `server` and its unit `unit`, with the exception reply
/// `exception` that the driver reports while it reports one (specification 42, §4 item 9).
fn detail_words(server: &impl std::fmt::Display, unit: u8, exception: Option<u8>) -> SmolStr {
    let exception = exception
        .map(|code| format!(", exception {code:02X} {}", exception_name(code)))
        .unwrap_or_default();
    SmolStr::new(format!("{server} unit {unit}{exception}"))
}

/// An exception code's name in the Modbus application protocol (V1.1b3, section 7).
fn exception_name(code: u8) -> &'static str {
    match code {
        1 => "illegal function",
        2 => "illegal data address",
        3 => "illegal data value",
        4 => "server device failure",
        5 => "acknowledge",
        6 => "server device busy",
        8 => "memory parity error",
        10 => "gateway path unavailable",
        11 => "gateway target device failed to respond",
        _ => "unknown exception",
    }
}

impl Drop for ModbusTcpDriver {
    /// Asks the worker to stop and wakes it, without waiting for it to end.
    fn drop(&mut self) {
        self.shared.lock().stop = true;
        self.shared.to_worker.notify_all();
    }
}
