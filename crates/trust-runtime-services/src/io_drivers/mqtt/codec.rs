//! The MQTT 3.1.1 packets of the `mqtt` driver's client (OASIS MQTT Version 3.1.1, 29 October
//! 2014; specification 32 §10 for the remaining length): the client encodes CONNECT, PUBLISH at
//! quality of service 0, SUBSCRIBE at 0 or 1, PUBACK, PINGREQ and DISCONNECT, and decodes the
//! CONNACK, PUBLISH at 0 or 1, SUBACK and PINGRESP that a broker sends it.
//!
//! [`decode`] takes the bytes that have arrived so far: it answers `Ok(None)` until a whole packet
//! is there, and refuses a malformed packet as soon as its bytes show it, with a [`DecodeError`].

use alloc::vec::Vec;
use core::fmt;

/// The largest remaining length, the most that four bytes encode (§2.2.3): 268,435,455.
pub const MAX_REMAINING_LENGTH: usize = 268_435_455;

/// PINGREQ (§3.12): its fixed header, with remaining length 0.
pub const PINGREQ: [u8; 2] = [0xC0, 0x00];

/// DISCONNECT (§3.14): its fixed header, with remaining length 0.
pub const DISCONNECT: [u8; 2] = [0xE0, 0x00];

/// PUBACK (§3.4) of the PUBLISH at quality of service 1 with the packet identifier `packet_id`.
#[must_use]
pub fn encode_puback(packet_id: u16) -> [u8; 4] {
    let [high, low] = packet_id.to_be_bytes();
    [0x40, 0x02, high, low]
}

/// The longest string or binary field: its length takes two bytes (§1.5.3).
const MAX_FIELD: usize = 65_535;

/// A CONNECT packet (§3.1), with protocol name `MQTT` and protocol level 4.
#[derive(Clone, Copy)]
pub struct Connect<'a> {
    /// The client identifier (§3.1.3.1); empty only with a clean session.
    pub client_id: &'a str,
    /// The keep alive in seconds (§3.1.2.10).
    pub keep_alive_s: u16,
    /// The clean session flag (§3.1.2.4).
    pub clean_session: bool,
    /// The will that the broker publishes when the connection ends without DISCONNECT (§3.1.2.5).
    pub will: Option<Will<'a>>,
    /// The user name (§3.1.3.4).
    pub username: Option<&'a str>,
    /// The password (§3.1.3.5), only with a user name.
    pub password: Option<&'a [u8]>,
}

/// Shows every field but the password, which it writes as `<redacted>`.
impl fmt::Debug for Connect<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Connect")
            .field("client_id", &self.client_id)
            .field("keep_alive_s", &self.keep_alive_s)
            .field("clean_session", &self.clean_session)
            .field("will", &self.will)
            .field("username", &self.username)
            .field("password", &self.password.map(|_| "<redacted>"))
            .finish()
    }
}

/// The will of a CONNECT (§3.1.2.5), at quality of service 0.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Will<'a> {
    /// The will topic: a topic name, without wildcards.
    pub topic: &'a str,
    /// The will message.
    pub message: &'a [u8],
    /// The will retain flag.
    pub retain: bool,
    /// The will at quality of service 1, at least once, instead of 0 (§3.1.2.6), as Sparkplug B's
    /// NDEATH is.
    pub at_least_once: bool,
}

/// The return code of a CONNACK (§3.2.2.3, Table 3.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectReturnCode {
    /// 0: connection accepted.
    Accepted,
    /// 1: the server does not support level 4 of the protocol.
    UnacceptableProtocolVersion,
    /// 2: the client identifier is correct UTF-8 but not allowed by the server.
    IdentifierRejected,
    /// 3: the MQTT service is unavailable.
    ServerUnavailable,
    /// 4: the user name or password is malformed.
    BadUserNameOrPassword,
    /// 5: the client is not authorized to connect.
    NotAuthorized,
}

impl ConnectReturnCode {
    /// The return code of the byte `code`; `None` above 5, which §3.2.2.3 reserves.
    #[must_use]
    pub fn from_code(code: u8) -> Option<Self> {
        Some(match code {
            0 => Self::Accepted,
            1 => Self::UnacceptableProtocolVersion,
            2 => Self::IdentifierRejected,
            3 => Self::ServerUnavailable,
            4 => Self::BadUserNameOrPassword,
            5 => Self::NotAuthorized,
            _ => return None,
        })
    }

    /// The byte of the return code.
    #[must_use]
    pub fn code(self) -> u8 {
        match self {
            Self::Accepted => 0,
            Self::UnacceptableProtocolVersion => 1,
            Self::IdentifierRejected => 2,
            Self::ServerUnavailable => 3,
            Self::BadUserNameOrPassword => 4,
            Self::NotAuthorized => 5,
        }
    }
}

/// The words of Table 3.1 and the code, such as `bad user name or password (return code 4)`.
impl fmt::Display for ConnectReturnCode {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let words = match self {
            Self::Accepted => "connection accepted",
            Self::UnacceptableProtocolVersion => "unacceptable protocol version",
            Self::IdentifierRejected => "identifier rejected",
            Self::ServerUnavailable => "server unavailable",
            Self::BadUserNameOrPassword => "bad user name or password",
            Self::NotAuthorized => "not authorized",
        };
        write!(formatter, "{words} (return code {})", self.code())
    }
}

/// A packet from the broker.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Incoming<'a> {
    /// CONNACK (§3.2).
    ConnAck {
        /// The session present flag, never with a refusal.
        session_present: bool,
        /// The return code.
        return_code: ConnectReturnCode,
    },
    /// PUBLISH at quality of service 0 or 1 (§3.3).
    Publish {
        /// The topic name.
        topic: &'a str,
        /// The application message.
        payload: &'a [u8],
        /// The retain flag: the message was retained by the broker.
        retain: bool,
        /// At quality of service 1, the packet identifier that the client's PUBACK names
        /// ([`encode_puback`]); `None` at 0.
        packet_id: Option<u16>,
    },
    /// SUBACK (§3.9).
    SubAck {
        /// The packet identifier of the SUBSCRIBE it answers.
        packet_id: u16,
        /// One return code per topic filter: 0, 1 or 2, the granted quality of service, or 0x80,
        /// a failure.
        return_codes: &'a [u8],
    },
    /// PINGRESP (§3.13).
    PingResp,
}

impl Incoming<'_> {
    /// The packet's name, such as `CONNACK`.
    #[must_use]
    pub fn name(&self) -> &'static str {
        match self {
            Self::ConnAck { .. } => "CONNACK",
            Self::Publish { .. } => "PUBLISH",
            Self::SubAck { .. } => "SUBACK",
            Self::PingResp => "PINGRESP",
        }
    }
}

/// Why bytes from the broker are not a packet the client takes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DecodeError {
    /// Packet type 0 or 15, which §2.2.1 reserves.
    ReservedPacketType(u8),
    /// A packet the client does not take from a broker: CONNECT, SUBSCRIBE, UNSUBSCRIBE, PINGREQ
    /// and DISCONNECT go from a client to a server, and the client, which publishes and
    /// subscribes at quality of service 0 and never unsubscribes, gets no PUBACK, PUBREC, PUBREL,
    /// PUBCOMP or UNSUBACK.
    UnexpectedPacket(&'static str),
    /// Fixed-header flags that the packet does not take (§2.2.2): a CONNACK, SUBACK or PINGRESP
    /// with a flag, or a PUBLISH at quality of service 3 or with DUP at quality of service 0
    /// (MQTT-3.3.1-2), (MQTT-3.3.1-4).
    InvalidFlags {
        /// The packet's name.
        packet: &'static str,
        /// The low four bits of its first byte.
        flags: u8,
    },
    /// A PUBLISH at quality of service 2: the client subscribes at 1 at most, so a broker sends 1
    /// at most (§3.8.4).
    UnexpectedQos(u8),
    /// A PUBLISH at quality of service 1 with packet identifier 0 (§2.3.1).
    ZeroPacketId,
    /// A remaining length whose fourth byte has its continuation bit (§2.2.3).
    RemainingLengthTooLong,
    /// A remaining length in more bytes than its value takes (specification 32 §10).
    RemainingLengthNotMinimal,
    /// A remaining length above the client's limit.
    PacketTooLarge {
        /// The remaining length.
        remaining: usize,
        /// The limit.
        limit: usize,
    },
    /// A field that runs past the end of its packet.
    FieldPastEnd(&'static str),
    /// A packet whose remaining length is not that of its fields.
    InvalidLength {
        /// The packet's name.
        packet: &'static str,
        /// Its remaining length.
        found: usize,
        /// The remaining length of its fields.
        expected: usize,
    },
    /// A CONNACK with a reserved acknowledge flag (§3.2.2.1).
    ConnAckReservedFlags(u8),
    /// A CONNACK return code above 5 (§3.2.2.3).
    UnknownReturnCode(u8),
    /// A CONNACK that refuses the connection with session present set (MQTT-3.2.2-4).
    SessionPresentOnRefusal(u8),
    /// A SUBACK return code other than 0, 1, 2 and 0x80 (MQTT-3.9.3-2).
    UnknownSubAckCode(u8),
    /// A string that is not well-formed UTF-8 (MQTT-1.5.3-1).
    InvalidUtf8,
    /// A string that holds U+0000 (MQTT-1.5.3-2).
    NullCharacter,
    /// A PUBLISH with an empty topic name (MQTT-4.7.3-1).
    EmptyTopicName,
    /// A PUBLISH whose topic name holds `+` or `#` (MQTT-3.3.2-2).
    WildcardInTopicName,
}

impl fmt::Display for DecodeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ReservedPacketType(kind) => write!(formatter, "reserved packet type {kind}"),
            Self::UnexpectedPacket(name) => write!(formatter, "unexpected {name}"),
            Self::InvalidFlags { packet, flags } => {
                write!(formatter, "{packet} with fixed-header flags {flags:#06b}")
            }
            Self::UnexpectedQos(qos) => {
                write!(
                    formatter,
                    "PUBLISH at QoS {qos}, above the QoS 1 the client subscribes at"
                )
            }
            Self::ZeroPacketId => formatter.write_str("PUBLISH at QoS 1 with packet identifier 0"),
            Self::RemainingLengthTooLong => {
                formatter.write_str("remaining length longer than four bytes")
            }
            Self::RemainingLengthNotMinimal => {
                formatter.write_str("remaining length not in its shortest encoding")
            }
            Self::PacketTooLarge { remaining, limit } => write!(
                formatter,
                "remaining length {remaining} above the limit of {limit} bytes"
            ),
            Self::FieldPastEnd(packet) => {
                write!(formatter, "{packet} field past the end of the packet")
            }
            Self::InvalidLength {
                packet,
                found,
                expected,
            } => write!(
                formatter,
                "{packet} with remaining length {found}, not {expected}"
            ),
            Self::ConnAckReservedFlags(flags) => {
                write!(
                    formatter,
                    "CONNACK with reserved acknowledge flags {flags:#04x}"
                )
            }
            Self::UnknownReturnCode(code) => {
                write!(formatter, "CONNACK with unknown return code {code}")
            }
            Self::SessionPresentOnRefusal(code) => write!(
                formatter,
                "CONNACK with session present on refusal code {code}"
            ),
            Self::UnknownSubAckCode(code) => {
                write!(formatter, "SUBACK with unknown return code {code:#04x}")
            }
            Self::InvalidUtf8 => formatter.write_str("string that is not UTF-8"),
            Self::NullCharacter => formatter.write_str("string with U+0000"),
            Self::EmptyTopicName => formatter.write_str("PUBLISH with an empty topic name"),
            Self::WildcardInTopicName => {
                formatter.write_str("PUBLISH with a wildcard in its topic name")
            }
        }
    }
}

/// Why a packet cannot be encoded.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EncodeError {
    /// A string longer than 65,535 bytes (§1.5.3).
    StringTooLong {
        /// The field.
        field: &'static str,
        /// Its length in bytes.
        length: usize,
    },
    /// A string that holds U+0000 (MQTT-1.5.3-2).
    NullCharacter(&'static str),
    /// A will message or a password longer than 65,535 bytes.
    DataTooLong {
        /// The field.
        field: &'static str,
        /// Its length in bytes.
        length: usize,
    },
    /// A topic name that is empty or holds `+` or `#` (MQTT-3.3.2-2), (MQTT-4.7.3-1).
    InvalidTopicName(&'static str),
    /// An empty topic filter (MQTT-4.7.3-1).
    EmptyTopicFilter,
    /// Packet identifier 0 (MQTT-2.3.1-1).
    ZeroPacketId,
    /// A password without a user name (MQTT-3.1.2-22).
    PasswordWithoutUserName,
    /// An empty client identifier without a clean session (MQTT-3.1.3-7).
    EmptyClientIdWithoutCleanSession,
    /// A remaining length above [`MAX_REMAINING_LENGTH`].
    RemainingLengthTooLarge(usize),
}

impl fmt::Display for EncodeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::StringTooLong { field, length } | Self::DataTooLong { field, length } => {
                write!(
                    formatter,
                    "{field} of {length} bytes, more than {MAX_FIELD}"
                )
            }
            Self::NullCharacter(field) => write!(formatter, "{field} with U+0000"),
            Self::InvalidTopicName(field) => {
                write!(formatter, "{field} empty or with a wildcard + or #")
            }
            Self::EmptyTopicFilter => formatter.write_str("empty topic filter"),
            Self::ZeroPacketId => formatter.write_str("packet identifier 0"),
            Self::PasswordWithoutUserName => formatter.write_str("password without user name"),
            Self::EmptyClientIdWithoutCleanSession => {
                formatter.write_str("empty client identifier without clean session")
            }
            Self::RemainingLengthTooLarge(length) => write!(
                formatter,
                "remaining length {length}, more than {MAX_REMAINING_LENGTH}"
            ),
        }
    }
}

/// CONNECT (§3.1): the variable header, then the client identifier, the will topic and message,
/// the user name and the password, each flag set when its field is there.
///
/// # Errors
///
/// [`EncodeError`] for a string longer than 65,535 bytes or with U+0000, a will message or
/// password longer than 65,535 bytes, a will topic that is no topic name, a password without a
/// user name, and an empty client identifier without a clean session.
pub fn encode_connect(connect: &Connect<'_>) -> Result<Vec<u8>, EncodeError> {
    let mut flags = 0_u8;
    if connect.clean_session {
        flags |= 0x02;
    } else if connect.client_id.is_empty() {
        return Err(EncodeError::EmptyClientIdWithoutCleanSession);
    }
    if let Some(will) = &connect.will {
        flags |= 0x04;
        if will.at_least_once {
            flags |= 0x08;
        }
        if will.retain {
            flags |= 0x20;
        }
    }
    if connect.username.is_some() {
        flags |= 0x80;
    }
    if connect.password.is_some() {
        if connect.username.is_none() {
            return Err(EncodeError::PasswordWithoutUserName);
        }
        flags |= 0x40;
    }
    let mut body = Vec::new();
    put_string(&mut body, "MQTT", "protocol name")?;
    body.push(4);
    body.push(flags);
    body.extend_from_slice(&connect.keep_alive_s.to_be_bytes());
    put_string(&mut body, connect.client_id, "client identifier")?;
    if let Some(will) = &connect.will {
        check_topic_name(will.topic, "will topic")?;
        put_string(&mut body, will.topic, "will topic")?;
        put_data(&mut body, will.message, "will message")?;
    }
    if let Some(username) = connect.username {
        put_string(&mut body, username, "user name")?;
    }
    if let Some(password) = connect.password {
        put_data(&mut body, password, "password")?;
    }
    packet(0x10, &body)
}

/// PUBLISH at quality of service 0, without DUP and RETAIN (§3.3): the topic name and the
/// message, without a packet identifier.
///
/// # Errors
///
/// [`EncodeError`] for a topic name that is empty, holds a wildcard or U+0000, or is longer than
/// 65,535 bytes, and a packet above [`MAX_REMAINING_LENGTH`].
pub fn encode_publish(topic: &str, payload: &[u8]) -> Result<Vec<u8>, EncodeError> {
    check_topic_name(topic, "topic name")?;
    let mut body = Vec::with_capacity(topic.len().saturating_add(payload.len()).saturating_add(2));
    put_string(&mut body, topic, "topic name")?;
    body.extend_from_slice(payload);
    packet(0x30, &body)
}

/// SUBSCRIBE with one topic filter at requested quality of service 0 (§3.8), as trust-runtime's
/// client sends one per topic.
///
/// # Errors
///
/// [`EncodeError`] for packet identifier 0, and a topic filter that is empty, holds U+0000 or is
/// longer than 65,535 bytes.
pub fn encode_subscribe(packet_id: u16, filter: &str) -> Result<Vec<u8>, EncodeError> {
    subscribe(packet_id, filter, 0)
}

/// SUBSCRIBE with one topic filter at requested quality of service 1 (§3.8), as Sparkplug B's
/// edge node subscribes to its NCMD topic.
///
/// # Errors
///
/// As [`encode_subscribe`].
pub fn encode_subscribe_at_least_once(
    packet_id: u16,
    filter: &str,
) -> Result<Vec<u8>, EncodeError> {
    subscribe(packet_id, filter, 1)
}

/// SUBSCRIBE with one topic filter at requested quality of service `qos`.
fn subscribe(packet_id: u16, filter: &str, qos: u8) -> Result<Vec<u8>, EncodeError> {
    if packet_id == 0 {
        return Err(EncodeError::ZeroPacketId);
    }
    if filter.is_empty() {
        return Err(EncodeError::EmptyTopicFilter);
    }
    let mut body = Vec::with_capacity(filter.len().saturating_add(5));
    body.extend_from_slice(&packet_id.to_be_bytes());
    put_string(&mut body, filter, "topic filter")?;
    body.push(qos);
    packet(0x82, &body)
}

/// Appends `length` in the variable-length encoding of §2.2.3: seven bits a byte, least
/// significant first, the top bit set on every byte but the last, in the fewest bytes.
///
/// # Errors
///
/// [`EncodeError::RemainingLengthTooLarge`] above [`MAX_REMAINING_LENGTH`].
pub fn encode_remaining_length(length: usize, out: &mut Vec<u8>) -> Result<(), EncodeError> {
    if length > MAX_REMAINING_LENGTH {
        return Err(EncodeError::RemainingLengthTooLarge(length));
    }
    let mut rest = length;
    loop {
        let digit = (rest % 128).to_le_bytes()[0];
        rest /= 128;
        if rest == 0 {
            out.push(digit);
            return Ok(());
        }
        out.push(digit | 0x80);
    }
}

/// The remaining length at the start of `bytes` and the number of bytes it takes; `None` while
/// its last byte has not arrived.
///
/// # Errors
///
/// [`DecodeError::RemainingLengthTooLong`] when the fourth byte has its continuation bit, and
/// [`DecodeError::RemainingLengthNotMinimal`] when a final byte of 0 follows another byte.
pub fn decode_remaining_length(bytes: &[u8]) -> Result<Option<(usize, usize)>, DecodeError> {
    let mut length = 0_usize;
    for (index, byte) in bytes.iter().copied().take(4).enumerate() {
        length |= usize::from(byte & 0x7F) << (7 * index);
        if byte & 0x80 == 0 {
            if index > 0 && byte == 0 {
                return Err(DecodeError::RemainingLengthNotMinimal);
            }
            return Ok(Some((length, index + 1)));
        }
    }
    if bytes.len() >= 4 {
        Err(DecodeError::RemainingLengthTooLong)
    } else {
        Ok(None)
    }
}

/// The packet at the start of `bytes` and the number of bytes it takes; `None` while it has not
/// all arrived. The fixed header is checked as soon as it is there, and a remaining length above
/// `max_remaining` is refused before the rest arrives.
///
/// # Errors
///
/// [`DecodeError`] for bytes that are no packet the client takes from a broker.
pub fn decode(
    bytes: &[u8],
    max_remaining: usize,
) -> Result<Option<(Incoming<'_>, usize)>, DecodeError> {
    let Some(&first) = bytes.first() else {
        return Ok(None);
    };
    let kind = header_kind(first)?;
    let Some((remaining, length_bytes)) =
        decode_remaining_length(bytes.get(1..).unwrap_or_default())?
    else {
        return Ok(None);
    };
    if remaining > max_remaining {
        return Err(DecodeError::PacketTooLarge {
            remaining,
            limit: max_remaining,
        });
    }
    let start = 1 + length_bytes;
    let end = start + remaining;
    let Some(body) = bytes.get(start..end) else {
        return Ok(None);
    };
    let packet = match kind {
        Kind::ConnAck => connack(body)?,
        Kind::Publish {
            retain,
            at_least_once,
        } => publish(body, retain, at_least_once)?,
        Kind::SubAck => suback(body)?,
        Kind::PingResp => {
            if !body.is_empty() {
                return Err(DecodeError::InvalidLength {
                    packet: "PINGRESP",
                    found: body.len(),
                    expected: 0,
                });
            }
            Incoming::PingResp
        }
    };
    Ok(Some((packet, end)))
}

/// What the first byte of a packet from the broker announces.
#[derive(Clone, Copy)]
enum Kind {
    ConnAck,
    Publish { retain: bool, at_least_once: bool },
    SubAck,
    PingResp,
}

/// The packet that the first byte `first` announces, with its flags checked.
fn header_kind(first: u8) -> Result<Kind, DecodeError> {
    let flags = first & 0x0F;
    match first >> 4 {
        2 => without_flags(flags, "CONNACK", Kind::ConnAck),
        3 => publish_kind(flags),
        9 => without_flags(flags, "SUBACK", Kind::SubAck),
        13 => without_flags(flags, "PINGRESP", Kind::PingResp),
        kind @ (0 | 15) => Err(DecodeError::ReservedPacketType(kind)),
        kind => Err(DecodeError::UnexpectedPacket(packet_name(kind))),
    }
}

/// `kind` when `flags` are 0, as §2.2.2 gives them for every packet the broker sends but PUBLISH.
fn without_flags(flags: u8, packet: &'static str, kind: Kind) -> Result<Kind, DecodeError> {
    if flags == 0 {
        Ok(kind)
    } else {
        Err(DecodeError::InvalidFlags { packet, flags })
    }
}

/// A PUBLISH with the flags `flags`: quality of service 0 without DUP, or 1 with or without it;
/// RETAIN as it comes.
fn publish_kind(flags: u8) -> Result<Kind, DecodeError> {
    let qos = (flags >> 1) & 0x03;
    let dup = flags & 0x08 != 0;
    let retain = flags & 0x01 != 0;
    match qos {
        0 if !dup => Ok(Kind::Publish {
            retain,
            at_least_once: false,
        }),
        1 => Ok(Kind::Publish {
            retain,
            at_least_once: true,
        }),
        2 => Err(DecodeError::UnexpectedQos(qos)),
        _ => Err(DecodeError::InvalidFlags {
            packet: "PUBLISH",
            flags,
        }),
    }
}

/// The name of the packet type `kind` (§2.2.1, Table 2.1).
fn packet_name(kind: u8) -> &'static str {
    match kind {
        1 => "CONNECT",
        2 => "CONNACK",
        3 => "PUBLISH",
        4 => "PUBACK",
        5 => "PUBREC",
        6 => "PUBREL",
        7 => "PUBCOMP",
        8 => "SUBSCRIBE",
        9 => "SUBACK",
        10 => "UNSUBSCRIBE",
        11 => "UNSUBACK",
        12 => "PINGREQ",
        13 => "PINGRESP",
        14 => "DISCONNECT",
        _ => "reserved packet",
    }
}

/// A CONNACK's two bytes (§3.2): acknowledge flags with only session present, and a return code
/// from 0 to 5; session present only with 0.
fn connack(body: &[u8]) -> Result<Incoming<'_>, DecodeError> {
    let [flags, code] = body else {
        return Err(DecodeError::InvalidLength {
            packet: "CONNACK",
            found: body.len(),
            expected: 2,
        });
    };
    if flags & 0xFE != 0 {
        return Err(DecodeError::ConnAckReservedFlags(*flags));
    }
    let return_code =
        ConnectReturnCode::from_code(*code).ok_or(DecodeError::UnknownReturnCode(*code))?;
    let session_present = flags & 0x01 != 0;
    if session_present && return_code != ConnectReturnCode::Accepted {
        return Err(DecodeError::SessionPresentOnRefusal(*code));
    }
    Ok(Incoming::ConnAck {
        session_present,
        return_code,
    })
}

/// A PUBLISH (§3.3): the topic name, a non-empty string without wildcards, at quality of service
/// 1 the packet identifier, not 0, and the rest of the packet as its message.
fn publish(body: &[u8], retain: bool, at_least_once: bool) -> Result<Incoming<'_>, DecodeError> {
    let (topic, rest) = take_string(body, "PUBLISH")?;
    if topic.is_empty() {
        return Err(DecodeError::EmptyTopicName);
    }
    if topic.contains(['+', '#']) {
        return Err(DecodeError::WildcardInTopicName);
    }
    let (packet_id, payload) = if at_least_once {
        let Some((id, payload)) = rest.split_first_chunk::<2>() else {
            return Err(DecodeError::FieldPastEnd("PUBLISH"));
        };
        match u16::from_be_bytes(*id) {
            0 => return Err(DecodeError::ZeroPacketId),
            id => (Some(id), payload),
        }
    } else {
        (None, rest)
    };
    Ok(Incoming::Publish {
        topic,
        payload,
        retain,
        packet_id,
    })
}

/// A SUBACK (§3.9): the packet identifier and at least one return code, each 0, 1, 2 or 0x80.
fn suback(body: &[u8]) -> Result<Incoming<'_>, DecodeError> {
    let Some((packet_id, return_codes)) = body.split_first_chunk::<2>() else {
        return Err(DecodeError::FieldPastEnd("SUBACK"));
    };
    if return_codes.is_empty() {
        return Err(DecodeError::FieldPastEnd("SUBACK"));
    }
    if let Some(code) = return_codes
        .iter()
        .copied()
        .find(|code| !matches!(code, 0x00 | 0x01 | 0x02 | 0x80))
    {
        return Err(DecodeError::UnknownSubAckCode(code));
    }
    Ok(Incoming::SubAck {
        packet_id: u16::from_be_bytes(*packet_id),
        return_codes,
    })
}

/// The UTF-8 string at the start of `bytes` (§1.5.3) and the bytes after it.
fn take_string<'a>(
    bytes: &'a [u8],
    packet: &'static str,
) -> Result<(&'a str, &'a [u8]), DecodeError> {
    let Some((length, rest)) = bytes.split_first_chunk::<2>() else {
        return Err(DecodeError::FieldPastEnd(packet));
    };
    let Some((text, rest)) = rest.split_at_checked(usize::from(u16::from_be_bytes(*length))) else {
        return Err(DecodeError::FieldPastEnd(packet));
    };
    let text = core::str::from_utf8(text).map_err(|_| DecodeError::InvalidUtf8)?;
    if text.contains('\0') {
        return Err(DecodeError::NullCharacter);
    }
    Ok((text, rest))
}

/// The packet of first byte `first` and `body`: the fixed header with the remaining length, then
/// the body.
fn packet(first: u8, body: &[u8]) -> Result<Vec<u8>, EncodeError> {
    let mut out = Vec::with_capacity(body.len().saturating_add(5));
    out.push(first);
    encode_remaining_length(body.len(), &mut out)?;
    out.extend_from_slice(body);
    Ok(out)
}

/// Appends the string `text` of `field`: its length in two bytes, most significant first, and its
/// UTF-8 bytes.
fn put_string(out: &mut Vec<u8>, text: &str, field: &'static str) -> Result<(), EncodeError> {
    if text.contains('\0') {
        return Err(EncodeError::NullCharacter(field));
    }
    let length = u16::try_from(text.len()).map_err(|_| EncodeError::StringTooLong {
        field,
        length: text.len(),
    })?;
    out.extend_from_slice(&length.to_be_bytes());
    out.extend_from_slice(text.as_bytes());
    Ok(())
}

/// Appends the binary data `data` of `field`: its length in two bytes and the bytes.
fn put_data(out: &mut Vec<u8>, data: &[u8], field: &'static str) -> Result<(), EncodeError> {
    let length = u16::try_from(data.len()).map_err(|_| EncodeError::DataTooLong {
        field,
        length: data.len(),
    })?;
    out.extend_from_slice(&length.to_be_bytes());
    out.extend_from_slice(data);
    Ok(())
}

/// Refuses a topic name that is empty or holds a wildcard.
fn check_topic_name(topic: &str, field: &'static str) -> Result<(), EncodeError> {
    if topic.is_empty() || topic.contains(['+', '#']) {
        return Err(EncodeError::InvalidTopicName(field));
    }
    Ok(())
}
