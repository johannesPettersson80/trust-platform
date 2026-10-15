//! Execution arms shared by the generated-program property tests: `property_executor_differential`
//! (checklist A03) and `property_metamorphic` (checklist A04). Each test crate includes this file
//! with `#[path]`, together with `st_program_gen` as a module at its own root.

#![allow(dead_code)]

use crate::st_program_gen::INPUT_NAMES;
use trust_runtime::bytecode::BytecodeVersion;
use trust_runtime::harness::CompileSession;
use trust_runtime_core::error::RuntimeError;
use trust_runtime_core::value::{DateTimeValue, Duration, Value};
use trust_runtime_core::vm::register::RegisterExecutionConfiguration;
use trust_runtime_core::vm::{ExecutionServices, PreparationLimits, PreparedModule, RuntimeState};

/// Compiles a program and encodes it as saved STBC.
pub fn compile(source: &str) -> Result<Vec<u8>, String> {
    CompileSession::from_source(source)
        .build_bytecode_module_for_version(BytecodeVersion::SOURCE_FREE)
        .map_err(|error| format!("compile failed: {error:?}"))?
        .encode()
        .map_err(|error| format!("serialize failed: {error:?}"))
}

/// Decodes, validates and prepares encoded STBC without source or a compiler.
pub fn admit(bytes: &[u8]) -> Result<PreparedModule, String> {
    PreparedModule::from_bytes(bytes, PreparationLimits::default())
        .map_err(|error| format!("admission failed: {error:?}"))
}

/// Compiles and admits a program as saved STBC.
pub fn prepare(source: &str) -> Result<PreparedModule, String> {
    admit(&compile(source)?)
}

/// Instantiates a prepared module with explicit services. The services outlive the state.
pub fn instantiate<'a>(
    prepared: &'a PreparedModule,
    services: &'a dyn ExecutionServices,
) -> Result<RuntimeState<'a>, String> {
    prepared
        .instantiate_with_services(0, services)
        .map_err(|error| format!("instantiate failed: {error:?}"))
}

/// Clears the fault, writes one input row and runs one cycle. The inner result is the outcome of
/// `execute_cycle`; the outer error is a failed input write.
pub fn run_cycle(
    state: &mut RuntimeState<'_>,
    row: &[Value; 4],
    time: Duration,
) -> Result<Result<(), RuntimeError>, String> {
    state.clear_fault();
    for (name, value) in INPUT_NAMES.iter().zip(row) {
        state
            .write_global(name, value.clone())
            .map_err(|error| format!("write {name} failed: {error:?}"))?;
    }
    Ok(state.execute_cycle(time))
}

/// The variables of `Main`, in declaration order.
pub fn main_variable_names(state: &RuntimeState<'_>) -> Result<Vec<String>, String> {
    let Some(Value::Instance(root)) = state.storage().get_global("Main") else {
        return Err("program Main is not an instance".to_string());
    };
    let instance = state
        .storage()
        .get_instance(*root)
        .ok_or("Main instance is missing")?;
    Ok(instance
        .variables
        .keys()
        .map(|name| name.to_string())
        .collect())
}

/// One variable of `Main`, read through the storage accessors.
pub fn main_value(state: &RuntimeState<'_>, name: &str) -> Result<Value, String> {
    let Some(Value::Instance(root)) = state.storage().get_global("Main") else {
        return Err("program Main is not an instance".to_string());
    };
    state
        .storage()
        .get_instance_var(*root, name)
        .cloned()
        .ok_or_else(|| format!("Main variable {name} is missing"))
}

/// Equality of two variable values. Reals compare by bit pattern, so NaN and -0.0 are exact.
pub fn same_value(left: &Value, right: &Value) -> bool {
    match (left, right) {
        (Value::Real(a), Value::Real(b)) => a.to_bits() == b.to_bits(),
        (Value::LReal(a), Value::LReal(b)) => a.to_bits() == b.to_bits(),
        _ => left == right,
    }
}

/// Execution services for one arm. `stack` selects the shared stack path.
pub struct ConfiguredServices {
    pub stack: bool,
    pub configuration: RegisterExecutionConfiguration,
}

impl ExecutionServices for ConfiguredServices {
    fn register_configuration(&self) -> RegisterExecutionConfiguration {
        self.configuration
    }
    fn debug_active(&self) -> bool {
        self.stack
    }
    fn deadline_exceeded(&self) -> bool {
        false
    }
    fn has_wall_clock(&self) -> bool {
        false
    }
    fn current_dt(&self) -> Result<DateTimeValue, RuntimeError> {
        Err(RuntimeError::UndefinedFunction("CURRENT_DT".into()))
    }
}

/// Telemetry on, Tier-1 off: the register arm.
pub fn profiled() -> RegisterExecutionConfiguration {
    RegisterExecutionConfiguration {
        profile_enabled: true,
        ..RegisterExecutionConfiguration::default()
    }
}

/// Telemetry on and Tier-1 on, selecting a block once it has `threshold` hits.
pub fn tier1_arm(threshold: u64) -> RegisterExecutionConfiguration {
    RegisterExecutionConfiguration {
        tier1_enabled: true,
        tier1_hot_threshold: threshold,
        ..profiled()
    }
}
