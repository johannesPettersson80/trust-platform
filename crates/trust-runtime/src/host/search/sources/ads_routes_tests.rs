//! The route list of an ADS target (specification 11, O20), read through a fake AMS router. The
//! fake answers index group 803 with literal entry bytes, worked out by hand from the layout that
//! the spec gives: a 44-byte header (AMS Net ID in bytes 0-5, flags as a u32 at 8, the host length
//! as a u32 at 32, the name length as a u32 at 36), then the host text from byte 44 and the name
//! text, each ending with its NUL. Both texts are Windows-1252.

use std::time::Instant;

use serde_json::json;

use crate::ads::diagnostics::TargetIdentity;
use crate::ads::{AdsIndexRead, AdsIndexReadError};

use super::*;

/// A fake AMS router. It answers index group 803 with `answers`, one per index offset, and with
/// ADS error 0x716 past the last one. Any other index group is refused with 0x702, as a router
/// refuses an unknown index group. It records every read it receives.
struct FakeAmsRouter {
    answers: Vec<Result<Vec<u8>, AdsIndexReadError>>,
    reads: Vec<(u32, u32)>,
}

impl FakeAmsRouter {
    fn new(answers: Vec<Result<Vec<u8>, AdsIndexReadError>>) -> Self {
        Self {
            answers,
            reads: Vec::new(),
        }
    }
}

impl AdsIndexRead for FakeAmsRouter {
    fn read_index(
        &mut self,
        index_group: u32,
        index_offset: u32,
        buffer: &mut [u8],
    ) -> Result<usize, AdsIndexReadError> {
        self.reads.push((index_group, index_offset));
        if index_group != 803 {
            return Err(AdsIndexReadError::Ads {
                code: 0x702,
                description: "Invalid index group".to_string(),
            });
        }
        let offset = usize::try_from(index_offset).expect("index offset fits usize");
        match self.answers.get(offset) {
            Some(Ok(entry)) => match buffer.get_mut(..entry.len()) {
                Some(slot) => {
                    slot.copy_from_slice(entry);
                    Ok(entry.len())
                }
                None => Err(AdsIndexReadError::Transport(
                    "the entry is larger than the buffer".to_string(),
                )),
            },
            Some(Err(error)) => Err(error.clone()),
            None => Err(AdsIndexReadError::Ads {
                code: 0x716,
                description: "No more notification handles".to_string(),
            }),
        }
    }
}

// The entries below are laid out by hand, one 8-byte row per line, so that each row can be
// checked against the byte numbers in its comment. rustfmt is kept off them for that reason.

/// Route 0, a temporary route. Bytes 0-5: AMS Net ID 5.23.91.12.1.1. Bytes 8-11: flags 0x01
/// (temporary). Bytes 12-15: timeout 5000 ms, which is 0x1388. Bytes 16-19: largest fragment 8192,
/// which is 0x2000. Bytes 32-35: host length 14. Bytes 36-39: name length 6. Bytes 44-57: the host
/// text `192.168.10.20` and its NUL, 14 bytes. Bytes 58-63: the name `PLC-1` and its NUL, 6 bytes.
/// The entry is 64 bytes long.
#[rustfmt::skip]
const TEMPORARY_ROUTE: [u8; 64] = [
    // bytes 0-7: AMS Net ID 5.23.91.12.1.1, then two bytes that are not read
    5, 23, 91, 12, 1, 1, 0, 0,
    // bytes 8-15: flags 0x01, then the timeout 5000
    1, 0, 0, 0, 0x88, 0x13, 0, 0,
    // bytes 16-23: the largest fragment 8192, then bytes 20-23 that are not read
    0, 0x20, 0, 0, 0, 0, 0, 0,
    // bytes 24-31: not read
    0, 0, 0, 0, 0, 0, 0, 0,
    // bytes 32-39: host length 14, name length 6
    14, 0, 0, 0, 6, 0, 0, 0,
    // bytes 40-47: bytes 40-43 that are not read, then the host text starts `192.`
    0, 0, 0, 0, b'1', b'9', b'2', b'.',
    // bytes 48-55: the host text continues `168.10.2`
    b'1', b'6', b'8', b'.', b'1', b'0', b'.', b'2',
    // bytes 56-63: the host text ends `0` and its NUL, then the name `PLC-1` and its NUL
    b'0', 0, b'P', b'L', b'C', b'-', b'1', 0,
];

/// Route 1, a unidirectional virtual route. Bytes 0-5: AMS Net ID 192.168.10.30.1.1. Bytes 8-11:
/// flags 0x180 (unidirectional, and virtual or NAT). Bytes 32-35: host length 9. Bytes 36-39: name
/// length 5. Bytes 44-52: the host text `10.0.0.7` and its NUL, 9 bytes. Bytes 53-57: the name
/// `Edge` and its NUL, 5 bytes. The entry is 58 bytes long.
#[rustfmt::skip]
const GATEWAY_ROUTE: [u8; 58] = [
    // bytes 0-7: AMS Net ID 192.168.10.30.1.1, then two bytes that are not read
    192, 168, 10, 30, 1, 1, 0, 0,
    // bytes 8-15: flags 0x180, then the timeout 5000
    0x80, 0x01, 0, 0, 0x88, 0x13, 0, 0,
    // bytes 16-23: the largest fragment 8192, then bytes 20-23 that are not read
    0, 0x20, 0, 0, 0, 0, 0, 0,
    // bytes 24-31: not read
    0, 0, 0, 0, 0, 0, 0, 0,
    // bytes 32-39: host length 9, name length 5
    9, 0, 0, 0, 5, 0, 0, 0,
    // bytes 40-47: bytes 40-43 that are not read, then the host text starts `10.0`
    0, 0, 0, 0, b'1', b'0', b'.', b'0',
    // bytes 48-55: the host text continues `.0.7` and its NUL, then the name starts `Edg`
    b'.', b'0', b'.', b'7', 0, b'E', b'd', b'g',
    // bytes 56-57: the name ends `e` and its NUL
    b'e', 0,
];

/// Route 2, with a name in Windows-1252. Bytes 32-35: host length 9. Bytes 36-39: name length 10.
/// Bytes 44-52: the host text `10.0.0.7` and its NUL. Bytes 53-62: the name `Prüfstand`, whose `ü`
/// is the single byte 0xFC, and its NUL: the bytes 50 72 FC 66 73 74 61 6E 64 00. The entry is
/// 63 bytes long.
#[rustfmt::skip]
const WINDOWS_1252_NAME_ROUTE: [u8; 63] = [
    // bytes 0-7: AMS Net ID 5.23.91.12.1.1, then two bytes that are not read
    5, 23, 91, 12, 1, 1, 0, 0,
    // bytes 8-15: flags 0x01, then the timeout 5000
    1, 0, 0, 0, 0x88, 0x13, 0, 0,
    // bytes 16-23: the largest fragment 8192, then bytes 20-23 that are not read
    0, 0x20, 0, 0, 0, 0, 0, 0,
    // bytes 24-31: not read
    0, 0, 0, 0, 0, 0, 0, 0,
    // bytes 32-39: host length 9, name length 10
    9, 0, 0, 0, 10, 0, 0, 0,
    // bytes 40-47: bytes 40-43 that are not read, then the host text starts `10.0`
    0, 0, 0, 0, b'1', b'0', b'.', b'0',
    // bytes 48-55: the host text continues `.0.7`, its NUL, then the name starts with `P r ü`
    b'.', b'0', b'.', b'7', 0, 0x50, 0x72, 0xFC,
    // bytes 56-62: the name continues `f s t a n d` and its NUL
    0x66, 0x73, 0x74, 0x61, 0x6E, 0x64, 0x00,
];

/// Route 3, with a host in Windows-1252. Bytes 32-35: host length 7. Bytes 36-39: name length 4.
/// Bytes 44-50: the host text `Lübeck` and its NUL, where `ü` is the single byte 0xFC. Bytes 51-54:
/// the name `PLC` and its NUL. The entry is 55 bytes long.
#[rustfmt::skip]
const WINDOWS_1252_HOST_ROUTE: [u8; 55] = [
    // bytes 0-7: AMS Net ID 5.23.91.12.1.1, then two bytes that are not read
    5, 23, 91, 12, 1, 1, 0, 0,
    // bytes 8-15: flags 0x01, then the timeout 5000
    1, 0, 0, 0, 0x88, 0x13, 0, 0,
    // bytes 16-23: the largest fragment 8192, then bytes 20-23 that are not read
    0, 0x20, 0, 0, 0, 0, 0, 0,
    // bytes 24-31: not read
    0, 0, 0, 0, 0, 0, 0, 0,
    // bytes 32-39: host length 7, name length 4
    7, 0, 0, 0, 4, 0, 0, 0,
    // bytes 40-47: bytes 40-43 that are not read, then the host text starts `L ü b e`
    0, 0, 0, 0, 0x4C, 0xFC, 0x62, 0x65,
    // bytes 48-54: the host text continues `c k` and its NUL, then the name `P L C` and its NUL
    0x63, 0x6B, 0x00, 0x50, 0x4C, 0x43, 0x00,
];

fn target_at(ip: &str, ams_net_id: &str, ams_port: u16) -> TargetIdentity {
    TargetIdentity {
        name: Some("PLC".to_string()),
        ip: ip.to_string(),
        ams_net_id: ams_net_id.to_string(),
        ams_port,
        tc_version: None,
        os_version: None,
        fingerprint: None,
    }
}

#[test]
fn twincat_route_list_is_reported_for_a_target() {
    // Spec 11, O20: the list is read at index offsets 0, 1, 2 and so on, until 0x716 ends it, and
    // each entry gives its name, AMS Net ID, host and flags. The expected routes are worked out by
    // hand from the bytes above: route 0 is temporary, and route 1 is unidirectional and virtual.
    // Wrong behaviour caught: a stop one entry early or late, a name and host swapped, a flag read
    // from the wrong bit, or a route dropped as damaged.
    let mut router = FakeAmsRouter::new(vec![
        Ok(TEMPORARY_ROUTE.to_vec()),
        Ok(GATEWAY_ROUTE.to_vec()),
    ]);
    let list = read_route_list(&mut router, Deadline::NONE);

    assert_eq!(
        list.to_json(),
        json!({
            "status": "ok",
            "routes": [
                {
                    "name": "PLC-1",
                    "ams_net_id": "5.23.91.12.1.1",
                    "host": "192.168.10.20",
                    "temporary": true,
                    "unidirectional": false,
                    "virtual_nat": false
                },
                {
                    "name": "Edge",
                    "ams_net_id": "192.168.10.30.1.1",
                    "host": "10.0.0.7",
                    "temporary": false,
                    "unidirectional": true,
                    "virtual_nat": true
                }
            ],
            "damaged": []
        })
    );
}

#[test]
fn a_refused_route_read_is_unavailable_not_empty() {
    // Spec 11, O20: a target that refuses the read reports its list as unavailable, with its error.
    // ADS error 0x704 is "Reading/writing not permitted" in the ads crate's table. Wrong behaviour
    // caught: reporting the refused list as an empty list, which would hide the refusal.
    let mut router = FakeAmsRouter::new(vec![Err(AdsIndexReadError::Ads {
        code: 0x704,
        description: "Reading/writing not permitted".to_string(),
    })]);
    let value = read_route_list(&mut router, Deadline::NONE).to_json();

    assert_eq!(value["status"], "unavailable");
    assert_eq!(
        value["error"],
        "ADS error 0x704: Reading/writing not permitted"
    );
    assert!(
        value.get("routes").is_none(),
        "an unavailable list carries no routes: {value}"
    );
}

#[test]
fn a_short_route_entry_is_reported_as_damaged() {
    // Spec 11, O20: an entry shorter than its 44-byte header is damaged. It is reported with its
    // index, and the list goes on: entry 1 is still read as a route. Wrong behaviour caught: a
    // short entry read past its bytes (a panic), dropped without a report, or ending the list.
    let mut router = FakeAmsRouter::new(vec![Ok(vec![0_u8; 43]), Ok(GATEWAY_ROUTE.to_vec())]);
    let value = read_route_list(&mut router, Deadline::NONE).to_json();

    assert_eq!(value["status"], "ok");
    assert_eq!(value["damaged"].as_array().map(Vec::len), Some(1));
    assert_eq!(value["damaged"][0]["index"], 0);
    assert_eq!(value["routes"].as_array().map(Vec::len), Some(1));
    assert_eq!(value["routes"][0]["name"], "Edge");
}

#[test]
fn the_route_read_writes_nothing() {
    // Spec 11, "Safety": a search only reads and never changes a device's state, so the route list
    // adds no route (O20). The fake records every call it receives. The reads must be index group
    // 803 at offsets 0, 1 and 2, where 0x716 ends the list, and no other call. The trait has no
    // write, so these reads are all the search can send. Wrong behaviour caught: reading another
    // index group (801 adds a route), reading past the end marker, or reading an entry twice.
    let mut router = FakeAmsRouter::new(vec![
        Ok(TEMPORARY_ROUTE.to_vec()),
        Ok(GATEWAY_ROUTE.to_vec()),
    ]);
    read_route_list(&mut router, Deadline::NONE);

    assert_eq!(router.reads, vec![(803, 0), (803, 1), (803, 2)]);
}

#[test]
fn a_target_without_a_route_is_unavailable_with_its_reason() {
    // Spec 11, O20: the read needs the route from the target to this runtime. A target that gives
    // no answer to the first read has none: the list is unavailable with that reason, and the
    // search adds no route. Wrong behaviour caught: reporting an empty list, or a transport error
    // with no reason that names the missing route.
    let target = target_at("192.168.10.20", "5.23.91.12.1.1", 851);
    let value = route_list_for_target(&target, Deadline::NONE, |_| {
        let reads: Box<dyn AdsIndexRead> = Box::new(FakeAmsRouter::new(vec![Err(
            AdsIndexReadError::Transport("connection closed by the router".to_string()),
        )]));
        Ok(reads)
    })
    .to_json();

    assert_eq!(value["status"], "unavailable");
    assert!(
        value["reason"]
            .as_str()
            .is_some_and(|reason| reason.contains("no route from the target to this runtime")),
        "the reason must name the missing route: {value}"
    );
    assert_eq!(value["error"], "connection closed by the router");
    assert!(value.get("routes").is_none());
}

#[test]
fn the_route_read_starts_nothing_after_the_search_deadline() {
    // Spec 11, O20: the route list starts no connection and no read after the search deadline, as
    // the ADS port probe starts no new port once it has passed. The deadline here has passed, so no
    // connection may open. Wrong behaviour caught: opening a connection, or reading an entry, after
    // the deadline.
    let target = target_at("192.168.10.20", "5.23.91.12.1.1", 851);
    let value = route_list_for_target(&target, Deadline::at(Instant::now()), |_| {
        panic!("the connection must not open after the deadline")
    })
    .to_json();

    assert_eq!(value["status"], "unavailable");
    assert!(
        value["reason"]
            .as_str()
            .is_some_and(|reason| reason.contains("deadline")),
        "the reason must name the deadline: {value}"
    );
}

#[test]
fn a_route_entry_whose_lengths_run_past_its_bytes_is_damaged_not_a_panic() {
    // Spec 11, O20: each length is checked against the bytes read. A host length of 0xFFFF_FFFF
    // needs more bytes than the 44-byte entry holds, so entry 0 is damaged, and no read goes past
    // the bytes. Wrong behaviour caught: a slice past the end (a panic), or a length that wraps.
    let mut header = [0_u8; 44];
    header[32..36].copy_from_slice(&[0xFF; 4]);
    header[36] = 1;
    let mut router = FakeAmsRouter::new(vec![Ok(header.to_vec()), Ok(GATEWAY_ROUTE.to_vec())]);
    let value = read_route_list(&mut router, Deadline::NONE).to_json();

    assert_eq!(value["status"], "ok");
    assert_eq!(value["damaged"][0]["index"], 0);
    assert_eq!(value["routes"][0]["name"], "Edge");
}

#[test]
fn a_text_without_its_nul_is_damaged() {
    // Spec 11, O20: a text ends with the NUL that its length counts. Byte 52 is the host's NUL; here
    // it reads `1`, so the host text lacks its NUL and entry 0 is damaged. Wrong behaviour caught:
    // a text taken without its terminator, or a length read off by one.
    let mut entry = GATEWAY_ROUTE;
    entry[52] = b'1';
    let mut router = FakeAmsRouter::new(vec![Ok(entry.to_vec()), Ok(GATEWAY_ROUTE.to_vec())]);
    let value = read_route_list(&mut router, Deadline::NONE).to_json();

    assert_eq!(value["damaged"][0]["index"], 0);
    assert!(
        value["damaged"][0]["reason"]
            .as_str()
            .is_some_and(|reason| reason.contains("host")),
        "the reason must name the host text: {value}"
    );
    assert_eq!(value["routes"][0]["name"], "Edge");
}

#[test]
fn a_route_name_in_windows_1252_is_read_as_its_text() {
    // Spec 11, O20: TwinCAT keeps its texts in the Windows ANSI code page, Windows-1252. The name
    // bytes 50 72 FC 66 73 74 61 6E 64 00 are `Prüfstand`, where 0xFC is `ü`, and 0xFC alone is not
    // valid UTF-8. Wrong behaviour caught: the entry reported as damaged for that byte, or decoded
    // lossily so that a replacement character stands where the `ü` is.
    let mut router = FakeAmsRouter::new(vec![Ok(WINDOWS_1252_NAME_ROUTE.to_vec())]);
    let value = read_route_list(&mut router, Deadline::NONE).to_json();

    assert_eq!(value["status"], "ok");
    assert_eq!(value["damaged"], json!([]));
    assert_eq!(value["routes"][0]["name"], "Prüfstand");
}

#[test]
fn a_route_host_in_windows_1252_is_read_as_its_text() {
    // Spec 11, O20: the host text is Windows-1252 too. The host bytes 4C FC 62 65 63 6B 00 are
    // `Lübeck`, and the name that follows the host is read at its own offset, `PLC`. Wrong
    // behaviour caught: the host decoded as UTF-8, which damages the entry for its 0xFC, or decoded
    // lossily, or a name read from the wrong offset.
    let mut router = FakeAmsRouter::new(vec![Ok(WINDOWS_1252_HOST_ROUTE.to_vec())]);
    let value = read_route_list(&mut router, Deadline::NONE).to_json();

    assert_eq!(value["status"], "ok");
    assert_eq!(value["damaged"], json!([]));
    assert_eq!(value["routes"][0]["host"], "Lübeck");
    assert_eq!(value["routes"][0]["name"], "PLC");
}

#[test]
fn an_empty_route_list_is_ok_not_unavailable() {
    // Spec 11, O20: a list that ends with 0x716 at its first entry is empty and has status ok,
    // because the target has no routes. Wrong behaviour caught: an empty but readable list reported
    // as unavailable, or as a refusal, which would tell an engineer that the target cannot be read.
    let mut router = FakeAmsRouter::new(Vec::new());
    let value = read_route_list(&mut router, Deadline::NONE).to_json();

    assert_eq!(value["status"], "ok");
    assert_eq!(value["routes"], json!([]));
    assert_eq!(value["damaged"], json!([]));
    assert_eq!(router.reads, vec![(803, 0)]);
}
