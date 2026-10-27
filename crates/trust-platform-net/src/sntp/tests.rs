//! The chip's time-server client against RFC 4330: the request's bytes, the eras of NTP
//! timestamps, and replies whose timestamps are worked out by hand. An NTP timestamp is the
//! seconds since 1900-01-01T00:00:00Z in its upper 32 bits and the fraction of a second, in units
//! of 2^-32 s, in its lower 32 (RFC 4330 §3).

use alloc::string::ToString;

use super::*;

/// T1 of the worked reply, the request's transmit timestamp: 1,000,000 ms after the Unix epoch by
/// the chip's clock, 2,208,988,800 + 1,000 = 2,208,989,800 s after 1900 = `0x83AA_8268`, fraction
/// 0.
const SENT: NtpTimestamp = NtpTimestamp(0x83AA_8268_0000_0000);
/// T2 of the worked reply, the server's receive timestamp: 4,600,020 ms after the Unix epoch,
/// 2,208,988,800 + 4,600 = 2,208,993,400 s = `0x83AA_9078`, and 0.020 s = 0.020 × 2^32 =
/// 85,899,345.92, rounded down 85,899,345 = `0x051E_B851`.
const RECEIVE: u64 = 0x83AA_9078_051E_B851;
/// T3 of the worked reply, the server's transmit timestamp: 4,600,025 ms after the Unix epoch,
/// `0x83AA_9078` s, and 0.025 s = 0.025 × 2^32 = 107,374,182.4, rounded down 107,374,182 =
/// `0x0666_6666`.
const TRANSMIT: u64 = 0x83AA_9078_0666_6666;
/// A stratum 2 server's reference identifier: the address of the server it takes its time from.
const UPSTREAM: [u8; 4] = [192, 0, 2, 1];

/// A server's reply of 48 bytes (RFC 4330 §4): the first byte (leap indicator, version and mode),
/// the stratum, the reference identifier and the originate, receive and transmit timestamps;
/// every other byte 0.
fn server_reply(
    first: u8,
    stratum: u8,
    reference: [u8; 4],
    originate: u64,
    receive: u64,
    transmit: u64,
) -> [u8; 48] {
    let mut reply = [0_u8; 48];
    reply[0] = first;
    reply[1] = stratum;
    reply[12..16].copy_from_slice(&reference);
    reply[24..32].copy_from_slice(&originate.to_be_bytes());
    reply[32..40].copy_from_slice(&receive.to_be_bytes());
    reply[40..48].copy_from_slice(&transmit.to_be_bytes());
    reply
}

/// Why `reply` to the worked request is refused, `None` when it is taken.
fn refusal(reply: &[u8]) -> Option<SntpError> {
    parse_reply(reply, SENT, 1_000_045, 1_000_045).err()
}

/// RFC 4330 §4 and §5: a request is 48 bytes, the first `0x23` (leap indicator 0, version 4, mode
/// 3, a client: 0b00 100 011) and the last eight its transmit timestamp, every other byte 0. 1,500
/// ms after the Unix epoch is 2,208,988,801 s after 1900, `0x83AA_7E81`, and half a second, 2^31 =
/// `0x8000_0000`. Catches a request that a server refuses, or answers with an originate timestamp
/// that the chip cannot match.
#[test]
fn a_request_carries_its_transmit_timestamp() {
    let transmit = NtpTimestamp::from_unix_ms(1_500);
    assert_eq!(transmit, NtpTimestamp(0x83AA_7E81_8000_0000));
    let mut expected = [0_u8; 48];
    expected[0] = 0x23;
    expected[40..].copy_from_slice(&[0x83, 0xAA, 0x7E, 0x81, 0x80, 0x00, 0x00, 0x00]);
    assert_eq!(request(transmit), expected);
}

/// RFC 5905 §6: the Unix epoch, 1970-01-01T00:00:00Z, is 70 years of 365 days and 17 leap days
/// after NTP's 1900-01-01: (70 × 365 + 17) × 86,400 s = 25,567 × 86,400 s = 2,208,988,800 s =
/// `0x83AA_7E80`. Catches every time set 70 years off, or off by the leap days.
#[test]
fn the_unix_epoch_is_2_208_988_800_seconds_after_1900() {
    assert_eq!(
        NtpTimestamp::from_unix_ms(0),
        NtpTimestamp(0x83AA_7E80_0000_0000)
    );
    assert_eq!(NtpTimestamp(0x83AA_7E80_0000_0000).to_unix_ms(0), 0);
}

/// A millisecond's fraction of a second is rounded down to whole 2^-32 s on the way in, and the
/// timestamp rounded to the nearest millisecond on the way out, so every millisecond comes back as
/// it went. The largest timestamp, `0xFFFF_FFFF.FFFF_FFFF` s after 1900, is 4,294,967,296 s less
/// 2^-32 s, so 2,085,978,496 s less 2^-32 s after the Unix epoch, which rounds to
/// 2,085,978,496,000 ms. Catches a conversion that loses a millisecond each way, so that a clock
/// set from a server drifts back.
#[test]
fn milliseconds_come_back_as_they_went() {
    for ms in [1, 20, 25, 499, 500, 999, 1_000, 1_000_045] {
        assert_eq!(NtpTimestamp::from_unix_ms(ms).to_unix_ms(ms), ms, "{ms} ms");
    }
    let in_2026 = 1_792_886_400_123;
    assert_eq!(
        NtpTimestamp::from_unix_ms(in_2026).to_unix_ms(in_2026),
        in_2026
    );
    assert_eq!(NtpTimestamp(u64::MAX).to_unix_ms(0), 2_085_978_496_000);
}

/// RFC 4330 §3: the seconds wrap at 2^32 s = 4,294,967,296 s after 1900, 2036-02-07T06:28:16Z,
/// which is 4,294,967,296 - 2,208,988,800 = 2,085,978,496 s after the Unix epoch. Era 1's second
/// 10 is 2,085,978,506,000 ms, and era 0's last second, `0xFFFF_FFFF`, 2,085,978,495,000 ms; each
/// takes the era nearest the caller's time, on either side of the wrap. A time that the nearest
/// era would put before 1970 is taken one era later: 2040-01-01T00:00:00Z, 70 years with 17 leap
/// days after 1970 and so 2,208,988,800 s, is era 1's 2,208,988,800 + 2,208,988,800 -
/// 4,294,967,296 = 123,010,304 s = `0x0754_FD00`, and comes out right even for a clock that reads
/// 1970, to which era 0's 1903 is nearer. Catches a chip that jumps 136 years back at the wrap in
/// 2036, or takes a time after 2038 as one before 1970.
#[test]
fn timestamps_take_the_era_nearest_the_callers_time() {
    let era_1_second_10 = NtpTimestamp(0x0000_000A_0000_0000);
    assert_eq!(
        NtpTimestamp::from_unix_ms(2_085_978_506_000),
        era_1_second_10
    );
    for near in [2_085_978_495_000, 2_085_978_506_000, 1_792_886_400_000, 0] {
        assert_eq!(
            era_1_second_10.to_unix_ms(near),
            2_085_978_506_000,
            "near {near}"
        );
    }
    let era_0_last_second = NtpTimestamp(0xFFFF_FFFF_0000_0000);
    for near in [2_085_978_495_000, 2_085_978_506_000, 1_792_886_400_000, 0] {
        assert_eq!(
            era_0_last_second.to_unix_ms(near),
            2_085_978_495_000,
            "near {near}"
        );
    }
    let new_year_2040 = NtpTimestamp(0x0754_FD00_0000_0000);
    assert_eq!(NtpTimestamp::from_unix_ms(2_208_988_800_000), new_year_2040);
    for near in [0, 1_792_886_400_000, 2_208_988_800_000] {
        assert_eq!(
            new_year_2040.to_unix_ms(near),
            2_208_988_800_000,
            "near {near}"
        );
    }
}

/// RFC 4330 §5, worked by hand: the chip asks at T1 = 1,000,000 ms by its clock ([`SENT`]); the
/// request takes 20 ms to a server whose clock is one hour ahead, which receives it at 1,000,020 +
/// 3,600,000 = 4,600,020 ms (T2, [`RECEIVE`]) and answers 5 ms later, at 4,600,025 ms (T3,
/// [`TRANSMIT`]); the reply takes 20 ms back and arrives at T4 = 1,000,045 ms by the chip's clock.
/// Offset ((T2 - T1) + (T3 - T4)) / 2 = (3,600,020 + 3,599,980) / 2 = 3,600,000 ms; delay (T4 -
/// T1) - (T3 - T2) = 45 - 5 = 40 ms; the server's time at T4, 1,000,045 + 3,600,000 = 4,600,045
/// ms. The same reply with a 20-byte authenticator after its 48 bytes gives the same. Catches an
/// offset or a delay of the wrong sign, not halved, or a millisecond short from the fractions
/// rounded down, and a time set without the reply's way back.
#[test]
fn a_reply_gives_the_offset_and_the_delay() {
    assert_eq!(NtpTimestamp::from_unix_ms(1_000_000), SENT);
    assert_eq!(NtpTimestamp::from_unix_ms(4_600_020), NtpTimestamp(RECEIVE));
    assert_eq!(
        NtpTimestamp::from_unix_ms(4_600_025),
        NtpTimestamp(TRANSMIT)
    );
    let reply = server_reply(0x24, 2, UPSTREAM, SENT.0, RECEIVE, TRANSMIT);
    let expected = SntpTime {
        unix_ms: 4_600_045,
        offset_ms: 3_600_000,
        delay_ms: 40,
        stratum: 2,
    };
    assert_eq!(
        parse_reply(&reply, SENT, 1_000_045, 1_000_045),
        Ok(expected)
    );
    let mut authenticated = reply.to_vec();
    authenticated.extend_from_slice(&[0xAA; 20]);
    assert_eq!(
        parse_reply(&authenticated, SENT, 1_000_045, 1_000_045),
        Ok(expected)
    );
}

/// Specification 34 TARGET-06: a chip whose clock nobody set counts from 1970 at power-up, and its
/// first reply sets it. It asks 5 s after power-up, at T1 = 5,000 ms, 2,208,988,805 s =
/// `0x83AA_7E85`; the server receives the request 20 ms later at 2026-10-25T00:00:00Z, 20,751 days
/// after 1970 (56 years with 14 leap days, and the 297 days of 2026 before 25 October), so
/// 1,792,886,400 s, or 1,792,886,400 + 2,208,988,800 = 4,001,875,200 s = `0xEE87_C500` after 1900,
/// and answers at once (T2 = T3); the reply arrives 20 ms later, at T4 = 5,040 ms. Offset
/// (1,792,886,395,000 + 1,792,886,394,960) / 2 = 1,792,886,394,980 ms, delay 40 ms, and the time at
/// T4 5,040 + 1,792,886,394,980 = 1,792,886,400,020 ms. The firmware's build date, 2026-10-01 =
/// 1,790,812,800,000 ms, is the era's floor. Catches a first time that the 56 years between the
/// clocks overflow or put in another era.
#[test]
fn a_clock_that_reads_1970_takes_the_servers_time() {
    let sent = NtpTimestamp(0x83AA_7E85_0000_0000);
    assert_eq!(NtpTimestamp::from_unix_ms(5_000), sent);
    let server = 0xEE87_C500_0000_0000;
    assert_eq!(
        NtpTimestamp::from_unix_ms(1_792_886_400_000),
        NtpTimestamp(server)
    );
    let reply = server_reply(0x24, 1, *b"GPS\0", sent.0, server, server);
    assert_eq!(
        parse_reply(&reply, sent, 5_040, 1_790_812_800_000),
        Ok(SntpTime {
            unix_ms: 1_792_886_400_020,
            offset_ms: 1_792_886_394_980,
            delay_ms: 40,
            stratum: 1,
        })
    );
}

/// RFC 4330 §5 and §8: a reply is refused when it is shorter than 48 bytes; not a server's reply,
/// mode 4 (a broadcast's mode 5, a client's 3 and a symmetric peer's 1 are not); a kiss-o'-death,
/// stratum 0 with its code in the reference identifier; from a server that is not synchronized,
/// leap indicator 3 or stratum 16 and above; or without a transmit timestamp. A leap second's
/// warning, leap indicator 1 or 2, is no refusal. Catches a clock set from a server that does not
/// know the time, or from bytes that are not a reply.
#[test]
fn replies_that_do_not_give_the_time_are_refused() {
    let reply = |first: u8, stratum: u8, reference: [u8; 4], transmit: u64| {
        server_reply(first, stratum, reference, SENT.0, RECEIVE, transmit)
    };
    assert_eq!(refusal(&reply(0x24, 2, UPSTREAM, TRANSMIT)), None);
    assert_eq!(
        refusal(&reply(0x24, 2, UPSTREAM, TRANSMIT)[..47]),
        Some(SntpError::Short)
    );
    assert_eq!(refusal(&[]), Some(SntpError::Short));
    for first in [0x25, 0x23, 0x21] {
        assert_eq!(
            refusal(&reply(first, 2, UPSTREAM, TRANSMIT)),
            Some(SntpError::NotServer),
            "{first:#04x}"
        );
    }
    assert_eq!(
        refusal(&reply(0x24, 0, *b"RATE", TRANSMIT)),
        Some(SntpError::KissOfDeath(*b"RATE"))
    );
    for (first, stratum) in [(0xE4, 2), (0x24, 16), (0x24, 255)] {
        assert_eq!(
            refusal(&reply(first, stratum, UPSTREAM, TRANSMIT)),
            Some(SntpError::Unsynchronized),
            "{first:#04x} stratum {stratum}"
        );
    }
    for first in [0x64, 0xA4] {
        assert_eq!(
            refusal(&reply(first, 2, UPSTREAM, TRANSMIT)),
            None,
            "{first:#04x}"
        );
    }
    assert_eq!(
        refusal(&reply(0x24, 2, UPSTREAM, 0)),
        Some(SntpError::ZeroTransmit)
    );
}

/// RFC 4330 §5: a reply whose originate timestamp is not the request's transmit timestamp answers
/// another request, an older one, or none, and is refused, even 2^-32 s off. It is checked before
/// the stratum, so that a forged kiss-o'-death does not stop the chip asking. Catches a clock set
/// from a replayed or forged reply.
#[test]
fn a_reply_to_another_request_is_refused() {
    let older = NtpTimestamp::from_unix_ms(999_000).0;
    for originate in [older, SENT.0 + 1, SENT.0 - 1, 0] {
        let reply = server_reply(0x24, 2, UPSTREAM, originate, RECEIVE, TRANSMIT);
        assert_eq!(
            refusal(&reply),
            Some(SntpError::Mismatch),
            "{originate:#018x}"
        );
    }
    let forged = server_reply(0x24, 0, *b"DENY", older, RECEIVE, TRANSMIT);
    assert_eq!(refusal(&forged), Some(SntpError::Mismatch));
}

/// Each refusal says why in words, and a kiss-o'-death its code, a byte that is not printable
/// escaped. Catches a refusal reported without its reason.
#[test]
fn each_refusal_says_why() {
    let texts = [
        (
            SntpError::Short,
            "the time server's reply is shorter than 48 bytes",
        ),
        (
            SntpError::NotServer,
            "the reply is not a time server's reply to a client",
        ),
        (
            SntpError::KissOfDeath(*b"RATE"),
            "the time server refuses the request with the kiss code RATE",
        ),
        (
            SntpError::KissOfDeath([b'X', 0x00, 0xFF, b'Z']),
            "the time server refuses the request with the kiss code X\\x00\\xffZ",
        ),
        (
            SntpError::Unsynchronized,
            "the time server's clock is not synchronized",
        ),
        (
            SntpError::Mismatch,
            "the reply does not answer the request: its originate timestamp is not the request's \
             transmit timestamp",
        ),
        (
            SntpError::ZeroTransmit,
            "the time server's reply has no transmit timestamp",
        ),
    ];
    for (error, text) in texts {
        assert_eq!(error.to_string(), text);
    }
}

/// Clock readings and timestamps at the ends of their ranges give a time held within a `u64` and
/// an offset within an `i64`, never an overflow, which panics in a debug build; the time is the
/// clock's reading plus the offset wherever that sum fits. A timestamp is taken within half an era,
/// 2,147,483,648,000 ms, of `near_unix_ms`, or at `u64::MAX`. Catches arithmetic that a broken or
/// hostile server's reply overflows.
#[test]
fn readings_at_the_ends_of_their_ranges_do_not_overflow() {
    for received in [0, 1, u64::MAX / 2, u64::MAX] {
        for near in [0, u64::MAX] {
            for stamp in [1, u64::MAX] {
                let reply = server_reply(0x24, 1, *b"GPS\0", stamp, stamp, stamp);
                let time = parse_reply(&reply, NtpTimestamp(stamp), received, near)
                    .expect("a server's reply");
                let sum = i128::from(received) + i128::from(time.offset_ms);
                if let Ok(fits) = u64::try_from(sum) {
                    assert_eq!(time.unix_ms, fits, "{received} {near} {stamp}");
                }
            }
        }
    }
    let resolved = NtpTimestamp(u64::MAX).to_unix_ms(u64::MAX);
    assert!(resolved >= u64::MAX - 2_147_483_648_000, "{resolved}");
}
