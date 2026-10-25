use alloc::format;
use alloc::vec;

use super::*;

/// The concrete, flat address of `area` and `size` at the byte offset `byte`: specification 11
/// §6.5 reads `%IB8` as the byte at offset 8 of the input image.
fn flat(area: IoArea, size: IoSize, byte: u32) -> IoAddress {
    IoAddress {
        area,
        size,
        byte,
        bit: 0,
        path: vec![byte],
        wildcard: false,
    }
}

/// The refusal of `key` for `reason`.
fn refused(key: &str, reason: &str) -> Result<CanParams, ParamsError> {
    Err(ParamsError {
        key: key.into(),
        reason: reason.into(),
    })
}

/// The parameters of a 500 kbit/s bus on `can0` with `fields`, more JSON members, after `line`.
fn bus_with(fields: &str) -> Result<CanParams, ParamsError> {
    CanParams::from_json(&format!(r#"{{"bitrate":500000,"line":"can0",{fields}}}"#))
}

/// The parameters of a 500 kbit/s bus on `can0` with the one receive `entry`, a JSON table.
fn receive(entry: &str) -> Result<CanParams, ParamsError> {
    bus_with(&format!(r#""receive":[{entry}]"#))
}

/// The parameters of a 500 kbit/s bus on `can0` with the one transmit `entry`, a JSON table.
fn transmit(entry: &str) -> Result<CanParams, ParamsError> {
    bus_with(&format!(r#""transmit":[{entry}]"#))
}

/// The 11-bit data frame `id` that carries `bytes`, with zeros past them.
fn frame(id: u32, bytes: &[u8]) -> CanFrame {
    let mut data = [0; 8];
    data[..bytes.len()].copy_from_slice(bytes);
    CanFrame {
        id: CanId {
            id,
            extended: false,
        },
        len: u8::try_from(bytes.len()).expect("at most 8 bytes"),
        data,
    }
}

/// The frames `send_due` gives at `now_ms` from the output image `outputs`, in order.
fn sent(exchange: &mut CanExchange, outputs: &[u8], now_ms: u64) -> Vec<CanFrame> {
    let mut frames = Vec::new();
    exchange.send_due(outputs, now_ms, |due| frames.push(due));
    frames
}

/// A bus with two receives, 0x181 (385) with a timeout of 100 ms and 0x182 (386) without, and two
/// transmits, 0x201 (513) every 10 ms and 0x202 (514) every 25 ms.
fn bus() -> CanParams {
    CanParams::from_json(concat!(
        r#"{"bitrate":500000,"line":"can0","receive":["#,
        r#"{"address":"%IB0","id":385,"length":8,"timeout_ms":100},"#,
        r#"{"address":"%IB8","id":386,"length":2}],"transmit":["#,
        r#"{"address":"%QB2","id":513,"length":3,"period_ms":10},"#,
        r#"{"address":"%QB0","id":514,"length":2,"period_ms":25}]}"#
    ))
    .expect("a valid bus")
}

/// A chip's controller with its pins, a receive with a timeout, an extended receive without one and
/// a transmit read into exactly those values; `id = 0x181` arrives as the JSON integer 385, and
/// `0x18FE_F100` as 419,361,024 (specification 34 §5.1.1: integers as JSON integers). Catches a
/// key read into the wrong field, an extended identifier read as an 11-bit one, and a timeout
/// invented for a receive without one.
#[test]
fn a_full_table_reads_into_its_values() {
    let params = CanParams::from_json(concat!(
        r#"{"bitrate":500000,"line":"TWAI0","on_error":"warn","receive":["#,
        r#"{"address":"%IB0","id":385,"length":8,"timeout_ms":100},"#,
        r#"{"address":"%IB8","extended":true,"id":419361024,"length":4}],"rx":"GPIO5","#,
        r#""transmit":[{"address":"%QB0","id":513,"length":2,"period_ms":50}],"tx":"GPIO4"}"#
    ));
    assert_eq!(
        params,
        Ok(CanParams {
            line: "TWAI0".into(),
            tx: Some("GPIO4".into()),
            rx: Some("GPIO5".into()),
            bitrate: 500_000,
            receive: vec![
                CanReceive {
                    id: CanId {
                        id: 0x181,
                        extended: false,
                    },
                    address: flat(IoArea::Input, IoSize::Byte, 0),
                    length: 8,
                    timeout_ms: Some(100),
                },
                CanReceive {
                    id: CanId {
                        id: 0x18FE_F100,
                        extended: true,
                    },
                    address: flat(IoArea::Input, IoSize::Byte, 8),
                    length: 4,
                    timeout_ms: None,
                },
            ],
            transmit: vec![CanTransmit {
                id: CanId {
                    id: 0x201,
                    extended: false,
                },
                address: flat(IoArea::Output, IoSize::Byte, 0),
                length: 2,
                period_ms: 50,
            }],
            on_error: OnError::Warn,
        })
    );
}

/// Without `tx`, `rx`, lists and `on_error`, a bus has no pins, no frames and `on_error = "fault"`;
/// every rate of `CiA 301` is taken, and each `on_error` word is read as its policy
/// (specification 11 §6.5). Catches a Linux interface, which has no pins, refused, a rate of the
/// list refused, and a default policy other than `fault`.
#[test]
fn a_bus_takes_its_defaults_and_every_rate_of_cia_301() {
    assert_eq!(
        CanParams::from_json(r#"{"bitrate":125000,"line":"can0"}"#),
        Ok(CanParams {
            line: "can0".into(),
            tx: None,
            rx: None,
            bitrate: 125_000,
            receive: vec![],
            transmit: vec![],
            on_error: OnError::Fault,
        })
    );
    for rate in [
        10_000, 20_000, 50_000, 100_000, 125_000, 250_000, 500_000, 800_000, 1_000_000,
    ] {
        let params = CanParams::from_json(&format!(r#"{{"bitrate":{rate},"line":"can0"}}"#))
            .expect("a rate of CiA 301");
        assert_eq!(params.bitrate, rate);
    }
    let policy =
        |word: &str| bus_with(&format!(r#""on_error":"{word}""#)).map(|params| params.on_error);
    assert_eq!(policy("fault"), Ok(OnError::Fault));
    assert_eq!(policy("warn"), Ok(OnError::Warn));
    assert_eq!(policy("ignore"), Ok(OnError::Ignore));
}

/// The controller's keys are checked (specification 11 §6.5): `line` is required, `tx` and `rx`
/// come together, a pin is used once, `bitrate` is required and a rate of `CiA 301`, and
/// `on_error` is `fault`, `warn` or `ignore`. Catches a chip's controller started with one pin to
/// its transceiver, both pins on one line, a rate the bus does not run at, and a misspelt policy
/// taken as `fault` or ignored.
#[test]
fn a_controller_key_that_breaks_its_rule_is_refused() {
    let rates = "one of the CiA 301 bit rates 10000, 20000, 50000, 100000, 125000, 250000, \
                 500000, 800000 or 1000000";
    assert_eq!(
        CanParams::from_json("[]"),
        refused("params", "must be a table, not []")
    );
    assert_eq!(
        bus_with(r#""timeout_ms":100"#),
        refused(
            "timeout_ms",
            "unknown key; the can driver takes bitrate, line, on_error, receive, rx, transmit \
             and tx"
        )
    );
    assert_eq!(
        CanParams::from_json(r#"{"bitrate":500000}"#),
        refused(
            "line",
            "is required, a non-empty string naming the CAN controller"
        )
    );
    assert_eq!(
        CanParams::from_json(r#"{"bitrate":500000,"line":""}"#),
        refused(
            "line",
            r#"must be a non-empty string naming the CAN controller, not """#
        )
    );
    assert_eq!(
        bus_with(r#""tx":"GPIO4""#),
        refused(
            "rx",
            "is required with tx: a chip's controller takes both pins, Linux neither"
        )
    );
    assert_eq!(
        bus_with(r#""rx":"GPIO5""#),
        refused(
            "tx",
            "is required with rx: a chip's controller takes both pins, Linux neither"
        )
    );
    assert_eq!(
        bus_with(r#""rx":"GPIO4","tx":4"#),
        refused("tx", "must be a non-empty string naming the pin, not 4")
    );
    assert_eq!(
        bus_with(r#""rx":"GPIO4","tx":"GPIO4""#),
        refused(
            "rx",
            r#""GPIO4" is used by tx already; a line is used once"#
        )
    );
    assert_eq!(
        CanParams::from_json(r#"{"bitrate":500000,"line":"GPIO4","rx":"GPIO5","tx":"GPIO4"}"#),
        refused(
            "tx",
            r#""GPIO4" is used by line already; a line is used once"#
        )
    );
    assert_eq!(
        CanParams::from_json(r#"{"line":"can0"}"#),
        refused("bitrate", &format!("is required, {rates}"))
    );
    assert_eq!(
        CanParams::from_json(r#"{"bitrate":300000,"line":"can0"}"#),
        refused("bitrate", &format!("must be {rates}, not 300000"))
    );
    assert_eq!(
        CanParams::from_json(r#"{"bitrate":"500000","line":"can0"}"#),
        refused("bitrate", &format!(r#"must be {rates}, not "500000""#))
    );
    assert_eq!(
        bus_with(r#""on_error":"halt""#),
        refused(
            "on_error",
            r#"must be "fault", "warn" or "ignore", not "halt""#
        )
    );
}

/// A frame identifier is 11 bits, up to 0x7FF, or 29 bits, up to `0x1FFF_FFFF`, with
/// `extended = true`; both ends are taken (specification 11 §6.5; ISO 11898-1). Catches an
/// identifier cut to fit, which would answer another device's frames, an extended identifier
/// without `extended`, and a misspelt `extended` taken as false.
#[test]
fn an_identifier_beyond_its_bits_is_refused() {
    let standard = "an 11-bit identifier from 0 to 0x7ff, or to 0x1fffffff with extended = true";
    let extended = "a 29-bit identifier from 0 to 0x1fffffff";
    assert_eq!(
        receive(r#"{"address":"%IB0","length":8}"#),
        refused("receive[0].id", &format!("is required, {standard}"))
    );
    assert_eq!(
        receive(r#"{"address":"%IB0","id":2048,"length":8}"#),
        refused(
            "receive[0].id",
            &format!("must be {standard}, not 2048 (0x800)")
        )
    );
    assert_eq!(
        receive(r#"{"address":"%IB0","id":-1,"length":8}"#),
        refused("receive[0].id", &format!("must be {standard}, not -1"))
    );
    assert_eq!(
        receive(r#"{"address":"%IB0","id":1.5,"length":8}"#),
        refused("receive[0].id", &format!("must be {standard}, not 1.5"))
    );
    assert_eq!(
        transmit(r#"{"address":"%QB0","id":"0x181","length":8,"period_ms":10}"#),
        refused(
            "transmit[0].id",
            &format!(r#"must be {standard}, not "0x181""#)
        )
    );
    assert_eq!(
        receive(r#"{"address":"%IB0","extended":true,"id":536870912,"length":8}"#),
        refused(
            "receive[0].id",
            &format!("must be {extended}, not 536870912 (0x20000000)")
        )
    );
    assert_eq!(
        receive(r#"{"address":"%IB0","extended":"yes","id":385,"length":8}"#),
        refused("receive[0].extended", r#"must be true or false, not "yes""#)
    );
    // The ends: 0x7FF is 2,047, and 0x1FFF_FFFF is 536,870,911.
    let ends = bus_with(concat!(
        r#""receive":[{"address":"%IB0","id":2047,"length":8},"#,
        r#"{"address":"%IB8","extended":true,"id":536870911,"length":8}]"#
    ))
    .expect("both ends are taken");
    let ids: Vec<CanId> = ends.receive.iter().map(|entry| entry.id).collect();
    assert_eq!(
        ids,
        vec![
            CanId {
                id: 0x7FF,
                extended: false,
            },
            CanId {
                id: 0x1FFF_FFFF,
                extended: true,
            },
        ]
    );
}

/// A receive's and a transmit's other keys are checked (specification 11 §6.5): the address a
/// concrete, flat `%IB` or `%QB` byte, `length` 0 to 8 and required, `timeout_ms` and `period_ms`
/// 1 to 60,000, `period_ms` required, and no key of the other kind. Catches a frame written to the
/// output image or read from the input image, a ninth data byte, a timeout of 0 that is always out,
/// and a transmit that is never sent.
#[test]
fn a_frame_key_that_breaks_its_rule_is_refused() {
    let to_60s = |given: &str| format!("must be an integer from 1 to 60000, not {given}");
    assert_eq!(
        bus_with(r#""receive":{}"#),
        refused("receive", "must be an array of tables, not {}")
    );
    assert_eq!(
        bus_with(r#""transmit":[513]"#),
        refused("transmit[0]", "must be a table, not 513")
    );
    assert_eq!(
        receive(r#"{"address":"%IB0","id":385,"length":8,"period_ms":10}"#),
        refused(
            "receive[0].period_ms",
            "unknown key; a receive takes address, extended, id, length and timeout_ms"
        )
    );
    assert_eq!(
        transmit(r#"{"address":"%QB0","id":513,"length":8,"period_ms":10,"timeout_ms":10}"#),
        refused(
            "transmit[0].timeout_ms",
            "unknown key; a transmit takes address, extended, id, length and period_ms"
        )
    );
    assert_eq!(
        receive(r#"{"address":"%QB0","id":385,"length":8}"#),
        refused(
            "receive[0].address",
            r#"must be a concrete, flat %IB byte such as "%IB0", not "%QB0""#
        )
    );
    assert_eq!(
        receive(r#"{"address":"%IW0","id":385,"length":8}"#),
        refused(
            "receive[0].address",
            r#"must be a concrete, flat %IB byte such as "%IB0", not "%IW0""#
        )
    );
    assert_eq!(
        transmit(r#"{"address":"%IB0","id":513,"length":8,"period_ms":10}"#),
        refused(
            "transmit[0].address",
            r#"must be a concrete, flat %QB byte such as "%QB0", not "%IB0""#
        )
    );
    assert_eq!(
        receive(r#"{"address":"%IB0","id":385}"#),
        refused("receive[0].length", "is required, an integer from 0 to 8")
    );
    assert_eq!(
        receive(r#"{"address":"%IB0","id":385,"length":9}"#),
        refused("receive[0].length", "must be an integer from 0 to 8, not 9")
    );
    assert_eq!(
        receive(r#"{"address":"%IB0","id":385,"length":8,"timeout_ms":0}"#),
        refused("receive[0].timeout_ms", &to_60s("0"))
    );
    assert_eq!(
        receive(r#"{"address":"%IB0","id":385,"length":8,"timeout_ms":60001}"#),
        refused("receive[0].timeout_ms", &to_60s("60001"))
    );
    assert_eq!(
        transmit(r#"{"address":"%QB0","id":513,"length":8}"#),
        refused(
            "transmit[0].period_ms",
            "is required, an integer from 1 to 60000"
        )
    );
    assert_eq!(
        transmit(r#"{"address":"%QB0","id":513,"length":8,"period_ms":0}"#),
        refused("transmit[0].period_ms", &to_60s("0"))
    );
    assert_eq!(
        transmit(r#"{"address":"%QB0","id":513,"length":8,"period_ms":60001}"#),
        refused("transmit[0].period_ms", &to_60s("60001"))
    );
}

/// The ends of `length`, `timeout_ms` and `period_ms` are taken: 0 and 8 bytes, 1 and 60,000 ms
/// (specification 11 §6.5). Catches a range cut short at either end, which would refuse a frame
/// without data or a once-a-minute heartbeat.
#[test]
fn the_ends_of_each_range_are_taken() {
    let params = bus_with(concat!(
        r#""receive":[{"address":"%IB0","id":385,"length":0,"timeout_ms":1},"#,
        r#"{"address":"%IB1","id":386,"length":8,"timeout_ms":60000}],"#,
        r#""transmit":[{"address":"%QB0","id":513,"length":0,"period_ms":1},"#,
        r#"{"address":"%QB1","id":514,"length":8,"period_ms":60000}]"#
    ))
    .expect("the ends are taken");
    let receives: Vec<(u8, Option<u32>)> = params
        .receive
        .iter()
        .map(|entry| (entry.length, entry.timeout_ms))
        .collect();
    assert_eq!(receives, vec![(0, Some(1)), (8, Some(60_000))]);
    let transmits: Vec<(u8, u32)> = params
        .transmit
        .iter()
        .map(|entry| (entry.length, entry.period_ms))
        .collect();
    assert_eq!(transmits, vec![(0, 1), (8, 60_000)]);
}

/// Two receives, or two transmits, with one `id` and `extended` are refused, and a receive and a
/// transmit may share one; an extended identifier is another than the 11-bit one of the same value,
/// and is written with all eight digits. An address is used once in its list (specification 11
/// §6.5). Catches two receives of one frame, where one would never be written, a transmit sending
/// two sets of bytes as one frame, and the shared identifier of a request and its answer refused.
#[test]
fn an_identifier_is_used_once_in_each_list() {
    assert_eq!(
        bus_with(concat!(
            r#""receive":[{"address":"%IB0","id":385,"length":8},"#,
            r#"{"address":"%IB8","id":385,"length":8}]"#
        )),
        refused(
            "receive[1].id",
            "0x181 is used by receive[0].id already; two receives with one id are refused"
        )
    );
    assert_eq!(
        bus_with(concat!(
            r#""receive":[{"address":"%IB0","extended":true,"id":385,"length":8},"#,
            r#"{"address":"%IB8","extended":true,"id":385,"length":8}]"#
        )),
        refused(
            "receive[1].id",
            "0x00000181 is used by receive[0].id already; two receives with one id are refused"
        )
    );
    assert_eq!(
        bus_with(concat!(
            r#""transmit":[{"address":"%QB0","id":513,"length":8,"period_ms":10},"#,
            r#"{"address":"%QB8","id":513,"length":8,"period_ms":20}]"#
        )),
        refused(
            "transmit[1].id",
            "0x201 is used by transmit[0].id already; two transmits with one id are refused"
        )
    );
    assert_eq!(
        bus_with(concat!(
            r#""receive":[{"address":"%IB0","id":385,"length":8},"#,
            r#"{"address":"%IB0","id":386,"length":8}]"#
        )),
        refused(
            "receive[1].address",
            r#""%IB0" overlaps the bytes of receive[0].address ("%IB0"); the entries of a list use bytes of their own"#
        )
    );
    let shared = bus_with(concat!(
        r#""receive":[{"address":"%IB0","id":385,"length":8},"#,
        r#"{"address":"%IB8","extended":true,"id":385,"length":8}],"#,
        r#""transmit":[{"address":"%QB0","id":385,"length":8,"period_ms":10}]"#
    ))
    .expect("a receive and a transmit share 0x181, and the extended 0x181 is another");
    assert_eq!(shared.receive.len(), 2);
    assert_eq!(shared.transmit.len(), 1);
}

/// The newest frame before a cycle start writes its data bytes to its receive's `length` bytes,
/// zeros where its data length code does not reach; a receive without a frame keeps its bytes, a
/// frame of no receive's identifier is dropped, and the extended 0x181 is not the 11-bit 0x181
/// (specification 11 §6.5). Catches bytes of an older, longer frame left behind, bytes written
/// past `length`, and a frame written to the receive of another identifier.
#[test]
fn the_newest_frame_writes_its_bytes_and_zeros_past_them() {
    let mut exchange = CanExchange::new(bus(), 1_000);
    let mut inputs = [0xEE; 12];
    assert_eq!(exchange.read_inputs(&mut inputs, 1_000), Ok(()));
    assert_eq!(inputs, [0xEE; 12]);
    exchange.received(&frame(0x181, &[1, 2, 3, 4, 5, 6, 7, 8]), 1_010);
    exchange.received(&frame(0x181, &[0x11, 0x22]), 1_020);
    exchange.received(&frame(0x200, &[9; 8]), 1_030);
    let extended = CanFrame {
        id: CanId {
            id: 0x181,
            extended: true,
        },
        len: 8,
        data: [9; 8],
    };
    exchange.received(&extended, 1_030);
    exchange.received(
        &frame(0x182, &[0xA1, 0xA2, 0xA3, 0xA4, 0xA5, 0xA6, 0xA7, 0xA8]),
        1_040,
    );
    assert_eq!(exchange.read_inputs(&mut inputs, 1_050), Ok(()));
    // 0x181: its newest frame's two bytes, then zeros to its length of 8. 0x182: the first 2 of
    // its frame's 8 bytes. Bytes 10 and 11 are no receive's.
    assert_eq!(
        inputs,
        [0x11, 0x22, 0, 0, 0, 0, 0, 0, 0xA1, 0xA2, 0xEE, 0xEE]
    );
}

/// Bytes past a frame's data length code are zeros, whatever the frame's buffer holds there
/// (specification 11 §6.5: zeros where its data length code does not reach), and a receive's bytes
/// past the end of the input image are left out, as `read_inputs` documents. Catches all eight data
/// bytes copied whatever the frame's length, and a write past the image's end, which would panic.
#[test]
fn bytes_past_the_data_length_code_are_zeros() {
    let mut exchange = CanExchange::new(bus(), 0);
    let mut inputs = [0xEE; 12];
    let three = CanFrame {
        id: CanId {
            id: 0x181,
            extended: false,
        },
        len: 3,
        data: [1, 2, 3, 4, 5, 6, 7, 8],
    };
    exchange.received(&three, 0);
    assert_eq!(exchange.read_inputs(&mut inputs, 0), Ok(()));
    assert_eq!(inputs[..8], [1, 2, 3, 0, 0, 0, 0, 0]);
    // A receive of 8 bytes from %IB10 in an input image of 12 bytes writes bytes 10 and 11.
    let edge = bus_with(r#""receive":[{"address":"%IB10","id":385,"length":8}]"#)
        .expect("a receive at byte 10");
    let mut exchange = CanExchange::new(edge, 0);
    exchange.received(&frame(0x181, &[1, 2, 3]), 0);
    let mut inputs = [0xEE; 12];
    assert_eq!(exchange.read_inputs(&mut inputs, 0), Ok(()));
    assert_eq!(inputs[8..], [0xEE, 0xEE, 1, 2]);
}

/// A receive with `timeout_ms` that has had no frame for that long, counted from the start until
/// its first frame, makes `read_inputs` fail with `no frame 0x181 for 100 ms`; the next frame ends
/// it, and the timeout runs again from that frame; the inputs are written all the same
/// (specification 11 §6.5). Catches a timeout one millisecond late or early, a timeout that never
/// ends, a timeout counted from the last read instead of the last frame, and inputs left unwritten
/// on a timeout.
#[test]
fn no_frame_for_the_timeout_fails_the_read() {
    let mut exchange = CanExchange::new(bus(), 1_000);
    let mut inputs = [0; 12];
    // 99 ms after the start is within the timeout; at 100 ms no frame has come for 100 ms.
    assert_eq!(exchange.read_inputs(&mut inputs, 1_099), Ok(()));
    assert_eq!(
        exchange.read_inputs(&mut inputs, 1_100),
        Err("no frame 0x181 for 100 ms".into())
    );
    // A frame at 1,150 ends it; then 1,249 is 99 ms after it and 1,250 is 100 ms.
    exchange.received(&frame(0x181, &[7]), 1_150);
    assert_eq!(exchange.read_inputs(&mut inputs, 1_150), Ok(()));
    assert_eq!(exchange.read_inputs(&mut inputs, 1_249), Ok(()));
    let mut inputs = [0xEE; 12];
    assert_eq!(
        exchange.read_inputs(&mut inputs, 1_250),
        Err("no frame 0x181 for 100 ms".into())
    );
    // Written all the same: the frame's byte, then zeros to its length of 8.
    assert_eq!(inputs[..8], [7, 0, 0, 0, 0, 0, 0, 0]);
    // 0x182 has no timeout: long after the start, with a fresh 0x181, the read succeeds.
    exchange.received(&frame(0x181, &[8]), 1_000_000);
    assert_eq!(exchange.read_inputs(&mut inputs, 1_000_000), Ok(()));
}

/// When several receives' timeouts ran out, the first receive in the list is named, and an
/// extended identifier is written with all eight digits (specification 11 §6.5). Catches the
/// last receive named, and an extended identifier written as the 11-bit one of the same value.
#[test]
fn the_first_receive_out_of_time_is_named() {
    let params = bus_with(concat!(
        r#""receive":[{"address":"%IB0","id":385,"length":1,"timeout_ms":200},"#,
        r#"{"address":"%IB1","extended":true,"id":385,"length":1,"timeout_ms":100}]"#
    ))
    .expect("two receives of 0x181");
    let mut exchange = CanExchange::new(params, 0);
    let mut inputs = [0; 2];
    // At 100 ms only the extended 0x181's timeout of 100 ms ran out.
    assert_eq!(
        exchange.read_inputs(&mut inputs, 100),
        Err("no frame 0x00000181 for 100 ms".into())
    );
    // At 200 ms both ran out, and the first in the list is named.
    assert_eq!(
        exchange.read_inputs(&mut inputs, 200),
        Err("no frame 0x181 for 200 ms".into())
    );
}

/// Every transmit is due at the start, in list order, with its identifier, its length and its
/// `length` bytes of the output image from its address, the image as it is at that call; bytes
/// past the image's end are sent as 0 (specification 11 §6.5; §6.4: the image reads zeros past
/// its end). Catches bytes taken from the wrong offset, a frame with more bytes than `length`,
/// bytes of an older image sent, and a read past the image's end.
#[test]
fn a_transmit_sends_its_bytes_of_the_output_image() {
    let mut exchange = CanExchange::new(bus(), 0);
    let outputs = [10, 11, 12, 13, 14, 15];
    assert_eq!(
        sent(&mut exchange, &outputs, 0),
        vec![frame(0x201, &[12, 13, 14]), frame(0x202, &[10, 11])]
    );
    // 0x201 is due again at 10 ms, with the image of that call.
    assert_eq!(
        sent(&mut exchange, &[20, 21, 22, 23, 24, 25], 10),
        vec![frame(0x201, &[22, 23, 24])]
    );
    // At 25 ms 0x201, due at 20, and 0x202, due at 25, are both due, in list order.
    assert_eq!(
        sent(&mut exchange, &outputs, 25),
        vec![frame(0x201, &[12, 13, 14]), frame(0x202, &[10, 11])]
    );
    // An image of 3 bytes holds 0x201's first byte, %QB2, only.
    let mut exchange = CanExchange::new(bus(), 0);
    assert_eq!(
        sent(&mut exchange, &[10, 11, 12], 0),
        vec![frame(0x201, &[12, 0, 0]), frame(0x202, &[10, 11])]
    );
}

/// A transmit is due one period after the time it was due, so a little lateness keeps its phase;
/// after a gap of a period or more it is sent once, not once per missed period, and then keeps its
/// period from that time (specification 11 §6.5: every `period_ms`). Catches a burst of the missed
/// frames after a long cycle, a period counted from each send so that it drifts, and a frame sent
/// before it is due.
#[test]
fn a_transmit_is_due_once_after_a_long_gap() {
    let mut exchange = CanExchange::new(bus(), 0);
    let ids = |frames: Vec<CanFrame>| -> Vec<u32> { frames.iter().map(|due| due.id.id).collect() };
    let outputs = [0; 6];
    // 0x201 every 10 ms, 0x202 every 25 ms, both due at the start.
    assert_eq!(ids(sent(&mut exchange, &outputs, 0)), vec![0x201, 0x202]);
    assert_eq!(ids(sent(&mut exchange, &outputs, 9)), Vec::<u32>::new());
    assert_eq!(ids(sent(&mut exchange, &outputs, 10)), vec![0x201]);
    // 3 ms late at 23 ms: sent, and the next stays at 30, one period after it was due at 20.
    assert_eq!(ids(sent(&mut exchange, &outputs, 23)), vec![0x201]);
    assert_eq!(ids(sent(&mut exchange, &outputs, 24)), Vec::<u32>::new());
    assert_eq!(ids(sent(&mut exchange, &outputs, 25)), vec![0x202]);
    assert_eq!(ids(sent(&mut exchange, &outputs, 30)), vec![0x201]);
    // A gap to 100 ms: 0x201, due at 40, and 0x202, due at 50, each sent once, not once for each
    // period missed; their next times are 110 and 125, a period after 100.
    assert_eq!(ids(sent(&mut exchange, &outputs, 100)), vec![0x201, 0x202]);
    assert_eq!(ids(sent(&mut exchange, &outputs, 100)), Vec::<u32>::new());
    assert_eq!(ids(sent(&mut exchange, &outputs, 109)), Vec::<u32>::new());
    assert_eq!(ids(sent(&mut exchange, &outputs, 110)), vec![0x201]);
    assert_eq!(ids(sent(&mut exchange, &outputs, 124)), vec![0x201]);
    assert_eq!(ids(sent(&mut exchange, &outputs, 125)), vec![0x202]);
    // Exactly one period late: 0x201, due at 130, is sent at 140, which serves 140 as well, so
    // the next is 150 and a second call at 140 sends nothing.
    assert_eq!(ids(sent(&mut exchange, &outputs, 140)), vec![0x201]);
    assert_eq!(ids(sent(&mut exchange, &outputs, 140)), Vec::<u32>::new());
    assert_eq!(ids(sent(&mut exchange, &outputs, 149)), Vec::<u32>::new());
    assert_eq!(ids(sent(&mut exchange, &outputs, 150)), vec![0x201, 0x202]);
}

/// Specification 11 §6.5: a receive takes its `length` bytes from its address, so one of 8 bytes at
/// `%IB0` (bytes 0 to 7) and one of 2 at `%IB4` (bytes 4 and 5) overlap and the second is refused;
/// frames of length 0 take no bytes and overlap nothing. `on_error` is read as for Modbus: trimmed,
/// in any case, with `warning` for `warn`. Catches two receives that write one byte, each over the
/// other, and a CAN `on_error` that refuses what a Modbus one takes.
#[test]
fn frames_of_a_list_use_bytes_of_their_own_and_on_error_reads_as_for_modbus() {
    assert_eq!(
        bus_with(concat!(
            r#""receive":[{"address":"%IB0","id":385,"length":8},"#,
            r#"{"address":"%IB4","id":386,"length":2}]"#
        )),
        refused(
            "receive[1].address",
            r#""%IB4" overlaps the bytes of receive[0].address ("%IB0"); the entries of a list use bytes of their own"#
        )
    );
    assert!(bus_with(concat!(
        r#""receive":[{"address":"%IB0","id":385,"length":0},"#,
        r#"{"address":"%IB0","id":386,"length":0}]"#
    ))
    .is_ok());
    let warned = CanParams::from_json(r#"{"line":"can0","bitrate":500000,"on_error":" Warning "}"#)
        .expect("a bus that warns");
    assert_eq!(warned.on_error, OnError::Warn);
}
