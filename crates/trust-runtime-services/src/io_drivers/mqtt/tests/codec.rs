//! The MQTT 3.1.1 packets of the client, with every byte worked out from the OASIS standard (MQTT
//! Version 3.1.1, 29 October 2014) and specification 32 §10.

use alloc::format;
use alloc::string::ToString;
use alloc::vec;
use alloc::vec::Vec;

use crate::io_drivers::mqtt::codec::{
    decode, decode_remaining_length, encode_connect, encode_puback, encode_publish,
    encode_remaining_length, encode_subscribe, encode_subscribe_at_least_once, Connect,
    ConnectReturnCode, DecodeError, EncodeError, Incoming, Will, DISCONNECT, PINGREQ,
};

/// The remaining length `length` as the encoder writes it.
fn remaining(length: usize) -> Result<Vec<u8>, EncodeError> {
    let mut out = Vec::new();
    encode_remaining_length(length, &mut out).map(|()| out)
}

/// A CONNECT of client `c1` with a clean session, a keep alive of 5 s and nothing else.
const PLAIN: Connect<'static> = Connect {
    client_id: "c1",
    keep_alive_s: 5,
    clean_session: true,
    will: None,
    username: None,
    password: None,
};

/// The remaining length takes one byte up to 127, two up to 16,383, three up to 2,097,151 and
/// four up to 268,435,455, seven bits a byte with the least significant first (MQTT 3.1.1
/// §2.2.3, Table 2.4), and nothing above. Catches the groups in the wrong order, a boundary off by
/// one, and a length that four bytes cannot hold written anyway.
#[test]
fn the_remaining_length_takes_the_fewest_bytes_at_each_boundary_of_table_2_4() {
    let table: [(usize, &[u8]); 8] = [
        (0, &[0x00]),
        (127, &[0x7F]),
        (128, &[0x80, 0x01]),
        (16_383, &[0xFF, 0x7F]),
        (16_384, &[0x80, 0x80, 0x01]),
        (2_097_151, &[0xFF, 0xFF, 0x7F]),
        (2_097_152, &[0x80, 0x80, 0x80, 0x01]),
        (268_435_455, &[0xFF, 0xFF, 0xFF, 0x7F]),
    ];
    for (length, bytes) in table {
        assert_eq!(remaining(length), Ok(bytes.to_vec()), "{length}");
        assert_eq!(
            decode_remaining_length(bytes),
            Ok(Some((length, bytes.len()))),
            "{length}"
        );
    }
    assert_eq!(
        remaining(268_435_456),
        Err(EncodeError::RemainingLengthTooLarge(268_435_456))
    );
}

/// A remaining length whose fourth byte still continues is refused, as is one in more bytes than
/// its value takes; one whose last byte has not come is incomplete, not wrong (specification 32
/// §10). Catches a fifth length byte read, a non-canonical length taken, and a partial length
/// taken as complete or as malformed.
#[test]
fn an_overlong_or_non_minimal_remaining_length_is_refused_and_a_partial_one_waits() {
    for bytes in [
        &[0xFF, 0xFF, 0xFF, 0xFF][..],
        &[0x80, 0x80, 0x80, 0x80, 0x01],
    ] {
        assert_eq!(
            decode_remaining_length(bytes),
            Err(DecodeError::RemainingLengthTooLong),
            "{bytes:?}"
        );
    }
    for bytes in [&[0x80, 0x00][..], &[0xFF, 0x80, 0x00]] {
        assert_eq!(
            decode_remaining_length(bytes),
            Err(DecodeError::RemainingLengthNotMinimal),
            "{bytes:?}"
        );
    }
    let empty: &[u8] = &[];
    for bytes in [empty, &[0x80], &[0xFF, 0xFF, 0xFF]] {
        assert_eq!(decode_remaining_length(bytes), Ok(None), "{bytes:?}");
    }
}

/// CONNECT carries the protocol name `MQTT`, level 4, the flags, the keep alive and then the
/// client identifier, the will topic and message, the user name and the password, in that order
/// (MQTT 3.1.1 §3.1). The flags are 0x80 user name, 0x40 password, 0x20 will retain, 0x04 will
/// and 0x02 clean session. Catches a flag in the wrong bit, a field out of order, and a keep alive
/// written least significant byte first.
#[test]
fn connect_carries_the_flags_and_the_fields_in_the_order_of_the_standard() {
    assert_eq!(
        encode_connect(&PLAIN),
        Ok(vec![
            0x10, 14, // CONNECT, 10 bytes of variable header and 4 of payload
            0x00, 0x04, b'M', b'Q', b'T', b'T', // protocol name
            0x04, // protocol level
            0x02, // clean session
            0x00, 0x05, // keep alive 5 s
            0x00, 0x02, b'c', b'1', // client identifier
        ])
    );
    let full = Connect {
        keep_alive_s: 300,
        clean_session: false,
        will: Some(Will {
            topic: "w",
            message: &[0xAA],
            retain: true,
            at_least_once: false,
        }),
        username: Some("u"),
        password: Some(b"p"),
        ..PLAIN
    };
    assert_eq!(
        encode_connect(&full),
        Ok(vec![
            0x10, 26, // CONNECT, 10 + 4 + 3 + 3 + 3 + 3 bytes
            0x00, 0x04, b'M', b'Q', b'T', b'T', 0x04, // protocol name and level
            0xE4, // user name, password, will retain, will at QoS 0, no clean session
            0x01, 0x2C, // keep alive 300 s
            0x00, 0x02, b'c', b'1', // client identifier
            0x00, 0x01, b'w', // will topic
            0x00, 0x01, 0xAA, // will message
            0x00, 0x01, b'u', // user name
            0x00, 0x01, b'p', // password
        ])
    );
    // Sparkplug B's will: at quality of service 1 (bits 3 and 4 = 01), not retained, in a clean
    // session: flags 0x02 | 0x04 | 0x08 = 0x0E.
    let sparkplug = Connect {
        will: Some(Will {
            topic: "w",
            message: &[0xAA],
            retain: false,
            at_least_once: true,
        }),
        ..PLAIN
    };
    assert_eq!(encode_connect(&sparkplug).expect("a CONNECT")[9], 0x0E);
}

/// CONNECT refuses what MQTT 3.1.1 forbids: a password without a user name (MQTT-3.1.2-22), an
/// empty client identifier without a clean session (MQTT-3.1.3-7), U+0000 in a string
/// (MQTT-1.5.3-2), a will topic with a wildcard, and a string or binary field beyond 65,535
/// bytes. Catches a packet that a broker must refuse sent anyway, and a length that does not fit
/// its two bytes cut short.
#[test]
fn connect_refuses_what_mqtt_3_1_1_forbids() {
    assert_eq!(
        encode_connect(&Connect {
            password: Some(b"p"),
            ..PLAIN
        }),
        Err(EncodeError::PasswordWithoutUserName)
    );
    assert_eq!(
        encode_connect(&Connect {
            client_id: "",
            clean_session: false,
            ..PLAIN
        }),
        Err(EncodeError::EmptyClientIdWithoutCleanSession)
    );
    assert!(encode_connect(&Connect {
        client_id: "",
        ..PLAIN
    })
    .is_ok());
    assert_eq!(
        encode_connect(&Connect {
            client_id: "a\0b",
            ..PLAIN
        }),
        Err(EncodeError::NullCharacter("client identifier"))
    );
    assert_eq!(
        encode_connect(&Connect {
            will: Some(Will {
                topic: "a/#",
                message: &[],
                retain: false,
                at_least_once: false,
            }),
            ..PLAIN
        }),
        Err(EncodeError::InvalidTopicName("will topic"))
    );
    let long_name = "u".repeat(65_536);
    assert_eq!(
        encode_connect(&Connect {
            username: Some(&long_name),
            ..PLAIN
        }),
        Err(EncodeError::StringTooLong {
            field: "user name",
            length: 65_536,
        })
    );
    let long_password = vec![0_u8; 65_536];
    assert_eq!(
        encode_connect(&Connect {
            username: Some("u"),
            password: Some(&long_password),
            ..PLAIN
        }),
        Err(EncodeError::DataTooLong {
            field: "password",
            length: 65_536,
        })
    );
}

/// A PUBLISH at quality of service 0 is 0x30, the remaining length, the topic name and the
/// message, without a packet identifier (MQTT 3.1.1 §3.3); 129 remaining bytes take the two
/// length bytes 0x81 0x01. SUBSCRIBE is 0x82, the packet identifier, the filter and the requested
/// quality of service 0 (§3.8); PINGREQ and DISCONNECT are their fixed headers. Catches a packet
/// identifier written at quality of service 0, a wrong first byte, a SUBSCRIBE without its
/// reserved flag 0x02, and a one-byte length beyond 127.
#[test]
fn publish_subscribe_pingreq_and_disconnect_are_laid_out_as_the_standard_says() {
    assert_eq!(
        encode_publish("a/b", &[1, 2, 3]),
        Ok(vec![0x30, 0x08, 0x00, 0x03, b'a', b'/', b'b', 1, 2, 3])
    );
    assert_eq!(
        encode_publish("t", &[]),
        Ok(vec![0x30, 0x03, 0x00, 0x01, b't'])
    );
    let long = encode_publish("t", &[7; 126]).expect("129 remaining bytes");
    assert_eq!(long.len(), 132);
    assert_eq!(long[..5], [0x30, 0x81, 0x01, 0x00, 0x01]);
    assert_eq!(
        encode_subscribe(1, "a/b"),
        Ok(vec![
            0x82, 0x08, 0x00, 0x01, 0x00, 0x03, b'a', b'/', b'b', 0x00
        ])
    );
    // Sparkplug B's NCMD subscription asks for quality of service 1, its last byte.
    assert_eq!(
        encode_subscribe_at_least_once(1, "a/b"),
        Ok(vec![
            0x82, 0x08, 0x00, 0x01, 0x00, 0x03, b'a', b'/', b'b', 0x01
        ])
    );
    assert_eq!(encode_puback(0x1234), [0x40, 0x02, 0x12, 0x34]);
    assert_eq!(encode_subscribe(0, "a/b"), Err(EncodeError::ZeroPacketId));
    assert_eq!(encode_subscribe(1, ""), Err(EncodeError::EmptyTopicFilter));
    for topic in ["a/+", "#", ""] {
        assert_eq!(
            encode_publish(topic, &[]),
            Err(EncodeError::InvalidTopicName("topic name")),
            "{topic}"
        );
    }
    assert_eq!(PINGREQ, [0xC0, 0x00]);
    assert_eq!(DISCONNECT, [0xE0, 0x00]);
}

/// The decoder reads the packets a broker sends: CONNACK with its session present flag and each
/// return code of Table 3.1, a retained PUBLISH, a SUBACK with a failure code, and PINGRESP; of
/// two packets in one buffer it gives the first and its length. Catches the flags or the return
/// code read from the wrong byte, the retain flag lost, and a packet that swallows the next.
#[test]
fn the_decoder_reads_the_packets_a_broker_sends() {
    assert_eq!(
        decode(&[0x20, 0x02, 0x00, 0x00], 10),
        Ok(Some((
            Incoming::ConnAck {
                session_present: false,
                return_code: ConnectReturnCode::Accepted,
            },
            4
        )))
    );
    assert_eq!(
        decode(&[0x20, 0x02, 0x01, 0x00], 10),
        Ok(Some((
            Incoming::ConnAck {
                session_present: true,
                return_code: ConnectReturnCode::Accepted,
            },
            4
        )))
    );
    for (code, return_code) in [
        (1, ConnectReturnCode::UnacceptableProtocolVersion),
        (2, ConnectReturnCode::IdentifierRejected),
        (3, ConnectReturnCode::ServerUnavailable),
        (4, ConnectReturnCode::BadUserNameOrPassword),
        (5, ConnectReturnCode::NotAuthorized),
    ] {
        assert_eq!(
            decode(&[0x20, 0x02, 0x00, code], 10),
            Ok(Some((
                Incoming::ConnAck {
                    session_present: false,
                    return_code,
                },
                4
            )))
        );
        assert_eq!(return_code.code(), code);
    }
    assert_eq!(
        decode(&[0x31, 0x05, 0x00, 0x01, b't', 0x07, 0x08], 10),
        Ok(Some((
            Incoming::Publish {
                topic: "t",
                payload: &[0x07, 0x08],
                retain: true,
                packet_id: None,
            },
            7
        )))
    );
    // At quality of service 1, here with DUP (flags 0x0A), the packet identifier 0x1234 follows
    // the topic name, and the rest is the message.
    assert_eq!(
        decode(&[0x3A, 0x07, 0x00, 0x01, b't', 0x12, 0x34, 0x07, 0x08], 10),
        Ok(Some((
            Incoming::Publish {
                topic: "t",
                payload: &[0x07, 0x08],
                retain: false,
                packet_id: Some(0x1234),
            },
            9
        )))
    );
    assert_eq!(
        decode(&[0x90, 0x03, 0x00, 0x01, 0x80], 10),
        Ok(Some((
            Incoming::SubAck {
                packet_id: 1,
                return_codes: &[0x80],
            },
            5
        )))
    );
    assert_eq!(
        decode(&[0xD0, 0x00, 0x20, 0x02, 0x00, 0x00], 10),
        Ok(Some((Incoming::PingResp, 2)))
    );
}

/// Every proper prefix of a packet is incomplete, and the whole packet decodes. Catches a packet
/// decoded before its last byte has come, which would read a payload short.
#[test]
fn the_decoder_waits_for_a_whole_packet() {
    let packet = [0x30, 0x05, 0x00, 0x01, b't', 0x07, 0x08];
    for end in 0..packet.len() {
        assert_eq!(decode(&packet[..end], 10), Ok(None), "{end} bytes");
    }
    assert!(matches!(decode(&packet, 10), Ok(Some((_, 7)))));
}

/// The decoder refuses each malformed packet with its own error (MQTT 3.1.1 §2 and §3,
/// specification 32 §10), the fixed header as soon as its first byte has come, and a remaining
/// length above the limit before the rest arrives: 0x30 0x81 0x50 announces 1 + 80 × 128 =
/// 10,241 bytes. Catches a malformed packet taken as data, a reserved flag ignored, a PUBLISH at
/// quality of service 2, which the client never asks for, a PUBLISH at 1 without its packet
/// identifier or with identifier 0, and a client that buffers a packet larger than its limit.
#[test]
fn the_decoder_refuses_malformed_packets_with_typed_errors() {
    let cases: &[(&[u8], DecodeError)] = &[
        (&[0x00, 0x00], DecodeError::ReservedPacketType(0)),
        (&[0xF0, 0x00], DecodeError::ReservedPacketType(15)),
        (&[0x10], DecodeError::UnexpectedPacket("CONNECT")),
        (
            &[0x40, 0x02, 0x00, 0x01],
            DecodeError::UnexpectedPacket("PUBACK"),
        ),
        (&[0xE0, 0x00], DecodeError::UnexpectedPacket("DISCONNECT")),
        (
            &[0x21, 0x02, 0x00, 0x00],
            DecodeError::InvalidFlags {
                packet: "CONNACK",
                flags: 0x01,
            },
        ),
        (
            &[0x20, 0x03, 0x00, 0x00, 0x00],
            DecodeError::InvalidLength {
                packet: "CONNACK",
                found: 3,
                expected: 2,
            },
        ),
        (
            &[0x20, 0x02, 0x02, 0x00],
            DecodeError::ConnAckReservedFlags(0x02),
        ),
        (&[0x20, 0x02, 0x00, 0x06], DecodeError::UnknownReturnCode(6)),
        (
            &[0x20, 0x02, 0x01, 0x05],
            DecodeError::SessionPresentOnRefusal(5),
        ),
        (
            &[0x34, 0x05, 0x00, 0x01, b't', 0x00, 0x01],
            DecodeError::UnexpectedQos(2),
        ),
        (
            &[0x32, 0x05, 0x00, 0x01, b't', 0x00, 0x00],
            DecodeError::ZeroPacketId,
        ),
        (
            &[0x32, 0x04, 0x00, 0x01, b't', 0x00],
            DecodeError::FieldPastEnd("PUBLISH"),
        ),
        (
            &[0x36, 0x03, 0x00, 0x01, b't'],
            DecodeError::InvalidFlags {
                packet: "PUBLISH",
                flags: 0x06,
            },
        ),
        (
            &[0x38, 0x03, 0x00, 0x01, b't'],
            DecodeError::InvalidFlags {
                packet: "PUBLISH",
                flags: 0x08,
            },
        ),
        (&[0x30, 0x01, 0x00], DecodeError::FieldPastEnd("PUBLISH")),
        (
            &[0x30, 0x03, 0x00, 0x02, b't'],
            DecodeError::FieldPastEnd("PUBLISH"),
        ),
        (&[0x30, 0x02, 0x00, 0x00], DecodeError::EmptyTopicName),
        (
            &[0x30, 0x03, 0x00, 0x01, b'#'],
            DecodeError::WildcardInTopicName,
        ),
        (&[0x30, 0x03, 0x00, 0x01, 0xFF], DecodeError::InvalidUtf8),
        (&[0x30, 0x03, 0x00, 0x01, 0x00], DecodeError::NullCharacter),
        (
            &[0x90, 0x02, 0x00, 0x01],
            DecodeError::FieldPastEnd("SUBACK"),
        ),
        (
            &[0x90, 0x03, 0x00, 0x01, 0x03],
            DecodeError::UnknownSubAckCode(0x03),
        ),
        (
            &[0xD0, 0x01, 0x00],
            DecodeError::InvalidLength {
                packet: "PINGRESP",
                found: 1,
                expected: 0,
            },
        ),
        (
            &[0x30, 0x81, 0x50],
            DecodeError::PacketTooLarge {
                remaining: 10_241,
                limit: 10_240,
            },
        ),
        (&[0x30, 0x80, 0x00], DecodeError::RemainingLengthNotMinimal),
    ];
    for (bytes, error) in cases {
        assert_eq!(decode(bytes, 10_240), Err(*error), "{bytes:?}");
    }
}

/// The texts a session failure shows: a CONNACK return code with its words and number (MQTT
/// 3.1.1 Table 3.1), and a decoder error. Catches a return code shown without its number or with
/// the words of another code.
#[test]
fn return_codes_and_decoder_errors_read_as_words() {
    assert_eq!(ConnectReturnCode::from_code(6), None);
    assert_eq!(
        ConnectReturnCode::BadUserNameOrPassword.to_string(),
        "bad user name or password (return code 4)"
    );
    assert_eq!(
        ConnectReturnCode::NotAuthorized.to_string(),
        "not authorized (return code 5)"
    );
    assert_eq!(
        DecodeError::UnexpectedPacket("PUBACK").to_string(),
        "unexpected PUBACK"
    );
    assert_eq!(
        DecodeError::UnexpectedQos(2).to_string(),
        "PUBLISH at QoS 2, above the QoS 1 the client subscribes at"
    );
    assert_eq!(
        DecodeError::ZeroPacketId.to_string(),
        "PUBLISH at QoS 1 with packet identifier 0"
    );
}

/// The `Debug` of a CONNECT never shows its password (specification 32 §3). Catches a password
/// written to a log through the packet's `Debug`.
#[test]
fn the_debug_of_a_connect_hides_its_password() {
    let connect = Connect {
        username: Some("plc"),
        password: Some(b"hunter2"),
        ..PLAIN
    };
    let shown = format!("{connect:?}");
    assert!(!shown.contains("hunter2"), "{shown}");
    assert!(shown.contains("<redacted>"), "{shown}");
}
