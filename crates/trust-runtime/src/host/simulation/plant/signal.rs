//! Plant command sources and feedback targets (Specification 46 §13.1–§13.3): a process-image address, the
//! symbolic path of a compiled program variable that has no address, or (feedback only) `pid:` and the path of a
//! Control.PID instance whose measurement the plant gives.

use crate::debug::resolve_control_parameter_path;
use crate::error::RuntimeError;
use crate::io::IoAddress;
use crate::memory::{InstanceId, IoArea, VariableStorage};
use crate::value::{Value, ValueRef};
use crate::Runtime;

/// The prefix of a Control.PID measurement target, `pid:Main.TIC101`.
const CONTROLLER_PREFIX: &str = "pid:";

#[derive(Clone, Debug, PartialEq)]
pub(super) enum PlantSignal {
    Address(IoAddress),
    Symbol(String),
    /// The measurement a Control.PID instance receives in `Update`, by the instance's path.
    Controller(String),
}

fn is_path(text: &str) -> bool {
    let identifier = |part: &str| {
        let mut chars = part.chars();
        chars
            .next()
            .is_some_and(|first| first.is_ascii_alphabetic() || first == '_')
            && chars.all(|next| next.is_ascii_alphanumeric() || next == '_')
    };
    text.len() <= 256 && text.split('.').all(identifier)
}

impl PlantSignal {
    /// An address always starts with `%`, a Control.PID measurement with `pid:`; any other text names a program
    /// variable by its dotted path.
    pub(super) fn parse(text: &str, areas: &[IoArea], role: &str) -> Result<Self, RuntimeError> {
        if text.trim_start().starts_with('%') {
            let mut parsed = super::validation::parse_addresses(&[text.to_string()], areas, role)?;
            return Ok(Self::Address(parsed.remove(0)));
        }
        if let Some(instance) = text.strip_prefix(CONTROLLER_PREFIX) {
            if role != "feedback" {
                return super::validation::invalid(format!(
                    "plant {role} cannot be a Control.PID measurement: {text}"
                ));
            }
            if !is_path(instance) {
                return super::validation::invalid(format!(
                    "plant {role} names no Control.PID instance path: {text}"
                ));
            }
            return Ok(Self::Controller(instance.to_string()));
        }
        if !is_path(text) {
            return super::validation::invalid(format!(
                "plant {role} is neither an address nor a program variable: {text}"
            ));
        }
        Ok(Self::Symbol(text.to_string()))
    }

    pub(super) fn address(&self) -> Option<&IoAddress> {
        match self {
            Self::Address(address) => Some(address),
            Self::Symbol(_) | Self::Controller(_) => None,
        }
    }

    /// Two targets that would write the same process-image bits, the same program variable or the same
    /// controller's measurement.
    pub(super) fn overlaps(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Address(left), Self::Address(right)) => {
                super::super::addresses_overlap(left, right)
            }
            (Self::Symbol(left), Self::Symbol(right))
            | (Self::Controller(left), Self::Controller(right)) => left.eq_ignore_ascii_case(right),
            _ => false,
        }
    }

    pub(super) fn read(&self, runtime: &Runtime) -> Result<f64, RuntimeError> {
        match self {
            Self::Address(address) => super::numeric_io::read_numeric(&runtime.io(), address),
            Self::Symbol(symbol) => super::numeric_io::decode_storage(&resolve(runtime, symbol)?.1),
            Self::Controller(path) => runtime
                .controller_measurement(controller_instance(runtime, path)?)
                .ok_or_else(|| {
                    RuntimeError::SimulationFault(
                        format!(
                            "plant feedback target pid:{path} has no published measurement yet"
                        )
                        .into(),
                    )
                }),
        }
    }
}

/// The Control.PID instance a `pid:` target names, refused with a plain reason when the path
/// names no instance of the Control library's `PID`.
pub(super) fn controller_instance(
    runtime: &Runtime,
    path: &str,
) -> Result<InstanceId, RuntimeError> {
    let fault = |reason: &str| {
        RuntimeError::SimulationFault(format!("plant feedback target pid:{path}: {reason}").into())
    };
    let resolved =
        resolve_control_parameter_path(runtime.storage(), path).map_err(|reason| fault(&reason))?;
    let Value::Instance(id) = resolved.value else {
        return Err(fault("names no function block instance"));
    };
    if !is_control_pid(runtime.storage(), id) {
        return Err(fault("is not an instance of Control.PID"));
    }
    Ok(id)
}

/// An instance of `Control.PID`: its type, by its qualified name, or by its own name with the
/// controller's parameter record and state.
fn is_control_pid(storage: &VariableStorage, id: InstanceId) -> bool {
    let Some(instance) = storage.get_instance(id) else {
        return false;
    };
    match instance.type_name.to_ascii_uppercase().as_str() {
        "CONTROL.PID" => true,
        "PID" => {
            storage.get_instance_var(id, "ParametersValue").is_some()
                && storage.get_instance_var(id, "StateValue").is_some()
        }
        _ => false,
    }
}

/// The program variable's storage reference and current value.
pub(super) fn resolve(runtime: &Runtime, symbol: &str) -> Result<(ValueRef, Value), RuntimeError> {
    let resolved = resolve_control_parameter_path(runtime.storage(), symbol).map_err(|reason| {
        RuntimeError::SimulationFault(format!("plant signal {symbol}: {reason}").into())
    })?;
    Ok((resolved.reference, resolved.value))
}
