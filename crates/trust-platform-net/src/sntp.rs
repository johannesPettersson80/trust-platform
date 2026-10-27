//! The chip's time-server client (specification 34 TARGET-06): a chip without a battery-held clock
//! takes its time from the time server of its settings, `[time] server` of `device.toml`, by the
//! Simple Network Time Protocol, version 4 (RFC 4330, on the NTP of RFC 5905).
//!
//! The chip's network loop sends [`request`] to the server's UDP port [`NTP_PORT`], with the
//! [`NtpTimestamp`] of its clock's reading as it sends, and hands the reply that comes back from
//! that server's address and port to [`parse_reply`], with that timestamp and its clock's reading
//! as the reply arrived. The [`SntpTime`] it gets back is the time to set the clock to. Nothing
//! here sends, receives or reads a clock. The loop asks no more often than every 15 seconds (RFC
//! 4330 §10), and after a kiss-o'-death it asks that server less often (`RATE`) or no more
//! (`DENY`, `RSTR`; RFC 4330 §8, RFC 5905 §7.4).
//!
//! Times are milliseconds since 1970-01-01T00:00:00Z. An NTP timestamp counts 32-bit seconds from
//! 1900-01-01T00:00:00Z, which wrap every 136 years, the first time at 2036-02-07T06:28:16Z;
//! [`NtpTimestamp::to_unix_ms`] says which era a timestamp is taken in. Not implemented: a reply's
//! optional authenticator (RFC 4330 §4), which public time servers do not use, and the root delay
//! and dispersion with which a full NTP client chooses among servers (RFC 5905), which a client
//! that sets its clock from one server has no use for.

use core::fmt;

/// The UDP port of NTP and SNTP (RFC 4330, RFC 5905).
pub const NTP_PORT: u16 = 123;

/// The bytes of an NTP message without extension fields or authentication (RFC 4330 §4).
const MESSAGE_BYTES: usize = 48;
/// The first byte of a request: leap indicator 0, version 4 and mode 3, a client (RFC 4330 §4,
/// §5).
const CLIENT_REQUEST: u8 = 0x23;
/// The mode of a server's reply (RFC 4330 §4).
const MODE_SERVER: u8 = 4;
/// The leap indicator of a server whose clock is not synchronized (RFC 4330 §4).
const LEAP_UNSYNCHRONIZED: u8 = 3;
/// The highest stratum of a synchronized server (RFC 4330 §4; RFC 5905 §7.3).
const MAX_STRATUM: u8 = 15;
/// The byte of the reference identifier, which a kiss-o'-death's code is in (RFC 4330 §8).
const REFERENCE_AT: usize = 12;
/// The byte of the originate timestamp (RFC 4330 §4).
const ORIGINATE_AT: usize = 24;
/// The byte of the receive timestamp (RFC 4330 §4).
const RECEIVE_AT: usize = 32;
/// The byte of the transmit timestamp (RFC 4330 §4).
const TRANSMIT_AT: usize = 40;
/// The seconds from 1900-01-01, the start of NTP's era 0, to 1970-01-01: 70 years with 17 leap
/// days, (70 × 365 + 17) × 86,400 s.
const UNIX_EPOCH_SECONDS: u64 = 2_208_988_800;
/// The seconds of one NTP era, 2^32.
const ERA_SECONDS: u64 = 1 << 32;
/// The Unix epoch in era 0, in 2^-32 ms: the unit in which [`NtpTimestamp::resolve`] works, which
/// holds an NTP timestamp, 2^-32 s, times 1,000 and a millisecond times 2^32, both exactly.
const UNIX_EPOCH_UNITS: i128 = 2_208_988_800_000 << 32;
/// One NTP era, 2^32 s, in 2^-32 ms.
const ERA_UNITS: i128 = 1_000 << 64;
/// Half a millisecond in 2^-32 ms, which rounds to the nearest millisecond.
const HALF_MS_UNITS: i128 = 1 << 31;

/// An NTP timestamp (RFC 4330 §3; RFC 5905 §6): the seconds since 1900-01-01T00:00:00Z in the
/// upper 32 bits and the fraction of a second in the lower 32. The seconds wrap every 2^32 s, about
/// 136 years: era 0 ends at 2036-02-07T06:28:16Z, and era 1 counts from 0 again.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NtpTimestamp(pub u64);

impl NtpTimestamp {
    /// The timestamp of `ms` milliseconds since 1970-01-01T00:00:00Z, its fraction rounded down to
    /// a whole 2^-32 s. A time from 2036-02-07T06:28:16Z on wraps into its era, as NTP's 32-bit
    /// seconds do.
    #[must_use]
    pub fn from_unix_ms(ms: u64) -> Self {
        let seconds = (ms / 1_000 + UNIX_EPOCH_SECONDS) % ERA_SECONDS;
        let fraction = ((ms % 1_000) << 32) / 1_000;
        Self((seconds << 32) | fraction)
    }

    /// The milliseconds since 1970-01-01T00:00:00Z of this timestamp, rounded to the nearest, in
    /// the era that puts it nearest `near_unix_ms`. A time that this era would put before 1970,
    /// which a `u64` of Unix milliseconds cannot hold, is taken one era later, and a time past
    /// `u64::MAX` gives `u64::MAX`.
    ///
    /// A time comes out right when it lies within 68 years of `near_unix_ms`, and when it lies
    /// further after it but before 2106, by the rule for times before 1970. So the caller gives as
    /// `near_unix_ms` the best time it knows: the chip's clock once a time source has set it, and
    /// before that a floor that the true time cannot precede, such as the firmware's build date. A
    /// clock that reads 1970 because nobody set it still resolves every time up to 2106.
    #[must_use]
    pub fn to_unix_ms(self, near_unix_ms: u64) -> u64 {
        clamp_u64(round_ms(self.resolve(near_unix_ms)))
    }

    /// This timestamp in the era that [`NtpTimestamp::to_unix_ms`] takes, in 2^-32 ms since
    /// 1970-01-01T00:00:00Z: never negative, and at most about 2^97.
    fn resolve(self, near_unix_ms: u64) -> i128 {
        let in_era_0 = i128::from(self.0) * 1_000 - UNIX_EPOCH_UNITS;
        let near = i128::from(near_unix_ms) << 32;
        let eras = (near - in_era_0 + ERA_UNITS / 2).div_euclid(ERA_UNITS);
        let resolved = in_era_0 + eras * ERA_UNITS;
        if resolved < 0 {
            resolved + ERA_UNITS
        } else {
            resolved
        }
    }
}

/// The time that a time server's reply gives.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SntpTime {
    /// The server's time when the reply arrived, the time to set the clock to, in milliseconds
    /// since 1970-01-01T00:00:00Z: the clock's reading when the reply arrived plus `offset_ms`,
    /// held between 0 and `u64::MAX`.
    pub unix_ms: u64,
    /// How far the server's clock is ahead of the chip's, in milliseconds; negative when it is
    /// behind.
    pub offset_ms: i64,
    /// The time the request and the reply took on the network, without the server's own, in
    /// milliseconds; 0 when the timestamps make it negative.
    pub delay_ms: u64,
    /// The server's stratum, 1 to 15: 1 for a server with a reference clock of its own, and one
    /// more for each server between it and one.
    pub stratum: u8,
}

/// Why a time server's reply is not taken (RFC 4330 §5).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SntpError {
    /// The reply is shorter than an NTP message's 48 bytes.
    Short,
    /// The reply's mode is not 4, a server's reply to a client: not a broadcast (5), which the
    /// chip does not ask for, nor any other mode.
    NotServer,
    /// The server refuses the request: stratum 0, a kiss-o'-death, with its four-character code
    /// from the reference identifier, such as `RATE` or `DENY` (RFC 4330 §8).
    KissOfDeath([u8; 4]),
    /// The server's clock is not synchronized: leap indicator 3, or a stratum above 15.
    Unsynchronized,
    /// The reply's originate timestamp is not the request's transmit timestamp: an old reply, a
    /// reply to another request, or a forged one.
    Mismatch,
    /// The reply's transmit timestamp is 0.
    ZeroTransmit,
}

impl fmt::Display for SntpError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Short => formatter.write_str("the time server's reply is shorter than 48 bytes"),
            Self::NotServer => {
                formatter.write_str("the reply is not a time server's reply to a client")
            }
            Self::KissOfDeath(code) => {
                formatter.write_str("the time server refuses the request with the kiss code ")?;
                for byte in code {
                    write!(formatter, "{}", core::ascii::escape_default(*byte))?;
                }
                Ok(())
            }
            Self::Unsynchronized => {
                formatter.write_str("the time server's clock is not synchronized")
            }
            Self::Mismatch => formatter.write_str(
                "the reply does not answer the request: its originate timestamp is not the \
                 request's transmit timestamp",
            ),
            Self::ZeroTransmit => {
                formatter.write_str("the time server's reply has no transmit timestamp")
            }
        }
    }
}

impl core::error::Error for SntpError {}

/// The request to a time server (RFC 4330 §5): 48 bytes, the first with leap indicator 0, version 4
/// and mode 3, a client, and the last eight `transmit`, which the server's reply returns as its
/// originate timestamp; every other byte 0.
#[must_use]
pub fn request(transmit: NtpTimestamp) -> [u8; 48] {
    let mut message = [0_u8; MESSAGE_BYTES];
    let [first, ..] = &mut message;
    *first = CLIENT_REQUEST;
    if let Some(timestamp) = message
        .get_mut(TRANSMIT_AT..)
        .and_then(<[u8]>::first_chunk_mut::<8>)
    {
        *timestamp = transmit.0.to_be_bytes();
    }
    message
}

/// Reads `reply`, a time server's reply to the request whose transmit timestamp was `sent`, which
/// arrived when the chip's clock read `received_unix_ms`, and gives the server's time and how far
/// its clock is from the chip's (RFC 4330 §5). `sent` is [`NtpTimestamp::from_unix_ms`] of the
/// same clock, not set between the request and the reply; the era of the server's timestamps is
/// the one nearest `near_unix_ms`, as [`NtpTimestamp::to_unix_ms`] says. A reply longer than 48
/// bytes, with extension fields or authentication, is read for its first 48.
///
/// With T1 `sent`, T2 the reply's receive timestamp, T3 its transmit timestamp and T4
/// `received_unix_ms`, the offset is ((T2 - T1) + (T3 - T4)) / 2 and the delay (T4 - T1) - (T3 -
/// T2), worked out in 2^-32 ms and rounded to the nearest millisecond.
///
/// # Errors
///
/// In the order they are checked: [`SntpError::Short`] for a reply shorter than 48 bytes,
/// [`SntpError::NotServer`] for a mode other than 4, [`SntpError::Mismatch`] for an originate
/// timestamp other than `sent`, checked before the stratum so that a forged kiss-o'-death is not
/// taken, [`SntpError::KissOfDeath`] for stratum 0, [`SntpError::Unsynchronized`] for leap
/// indicator 3 or a stratum above 15, and [`SntpError::ZeroTransmit`] for a transmit timestamp of
/// 0.
pub fn parse_reply(
    reply: &[u8],
    sent: NtpTimestamp,
    received_unix_ms: u64,
    near_unix_ms: u64,
) -> Result<SntpTime, SntpError> {
    let message = reply
        .first_chunk::<MESSAGE_BYTES>()
        .ok_or(SntpError::Short)?;
    let [first, stratum, ..] = *message;
    if (first & 0x07) != MODE_SERVER {
        return Err(SntpError::NotServer);
    }
    if u64::from_be_bytes(field(message, ORIGINATE_AT)) != sent.0 {
        return Err(SntpError::Mismatch);
    }
    if stratum == 0 {
        return Err(SntpError::KissOfDeath(field(message, REFERENCE_AT)));
    }
    if (first >> 6) == LEAP_UNSYNCHRONIZED || stratum > MAX_STRATUM {
        return Err(SntpError::Unsynchronized);
    }
    let receive = NtpTimestamp(u64::from_be_bytes(field(message, RECEIVE_AT)));
    let transmit = NtpTimestamp(u64::from_be_bytes(field(message, TRANSMIT_AT)));
    if transmit.0 == 0 {
        return Err(SntpError::ZeroTransmit);
    }
    let t1 = sent.resolve(received_unix_ms);
    let t2 = receive.resolve(near_unix_ms);
    let t3 = transmit.resolve(near_unix_ms);
    let t4 = i128::from(received_unix_ms) << 32;
    let offset_ms = clamp_i64(round_ms((t2 - t1).midpoint(t3 - t4)));
    let delay_ms = clamp_u64(round_ms((t4 - t1) - (t3 - t2)));
    Ok(SntpTime {
        unix_ms: clamp_u64(i128::from(received_unix_ms) + i128::from(offset_ms)),
        offset_ms,
        delay_ms,
        stratum,
    })
}

/// The `N` bytes at `at` of `message`; zeros past its end, which a reply's fixed fields never
/// reach.
fn field<const N: usize>(message: &[u8], at: usize) -> [u8; N] {
    message
        .get(at..)
        .and_then(<[u8]>::first_chunk::<N>)
        .copied()
        .unwrap_or([0; N])
}

/// The whole milliseconds nearest `units` of 2^-32 ms, half a millisecond rounding up.
fn round_ms(units: i128) -> i128 {
    (units + HALF_MS_UNITS) >> 32
}

/// `value` in a `u64`: 0 below its range and `u64::MAX` above.
fn clamp_u64(value: i128) -> u64 {
    u64::try_from(value).unwrap_or(if value < 0 { 0 } else { u64::MAX })
}

/// `value` in an `i64`: `i64::MIN` below its range and `i64::MAX` above.
fn clamp_i64(value: i128) -> i64 {
    i64::try_from(value).unwrap_or(if value < 0 { i64::MIN } else { i64::MAX })
}

#[cfg(test)]
mod tests;
