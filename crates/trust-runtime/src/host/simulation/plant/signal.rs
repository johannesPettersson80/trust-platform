//! Plant command sources and feedback targets (Specification 46 §13.1–§13.3): a process-image address, the
//! symbolic path of a compiled program variable that has no address, or (feedback only) `pid:` and the path of a
//! Control.PID instance whose measurement the plant gives.

use crate::debug::resolve_control_parameter_path;
use crate::error::RuntimeError;
use crate::io::IoAddress;
use crate::memory::IoArea;
use crate::value::Value;
use crate::{CompiledRuntime, Located, ProgramRead, ProgramWrite};

/// The prefix of a Control.PID measurement target, `pid:Main.TIC101`.
const CONTROLLER_PREFIX: &str = "pid:";

/// The members in which a Control.PID holds the measurement a plant gives it, the Control library's
/// simulation extension (Specification 46 §13.2): the value, and whether one is held.
const HELD_MEASUREMENT: &str = "HeldMeasurement";
const MEASUREMENT_HELD: &str = "MeasurementHeld";

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

    /// The signal's value as the last scan left the program: a command source's command, or the
    /// measurement a `pid:` target holds.
    pub(super) fn read(&self, runtime: &CompiledRuntime) -> Result<f64, RuntimeError> {
        match self {
            Self::Address(address) => super::numeric_io::read_numeric(runtime, address),
            Self::Symbol(symbol) => runtime.with_program_view(|program| {
                super::numeric_io::decode_storage(&resolve(program, symbol)?.1)
            }),
            Self::Controller(path) => runtime
                .with_program_view(|program| controller_instance(program, path)?.held(program))?
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

/// Where a Control.PID instance holds the measurement a plant gives it (Specification 46 §13.2):
/// while one is held, each `Update` takes it as `Input.ProcessValue`.
#[derive(Clone, Copy, Debug)]
pub(super) struct HeldMeasurement {
    value: Located,
    held: Located,
}

impl HeldMeasurement {
    /// The measurement held, `None` while none is.
    fn held<P: ProgramRead + ?Sized>(&self, program: &P) -> Result<Option<f64>, RuntimeError> {
        if program.read_at(&self.held)? != Value::Bool(true) {
            return Ok(None);
        }
        match program.read_at(&self.value)? {
            Value::LReal(value) => Ok(Some(value)),
            _ => Err(RuntimeError::TypeMismatch),
        }
    }

    /// Holds `value` for every later `Update` of the instance, until the next publish replaces it.
    pub(super) fn hold(
        &self,
        program: &mut dyn ProgramWrite,
        value: f64,
    ) -> Result<(), RuntimeError> {
        program.write_at(&self.value, &Value::LReal(value))?;
        program.write_at(&self.held, &Value::Bool(true))
    }

    /// Removes the held measurement: `Update` takes the program's own `ProcessValue` again.
    pub(super) fn release(&self, program: &mut dyn ProgramWrite) -> Result<(), RuntimeError> {
        program.write_at(&self.held, &Value::Bool(false))
    }
}

/// The Control.PID instance a `pid:` target names, refused with a plain reason when the path
/// names no instance of the Control library's `PID`.
pub(super) fn controller_instance<P: ProgramRead + ?Sized>(
    program: &P,
    path: &str,
) -> Result<HeldMeasurement, RuntimeError> {
    let fault = |reason: &str| {
        RuntimeError::SimulationFault(format!("plant feedback target pid:{path}: {reason}").into())
    };
    let at = program
        .locate(path)
        .map_err(|error| fault(&error.to_string()))?;
    if !is_control_pid(program, &at) {
        // A value reads; an instance holds none.
        return Err(fault(if program.read_at(&at).is_ok() {
            "names no function block instance"
        } else {
            "is not an instance of Control.PID"
        }));
    }
    match (
        program.member(&at, HELD_MEASUREMENT),
        program.member(&at, MEASUREMENT_HELD),
    ) {
        (Some(value), Some(held)) => Ok(HeldMeasurement { value, held }),
        _ => Err(fault(
            "is a Control.PID without the simulation members HeldMeasurement and MeasurementHeld",
        )),
    }
}

/// An instance of `Control.PID`: its type, by its qualified name, or by its own name with the
/// controller's parameter record and state.
fn is_control_pid<P: ProgramRead + ?Sized>(program: &P, at: &Located) -> bool {
    program.is_a(at, "Control.PID")
        || (program.is_a(at, "PID")
            && program.member(at, "ParametersValue").is_some()
            && program.member(at, "StateValue").is_some())
}

/// Where the program variable lies, and its value as the program holds it, found as Control Studio
/// finds a symbol.
pub(super) fn resolve(
    program: &dyn ProgramRead,
    symbol: &str,
) -> Result<(Located, Value), RuntimeError> {
    let resolved = resolve_control_parameter_path(program, symbol).map_err(|reason| {
        RuntimeError::SimulationFault(format!("plant signal {symbol}: {reason}").into())
    })?;
    Ok((resolved.at, resolved.value))
}
