//! Shared harness for the control lessons that execute the real `Control.PID` project.
#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::sync::Arc;

use trust_runtime::harness::{CompileSession, SourceFile};
use trust_runtime::io::IoAddress;
use trust_runtime::memory::InstanceId;
use trust_runtime::simulation::{SimulationConfig, SimulationController};
use trust_runtime::value::{Duration, Value};
use trust_runtime::Runtime;

pub fn project_dir(relative: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/control")
        .join(relative)
}

pub fn number(runtime: &Runtime, address: &str) -> f64 {
    match runtime
        .io()
        .read(&IoAddress::parse(address).unwrap())
        .unwrap()
    {
        Value::LWord(bits) => f64::from_bits(bits),
        other => panic!("expected LREAL at {address}, got {other:?}"),
    }
}

pub fn setup(relative: &str) -> (Runtime, SimulationController) {
    let project = project_dir(relative);
    let session = CompileSession::from_sources(vec![
        SourceFile::with_path(
            "src/Main.st",
            std::fs::read_to_string(project.join("src/Main.st")).unwrap(),
        ),
        SourceFile::with_path(
            "Control/control.st",
            include_str!("../../../../libraries/control/src/control.st"),
        ),
    ]);
    let mut runtime = session.build_runtime().unwrap();
    runtime
        .apply_bytecode_bytes(&session.build_bytecode_bytes().unwrap(), None)
        .unwrap();
    let simulation =
        SimulationController::new(SimulationConfig::load(project.join("simulation.toml")).unwrap());
    (runtime, simulation)
}

pub fn scan(
    runtime: &mut Runtime,
    simulation: &mut SimulationController,
    debug: &trust_runtime::debug::DebugControl,
    now_ms: u64,
    writes: &[(&str, Value)],
) {
    scan_every(runtime, simulation, debug, now_ms, 100, writes);
}

/// One task cycle of `period_ms`: the plant advances by the same period.
pub fn scan_every(
    runtime: &mut Runtime,
    simulation: &mut SimulationController,
    debug: &trust_runtime::debug::DebugControl,
    now_ms: u64,
    period_ms: u64,
    writes: &[(&str, Value)],
) {
    simulation
        .apply_pre_cycle(Duration::from_millis(now_ms as i64), runtime)
        .unwrap();
    for (address, value) in writes {
        debug.enqueue_io_write(IoAddress::parse(address).unwrap(), value.clone());
    }
    runtime.advance_time(Duration::from_millis(period_ms as i64));
    runtime.execute_cycle().unwrap();
    simulation
        .apply_post_cycle(Duration::from_millis((now_ms + period_ms) as i64), runtime)
        .unwrap();
}

pub fn program(runtime: &Runtime) -> InstanceId {
    let Value::Instance(program) = runtime.storage().get_global("P1").cloned().unwrap() else {
        panic!("the lesson must execute its P1 program");
    };
    program
}

pub fn program_number(runtime: &Runtime, program: InstanceId, name: &str) -> f64 {
    match runtime.storage().get_instance_var(program, name) {
        Some(Value::LReal(value)) => *value,
        other => panic!("{name}: {other:?}"),
    }
}

fn controller(runtime: &Runtime, program: InstanceId) -> InstanceId {
    let Some(Value::Instance(controller)) = runtime
        .storage()
        .get_instance_var(program, "Controller")
        .cloned()
    else {
        panic!("the lesson must execute Control.PID");
    };
    controller
}

pub fn controller_state(runtime: &Runtime, program: InstanceId, field: &str) -> f64 {
    let Some(Value::Struct(state)) = runtime
        .storage()
        .get_instance_var(controller(runtime, program), "StateValue")
    else {
        panic!("Control.PID must keep its state record");
    };
    match state.field(field) {
        Some(Value::LReal(value)) => *value,
        other => panic!("{field}: {other:?}"),
    }
}

/// Writes Control.PID's seven retained values, as Control Studio's verification and Apply do.
pub fn set_controller_state(runtime: &mut Runtime, program: InstanceId, values: &[(&str, f64)]) {
    let controller = controller(runtime, program);
    let Some(Value::Struct(state)) = runtime
        .storage()
        .get_instance_var(controller, "StateValue")
        .cloned()
    else {
        panic!("Control.PID must keep its state record");
    };
    let mut state = (*state).clone();
    for (field, value) in values {
        assert!(
            state.set_existing_field((*field).into(), Value::LReal(*value)),
            "Control.PID state {field}"
        );
    }
    assert!(runtime.storage_mut().set_instance_var(
        controller,
        "StateValue",
        Value::Struct(Arc::new(state))
    ));
}

pub fn input_cycle_time(runtime: &Runtime, program: InstanceId) -> f64 {
    let Some(Value::Struct(input)) = runtime.storage().get_instance_var(program, "Input") else {
        panic!("the lesson must keep the Control.PID input record");
    };
    match input.field("CycleTime") {
        Some(Value::LReal(value)) => *value,
        other => panic!("CycleTime: {other:?}"),
    }
}

/// Replaces fields of one program record (for example `Parameters`) in a single write, as Control Studio's Apply does.
pub fn set_program_record(
    runtime: &mut Runtime,
    program: InstanceId,
    record: &str,
    values: &[(&str, f64)],
) {
    let Some(Value::Struct(current)) = runtime.storage().get_instance_var(program, record).cloned()
    else {
        panic!("the lesson must keep the {record} record");
    };
    let mut next = (*current).clone();
    for (field, value) in values {
        assert!(
            next.set_existing_field((*field).into(), Value::LReal(*value)),
            "{record}.{field}"
        );
    }
    assert!(runtime
        .storage_mut()
        .set_instance_var(program, record, Value::Struct(Arc::new(next))));
}

pub fn program_dint(runtime: &Runtime, program: InstanceId, name: &str) -> i64 {
    match runtime.storage().get_instance_var(program, name) {
        Some(Value::DInt(value)) => i64::from(*value),
        other => panic!("{name}: {other:?}"),
    }
}

pub fn program_record_number(
    runtime: &Runtime,
    program: InstanceId,
    record: &str,
    field: &str,
) -> f64 {
    let Some(Value::Struct(value)) = runtime.storage().get_instance_var(program, record) else {
        panic!("the lesson must keep the {record} record");
    };
    match value.field(field) {
        Some(Value::LReal(number)) => *number,
        other => panic!("{record}.{field}: {other:?}"),
    }
}

pub fn program_array(runtime: &Runtime, program: InstanceId, name: &str) -> Vec<Value> {
    match runtime.storage().get_instance_var(program, name) {
        Some(Value::Array(array)) => array.elements().to_vec(),
        other => panic!("{name}: {other:?}"),
    }
}

/// Control.PID as the heater lessons configure it, in the block's order of operations: PI in incremental form with β = 1,
/// back-calculation with Tt = 1 s, 0.1 s updates and limits 0–100 %.
pub struct MirrorPi {
    pub setpoint: f64,
    pub process: f64,
    pub nominal: f64,
}

impl MirrorPi {
    pub fn at_rest(hold: f64, temperature: f64) -> Self {
        Self {
            setpoint: 35.0,
            process: temperature,
            nominal: hold,
        }
    }

    /// One Auto update.
    pub fn update(&mut self, kp: f64, ki: f64, setpoint: f64, temperature: f64) -> f64 {
        let proportional = kp * (1.0 * (setpoint - self.setpoint) - (temperature - self.process));
        let integral = ki * 0.1 * (setpoint - temperature);
        let mut nominal = self.nominal + proportional + integral + 0.0 + 0.0;
        let candidate = nominal.clamp(0.0, 100.0);
        nominal -= 0.1 / 1.0 * (nominal - candidate);
        self.setpoint = setpoint;
        self.process = temperature;
        self.nominal = nominal;
        nominal.clamp(0.0, 100.0)
    }

    /// One Manual update: the requested output becomes the retained nominal output.
    pub fn manual(&mut self, output: f64, setpoint: f64, temperature: f64) -> f64 {
        self.setpoint = setpoint;
        self.process = temperature;
        self.nominal = output;
        output.clamp(0.0, 100.0)
    }
}
