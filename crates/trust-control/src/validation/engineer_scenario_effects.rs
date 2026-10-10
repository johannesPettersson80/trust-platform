//! Scenario effects (specification 35, B5.3 and C0; numerical annex N8.1): the bounded plant effects that a validation
//! scenario or an envelope case applies to the simulated plant. This module holds the rules of `withScenarioEffects`
//! (`engineerScenarioEffects.ts`) that need no request: its checks, and the native `[[plants.uncertainties]]` tables as
//! text. Appending the tables to the simulation and selecting the plant belong to the verification client (V3.2).

use std::collections::BTreeSet;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::experiment::number_text::js_number;
use crate::failure::{Cause, Failure, NextAction};
use crate::model::GeneralModel;
use crate::validation::texts;

/// The most effects one scenario applies: the bound of the native plant's uncertainty table (`withScenarioEffects`).
pub const MAXIMUM_EFFECTS: usize = 32;

/// A bounded change of one plant quantity over a window of the run (`EngineerScenarioEffect`): a constant, a pulse or
/// uniform white noise, combined with the plant's value by its mode.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EngineerScenarioEffect {
    /// The identity: a letter, then letters, digits, `_` or `-`.
    pub id: String,
    /// The plant quantity the effect acts on.
    pub target: EffectTarget,
    /// The channel of the target, counted from zero inside the model's channels of that kind.
    pub channel: usize,
    /// How the effect combines with the plant's value.
    pub mode: EffectMode,
    /// When the effect starts, in seconds from the start of the run.
    pub start_seconds: f64,
    /// When the effect ends, in seconds; absent when the table gives no end.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_seconds: Option<f64>,
    /// The signal the effect generates.
    pub generator: EffectGenerator,
    /// The value of a constant or of a pulse.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<f64>,
    /// The lower bound of uniform white noise.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub minimum: Option<f64>,
    /// The upper bound of uniform white noise.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub maximum: Option<f64>,
    /// The length of a pulse, in seconds.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_seconds: Option<f64>,
}

/// The plant quantity an effect acts on (`target`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum EffectTarget {
    /// The command the program writes to the plant.
    Command,
    /// The input of the plant.
    ProcessInput,
    /// The output of the plant.
    ProcessOutput,
    /// The measured output, as the program reads it.
    Measurement,
    /// A state of the model.
    State,
}

/// How an effect combines with the plant's value (`mode`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum EffectMode {
    /// Added to the value.
    Add,
    /// Multiplies the value.
    Multiply,
    /// Replaces the value.
    Replace,
    /// Holds the value.
    Hold,
}

/// The signal an effect generates (`generator`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum EffectGenerator {
    /// A constant value.
    Constant,
    /// Uniform white noise between two bounds.
    UniformWhite,
    /// A pulse of the value for the duration.
    Pulse,
}

/// The `[[plants.uncertainties]]` tables of the effects, as the native plant reads them: one table per effect, in the
/// order given, the tables separated by a blank line. The checks of `withScenarioEffects` run first and in its order.
///
/// # Errors
/// More than 32 effects, or a repeated identity (`scenario_effects_need_plant`); a coordinate that the model or the run
/// does not hold (`scenario_effect_coordinates_invalid`); a constant or a pulse without a finite value
/// (`constant_effect_needs_value`); a pulse without a positive duration (`pulse_needs_duration`); noise without ordered
/// finite bounds (`noise_bounds_invalid`). Each refusal is a `Settings` failure whose next action is Requirements.
pub fn scenario_effect_tables(
    effects: &[EngineerScenarioEffect],
    model: &GeneralModel,
    duration_ms: u64,
) -> Result<String, Failure> {
    let identities: BTreeSet<&str> = effects.iter().map(|effect| effect.id.as_str()).collect();
    if effects.len() > MAXIMUM_EFFECTS || identities.len() != effects.len() {
        return Err(requirements_failure(&texts::scenario_effects_need_plant()));
    }
    let duration_seconds = milliseconds_as_float(duration_ms) / 1000.0;
    let mut tables = Vec::with_capacity(effects.len());
    for effect in effects {
        tables.push(effect_table(effect, model, duration_seconds)?);
    }
    Ok(tables.join("\n"))
}

/// The checks of `withScenarioEffects` without the table text. The campaign checks a case's effects with this before
/// any run starts.
///
/// # Errors
/// As `scenario_effect_tables`.
pub fn check_scenario_effects(
    effects: &[EngineerScenarioEffect],
    model: &GeneralModel,
    duration_ms: u64,
) -> Result<(), Failure> {
    scenario_effect_tables(effects, model, duration_ms).map(|_| ())
}

/// A refusal of a scenario or requirement setting. The setting is the engineer's own, so its fix is in Requirements
/// (A11.2): the cause is `Settings` and the next action opens the requirements.
#[must_use]
pub(crate) fn requirements_failure(text: &str) -> Failure {
    Failure::new(text, Cause::Settings, NextAction::OpenRequirements)
}

/// A duration or a time in milliseconds as a float. Plan durations are at most 24 h (86,400,000 ms), far below 2^53, so
/// the value is exact.
#[allow(clippy::cast_precision_loss)]
#[must_use]
pub(crate) fn milliseconds_as_float(milliseconds: u64) -> f64 {
    milliseconds as f64
}

/// One effect's table: its coordinates are checked first, then its start and end, then its generator's fields.
fn effect_table(
    effect: &EngineerScenarioEffect,
    model: &GeneralModel,
    duration_seconds: f64,
) -> Result<String, Failure> {
    if !coordinates_valid(effect, model, duration_seconds) {
        return Err(requirements_failure(
            &texts::scenario_effect_coordinates_invalid(),
        ));
    }
    let mut fields = vec![
        ("id", quoted(&effect.id)),
        ("target", quoted(target_word(effect.target))),
        ("channel", effect.channel.to_string()),
        ("mode", quoted(mode_word(effect.mode))),
        ("generator", quoted(generator_word(effect.generator))),
        ("start_s", js_number(effect.start_seconds)),
    ];
    if let Some(end) = effect.end_seconds {
        fields.push(("end_s", js_number(end)));
    }
    generator_fields(effect, &mut fields)?;
    Ok(table_text(&fields))
}

/// The fields of the effect's generator: the value of a constant or a pulse, the duration of a pulse, or the bounds of
/// noise. Each field is appended after the ones before it, in the order of the native table.
fn generator_fields(
    effect: &EngineerScenarioEffect,
    fields: &mut Vec<(&'static str, String)>,
) -> Result<(), Failure> {
    match effect.generator {
        EffectGenerator::Constant | EffectGenerator::Pulse => {
            let value = effect
                .value
                .filter(|value| value.is_finite())
                .ok_or_else(|| requirements_failure(&texts::constant_effect_needs_value()))?;
            fields.push(("value", js_number(value)));
            if effect.generator == EffectGenerator::Pulse {
                let duration = effect
                    .duration_seconds
                    .filter(|duration| duration.is_finite() && *duration > 0.0)
                    .ok_or_else(|| requirements_failure(&texts::pulse_needs_duration()))?;
                fields.push(("duration_s", js_number(duration)));
            }
            Ok(())
        }
        EffectGenerator::UniformWhite => match (effect.minimum, effect.maximum) {
            (Some(minimum), Some(maximum))
                if minimum.is_finite() && maximum.is_finite() && minimum < maximum =>
            {
                fields.push(("minimum", js_number(minimum)));
                fields.push(("maximum", js_number(maximum)));
                Ok(())
            }
            _ => Err(requirements_failure(&texts::noise_bounds_invalid())),
        },
    }
}

/// The coordinates an effect must have: an identity, a channel of its target inside the model, no hold on a state or a
/// command, and a window that starts inside the run and ends after its start, inside the run.
fn coordinates_valid(
    effect: &EngineerScenarioEffect,
    model: &GeneralModel,
    duration_seconds: f64,
) -> bool {
    let start = effect.start_seconds;
    let holds_allowed = effect.mode != EffectMode::Hold
        || !matches!(effect.target, EffectTarget::State | EffectTarget::Command);
    let ends_inside = effect
        .end_seconds
        .is_none_or(|end| end.is_finite() && end > start && end <= duration_seconds);
    identity_valid(&effect.id)
        && effect.channel < channel_count(effect.target, model)
        && holds_allowed
        && start.is_finite()
        && start >= 0.0
        && start < duration_seconds
        && ends_inside
}

/// The number of channels of a target in the model: the inputs for a process input or a command, the states for a
/// state (a transfer function has its denominator's length less one), and the outputs otherwise.
fn channel_count(target: EffectTarget, model: &GeneralModel) -> usize {
    match target {
        EffectTarget::ProcessInput | EffectTarget::Command => model.identity().inputs.len(),
        EffectTarget::State => match model {
            GeneralModel::TransferFunction { transfer, .. } => {
                transfer.denominator.len().saturating_sub(1)
            }
            GeneralModel::ContinuousStateSpace { states, .. }
            | GeneralModel::DiscreteStateSpace { states, .. } => states.len(),
        },
        EffectTarget::ProcessOutput | EffectTarget::Measurement => model.identity().outputs.len(),
    }
}

/// The identity rule of the native table: a letter, then letters, digits, `_` or `-` (`^[A-Za-z][A-Za-z0-9_-]*$`).
fn identity_valid(id: &str) -> bool {
    let mut characters = id.chars();
    characters
        .next()
        .is_some_and(|first| first.is_ascii_alphabetic())
        && characters.all(|character| {
            character.is_ascii_alphanumeric() || character == '_' || character == '-'
        })
}

/// The text of one table: its header line, then one `key = value` line per field, each ending in a newline.
fn table_text(fields: &[(&str, String)]) -> String {
    let mut text = String::from("[[plants.uncertainties]]\n");
    for (key, value) in fields {
        text.push_str(key);
        text.push_str(" = ");
        text.push_str(value);
        text.push('\n');
    }
    text
}

/// A word in the table as JSON writes it. The identities and the words here hold no character that JSON escapes.
fn quoted(word: &str) -> String {
    format!("\"{word}\"")
}

fn target_word(target: EffectTarget) -> &'static str {
    match target {
        EffectTarget::Command => "command",
        EffectTarget::ProcessInput => "process_input",
        EffectTarget::ProcessOutput => "process_output",
        EffectTarget::Measurement => "measurement",
        EffectTarget::State => "state",
    }
}

fn mode_word(mode: EffectMode) -> &'static str {
    match mode {
        EffectMode::Add => "add",
        EffectMode::Multiply => "multiply",
        EffectMode::Replace => "replace",
        EffectMode::Hold => "hold",
    }
}

fn generator_word(generator: EffectGenerator) -> &'static str {
    match generator {
        EffectGenerator::Constant => "constant",
        EffectGenerator::UniformWhite => "uniform_white",
        EffectGenerator::Pulse => "pulse",
    }
}

#[cfg(test)]
#[path = "engineer_scenario_effects_tests.rs"]
mod tests;
