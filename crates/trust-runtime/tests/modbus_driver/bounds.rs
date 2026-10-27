//! The scan side's bounds over real connections (specification 11, "Modbus default-fault and
//! deadline integrity"): a read and a write return within the scan's deadline while the worker
//! waits on a slow server, a cold start gives bounded no data under each `on_error`, the accepted
//! snapshot is read while the worker reconnects, the reconnect backoff neither spins nor grows
//! without bound, an output handoff stays bounded when the scan outpaces the worker, and dropping
//! the driver does not wait for a worker blocked on a response.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration as StdDuration, Instant};

use trust_runtime::io::{IoDriver, ModbusTcpDriver};

use super::modbus_support::{
    start_closing_modbus_server, start_delayed_modbus_server, start_modbus_server, ModbusTestState,
};
use super::{MODBUS_CI_SCAN_BOUND, SLOW_SERVER};

#[test]
fn modbus_read_inputs_returns_within_scan_bound_while_response_delayed() {
    let delay = SLOW_SERVER;
    let state = Arc::new(Mutex::new(ModbusTestState::with_registers(
        vec![0x1122],
        vec![0u16; 1],
    )));
    let addr = start_delayed_modbus_server(state, delay);
    let params: toml::Value = toml::from_str(&format!(
        "address = \"{addr}\"\nunit_id = 1\ninput_start = 0\noutput_start = 0\ntimeout_ms = 2000\non_error = \"warn\"\n"
    ))
    .expect("params");
    let mut driver = ModbusTcpDriver::from_params(&params).expect("driver");
    let mut inputs = vec![0u8; 2];

    let started = Instant::now();
    driver
        .read_inputs(&mut inputs)
        .expect("warn policy should not fault the scan while worker waits");
    let elapsed = started.elapsed();

    assert!(
        elapsed < MODBUS_CI_SCAN_BOUND,
        "Modbus read_inputs must return within the scan-thread bound while the worker waits on the delayed response, elapsed={elapsed:?}"
    );
}

#[test]
fn modbus_write_outputs_returns_within_scan_bound_while_response_delayed() {
    let delay = SLOW_SERVER;
    let state = Arc::new(Mutex::new(ModbusTestState::with_registers(
        vec![0u16; 1],
        vec![0u16; 1],
    )));
    let addr = start_delayed_modbus_server(state, delay);
    let params: toml::Value = toml::from_str(&format!(
        "address = \"{addr}\"\nunit_id = 1\ninput_start = 0\noutput_start = 0\ntimeout_ms = 2000\non_error = \"warn\"\n"
    ))
    .expect("params");
    let mut driver = ModbusTcpDriver::from_params(&params).expect("driver");

    let started = Instant::now();
    driver
        .write_outputs(&[0x12, 0x34])
        .expect("warn policy should not fault the scan while worker writes");
    let elapsed = started.elapsed();

    assert!(
        elapsed < MODBUS_CI_SCAN_BOUND,
        "Modbus write_outputs must return within the scan-thread bound while the worker waits on the delayed response, elapsed={elapsed:?}"
    );
}

#[test]
fn modbus_stale_snapshot_is_returned_when_worker_reconnects() {
    let state = Arc::new(Mutex::new(ModbusTestState::with_registers(
        vec![0x1122],
        vec![0u16; 1],
    )));
    let addr = start_modbus_server(state, 1);
    let params: toml::Value = toml::from_str(&format!(
        "address = \"{addr}\"\nunit_id = 1\ninput_start = 0\noutput_start = 0\ntimeout_ms = 50\non_error = \"warn\"\n"
    ))
    .expect("params");
    let mut driver = ModbusTcpDriver::from_params(&params).expect("driver");
    let mut first = vec![0u8; 2];
    driver.read_inputs(&mut first).expect("initial read");
    assert_eq!(first, vec![0x11, 0x22]);

    let mut stale = vec![0u8; 2];
    driver
        .read_inputs(&mut stale)
        .expect("warn policy should return stale snapshot while reconnecting");

    assert_eq!(
        stale,
        vec![0x11, 0x22],
        "worker snapshot should preserve the last known input image while reconnecting"
    );
    assert!(matches!(
        driver.health(),
        trust_runtime::io::IoDriverHealth::Degraded { .. }
    ));
}

#[test]
fn modbus_cold_start_before_first_response_returns_bounded_no_data() {
    let delay = SLOW_SERVER;
    let state = Arc::new(Mutex::new(ModbusTestState::with_registers(
        vec![0x1122],
        vec![0u16; 1],
    )));
    let addr = start_delayed_modbus_server(state, delay);
    let params: toml::Value = toml::from_str(&format!(
        "address = \"{addr}\"\nunit_id = 1\ninput_start = 0\noutput_start = 0\ntimeout_ms = 2000\non_error = \"warn\"\n"
    ))
    .expect("params");
    let mut driver = ModbusTcpDriver::from_params(&params).expect("driver");
    let mut inputs = vec![0xAA, 0xBB];

    let started = Instant::now();
    driver
        .read_inputs(&mut inputs)
        .expect("warn policy should not fault cold-start no-data");
    let elapsed = started.elapsed();

    assert!(
        elapsed < MODBUS_CI_SCAN_BOUND,
        "cold-start read_inputs must return before the worker's first response, elapsed={elapsed:?}"
    );
    assert_eq!(
        inputs,
        vec![0xAA, 0xBB],
        "cold-start no-data must not invent a fresh input image"
    );
    assert!(matches!(
        driver.health(),
        trust_runtime::io::IoDriverHealth::Degraded { .. }
    ));
}

#[test]
fn modbus_cold_start_no_data_follows_on_error_policy() {
    for (policy, should_error) in [("fault", true), ("warn", false), ("ignore", false)] {
        let delay = SLOW_SERVER;
        let state = Arc::new(Mutex::new(ModbusTestState::with_registers(
            vec![0x1122],
            vec![0u16; 1],
        )));
        let addr = start_delayed_modbus_server(state, delay);
        let params: toml::Value = toml::from_str(&format!(
            "address = \"{addr}\"\nunit_id = 1\ninput_start = 0\noutput_start = 0\ntimeout_ms = 2000\non_error = \"{policy}\"\n"
        ))
        .expect("params");
        let mut driver = ModbusTcpDriver::from_params(&params).expect("driver");
        let mut inputs = vec![0u8; 2];

        let started = Instant::now();
        let result = driver.read_inputs(&mut inputs);
        let elapsed = started.elapsed();

        assert!(
            elapsed < MODBUS_CI_SCAN_BOUND,
            "{policy} cold-start no-data handling must be scan-bounded, elapsed={elapsed:?}"
        );
        assert_eq!(
            result.is_err(),
            should_error,
            "{policy} result should match on_error policy for cold-start no-data"
        );
        match (policy, driver.health()) {
            ("fault", trust_runtime::io::IoDriverHealth::Faulted { .. }) => {}
            ("warn" | "ignore", trust_runtime::io::IoDriverHealth::Degraded { .. }) => {}
            (_, other) => panic!("unexpected {policy} health for cold-start no-data: {other:?}"),
        }
    }
}

#[test]
fn modbus_reconnect_backoff_is_bounded_and_non_spinning() {
    let connection_count = Arc::new(AtomicUsize::new(0));
    let addr = start_closing_modbus_server(Arc::clone(&connection_count));
    let params: toml::Value = toml::from_str(&format!(
        "address = \"{addr}\"\nunit_id = 1\ninput_start = 0\noutput_start = 0\ntimeout_ms = 50\non_error = \"warn\"\n"
    ))
    .expect("params");
    let mut driver = ModbusTcpDriver::from_params(&params).expect("driver");
    let mut inputs = vec![0u8; 2];

    for _ in 0..20 {
        driver
            .read_inputs(&mut inputs)
            .expect("warn policy should degrade without faulting");
    }
    thread::sleep(StdDuration::from_millis(20));
    let attempts = connection_count.load(Ordering::SeqCst);

    assert!(
        attempts <= 4,
        "worker reconnect backoff must avoid scan-thread driven reconnect spinning, attempts={attempts}"
    );

    thread::sleep(StdDuration::from_millis(120));
    driver
        .read_inputs(&mut inputs)
        .expect("warn policy should retry without faulting");
    let retry_attempts = connection_count.load(Ordering::SeqCst);
    assert!(
        retry_attempts >= 2,
        "an unavailable endpoint must produce a reconnect attempt, attempts={retry_attempts}"
    );
}

#[test]
fn modbus_output_handoff_is_bounded_when_scan_outpaces_worker() {
    let delay = SLOW_SERVER;
    let state = Arc::new(Mutex::new(ModbusTestState::with_registers(
        vec![0u16; 1],
        vec![0u16; 1],
    )));
    let addr = start_delayed_modbus_server(state, delay);
    let params: toml::Value = toml::from_str(&format!(
        "address = \"{addr}\"\nunit_id = 1\ninput_start = 0\noutput_start = 0\ntimeout_ms = 2000\non_error = \"warn\"\n"
    ))
    .expect("params");
    let mut driver = ModbusTcpDriver::from_params(&params).expect("driver");

    let started = Instant::now();
    for value in 0..32u8 {
        driver
            .write_outputs(&[value, value.wrapping_add(1)])
            .expect("warn policy should not fault when worker handoff is backpressured");
    }
    let elapsed = started.elapsed();

    assert!(
        elapsed < MODBUS_CI_SCAN_BOUND,
        "output command handoff must stay bounded when the scan thread outpaces the Modbus worker, elapsed={elapsed:?}"
    );
}

/// "Dropping the driver requests worker shutdown and returns within the bounded driver-drop
/// deadline even when the worker is awaiting its first response": a read puts the first request on
/// the wire of a server that answers after a second, and the drop is timed alone. (The test dropped a
/// driver that had sent nothing; it now drops one whose worker awaits the response, the case the
/// specification names.)
#[test]
fn modbus_driver_drop_is_bounded_while_worker_waits_for_first_response() {
    let delay = SLOW_SERVER;
    let state = Arc::new(Mutex::new(ModbusTestState::with_registers(
        vec![0x1122],
        vec![0u16; 1],
    )));
    let addr = start_delayed_modbus_server(state, delay);
    let params: toml::Value = toml::from_str(&format!(
        "address = \"{addr}\"\nunit_id = 1\ninput_start = 0\noutput_start = 0\ntimeout_ms = 2000\non_error = \"warn\"\n"
    ))
    .expect("params");
    let mut driver = ModbusTcpDriver::from_params(&params).expect("driver");
    driver
        .read_inputs(&mut [0u8; 2])
        .expect("warn policy returns at the deadline with the request on the wire");

    let started = Instant::now();
    drop(driver);
    let elapsed = started.elapsed();

    assert!(
        elapsed < MODBUS_CI_SCAN_BOUND,
        "dropping the Modbus driver must not wait for a worker blocked on first response, elapsed={elapsed:?}"
    );
}
