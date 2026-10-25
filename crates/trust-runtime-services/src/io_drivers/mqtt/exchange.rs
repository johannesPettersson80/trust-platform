//! The `mqtt` driver's session with its broker (specification 32 §8 and §9; specification 11,
//! deadline integrity, safe-state handoff confirmation and MQTT runtime integration integrity;
//! specification 34 §11.1), a state machine without a transport or a clock.
//!
//! The numbers are those of trust-runtime's driver and its client: a reconnect backoff of
//! `reconnect_ms` after each connection failure, a PINGREQ every keep alive with the connection
//! lost when the broker has not answered the last one by the next, 5 s from opening the stream to
//! CONNACK, 5 s for outgoing bytes that the transport takes none of, packets of at most 10 KiB,
//! and at most 256 received messages kept for the scan. A chip gives the smaller bounds its memory
//! holds ([`MqttLimits`]), with its outgoing bytes bounded too.
//!
//! The driver's health has two paths (specification 32 §8). The input path holds stale or missing
//! input, a payload that does not convert, and the link as a read finds it. The output path holds
//! the link condition, which any operation that finds the link up clears, and the publish
//! condition, a failed typed-output preflight or a pending handoff, which only a completed publish
//! clears; it decides a safe-state handoff (specification 11). The reported health is the more
//! severe of the two paths, the output path's detail on a tie, degraded further by an overflow of
//! the incoming queue (specification 34 §11.1). A direction the configuration disables is always
//! healthy.

use alloc::collections::VecDeque;
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::fmt;

use smol_str::SmolStr;

use super::codec::{self, Connect, ConnectReturnCode, Incoming, Will};
use super::params::MqttParams;
use super::payload::ImageValue;
use super::sparkplug;
use crate::io_drivers::{quoted, OnError};
use crate::status::IoDriverHealth;

mod scan;

/// How long trust-runtime's scan waits for its MQTT worker to complete an output handoff, 40 ms
/// (specification 11, deadline integrity): a runtime that drives the transport on another thread
/// waits up to this long while [`MqttExchange::output_settling`] holds before
/// [`MqttExchange::write_outputs`].
pub const SCAN_WAIT_MS: u64 = 40;

/// The longest time from [`MqttPoll::Open`] to the broker's CONNACK, 5 s, the connection timeout of
/// trust-runtime's client.
pub const CONNECT_TIMEOUT_MS: u64 = 5_000;

/// The longest time that outgoing bytes wait while the transport takes none of them, 5 s, the
/// flush timeout of trust-runtime's client; after it the connection is lost.
pub const FLUSH_TIMEOUT_MS: u64 = 5_000;

/// The largest packet, 10 KiB as trust-runtime's client allows: the remaining length of a packet
/// from the broker, and the whole of a packet to it.
pub const MAX_PACKET_BYTES: usize = 10 * 1024;

/// The received messages kept for the scan (specification 34 §11.1); at the bound the oldest is
/// dropped and counted.
pub const INCOMING_QUEUE_LIMIT: usize = 256;

/// The bounds of an exchange's packets, of its incoming queue and of its outgoing bytes
/// (specification 34 §11.1): a computer's are [`MqttLimits::COMPUTER`], trust-runtime's client's;
/// a chip gives the smaller ones its memory holds, which its appendix of specification 34 names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MqttLimits {
    /// The largest packet from the broker and to it.
    pub max_packet_bytes: usize,
    /// The most received messages kept for the scan.
    pub queue_messages: usize,
    /// The most bytes of payload kept for the scan; at either bound the oldest message is dropped
    /// and counted.
    pub queue_bytes: usize,
    /// The most bytes that wait for the transport. A handoff that does not fit waits as
    /// `mqtt output handoff pending` until the transport has taken enough, a handoff larger than
    /// the bound is not published, a session whose first packets do not fit fails to connect, and a
    /// keep alive's PINGREQ that does not fit is not sent.
    pub outgoing_bytes: usize,
}

impl MqttLimits {
    /// A computer's: packets of [`MAX_PACKET_BYTES`], [`INCOMING_QUEUE_LIMIT`] messages of any
    /// total size, and outgoing bytes without a bound.
    pub const COMPUTER: Self = Self {
        max_packet_bytes: MAX_PACKET_BYTES,
        queue_messages: INCOMING_QUEUE_LIMIT,
        queue_bytes: usize::MAX,
        outgoing_bytes: usize::MAX,
    };
}

/// The health of a direction before anything happened on it.
const INITIALIZING: &str = "mqtt worker initializing";

/// The link condition while the stream opens and the session starts.
const CONNECTING: &str = "mqtt connecting";

/// The link condition while the reconnect backoff runs; its cause follows in parentheses.
const BACKOFF: &str = "mqtt reconnect backoff active";

/// The link condition after [`MqttExchange::disconnect`].
const STOPPED: &str = "mqtt disconnected: the driver stopped";

/// What the transport does next, which [`MqttExchange::poll`] answers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MqttPoll {
    /// Keep the byte stream as it is: send [`MqttExchange::outgoing`] and hand every byte from the
    /// broker to [`MqttExchange::received`].
    Wait,
    /// Open the byte stream to the broker: TCP to [`MqttParams::broker`], then TLS when
    /// [`MqttParams::tls`] is given, verifying the broker's host; then report
    /// [`MqttExchange::opened`] or [`MqttExchange::open_failed`].
    Open,
    /// Close the byte stream: the session ended, and its failure is in the health.
    Close,
}

/// The `mqtt` driver's session with its broker and its exchange with the process images. Times are
/// milliseconds of a monotonic clock.
///
/// The transport asks [`MqttExchange::poll`] for its next step after every event, opens and closes
/// the byte stream as it says, sends [`MqttExchange::outgoing`] and reports with
/// [`MqttExchange::sent`] how many bytes it took, hands every byte from the broker to
/// [`MqttExchange::received`], and reports [`MqttExchange::opened`],
/// [`MqttExchange::open_failed`] and [`MqttExchange::closed`]. The scan calls
/// [`MqttExchange::read_inputs`] at the cycle start and [`MqttExchange::write_outputs`] after the
/// tasks; neither waits for the broker.
///
/// A handoff completes when its PUBLISH packets are in the outgoing bytes of a connected session,
/// as trust-runtime's worker completes it when its client queues the publish; a runtime that
/// drives the transport itself sends the outgoing bytes after each write. A safe-state handoff is
/// confirmed when `write_outputs` succeeds and [`MqttExchange::output_health`] is `Ok` right after
/// it (specification 11).
pub struct MqttExchange {
    params: MqttParams,
    client_id: SmolStr,
    /// The subscribed topics, in order; in typed mode the index of a topic is that of its input
    /// point.
    subscriptions: Vec<SmolStr>,
    link: Link,
    /// Whether the next poll answers [`MqttPoll::Close`].
    close_pending: bool,
    outgoing: Vec<u8>,
    /// Since when the outgoing bytes wait without the transport taking any.
    stalled_since: Option<u64>,
    /// Received bytes of a packet that has not all arrived.
    inbound: Vec<u8>,
    /// The end, in `outgoing`, of the handoff the transport has not all taken.
    handoff_end: Option<usize>,
    /// The newest output image that waits for a connection or for the handoff in flight.
    pending: Option<Vec<u8>>,
    /// Received messages for the scan.
    queue: VecDeque<Message>,
    /// The payload bytes of `queue`.
    queued_bytes: usize,
    /// The bounds of packets and of the queue.
    limits: MqttLimits,
    /// Messages dropped from `queue` since the exchange started.
    dropped: u64,
    snapshot: Snapshot,
    /// The Sparkplug B sequence number of the next payload, wrapping after 255.
    sparkplug_seq: u8,
    /// The Sparkplug B `bdSeq` of the session, its CONNECT's will's.
    bd_seq: u64,
    /// The `bdSeq` of the next CONNECT: one more than the last, from 255 to 0.
    next_bd_seq: u64,
    /// With Sparkplug B, the output image last handed off, whose values a rebirth's NBIRTH
    /// carries.
    last_image: Option<Vec<u8>>,
    /// The monotonic time and the milliseconds since the Unix epoch it was set at.
    clock: Option<(u64, u64)>,
    paths: Paths,
}

/// The state of the link to the broker.
enum Link {
    /// No stream: the backoff after `cause` runs until `until_ms`; without a cause, before the
    /// first connection.
    Down {
        until_ms: u64,
        cause: Option<SmolStr>,
    },
    /// The transport opens the stream.
    Opening { since_ms: u64 },
    /// CONNECT is sent; CONNACK has not come.
    AwaitingConnAck { since_ms: u64 },
    /// The session runs.
    Connected(Session),
    /// DISCONNECT waits for the transport.
    Stopping,
    /// The exchange stopped and connects no more.
    Stopped,
}

/// A running session.
struct Session {
    /// When the next PINGREQ is due.
    next_ping_ms: u64,
    /// Whether the last PINGREQ waits for its PINGRESP.
    awaiting_pingresp: bool,
    /// A topic whose subscription the broker refused.
    refused: Option<SmolStr>,
}

/// A received message for the scan.
struct Message {
    /// The index of its topic in the subscriptions.
    topic: usize,
    payload: Vec<u8>,
}

/// The last accepted input snapshot.
enum Snapshot {
    /// Raw mode: the newest payload of `topic_in`.
    Raw(Option<Vec<u8>>),
    /// Typed points: each input point's newest value, in the order of the points.
    Points(Vec<Option<ImageValue>>),
}

/// The conditions of the driver's two paths.
struct Paths {
    input: IoDriverHealth,
    /// The output path's link condition.
    link: IoDriverHealth,
    /// The output path's publish condition.
    publish: IoDriverHealth,
}

/// The link as a received packet sees it.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Phase {
    AwaitingConnAck,
    Connected,
    Other,
}

/// What a timer asks for.
enum Due {
    Nothing,
    Ping,
    Fail(String),
    Stop,
}

impl MqttExchange {
    /// The exchange of `params` at `now_ms`, with the client identifier `params.client_id`, or
    /// `default_client_id` without one (trust-runtime's is `trust-runtime-<process id>`). The
    /// first [`MqttExchange::poll`] opens the stream when the driver exchanges inputs or outputs.
    #[must_use]
    pub fn new(params: MqttParams, default_client_id: &str, now_ms: u64) -> Self {
        let client_id = params
            .client_id
            .clone()
            .unwrap_or_else(|| SmolStr::new(default_client_id));
        // With Sparkplug B the node takes its commands, and the topics are inputs' otherwise.
        let subscriptions = match &params.sparkplug {
            Some(sparkplug) => alloc::vec![SmolStr::new(sparkplug.ncmd_topic())],
            None => params.subscribe_topics(),
        };
        let next_bd_seq = params
            .sparkplug
            .as_ref()
            .map_or(0, |sparkplug| sparkplug.birth_death_seq);
        let snapshot = if params.input_points.is_empty() {
            Snapshot::Raw(None)
        } else {
            Snapshot::Points(alloc::vec![None; params.input_points.len()])
        };
        let initializing = IoDriverHealth::Degraded {
            error: SmolStr::new(INITIALIZING),
        };
        let paths = Paths {
            input: if params.input_enabled() {
                initializing.clone()
            } else {
                IoDriverHealth::Ok
            },
            link: if params.output_enabled() {
                initializing
            } else {
                IoDriverHealth::Ok
            },
            publish: IoDriverHealth::Ok,
        };
        Self {
            params,
            client_id,
            subscriptions,
            link: Link::Down {
                until_ms: now_ms,
                cause: None,
            },
            close_pending: false,
            outgoing: Vec::new(),
            stalled_since: None,
            inbound: Vec::new(),
            handoff_end: None,
            pending: None,
            queue: VecDeque::new(),
            queued_bytes: 0,
            limits: MqttLimits::COMPUTER,
            dropped: 0,
            snapshot,
            sparkplug_seq: 0,
            bd_seq: next_bd_seq,
            next_bd_seq,
            last_image: None,
            clock: None,
            paths,
        }
    }

    /// The exchange with the bounds `limits` instead of a computer's.
    #[must_use]
    pub fn with_limits(mut self, limits: MqttLimits) -> Self {
        self.limits = limits;
        self
    }

    /// Takes the heap of a chip's buffers at once, so that what the driver needs is counted when it
    /// is claimed rather than found missing while it runs: the outgoing bytes, a packet from the
    /// broker with the `chunk_bytes` that the transport hands over at once, and the incoming
    /// queue's entries. The payloads of the queued messages, at most
    /// [`MqttLimits::queue_bytes`], are taken as they come.
    ///
    /// # Errors
    ///
    /// The allocator's refusal, with nothing kept.
    pub fn reserve_buffers(
        &mut self,
        chunk_bytes: usize,
    ) -> Result<(), alloc::collections::TryReserveError> {
        let inbound = self.limits.max_packet_bytes.saturating_add(chunk_bytes);
        let reserved = self
            .outgoing
            .try_reserve_exact(self.limits.outgoing_bytes)
            .and_then(|()| self.inbound.try_reserve_exact(inbound))
            .and_then(|()| self.queue.try_reserve_exact(self.limits.queue_messages));
        if reserved.is_err() {
            self.outgoing = Vec::new();
            self.inbound = Vec::new();
            self.queue = VecDeque::new();
        }
        reserved
    }

    /// When [`MqttExchange::poll`] next has work that no event of the transport or the scan
    /// brings: a close it asks for, the end of the backoff, the connect timeout, the keep alive, a
    /// stalled stream's flush timeout, or the end of an orderly stop; `None` while only an event
    /// moves it on.
    #[must_use]
    pub fn due_ms(&self) -> Option<u64> {
        if self.close_pending {
            return Some(0);
        }
        let stalled = self
            .stalled_since
            .map(|since| since.saturating_add(FLUSH_TIMEOUT_MS));
        let timer = match &self.link {
            Link::Down { until_ms, .. } if self.wants_link() => Some(*until_ms),
            Link::Opening { since_ms } | Link::AwaitingConnAck { since_ms } => {
                Some(since_ms.saturating_add(CONNECT_TIMEOUT_MS))
            }
            Link::Connected(session) => Some(session.next_ping_ms),
            Link::Stopping if self.outgoing.is_empty() => Some(0),
            Link::Down { .. } | Link::Stopping | Link::Stopped => None,
        };
        [timer, stalled].into_iter().flatten().min()
    }

    /// The parameters, whose broker and TLS settings the transport connects with.
    #[must_use]
    pub fn params(&self) -> &MqttParams {
        &self.params
    }

    /// The client identifier the session connects with.
    #[must_use]
    pub fn client_id(&self) -> &str {
        &self.client_id
    }

    /// What an error of the driver does to the resource.
    #[must_use]
    pub fn on_error(&self) -> OnError {
        self.params.on_error
    }

    /// Whether the session runs: CONNACK accepted it and it has not ended.
    #[must_use]
    pub fn is_connected(&self) -> bool {
        matches!(self.link, Link::Connected(_))
    }

    /// Sets the wall clock: `utc_ms` milliseconds since the Unix epoch at `now_ms`. The Sparkplug B
    /// timestamps count from it; before it is set they are 0.
    pub fn set_clock(&mut self, now_ms: u64, utc_ms: u64) {
        self.clock = Some((now_ms, utc_ms));
    }

    /// The transport's next step at `now_ms`, after the timers ran: a connect timeout, a stalled
    /// stream and a keep alive without its PINGRESP end the session, a keep alive that is due
    /// queues a PINGREQ, and the end of the backoff opens the stream again.
    #[must_use]
    pub fn poll(&mut self, now_ms: u64) -> MqttPoll {
        self.run_timers(now_ms);
        if core::mem::take(&mut self.close_pending) {
            return MqttPoll::Close;
        }
        if let Link::Down { until_ms, .. } = self.link
            && self.wants_link()
            && now_ms >= until_ms
        {
            self.link = Link::Opening { since_ms: now_ms };
            return MqttPoll::Open;
        }
        MqttPoll::Wait
    }

    /// The stream that [`MqttPoll::Open`] asked for is open at `now_ms`: CONNECT goes out, with the
    /// keep alive, the credentials, a clean session, and with Sparkplug, its next `bdSeq` and its
    /// NDEATH as the will. A stream the exchange no longer wants is closed at the next poll.
    pub fn opened(&mut self, now_ms: u64) {
        match self.link {
            Link::Opening { since_ms } => {
                self.next_bd_seq();
                match self.connect_packet(now_ms) {
                    Ok(packet) => {
                        self.clear_session_bytes();
                        if self.queue_bytes(now_ms, &packet) {
                            self.link = Link::AwaitingConnAck { since_ms };
                        } else {
                            let text = self.no_room("CONNECT", packet.len());
                            self.end_session(now_ms, format!("mqtt connect failed: {text}"), true);
                        }
                    }
                    Err(text) => {
                        self.end_session(now_ms, format!("mqtt connect failed: {text}"), true);
                    }
                }
            }
            Link::Down { .. } | Link::Stopped => self.close_pending = true,
            Link::AwaitingConnAck { .. } | Link::Connected(_) | Link::Stopping => {}
        }
    }

    /// The stream that [`MqttPoll::Open`] asked for could not be opened at `now_ms`, for the
    /// transport's `detail`: the backoff starts.
    pub fn open_failed(&mut self, now_ms: u64, detail: &str) {
        if matches!(self.link, Link::Opening { .. }) {
            self.end_session(now_ms, format!("mqtt connect failed: {detail}"), false);
        }
    }

    /// The stream closed or failed at `now_ms`, for the transport's `detail`: a connect that had
    /// not finished failed, and a running session is lost; the backoff starts.
    pub fn closed(&mut self, now_ms: u64, detail: &str) {
        match self.link {
            Link::Opening { .. } | Link::AwaitingConnAck { .. } => {
                self.end_session(now_ms, format!("mqtt connect failed: {detail}"), false);
            }
            Link::Connected(_) => {
                self.end_session(now_ms, format!("mqtt disconnected: {detail}"), false);
            }
            Link::Stopping => self.stop(),
            Link::Down { .. } | Link::Stopped => {}
        }
    }

    /// Takes the bytes `bytes` that came from the broker at `now_ms`. Each whole packet is taken in
    /// order: a CONNACK that accepts starts the session (its subscriptions, Sparkplug's NBIRTH,
    /// then the newest output image that waited), a PUBLISH on a subscribed topic joins the
    /// incoming queue, and a PINGRESP answers the keep alive. A refused CONNACK, a malformed
    /// packet, or a packet the client does not take ends the session, which the next poll closes.
    pub fn received(&mut self, now_ms: u64, bytes: &[u8]) {
        if self.phase() == Phase::Other {
            return;
        }
        let mut buffer = core::mem::take(&mut self.inbound);
        buffer.extend_from_slice(bytes);
        let mut used = 0;
        while self.phase() != Phase::Other {
            match codec::decode(
                buffer.get(used..).unwrap_or_default(),
                self.limits.max_packet_bytes,
            ) {
                Ok(Some((packet, length))) => {
                    used += length;
                    self.take_packet(now_ms, packet);
                }
                Ok(None) => break,
                Err(error) => {
                    let failure = if self.phase() == Phase::AwaitingConnAck {
                        "mqtt connect failed"
                    } else {
                        "mqtt disconnected"
                    };
                    self.end_session(
                        now_ms,
                        format!("{failure}: malformed packet from the broker: {error}"),
                        true,
                    );
                }
            }
        }
        if self.phase() == Phase::Other {
            // The session ended with its bytes; the buffer stays for the next one.
            buffer.clear();
        } else {
            buffer.drain(..used);
        }
        self.inbound = buffer;
    }

    /// The bytes that wait for the transport, oldest first.
    #[must_use]
    pub fn outgoing(&self) -> &[u8] {
        &self.outgoing
    }

    /// The transport took the first `count` bytes of [`MqttExchange::outgoing`] at `now_ms`. When
    /// that completes the handoff in flight, the newest image that waited for it is handed off.
    pub fn sent(&mut self, now_ms: u64, count: usize) {
        let count = count.min(self.outgoing.len());
        if count == 0 {
            return;
        }
        self.outgoing.drain(..count);
        self.stalled_since = if self.outgoing.is_empty() {
            None
        } else {
            Some(now_ms)
        };
        if let Some(end) = self.handoff_end {
            if count < end {
                self.handoff_end = Some(end - count);
                return;
            }
            self.handoff_end = None;
        }
        // The newest image that waited, for the handoff in flight or for room in the outgoing
        // bytes, is handed off now.
        if self.is_connected()
            && let Some(image) = self.pending.take()
        {
            self.publish_image(now_ms, &image);
        }
    }

    /// Ends the session in order at `now_ms` and connects no more: with Sparkplug an NDEATH, then
    /// DISCONNECT, which the next poll closes the stream after; a stream that is still opening is
    /// closed at once.
    pub fn disconnect(&mut self, now_ms: u64) {
        match self.link {
            Link::Connected(_) => {
                // DISCONNECT tells the broker to drop the will, so it goes only behind the NDEATH
                // that the will is; without room for them the broker publishes the will when the
                // stream closes.
                let death_queued = match self.death_packet(now_ms) {
                    Some(packet) => self.queue_bytes(now_ms, &packet),
                    None => true,
                };
                if death_queued {
                    self.queue_bytes(now_ms, &codec::DISCONNECT);
                }
                self.link = Link::Stopping;
            }
            Link::Opening { .. } | Link::AwaitingConnAck { .. } => {
                self.stop();
                self.close_pending = true;
            }
            Link::Down { .. } => self.stop(),
            Link::Stopping | Link::Stopped => {}
        }
    }

    /// Whether the link is wanted: the driver exchanges inputs or outputs.
    fn wants_link(&self) -> bool {
        self.params.input_enabled() || self.params.output_enabled()
    }

    /// The link as a received packet sees it.
    fn phase(&self) -> Phase {
        match self.link {
            Link::AwaitingConnAck { .. } => Phase::AwaitingConnAck,
            Link::Connected(_) => Phase::Connected,
            Link::Down { .. } | Link::Opening { .. } | Link::Stopping | Link::Stopped => {
                Phase::Other
            }
        }
    }

    /// The link condition of a session that does not run, at `now_ms`.
    fn link_text(&self, now_ms: u64) -> String {
        match &self.link {
            Link::Down {
                until_ms,
                cause: Some(cause),
            } if now_ms < *until_ms => format!("{BACKOFF} ({cause})"),
            Link::Stopping | Link::Stopped => String::from(STOPPED),
            _ => String::from(CONNECTING),
        }
    }

    /// `text` as a condition under `on_error`: faulted under `fault`, degraded otherwise.
    fn policy_health(&self, text: &str) -> IoDriverHealth {
        let error = SmolStr::new(text);
        match self.params.on_error {
            OnError::Fault => IoDriverHealth::Faulted { error },
            OnError::Warn | OnError::Ignore => IoDriverHealth::Degraded { error },
        }
    }

    /// Runs the timers at `now_ms`.
    fn run_timers(&mut self, now_ms: u64) {
        let stalled = self
            .stalled_since
            .is_some_and(|since| now_ms.saturating_sub(since) >= FLUSH_TIMEOUT_MS);
        let keep_alive_s = self.params.keep_alive_s;
        let drained = self.outgoing.is_empty();
        let due = match &mut self.link {
            Link::Opening { since_ms } | Link::AwaitingConnAck { since_ms }
                if now_ms.saturating_sub(*since_ms) >= CONNECT_TIMEOUT_MS =>
            {
                Due::Fail(format!(
                    "mqtt connect failed: no session within {CONNECT_TIMEOUT_MS} ms"
                ))
            }
            Link::Connected(_) if stalled => Due::Fail(format!(
                "mqtt disconnected: the connection took no bytes for {FLUSH_TIMEOUT_MS} ms"
            )),
            Link::Connected(session) if now_ms >= session.next_ping_ms => {
                if session.awaiting_pingresp {
                    Due::Fail(format!(
                        "mqtt disconnected: no PINGRESP within the keep alive of {keep_alive_s} s"
                    ))
                } else {
                    session.awaiting_pingresp = true;
                    session.next_ping_ms =
                        now_ms.saturating_add(u64::from(keep_alive_s).saturating_mul(1_000));
                    Due::Ping
                }
            }
            Link::Stopping if stalled || drained => Due::Stop,
            _ => Due::Nothing,
        };
        match due {
            Due::Nothing => {}
            Due::Ping => {
                // Without room the stream is behind, and the session ends at the next keep alive
                // unless a PINGRESP came.
                self.queue_bytes(now_ms, &codec::PINGREQ);
            }
            Due::Fail(cause) => self.end_session(now_ms, cause, true),
            Due::Stop => {
                self.stop();
                self.close_pending = true;
            }
        }
    }

    /// Takes the Sparkplug B `bdSeq` of a new connection: the first connection's is
    /// `birth_death_seq`, and each later one's one more than the last, from 255 to 0
    /// (`tck-id-message-flow-edge-node-birth-publish-will-message-payload-bdSeq`).
    fn next_bd_seq(&mut self) {
        self.bd_seq = self.next_bd_seq;
        self.next_bd_seq = (self.bd_seq + 1) % 256;
    }

    /// The CONNECT of the session at `now_ms`: a clean session, as Sparkplug B requires of its
    /// edge nodes too (`tck-id-principles-persistence-clean-session-311`), and with Sparkplug B the
    /// NDEATH as the will, at quality of service 1 and not retained.
    fn connect_packet(&self, now_ms: u64) -> Result<Vec<u8>, String> {
        let death = self.params.sparkplug.as_ref().map(|sparkplug| {
            (
                sparkplug.ndeath_topic(),
                sparkplug::encode_ndeath(self.bd_seq, self.utc_ms(now_ms)),
            )
        });
        let credentials = self.params.credentials.as_ref();
        let connect = Connect {
            client_id: &self.client_id,
            keep_alive_s: self.params.keep_alive_s,
            clean_session: true,
            will: death.as_ref().map(|(topic, message)| Will {
                topic: topic.as_str(),
                message: message.as_slice(),
                retain: false,
                at_least_once: true,
            }),
            username: credentials.map(|credentials| credentials.username.as_str()),
            // An empty password is not sent, as trust-runtime's client does not send one.
            password: credentials
                .map(|credentials| credentials.password.as_bytes())
                .filter(|password| !password.is_empty()),
        };
        let packet =
            codec::encode_connect(&connect).map_err(|error| format!("CONNECT: {error}"))?;
        let max = self.limits.max_packet_bytes;
        if packet.len() > max {
            return Err(format!(
                "CONNECT of {} bytes exceeds the {max}-byte packet limit",
                packet.len()
            ));
        }
        Ok(packet)
    }

    /// Takes one packet from the broker at `now_ms`.
    fn take_packet(&mut self, now_ms: u64, packet: Incoming<'_>) {
        match (self.phase(), packet) {
            (
                Phase::AwaitingConnAck,
                Incoming::ConnAck {
                    return_code: ConnectReturnCode::Accepted,
                    ..
                },
            ) => self.session_up(now_ms),
            (Phase::AwaitingConnAck, Incoming::ConnAck { return_code, .. }) => self.end_session(
                now_ms,
                format!("mqtt connect failed: the broker refused the connection: {return_code}"),
                true,
            ),
            (Phase::AwaitingConnAck, other) => self.end_session(
                now_ms,
                format!(
                    "mqtt connect failed: the broker sent {} before CONNACK",
                    other.name()
                ),
                true,
            ),
            (
                Phase::Connected,
                Incoming::Publish {
                    topic,
                    payload,
                    packet_id,
                    ..
                },
            ) => {
                if let Some(packet_id) = packet_id {
                    self.queue_bytes(now_ms, &codec::encode_puback(packet_id));
                }
                if self.params.sparkplug.is_none() {
                    self.enqueue(topic, payload);
                } else if self.subscriptions.first().is_some_and(|ncmd| ncmd == topic)
                    && sparkplug::is_rebirth_request(payload)
                {
                    self.rebirth(now_ms);
                }
            }
            (
                Phase::Connected,
                Incoming::SubAck {
                    packet_id,
                    return_codes,
                },
            ) => self.note_suback(packet_id, return_codes),
            (Phase::Connected, Incoming::PingResp) => {
                if let Link::Connected(session) = &mut self.link {
                    session.awaiting_pingresp = false;
                }
            }
            (Phase::Connected, Incoming::ConnAck { .. }) => self.end_session(
                now_ms,
                String::from("mqtt disconnected: the broker sent a second CONNACK"),
                true,
            ),
            (Phase::Other, _) => {}
        }
    }

    /// Starts the session at `now_ms`: a SUBSCRIBE per topic, with Sparkplug B to the NCMD topic at
    /// quality of service 1 before the NBIRTH (`tck-id-message-flow-edge-node-ncmd-subscribe`),
    /// and then the newest output image that waited.
    fn session_up(&mut self, now_ms: u64) {
        let sparkplug = self.params.sparkplug.is_some();
        let subscribe = self
            .subscriptions
            .iter()
            .enumerate()
            .map(|(index, topic)| {
                if sparkplug {
                    codec::encode_subscribe_at_least_once(packet_id(index), topic)
                } else {
                    codec::encode_subscribe(packet_id(index), topic)
                }
            })
            .collect::<Result<Vec<_>, _>>();
        let packets = match subscribe {
            Ok(packets) => packets,
            Err(error) => {
                self.end_session(
                    now_ms,
                    format!("mqtt connect failed: SUBSCRIBE: {error}"),
                    true,
                );
                return;
            }
        };
        let birth = match self.birth_packet(now_ms) {
            Ok(birth) => birth,
            Err(text) => {
                self.end_session(
                    now_ms,
                    format!("mqtt connect failed: Sparkplug NBIRTH: {text}"),
                    true,
                );
                return;
            }
        };
        let first: usize = packets.iter().chain(birth.iter()).map(Vec::len).sum();
        if self.outgoing.len().saturating_add(first) > self.limits.outgoing_bytes {
            let text = self.no_room("the session start", first);
            self.end_session(now_ms, format!("mqtt connect failed: {text}"), true);
            return;
        }
        let keep_alive_ms = u64::from(self.params.keep_alive_s).saturating_mul(1_000);
        self.link = Link::Connected(Session {
            next_ping_ms: now_ms.saturating_add(keep_alive_ms),
            awaiting_pingresp: false,
            refused: None,
        });
        for packet in packets.iter().chain(birth.iter()) {
            self.queue_bytes(now_ms, packet);
        }
        if self.params.output_enabled() {
            self.paths.link = IoDriverHealth::Ok;
        }
        if let Some(image) = self.pending.take() {
            self.publish_image(now_ms, &image);
        }
    }

    /// The NBIRTH of a Sparkplug session at `now_ms`, which takes the next sequence number, with
    /// the session's `bdSeq` and the values of the newest output image, the one that waits for the
    /// session or the one last handed off.
    fn birth_packet(&mut self, now_ms: u64) -> Result<Option<Vec<u8>>, String> {
        let Some(sparkplug) = &self.params.sparkplug else {
            return Ok(None);
        };
        let seq = self.sparkplug_seq;
        let image = self.pending.as_deref().or(self.last_image.as_deref());
        let payload = sparkplug::encode_nbirth(
            &self.params.output_points,
            image,
            self.bd_seq,
            u64::from(seq),
            self.utc_ms(now_ms),
        );
        let packet = publish_packet(
            &sparkplug.nbirth_topic(),
            &payload,
            self.limits.max_packet_bytes,
        )?;
        self.sparkplug_seq = seq.wrapping_add(1);
        Ok(Some(packet))
    }

    /// A rebirth at `now_ms`, which a host asked for with `Node Control/Rebirth` in NCMD: a new
    /// NBIRTH of the session, with its `bdSeq`, ahead of any later NDATA
    /// (`tck-id-operational-behavior-data-commands-rebirth-action-1` to `-3`). An NBIRTH that the
    /// outgoing bound has no room for ends the session instead, and the next one begins with its
    /// birth.
    fn rebirth(&mut self, now_ms: u64) {
        match self.birth_packet(now_ms) {
            Ok(Some(packet)) => {
                if !self.queue_bytes(now_ms, &packet) {
                    let text = self.no_room("the rebirth's NBIRTH", packet.len());
                    self.end_session(now_ms, format!("mqtt disconnected: {text}"), true);
                }
            }
            Ok(None) => {}
            Err(text) => self.end_session(
                now_ms,
                format!("mqtt disconnected: Sparkplug NBIRTH: {text}"),
                true,
            ),
        }
    }

    /// The NDEATH that an orderly end publishes with Sparkplug, as the will would.
    fn death_packet(&self, now_ms: u64) -> Option<Vec<u8>> {
        let sparkplug = self.params.sparkplug.as_ref()?;
        let payload = sparkplug::encode_ndeath(self.bd_seq, self.utc_ms(now_ms));
        publish_packet(
            &sparkplug.ndeath_topic(),
            &payload,
            self.limits.max_packet_bytes,
        )
        .ok()
    }

    /// Queues the message of `topic` for the scan when the topic is subscribed; at the queue's
    /// bounds ([`MqttLimits`]) the oldest message is dropped and counted.
    fn enqueue(&mut self, topic: &str, payload: &[u8]) {
        let Some(index) = self
            .subscriptions
            .iter()
            .position(|subscribed| subscribed.as_str() == topic)
        else {
            return;
        };
        if payload.len() > self.limits.queue_bytes {
            // A message larger than the whole queue is dropped and counted; the queue stays.
            self.dropped = self.dropped.saturating_add(1);
            return;
        }
        while !self.queue.is_empty()
            && (self.queue.len() >= self.limits.queue_messages
                || self.queued_bytes.saturating_add(payload.len()) > self.limits.queue_bytes)
        {
            if let Some(oldest) = self.queue.pop_front() {
                self.queued_bytes = self.queued_bytes.saturating_sub(oldest.payload.len());
            }
            self.dropped = self.dropped.saturating_add(1);
        }
        self.queued_bytes = self.queued_bytes.saturating_add(payload.len());
        self.queue.push_back(Message {
            topic: index,
            payload: payload.to_vec(),
        });
    }

    /// Notes the topic of a SUBSCRIBE that the broker refused, which a stale read then names.
    fn note_suback(&mut self, packet_id: u16, return_codes: &[u8]) {
        if !return_codes.contains(&0x80) {
            return;
        }
        let topic = usize::from(packet_id)
            .checked_sub(1)
            .and_then(|index| self.subscriptions.get(index))
            .cloned();
        if let (Some(topic), Link::Connected(session)) = (topic, &mut self.link) {
            session.refused = Some(topic);
        }
    }

    /// Ends the session at `now_ms` for `cause`, which becomes the link condition of both paths;
    /// the backoff starts, and with `close` the next poll closes the stream. The bytes of the
    /// session and the messages not yet read go; the newest output image waits on.
    fn end_session(&mut self, now_ms: u64, cause: String, close: bool) {
        let condition = self.policy_health(&cause);
        if self.params.input_enabled() {
            self.paths.input = condition.clone();
        }
        if self.params.output_enabled() {
            self.paths.link = condition;
        }
        self.link = Link::Down {
            until_ms: now_ms.saturating_add(self.params.reconnect_ms),
            cause: Some(SmolStr::from(cause)),
        };
        self.close_pending |= close;
        self.clear_session_bytes();
    }

    /// Stops the exchange: it connects no more.
    fn stop(&mut self) {
        self.link = Link::Stopped;
        self.clear_session_bytes();
    }

    /// Drops the bytes and the messages of the session that ended.
    fn clear_session_bytes(&mut self) {
        self.outgoing.clear();
        self.inbound.clear();
        self.stalled_since = None;
        self.handoff_end = None;
        self.queue.clear();
        self.queued_bytes = 0;
    }

    /// Appends `bytes` to the outgoing bytes at `now_ms` when the outgoing bound has room for all
    /// of them; whether it had.
    fn queue_bytes(&mut self, now_ms: u64, bytes: &[u8]) -> bool {
        if bytes.is_empty() {
            return true;
        }
        if self.outgoing.len().saturating_add(bytes.len()) > self.limits.outgoing_bytes {
            return false;
        }
        if self.outgoing.is_empty() {
            self.stalled_since = Some(now_ms);
        }
        self.outgoing.extend_from_slice(bytes);
        true
    }

    /// The condition of `what`, `bytes` long, for which the outgoing bound has no room.
    pub(super) fn no_room(&self, what: &str, bytes: usize) -> String {
        format!(
            "{what} of {bytes} bytes exceeds the {}-byte outgoing buffer",
            self.limits.outgoing_bytes
        )
    }

    /// The milliseconds since the Unix epoch at `now_ms`, 0 while the clock is not set.
    fn utc_ms(&self, now_ms: u64) -> u64 {
        self.clock.map_or(0, |(at_ms, utc_ms)| {
            utc_ms.saturating_add(now_ms.saturating_sub(at_ms))
        })
    }

    /// The link's state in words, for `Debug`.
    fn link_name(&self) -> &'static str {
        match self.link {
            Link::Down { .. } => "down",
            Link::Opening { .. } => "opening",
            Link::AwaitingConnAck { .. } => "awaiting CONNACK",
            Link::Connected(_) => "connected",
            Link::Stopping => "stopping",
            Link::Stopped => "stopped",
        }
    }
}

/// Shows the broker, the client identifier, the link and the health; never the outgoing bytes,
/// whose CONNECT holds the password.
impl fmt::Debug for MqttExchange {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("MqttExchange")
            .field("broker", &self.params.broker)
            .field("client_id", &self.client_id)
            .field("link", &self.link_name())
            .field("health", &self.health())
            .field("output_health", &self.output_health())
            .finish_non_exhaustive()
    }
}

/// The PUBLISH of `payload` to `topic`, within `max` bytes.
pub(super) fn publish_packet(topic: &str, payload: &[u8], max: usize) -> Result<Vec<u8>, String> {
    let packet = codec::encode_publish(topic, payload).map_err(|error| error.to_string())?;
    if packet.len() > max {
        return Err(format!(
            "the PUBLISH to {} of {} bytes exceeds the {max}-byte packet limit",
            quoted(topic),
            packet.len()
        ));
    }
    Ok(packet)
}

/// The packet identifier of the SUBSCRIBE of the topic at `index`: 1 for the first, wrapping
/// after 65,535.
fn packet_id(index: usize) -> u16 {
    u16::try_from(index % 65_535).map_or(1, |id| id + 1)
}
