use std::io::{ErrorKind, Read, Write};
use std::net::{Shutdown, SocketAddr, TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration as StdDuration, Instant};

use serde::Serialize;
use sha2::{Digest, Sha256};
use trust_runtime::config::IoDriverConfig;
use trust_runtime::eval::expr::{Expr, LValue};
use trust_runtime::eval::stmt::Stmt;
use trust_runtime::io::{IoAddress, IoDriverRegistry};
use trust_runtime::task::{ProgramDef, TaskConfig};
use trust_runtime::value::{Duration, Value};
use trust_runtime::Runtime;

const INPUT_ADDRESS: &str = "%IW0";
const OUTPUT_ADDRESS: &str = "%QW0";
const INPUT_REGISTER: u16 = 0;
const OUTPUT_REGISTER: u16 = 0;
const REGISTER_VALUE: u16 = 0x3434;
const EXPECTED_REQUESTS: usize = 2;

#[test]
fn trust_twin_io_boundary_modbus_runtime_cycle_is_deterministic() {
    assert_modbus_address_policy();

    let first = run_boundary_scenario().expect("first IO-boundary scenario");
    let second = run_boundary_scenario().expect("second IO-boundary scenario");

    assert_eq!(
        first.stable_trace, second.stable_trace,
        "stable IO-boundary trace must match across consecutive runs"
    );
    assert_eq!(
        first.stable_hash, second.stable_hash,
        "stable IO-boundary trace hash must match across consecutive runs"
    );
    assert!(
        first.port_released && second.port_released,
        "simulator ports must be released after teardown"
    );

    write_gate_artifact(&first, &second).expect("write trust-twin IO-boundary artifact");
}

fn assert_modbus_address_policy() {
    let registry = IoDriverRegistry::default_registry();
    let loopback = modbus_params_for("127.0.0.1:502");
    registry
        .validate("modbus-tcp", &loopback)
        .expect("numeric loopback SocketAddr must validate");

    let hostname = modbus_params_for("localhost:502");
    assert!(
        registry.validate("modbus-tcp", &hostname).is_err(),
        "Modbus TCP config intentionally requires a numeric SocketAddr, not a hostname"
    );
}

fn run_boundary_scenario() -> Result<ScenarioRun, String> {
    let simulator = ModbusSimulator::start(REGISTER_VALUE)?;
    let mut runtime = runtime_copying_input_word_to_output_word();

    let driver_config = IoDriverConfig {
        name: "modbus-tcp".into(),
        params: modbus_params_for(&simulator.addr().to_string()),
    };
    let registry = IoDriverRegistry::default_registry();
    registry
        .validate(driver_config.name.as_str(), &driver_config.params)
        .map_err(|err| format!("validate modbus driver config: {err}"))?;
    let spec = registry
        .build(driver_config.name.as_str(), &driver_config.params)
        .map_err(|err| format!("build modbus driver: {err}"))?
        .ok_or_else(|| "modbus driver config unexpectedly disabled IO".to_string())?;
    runtime.add_io_driver(spec.name, spec.driver);

    runtime
        .storage_mut()
        .set_global("trigger", Value::Bool(true));

    let cycle_start = Instant::now();
    runtime
        .execute_cycle()
        .map_err(|err| format!("execute runtime cycle: {err}"))?;
    let cycle_ms = cycle_start.elapsed().as_secs_f64() * 1000.0;

    let observed_input = runtime
        .storage()
        .get_global("input_word")
        .cloned()
        .ok_or_else(|| "missing input_word global after cycle".to_string())?;
    assert_eq!(observed_input, Value::Word(REGISTER_VALUE));

    let output_value = simulator.output_register(OUTPUT_REGISTER as usize)?;
    assert_eq!(output_value, REGISTER_VALUE);
    assert_eq!(
        runtime
            .io()
            .read(&IoAddress::parse(OUTPUT_ADDRESS).expect("output address"))
            .map_err(|err| format!("read runtime output image: {err}"))?,
        Value::Word(REGISTER_VALUE)
    );

    runtime.clear_io_drivers();
    drop(runtime);

    let request_count = simulator.request_count();
    assert_eq!(
        request_count, EXPECTED_REQUESTS,
        "one cycle should issue one Modbus read and one Modbus write"
    );
    let request_log = simulator.request_log();
    let simulator_addr = simulator.addr();
    simulator.stop()?;
    let port_released = port_is_released(simulator_addr);

    let stable_trace = StableTrace {
        driver_init: "ok",
        read_inputs: IoEvidence {
            address: INPUT_ADDRESS,
            modbus_register: INPUT_REGISTER,
            value: REGISTER_VALUE,
        },
        write_outputs: IoEvidence {
            address: OUTPUT_ADDRESS,
            modbus_register: OUTPUT_REGISTER,
            value: output_value,
        },
        request_count,
        request_log,
        evidence_blockers: Vec::new(),
    };
    let stable_hash = hash_trace(&stable_trace)?;

    Ok(ScenarioRun {
        stable_trace,
        stable_hash,
        cycle_ms,
        bound_port: simulator_addr.port(),
        port_released,
    })
}

fn runtime_copying_input_word_to_output_word() -> Runtime {
    let mut runtime = Runtime::new();
    runtime.io_mut().resize(2, 2, 0);
    runtime
        .storage_mut()
        .set_global("input_word", Value::Word(0));
    runtime
        .storage_mut()
        .set_global("output_word", Value::Word(0));
    runtime
        .storage_mut()
        .set_global("trigger", Value::Bool(false));

    runtime
        .register_program(ProgramDef {
            name: "TwinBoundary".into(),
            vars: Vec::new(),
            temps: Vec::new(),
            using: Vec::new(),
            body: vec![Stmt::Assign {
                target: LValue::Name("output_word".into()),
                value: Expr::Name("input_word".into()),
                location: None,
            }],
        })
        .expect("register trust-twin boundary program");
    runtime.io_mut().bind(
        "input_word",
        IoAddress::parse(INPUT_ADDRESS).expect("parse input address"),
    );
    runtime.io_mut().bind(
        "output_word",
        IoAddress::parse(OUTPUT_ADDRESS).expect("parse output address"),
    );
    runtime.register_task(TaskConfig {
        name: "TwinBoundaryTask".into(),
        interval: Duration::ZERO,
        single: Some("trigger".into()),
        priority: 0,
        programs: vec!["TwinBoundary".into()],
        fb_instances: Vec::new(),
    });
    runtime
}

fn modbus_params_for(address: &str) -> toml::Value {
    toml::from_str(&format!(
        "address = \"{address}\"\nunit_id = 1\ninput_start = {INPUT_REGISTER}\noutput_start = {OUTPUT_REGISTER}\ntimeout_ms = 1000\n"
    ))
    .expect("modbus params")
}

fn write_gate_artifact(first: &ScenarioRun, second: &ScenarioRun) -> Result<(), String> {
    let artifact = GateArtifact {
        driver_init: "ok",
        read_inputs: first.stable_trace.read_inputs.clone(),
        write_outputs: first.stable_trace.write_outputs.clone(),
        cycle_ms: CycleMeasurements {
            run_1: first.cycle_ms,
            run_2: second.cycle_ms,
        },
        determinism_trace_hash: first.stable_hash.clone(),
        evidence_blockers: Vec::new(),
        stable_trace_match: first.stable_trace == second.stable_trace,
        runs: vec![
            RunArtifact {
                run: 1,
                request_count: first.stable_trace.request_count,
                bound_port: first.bound_port,
                port_released: first.port_released,
                determinism_trace_hash: first.stable_hash.clone(),
            },
            RunArtifact {
                run: 2,
                request_count: second.stable_trace.request_count,
                bound_port: second.bound_port,
                port_released: second.port_released,
                determinism_trace_hash: second.stable_hash.clone(),
            },
        ],
    };

    let path = workspace_root()
        .join("target")
        .join("gate-artifacts")
        .join("trust-twin-io-boundary.json");
    let parent = path
        .parent()
        .ok_or_else(|| format!("artifact path has no parent: {}", path.display()))?;
    std::fs::create_dir_all(parent)
        .map_err(|err| format!("create artifact dir {}: {err}", parent.display()))?;
    let json = serde_json::to_string_pretty(&artifact)
        .map_err(|err| format!("serialize gate artifact: {err}"))?;
    std::fs::write(&path, format!("{json}\n"))
        .map_err(|err| format!("write {}: {err}", path.display()))?;
    Ok(())
}

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(std::path::Path::parent)
        .expect("workspace root")
        .to_path_buf()
}

fn hash_trace(trace: &StableTrace) -> Result<String, String> {
    let bytes =
        serde_json::to_vec(trace).map_err(|err| format!("serialize stable trace: {err}"))?;
    let digest = Sha256::digest(bytes);
    Ok(format!("{digest:x}"))
}

fn port_is_released(addr: SocketAddr) -> bool {
    TcpListener::bind(addr).is_ok()
}

#[derive(Debug)]
struct ScenarioRun {
    stable_trace: StableTrace,
    stable_hash: String,
    cycle_ms: f64,
    bound_port: u16,
    port_released: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
struct StableTrace {
    driver_init: &'static str,
    read_inputs: IoEvidence,
    write_outputs: IoEvidence,
    request_count: usize,
    request_log: Vec<ObservedRequest>,
    evidence_blockers: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
struct IoEvidence {
    address: &'static str,
    modbus_register: u16,
    value: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
struct ObservedRequest {
    function: &'static str,
    address: u16,
    quantity: u16,
    values: Vec<u16>,
}

#[derive(Debug, Serialize)]
struct GateArtifact {
    driver_init: &'static str,
    read_inputs: IoEvidence,
    write_outputs: IoEvidence,
    cycle_ms: CycleMeasurements,
    determinism_trace_hash: String,
    evidence_blockers: Vec<String>,
    stable_trace_match: bool,
    runs: Vec<RunArtifact>,
}

#[derive(Debug, Serialize)]
struct CycleMeasurements {
    run_1: f64,
    run_2: f64,
}

#[derive(Debug, Serialize)]
struct RunArtifact {
    run: u8,
    request_count: usize,
    bound_port: u16,
    port_released: bool,
    determinism_trace_hash: String,
}

struct ModbusSimulator {
    addr: SocketAddr,
    state: Arc<Mutex<SimulatorState>>,
    stop: Arc<AtomicBool>,
    request_count: Arc<AtomicUsize>,
    handle: Option<JoinHandle<Result<(), String>>>,
}

impl ModbusSimulator {
    fn start(input_value: u16) -> Result<Self, String> {
        let listener =
            TcpListener::bind("127.0.0.1:0").map_err(|err| format!("bind simulator: {err}"))?;
        listener
            .set_nonblocking(true)
            .map_err(|err| format!("set simulator nonblocking: {err}"))?;
        let addr = listener
            .local_addr()
            .map_err(|err| format!("read simulator addr: {err}"))?;
        let state = Arc::new(Mutex::new(SimulatorState::new(input_value)));
        let stop = Arc::new(AtomicBool::new(false));
        let request_count = Arc::new(AtomicUsize::new(0));
        let thread_state = Arc::clone(&state);
        let thread_stop = Arc::clone(&stop);
        let thread_request_count = Arc::clone(&request_count);
        let handle = thread::spawn(move || {
            run_modbus_server(listener, thread_state, thread_stop, thread_request_count)
        });
        Ok(Self {
            addr,
            state,
            stop,
            request_count,
            handle: Some(handle),
        })
    }

    fn addr(&self) -> SocketAddr {
        self.addr
    }

    fn output_register(&self, index: usize) -> Result<u16, String> {
        let guard = self
            .state
            .lock()
            .map_err(|_| "simulator state lock poisoned".to_string())?;
        guard
            .output_registers
            .get(index)
            .copied()
            .ok_or_else(|| format!("missing output register {index}"))
    }

    fn request_count(&self) -> usize {
        self.request_count.load(Ordering::SeqCst)
    }

    fn request_log(&self) -> Vec<ObservedRequest> {
        self.state
            .lock()
            .expect("simulator state lock")
            .requests
            .clone()
    }

    fn stop(mut self) -> Result<(), String> {
        self.stop.store(true, Ordering::SeqCst);
        let _ = TcpStream::connect_timeout(&self.addr, StdDuration::from_millis(100));
        if let Some(handle) = self.handle.take() {
            match handle.join() {
                Ok(result) => result,
                Err(_) => Err("simulator thread panicked".to_string()),
            }
        } else {
            Ok(())
        }
    }
}

struct SimulatorState {
    input_registers: Vec<u16>,
    output_registers: Vec<u16>,
    requests: Vec<ObservedRequest>,
}

impl SimulatorState {
    fn new(input_value: u16) -> Self {
        let mut input_registers = vec![0; 4];
        input_registers[INPUT_REGISTER as usize] = input_value;
        Self {
            input_registers,
            output_registers: vec![0; 4],
            requests: Vec::new(),
        }
    }
}

fn run_modbus_server(
    listener: TcpListener,
    state: Arc<Mutex<SimulatorState>>,
    stop: Arc<AtomicBool>,
    request_count: Arc<AtomicUsize>,
) -> Result<(), String> {
    while !stop.load(Ordering::SeqCst) {
        match listener.accept() {
            Ok((mut stream, _)) => {
                if stop.load(Ordering::SeqCst) {
                    let _ = stream.shutdown(Shutdown::Both);
                    break;
                }
                stream
                    .set_nonblocking(false)
                    .map_err(|err| format!("set simulator client blocking: {err}"))?;
                stream
                    .set_read_timeout(Some(StdDuration::from_secs(1)))
                    .map_err(|err| format!("set read timeout: {err}"))?;
                stream
                    .set_write_timeout(Some(StdDuration::from_secs(1)))
                    .map_err(|err| format!("set write timeout: {err}"))?;
                handle_connection(&mut stream, &state, &stop, &request_count)?;
            }
            Err(err) if err.kind() == ErrorKind::WouldBlock => {
                thread::sleep(StdDuration::from_millis(5));
            }
            Err(err) => return Err(format!("accept simulator connection: {err}")),
        }
    }
    Ok(())
}

fn handle_connection(
    stream: &mut TcpStream,
    state: &Arc<Mutex<SimulatorState>>,
    stop: &Arc<AtomicBool>,
    request_count: &Arc<AtomicUsize>,
) -> Result<(), String> {
    loop {
        if stop.load(Ordering::SeqCst) {
            return Ok(());
        }
        let Some(frame) = read_request_frame(stream)? else {
            return Ok(());
        };
        let response = handle_request(&frame, state)?;
        request_count.fetch_add(1, Ordering::SeqCst);
        write_response_frame(stream, &frame, &response)?;
    }
}

struct RequestFrame {
    tx: u16,
    unit_id: u8,
    pdu: Vec<u8>,
}

fn read_request_frame(stream: &mut TcpStream) -> Result<Option<RequestFrame>, String> {
    let mut header = [0u8; 6];
    if let Err(err) = stream.read_exact(&mut header) {
        return match err.kind() {
            ErrorKind::UnexpectedEof
            | ErrorKind::ConnectionReset
            | ErrorKind::ConnectionAborted
            | ErrorKind::TimedOut
            | ErrorKind::WouldBlock => Ok(None),
            _ => Err(format!("read modbus header: {err}")),
        };
    }
    let tx = u16::from_be_bytes([header[0], header[1]]);
    let length = u16::from_be_bytes([header[4], header[5]]) as usize;
    let mut body = vec![0u8; length];
    stream
        .read_exact(&mut body)
        .map_err(|err| format!("read modbus body: {err}"))?;
    if body.is_empty() {
        return Err("empty modbus body".to_string());
    }
    Ok(Some(RequestFrame {
        tx,
        unit_id: body[0],
        pdu: body[1..].to_vec(),
    }))
}

fn handle_request(
    frame: &RequestFrame,
    state: &Arc<Mutex<SimulatorState>>,
) -> Result<Vec<u8>, String> {
    let function = frame
        .pdu
        .first()
        .copied()
        .ok_or_else(|| "empty modbus pdu".to_string())?;
    match function {
        0x04 => handle_read_input_registers(&frame.pdu, state),
        0x10 => handle_write_multiple_registers(&frame.pdu, state),
        _ => Ok(vec![function | 0x80, 0x01]),
    }
}

fn handle_read_input_registers(
    pdu: &[u8],
    state: &Arc<Mutex<SimulatorState>>,
) -> Result<Vec<u8>, String> {
    if pdu.len() < 5 {
        return Ok(vec![0x84, 0x03]);
    }
    let start = u16::from_be_bytes([pdu[1], pdu[2]]);
    let quantity = u16::from_be_bytes([pdu[3], pdu[4]]);
    let mut guard = state
        .lock()
        .map_err(|_| "simulator state lock poisoned".to_string())?;
    let start_index = start as usize;
    let end_index = start_index + quantity as usize;
    if end_index > guard.input_registers.len() {
        return Ok(vec![0x84, 0x02]);
    }
    let values = guard.input_registers[start_index..end_index].to_vec();
    guard.requests.push(ObservedRequest {
        function: "read_input_registers",
        address: start,
        quantity,
        values: values.clone(),
    });
    let mut payload = Vec::with_capacity(2 + values.len() * 2);
    payload.push(0x04);
    payload.push((values.len() * 2) as u8);
    for value in values {
        payload.extend_from_slice(&value.to_be_bytes());
    }
    Ok(payload)
}

fn handle_write_multiple_registers(
    pdu: &[u8],
    state: &Arc<Mutex<SimulatorState>>,
) -> Result<Vec<u8>, String> {
    if pdu.len() < 6 {
        return Ok(vec![0x90, 0x03]);
    }
    let start = u16::from_be_bytes([pdu[1], pdu[2]]);
    let quantity = u16::from_be_bytes([pdu[3], pdu[4]]);
    let byte_count = pdu[5] as usize;
    if pdu.len() < 6 + byte_count || byte_count < quantity as usize * 2 {
        return Ok(vec![0x90, 0x03]);
    }
    let start_index = start as usize;
    let end_index = start_index + quantity as usize;
    let mut guard = state
        .lock()
        .map_err(|_| "simulator state lock poisoned".to_string())?;
    if end_index > guard.output_registers.len() {
        return Ok(vec![0x90, 0x02]);
    }
    let mut values = Vec::with_capacity(quantity as usize);
    for idx in 0..quantity as usize {
        let offset = 6 + idx * 2;
        let value = u16::from_be_bytes([pdu[offset], pdu[offset + 1]]);
        guard.output_registers[start_index + idx] = value;
        values.push(value);
    }
    guard.requests.push(ObservedRequest {
        function: "write_multiple_registers",
        address: start,
        quantity,
        values,
    });
    Ok(vec![
        0x10,
        (start >> 8) as u8,
        start as u8,
        (quantity >> 8) as u8,
        quantity as u8,
    ])
}

fn write_response_frame(
    stream: &mut TcpStream,
    frame: &RequestFrame,
    response: &[u8],
) -> Result<(), String> {
    let mut header = [0u8; 6];
    header[0..2].copy_from_slice(&frame.tx.to_be_bytes());
    header[2..4].copy_from_slice(&0u16.to_be_bytes());
    header[4..6].copy_from_slice(&((response.len() + 1) as u16).to_be_bytes());
    stream
        .write_all(&header)
        .map_err(|err| format!("write modbus response header: {err}"))?;
    stream
        .write_all(&[frame.unit_id])
        .map_err(|err| format!("write modbus response unit id: {err}"))?;
    stream
        .write_all(response)
        .map_err(|err| format!("write modbus response pdu: {err}"))?;
    stream
        .flush()
        .map_err(|err| format!("flush modbus response: {err}"))?;
    Ok(())
}
