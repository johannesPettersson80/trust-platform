//! The computer driver's wiring of the shared exchange (specification 11 §6.5): its worker carries
//! the exchange over a real TCP connection to a test server, and its `IoDriver` methods give the
//! exchange's results at the scan boundary. The shared module's tests prove the exchange's rules;
//! these prove that the computer reaches them: the deadline under each `on_error` with a snapshot
//! accepted before, a pending output written later, the output path that confirms a safe state,
//! a reconnect after the server closed the connection, an IPv6 server, and the point maps' orders
//! and image bits on the wire. Two of them are the computer worker's former unit tests and two its
//! point-map unit tests, moved onto a real connection.

use std::net::SocketAddr;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration as StdDuration, Instant};

use trust_runtime::error::RuntimeError;
use trust_runtime::io::{IoDriver, IoDriverHealth, ModbusTcpDriver};

use super::modbus_support::{
    start_closing_modbus_server, start_closing_once_modbus_server, start_modbus_server,
    start_modbus_server_on, start_withholding_modbus_server, ModbusTestState,
};

/// The error and the health of an input refresh that is not complete at its deadline
/// (specification 11, "Modbus default-fault and deadline integrity").
const INPUT_PENDING: &str = "modbus input refresh pending";

/// The error and the health of an output handoff that is not complete at its deadline.
const OUTPUT_PENDING: &str = "modbus output handoff pending";

/// The driver of the server at `addr` with `fields`, more lines of its `io.toml` table.
fn driver_at(addr: SocketAddr, fields: &str) -> ModbusTcpDriver {
    let params: toml::Value =
        toml::from_str(&format!("address = \"{addr}\"\n{fields}\n")).expect("params");
    ModbusTcpDriver::from_params(&params).expect("driver")
}

/// A degraded health with `error`.
fn degraded(error: &str) -> IoDriverHealth {
    IoDriverHealth::Degraded {
        error: error.into(),
    }
}

/// A faulted health with `error`.
fn faulted(error: &str) -> IoDriverHealth {
    IoDriverHealth::Faulted {
        error: error.into(),
    }
}

/// Waits up to two seconds for `condition`, looking every 5 ms; `what` says what a timeout means.
fn wait_until(what: &str, mut condition: impl FnMut() -> bool) {
    let deadline = Instant::now() + StdDuration::from_secs(2);
    while !condition() {
        assert!(Instant::now() < deadline, "{what}");
        thread::sleep(StdDuration::from_millis(5));
    }
}

/// "With `fault`, an incomplete input refresh sets `Faulted`, records `modbus input refresh
/// pending`, returns `IoTransport`, and leaves the caller's input bytes unchanged even when an older
/// accepted snapshot is cached"; with `warn` or `ignore` it "sets `Degraded` ... and returns
/// success. If an accepted snapshot exists, it may be copied". The server answers the first read
/// with input register 0x1122 and holds its answer to the second past the 40 ms deadline. The
/// computer worker's unit test with a stalled worker, moved onto a real connection. Catches stale
/// data handed to a faulting scan, a running scan under `warn` or `ignore` left without its last
/// good inputs, and a pending refresh reported as ok.
#[test]
fn a_pending_refresh_follows_on_error_with_an_accepted_snapshot() {
    for policy in ["fault", "warn", "ignore"] {
        let state = Arc::new(Mutex::new(ModbusTestState::with_registers(
            vec![0x1122],
            vec![],
        )));
        let (addr, release) = start_withholding_modbus_server(state, 1);
        let mut driver = driver_at(addr, &format!("timeout_ms = 1000\non_error = \"{policy}\""));
        let mut first = [0; 2];
        driver
            .read_inputs(&mut first)
            .expect("the first refresh is answered");
        assert_eq!(first, [0x11, 0x22], "{policy}");

        let mut inputs = [0xAA, 0xBB];
        let result = driver.read_inputs(&mut inputs);

        if policy == "fault" {
            assert_eq!(result, Err(RuntimeError::IoTransport(INPUT_PENDING.into())));
            assert_eq!(
                inputs,
                [0xAA, 0xBB],
                "fault must not copy the cached snapshot"
            );
            assert_eq!(driver.health(), faulted(INPUT_PENDING));
        } else {
            assert_eq!(result, Ok(()), "{policy}");
            assert_eq!(
                inputs,
                [0x11, 0x22],
                "{policy} copies the accepted snapshot"
            );
            assert_eq!(driver.health(), degraded(INPUT_PENDING), "{policy}");
        }
        drop(release);
    }
}

/// "With `fault`, an output handoff that remains incomplete at the deadline sets `Faulted`, records
/// `modbus output handoff pending`, and returns `IoTransport`. With `warn` or `ignore`, it sets
/// `Degraded` and returns success", and "Deadline reporting does not cancel that pending or
/// in-flight output; a later worker result governs its eventual transport outcome": the server
/// holds its answer to the write of 33 44 past the deadline; released, it writes holding register
/// 0x3344 and answers, and the output path becomes ok. The computer worker's unit test with a
/// stalled worker, moved onto a real connection. Catches a pending handoff confirmed, a handoff
/// cancelled at its deadline, and an output path left unconfirmed after the late answer.
#[test]
fn an_output_handoff_pending_at_its_deadline_is_written_later() {
    for policy in ["fault", "warn", "ignore"] {
        let state = Arc::new(Mutex::new(ModbusTestState::with_registers(vec![], vec![0])));
        let (addr, release) = start_withholding_modbus_server(Arc::clone(&state), 0);
        let mut driver = driver_at(addr, &format!("timeout_ms = 1000\non_error = \"{policy}\""));

        let result = driver.write_outputs(&[0x33, 0x44]);

        if policy == "fault" {
            assert_eq!(
                result,
                Err(RuntimeError::IoTransport(OUTPUT_PENDING.into()))
            );
            assert_eq!(driver.output_health(), faulted(OUTPUT_PENDING));
        } else {
            assert_eq!(result, Ok(()), "{policy}");
            assert_eq!(driver.output_health(), degraded(OUTPUT_PENDING), "{policy}");
        }
        assert_eq!(
            state.lock().expect("modbus state lock").holding_registers,
            [0],
            "{policy}: the server holds the write"
        );
        release.send(()).expect("the server holds the write");
        wait_until(
            &format!("{policy}: the held write must reach the server and confirm the output path"),
            || {
                state.lock().expect("modbus state lock").holding_registers == [0x3344]
                    && driver.output_health() == IoDriverHealth::Ok
            },
        );
    }
}

/// "Output-path health is the driver's health, except that input-path conditions degrade or fault
/// the reported driver health but not the output path, so they never unconfirm a safe-state
/// handoff": the read of input register 7 is refused with exception 2 and faults the driver as the
/// typed `IoAddress` error, while the write of 12 34 that the server then answers leaves the output
/// path ok, the health the runtime reads to confirm a safe state. The server takes one connection,
/// so the write also proves that a valid exception keeps the connection. Catches a safe state
/// unconfirmed by an input fault (the computer's output path was its whole health), an exception
/// rewritten to a transport error, and a connection given up after a valid exception.
#[test]
fn an_input_failure_leaves_the_output_path_confirmed() {
    let state = Arc::new(Mutex::new(ModbusTestState::with_registers(
        vec![0],
        vec![0],
    )));
    let addr = start_modbus_server(Arc::clone(&state), 2);
    let mut driver = driver_at(addr, "input_start = 7");
    let exception =
        "modbus exception code 2 (ILLEGAL DATA ADDRESS): 0x04 Read Input Registers at address 7";

    assert_eq!(
        driver.read_inputs(&mut [0; 2]),
        Err(RuntimeError::IoAddress(exception.into()))
    );
    assert_eq!(driver.write_outputs(&[0x12, 0x34]), Ok(()));

    assert_eq!(driver.output_health(), IoDriverHealth::Ok);
    assert_eq!(driver.health(), faulted(exception));
    assert_eq!(
        state.lock().expect("modbus state lock").holding_registers,
        [0x1234]
    );
}

/// "Reconnect uses a non-zero bounded backoff and cannot spin on an unavailable endpoint", and the
/// driver comes back: the server answers a write and a read of input register 0x1122 and closes
/// the connection, as a server that goes away. The next read finds the connection closed: under
/// `warn` the scan runs on with the accepted snapshot and the driver is degraded. Once the backoff
/// has passed, the worker connects again and a read gets the server's new value 0x3344 with the
/// driver ok. Catches a driver that stays down after its server went away, and a closed connection
/// reported as ok.
#[test]
fn the_driver_reconnects_after_the_server_closes_the_connection() {
    let state = Arc::new(Mutex::new(ModbusTestState::with_registers(
        vec![0x1122],
        vec![0],
    )));
    let (addr, closes) = start_closing_once_modbus_server(Arc::clone(&state), 2);
    let mut driver = driver_at(addr, "on_error = \"warn\"");
    driver.write_outputs(&[0, 0]).expect("the first write");
    let mut inputs = [0; 2];
    driver.read_inputs(&mut inputs).expect("the first read");
    assert_eq!(inputs, [0x11, 0x22]);
    assert_eq!(driver.health(), IoDriverHealth::Ok);
    state.lock().expect("modbus state lock").input_registers[0] = 0x3344;
    closes
        .recv_timeout(StdDuration::from_secs(2))
        .expect("the server closes the first connection");

    let mut inputs = [0; 2];
    assert_eq!(driver.read_inputs(&mut inputs), Ok(()), "warn runs on");
    assert_eq!(inputs, [0x11, 0x22], "the accepted snapshot is copied");
    let health = driver.health();
    assert!(
        matches!(health, IoDriverHealth::Degraded { .. }),
        "a closed connection degrades the driver, got {health:?}"
    );

    wait_until(
        "the driver must read the new value over a new connection",
        || {
            let mut inputs = [0; 2];
            driver.read_inputs(&mut inputs).is_ok()
                && inputs == [0x33, 0x44]
                && driver.health() == IoDriverHealth::Ok
        },
    );
}

/// The address `[::1]:<port>` reaches an IPv6 server: an IPv6 address in brackets is taken, as the
/// computer's driver took any socket address, and the worker connects to it. The test fails rather
/// than passes on a computer without an IPv6 loopback, so that such a computer is seen. Catches
/// IPv6 refused, or lost between the parameters and the connection.
#[test]
fn the_driver_reaches_an_ipv6_server() {
    let state = Arc::new(Mutex::new(ModbusTestState::with_registers(
        vec![0x5566],
        vec![],
    )));
    let addr = start_modbus_server_on("[::1]:0", state, 1);
    assert!(addr.is_ipv6(), "{addr}");
    let mut driver = driver_at(addr, "");
    let mut inputs = [0; 2];
    driver.read_inputs(&mut inputs).expect("a read over IPv6");
    assert_eq!(inputs, [0x55, 0x66]);
}

/// "Byte order applies inside every 16-bit register and word order applies between the two
/// registers of a 32-bit value", and "Process-image numeric values are little-endian": four `u32`
/// points read the holding registers that each order makes of 0x1122_3344 — 1122 3344 with big
/// bytes and words, 2211 4433 with little bytes, 3344 1122 with little words, 4433 2211 with both
/// — and each lands as 44 33 22 11 in the image; written back from such an image, each reaches
/// holding registers 10 to 17 in its order. "A Boolean occupies one selected bit ... writes
/// preserve every other bit in the byte": coil 0, ON, sets bit 3 and coil 1, OFF, clears bit 0 of
/// image byte 16, A5 = 1010_0101, giving 1010_1100 = AC, and byte 17, which no point maps, keeps its
/// A5 (the computer's driver wrote zeros over every byte that no point gave). The computer's
/// point-map unit tests of the order cross product and of a bit's neighbours, moved onto the wire.
/// A cold first refresh, a connection and six round trips, and the four writes may each miss the
/// 40 ms scan deadline on a loaded computer, which the specification allows (`modbus input refresh
/// pending`, `modbus output handoff pending`), so the test reads until a refresh is accepted and
/// waits for the server to hold the writes. Catches the two orders mixed up or applied to the image,
/// a whole byte written for one coil, and unmapped image bytes overwritten.
#[test]
fn mapped_points_carry_each_order_and_keep_the_other_image_bits() {
    let state = Arc::new(Mutex::new(ModbusTestState {
        coils: vec![true, false],
        holding_registers: vec![
            0x1122, 0x3344, 0x2211, 0x4433, 0x3344, 0x1122, 0x4433, 0x2211, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0,
        ],
        ..ModbusTestState::default()
    }));
    let addr = start_modbus_server(Arc::clone(&state), 64);
    let orders = [
        "",
        "byte_order = \"little\"",
        "word_order = \"little\"",
        "byte_order = \"little\"\nword_order = \"little\"",
    ];
    let mut fields = String::from(
        "input_function = \"read_holding_registers\"\n\
         output_function = \"write_multiple_registers\"\n",
    );
    for (index, order) in orders.iter().enumerate() {
        let offset = index * 4;
        let register = index * 2;
        fields.push_str(&format!(
            "[[input_points]]\nimage_offset = {offset}\naddress = {register}\n\
             data_type = \"u32\"\n{order}\n"
        ));
    }
    fields.push_str(
        "[[input_points]]\nimage_offset = 16\nimage_bit = 3\naddress = 0\nfunction = \"read_coils\"\n\
         [[input_points]]\nimage_offset = 16\nimage_bit = 0\naddress = 1\nfunction = \"read_coils\"\n",
    );
    for (index, order) in orders.iter().enumerate() {
        let offset = index * 4;
        let register = 10 + index * 2;
        fields.push_str(&format!(
            "[[output_points]]\nimage_offset = {offset}\naddress = {register}\n\
             data_type = \"u32\"\n{order}\n"
        ));
    }
    let mut driver = driver_at(addr, &fields);

    let mut inputs = [0xA5; 18];
    wait_until("a mapped refresh must be accepted", || {
        inputs = [0xA5; 18];
        driver.read_inputs(&mut inputs).is_ok()
    });
    let value = [0x44, 0x33, 0x22, 0x11];
    for (index, point) in inputs[..16].chunks(4).enumerate() {
        assert_eq!(point, value, "point {index}");
    }
    assert_eq!(inputs[16..], [0xAC, 0xA5]);

    let written = driver.write_outputs(&value.repeat(4));
    assert!(
        written.is_ok() || written == Err(RuntimeError::IoTransport(OUTPUT_PENDING.into())),
        "the mapped write: {written:?}"
    );
    let expected = [
        0x1122, 0x3344, 0x2211, 0x4433, 0x3344, 0x1122, 0x4433, 0x2211,
    ];
    wait_until("the server must hold the mapped writes", || {
        state.lock().expect("modbus state lock").holding_registers[10..] == expected
    });
    let guard = state.lock().expect("modbus state lock");
    let functions = &guard.functions;
    assert_eq!(
        functions[..6],
        [3, 3, 3, 3, 1, 1],
        "a refresh reads the points in order"
    );
    assert_eq!(
        functions[functions.len() - 4..],
        [16, 16, 16, 16],
        "the handoff writes each point with its function"
    );
}

/// "Every offset-plus-width calculation is checked for arithmetic overflow and image bounds before
/// mutation. A failed conversion or range check leaves the complete destination bytes unchanged":
/// a `u32` point at image byte 1 takes bytes 1 to 4, past an input image of 3 bytes, so the refresh
/// fails at once with the `IoDriver` error `modbus point image range 1..5 out of 3`, no byte of the
/// image changes, and the driver does not connect, as a refresh that cannot be taken asks the server
/// nothing. The computer's point-map unit test, moved onto the driver. Catches a value written past
/// the image's end, and a request sent for a refresh that cannot be taken.
#[test]
fn a_point_outside_the_image_fails_before_any_byte_changes() {
    let connections = Arc::new(AtomicUsize::new(0));
    let addr = start_closing_modbus_server(Arc::clone(&connections));
    let mut driver = driver_at(
        addr,
        "[[input_points]]\nimage_offset = 1\naddress = 0\ndata_type = \"u32\"",
    );
    let mut inputs = [0xA5; 3];

    assert_eq!(
        driver.read_inputs(&mut inputs),
        Err(RuntimeError::IoDriver(
            "modbus point image range 1..5 out of 3".into()
        ))
    );

    assert_eq!(inputs, [0xA5; 3]);
    thread::sleep(StdDuration::from_millis(50));
    assert_eq!(connections.load(Ordering::SeqCst), 0, "no connection");
}
