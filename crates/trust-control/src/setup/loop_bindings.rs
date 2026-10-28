//! The bindings of a generated loop (specification 35, B1.3, B1.4, §17.1, A5 and A7): the signals it reads and writes, its id,
//! its revision, its identification experiment and the one task that runs its controller. Ported from `loopBindings.ts`.

use serde::{Deserialize, Deserializer};
use serde_json::Value;
use sha2::{Digest, Sha256};

use super::catalog::{CompiledCatalog, CompiledController, CompiledSymbol, CompiledTask};
use super::texts::generators as texts;
use super::texts::save;
use crate::failure::{Cause, Failure, NextAction};
use crate::json::number_or_text;
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

/// A measured signal as the setup form sends it. Each field may be missing or blank, and the checks name what is missing
/// (`signal` of `loopSetupForm.ts`). A draft's signal is a `FormSignal`; `measured_signal` checks it into a `MeasuredSignal`.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FormSignal {
    /// The variable's path; blank when none is chosen.
    #[serde(default)]
    pub symbol: String,
    /// The signal's unit; blank when none is given.
    #[serde(default)]
    pub unit: String,
    /// The smallest value, as the form's text reads it ([`form_number`]): absent when it is blank, NaN when no number reads it.
    #[serde(default, deserialize_with = "form_number")]
    pub minimum: Option<f64>,
    /// The largest value, as the form's text reads it ([`form_number`]): absent when it is blank, NaN when no number reads it.
    #[serde(default, deserialize_with = "form_number")]
    pub maximum: Option<f64>,
}

/// A signal of a setup form as a `MeasuredSignal`, checked in the order of `signal` in `loopSetupForm.ts`: its symbol is chosen, its unit
/// is given, both bounds are numbers, and the maximum is above the minimum. `label` names the signal's role in the sentences. `field` is
/// the form field of the signal: its symbol is `field`, its unit `{field}.unit` and its bounds `{field}.minimum` and `{field}.maximum`,
/// under which a refusal shows; `None` when the page has no such field. The unit is kept as sent.
///
/// # Errors
/// The symbol is blank, the unit is blank, the minimum or the maximum is missing or not finite, or the maximum is not above the minimum.
pub fn measured_signal(
    signal: &FormSignal,
    label: &str,
    field: Option<&str>,
) -> Result<MeasuredSignal, Failure> {
    let refuse = |text: &str, part: &str| {
        let failure = Failure::new(text, Cause::Settings, NextAction::OpenLoopSetup);
        match field {
            Some(field) if part.is_empty() => failure.with_field(field),
            Some(field) => failure.with_field(&format!("{field}.{part}")),
            None => failure,
        }
    };
    if signal.symbol.trim().is_empty() {
        return Err(refuse(&save::signal_symbol_needed(label), ""));
    }
    if signal.unit.trim().is_empty() {
        return Err(refuse(&save::signal_unit_needed(label), "unit"));
    }
    let (minimum, maximum) = match (signal.minimum, signal.maximum) {
        (Some(minimum), Some(maximum)) if minimum.is_finite() && maximum.is_finite() => {
            (minimum, maximum)
        }
        (minimum, _) => {
            let bound = if minimum.is_some_and(f64::is_finite) {
                "maximum"
            } else {
                "minimum"
            };
            return Err(refuse(&save::signal_range_needed(label), bound));
        }
    };
    if maximum <= minimum {
        return Err(refuse(&save::signal_range_order(label), "maximum"));
    }
    Ok(MeasuredSignal {
        symbol: signal.symbol.clone(),
        unit: signal.unit.clone(),
        minimum,
        maximum,
    })
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
        "sha256:{}",
        hex::encode(Sha256::digest(canonical_json(&value).as_bytes()))
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

/// A choice the setup form asks for that is blank: the sentence the engine answers, under the form field it names (B1.3). The checks
/// run in the order the form asks its questions, so the first blank choice is the one the page shows.
///
/// # Errors
/// The value is blank.
pub fn required_choice(value: &str, sentence: &str, field: &str) -> Result<(), Failure> {
    if value.trim().is_empty() {
        return Err(
            Failure::new(sentence, Cause::Settings, NextAction::OpenLoopSetup).with_field(field),
        );
    }
    Ok(())
}

/// A name the form sends as null while it is blank (`optionalText`, `loopSetupForm.ts`): read as the blank text, so the setup's own check
/// names the choice that is missing.
///
/// # Errors
/// The value is neither null nor text.
pub fn blank_when_null<'de, D: Deserializer<'de>>(deserializer: D) -> Result<String, D::Error> {
    Ok(Option::<String>::deserialize(deserializer)?.unwrap_or_default())
}

/// A number field of a setup draft, which the form sends as the text the engineer typed (B1.3): absent, null or blank text is not given
/// (`None`); a JSON number, or a text that a finite number reads (`number_or_text`), is that number; any other text, or a value of
/// another type, is NaN (`Some(NaN)`), which the setup's checks refuse under the field with the sentence that asks for it.
///
/// # Errors
/// The value cannot be read as JSON.
pub fn form_number<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Option<f64>, D::Error> {
    Ok(match Option::<Value>::deserialize(deserializer)? {
        None | Some(Value::Null) => None,
        Some(Value::String(text)) if text.trim().is_empty() => None,
        Some(value) => Some(number_or_text(&value)),
    })
}

/// The seconds of one unit word of a recording duration (`durationField.ts`, `UNIT_SECONDS`).
fn unit_seconds(unit: &str) -> Option<f64> {
    match unit {
        "h" | "hr" | "hrs" | "hour" | "hours" => Some(3_600.0),
        "min" | "mins" | "minute" | "minutes" => Some(60.0),
        "s" | "sec" | "secs" | "second" | "seconds" => Some(1.0),
        _ => None,
    }
}

/// A number of the form `\d+(\.\d+)?`, which is the only form a duration's number takes (`durationField.ts`).
fn plain_number(text: &str) -> Option<f64> {
    let digits = |part: &str| !part.is_empty() && part.bytes().all(|byte| byte.is_ascii_digit());
    let (whole, fraction) = match text.split_once('.') {
        Some((whole, fraction)) => (whole, Some(fraction)),
        None => (text, None),
    };
    if !digits(whole) || fraction.is_some_and(|part| !digits(part)) {
        return None;
    }
    text.parse::<f64>().ok()
}

/// A recording duration as the setup form writes it: a plain number of seconds, or words of hours, minutes and seconds such as `15 min`
/// or `1 h 30 min` (`durationField.ts`, `parseDurationSeconds`). Any other text is `None`, which the setup refuses.
#[must_use]
pub fn duration_seconds_of_text(text: &str) -> Option<f64> {
    let source = text.trim().to_ascii_lowercase();
    if source.is_empty() {
        return None;
    }
    if let Some(seconds) = plain_number(&source) {
        return Some(seconds);
    }
    let mut rest = source.as_str();
    let mut total = 0.0;
    while !rest.is_empty() {
        let end = rest
            .find(|character: char| !(character.is_ascii_digit() || character == '.'))
            .unwrap_or(rest.len());
        let (number, after) = rest.split_at(end);
        let value = plain_number(number)?;
        let after = after.trim_start();
        let unit_end = after
            .find(|character: char| !character.is_ascii_lowercase())
            .unwrap_or(after.len());
        let (unit, remaining) = after.split_at(unit_end);
        total += value * unit_seconds(unit)?;
        rest = remaining.trim_start();
    }
    Some(total)
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
