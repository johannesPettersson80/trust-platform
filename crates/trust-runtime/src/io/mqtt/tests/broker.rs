//! A broker stand-in for the driver's tests: a TCP server on the loopback, with TLS when a test
//! asks for it, that speaks the MQTT 3.1.1 packets of a client's session (OASIS MQTT Version
//! 3.1.1, 29 October 2014). It answers CONNECT with CONNACK (§3.2), SUBSCRIBE with SUBACK (§3.9)
//! and PINGREQ with PINGRESP (§3.13), records what the client sends, and delivers the messages that
//! a test gives it as PUBLISH packets at quality of service 0 (§3.3). Its packets are written here
//! from the standard, not with the shared codec, so that a fault of the codec cannot hide in both
//! ends of a test.
//!
//! One connection is current at a time: a new connection replaces the one before, as a broker
//! replaces the session of a client that connects again (MQTT-3.1.4-2). The messages of one
//! [`Broker::deliver`] go out in one write, so they reach the driver together, as one batch.

use std::collections::VecDeque;
use std::io::{ErrorKind, Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};
use std::thread;
use std::time::{Duration, Instant};

use native_tls::{Identity, TlsAcceptor};

/// How long a test waits for the driver and the broker before it fails.
pub(super) const PATIENCE: Duration = Duration::from_secs(5);

/// How long the broker waits for a byte of a connection, or for a new connection, before it looks
/// at the test's requests again.
const SLICE: Duration = Duration::from_millis(1);

/// A CONNECT as the broker read it (§3.1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Connect {
    /// The protocol level, 4 for MQTT 3.1.1 (§3.1.2.2).
    pub(super) level: u8,
    /// The connect flags (§3.1.2.3).
    pub(super) flags: u8,
    /// The clean session flag (§3.1.2.4).
    pub(super) clean_session: bool,
    /// The keep alive in seconds (§3.1.2.10).
    pub(super) keep_alive_s: u16,
    /// The client identifier (§3.1.3.1).
    pub(super) client_id: String,
    /// The will (§3.1.2.5).
    pub(super) will: Option<Will>,
    /// The user name (§3.1.3.4).
    pub(super) username: Option<String>,
    /// The password (§3.1.3.5).
    pub(super) password: Option<Vec<u8>>,
}

/// The will of a CONNECT.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Will {
    /// The will topic.
    pub(super) topic: String,
    /// The will message.
    pub(super) message: Vec<u8>,
    /// The will's quality of service.
    pub(super) qos: u8,
    /// The will's retain flag.
    pub(super) retain: bool,
}

/// A PUBLISH from the client (§3.3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Publish {
    /// The topic name.
    pub(super) topic: String,
    /// The application message.
    pub(super) payload: Vec<u8>,
    /// The low four bits of the fixed header: DUP, the quality of service and RETAIN.
    pub(super) flags: u8,
}

/// What the broker answers a CONNECT with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Answer {
    /// CONNACK with this return code (§3.2.2.3); 0 accepts the session, and after any other the
    /// broker closes the connection (MQTT-3.2.2-5).
    ConnAck(u8),
    /// Nothing: the client waits for a CONNACK that never comes.
    Silence,
    /// The broker closes the connection without a CONNACK.
    Close,
}

/// What the broker saw.
#[derive(Debug, Default)]
pub(super) struct Record {
    /// The TCP connections it accepted.
    pub(super) accepted: usize,
    /// The CONNECT of each connection, in order.
    pub(super) connects: Vec<Connect>,
    /// The topic filters subscribed to, in order, over every connection.
    pub(super) subscriptions: Vec<String>,
    /// The client's publications, in order, over every connection.
    pub(super) publishes: Vec<Publish>,
    /// The PINGREQ packets.
    pub(super) pings: usize,
    /// The DISCONNECT packets.
    pub(super) disconnects: usize,
    /// The TLS handshakes that failed.
    pub(super) handshake_failures: usize,
}

impl Record {
    /// The payloads that the client published to `topic`, in order.
    pub(super) fn published(&self, topic: &str) -> Vec<Vec<u8>> {
        self.publishes
            .iter()
            .filter(|publish| publish.topic == topic)
            .map(|publish| publish.payload.clone())
            .collect()
    }

    /// The topics of the client's publications, in order.
    pub(super) fn topics(&self) -> Vec<&str> {
        self.publishes
            .iter()
            .map(|publish| publish.topic.as_str())
            .collect()
    }
}

/// What the test asks of the broker, and what it saw.
#[derive(Default)]
struct Control {
    record: Record,
    /// The answers to the next CONNECTs, in order.
    answers: VecDeque<Answer>,
    /// The answer to a CONNECT once `answers` are used up; `None` accepts the session.
    always: Option<Answer>,
    /// The return code of each topic of a SUBACK: 0, the granted quality of service, or 0x80, a
    /// refusal (§3.9.3).
    suback: u8,
    /// Whether the broker closes the connection after each PUBLISH it takes.
    close_on_publish: bool,
    /// The bytes for the client of the current connection, sent once its session is accepted.
    outbox: Vec<u8>,
    /// The number of the current connection; 0 before the first.
    current: u64,
    /// The number of a connection that the broker is to close.
    close: Option<u64>,
    /// Whether the broker stops.
    stop: bool,
}

/// The broker's state and its signal.
struct Inner {
    control: Mutex<Control>,
    /// Signals each change of the record.
    changed: Condvar,
}

impl Inner {
    fn lock(&self) -> MutexGuard<'_, Control> {
        self.control.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// The broker stand-in; dropping it stops it and closes its connections.
pub(super) struct Broker {
    address: SocketAddr,
    inner: Arc<Inner>,
}

impl Broker {
    /// A broker without TLS on a free port of the loopback.
    pub(super) fn start() -> Self {
        Self::listen(None)
    }

    /// A broker that takes each connection with the TLS of `acceptor` first.
    pub(super) fn start_tls(acceptor: TlsAcceptor) -> Self {
        Self::listen(Some(Arc::new(acceptor)))
    }

    fn listen(acceptor: Option<Arc<TlsAcceptor>>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind the broker stand-in");
        let address = listener.local_addr().expect("the broker's address");
        listener
            .set_nonblocking(true)
            .expect("a listener that does not block");
        let inner = Arc::new(Inner {
            control: Mutex::new(Control::default()),
            changed: Condvar::new(),
        });
        let accepting = Arc::clone(&inner);
        thread::spawn(move || accept(&listener, &accepting, acceptor.as_ref()));
        Self { address, inner }
    }

    /// The broker as `io.toml` names it, `127.0.0.1:<port>`.
    pub(super) fn endpoint(&self) -> String {
        self.address.to_string()
    }

    /// The broker's port.
    pub(super) fn port(&self) -> u16 {
        self.address.port()
    }

    /// Answers the next CONNECTs with `answers`, in order.
    pub(super) fn answer_next(&self, answers: &[Answer]) {
        self.inner.lock().answers.extend(answers.iter().copied());
    }

    /// Answers every CONNECT after the queued answers with `answer`; `None` accepts them.
    pub(super) fn answer_always(&self, answer: Option<Answer>) {
        self.inner.lock().always = answer;
    }

    /// Refuses every topic of every SUBSCRIBE from now on, with return code 0x80.
    pub(super) fn refuse_subscriptions(&self) {
        self.inner.lock().suback = 0x80;
    }

    /// Closes the connection after each PUBLISH that the broker takes.
    pub(super) fn close_on_publish(&self) {
        self.inner.lock().close_on_publish = true;
    }

    /// Delivers `messages`, each a topic and its payload, to the client of the current connection
    /// as PUBLISH packets at quality of service 0, in one write.
    pub(super) fn deliver(&self, messages: &[(&str, &[u8])]) {
        let mut bytes = Vec::new();
        for (topic, payload) in messages {
            let length = u16::try_from(topic.len()).expect("a short topic");
            let mut body = length.to_be_bytes().to_vec();
            body.extend_from_slice(topic.as_bytes());
            body.extend_from_slice(payload);
            bytes.push(0x30);
            bytes.extend(remaining_length(body.len()));
            bytes.extend(body);
        }
        self.inner.lock().outbox.extend(bytes);
    }

    /// Closes the current connection, as a broker that goes away does.
    pub(super) fn close_connection(&self) {
        let mut control = self.inner.lock();
        control.close = Some(control.current);
    }

    /// Waits until `done` holds for what the broker saw, at most [`PATIENCE`]; fails the test with
    /// `what` otherwise.
    pub(super) fn wait_for(&self, what: &str, done: impl Fn(&Record) -> bool) {
        let deadline = Instant::now() + PATIENCE;
        let mut control = self.inner.lock();
        while !done(&control.record) {
            let now = Instant::now();
            assert!(
                now < deadline,
                "the broker saw no {what} within {PATIENCE:?}: {:?}",
                control.record
            );
            control = self
                .inner
                .changed
                .wait_timeout(control, deadline - now)
                .unwrap_or_else(PoisonError::into_inner)
                .0;
        }
    }

    /// What `read` gives of what the broker saw so far.
    pub(super) fn record<T>(&self, read: impl FnOnce(&Record) -> T) -> T {
        read(&self.inner.lock().record)
    }
}

impl Drop for Broker {
    fn drop(&mut self) {
        self.inner.lock().stop = true;
    }
}

/// The path of the TLS fixture `name`, as a TOML string holds it on every system.
pub(super) fn tls_fixture(name: &str) -> String {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/tls")
        .join(name)
        .to_string_lossy()
        .replace('\\', "/")
}

/// The TLS of a broker for `localhost`, `127.0.0.1` and `::1`, with the fixture's self-signed
/// certificate, which `tls_ca_path` names as the authority.
pub(super) fn tls_acceptor() -> TlsAcceptor {
    let certificate = std::fs::read(tls_fixture("server-cert.pem")).expect("the certificate");
    let key = std::fs::read(tls_fixture("server-key.pem")).expect("the key");
    let identity = Identity::from_pkcs8(&certificate, &key).expect("the broker's identity");
    TlsAcceptor::new(identity).expect("the broker's TLS")
}

/// Takes the connections to `listener` until the broker stops, each on a thread of its own.
fn accept(listener: &TcpListener, inner: &Arc<Inner>, acceptor: Option<&Arc<TlsAcceptor>>) {
    loop {
        if inner.lock().stop {
            return;
        }
        match listener.accept() {
            Ok((stream, _)) => {
                let number = {
                    let mut control = inner.lock();
                    control.record.accepted += 1;
                    control.current += 1;
                    control.outbox.clear();
                    inner.changed.notify_all();
                    control.current
                };
                let serving = Arc::clone(inner);
                let acceptor = acceptor.cloned();
                thread::spawn(move || {
                    serve_connection(stream, &serving, acceptor.as_deref(), number);
                });
            }
            Err(_) => thread::sleep(SLICE),
        }
    }
}

/// Serves the connection `stream`, number `number`, with TLS through `acceptor` when it is given.
fn serve_connection(stream: TcpStream, inner: &Inner, acceptor: Option<&TlsAcceptor>, number: u64) {
    // A connection from a listener that does not block may not block either on some systems.
    if stream.set_nonblocking(false).is_err() {
        return;
    }
    let Some(acceptor) = acceptor else {
        if stream.set_read_timeout(Some(SLICE)).is_ok() {
            serve(stream, inner, number);
        }
        return;
    };
    if stream.set_read_timeout(Some(PATIENCE)).is_err() {
        return;
    }
    match acceptor.accept(stream) {
        Ok(tls) => {
            if tls.get_ref().set_read_timeout(Some(SLICE)).is_ok() {
                serve(tls, inner, number);
            }
        }
        Err(_) => {
            inner.lock().record.handshake_failures += 1;
            inner.changed.notify_all();
        }
    }
}

/// Answers the client of `stream` until it disconnects, the connection fails, a newer connection
/// replaces it, the test closes it or the broker stops; the outbox goes out once the session is
/// accepted.
fn serve(mut stream: impl Read + Write, inner: &Inner, number: u64) {
    let mut accepted = false;
    let mut inbound = Vec::new();
    let mut buffer = [0_u8; 4096];
    loop {
        let outgoing = {
            let mut control = inner.lock();
            if control.stop || control.current != number || control.close == Some(number) {
                return;
            }
            if accepted {
                std::mem::take(&mut control.outbox)
            } else {
                Vec::new()
            }
        };
        if !outgoing.is_empty() && send(&mut stream, &outgoing).is_err() {
            return;
        }
        match stream.read(&mut buffer) {
            Ok(0) => return,
            Ok(count) => {
                inbound.extend_from_slice(&buffer[..count]);
                while let Some((first, body)) = take_packet(&mut inbound) {
                    if !answer(&mut stream, inner, first, &body, &mut accepted) {
                        return;
                    }
                }
            }
            Err(error)
                if matches!(
                    error.kind(),
                    ErrorKind::WouldBlock | ErrorKind::TimedOut | ErrorKind::Interrupted
                ) => {}
            Err(_) => return,
        }
    }
}

/// Answers the packet of first byte `first` and body `body`; false when the connection ends.
fn answer(
    stream: &mut impl Write,
    inner: &Inner,
    first: u8,
    body: &[u8],
    accepted: &mut bool,
) -> bool {
    match first >> 4 {
        1 => {
            let Some(connect) = parse_connect(body) else {
                return false;
            };
            let answer = {
                let mut control = inner.lock();
                control.record.connects.push(connect);
                inner.changed.notify_all();
                control
                    .answers
                    .pop_front()
                    .or(control.always)
                    .unwrap_or(Answer::ConnAck(0))
            };
            match answer {
                Answer::ConnAck(code) => {
                    *accepted = code == 0;
                    send(stream, &[0x20, 0x02, 0x00, code]).is_ok() && code == 0
                }
                Answer::Silence => true,
                Answer::Close => false,
            }
        }
        3 => {
            let Some(publish) = parse_publish(first, body) else {
                return false;
            };
            let mut control = inner.lock();
            control.record.publishes.push(publish);
            inner.changed.notify_all();
            !control.close_on_publish
        }
        8 => {
            let Some((packet_id, filters)) = parse_subscribe(body) else {
                return false;
            };
            let code = inner.lock().suback;
            let length = u8::try_from(2 + filters.len()).expect("a short SUBACK");
            let mut suback = vec![0x90, length];
            suback.extend_from_slice(&packet_id.to_be_bytes());
            suback.extend(std::iter::repeat_n(code, filters.len()));
            let sent = send(stream, &suback).is_ok();
            let mut control = inner.lock();
            control.record.subscriptions.extend(filters);
            inner.changed.notify_all();
            sent
        }
        12 => {
            let sent = send(stream, &[0xD0, 0x00]).is_ok();
            inner.lock().record.pings += 1;
            inner.changed.notify_all();
            sent
        }
        14 => {
            inner.lock().record.disconnects += 1;
            inner.changed.notify_all();
            false
        }
        _ => true,
    }
}

/// Writes all of `bytes` to `stream`.
fn send(stream: &mut impl Write, bytes: &[u8]) -> std::io::Result<()> {
    stream.write_all(bytes)?;
    stream.flush()
}

/// `length` in the variable-length encoding of §2.2.3: seven bits a byte, least significant first,
/// the top bit set on every byte but the last.
fn remaining_length(mut length: usize) -> Vec<u8> {
    let mut bytes = Vec::new();
    loop {
        let digit = u8::try_from(length % 128).expect("seven bits");
        length /= 128;
        if length == 0 {
            bytes.push(digit);
            return bytes;
        }
        bytes.push(digit | 0x80);
    }
}

/// The first whole packet of `inbound`, taken out of it: its first byte and its body (§2.2).
fn take_packet(inbound: &mut Vec<u8>) -> Option<(u8, Vec<u8>)> {
    let first = *inbound.first()?;
    let mut length = 0_usize;
    let mut header = 1;
    loop {
        let byte = *inbound.get(header)?;
        length |= usize::from(byte & 0x7F) << (7 * (header - 1));
        header += 1;
        if byte & 0x80 == 0 || header == 5 {
            break;
        }
    }
    let end = header + length;
    let body = inbound.get(header..end)?.to_vec();
    inbound.drain(..end);
    Some((first, body))
}

/// Reads the fields of a packet's body in order.
struct Reader<'a>(&'a [u8]);

impl Reader<'_> {
    fn byte(&mut self) -> Option<u8> {
        let (&first, rest) = self.0.split_first()?;
        self.0 = rest;
        Some(first)
    }

    fn two_bytes(&mut self) -> Option<u16> {
        Some(u16::from_be_bytes([self.byte()?, self.byte()?]))
    }

    /// Binary data: its length in two bytes, then the bytes (§1.5.3, §3.1.3.5).
    fn data(&mut self) -> Option<Vec<u8>> {
        let length = usize::from(self.two_bytes()?);
        let (data, rest) = self.0.split_at_checked(length)?;
        self.0 = rest;
        Some(data.to_vec())
    }

    /// A UTF-8 string, in the form of binary data (§1.5.3).
    fn string(&mut self) -> Option<String> {
        String::from_utf8(self.data()?).ok()
    }

    fn rest(&mut self) -> Vec<u8> {
        let rest = self.0.to_vec();
        self.0 = &[];
        rest
    }
}

/// A CONNECT's body (§3.1): protocol name `MQTT`, level, flags, keep alive, then the client
/// identifier and each field its flag announces; nothing after them.
fn parse_connect(body: &[u8]) -> Option<Connect> {
    let mut reader = Reader(body);
    if reader.string()? != "MQTT" {
        return None;
    }
    let level = reader.byte()?;
    let flags = reader.byte()?;
    let keep_alive_s = reader.two_bytes()?;
    let client_id = reader.string()?;
    let will = if flags & 0x04 == 0 {
        None
    } else {
        Some(Will {
            topic: reader.string()?,
            message: reader.data()?,
            qos: (flags >> 3) & 0x03,
            retain: flags & 0x20 != 0,
        })
    };
    let username = if flags & 0x80 == 0 {
        None
    } else {
        Some(reader.string()?)
    };
    let password = if flags & 0x40 == 0 {
        None
    } else {
        Some(reader.data()?)
    };
    if !reader.rest().is_empty() {
        return None;
    }
    Some(Connect {
        level,
        flags,
        clean_session: flags & 0x02 != 0,
        keep_alive_s,
        client_id,
        will,
        username,
        password,
    })
}

/// A PUBLISH's body (§3.3): the topic name, the packet identifier above quality of service 0, and
/// the message.
fn parse_publish(first: u8, body: &[u8]) -> Option<Publish> {
    let mut reader = Reader(body);
    let topic = reader.string()?;
    if (first >> 1) & 0x03 > 0 {
        reader.two_bytes()?;
    }
    Some(Publish {
        topic,
        payload: reader.rest(),
        flags: first & 0x0F,
    })
}

/// A SUBSCRIBE's body (§3.8): the packet identifier, then each topic filter with its quality of
/// service.
fn parse_subscribe(body: &[u8]) -> Option<(u16, Vec<String>)> {
    let mut reader = Reader(body);
    let packet_id = reader.two_bytes()?;
    let mut filters = Vec::new();
    while !reader.0.is_empty() {
        filters.push(reader.string()?);
        reader.byte()?;
    }
    Some((packet_id, filters))
}
