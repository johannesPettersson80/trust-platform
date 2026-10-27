//! The chip's mDNS responder against RFC 6762 and RFC 6763. The replies about [`SHORT`] are worked
//! out byte by byte: a name is each label after its length byte and then the root's zero (RFC 1035
//! §3.1), a record its name, type, class, time to live, data length and data (§4.1.3), and a
//! header its ID, flags and four counts (§4.1.1), every word high byte first.

use alloc::string::String;
use alloc::vec::Vec;

use super::*;

/// The announcement whose replies are worked out byte by byte: instance `a`, host `h`,
/// 192.168.1.50, port 8080 and the one property `id=r1`.
const SHORT: Announcement<'static> = Announcement {
    instance: "a",
    host: "h",
    address: [192, 168, 1, 50],
    port: 8080,
    txt: &[("id", "r1")],
};

/// A browse's query (RFC 6762 §5.2): ID 0, one question, `_trust._plc._tcp.local` PTR IN.
const BROWSE: &[u8] = &[
    0x00, 0x00, // ID 0
    0x00, 0x00, // a standard query
    0x00, 0x01, // one question
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // no answer, authority or additional records
    6, b'_', b't', b'r', b'u', b's', b't', // _trust
    4, b'_', b'p', b'l', b'c', // _plc
    4, b'_', b't', b'c', b'p', // _tcp
    5, b'l', b'o', b'c', b'a', b'l', 0, // local and the root
    0x00, 0x0C, // type PTR, 12
    0x00, 0x01, // class IN, the unicast-response bit clear
];

/// `_trust._plc._tcp.local` PTR `a._trust._plc._tcp.local`: shared, so class IN without the
/// cache-flush bit (RFC 6762 §10.2), 4,500 s = `0x1194` (§10), 26 bytes of data; 60 bytes.
const PTR_RECORD: &[u8] = &[
    6, b'_', b't', b'r', b'u', b's', b't', 4, b'_', b'p', b'l', b'c', 4, b'_', b't', b'c', b'p', 5,
    b'l', b'o', b'c', b'a', b'l', 0, // _trust._plc._tcp.local
    0x00, 0x0C, // type PTR, 12
    0x00, 0x01, // class IN
    0x00, 0x00, 0x11, 0x94, // 4,500 s
    0x00, 0x1A, // 26 bytes
    1, b'a', 6, b'_', b't', b'r', b'u', b's', b't', 4, b'_', b'p', b'l', b'c', 4, b'_', b't', b'c',
    b'p', 5, b'l', b'o', b'c', b'a', b'l', 0, // a._trust._plc._tcp.local
];

/// The SRV record of `a._trust._plc._tcp.local`: unique, so with the cache-flush bit, `0x8001`;
/// 120 s = `0x78`; priority 0, weight 0, port 8080 = `0x1F90` and `h.local`, 15 bytes of data; 51
/// bytes.
const SRV_RECORD: &[u8] = &[
    1, b'a', 6, b'_', b't', b'r', b'u', b's', b't', 4, b'_', b'p', b'l', b'c', 4, b'_', b't', b'c',
    b'p', 5, b'l', b'o', b'c', b'a', b'l', 0, // a._trust._plc._tcp.local
    0x00, 0x21, // type SRV, 33
    0x80, 0x01, // class IN with the cache-flush bit
    0x00, 0x00, 0x00, 0x78, // 120 s
    0x00, 0x0F, // 15 bytes
    0x00, 0x00, // priority 0
    0x00, 0x00, // weight 0
    0x1F, 0x90, // port 8080
    1, b'h', 5, b'l', b'o', b'c', b'a', b'l', 0, // h.local
];

/// The TXT record of `a._trust._plc._tcp.local`: unique, 4,500 s, the one string `id=r1` after
/// its length 5, 6 bytes of data; 42 bytes.
const TXT_RECORD: &[u8] = &[
    1, b'a', 6, b'_', b't', b'r', b'u', b's', b't', 4, b'_', b'p', b'l', b'c', 4, b'_', b't', b'c',
    b'p', 5, b'l', b'o', b'c', b'a', b'l', 0, // a._trust._plc._tcp.local
    0x00, 0x10, // type TXT, 16
    0x80, 0x01, // class IN with the cache-flush bit
    0x00, 0x00, 0x11, 0x94, // 4,500 s
    0x00, 0x06, // 6 bytes
    5, b'i', b'd', b'=', b'r', b'1', // id=r1
];

/// The A record of `h.local`: unique, 120 s, 192.168.1.50; 23 bytes.
const A_RECORD: &[u8] = &[
    1, b'h', 5, b'l', b'o', b'c', b'a', b'l', 0, // h.local
    0x00, 0x01, // type A, 1
    0x80, 0x01, // class IN with the cache-flush bit
    0x00, 0x00, 0x00, 0x78, // 120 s
    0x00, 0x04, // 4 bytes
    192, 168, 1, 50,
];

/// `_services._dns-sd._udp.local` PTR `_trust._plc._tcp.local` (RFC 6763 §9): shared, 4,500 s, 24
/// bytes of data; 64 bytes.
const TYPE_PTR_RECORD: &[u8] = &[
    9, b'_', b's', b'e', b'r', b'v', b'i', b'c', b'e', b's', // _services
    7, b'_', b'd', b'n', b's', b'-', b's', b'd', // _dns-sd
    4, b'_', b'u', b'd', b'p', // _udp
    5, b'l', b'o', b'c', b'a', b'l', 0, // local and the root
    0x00, 0x0C, // type PTR, 12
    0x00, 0x01, // class IN
    0x00, 0x00, 0x11, 0x94, // 4,500 s
    0x00, 0x18, // 24 bytes
    6, b'_', b't', b'r', b'u', b's', b't', 4, b'_', b'p', b'l', b'c', 4, b'_', b't', b'c', b'p', 5,
    b'l', b'o', b'c', b'a', b'l', 0, // _trust._plc._tcp.local
];

/// The header of a reply to the group: ID 0, flags `0x8400`, QR and AA (RFC 6762 §18.2, §18.4),
/// no questions (§6), `answers` answers, no authority records and `additionals` additional
/// records.
fn header(answers: u8, additionals: u8) -> [u8; 12] {
    [0, 0, 0x84, 0x00, 0, 0, 0, answers, 0, 0, 0, additionals]
}

/// `parts`, one after the other.
fn joined(parts: &[&[u8]]) -> Vec<u8> {
    parts.concat()
}

/// A query with `id` and `flags` that asks `questions`, each a dotted name with its type and its
/// class, and has no other records.
fn query_for(id: u16, flags: u16, questions: &[(&str, u16, u16)]) -> Vec<u8> {
    let count = u16::try_from(questions.len()).expect("a few questions");
    let mut query = Vec::new();
    for word in [id, flags, count, 0, 0, 0] {
        query.extend_from_slice(&word.to_be_bytes());
    }
    for (name, kind, class) in questions {
        for label in name.split('.') {
            query.push(u8::try_from(label.len()).expect("a short label"));
            query.extend_from_slice(label.as_bytes());
        }
        query.push(0);
        query.extend_from_slice(&kind.to_be_bytes());
        query.extend_from_slice(&class.to_be_bytes());
    }
    query
}

/// The reply to `query` from `source_port` about `announcement`, and where it goes.
fn reply_about(
    announcement: &Announcement<'_>,
    query: &[u8],
    source_port: u16,
) -> Option<(Reply, Vec<u8>)> {
    let mut buf = [0_u8; 1500];
    let reply = answer(query, source_port, announcement, &mut buf)?;
    let (Reply::Multicast(len) | Reply::Unicast(len)) = reply;
    Some((reply, buf[..len].to_vec()))
}

/// The reply to `query` from `source_port` about [`SHORT`], and where it goes.
fn reply_to(query: &[u8], source_port: u16) -> Option<(Reply, Vec<u8>)> {
    reply_about(&SHORT, query, source_port)
}

/// The bytes of the reply to `query` from port 5353 about [`SHORT`].
fn bytes_of(query: &[u8]) -> Option<Vec<u8>> {
    reply_to(query, MDNS_PORT).map(|(_, bytes)| bytes)
}

/// One record of a reply, read back.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Resource {
    name: String,
    kind: u16,
    class: u16,
    ttl: u32,
    data: Vec<u8>,
}

/// A reply, read back.
#[derive(Debug)]
struct Decoded {
    id: u16,
    flags: u16,
    questions: Vec<(String, u16, u16)>,
    answers: Vec<Resource>,
    additionals: Vec<Resource>,
}

/// The 16-bit word at `at`, high byte first, and `at` moved past it.
fn word(bytes: &[u8], at: &mut usize) -> u16 {
    let value = u16::from_be_bytes([bytes[*at], bytes[*at + 1]]);
    *at += 2;
    value
}

/// The uncompressed name at `at`, dotted, and `at` moved past it; the test fails on a pointer.
fn dotted(bytes: &[u8], at: &mut usize) -> String {
    let mut labels = Vec::new();
    loop {
        let length = usize::from(bytes[*at]);
        *at += 1;
        if length == 0 {
            return labels.join(".");
        }
        assert!(length <= 63, "a label, not a pointer");
        labels.push(core::str::from_utf8(&bytes[*at..*at + length]).expect("a text label"));
        *at += length;
    }
}

/// The record at `at`, and `at` moved past it.
fn resource(bytes: &[u8], at: &mut usize) -> Resource {
    let name = dotted(bytes, at);
    let kind = word(bytes, at);
    let class = word(bytes, at);
    let ttl = (u32::from(word(bytes, at)) << 16) | u32::from(word(bytes, at));
    let length = usize::from(word(bytes, at));
    let data = bytes[*at..*at + length].to_vec();
    *at += length;
    Resource {
        name,
        kind,
        class,
        ttl,
        data,
    }
}

/// Reads `bytes` as a reply with no authority records whose names are not compressed, as the
/// responder writes them; the test fails on anything else, or on bytes after the last record.
fn decode(bytes: &[u8]) -> Decoded {
    let mut at = 0;
    let id = word(bytes, &mut at);
    let flags = word(bytes, &mut at);
    let question_count = word(bytes, &mut at);
    let answer_count = word(bytes, &mut at);
    assert_eq!(word(bytes, &mut at), 0, "no authority records");
    let additional_count = word(bytes, &mut at);
    let mut questions = Vec::new();
    for _ in 0..question_count {
        let name = dotted(bytes, &mut at);
        let kind = word(bytes, &mut at);
        let class = word(bytes, &mut at);
        questions.push((name, kind, class));
    }
    let answers = (0..answer_count)
        .map(|_| resource(bytes, &mut at))
        .collect();
    let additionals = (0..additional_count)
        .map(|_| resource(bytes, &mut at))
        .collect();
    assert_eq!(at, bytes.len(), "the counts cover the whole reply");
    Decoded {
        id,
        flags,
        questions,
        answers,
        additionals,
    }
}

/// A query whose first question is `_trust._plc._tcp.local` PTR IN, and each of the `pointers`
/// questions after it a compression pointer to the name of the question before it, so that the
/// last question's name is reached through `pointers` pointers.
fn chain(pointers: usize) -> Vec<u8> {
    let mut query = query_for(0, 0, &[(SERVICE, 12, 1)]);
    let mut previous: u16 = 12;
    for _ in 0..pointers {
        let here = u16::try_from(query.len()).expect("a short query");
        query.extend_from_slice(&(0xC000 | previous).to_be_bytes());
        query.extend_from_slice(&[0x00, 0x0C, 0x00, 0x01]);
        previous = here;
    }
    let count = u16::try_from(pointers + 1).expect("a few questions");
    query[4..6].copy_from_slice(&count.to_be_bytes());
    query
}

/// A query of one question, `h.local` A IN, after a question for a name of labels of `sizes`
/// bytes of `x`.
fn after_long_name(sizes: &[u8]) -> Vec<u8> {
    let mut query = query_for(0, 0, &[]);
    query[5] = 2;
    for &size in sizes {
        query.push(size);
        query.extend(core::iter::repeat_n(b'x', usize::from(size)));
    }
    query.extend_from_slice(&[0, 0x00, 0x01, 0x00, 0x01]);
    query.extend_from_slice(&query_for(0, 0, &[("h.local", 1, 1)])[12..]);
    query
}

/// RFC 6762 §6 and RFC 6763 §12.1: a browse from port 5353 is answered to the group, with ID 0,
/// flags QR and AA and no questions (RFC 6762 §18), the PTR record of the instance as its answer,
/// and as additional records the SRV, TXT and A records that the PTR record leads to, so that the
/// browser resolves the chip without asking again. The shared PTR record has no cache-flush bit and
/// the unique records have it (§10.2); the SRV and A records name the host and live 120 s, the
/// others 4,500 s (§10). Catches any wrong byte of the reply: a browser that cannot read it, has
/// to ask again, keeps a stale record or drops the chip early.
#[test]
fn a_browse_is_answered_to_the_group_with_the_instance_and_its_records() {
    let expected = joined(&[&header(1, 3), PTR_RECORD, SRV_RECORD, TXT_RECORD, A_RECORD]);
    assert_eq!(expected.len(), 188, "12 + 60 + 51 + 42 + 23 bytes");
    assert_eq!(
        reply_to(BROWSE, MDNS_PORT),
        Some((Reply::Multicast(188), expected))
    );
}

/// RFC 6762 §5.4 and §18.1: a question with the unicast-response bit, the top bit of its class, is
/// answered to the asker alone with the query's ID. When a query also asks a question without the
/// bit that is answered, the reply goes to the group, which the asker hears too, with ID 0.
/// Catches a querier that asked for a unicast reply and gets none, and the other queriers of a
/// mixed query left without the reply.
#[test]
fn a_question_with_the_unicast_response_bit_is_answered_to_the_asker() {
    let unicast = query_for(0x1234, 0, &[(SERVICE, 12, 0x8001)]);
    let mut expected = joined(&[&header(1, 3), PTR_RECORD, SRV_RECORD, TXT_RECORD, A_RECORD]);
    expected[..2].copy_from_slice(&[0x12, 0x34]);
    assert_eq!(
        reply_to(&unicast, MDNS_PORT),
        Some((Reply::Unicast(188), expected))
    );

    let mixed = query_for(0x1234, 0, &[(SERVICE, 12, 0x8001), ("h.local", 1, 1)]);
    let (reply, bytes) = reply_to(&mixed, MDNS_PORT).expect("answered");
    assert!(matches!(reply, Reply::Multicast(_)), "{reply:?}");
    let decoded = decode(&bytes);
    assert_eq!(decoded.id, 0);
    let kinds = |records: &[Resource]| records.iter().map(|record| record.kind).collect::<Vec<_>>();
    assert_eq!(kinds(&decoded.answers), [12, 1], "the PTR and A records");
    assert_eq!(
        kinds(&decoded.additionals),
        [33, 16],
        "the SRV and TXT records"
    );

    let other = query_for(0, 0, &[(SERVICE, 12, 0x8001), ("_http._tcp.local", 12, 1)]);
    let (reply, _) = reply_to(&other, MDNS_PORT).expect("answered");
    assert!(matches!(reply, Reply::Unicast(_)), "{reply:?}");
}

/// RFC 6762 §6.7: a query from a port other than 5353 comes from a simple resolver, which gets a
/// conventional reply to its port: the query's ID and question repeated, recursion desired copied
/// from the query (RFC 1035 §4.1.1), no cache-flush bit and no time to live above 10 s. Catches a
/// resolver that gets no reply it can match to its query, or caches the chip's records for 75
/// minutes with no means of learning that they changed.
#[test]
fn a_legacy_query_gets_a_conventional_reply_to_its_port() {
    // `h.local` A IN from port 49152 with ID 0xBEEF and recursion desired, as `dig` asks.
    let query = query_for(0xBEEF, 0x0100, &[("h.local", 1, 1)]);
    let expected: &[u8] = &[
        0xBE, 0xEF, // the query's ID
        0x85, 0x00, // QR, AA and RD
        0x00, 0x01, // one question
        0x00, 0x01, // one answer
        0x00, 0x00, 0x00, 0x00, // no authority or additional records
        1, b'h', 5, b'l', b'o', b'c', b'a', b'l', 0, // h.local
        0x00, 0x01, 0x00, 0x01, // A IN: the question repeated
        1, b'h', 5, b'l', b'o', b'c', b'a', b'l', 0, // h.local
        0x00, 0x01, // type A
        0x00, 0x01, // class IN without the cache-flush bit
        0x00, 0x00, 0x00, 0x0A, // 10 s
        0x00, 0x04, // 4 bytes
        192, 168, 1, 50,
    ];
    assert_eq!(
        reply_to(&query, 49_152),
        Some((Reply::Unicast(48), expected.to_vec()))
    );

    let browse = query_for(7, 0, &[(SERVICE, 12, 1)]);
    let (reply, bytes) = reply_to(&browse, 49_152).expect("answered");
    assert!(matches!(reply, Reply::Unicast(_)), "{reply:?}");
    let decoded = decode(&bytes);
    assert_eq!((decoded.id, decoded.flags), (7, 0x8400));
    assert_eq!(decoded.questions, [(String::from(SERVICE), 12, 1)]);
    assert_eq!(decoded.answers.len(), 1);
    assert_eq!(decoded.answers[0].name, SERVICE);
    assert_eq!(decoded.additionals.len(), 3);
    for record in decoded.answers.iter().chain(&decoded.additionals) {
        assert_eq!((record.class, record.ttl), (1, 10), "{record:?}");
    }
}

/// RFC 1035 §2.3.3: names are compared without regard to ASCII case, and a reply names the records
/// as the chip writes them. Catches a browser or resolver that writes a name in another case and
/// never finds the chip.
#[test]
fn names_are_compared_without_regard_to_case() {
    let shouting = query_for(0, 0, &[("_TRUST._PLC._TCP.LOCAL", 12, 1)]);
    assert!(reply_to(BROWSE, MDNS_PORT).is_some());
    assert_eq!(reply_to(&shouting, MDNS_PORT), reply_to(BROWSE, MDNS_PORT));
    assert_eq!(
        bytes_of(&query_for(0, 0, &[("H.Local", 1, 1)])),
        Some(joined(&[&header(1, 0), A_RECORD]))
    );
    assert_eq!(
        bytes_of(&query_for(0, 0, &[("A._Trust._PLC._tcp.LOCAL", 16, 1)])),
        Some(joined(&[&header(1, 0), TXT_RECORD]))
    );
}

/// RFC 6762 §6 and §6.5: the host's A question is answered with its A record, unique and so with
/// the cache-flush bit, 120 s, and so are its ANY question and its A question of class ANY. The
/// chip has no IPv6 address, so the AAAA question gets nothing. Catches a browser that found the
/// instance but cannot reach its host.
#[test]
fn the_host_name_gives_the_chips_address() {
    let expected = joined(&[&header(1, 0), A_RECORD]);
    assert_eq!(expected.len(), 35, "12 + 23 bytes");
    for (kind, class) in [(1, 1), (255, 1), (1, 255), (1, 0x8001)] {
        let query = query_for(0, 0, &[("h.local", kind, class)]);
        let reply = if class == 0x8001 {
            Reply::Unicast(35)
        } else {
            Reply::Multicast(35)
        };
        assert_eq!(
            reply_to(&query, MDNS_PORT),
            Some((reply, expected.clone())),
            "type {kind} class {class:#06x}"
        );
    }
    assert_eq!(bytes_of(&query_for(0, 0, &[("h.local", 28, 1)])), None);
}

/// RFC 6763 §12.2, §12.3 and §9: the instance's SRV question is answered with the SRV record and
/// the host's A record as an additional record, its TXT question with the TXT record alone, its
/// ANY question with both and the A record; the service type enumeration's PTR question gets the
/// PTR record that names the service type. Catches a resolver that has to ask again for the
/// address, and a browser of service types that never sees truST.
#[test]
fn the_instance_and_the_service_type_are_answered_with_their_records() {
    let instance = "a._trust._plc._tcp.local";
    let cases: [(u16, Vec<u8>); 3] = [
        (33, joined(&[&header(1, 1), SRV_RECORD, A_RECORD])),
        (16, joined(&[&header(1, 0), TXT_RECORD])),
        (
            255,
            joined(&[&header(2, 1), SRV_RECORD, TXT_RECORD, A_RECORD]),
        ),
    ];
    for (kind, expected) in cases {
        let query = query_for(0, 0, &[(instance, kind, 1)]);
        assert_eq!(bytes_of(&query), Some(expected), "type {kind}");
    }
    let types = query_for(0, 0, &[("_services._dns-sd._udp.local", 12, 1)]);
    let expected = joined(&[&header(1, 0), TYPE_PTR_RECORD]);
    assert_eq!(expected.len(), 76, "12 + 64 bytes");
    assert_eq!(bytes_of(&types), Some(expected));
}

/// RFC 6762 §6: a responder answers for its own records only. A question for another service,
/// another instance, another host, a type that the name has no record of, or a class other than
/// IN or ANY gets no reply. Catches a chip that answers for names that are not its own.
#[test]
fn questions_about_other_names_get_no_reply() {
    for (name, kind, class) in [
        ("_http._tcp.local", 12, 1),
        ("b._trust._plc._tcp.local", 33, 1),
        ("a._trust._plc._tcp", 33, 1),
        ("aa._trust._plc._tcp.local", 33, 1),
        ("other.local", 1, 1),
        ("h.local.local", 1, 1),
        ("h", 1, 1),
        (SERVICE, 33, 1),
        ("a._trust._plc._tcp.local", 1, 1),
        ("h.local", 16, 1),
        ("h.local", 1, 3),
    ] {
        let query = query_for(0, 0, &[(name, kind, class)]);
        assert_eq!(bytes_of(&query), None, "{name} type {kind} class {class}");
    }
}

/// RFC 6762 §18.2, §18.3 and §18.11: a response (QR set), and a message whose OPCODE or RCODE is
/// not 0, is ignored, even when it asks for the chip's records; the flags of a query that it may
/// carry, recursion desired and truncated, change nothing. Catches a chip that answers another
/// responder's reply, so that two responders answer each other without end.
#[test]
fn responses_and_other_opcodes_get_no_reply() {
    for flags in [0x8000, 0x8400, 0x0800, 0x2000, 0x0003] {
        let message = query_for(0, flags, &[(SERVICE, 12, 1)]);
        assert_eq!(bytes_of(&message), None, "flags {flags:#06x}");
    }
    for flags in [0x0100, 0x0200] {
        let query = query_for(0, flags, &[(SERVICE, 12, 1)]);
        assert_eq!(bytes_of(&query), bytes_of(BROWSE), "flags {flags:#06x}");
    }
}

/// RFC 1035 §2.3.4 and §4.1.4: a malformed query is ignored, never read past its end: a header or
/// a question cut short at any byte, a label running past the end, a label of a reserved type, a
/// name of more than 255 bytes, and compression pointers that point at themselves, at their own
/// name, forward or into the header, or through more than 16 hops. A name of 255 bytes and 16 hops
/// are read. Catches a responder that panics, loops, or reads beyond the datagram on a hostile
/// packet.
#[test]
fn malformed_queries_are_ignored() {
    for end in 0..BROWSE.len() {
        assert_eq!(bytes_of(&BROWSE[..end]), None, "{end} bytes");
    }
    let mut two = BROWSE.to_vec();
    two[5] = 2;
    assert_eq!(bytes_of(&two), None, "a second question missing");
    for name in [
        &[63, b'_', b't', 0][..],
        &[0x40, b'a', 0][..],
        &[0x80, b'a', 0][..],
        &[0xC0, 0x0C][..],
        &[1, b'a', 0xC0, 0x0C][..],
        &[0xC0, 0x0E, 1, b'a', 0][..],
        &[0xC0, 0x00][..],
    ] {
        let mut query = BROWSE[..12].to_vec();
        query.extend_from_slice(name);
        query.extend_from_slice(&[0x00, 0x0C, 0x00, 0x01]);
        assert_eq!(bytes_of(&query), None, "{name:02x?}");
    }
    // 4 labels of 63, 63, 63 and 61 bytes, each after its length byte, and the root: 255 bytes.
    let host = Some(joined(&[&header(1, 0), A_RECORD]));
    assert_eq!(bytes_of(&after_long_name(&[63, 63, 63, 61])), host);
    assert_eq!(bytes_of(&after_long_name(&[63, 63, 63, 62])), None);
    assert!(bytes_of(&chain(16)).is_some(), "16 hops are followed");
    assert_eq!(bytes_of(&chain(17)), None, "17 hops are not");
}

/// RFC 6763 §6.1 and §6.3: each property is one string, `key=value` after its length byte, and no
/// property is a single empty string, one zero byte. Catches properties that a browser reads as
/// other keys or values, such as `auth` lost, so that a tool talks to the chip without its token.
#[test]
fn txt_properties_are_length_prefixed_strings() {
    let instance = "a._trust._plc._tcp.local";
    let query = query_for(0, 0, &[(instance, 16, 1)]);
    let two = Announcement {
        txt: &[("id", "r1"), ("auth", "token")],
        ..SHORT
    };
    let (_, bytes) = reply_about(&two, &query, MDNS_PORT).expect("answered");
    let decoded = decode(&bytes);
    assert_eq!(decoded.answers.len(), 1);
    assert_eq!(
        decoded.answers[0].data,
        [
            0x05, 0x69, 0x64, 0x3d, 0x72, 0x31, 0x0a, 0x61, 0x75, 0x74, 0x68, 0x3d, 0x74, 0x6f,
            0x6b, 0x65, 0x6e
        ]
    );
    let none = Announcement { txt: &[], ..SHORT };
    let (_, bytes) = reply_about(&none, &query, MDNS_PORT).expect("answered");
    assert_eq!(decode(&bytes).answers[0].data, [0]);
}

/// RFC 6762 §8.3, §10.1 and §10.2: the announcement is an unsolicited response, ID 0, QR and AA,
/// with the PTR, SRV, TXT and A records as its answers; the goodbye is the same records with a
/// time to live of 0, the cache-flush bits unchanged. Catches a browser that does not take the
/// announcement, and a goodbye that leaves browsers listing a chip that has gone for 75 minutes.
#[test]
fn a_goodbye_sends_every_record_with_a_time_to_live_of_0() {
    let mut buf = [0_u8; 512];
    let hello = announcement(&SHORT, Announce::Hello, &mut buf).expect("fits");
    let expected = joined(&[&header(4, 0), PTR_RECORD, SRV_RECORD, TXT_RECORD, A_RECORD]);
    assert_eq!(&buf[..hello], expected.as_slice());

    let goodbye = announcement(&SHORT, Announce::Goodbye, &mut buf).expect("fits");
    assert_eq!(goodbye, hello);
    let decoded = decode(&buf[..goodbye]);
    assert_eq!((decoded.id, decoded.flags), (0, 0x8400));
    let announced = decode(&expected).answers;
    assert_eq!(decoded.answers.len(), 4);
    for (gone, record) in decoded.answers.iter().zip(&announced) {
        assert_eq!(gone.ttl, 0, "{gone:?}");
        assert_eq!(
            Resource {
                ttl: record.ttl,
                ..gone.clone()
            },
            *record
        );
    }
}

/// RFC 6763 §12: additional records are optional, so those that do not fit the buffer are left
/// out, one by one, and the answers stay; a reply whose answers do not fit is not written.
/// Catches a reply cut off in the middle of a record, or a browse left unanswered for want of the
/// additional records' room.
#[test]
fn additional_records_that_do_not_fit_are_left_out() {
    let at_most = |size: usize| -> Option<Vec<u8>> {
        let mut buf = [0_u8; 512];
        let reply = answer(BROWSE, MDNS_PORT, &SHORT, &mut buf[..size])?;
        let (Reply::Multicast(len) | Reply::Unicast(len)) = reply;
        Some(buf[..len].to_vec())
    };
    assert_eq!(at_most(71), None, "the PTR record needs 12 + 60 bytes");
    assert_eq!(at_most(72), Some(joined(&[&header(1, 0), PTR_RECORD])));
    // 72 + 51 for the SRV record leaves 23: no room for the TXT record's 42, room for the A record.
    assert_eq!(
        at_most(146),
        Some(joined(&[&header(1, 2), PTR_RECORD, SRV_RECORD, A_RECORD]))
    );
    let mut buf = [0_u8; 512];
    assert_eq!(announcement(&SHORT, Announce::Hello, &mut buf[..187]), None);
    assert_eq!(
        announcement(&SHORT, Announce::Hello, &mut buf[..188]),
        Some(188)
    );
    assert_eq!(answer(BROWSE, MDNS_PORT, &SHORT, &mut buf[..11]), None);
}

/// RFC 1035 §2.3.4 and RFC 6763 §6.1: a reply about an announcement whose instance or host label is
/// empty or longer than 63 bytes, or one of whose properties is longer than 255 bytes, is not
/// written; 63 and 255 bytes are. Catches a label or a string whose length byte does not hold its
/// length, which browsers would read as other names and properties.
#[test]
fn an_announcement_that_dns_cannot_carry_is_not_written() {
    let label = "x".repeat(63);
    let longer = "x".repeat(64);
    let value = "v".repeat(252);
    let property = [("id", value.as_str())];
    let too_long = "v".repeat(253);
    let too_long_property = [("id", too_long.as_str())];
    let mut buf = [0_u8; 1500];
    for (fits, instance, host, txt) in [
        (true, label.as_str(), label.as_str(), SHORT.txt),
        (true, "a", "h", &property[..]),
        (false, "", "h", SHORT.txt),
        (false, "a", "", SHORT.txt),
        (false, longer.as_str(), "h", SHORT.txt),
        (false, "a", longer.as_str(), SHORT.txt),
        (false, "a", "h", &too_long_property[..]),
    ] {
        let case = Announcement {
            instance,
            host,
            txt,
            ..SHORT
        };
        let written = announcement(&case, Announce::Hello, &mut buf);
        assert_eq!(written.is_some(), fits, "{case:?}");
        assert_eq!(
            reply_about(&case, BROWSE, MDNS_PORT).is_some(),
            fits,
            "{case:?}"
        );
    }
}
