//! The `mqtt` driver of a computer (specification 32; specification 11 §6.5). Its logic is the
//! shared exchange of `trust_runtime_services::io_drivers::mqtt`, the one implementation that every
//! runtime calls (specification 34 ARCH-11): the parameters of `io.toml`, the MQTT 3.1.1 packets of
//! the client, the typed points and their payloads, the Sparkplug B profile, the session with the
//! broker and its reconnect backoff, `on_error`, and the health of the input and the output path.
//! This module is the driver's `IoDriver` side; `tls` builds the computer's TLS client from the
//! files that the parameters name, and the worker thread (`worker`) is the transport that carries
//! the exchange's bytes over a TCP connection, with TLS when the parameters ask for it.
//!
//! The computer gives the exchange what the shared module leaves to each runtime: the client
//! identifier `trust-runtime-<process id>` when the parameters give none, the wall clock that the
//! Sparkplug B timestamps count from, the TLS files and the TLS connection, and the transport.
//! Symbolic `mappings` reach the driver lowered to typed points by `super::mqtt_tag_mapping`, which
//! holds the program's symbols (specification 32 §5).
//!
//! The scan never waits on the network, only on the exchange, which the worker changes under the
//! shared lock and never holds while it waits for the network. A read takes the messages that the
//! worker has received, without waiting (specification 32 §8). A write waits at most
//! [`SCAN_WAIT_MS`] while the exchange says that the handoff before it, or the session being
//! opened, has not settled, and then hands its image to the exchange (specification 11, deadline
//! integrity); the worker sends it.

mod tls;
mod worker;

#[cfg(test)]
mod tests;

use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use native_tls::TlsConnector;
use trust_runtime_services::io_drivers::mqtt::{MqttExchange, MqttParams, SCAN_WAIT_MS};

use super::registry::shared_params;
use crate::error::RuntimeError;
use crate::io::{IoDriver, IoDriverHealth};

/// The driver's name in `io.toml`, which starts each refusal of its parameters by the shared rules.
const NAME: &str = "mqtt";

/// An `mqtt` driver: the exchange that it shares with the worker thread carrying it.
///
/// Dropping the driver asks the worker to end the session and stop, and returns without waiting
/// for it (specification 32 §8: the drop returns within its bounded deadline even while a first
/// connection is pending; the detached worker ends the session in order, closes the connection and
/// ends).
#[derive(Debug)]
pub struct MqttIoDriver {
    /// The exchange and the signals between the scan and the worker.
    shared: Arc<Shared>,
}

/// What the scan side and the worker share.
#[derive(Debug)]
struct Shared {
    /// The exchange, and whether the driver asked the worker to stop.
    state: Mutex<State>,
    /// Wakes the worker: the scan handed an image off, or the driver asks the worker to stop.
    to_worker: Condvar,
    /// Wakes the scan: the worker told the exchange what became of the connection or its bytes.
    to_scan: Condvar,
    /// The start of the exchange's clock, from which its milliseconds count.
    started: Instant,
}

/// The state behind the shared lock.
#[derive(Debug)]
struct State {
    /// The exchange.
    exchange: MqttExchange,
    /// Whether the driver was dropped, so the worker is to end the session and stop.
    stop: bool,
}

impl Shared {
    /// The milliseconds of the exchange's clock, a monotonic clock that starts with the driver.
    fn now_ms(&self) -> u64 {
        u64::try_from(self.started.elapsed().as_millis()).unwrap_or(u64::MAX)
    }

    /// The exchange's time now, with its wall clock set from the computer's, which the Sparkplug B
    /// timestamps count from; the clock is set anew each time, so that it follows the computer's.
    fn now(&self, state: &mut State) -> u64 {
        let now = self.now_ms();
        state.exchange.set_clock(now, wall_clock_ms());
        now
    }

    /// The state, also after a thread panicked while it held the lock: the exchange keeps its own
    /// state consistent between its calls.
    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Waits, from the scan side, while the exchange says that a write would wait for the
    /// transport, at most [`SCAN_WAIT_MS`], and gives the state back.
    fn settled<'a>(&'a self, mut state: MutexGuard<'a, State>) -> MutexGuard<'a, State> {
        let deadline = self.now_ms().saturating_add(SCAN_WAIT_MS);
        loop {
            let now = self.now_ms();
            if now >= deadline || !state.exchange.output_settling(now) {
                return state;
            }
            state = self
                .to_scan
                .wait_timeout(state, Duration::from_millis(deadline - now))
                .unwrap_or_else(PoisonError::into_inner)
                .0;
        }
    }
}

/// The milliseconds since the Unix epoch of the computer's clock; 0 for a clock before it.
fn wall_clock_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |since| {
            u64::try_from(since.as_millis()).unwrap_or(u64::MAX)
        })
}

impl MqttIoDriver {
    /// Checks `value`, the driver's `params` table in `io.toml`, by the shared rules of
    /// specification 32, as a chip checks it (specification 34 §5.1.1, section 8), and builds the
    /// TLS client from the files that it names, so that the configuration is checked completely
    /// before any worker starts (specification 32 §2). It opens no connection and starts no worker
    /// (specification 11 §6.5: validation constructs no driver).
    ///
    /// # Errors
    ///
    /// `mqtt: <key>: <reason>` for the first key that the rules refuse, and the refusal of a TLS
    /// file that cannot be read or does not hold what its key names.
    pub fn validate_params(value: &toml::Value) -> Result<(), RuntimeError> {
        let params = Self::read_params(value)?;
        tls::client(&params).map(drop)
    }

    /// The driver of `value`, the driver's `params` table in `io.toml`, its worker started.
    ///
    /// # Errors
    ///
    /// The refusal of [`MqttIoDriver::validate_params`], and the failure to start the worker.
    pub fn from_params(value: &toml::Value) -> Result<Self, RuntimeError> {
        let params = Self::read_params(value)?;
        let tls = tls::client(&params)?;
        Self::start(params, tls)
    }

    /// Whether `requested` keeps the authentication context of `stored`, under which a redacted
    /// edit may keep a stored password (specification 32 §3): both tables are read by the shared
    /// rules that set up the connection, which compare the broker, the TLS paths and ALPN, the user
    /// name, the client identifier and `allow_insecure_remote`, defaults included.
    ///
    /// # Errors
    ///
    /// The refusal of either table by the shared rules.
    pub(crate) fn same_credential_context(
        requested: &toml::Value,
        stored: &toml::Value,
    ) -> Result<bool, RuntimeError> {
        let requested = Self::read_params(requested)?;
        let stored = Self::read_params(stored)?;
        Ok(requested.same_credential_context(&stored))
    }

    /// The parameters of `value` by the shared rules, read from the JSON text of the table as a
    /// chip reads them (specification 11 §6.5). The paths of the TLS files are checked as text;
    /// [`MqttIoDriver::validate_params`] reads the files.
    ///
    /// # Errors
    ///
    /// `mqtt: <key>: <reason>` for the first key that the rules refuse.
    pub(super) fn read_params(value: &toml::Value) -> Result<MqttParams, RuntimeError> {
        shared_params(NAME, value, MqttParams::from_json)
    }

    /// The driver of `params`: the exchange, started now with no connection, and the worker thread
    /// that carries it with the TLS client `tls`. The worker opens the connection at once. A driver
    /// whose configuration exchanges neither inputs nor outputs, such as an empty `mappings` list
    /// (specification 32 §4), needs no connection and starts no worker.
    ///
    /// # Errors
    ///
    /// `mqtt: start the worker: <reason>` when the operating system starts no thread.
    fn start(params: MqttParams, tls: Option<TlsConnector>) -> Result<Self, RuntimeError> {
        let carries = params.input_enabled() || params.output_enabled();
        let client_id = format!("trust-runtime-{}", std::process::id());
        let shared = Arc::new(Shared {
            state: Mutex::new(State {
                exchange: MqttExchange::new(params, &client_id, 0),
                stop: false,
            }),
            to_worker: Condvar::new(),
            to_scan: Condvar::new(),
            started: Instant::now(),
        });
        if carries {
            let carried = Arc::clone(&shared);
            thread::Builder::new()
                .name(String::from(NAME))
                .spawn(move || worker::run(&carried, tls.as_ref()))
                .map_err(|error| {
                    RuntimeError::IoDriver(format!("{NAME}: start the worker: {error}").into())
                })?;
        }
        Ok(Self { shared })
    }
}

impl IoDriver for MqttIoDriver {
    /// Takes the messages that the worker received since the last read into the input image,
    /// without waiting for the broker, or applies `on_error` (specification 32 §8).
    fn read_inputs(&mut self, inputs: &mut [u8]) -> Result<(), RuntimeError> {
        let shared = &self.shared;
        let mut state = shared.lock();
        let now = shared.now_ms();
        state.exchange.read_inputs(now, inputs)
    }

    /// Hands the output image off after the tasks: waits at most [`SCAN_WAIT_MS`] while the
    /// handoff before it, or the session being opened, settles, hands the image to the exchange
    /// and wakes the worker that sends it, and gives the exchange's result under `on_error`
    /// (specification 11, deadline integrity). An image that cannot be handed off yet waits in the
    /// exchange as the newest and is sent when the session can take it.
    fn write_outputs(&mut self, outputs: &[u8]) -> Result<(), RuntimeError> {
        let shared = &self.shared;
        shared.to_worker.notify_one();
        let mut state = shared.settled(shared.lock());
        let now = shared.now(&mut state);
        let result = state.exchange.write_outputs(now, outputs);
        drop(state);
        shared.to_worker.notify_one();
        result
    }

    /// The more severe of the input and the output path's health, degraded by an overflow of the
    /// received messages (specification 32 §8, specification 34 §11.1), the exchange's.
    fn health(&self) -> IoDriverHealth {
        self.shared.lock().exchange.health()
    }

    /// The output path's health, on which a safe-state handoff's confirmation depends: stale or
    /// missing input and an overflow of the received messages do not change it (specification 11,
    /// "Safe-state handoff confirmation").
    fn output_health(&self) -> IoDriverHealth {
        self.shared.lock().exchange.output_health()
    }
}

impl Drop for MqttIoDriver {
    /// Asks the worker to end the session and stop, and wakes it, without waiting for it to end.
    fn drop(&mut self) {
        self.shared.lock().stop = true;
        self.shared.to_worker.notify_all();
    }
}
