//! Shared harness for the control lessons that execute the real `Control.PID` project, compiled for
//! this computer (the toolchain of specification 44 §4.6 at `TRUST_TOOLCHAIN`). A lesson's values
//! are read through the program access seam, by their paths below the program instance (`P1`), as
//! the last scan left them, and written between scans as Control Studio's Apply writes them.
#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::sync::Arc;

use trust_runtime::debug::DebugControl;
use trust_runtime::error::RuntimeError;
use trust_runtime::harness::{CompileSession, SourceFile, TestHarness};
use trust_runtime::io::IoAddress;
use trust_runtime::simulation::{SimulationConfig, SimulationController};
use trust_runtime::value::{Duration, Value};
use trust_runtime::CompiledRuntime;

pub fn project_dir(relative: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/control")
        .join(relative)
}

/// The LREAL at the process-image `address`: the image holds its bits, a typed read its value.
pub fn number(runtime: &CompiledRuntime, address: &str) -> f64 {
    match runtime
        .read_io(&IoAddress::parse(address).unwrap())
        .unwrap()
    {
        Value::LWord(bits) => f64::from_bits(bits),
        Value::LReal(value) => value,
        other => panic!("expected LREAL at {address}, got {other:?}"),
    }
}

/// The program of `session`, built for this computer and started cold, as `trust-runtime` starts
/// a program it loads.
pub fn compile(session: CompileSession) -> CompiledRuntime {
    TestHarness::from_session(session)
        .unwrap_or_else(|error| panic!("the lesson must compile: {error}"))
        .into_runtime()
}

pub fn setup(relative: &str) -> (CompiledRuntime, SimulationController) {
    let project = project_dir(relative);
    let runtime = compile(CompileSession::from_sources(vec![
        SourceFile::with_path(
            "src/Main.st",
            std::fs::read_to_string(project.join("src/Main.st")).unwrap(),
        ),
        SourceFile::with_path(
            "Control/control.st",
            include_str!("../../../../libraries/control/src/control.st"),
        ),
    ]));
    let simulation =
        SimulationController::new(SimulationConfig::load(project.join("simulation.toml")).unwrap());
    (runtime, simulation)
}

/// What the lessons do with a compiled runtime between its scans, through its debug control and the
/// program access seam.
pub trait LessonRuntime {
    /// Attaches a debug control: the process-image and variable writes queued on it reach the
    /// program at the next scan's access points (specification 44 §13.3).
    fn enable_debug(&mut self) -> DebugControl;

    /// Moves the program's logical time on by `period`: the next scan runs at the later time.
    fn advance_time(&mut self, period: Duration);

    /// Writes the variable `name` of the instance `instance` (`P1`) between scans, as
    /// [`write_path`] writes it.
    ///
    /// # Errors
    ///
    /// As [`write_path`].
    fn write_instance(
        &mut self,
        instance: &str,
        name: &str,
        value: Value,
    ) -> Result<(), RuntimeError>;
}

impl LessonRuntime for CompiledRuntime {
    fn enable_debug(&mut self) -> DebugControl {
        let debug = DebugControl::new();
        self.set_debug_control(debug.clone());
        debug
    }

    fn advance_time(&mut self, period: Duration) {
        let now = self.current_time();
        self.set_current_time(Duration::from_nanos(
            now.as_nanos().saturating_add(period.as_nanos()),
        ));
    }

    fn write_instance(
        &mut self,
        instance: &str,
        name: &str,
        value: Value,
    ) -> Result<(), RuntimeError> {
        write_path(self, &format!("{instance}.{name}"), &value)
    }
}

/// Queues a write of the variable `name` of the program instance `instance`, which the next scan
/// takes at its start, before the program runs (specification 44 §13.3).
pub fn enqueue_instance_write(debug: &DebugControl, instance: &str, name: &str, value: Value) {
    debug
        .try_enqueue_global_write(format!("{instance}.{name}"), value)
        .expect("the write queue takes the write");
}

pub fn scan(
    runtime: &mut CompiledRuntime,
    simulation: &mut SimulationController,
    debug: &DebugControl,
    now_ms: u64,
    writes: &[(&str, Value)],
) {
    scan_every(runtime, simulation, debug, now_ms, 100, writes);
}

/// One task cycle of `period_ms`: the plant advances by the same period.
pub fn scan_every(
    runtime: &mut CompiledRuntime,
    simulation: &mut SimulationController,
    debug: &DebugControl,
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

/// The value at `path` (`P1.Controller.StateValue`), as the last scan left it; `None` for a path
/// that names nothing or an instance.
pub fn value(runtime: &CompiledRuntime, path: &str) -> Option<Value> {
    runtime.with_program_view(|program| program.value(path))
}

/// Whether `path` names a variable, a member or an instance of the program.
pub fn exists(runtime: &CompiledRuntime, path: &str) -> bool {
    runtime.with_program_view(|program| program.locate(path).is_ok())
}

/// The process-image address the program binds the variable `name` (`P1.TemperatureC`) to
/// (specification 44 §5.6); `None` for a name that names no variable or a variable bound nowhere.
pub fn bound_address(runtime: &CompiledRuntime, name: &str) -> Option<IoAddress> {
    let symbol = runtime.find_variable(name).ok()?;
    runtime
        .io_bindings()
        .iter()
        .find(|binding| binding.symbol == symbol)
        .map(|binding| binding.address.clone())
}

/// The IEC name of the type of the variable or instance at `path` (`Control.PID`); `None` for a
/// path that names nothing.
pub fn type_name(runtime: &CompiledRuntime, path: &str) -> Option<String> {
    runtime.with_program_view(|program| {
        let at = program.locate(path).ok()?;
        Some(program.type_name_of(&at).to_string())
    })
}

/// Writes `value` at `path` between scans, converted to the place's type, as Control Studio's
/// Apply writes a program variable; the place keeps its value when the write is refused.
///
/// # Errors
///
/// [`RuntimeError::UndefinedVariable`] for a path that names nothing, and the refusal of a value
/// the place does not take.
pub fn write_path(
    runtime: &mut CompiledRuntime,
    path: &str,
    value: &Value,
) -> Result<(), RuntimeError> {
    runtime.with_program(|program| {
        let at = program
            .locate(path)
            .map_err(|_| RuntimeError::UndefinedVariable(path.into()))?;
        program.write_at(&at, value)
    })
}

/// The program instance `P1` the lessons execute.
pub fn program(runtime: &CompiledRuntime) -> &'static str {
    assert!(
        exists(runtime, "P1"),
        "the lesson must execute its P1 program"
    );
    "P1"
}

pub fn program_number(runtime: &CompiledRuntime, program: &str, name: &str) -> f64 {
    match value(runtime, &format!("{program}.{name}")) {
        Some(Value::LReal(value)) => value,
        other => panic!("{name}: {other:?}"),
    }
}

fn controller(runtime: &CompiledRuntime, program: &str) -> String {
    let controller = format!("{program}.Controller");
    assert!(
        exists(runtime, &controller),
        "the lesson must execute Control.PID"
    );
    controller
}

pub fn controller_state(runtime: &CompiledRuntime, program: &str, field: &str) -> f64 {
    let Some(Value::Struct(state)) = value(
        runtime,
        &format!("{}.StateValue", controller(runtime, program)),
    ) else {
        panic!("Control.PID must keep its state record");
    };
    match state.field(field) {
        Some(Value::LReal(value)) => *value,
        other => panic!("{field}: {other:?}"),
    }
}

/// Writes Control.PID's seven retained values, as Control Studio's verification and Apply do.
pub fn set_controller_state(runtime: &mut CompiledRuntime, program: &str, values: &[(&str, f64)]) {
    let path = format!("{}.StateValue", controller(runtime, program));
    set_record(runtime, &path, values, "Control.PID state");
}

pub fn input_cycle_time(runtime: &CompiledRuntime, program: &str) -> f64 {
    let Some(Value::Struct(input)) = value(runtime, &format!("{program}.Input")) else {
        panic!("the lesson must keep the Control.PID input record");
    };
    match input.field("CycleTime") {
        Some(Value::LReal(value)) => *value,
        other => panic!("CycleTime: {other:?}"),
    }
}

/// Replaces fields of one program record (for example `Parameters`) in a single write, as Control Studio's Apply does.
pub fn set_program_record(
    runtime: &mut CompiledRuntime,
    program: &str,
    record: &str,
    values: &[(&str, f64)],
) {
    let path = format!("{program}.{record}");
    set_record(runtime, &path, values, record);
}

/// Replaces fields of the record at `path` in a single write; `label` names the record in a
/// failure.
fn set_record(runtime: &mut CompiledRuntime, path: &str, values: &[(&str, f64)], label: &str) {
    let Some(Value::Struct(current)) = value(runtime, path) else {
        panic!("the lesson must keep the {label} record");
    };
    let mut next = (*current).clone();
    for (field, value) in values {
        assert!(
            next.set_existing_field((*field).into(), Value::LReal(*value)),
            "{label}.{field}"
        );
    }
    assert!(write_path(runtime, path, &Value::Struct(Arc::new(next))).is_ok());
}

pub fn program_dint(runtime: &CompiledRuntime, program: &str, name: &str) -> i64 {
    match value(runtime, &format!("{program}.{name}")) {
        Some(Value::DInt(value)) => i64::from(value),
        other => panic!("{name}: {other:?}"),
    }
}

pub fn program_record_number(
    runtime: &CompiledRuntime,
    program: &str,
    record: &str,
    field: &str,
) -> f64 {
    let Some(Value::Struct(value)) = value(runtime, &format!("{program}.{record}")) else {
        panic!("the lesson must keep the {record} record");
    };
    match value.field(field) {
        Some(Value::LReal(number)) => *number,
        other => panic!("{record}.{field}: {other:?}"),
    }
}

pub fn program_array(runtime: &CompiledRuntime, program: &str, name: &str) -> Vec<Value> {
    match value(runtime, &format!("{program}.{name}")) {
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
