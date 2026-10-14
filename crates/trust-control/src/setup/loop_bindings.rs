//! The bindings of a generated loop (specification 35, B1.3, B1.4, §17.1, A5 and A7): the signals it reads and writes, its id,
//! its revision, its identification experiment and the one task that runs its controller. Ported from `loopBindings.ts`.

use serde::Deserialize;
use serde_json::Value;
use sha2::{Digest, Sha256};

use super::catalog::{CompiledCatalog, CompiledController, CompiledSymbol, CompiledTask};
use super::texts::generators as texts;
use crate::failure::{Cause, Failure, NextAction};
use crate::limits::{MAXIMUM_EXPERIMENT_DURATION_MS, MINIMUM_EXPERIMENT_DURATION_MS};
use crate::loop_definition::{
    Direction, Execution, Experiment, ExperimentKind, LoopDefinition, LoopSignal, ManualOutputType,
    Range, RealType, StateType,
};
use crate::study::canonical_json;

/// A signal the engineer chose: its path, its unit and the range it may take (`MeasuredSignal`).
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MeasuredSignal {
    /// The variable's path.
    pub symbol: String,
    /// The signal's unit; blank means dimensionless (see [`stated_unit`]).
    pub unit: String,
    /// The smallest value the signal takes.
    pub minimum: f64,
    /// The largest value the signal takes.
    pub maximum: f64,
}

/// The compiled symbol with this exact path (`compiled`).
///
/// # Errors
/// The symbol is not compiled in this project.
pub fn compiled<'a>(
    catalog: &'a CompiledCatalog,
    symbol: &str,
) -> Result<&'a CompiledSymbol, Failure> {
    catalog
        .symbols
        .iter()
        .find(|entry| entry.symbol == symbol)
        .ok_or_else(|| {
            Failure::new(
                &texts::not_compiled_symbol(symbol),
                Cause::Implementation,
                NextAction::OpenLoopSetup,
            )
        })
}

/// The real type of a compiled variable: `REAL` or `LREAL` only (`realType`).
///
/// # Errors
/// The variable is not `REAL` or `LREAL`.
pub fn real_type(entry: &CompiledSymbol) -> Result<RealType, Failure> {
    match entry.iec_type.as_str() {
        "REAL" => Ok(RealType::Real),
        "LREAL" => Ok(RealType::Lreal),
        _ => Err(Failure::new(
            &texts::must_be_real(&entry.symbol),
            Cause::Settings,
            NextAction::OpenLoopSetup,
        )),
    }
}

/// A loop signal: addressed, or a program variable without an address (§17.1), with its stated unit and range (`binding`).
///
/// # Errors
/// The symbol is not compiled, or it is not `REAL` or `LREAL`.
pub fn binding(
    catalog: &CompiledCatalog,
    signal: &MeasuredSignal,
    direction: Direction,
) -> Result<LoopSignal, Failure> {
    let entry = compiled(catalog, &signal.symbol)?;
    let real = real_type(entry)?;
    Ok(LoopSignal {
        symbol: entry.symbol.clone(),
        address: entry.address.clone().filter(|address| !address.is_empty()),
        iec_type: real_name(real).to_string(),
        direction,
        unit: stated_unit(&signal.unit),
        range: Range {
            minimum: signal.minimum,
            maximum: signal.maximum,
        },
    })
}

/// A measured output that validation's simulated plant writes (§13.2): an input, or a program variable the program only reads.
/// A variable the program computes would overwrite the simulated measurement every scan (`measurementBinding`).
///
/// # Errors
/// The symbol is not compiled or is not `REAL` or `LREAL`, or it is computed by the program and has no address.
pub fn measurement_binding(
    catalog: &CompiledCatalog,
    signal: &MeasuredSignal,
) -> Result<LoopSignal, Failure> {
    let entry = compiled(catalog, &signal.symbol)?;
    let computed = entry.address.as_deref().is_none_or(str::is_empty)
        && catalog
            .writes
            .iter()
            .any(|written| written.to_uppercase() == entry.symbol.to_uppercase());
    if computed {
        return Err(Failure::new(
            &texts::computed_measurement(&entry.symbol),
            Cause::Settings,
            NextAction::OpenLoopSetup,
        ));
    }
    binding(catalog, signal, Direction::Read)
}

/// The loop id: `loop:` and the lower-case words of the name joined by hyphens, `loop` when no word is left (`loopId`).
#[must_use]
pub fn loop_id(name: &str) -> String {
    let mut slug = String::new();
    for character in name.to_lowercase().chars() {
        if character.is_ascii_lowercase() || character.is_ascii_digit() {
            slug.push(character);
        } else if !slug.ends_with('-') {
            slug.push('-');
        }
    }
    let words = slug.trim_matches('-');
    let words = if words.is_empty() { "loop" } else { words };
    format!("loop:{words}")
}

/// The loop with its revision: `sha256:` and the lower-case hex SHA-256 of its canonical JSON without the revision field
/// (`withRevision`). The TypeScript hashed `JSON.stringify` in insertion order, so the two digests of one loop differ.
#[must_use]
pub fn with_revision(mut loop_definition: LoopDefinition) -> LoopDefinition {
    loop_definition.revision = revision_of(&loop_definition);
    loop_definition
}

/// The digest of a loop's content, which is the loop without its revision.
fn revision_of(loop_definition: &LoopDefinition) -> String {
    // A loop definition always serializes; the default (null) would only digest a value no loop has.
    let mut value = serde_json::to_value(loop_definition).unwrap_or_default();
    if let Value::Object(object) = &mut value {
        object.remove("revision");
    }
    format!(
        "sha256:{:x}",
        Sha256::digest(canonical_json(&value).as_bytes())
    )
}

/// The unit a loop file states for a signal: a blank unit is dimensionless and is written `1`, as the loop reader writes it
/// (`state_dimensionless_units`). A parameter's unit must not be blank either (C0).
#[must_use]
pub fn stated_unit(unit: &str) -> String {
    if unit.trim().is_empty() {
        "1".to_string()
    } else {
        unit.to_string()
    }
}

/// The signal with its stated unit (see [`stated_unit`]).
#[must_use]
pub fn stated_signal(signal: &MeasuredSignal) -> MeasuredSignal {
    MeasuredSignal {
        unit: stated_unit(&signal.unit),
        ..signal.clone()
    }
}

/// The identification duration in s, within the experiment limits of A7 (`identificationDuration`).
///
/// # Errors
/// The duration is missing, is not finite, or is outside 1 s to 24 h.
pub fn identification_duration(name: &str, seconds: Option<f64>) -> Result<f64, Failure> {
    let (minimum, maximum) = duration_limits();
    match seconds {
        Some(seconds) if seconds.is_finite() && (minimum..=maximum).contains(&seconds) => {
            Ok(seconds)
        }
        _ => Err(Failure::new(
            &texts::identification_duration(name),
            Cause::Settings,
            NextAction::OpenLoopSetup,
        )),
    }
}

/// The identification duration limits in s (A7): 1 s to 24 h of process time.
#[allow(clippy::cast_precision_loss)] // Whole-millisecond limits far below 2^53 are exact in f64.
fn duration_limits() -> (f64, f64) {
    (
        MINIMUM_EXPERIMENT_DURATION_MS as f64 / 1_000.0,
        MAXIMUM_EXPERIMENT_DURATION_MS as f64 / 1_000.0,
    )
}

/// The identification experiment a setup writes (B1.3): only when the engineer set both its output step and its duration. Otherwise
/// the block is left out, and Identify chooses both within the limits (B3.3, B3.4).
///
/// # Errors
/// Only one of the step and the duration is set, or the duration is outside 1 s to 24 h.
pub fn explicit_experiment(
    name: &str,
    execution: Execution,
    kind: ExperimentKind,
    step: Option<f64>,
    seconds: Option<f64>,
) -> Result<Option<Experiment>, Failure> {
    match (step, seconds) {
        (None, None) => Ok(None),
        (Some(step), Some(seconds)) => Ok(Some(Experiment {
            execution,
            kind,
            amplitude: step,
            relay_direction: None,
            duration_seconds: Some(identification_duration(name, Some(seconds))?),
            baseline_seconds: None,
            hold_parameter: None,
            seed: 1,
            activation: None,
            abort: None,
        })),
        _ => Err(Failure::new(
            &texts::output_step_and_duration(name),
            Cause::Settings,
            NextAction::OpenLoopSetup,
        )),
    }
}

/// The one task that runs a controller, when its interval is positive (`controller.tasks.length !== 1 || !(intervalMs > 0)`).
///
/// # Errors
/// The controller does not run in exactly one task, or that task's interval is not positive.
pub fn cyclic_task(controller: &CompiledController) -> Result<&CompiledTask, Failure> {
    match controller.tasks.as_slice() {
        [task] if task.interval_ms > 0.0 => Ok(task),
        _ => Err(Failure::new(
            &texts::not_one_cyclic_task(&controller.instance),
            Cause::Implementation,
            NextAction::None,
        )),
    }
}

/// The manual output's type for a real type (`manualOutputType: realType(manual)`).
#[must_use]
pub fn manual_output_type(real: RealType) -> ManualOutputType {
    match real {
        RealType::Real => ManualOutputType::Real,
        RealType::Lreal => ManualOutputType::Lreal,
    }
}

/// The state type for a real type (the states of `REAL` and `LREAL` variables).
#[must_use]
pub fn state_type(real: RealType) -> StateType {
    match real {
        RealType::Real => StateType::Real,
        RealType::Lreal => StateType::Lreal,
    }
}

/// The IEC name of a real type, as a loop file writes it.
fn real_name(real: RealType) -> &'static str {
    match real {
        RealType::Real => "REAL",
        RealType::Lreal => "LREAL",
    }
}

#[cfg(test)]
#[path = "loop_bindings_tests.rs"]
mod tests;
