use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration as StdDuration, Instant};

#[path = "support/modbus.rs"]
mod modbus_support;

use modbus_support::{
    start_closing_modbus_server, start_delayed_modbus_server, start_modbus_server, ModbusTestState,
};
use trust_runtime::io::{IoAddress, IoDriver, IoSafeState, ModbusTcpDriver};
use trust_runtime::value::Value;
use trust_runtime::Runtime;

const MODBUS_CI_SCAN_BOUND: StdDuration = StdDuration::from_millis(150);

#[test]
fn modbus_driver_reads_and_writes_default_register_functions() {
    let state = Arc::new(Mutex::new(ModbusTestState::with_registers(
        vec![0x1122, 0x3344, 0, 0],
        vec![0u16; 4],
    )));
    let addr = start_modbus_server(Arc::clone(&state), 2);
    let params: toml::Value = toml::from_str(&format!(
        "address = \"{addr}\"\nunit_id = 1\ninput_start = 0\noutput_start = 0\n"
    ))
    .expect("params");
    let mut driver = ModbusTcpDriver::from_params(&params).expect("driver");
    let mut inputs = vec![0u8; 4];
    driver.read_inputs(&mut inputs).expect("read inputs");
    assert_eq!(inputs, vec![0x11, 0x22, 0x33, 0x44]);

    let outputs = vec![0xAA, 0xBB, 0xCC, 0xDD];
    driver.write_outputs(&outputs).expect("write outputs");
    let guard = state.lock().expect("modbus state lock");
    assert_eq!(guard.holding_registers[0], 0xAABB);
    assert_eq!(guard.holding_registers[1], 0xCCDD);
    assert_eq!(guard.functions, vec![0x04, 0x10]);
}

#[test]
fn modbus_raw_register_mode_uses_wire_big_endian_order() {
    let state = Arc::new(Mutex::new(ModbusTestState::with_registers(
        vec![0x1234, 0xABCD],
        vec![0u16; 2],
    )));
    let addr = start_modbus_server(Arc::clone(&state), 2);
    let params: toml::Value = toml::from_str(&format!(
        "address = \"{addr}\"\nunit_id = 1\ninput_start = 0\noutput_start = 0\n"
    ))
    .expect("params");
    let mut driver = ModbusTcpDriver::from_params(&params).expect("driver");
    let mut inputs = vec![0u8; 4];

    driver.read_inputs(&mut inputs).expect("read raw registers");
    assert_eq!(
        inputs,
        vec![0x12, 0x34, 0xAB, 0xCD],
        "raw Modbus register reads must preserve big-endian wire byte order"
    );

    driver
        .write_outputs(&[0x55, 0xAA, 0xC0, 0xDE])
        .expect("write raw registers");
    let guard = state.lock().expect("modbus state lock");
    assert_eq!(guard.holding_registers[0], 0x55AA);
    assert_eq!(guard.holding_registers[1], 0xC0DE);
    assert_eq!(guard.functions, vec![0x04, 0x10]);
}

#[test]
fn modbus_explicit_input_functions_cover_fc01_fc02_fc03() {
    let state = Arc::new(Mutex::new(ModbusTestState {
        coils: vec![true, false, true, true, false, false, false, true],
        ..ModbusTestState::default()
    }));
    let addr = start_modbus_server(Arc::clone(&state), 1);
    let params: toml::Value = toml::from_str(&format!(
        "address = \"{addr}\"\nunit_id = 1\ninput_start = 0\ninput_function = \"read_coils\"\n"
    ))
    .expect("params");
    let mut driver = ModbusTcpDriver::from_params(&params).expect("driver");
    let mut inputs = vec![0u8; 1];
    driver.read_inputs(&mut inputs).expect("read coils");
    assert_eq!(inputs, vec![0x8D]);
    assert_eq!(
        state.lock().expect("modbus state lock").functions,
        vec![0x01]
    );

    let state = Arc::new(Mutex::new(ModbusTestState {
        discrete_inputs: vec![false, true, false, true, true, false, false, false],
        ..ModbusTestState::default()
    }));
    let addr = start_modbus_server(Arc::clone(&state), 1);
    let params: toml::Value = toml::from_str(&format!(
        "address = \"{addr}\"\nunit_id = 1\ninput_start = 0\ninput_function = \"read_discrete_inputs\"\n"
    ))
    .expect("params");
    let mut driver = ModbusTcpDriver::from_params(&params).expect("driver");
    let mut inputs = vec![0u8; 1];
    driver
        .read_inputs(&mut inputs)
        .expect("read discrete inputs");
    assert_eq!(inputs, vec![0x1A]);
    assert_eq!(
        state.lock().expect("modbus state lock").functions,
        vec![0x02]
    );

    let state = Arc::new(Mutex::new(ModbusTestState::with_registers(
        vec![0u16; 1],
        vec![0x5566],
    )));
    let addr = start_modbus_server(Arc::clone(&state), 1);
    let params: toml::Value = toml::from_str(&format!(
        "address = \"{addr}\"\nunit_id = 1\ninput_start = 0\ninput_function = \"read_holding_registers\"\n"
    ))
    .expect("params");
    let mut driver = ModbusTcpDriver::from_params(&params).expect("driver");
    let mut inputs = vec![0u8; 2];
    driver
        .read_inputs(&mut inputs)
        .expect("read holding registers");
    assert_eq!(inputs, vec![0x55, 0x66]);
    assert_eq!(
        state.lock().expect("modbus state lock").functions,
        vec![0x03]
    );
}

#[test]
fn modbus_explicit_output_functions_cover_fc05_fc06_fc15_fc16() {
    let state = Arc::new(Mutex::new(ModbusTestState {
        coils: vec![false; 8],
        ..ModbusTestState::default()
    }));
    let addr = start_modbus_server(Arc::clone(&state), 1);
    let params: toml::Value = toml::from_str(&format!(
        "address = \"{addr}\"\nunit_id = 1\noutput_start = 2\noutput_function = \"write_single_coil\"\n"
    ))
    .expect("params");
    let mut driver = ModbusTcpDriver::from_params(&params).expect("driver");
    driver.write_outputs(&[0x01]).expect("write single coil");
    let guard = state.lock().expect("modbus state lock");
    assert!(guard.coils[2]);
    assert_eq!(guard.functions, vec![0x05]);
    drop(guard);

    let state = Arc::new(Mutex::new(ModbusTestState::with_registers(
        vec![0u16; 1],
        vec![0u16; 3],
    )));
    let addr = start_modbus_server(Arc::clone(&state), 1);
    let params: toml::Value = toml::from_str(&format!(
        "address = \"{addr}\"\nunit_id = 1\noutput_start = 1\noutput_function = \"write_single_register\"\n"
    ))
    .expect("params");
    let mut driver = ModbusTcpDriver::from_params(&params).expect("driver");
    driver
        .write_outputs(&[0x12, 0x34])
        .expect("write single register");
    let guard = state.lock().expect("modbus state lock");
    assert_eq!(guard.holding_registers[1], 0x1234);
    assert_eq!(guard.functions, vec![0x06]);
    drop(guard);

    let state = Arc::new(Mutex::new(ModbusTestState {
        coils: vec![false; 12],
        ..ModbusTestState::default()
    }));
    let addr = start_modbus_server(Arc::clone(&state), 1);
    let params: toml::Value = toml::from_str(&format!(
        "address = \"{addr}\"\nunit_id = 1\noutput_start = 1\noutput_function = \"write_multiple_coils\"\n"
    ))
    .expect("params");
    let mut driver = ModbusTcpDriver::from_params(&params).expect("driver");
    driver
        .write_outputs(&[0b0000_0101])
        .expect("write multiple coils");
    let guard = state.lock().expect("modbus state lock");
    assert!(guard.coils[1]);
    assert!(!guard.coils[2]);
    assert!(guard.coils[3]);
    assert_eq!(guard.functions, vec![0x0F]);
    drop(guard);

    let state = Arc::new(Mutex::new(ModbusTestState::with_registers(
        vec![0u16; 1],
        vec![0u16; 3],
    )));
    let addr = start_modbus_server(Arc::clone(&state), 1);
    let params: toml::Value = toml::from_str(&format!(
        "address = \"{addr}\"\nunit_id = 1\noutput_start = 0\noutput_function = \"write_multiple_registers\"\n"
    ))
    .expect("params");
    let mut driver = ModbusTcpDriver::from_params(&params).expect("driver");
    driver
        .write_outputs(&[0xAA, 0xBB, 0xCC])
        .expect("write multiple registers");
    let guard = state.lock().expect("modbus state lock");
    assert_eq!(guard.holding_registers[0], 0xAABB);
    assert_eq!(guard.holding_registers[1], 0xCC00);
    assert_eq!(guard.functions, vec![0x10]);
}

#[test]
fn modbus_point_map_reads_scaled_registers_and_coils() {
    let state = Arc::new(Mutex::new(ModbusTestState {
        coils: vec![false, false, true, false],
        holding_registers: vec![0xE803, 0x0000],
        ..ModbusTestState::default()
    }));
    let addr = start_modbus_server(Arc::clone(&state), 2);
    let params: toml::Value = toml::from_str(&format!(
        r#"
address = "{addr}"
unit_id = 1

[[input_points]]
image_offset = 0
image_bit = 3
address = 2
function = "read_coils"
data_type = "bool"

[[input_points]]
image_offset = 1
address = 0
function = "read_holding_registers"
data_type = "u32"
scale = 0.5
offset = 1.0
byte_order = "little"
word_order = "little"
"#
    ))
    .expect("params");
    let mut driver = ModbusTcpDriver::from_params(&params).expect("driver");
    let mut inputs = vec![0u8; 5];

    driver.read_inputs(&mut inputs).expect("read mapped inputs");

    assert_eq!(inputs[0], 0b0000_1000);
    assert_eq!(&inputs[1..5], &501u32.to_le_bytes());
    assert_eq!(
        state.lock().expect("modbus state lock").functions,
        vec![0x01, 0x03]
    );
}

#[test]
fn modbus_point_map_writes_scaled_registers_and_coils() {
    let state = Arc::new(Mutex::new(ModbusTestState {
        coils: vec![false; 8],
        holding_registers: vec![0u16; 4],
        ..ModbusTestState::default()
    }));
    let addr = start_modbus_server(Arc::clone(&state), 3);
    let params: toml::Value = toml::from_str(&format!(
        r#"
address = "{addr}"
unit_id = 1

[[output_points]]
image_offset = 0
image_bit = 1
address = 2
function = "write_multiple_coils"
data_type = "bool"

[[output_points]]
image_offset = 1
address = 0
function = "write_multiple_registers"
data_type = "u32"
scale = 0.5
offset = 1.0
byte_order = "little"
word_order = "little"

[[output_points]]
image_offset = 5
address = 3
function = "write_single_register"
data_type = "u16"
scale = 0.5
offset = 10.0
byte_order = "little"
"#
    ))
    .expect("params");
    let mut driver = ModbusTcpDriver::from_params(&params).expect("driver");
    let mut outputs = vec![0u8; 7];
    outputs[0] = 0b0000_0010;
    outputs[1..5].copy_from_slice(&501u32.to_le_bytes());
    outputs[5..7].copy_from_slice(&60u16.to_le_bytes());

    driver
        .write_outputs(&outputs)
        .expect("write mapped outputs");

    let deadline = Instant::now() + StdDuration::from_millis(500);
    loop {
        let guard = state.lock().expect("modbus state lock");
        let written = guard.coils[2]
            && guard.holding_registers[0] == 0xE803
            && guard.holding_registers[1] == 0x0000
            && guard.holding_registers[3] == 0x6400
            && guard.functions == vec![0x0F, 0x10, 0x06];
        if written {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "mapped Modbus output handoff should eventually reach device, state={guard:?}"
        );
        drop(guard);
        thread::sleep(StdDuration::from_millis(5));
    }

    let guard = state.lock().expect("modbus state lock");
    assert!(guard.coils[2]);
    assert_eq!(guard.holding_registers[0], 0xE803);
    assert_eq!(guard.holding_registers[1], 0x0000);
    assert_eq!(guard.holding_registers[3], 0x6400);
    assert_eq!(guard.functions, vec![0x0F, 0x10, 0x06]);
}

#[test]
fn modbus_point_map_rejects_invalid_type_and_scaling_config() {
    let params: toml::Value = toml::from_str(
        r#"
address = "127.0.0.1:65000"

[[input_points]]
image_offset = 0
address = 0
function = "read_coils"
data_type = "u16"
"#,
    )
    .expect("params");
    let err = ModbusTcpDriver::from_params(&params).expect_err("coil map type must fail");
    assert!(
        err.to_string().contains("input_points"),
        "unexpected error: {err}"
    );

    let params: toml::Value = toml::from_str(
        r#"
address = "127.0.0.1:65000"

[[output_points]]
image_offset = 0
address = 0
function = "write_single_register"
data_type = "u16"
scale = 0.0
"#,
    )
    .expect("params");
    let err = ModbusTcpDriver::from_params(&params).expect_err("zero scale must fail");
    assert!(err.to_string().contains("scale"), "unexpected error: {err}");
}

#[test]
fn modbus_rejects_unknown_function_config() {
    let params: toml::Value =
        toml::from_str("address = \"127.0.0.1:65000\"\ninput_function = \"read_magic\"\n")
            .expect("params");
    let err = ModbusTcpDriver::from_params(&params).expect_err("invalid function must fail");
    assert!(
        err.to_string().contains("io.params.input_function"),
        "unexpected error: {err}"
    );
}

#[test]
fn modbus_driver_warn_policy_degrades_without_transport_error() {
    let params: toml::Value = toml::from_str(
        "address = \"127.0.0.1:65000\"\nunit_id = 1\ninput_start = 0\noutput_start = 0\non_error = \"warn\"\n",
    )
    .expect("params");
    let mut driver = ModbusTcpDriver::from_params(&params).expect("driver");
    let mut inputs = vec![0u8; 2];
    let result = driver.read_inputs(&mut inputs);
    assert!(
        result.is_ok(),
        "warn policy should degrade health without faulting the scan"
    );
    let health = driver.health();
    match health {
        trust_runtime::io::IoDriverHealth::Degraded { .. } => {}
        other => panic!("expected degraded health, got {other:?}"),
    }
}

#[test]
fn modbus_driver_ignore_policy_degrades_without_transport_error() {
    let params: toml::Value = toml::from_str(
        "address = \"127.0.0.1:65000\"\nunit_id = 1\ninput_start = 0\noutput_start = 0\non_error = \"ignore\"\n",
    )
    .expect("params");
    let mut driver = ModbusTcpDriver::from_params(&params).expect("driver");
    let mut inputs = vec![0u8; 2];
    let result = driver.read_inputs(&mut inputs);
    assert!(
        result.is_ok(),
        "ignore policy should degrade health without faulting the scan"
    );
    let health = driver.health();
    match health {
        trust_runtime::io::IoDriverHealth::Degraded { .. } => {}
        other => panic!("expected degraded health, got {other:?}"),
    }
}

#[test]
fn modbus_driver_write_failure_follows_on_error_policy() {
    for (policy, should_error) in [("fault", true), ("warn", false), ("ignore", false)] {
        let state = Arc::new(Mutex::new(ModbusTestState::with_registers(
            vec![0u16; 1],
            vec![],
        )));
        let addr = start_modbus_server(state, 1);
        let params: toml::Value = toml::from_str(&format!(
            "address = \"{addr}\"\nunit_id = 1\ninput_start = 0\noutput_start = 0\ntimeout_ms = 1000\non_error = \"{policy}\"\n"
        ))
        .expect("params");
        let mut driver = ModbusTcpDriver::from_params(&params).expect("driver");

        let result = driver.write_outputs(&[0x12, 0x34]);

        if should_error && result.is_err() {
            // Fast platforms can observe the worker-completed fault in the scan wait.
        } else {
            assert!(
                result.is_ok(),
                "{policy} write handoff should be bounded while worker reports the Modbus exception"
            );
        }
        let health = wait_for_modbus_health(&driver, policy);
        match (policy, health) {
            ("fault", trust_runtime::io::IoDriverHealth::Faulted { .. }) => {}
            ("warn" | "ignore", trust_runtime::io::IoDriverHealth::Degraded { .. }) => {}
            (_, other) => panic!("unexpected {policy} health after write failure: {other:?}"),
        }
    }
}

fn wait_for_modbus_health(
    driver: &ModbusTcpDriver,
    policy: &str,
) -> trust_runtime::io::IoDriverHealth {
    let deadline = Instant::now() + StdDuration::from_millis(500);
    loop {
        let health = driver.health();
        let expected = matches!(
            (policy, &health),
            ("fault", trust_runtime::io::IoDriverHealth::Faulted { .. })
                | (
                    "warn" | "ignore",
                    trust_runtime::io::IoDriverHealth::Degraded { .. },
                )
        );
        if expected {
            return health;
        }
        assert!(
            Instant::now() < deadline,
            "{policy} Modbus write failure did not reach expected health state, last={health:?}"
        );
        thread::sleep(StdDuration::from_millis(5));
    }
}

#[test]
fn modbus_read_inputs_returns_within_scan_bound_while_response_delayed() {
    let delay = StdDuration::from_millis(120);
    let state = Arc::new(Mutex::new(ModbusTestState::with_registers(
        vec![0x1122],
        vec![0u16; 1],
    )));
    let addr = start_delayed_modbus_server(state, delay);
    let params: toml::Value = toml::from_str(&format!(
        "address = \"{addr}\"\nunit_id = 1\ninput_start = 0\noutput_start = 0\ntimeout_ms = 1000\non_error = \"warn\"\n"
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
    let delay = StdDuration::from_millis(120);
    let state = Arc::new(Mutex::new(ModbusTestState::with_registers(
        vec![0u16; 1],
        vec![0u16; 1],
    )));
    let addr = start_delayed_modbus_server(state, delay);
    let params: toml::Value = toml::from_str(&format!(
        "address = \"{addr}\"\nunit_id = 1\ninput_start = 0\noutput_start = 0\ntimeout_ms = 1000\non_error = \"warn\"\n"
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
    let delay = StdDuration::from_millis(120);
    let state = Arc::new(Mutex::new(ModbusTestState::with_registers(
        vec![0x1122],
        vec![0u16; 1],
    )));
    let addr = start_delayed_modbus_server(state, delay);
    let params: toml::Value = toml::from_str(&format!(
        "address = \"{addr}\"\nunit_id = 1\ninput_start = 0\noutput_start = 0\ntimeout_ms = 1000\non_error = \"warn\"\n"
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
        let delay = StdDuration::from_millis(120);
        let state = Arc::new(Mutex::new(ModbusTestState::with_registers(
            vec![0x1122],
            vec![0u16; 1],
        )));
        let addr = start_delayed_modbus_server(state, delay);
        let params: toml::Value = toml::from_str(&format!(
            "address = \"{addr}\"\nunit_id = 1\ninput_start = 0\noutput_start = 0\ntimeout_ms = 1000\non_error = \"{policy}\"\n"
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
}

#[test]
fn modbus_output_handoff_is_bounded_when_scan_outpaces_worker() {
    let delay = StdDuration::from_millis(120);
    let state = Arc::new(Mutex::new(ModbusTestState::with_registers(
        vec![0u16; 1],
        vec![0u16; 1],
    )));
    let addr = start_delayed_modbus_server(state, delay);
    let params: toml::Value = toml::from_str(&format!(
        "address = \"{addr}\"\nunit_id = 1\ninput_start = 0\noutput_start = 0\ntimeout_ms = 1000\non_error = \"warn\"\n"
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

#[test]
fn modbus_safe_state_handoff_is_bounded_and_reaches_delayed_device() {
    let delay = StdDuration::from_millis(120);
    let state = Arc::new(Mutex::new(ModbusTestState::with_registers(
        vec![0u16; 1],
        vec![0u16; 1],
    )));
    let addr = start_delayed_modbus_server(Arc::clone(&state), delay);
    let params: toml::Value = toml::from_str(&format!(
        "address = \"{addr}\"\nunit_id = 1\ninput_start = 0\noutput_start = 0\ntimeout_ms = 1000\non_error = \"warn\"\n"
    ))
    .expect("params");
    let mut runtime = Runtime::new();
    runtime.io_mut().resize(0, 2, 0);
    let mut safe_state = IoSafeState::default();
    safe_state.outputs.push((
        IoAddress::parse("%QW0").expect("safe-state output word"),
        Value::Word(0x1234),
    ));
    runtime.set_io_safe_state(safe_state);
    runtime.add_io_driver(
        "modbus-safe-state",
        Box::new(ModbusTcpDriver::from_params(&params).expect("driver")),
    );

    let started = Instant::now();
    runtime
        .apply_io_safe_state()
        .expect("warn policy should not hide safe-state handoff");
    let elapsed = started.elapsed();

    assert!(
        elapsed < MODBUS_CI_SCAN_BOUND,
        "safe-state output handoff must return within the scan bound while the worker writes, elapsed={elapsed:?}"
    );

    let deadline = Instant::now() + StdDuration::from_millis(500);
    loop {
        let value = state
            .lock()
            .expect("modbus state lock")
            .holding_registers
            .first()
            .copied()
            .unwrap_or(0);
        if value == 0x3412 {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "safe-state output should eventually reach the delayed Modbus device, got 0x{value:04x}"
        );
        thread::sleep(StdDuration::from_millis(5));
    }
}

#[test]
fn modbus_driver_drop_is_bounded_while_worker_waits_for_first_response() {
    let delay = StdDuration::from_millis(120);
    let state = Arc::new(Mutex::new(ModbusTestState::with_registers(
        vec![0x1122],
        vec![0u16; 1],
    )));
    let addr = start_delayed_modbus_server(state, delay);
    let params: toml::Value = toml::from_str(&format!(
        "address = \"{addr}\"\nunit_id = 1\ninput_start = 0\noutput_start = 0\ntimeout_ms = 1000\non_error = \"warn\"\n"
    ))
    .expect("params");

    let started = Instant::now();
    let driver = ModbusTcpDriver::from_params(&params).expect("driver");
    drop(driver);
    let elapsed = started.elapsed();

    assert!(
        elapsed < MODBUS_CI_SCAN_BOUND,
        "dropping the Modbus driver must not wait for a worker blocked on first response, elapsed={elapsed:?}"
    );
}

#[test]
#[ignore = "red test for runtime-safety fail-closed Phase 1"]
fn modbus_exception_is_not_reported_as_generic_transport() {
    let state = Arc::new(Mutex::new(ModbusTestState::with_registers(
        vec![0u16; 1],
        vec![0u16; 1],
    )));
    let addr = start_modbus_server(state, 1);
    let params: toml::Value = toml::from_str(&format!(
        "address = \"{addr}\"\nunit_id = 1\ninput_start = 7\noutput_start = 0\n"
    ))
    .expect("params");
    let mut driver = ModbusTcpDriver::from_params(&params).expect("driver");
    let mut inputs = vec![0u8; 2];

    let err = driver
        .read_inputs(&mut inputs)
        .expect_err("Modbus exception must be observable");
    let text = err.to_string();
    assert!(
        text.contains("exception"),
        "expected Modbus exception, got {err}"
    );
    assert!(
        !text.starts_with("i/o driver error"),
        "Modbus exception should not use the generic transport error variant: {err}"
    );
}
