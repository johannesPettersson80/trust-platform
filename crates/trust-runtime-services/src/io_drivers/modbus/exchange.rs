//! The exchange of one `modbus-tcp` driver (specification 11 §6.5): the requests that the scan side
//! asks for, sent one at a time over one connection, the checks of each response, the accepted
//! input snapshot, the level/latest output handoff, and the deadline, `on_error` and health of each
//! scan boundary. It holds no socket and reads no clock: its transport gives it the bytes that
//! come and the time.

use alloc::format;
use alloc::vec::Vec;

use smol_str::SmolStr;
use trust_modbus::tcp::{MbapHeader, HEADER_LEN, MAX_ADU_LEN};
use trust_runtime_core::error::RuntimeError;

use super::frame::Assembler;
use super::params::{ModbusTcpAddress, ModbusTcpParams};
use super::requests::{
    apply, encode, input_plan, output_data, read_request, take_read, take_write, unplanned,
    write_request, InputData, InputRun, OutputRun, Step,
};
use super::{Failure, FailureKind};
use crate::io_drivers::OnError;
use crate::status::IoDriverHealth;

/// The fixed scan-side deadline of an input refresh and of an output handoff, in milliseconds: the
/// computer's driver waits this long for its worker (specification 11, "Modbus default-fault and
/// deadline integrity").
pub const SCAN_DEADLINE_MS: u64 = 40;

/// How long the exchange waits after a transport failure before it connects again, in
/// milliseconds: not zero and bounded, so that an unavailable server is never asked in a loop
/// (specification 11 §6.5).
pub const RECONNECT_BACKOFF_MS: u64 = 100;

/// The health and the error of an input refresh that is not complete at its deadline.
const INPUT_PENDING: &str = "modbus input refresh pending";

/// The health and the error of an output handoff that is not complete at its deadline.
const OUTPUT_PENDING: &str = "modbus output handoff pending";

/// The health of each path before its first result.
const INITIALIZING: &str = "modbus worker initializing";

/// What the transport does next, as [`ModbusExchange::poll`] gives it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModbusPoll {
    /// Open a TCP connection to this address, then report [`ModbusExchange::connected`] or
    /// [`ModbusExchange::failed`]. The exchange gives the attempt up after `timeout_ms`.
    Connect(ModbusTcpAddress),
    /// Send this many bytes from the start of the frame buffer on the open connection, and hand
    /// the bytes that come back to [`ModbusExchange::received`]; a send that fails is
    /// [`ModbusExchange::failed`].
    Send(usize),
    /// Close the connection: the exchange gave it up. The exchange connects again, when there is
    /// work, after [`RECONNECT_BACKOFF_MS`].
    Close,
    /// Nothing to send: hand on what comes on an open connection, and poll again at the next scan
    /// call or at the time given, when there is one: a timeout or the end of a backoff.
    Wait(Option<u64>),
}

/// The connection, as the exchange sees it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Link {
    /// No connection; the next one not before `retry_ms`.
    Down {
        /// When the exchange may connect again.
        retry_ms: u64,
    },
    /// The transport opens a connection.
    Connecting {
        /// When the exchange asked for it.
        since_ms: u64,
    },
    /// The connection is open.
    Up,
    /// The exchange gave the connection up: the transport is to close it, and it is down until
    /// `retry_ms`.
    Closing {
        /// When the exchange may connect again.
        retry_ms: u64,
    },
}

/// The request on the wire.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct InFlight {
    /// Its transaction identifier.
    transaction_id: u16,
    /// When its response is late: the time it was sent plus `timeout_ms`.
    deadline_ms: u64,
}

/// A side of the exchange: the input refreshes or the output handoffs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Side {
    /// The input refreshes.
    Inputs,
    /// The output handoffs.
    Outputs,
}

/// The input side: the scan's latest request, the refresh carried out, and the accepted snapshot.
#[derive(Debug, Clone, Default)]
struct Inputs {
    /// The number of the latest request; 0 before the first.
    latest: u64,
    /// The latest request's refresh while another is carried out.
    queued: Option<InputRun>,
    /// The refresh carried out.
    run: Option<InputRun>,
    /// The number of the request that the latest completed refresh served.
    done: u64,
    /// Why the latest completed refresh failed; `None` when it succeeded.
    failure: Option<Failure>,
    /// The accepted snapshot: the data of the latest refresh that succeeded.
    snapshot: Option<InputData>,
}

/// The output side: the scan's latest handoff and the handoff on its way.
#[derive(Debug, Clone, Default)]
struct Outputs {
    /// The number of the latest handoff; 0 before the first.
    latest: u64,
    /// The latest handoff while another is on its way.
    queued: Option<OutputRun>,
    /// The handoff on its way.
    run: Option<OutputRun>,
    /// The number of the latest completed handoff.
    done: u64,
    /// Why the latest completed handoff failed; `None` when it succeeded.
    failure: Option<Failure>,
}

/// The exchange of one `modbus-tcp` driver with its server and with the process images
/// (specification 11 §6.5). See the module for the order of the calls.
///
/// One request is on the wire at a time. An input refresh reads the whole raw image, or each point
/// in `input_points` order, and becomes the accepted snapshot only when every request of it
/// succeeded. An output handoff writes the whole raw image, or each point in `output_points` order;
/// the scan's latest handoff replaces one that has not started, and a handoff that has started
/// completes. Input refreshes and output handoffs take turns when both wait.
#[derive(Debug, Clone)]
pub struct ModbusExchange {
    /// The driver's parameters.
    params: ModbusTcpParams,
    /// The connection.
    link: Link,
    /// The request on the wire.
    in_flight: Option<InFlight>,
    /// The transaction identifier of the next request: from 1, then one more each request, as the
    /// computer's driver counts.
    transaction_id: u16,
    /// The frame being received.
    incoming: Assembler,
    /// The input side.
    inputs: Inputs,
    /// The output side.
    outputs: Outputs,
    /// The side whose run started last, so that the other goes first when both wait.
    last_run: Option<Side>,
    /// The input path's health.
    input_health: IoDriverHealth,
    /// The output path's health.
    output_health: IoDriverHealth,
    /// The path whose health changed last, which [`ModbusExchange::health`] reports on a tie.
    last_health: Side,
}

impl ModbusExchange {
    /// The exchange of `params`, started at `now_ms`: no connection yet, so the first request
    /// connects at once; transaction identifiers from 1; each path's health degraded with `modbus
    /// worker initializing` until its first result.
    #[must_use]
    pub fn new(params: ModbusTcpParams, now_ms: u64) -> Self {
        let initializing = IoDriverHealth::Degraded {
            error: SmolStr::new(INITIALIZING),
        };
        Self {
            params,
            link: Link::Down { retry_ms: now_ms },
            in_flight: None,
            transaction_id: 1,
            incoming: Assembler::new(),
            inputs: Inputs::default(),
            outputs: Outputs::default(),
            last_run: None,
            input_health: initializing.clone(),
            output_health: initializing,
            last_health: Side::Inputs,
        }
    }

    /// The driver's parameters.
    #[must_use]
    pub fn params(&self) -> &ModbusTcpParams {
        &self.params
    }

    /// Asks at `now_ms`, a cycle start, for a fresh snapshot of an input image of `input_len`
    /// bytes, and gives the deadline: the time until which the caller may drive the transport, or
    /// wait for the thread that drives it, before [`ModbusExchange::read_inputs`]. The request
    /// replaces an earlier one whose refresh has not started; a refresh that has started completes
    /// first.
    ///
    /// The deadline is `now_ms` itself when the request is settled at once (an empty raw image
    /// exchanges nothing; a raw range past address 65535 or a point outside the image fails), and
    /// when the backoff after a transport failure lasts past the usual deadline,
    /// `now_ms + SCAN_DEADLINE_MS`.
    #[must_use]
    pub fn request_inputs(&mut self, now_ms: u64, input_len: usize) -> u64 {
        self.expire(now_ms);
        self.inputs.latest += 1;
        let seq = self.inputs.latest;
        let plan = match input_plan(&self.params, input_len) {
            Ok(plan) if plan.requests() > 0 => plan,
            outcome => {
                self.inputs.queued = None;
                if !self.input_run_started() {
                    self.inputs.run = None;
                }
                self.complete_inputs(seq, outcome.map(|_| InputData::Raw(Vec::new())));
                return now_ms;
            }
        };
        let run = InputRun::new(seq, plan, input_len);
        if self.inputs.run.is_some() && !self.input_run_started() {
            self.inputs.run = Some(run);
        } else {
            self.inputs.queued = Some(run);
        }
        self.deadline(now_ms)
    }

    /// Whether the input refresh of the latest request completed, with data or with a failure.
    #[must_use]
    pub fn inputs_settled(&self) -> bool {
        self.inputs.latest > 0 && self.inputs.done >= self.inputs.latest
    }

    /// The input handoff at `now_ms`, the deadline or a time after the refresh settled, into
    /// `image` (specification 11, "Modbus default-fault and deadline integrity"):
    ///
    /// - a refresh that completed with data copies it into `image`: the raw bytes, or each point's
    ///   bytes or bit, after every point was checked to lie in the image; the input health is ok;
    /// - a refresh that completed with a failure, or that is not complete (`modbus input refresh
    ///   pending`), makes the input health faulted under `fault` and degraded under `warn` and
    ///   `ignore`, with the failure's text. Under `fault` no byte of `image` changes, even when an
    ///   older snapshot is accepted; under `warn` and `ignore` the accepted snapshot, if there is
    ///   one, is copied, and without one no byte changes.
    ///
    /// # Errors
    ///
    /// Under `on_error = "fault"` only: the failure, typed as it happened. A connection, a timeout,
    /// a response that fails a check and a pending refresh are `IoTransport`, a Modbus exception is
    /// `IoAddress`, and a value that does not convert or a point outside the image is `IoDriver`.
    pub fn read_inputs(&mut self, now_ms: u64, image: &mut [u8]) -> Result<(), RuntimeError> {
        self.expire(now_ms);
        let failure = match (self.inputs_settled(), self.inputs.failure.clone()) {
            (false, _) => Failure::transport(INPUT_PENDING),
            (true, Some(failure)) => failure,
            (true, None) => {
                let applied = self.inputs.snapshot.as_ref().map_or(Ok(()), |snapshot| {
                    apply(snapshot, &self.params.input_points, image)
                });
                match applied {
                    Ok(()) => {
                        self.set_input_health(IoDriverHealth::Ok);
                        return Ok(());
                    }
                    Err(failure) => failure,
                }
            }
        };
        self.set_input_health(policy_health(self.params.on_error, &failure.text));
        if self.params.on_error == OnError::Fault {
            return Err(failure.error());
        }
        if let Some(snapshot) = &self.inputs.snapshot {
            // Atomic: a snapshot that does not fit the image changes no byte of it.
            let _ = apply(snapshot, &self.params.input_points, image);
        }
        Ok(())
    }

    /// Hands the output image `image` off at `now_ms`, after the tasks ran, and gives the deadline
    /// until which the caller may drive the transport, or wait, before
    /// [`ModbusExchange::output_result`]. The handoff is level/latest: it replaces an earlier one
    /// that has not started, and waits for one that has.
    ///
    /// Every point is converted first: a point that does not convert settles the handoff at once
    /// with its failure, and nothing of it reaches the wire. The deadline is `now_ms` itself when
    /// the handoff is settled at once, when an earlier handoff is still on its way or waiting (the
    /// scan does not wait behind the server), and when the backoff after a transport failure lasts
    /// past the usual deadline, `now_ms + SCAN_DEADLINE_MS`.
    #[must_use]
    pub fn write_outputs(&mut self, now_ms: u64, image: &[u8]) -> u64 {
        self.expire(now_ms);
        self.outputs.latest += 1;
        let seq = self.outputs.latest;
        let data = match output_data(&self.params, image) {
            Ok(Some(data)) => data,
            Ok(None) => {
                self.complete_outputs(seq, Ok(()));
                return now_ms;
            }
            Err(failure) => {
                self.complete_outputs(seq, Err(failure));
                return now_ms;
            }
        };
        let busy = self.outputs.run.is_some() || self.outputs.queued.is_some();
        let run = OutputRun { seq, data, next: 0 };
        if self.outputs.run.is_some() && !self.output_run_started() {
            self.outputs.run = Some(run);
        } else {
            self.outputs.queued = Some(run);
        }
        if busy {
            now_ms
        } else {
            self.deadline(now_ms)
        }
    }

    /// Whether the latest output handoff completed, written or failed.
    #[must_use]
    pub fn outputs_settled(&self) -> bool {
        self.outputs.latest > 0 && self.outputs.done >= self.outputs.latest
    }

    /// The result of the latest output handoff at `now_ms`, the deadline or a time after it
    /// settled (specification 11, "Modbus default-fault and deadline integrity"): ok and an ok
    /// output health when it was written; when it failed, or is not complete (`modbus output
    /// handoff pending`), a faulted output health under `fault` and a degraded one under `warn`
    /// and `ignore`, with the failure's text. A pending handoff is not cancelled: it is written
    /// when the server answers, and its result then sets the output health.
    ///
    /// # Errors
    ///
    /// Under `on_error = "fault"` only: the failure, typed as for
    /// [`ModbusExchange::read_inputs`].
    pub fn output_result(&mut self, now_ms: u64) -> Result<(), RuntimeError> {
        self.expire(now_ms);
        let failure = match (self.outputs_settled(), self.outputs.failure.clone()) {
            (false, _) => Failure::transport(OUTPUT_PENDING),
            (true, Some(failure)) => failure,
            (true, None) => {
                self.set_output_health(IoDriverHealth::Ok);
                return Ok(());
            }
        };
        self.set_output_health(policy_health(self.params.on_error, &failure.text));
        if self.params.on_error == OnError::Fault {
            Err(failure.error())
        } else {
            Ok(())
        }
    }

    /// The driver's health: the more severe of the input and the output path's, the path that
    /// changed last on a tie.
    #[must_use]
    pub fn health(&self) -> IoDriverHealth {
        let input = severity(&self.input_health);
        let output = severity(&self.output_health);
        if input > output || (input == output && self.last_health == Side::Inputs) {
            self.input_health.clone()
        } else {
            self.output_health.clone()
        }
    }

    /// The output path's health, on which a safe-state handoff's confirmation depends
    /// (specification 11, "Safe-state handoff confirmation"): ok only when the latest output
    /// handoff was written. A failed or pending input refresh does not change it.
    #[must_use]
    pub fn output_health(&self) -> IoDriverHealth {
        self.output_health.clone()
    }

    /// What the transport does next at `now_ms`. A request is written into `frame`, the transport's
    /// send buffer, for [`ModbusPoll::Send`]. A connection or a response that takes longer than
    /// `timeout_ms` fails here, and the exchange asks for [`ModbusPoll::Close`].
    #[must_use]
    pub fn poll(&mut self, now_ms: u64, frame: &mut [u8; MAX_ADU_LEN]) -> ModbusPoll {
        self.expire(now_ms);
        match self.link {
            Link::Closing { retry_ms } => {
                self.link = Link::Down { retry_ms };
                return ModbusPoll::Close;
            }
            Link::Connecting { since_ms } => {
                return ModbusPoll::Wait(Some(since_ms.saturating_add(self.timeout_ms())));
            }
            Link::Down { .. } | Link::Up => {}
        }
        if let Some(in_flight) = self.in_flight {
            return ModbusPoll::Wait(Some(in_flight.deadline_ms));
        }
        if self.inputs.run.is_none() && self.outputs.run.is_none() {
            self.start_run();
            if self.inputs.run.is_none() && self.outputs.run.is_none() {
                return ModbusPoll::Wait(None);
            }
        }
        match self.link {
            Link::Down { retry_ms } if now_ms < retry_ms => ModbusPoll::Wait(Some(retry_ms)),
            Link::Down { .. } => {
                self.link = Link::Connecting { since_ms: now_ms };
                ModbusPoll::Connect(self.params.address)
            }
            Link::Up => self.send_next(now_ms, frame),
            Link::Connecting { .. } | Link::Closing { .. } => ModbusPoll::Wait(None),
        }
    }

    /// The connection that [`ModbusPoll::Connect`] asked for is open.
    pub fn connected(&mut self) {
        if matches!(self.link, Link::Connecting { .. }) {
            self.link = Link::Up;
        }
    }

    /// Takes `bytes`, which came on the open connection at `now_ms`, in pieces of any size. A
    /// complete frame of another transaction than the request's is discarded, and reading goes on
    /// (V1.0b §4.4.1.3); the request's own response is checked before any of its data is taken.
    pub fn received(&mut self, now_ms: u64, bytes: &[u8]) {
        let mut rest = bytes;
        while !rest.is_empty() && self.link == Link::Up {
            let (taken, frame) = self.incoming.push(rest);
            rest = rest.get(taken..).unwrap_or_default();
            match frame {
                Ok(Some(header)) => {
                    let buffer = self.incoming.take();
                    let len = HEADER_LEN + header.pdu_len();
                    self.take_frame(now_ms, header, buffer.get(..len).unwrap_or_default());
                }
                Ok(None) if taken == 0 => break,
                Ok(None) => {}
                Err(error) => {
                    let text = format!(
                        "modbus tcp {}: refused response: {error}",
                        self.params.address
                    );
                    self.lose_link(now_ms, Failure::transport(text));
                }
            }
        }
    }

    /// The connection could not be opened, a send failed, a receive failed or the server closed
    /// the connection, at `now_ms`, for `reason`, such as the operating system's text. The request
    /// on its way fails with `modbus tcp connect <address>: <reason>` while connecting, and with
    /// `modbus tcp <address>: <reason>` after; the exchange connects again after
    /// [`RECONNECT_BACKOFF_MS`].
    pub fn failed(&mut self, now_ms: u64, reason: &str) {
        let address = self.params.address;
        let text = match self.link {
            Link::Connecting { .. } => format!("modbus tcp connect {address}: {reason}"),
            Link::Up => format!("modbus tcp {address}: {reason}"),
            Link::Down { .. } | Link::Closing { .. } => return,
        };
        self.lose_link(now_ms, Failure::transport(text));
    }

    /// `timeout_ms` in milliseconds of the exchange's clock.
    fn timeout_ms(&self) -> u64 {
        u64::from(self.params.timeout_ms)
    }

    /// The deadline of a scan call at `now_ms`: [`SCAN_DEADLINE_MS`] later, or `now_ms` when the
    /// backoff after a transport failure lasts that long, so that the scan does not wait for
    /// nothing.
    fn deadline(&self, now_ms: u64) -> u64 {
        let deadline = now_ms.saturating_add(SCAN_DEADLINE_MS);
        match self.link {
            Link::Down { retry_ms } | Link::Closing { retry_ms } if retry_ms >= deadline => now_ms,
            _ => deadline,
        }
    }

    /// Fails the connection that takes longer than `timeout_ms` to open, or the request whose
    /// response takes longer, at `now_ms`.
    fn expire(&mut self, now_ms: u64) {
        let address = self.params.address;
        let timeout_ms = self.params.timeout_ms;
        let text = match (self.link, self.in_flight) {
            (Link::Connecting { since_ms }, _)
                if now_ms >= since_ms.saturating_add(self.timeout_ms()) =>
            {
                format!("modbus tcp connect {address}: no connection within {timeout_ms} ms")
            }
            (Link::Up, Some(in_flight)) if now_ms >= in_flight.deadline_ms => {
                format!("modbus tcp {address}: no response within {timeout_ms} ms")
            }
            _ => return,
        };
        self.lose_link(now_ms, Failure::transport(text));
    }

    /// Gives the connection up at `now_ms` for `failure`, which the run on its way fails with:
    /// the transport is to close it, and the exchange connects again after
    /// [`RECONNECT_BACKOFF_MS`].
    fn lose_link(&mut self, now_ms: u64, failure: Failure) {
        self.in_flight = None;
        self.incoming.clear();
        self.link = Link::Closing {
            retry_ms: now_ms.saturating_add(RECONNECT_BACKOFF_MS),
        };
        self.finish_run(Err(failure));
    }

    /// Whether the input refresh carried out has sent a request.
    fn input_run_started(&self) -> bool {
        self.inputs
            .run
            .as_ref()
            .is_some_and(|run| run.next > 0 || self.in_flight.is_some())
    }

    /// Whether the output handoff on its way has sent a request.
    fn output_run_started(&self) -> bool {
        self.outputs
            .run
            .as_ref()
            .is_some_and(|run| run.next > 0 || self.in_flight.is_some())
    }

    /// Starts the waiting refresh or handoff, the side that did not start last when both wait.
    fn start_run(&mut self) {
        let side = match (self.inputs.queued.is_some(), self.outputs.queued.is_some()) {
            (true, true) if self.last_run == Some(Side::Outputs) => Side::Inputs,
            (_, true) => Side::Outputs,
            (true, false) => Side::Inputs,
            (false, false) => return,
        };
        self.last_run = Some(side);
        match side {
            Side::Inputs => self.inputs.run = self.inputs.queued.take(),
            Side::Outputs => self.outputs.run = self.outputs.queued.take(),
        }
    }

    /// Writes the next request of the run into `frame` and gives [`ModbusPoll::Send`]; a request
    /// that cannot be written fails its run.
    fn send_next(&mut self, now_ms: u64, frame: &mut [u8; MAX_ADU_LEN]) -> ModbusPoll {
        let transaction_id = self.transaction_id;
        let unit_id = self.params.unit_id;
        let encoded = if let Some(run) = &self.inputs.run {
            read_request(&self.params, run.plan, run.next)
                .ok_or_else(unplanned)
                .and_then(|(request, _)| encode(transaction_id, unit_id, &request, frame))
        } else if let Some(run) = &self.outputs.run {
            write_request(&self.params, &run.data, run.next)
                .and_then(|(request, _)| encode(transaction_id, unit_id, &request, frame))
        } else {
            return ModbusPoll::Wait(None);
        };
        match encoded {
            Ok(len) => {
                self.transaction_id = transaction_id.wrapping_add(1);
                self.in_flight = Some(InFlight {
                    transaction_id,
                    deadline_ms: now_ms.saturating_add(self.timeout_ms()),
                });
                ModbusPoll::Send(len)
            }
            Err(failure) => {
                self.finish_run(Err(failure));
                ModbusPoll::Wait(Some(now_ms))
            }
        }
    }

    /// Takes a complete frame with `header` at `now_ms`: the response to the request on the wire,
    /// or a frame of another transaction, which is discarded.
    fn take_frame(&mut self, now_ms: u64, header: MbapHeader, frame: &[u8]) {
        let Some(in_flight) = self.in_flight else {
            return;
        };
        if header.transaction_id != in_flight.transaction_id {
            return;
        }
        self.in_flight = None;
        let step = if let Some(run) = self.inputs.run.as_mut() {
            take_read(&self.params, run, in_flight.transaction_id, frame)
        } else if let Some(run) = self.outputs.run.as_mut() {
            take_write(&self.params, run, in_flight.transaction_id, frame)
        } else {
            return;
        };
        match step {
            Step::More => {}
            Step::Done => self.finish_run(Ok(())),
            Step::Failed(failure) if failure.kind == FailureKind::Transport => {
                self.lose_link(now_ms, failure);
            }
            Step::Failed(failure) => self.finish_run(Err(failure)),
        }
    }

    /// Ends the run on its way with `outcome`.
    fn finish_run(&mut self, outcome: Result<(), Failure>) {
        if let Some(run) = self.inputs.run.take() {
            self.complete_inputs(run.seq, outcome.map(|()| run.data));
        } else if let Some(run) = self.outputs.run.take() {
            self.complete_outputs(run.seq, outcome);
        }
    }

    /// Records the outcome of the refresh of request `seq`: its data becomes the accepted snapshot,
    /// or its failure the input health under `on_error`. An outcome older than the latest recorded
    /// one changes nothing.
    fn complete_inputs(&mut self, seq: u64, outcome: Result<InputData, Failure>) {
        if seq < self.inputs.done {
            return;
        }
        self.inputs.done = seq;
        match outcome {
            Ok(data) => {
                self.inputs.snapshot = Some(data);
                self.inputs.failure = None;
                self.set_input_health(IoDriverHealth::Ok);
            }
            Err(failure) => {
                self.set_input_health(policy_health(self.params.on_error, &failure.text));
                self.inputs.failure = Some(failure);
            }
        }
    }

    /// Records the outcome of handoff `seq`, which sets the output health under `on_error`. An
    /// outcome older than the latest recorded one changes nothing.
    fn complete_outputs(&mut self, seq: u64, outcome: Result<(), Failure>) {
        if seq < self.outputs.done {
            return;
        }
        self.outputs.done = seq;
        let health = match &outcome {
            Ok(()) => IoDriverHealth::Ok,
            Err(failure) => policy_health(self.params.on_error, &failure.text),
        };
        self.set_output_health(health);
        self.outputs.failure = outcome.err();
    }

    /// Sets the input path's health.
    fn set_input_health(&mut self, health: IoDriverHealth) {
        self.input_health = health;
        self.last_health = Side::Inputs;
    }

    /// Sets the output path's health.
    fn set_output_health(&mut self, health: IoDriverHealth) {
        self.output_health = health;
        self.last_health = Side::Outputs;
    }
}

/// The health of a path with the failure `text` under `on_error`: faulted under `fault`, degraded
/// under `warn` and `ignore`.
fn policy_health(on_error: OnError, text: &str) -> IoDriverHealth {
    let error = SmolStr::new(text);
    match on_error {
        OnError::Fault => IoDriverHealth::Faulted { error },
        OnError::Warn | OnError::Ignore => IoDriverHealth::Degraded { error },
    }
}

/// How severe a health is: ok, degraded, faulted.
fn severity(health: &IoDriverHealth) -> u8 {
    match health {
        IoDriverHealth::Ok => 0,
        IoDriverHealth::Degraded { .. } => 1,
        IoDriverHealth::Faulted { .. } => 2,
    }
}
