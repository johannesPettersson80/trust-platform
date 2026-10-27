//! The worker thread of a computer's `modbus-tcp` driver: the transport that carries the shared
//! exchange's bytes over a `std::net::TcpStream`, so that connections and round trips happen on the
//! worker and never on the scan (specification 11 §6.5). The worker asks the exchange for its next
//! step with `poll`, connects, sends, reads and closes as the exchange says, and tells it what
//! happened with `connected`, `received` and `failed`; it decides nothing itself. It holds the
//! shared lock only while it calls the exchange, never while it waits for the network.

use std::io::{ErrorKind, Read, Write};
use std::net::TcpStream;
use std::sync::{MutexGuard, PoisonError};
use std::time::Duration;

use trust_runtime_services::io_drivers::modbus::ModbusPoll;

use super::{Shared, State};

/// The bytes of the largest Modbus TCP frame, `trust_modbus::tcp::MAX_ADU_LEN`: the 7 bytes of the
/// MBAP header and a PDU of at most 253 (Modbus Messaging on TCP/IP V1.0b §3.1.3). The exchange
/// writes each request into a buffer of this size, and a read takes at most one frame.
const FRAME_BYTES: usize = 260;

/// The longest that one read waits for a response before the worker looks at the exchange again,
/// so that it sees a stop soon.
const READ_SLICE: Duration = Duration::from_millis(10);

/// The shortest read timeout: a read timeout of zero is refused by the operating system.
const SHORTEST_READ: Duration = Duration::from_millis(1);

/// What the exchange is told when a read gives no bytes: the server closed the connection, in the
/// words of the chips' network.
const CLOSED: &str = "the server closed the connection";

/// Carries the exchange of `shared` until the driver asks the worker to stop: one step of the
/// exchange at a time, with the connection that the exchange asked for, if it is open.
pub(super) fn run(shared: &Shared) {
    let timeout = Duration::from_millis(u64::from(shared.lock().exchange.params().timeout_ms));
    let mut frame = [0_u8; FRAME_BYTES];
    let mut incoming = [0_u8; FRAME_BYTES];
    let mut stream: Option<TcpStream> = None;
    while let Some(step) = next_step(shared, &mut frame, stream.is_some()) {
        match step {
            ModbusPoll::Connect(address) => {
                let opened = TcpStream::connect_timeout(&address.socket, timeout)
                    .and_then(|opened| configure(opened, timeout));
                let mut state = shared.lock();
                match opened {
                    Ok(opened) => {
                        stream = Some(opened);
                        state.exchange.connected();
                    }
                    Err(error) => state.exchange.failed(shared.now_ms(), &error.to_string()),
                }
                shared.to_scan.notify_all();
            }
            ModbusPoll::Send(len) => {
                let sent = match (stream.as_mut(), frame.get(..len)) {
                    (Some(open), Some(request)) => open.write_all(request),
                    _ => Err(ErrorKind::NotConnected.into()),
                };
                if let Err(error) = sent {
                    shared
                        .lock()
                        .exchange
                        .failed(shared.now_ms(), &error.to_string());
                    shared.to_scan.notify_all();
                }
            }
            ModbusPoll::Close => {
                // The exchange gave the connection up, which failed the request on its way, such
                // as a response past `timeout_ms`: a scan that waits for it can take the failure.
                stream = None;
                shared.to_scan.notify_all();
            }
            ModbusPoll::Wait(until) => {
                if let Some(open) = stream.as_mut() {
                    receive(shared, open, until, &mut incoming);
                }
            }
        }
    }
}

/// The exchange's next step for the worker, or `None` when the driver asks the worker to stop.
/// While the exchange has nothing to carry, the worker waits here for the scan or for the time the
/// exchange gives, such as the end of a backoff; with the connection open, a wait with a time is a
/// request on the wire, whose response the worker reads (`open` says whether it is).
fn next_step(shared: &Shared, frame: &mut [u8; FRAME_BYTES], open: bool) -> Option<ModbusPoll> {
    let mut state = shared.lock();
    loop {
        if state.stop {
            return None;
        }
        let now = shared.now_ms();
        match state.exchange.poll(now, frame) {
            ModbusPoll::Wait(None) => state = wait(shared, state, None),
            ModbusPoll::Wait(Some(until)) if !open => {
                let left = Duration::from_millis(until.saturating_sub(now));
                state = wait(shared, state, Some(left));
            }
            step => return Some(step),
        }
    }
}

/// Waits for the scan or for the driver's drop, at most `timeout` when it is given.
fn wait<'a>(
    shared: &'a Shared,
    state: MutexGuard<'a, State>,
    timeout: Option<Duration>,
) -> MutexGuard<'a, State> {
    match timeout {
        Some(timeout) => {
            shared
                .to_worker
                .wait_timeout(state, timeout)
                .unwrap_or_else(PoisonError::into_inner)
                .0
        }
        None => shared
            .to_worker
            .wait(state)
            .unwrap_or_else(PoisonError::into_inner),
    }
}

/// Reads what comes on `open` for the request on the wire, at most one read slice and not past
/// `until`, the time at which the exchange gives the response up, and hands it to the exchange: the
/// bytes that came, or the connection's failure. A read that times out tells the exchange nothing;
/// the next step lets it see its own deadline.
fn receive(
    shared: &Shared,
    open: &mut TcpStream,
    until: Option<u64>,
    incoming: &mut [u8; FRAME_BYTES],
) {
    let left = until.map_or(READ_SLICE, |deadline| {
        Duration::from_millis(deadline.saturating_sub(shared.now_ms()))
    });
    let read = open
        .set_read_timeout(Some(left.clamp(SHORTEST_READ, READ_SLICE)))
        .and_then(|()| open.read(incoming));
    let mut state = shared.lock();
    match read {
        Ok(0) => state.exchange.failed(shared.now_ms(), CLOSED),
        Ok(count) => {
            let bytes = incoming.get(..count).unwrap_or_default();
            state.exchange.received(shared.now_ms(), bytes);
        }
        Err(error)
            if matches!(
                error.kind(),
                ErrorKind::WouldBlock | ErrorKind::TimedOut | ErrorKind::Interrupted
            ) =>
        {
            return;
        }
        Err(error) => state.exchange.failed(shared.now_ms(), &error.to_string()),
    }
    shared.to_scan.notify_all();
}

/// `opened` ready for the exchange: requests sent without delay, as each is a whole frame, and a
/// send that cannot finish within `timeout` failed rather than blocking the worker.
fn configure(opened: TcpStream, timeout: Duration) -> std::io::Result<TcpStream> {
    opened.set_nodelay(true)?;
    opened.set_write_timeout(Some(timeout))?;
    Ok(opened)
}
