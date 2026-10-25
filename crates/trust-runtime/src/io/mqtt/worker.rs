//! The worker thread of a computer's `mqtt` driver: the transport that carries the shared
//! exchange's bytes over a `std::net::TcpStream`, with TLS through `native-tls` when the
//! parameters ask for it, so that connecting, sending and receiving happen on the worker and never
//! on the scan (specification 32 §8; specification 11 §6.5). The worker asks the exchange for its
//! next step with `poll`, opens and closes the connection as the exchange says, sends the
//! exchange's outgoing bytes and reports with `sent` how many the connection took, hands every
//! byte from the broker to `received`, and tells the exchange what became of the connection with
//! `opened`, `open_failed` and `closed`; it decides nothing itself. When the driver is dropped it
//! asks the exchange to end the session in order with `disconnect` and carries that to its end.
//!
//! The worker holds the shared lock only while it calls the exchange, never while it waits for the
//! network: opening a connection blocks the worker alone, at most for the exchange's connect
//! timeout, and an open connection is non-blocking. Between its looks at an open connection the
//! worker waits on its condition variable, at most [`IDLE`]; without a connection it waits until
//! the exchange's next timer (`due_ms`), such as the end of a backoff. The scan wakes it with each
//! handoff, so that a handoff goes out at once.

use std::io::{self, ErrorKind, Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::sync::{MutexGuard, PoisonError};
use std::time::{Duration, Instant};

use native_tls::{HandshakeError, TlsConnector, TlsStream};
use trust_runtime_services::io_drivers::mqtt::{BrokerEndpoint, MqttPoll, CONNECT_TIMEOUT_MS};

use super::{Shared, State};

/// The longest the worker waits between two looks at an open connection: a message from the
/// broker waits at most this long for the worker before the scan can read it.
const IDLE: Duration = Duration::from_millis(1);

/// The shortest wait for a timer of the exchange, so that a timer due now is not waited for in a
/// loop that does not sleep.
const MINIMUM_WAIT: Duration = Duration::from_millis(1);

/// The bytes that one read of the connection takes, the largest plaintext of a TLS record.
const READ_BYTES: usize = 16 * 1024;

/// The most bytes that one look at the connection takes from it before it hands them to the
/// exchange, at once, so that the messages that arrived together reach the scan together.
const RECEIVE_LIMIT: usize = 64 * 1024;

/// What the exchange is told when the broker closes the connection.
const CLOSED: &str = "the broker closed the connection";

/// What the exchange is told when the connection takes no byte of a write.
const WRITE_ZERO: &str = "the connection took no bytes";

/// The worker's next step, which the exchange's `poll` decides.
enum Step {
    /// Open a connection to the broker.
    Open(BrokerEndpoint),
    /// Send these bytes, the exchange's outgoing bytes, and take what came, on the open connection.
    Carry(Vec<u8>),
    /// Close the connection.
    Close,
    /// End: the driver was dropped and the session is over.
    Exit,
}

/// A connection to the broker.
enum Connection {
    /// A plain TCP connection.
    Plain(TcpStream),
    /// A TLS connection over TCP.
    Tls(Box<TlsStream<TcpStream>>),
}

impl Connection {
    /// The TCP connection under it.
    fn socket(&self) -> &TcpStream {
        match self {
            Self::Plain(stream) => stream,
            Self::Tls(stream) => stream.get_ref(),
        }
    }

    /// Closes the connection, with TLS's close notification first; the broker learns of the end
    /// from the closed connection either way.
    fn close(self) {
        if let Self::Tls(mut stream) = self {
            let _ = stream.shutdown();
        }
    }
}

impl Read for Connection {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        match self {
            Self::Plain(stream) => stream.read(buffer),
            Self::Tls(stream) => stream.read(buffer),
        }
    }
}

impl Write for Connection {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        match self {
            Self::Plain(stream) => stream.write(bytes),
            Self::Tls(stream) => stream.write(bytes),
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        match self {
            Self::Plain(stream) => stream.flush(),
            Self::Tls(stream) => stream.flush(),
        }
    }
}

/// Carries the exchange of `shared` with the TLS client `tls` until the driver is dropped and the
/// session is over: one step of the exchange at a time, on the connection that the exchange asked
/// for, if it is open.
pub(super) fn run(shared: &Shared, tls: Option<&TlsConnector>) {
    let mut connection: Option<Connection> = None;
    let mut stopping = false;
    let mut buffer = vec![0_u8; READ_BYTES];
    loop {
        match next_step(shared, &mut stopping, connection.is_some()) {
            Step::Open(broker) => {
                if let Some(earlier) = connection.take() {
                    earlier.close();
                }
                let opened = open(&broker, tls);
                let mut state = shared.lock();
                let now = shared.now(&mut state);
                match opened {
                    Ok(opened) => {
                        connection = Some(opened);
                        state.exchange.opened(now);
                    }
                    Err(detail) => state.exchange.open_failed(now, &detail),
                }
                drop(state);
                shared.to_scan.notify_all();
            }
            Step::Carry(outgoing) => {
                let carried = connection
                    .as_mut()
                    .is_some_and(|open| carry(shared, open, &outgoing, &mut buffer));
                if !carried {
                    connection = None;
                }
            }
            Step::Close => {
                if let Some(closing) = connection.take() {
                    closing.close();
                }
                shared.to_scan.notify_all();
                if stopping {
                    return;
                }
            }
            Step::Exit => return,
        }
    }
}

/// The exchange's next step for the worker. A dropped driver makes the exchange end the session
/// first (`stopping` then holds). While the exchange has nothing to carry and no connection is
/// open, the worker waits here for the scan, for the drop or for the exchange's next timer, such as
/// the end of a backoff; `open` says whether a connection is open.
fn next_step(shared: &Shared, stopping: &mut bool, open: bool) -> Step {
    let mut state = shared.lock();
    loop {
        let now = shared.now(&mut state);
        if state.stop && !*stopping {
            *stopping = true;
            state.exchange.disconnect(now);
        }
        match state.exchange.poll(now) {
            MqttPoll::Open => return Step::Open(state.exchange.params().broker.clone()),
            MqttPoll::Close => return Step::Close,
            MqttPoll::Wait if open => return Step::Carry(state.exchange.outgoing().to_vec()),
            MqttPoll::Wait if *stopping => return Step::Exit,
            MqttPoll::Wait => {
                let due = state.exchange.due_ms().map(|due_ms| {
                    Duration::from_millis(due_ms.saturating_sub(now)).max(MINIMUM_WAIT)
                });
                state = wait(shared, state, due);
            }
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

/// Sends what `connection` takes of `outgoing` now, takes what came from the broker, and tells the
/// exchange both, the bytes sent first; false when the connection failed or the broker closed it,
/// which the exchange is told after the bytes that came before. When nothing moved the worker waits
/// for the scan, at most [`IDLE`], before it looks again.
fn carry(shared: &Shared, connection: &mut Connection, outgoing: &[u8], buffer: &mut [u8]) -> bool {
    let (sent, write_failure) = send(connection, outgoing);
    let (received, read_failure) = receive(connection, buffer);
    let failure = write_failure.or(read_failure);
    let moved = sent > 0 || !received.is_empty() || failure.is_some();
    let mut state = shared.lock();
    let now = shared.now(&mut state);
    if sent > 0 {
        state.exchange.sent(now, sent);
    }
    if !received.is_empty() {
        state.exchange.received(now, &received);
    }
    if let Some(detail) = &failure {
        state.exchange.closed(now, detail);
    }
    if moved {
        drop(state);
        shared.to_scan.notify_all();
    } else {
        drop(wait(shared, state, Some(IDLE)));
    }
    failure.is_none()
}

/// Writes `outgoing` to `connection` until it takes no more without waiting; the bytes it took,
/// and the failure of the connection, if it failed.
///
/// A TLS write that cannot go on keeps the record it encrypted and completes it with the next
/// write of the same bytes, which the exchange gives again, since bytes not reported sent stay at
/// the front of its outgoing bytes. So the worker never flushes a TLS connection between two
/// writes: a flush that sent such a record would make the next write encrypt its bytes twice.
fn send(connection: &mut Connection, outgoing: &[u8]) -> (usize, Option<String>) {
    let mut sent = 0;
    while let Some(rest) = outgoing.get(sent..).filter(|rest| !rest.is_empty()) {
        match connection.write(rest) {
            Ok(0) => return (sent, Some(String::from(WRITE_ZERO))),
            Ok(count) => sent += count,
            Err(error) if waits(&error) => break,
            Err(error) if error.kind() == ErrorKind::Interrupted => {}
            Err(error) => return (sent, Some(error.to_string())),
        }
    }
    (sent, None)
}

/// Reads from `connection` what came, until nothing more waits or [`RECEIVE_LIMIT`] bytes came;
/// the bytes, and [`CLOSED`] or the failure of the connection, if the broker closed it or it
/// failed.
fn receive(connection: &mut Connection, buffer: &mut [u8]) -> (Vec<u8>, Option<String>) {
    let mut received = Vec::new();
    while received.len() < RECEIVE_LIMIT {
        match connection.read(buffer) {
            Ok(0) => return (received, Some(String::from(CLOSED))),
            Ok(count) => received.extend_from_slice(buffer.get(..count).unwrap_or_default()),
            Err(error) if waits(&error) => break,
            Err(error) if error.kind() == ErrorKind::Interrupted => {}
            Err(error) => return (received, Some(error.to_string())),
        }
    }
    (received, None)
}

/// Whether `error` says only that the connection cannot go on without waiting.
fn waits(error: &io::Error) -> bool {
    matches!(error.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut)
}

/// A connection to `broker`, with TLS through `tls` when it is given, made non-blocking; or why
/// there is none. The host is resolved, each of its addresses is tried in turn, and the TLS
/// handshake verifies the broker's host, all within the exchange's connect timeout
/// [`CONNECT_TIMEOUT_MS`].
fn open(broker: &BrokerEndpoint, tls: Option<&TlsConnector>) -> Result<Connection, String> {
    let deadline = Instant::now() + Duration::from_millis(CONNECT_TIMEOUT_MS);
    let stream = connect(broker, deadline)?;
    stream
        .set_nodelay(true)
        .map_err(|error| error.to_string())?;
    let connection = match tls {
        None => Connection::Plain(stream),
        Some(connector) => Connection::Tls(Box::new(handshake(
            connector,
            &broker.host,
            stream,
            deadline,
        )?)),
    };
    connection
        .socket()
        .set_nonblocking(true)
        .map_err(|error| error.to_string())?;
    Ok(connection)
}

/// A TCP connection to the first address of `broker` that takes one before `deadline`.
fn connect(broker: &BrokerEndpoint, deadline: Instant) -> Result<TcpStream, String> {
    let addresses = (broker.host.as_str(), broker.port)
        .to_socket_addrs()
        .map_err(|error| format!("cannot resolve {}: {error}", broker.host))?;
    let mut failure = format!("{} has no address", broker.host);
    for address in addresses {
        let Some(left) = time_left(deadline) else {
            break;
        };
        match TcpStream::connect_timeout(&address, left) {
            Ok(stream) => return Ok(stream),
            Err(error) => failure = error.to_string(),
        }
    }
    Err(failure)
}

/// The TLS handshake over `stream` with `connector`, which verifies the broker's certificate for
/// `host`, done before `deadline`.
fn handshake(
    connector: &TlsConnector,
    host: &str,
    stream: TcpStream,
    deadline: Instant,
) -> Result<TlsStream<TcpStream>, String> {
    let timed_out = || format!("TLS handshake took more than {CONNECT_TIMEOUT_MS} ms");
    let left = time_left(deadline).ok_or_else(timed_out)?;
    limit(&stream, left)?;
    let mut attempt = connector.connect(host, stream);
    loop {
        match attempt {
            Ok(tls) => return Ok(tls),
            Err(HandshakeError::Failure(error)) => {
                return Err(format!("TLS handshake failed: {error}"));
            }
            // A read or a write of the handshake reached its time limit.
            Err(HandshakeError::WouldBlock(pending)) => {
                let left = time_left(deadline).ok_or_else(timed_out)?;
                limit(pending.get_ref(), left)?;
                attempt = pending.handshake();
            }
        }
    }
}

/// Limits each blocking read and write of `stream` to `left`.
fn limit(stream: &TcpStream, left: Duration) -> Result<(), String> {
    stream
        .set_read_timeout(Some(left))
        .and_then(|()| stream.set_write_timeout(Some(left)))
        .map_err(|error| error.to_string())
}

/// The time from now to `deadline`; `None` once it has passed.
fn time_left(deadline: Instant) -> Option<Duration> {
    deadline
        .checked_duration_since(Instant::now())
        .filter(|left| !left.is_zero())
}
