//! The words of a Tune answer (the Tune page specification, "Result and plot"): the margins that the engine names in words, the
//! outcome sentence, and the labels of the setting rows of a PID law. The engine composes them, so the page only shows them. Each
//! rule and each text is the one the page composed before (`threePageResults.ts`, `controllerGroupPresentation.ts`).

use std::collections::{BTreeMap, BTreeSet};

use schemars::JsonSchema;
use serde::Serialize;
use trust_control_math::margins::MarginValue;

use super::{CandidateOutcome, TuneResult};
use crate::loop_definition::{
    ControllerLaw, ControllerProject, LoopDefinition, Parameter, PidControllerGroup,
    ProjectSeedBlock,
};
use crate::tuning::project_components::ProjectTuningComponent;
use crate::tuning::project_seed::ParallelPidSeed;
use crate::tuning::state_controller::design::{HorizonPolicy, StateHorizon};
use crate::tuning::structure::Structure;

/// The label of the proportional gain, as `Control.PID` names it (`PID_GAIN_LABELS.ProportionalGain`).
const PROPORTIONAL_LABEL: &str = "Proportional gain (Kp)";
/// The label of the integral gain, as `Control.PID` names it (`PID_GAIN_LABELS.IntegralGain`).
const INTEGRAL_LABEL: &str = "Integral gain (Ki)";
/// The label of the derivative gain, as `Control.PID` names it (`PID_GAIN_LABELS.DerivativeGain`).
const DERIVATIVE_LABEL: &str = "Derivative gain (Kd)";
/// The label of the proportional setpoint weight `β` in the manual editor (`manualTuneSettings.ts`).
const SETPOINT_WEIGHT_LABEL: &str = "Setpoint weight (β)";
/// The label of the derivative setpoint weight `γ` in the manual editor (`manualTuneSettings.ts`).
const DERIVATIVE_WEIGHT_LABEL: &str = "Derivative setpoint weight (γ)";
/// The label of the measurement filter time `Tf` in the manual editor (`manualTuneSettings.ts`).
const FILTER_TIME_LABEL: &str = "Measurement filter time (Tf)";

/// The words of a Tune answer that the page shows as text (the Tune page specification, "Result and plot").
#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct TuneWords {
    /// The gain margin in words, when it is not a number: `unbounded`, or `not observed in range`. Absent when it is finite.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gain_margin: Option<String>,
    /// The phase margin in words, when it is not a number (the words of `gain_margin`). Absent when it is finite.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub phase_margin: Option<String>,
    /// The delay margin in words, when it is not a number (the words of `gain_margin`). Absent when it is finite.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub delay_margin: Option<String>,
    /// The outcome sentence: `Meets the requirements`, `Misses: ` and the shortfall, or `No prediction: ` and the reason.
    pub outcome: String,
    /// The rows of the settings that the page draws for a PID law, in the order of the loop's parameter binding, each with its label.
    /// A setting without a finite value has no row. Empty when no row applies.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub setting_labels: Vec<SettingLabel>,
    /// The names of `Control.PID`'s gains as the engine words them (C6, C7, C9, C10): the labels that the structure tables of the page show.
    pub gain_labels: GainLabels,
    /// The line that names a state law's finite horizon and its policy, such as `Finite horizon: 3 steps · Repeated first gain` (C4,
    /// N5.14). Absent for an answer without a finite horizon. The page shows the line and keeps no table of policy words.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub horizon_policy: Option<String>,
    /// The heading of the finite horizon's stages, such as `Riccati stages`; the stages of a design that a pin fixed are the nominal ones
    /// (D-B9). Absent where `horizon_policy` is.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub horizon_stages: Option<String>,
}

/// One row of the settings that the page draws: the setting's symbol, the label of its row and its unit.
#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SettingLabel {
    /// The setting's symbol, as the loop's parameter binding names it.
    pub symbol: String,
    /// The label of the row.
    pub label: String,
    /// The setting's unit, as the parameter binding names it: `1` for a number without a unit. The page shows the value with it and
    /// reads no unit from the loop.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unit: Option<String>,
}

/// The names of `Control.PID`'s three gains, as the engine words them (C6, C7, C9, C10).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct GainLabels {
    /// The proportional gain's name.
    pub proportional: String,
    /// The integral gain's name.
    pub integral: String,
    /// The derivative gain's name.
    pub derivative: String,
}

impl GainLabels {
    /// The engine's names of the three gains: the labels of the PID path (`primary_labels`).
    #[must_use]
    pub fn engine() -> Self {
        Self {
            proportional: PROPORTIONAL_LABEL.to_owned(),
            integral: INTEGRAL_LABEL.to_owned(),
            derivative: DERIVATIVE_LABEL.to_owned(),
        }
    }
}

impl TuneWords {
    /// Gives a Tune answer its words (B4.4 to B4.6), so that a candidate that the open answer rebuilds shows the words a Tune run shows.
    pub fn attach(loop_definition: &LoopDefinition, result: &mut TuneResult) {
        let words = Self::of_result(loop_definition, result);
        result.words = Some(words);
    }

    /// The words of a Tune answer for the loop that it tunes: its margins in words, its outcome sentence and its setting rows
    /// (B4.4 to B4.6). The answer itself is not changed.
    #[must_use]
    pub fn of_result(loop_definition: &LoopDefinition, result: &TuneResult) -> Self {
        let (gain_margin, phase_margin) = result.margins.as_ref().map_or((None, None), |margins| {
            (
                margin_word(margins.gain_margin_db),
                margin_word(margins.phase_margin_degrees),
            )
        });
        let values = result
            .project_candidate
            .as_ref()
            .map_or(&result.settings, |project| &project.values);
        let horizon = result
            .state
            .as_ref()
            .and_then(|state| state.horizon.as_ref());
        Self {
            gain_margin,
            phase_margin,
            delay_margin: result.delay_margin_seconds.and_then(margin_word),
            outcome: result.outcome.sentence(),
            setting_labels: setting_labels(loop_definition, family_of(result), values),
            gain_labels: GainLabels::engine(),
            horizon_policy: horizon.map(horizon_policy_line),
            horizon_stages: horizon.map(|horizon| horizon_stages_heading(horizon).to_owned()),
        }
    }
}

/// The line that names a finite horizon and its policy (C4, N5.14): its number of steps and the policy that runs the design.
fn horizon_policy_line(horizon: &StateHorizon) -> String {
    let steps = horizon.steps;
    let noun = if steps == 1 { "step" } else { "steps" };
    let policy = match horizon.policy {
        HorizonPolicy::RepeatFirstFiniteGain => "Repeated first gain",
        HorizonPolicy::PinnedFixedGain => "Pinned fixed gain",
    };
    format!("Finite horizon: {steps} {noun} · {policy}")
}

/// The heading of a finite horizon's stages (D-B9): a pin fixes the gains of the design, so its stages are the nominal ones.
fn horizon_stages_heading(horizon: &StateHorizon) -> &'static str {
    match horizon.policy {
        HorizonPolicy::RepeatFirstFiniteGain => "Riccati stages",
        HorizonPolicy::PinnedFixedGain => "Nominal Riccati stages",
    }
}

impl CandidateOutcome {
    /// The outcome in the page's words: `Meets the requirements`, `Misses: ` and the shortfall, or `No prediction: ` and the reason
    /// (B4.4, B4.5).
    #[must_use]
    pub fn sentence(&self) -> String {
        match self {
            Self::Meets => "Meets the requirements".to_owned(),
            Self::Misses { shortfall } => format!("Misses: {shortfall}"),
            Self::Unpredicted { reason } => format!("No prediction: {reason}"),
        }
    }
}

/// The word of a margin that is not a number: `unbounded` for a margin the loop does not have, and `not observed in range` for a
/// margin that the grid does not show (N7.7, N7.8). A finite margin has no word.
fn margin_word(margin: MarginValue) -> Option<String> {
    match margin {
        MarginValue::Finite(_) => None,
        MarginValue::Unbounded => Some("unbounded".to_owned()),
        MarginValue::NotObservedInRange => Some("not observed in range".to_owned()),
    }
}

/// The structure of the candidate that the answer holds: a `Control.PID`'s, or else a program's own law's (C2).
fn family_of(result: &TuneResult) -> Option<Structure> {
    result
        .candidate
        .as_ref()
        .map(|candidate| candidate.structure)
        .or_else(|| {
            result
                .project_candidate
                .as_ref()
                .map(|project| project.structure)
        })
}

/// The rows of the settings that the page draws for a loop's PID path: each bound setting with a finite value, in the order of the
/// parameter binding, with the label that its gain or its seed names. A group member's label names its instance. The rows of a
/// served structure (a group, a cascade, a Smith predictor, an ON/OFF controller, a gain schedule) are its own section's, which the
/// page draws instead; their labels are the same as the PID path would give.
fn setting_labels(
    loop_definition: &LoopDefinition,
    family: Option<Structure>,
    values: &BTreeMap<String, f64>,
) -> Vec<SettingLabel> {
    let Some(binding) = &loop_definition.parameter_binding else {
        return Vec::new();
    };
    let controller = &loop_definition.controller;
    let project = controller.project.as_ref();
    let primary = primary_labels(
        controller.law.as_ref(),
        project.and_then(|project| project.seed.as_ref()),
        family,
    );
    let group = group_symbols(controller.group.as_ref(), project);
    binding
        .parameters
        .iter()
        .filter(|parameter| {
            values
                .get(&parameter.symbol)
                .is_some_and(|value| value.is_finite())
        })
        .filter(|parameter| {
            primary
                .as_ref()
                .is_none_or(|labels| labels.contains_key(&parameter.symbol))
        })
        .filter(|parameter| {
            group
                .as_ref()
                .is_none_or(|symbols| symbols.contains(&parameter.symbol))
        })
        .map(|parameter| {
            let text = primary
                .as_ref()
                .and_then(|labels| labels.get(&parameter.symbol).cloned())
                .unwrap_or_else(|| readable(convention_or_symbol(parameter)));
            SettingLabel {
                symbol: parameter.symbol.clone(),
                label: owned_label(controller.group.as_ref(), &parameter.symbol, &text),
                unit: Some(parameter.unit.clone()),
            }
        })
        .collect()
}

/// The label of each setting that a loop's law or seed names: a `Control.PID`'s gains by the terms of its family, a proportional law's
/// gain, or the seed's variables. `None` when the loop names none, so every bound setting keeps its own name.
fn primary_labels(
    law: Option<&ControllerLaw>,
    seed: Option<&ProjectSeedBlock>,
    family: Option<Structure>,
) -> Option<BTreeMap<String, String>> {
    match law {
        Some(ControllerLaw::Pid(pid)) => {
            let mut pairs = vec![pair(&pid.proportional_gain_symbol, PROPORTIONAL_LABEL)];
            if matches!(family, Some(Structure::PI | Structure::PID)) {
                pairs.push(pair(&pid.integral_gain_symbol, INTEGRAL_LABEL));
            }
            if matches!(family, Some(Structure::PD | Structure::PID)) {
                pairs.push(pair(&pid.derivative_gain_symbol, DERIVATIVE_LABEL));
            }
            Some(pairs.into_iter().collect())
        }
        Some(ControllerLaw::Proportional { gain_symbol }) => {
            Some(BTreeMap::from([pair(gain_symbol, PROPORTIONAL_LABEL)]))
        }
        None => seed.and_then(seeded_labels),
    }
}

/// The labels that a program's own seed gives its variables (C2, C6 to C9, C11): the terms of its controller and the names of its
/// feedforward or model variables. The page names no variable of a decentralized or decoupled group, nor of a seed of a parked kind,
/// so those give `None`.
fn seeded_labels(seed: &ProjectSeedBlock) -> Option<BTreeMap<String, String>> {
    let pairs = match seed {
        ProjectSeedBlock::ParallelPid(pid) => pid_pairs(pid, true),
        ProjectSeedBlock::TrajectoryFeedforward(trajectory) => {
            let mut pairs = pid_pairs(&trajectory.pid, true);
            pairs.push(pair(&trajectory.rate_symbol, "Rate feedforward (Kv)"));
            pairs.push(pair(
                &trajectory.acceleration_symbol,
                "Acceleration feedforward (Ka)",
            ));
            pairs
        }
        ProjectSeedBlock::Feedforward(feedforward) => {
            let mut pairs = pid_pairs(&feedforward.pid, false);
            pairs.push(pair(&feedforward.gain_symbol, "Feedforward gain"));
            if let Some(lead_lag) = &feedforward.lead_lag {
                pairs.push(pair(&lead_lag.lead_symbol, "Lead time"));
                pairs.push(pair(&lead_lag.lag_symbol, "Lag time"));
            }
            pairs
        }
        ProjectSeedBlock::Smith(smith) => {
            let mut pairs = pid_pairs(&smith.pid, false);
            pairs.push(pair(&smith.gain_symbol, "Model gain K"));
            pairs.push(pair(&smith.time_constant_symbol, "Model time constant T"));
            pairs.push(pair(&smith.delay_symbol, "Model delay θ"));
            pairs
        }
        ProjectSeedBlock::Cascade(cascade) => [&cascade.inner, &cascade.outer]
            .into_iter()
            .flat_map(|gains| {
                [
                    pair(
                        &gains.proportional_symbol,
                        &format!("{} · {PROPORTIONAL_LABEL}", gains.instance),
                    ),
                    pair(
                        &gains.integral_symbol,
                        &format!("{} · {INTEGRAL_LABEL}", gains.instance),
                    ),
                ]
            })
            .collect::<Vec<_>>(),
        ProjectSeedBlock::Decentralized
        | ProjectSeedBlock::Decoupled(_)
        | ProjectSeedBlock::Parked { .. } => return None,
    };
    Some(pairs.into_iter().collect())
}

/// The labelled gains of a parallel seed: its proportional gain, its integral gain when it has one, and its derivative gain when
/// `derivative` is set.
fn pid_pairs(pid: &ParallelPidSeed, derivative: bool) -> Vec<(String, String)> {
    let mut pairs = vec![pair(&pid.proportional_symbol, PROPORTIONAL_LABEL)];
    if let Some(symbol) = &pid.integral_symbol {
        pairs.push(pair(symbol, INTEGRAL_LABEL));
    }
    if let Some(symbol) = pid.derivative_symbol.as_ref().filter(|_| derivative) {
        pairs.push(pair(symbol, DERIVATIVE_LABEL));
    }
    pairs
}

/// The settings that a group's page may name (`groupSettingSymbols`): the search axes, the components' symbols, and, for a group
/// designed from its model (C7), each member's proportional and integral gain. `None` for a loop that is not a group.
fn group_symbols(
    group: Option<&PidControllerGroup>,
    project: Option<&ControllerProject>,
) -> Option<BTreeSet<String>> {
    let group = group?;
    let designed = matches!(
        project.and_then(|project| project.seed.as_ref()),
        Some(ProjectSeedBlock::Decentralized | ProjectSeedBlock::Decoupled(_))
    );
    let mut symbols = BTreeSet::new();
    if let Some(project) = project {
        symbols.extend(project.axes.iter().map(|axis| axis.symbol.clone()));
        symbols.extend(
            project
                .components
                .iter()
                .flat_map(ProjectTuningComponent::symbols)
                .map(str::to_owned),
        );
    }
    if designed {
        symbols.extend(
            group
                .members
                .iter()
                .flat_map(|member| &member.parameter_symbols)
                .filter(|symbol| {
                    symbol.ends_with(".ProportionalGain") || symbol.ends_with(".IntegralGain")
                })
                .cloned(),
        );
    }
    Some(symbols)
}

/// A group member's label names its instance: `<instance> · <label>` (`ownedParameterLabel`). The label's field name is read the way
/// the page read it: `Control.PID` and the spaces go, and the field `ProportionalGain`, `IntegralGain` or `DerivativeGain` takes the
/// gain's label; any other field is its words, split at each capital letter.
fn owned_label(group: Option<&PidControllerGroup>, symbol: &str, label: &str) -> String {
    let Some(owner) = group.and_then(|group| {
        group.members.iter().find(|member| {
            member
                .parameter_symbols
                .iter()
                .any(|candidate| candidate == symbol)
        })
    }) else {
        return label.to_owned();
    };
    let stripped = label
        .strip_prefix("Control.PID")
        .map_or(label, str::trim_start);
    let field: String = stripped.split_whitespace().collect();
    let name = match field.as_str() {
        "ProportionalGain" => PROPORTIONAL_LABEL.to_owned(),
        "IntegralGain" => INTEGRAL_LABEL.to_owned(),
        "DerivativeGain" => DERIVATIVE_LABEL.to_owned(),
        _ => split_camel(stripped, true),
    };
    format!("{} · {name}", owner.instance)
}

/// The settings that a loop's group may name (`groupSettingSymbols`, C7): the search axes, the components' symbols and, for a group
/// designed from its model, each member's proportional and integral gain. `None` for a loop that is not a group.
#[must_use]
pub(crate) fn group_setting_symbols(loop_definition: &LoopDefinition) -> Option<BTreeSet<String>> {
    group_symbols(
        loop_definition.controller.group.as_ref(),
        loop_definition.controller.project.as_ref(),
    )
}

/// The label of a setting in the manual editor (B4.2): a `Control.PID` law's gains, setpoint weights and filter time by their terms, a
/// proportional law's gain, and a seed's variables. A setting that none of them names is labelled by its convention, read as the page
/// read it. A group member's label names its instance.
#[must_use]
pub(crate) fn manual_setting_label(
    loop_definition: &LoopDefinition,
    parameter: &Parameter,
) -> String {
    let controller = &loop_definition.controller;
    let seed = controller
        .project
        .as_ref()
        .and_then(|project| project.seed.as_ref());
    let labelled = match controller.law.as_ref() {
        Some(ControllerLaw::Pid(pid)) => [
            (
                Some(pid.proportional_gain_symbol.as_str()),
                PROPORTIONAL_LABEL,
            ),
            (Some(pid.integral_gain_symbol.as_str()), INTEGRAL_LABEL),
            (Some(pid.derivative_gain_symbol.as_str()), DERIVATIVE_LABEL),
            (
                pid.proportional_setpoint_weight_symbol.as_deref(),
                SETPOINT_WEIGHT_LABEL,
            ),
            (
                pid.derivative_setpoint_weight_symbol.as_deref(),
                DERIVATIVE_WEIGHT_LABEL,
            ),
            (
                pid.measurement_filter_time_symbol.as_deref(),
                FILTER_TIME_LABEL,
            ),
        ]
        .into_iter()
        .find(|(symbol, _)| *symbol == Some(parameter.symbol.as_str()))
        .map(|(_, label)| label.to_owned()),
        Some(ControllerLaw::Proportional { gain_symbol }) => {
            (gain_symbol == &parameter.symbol).then(|| PROPORTIONAL_LABEL.to_owned())
        }
        None => seed
            .and_then(seeded_labels)
            .and_then(|labels| labels.get(&parameter.symbol).cloned()),
    };
    let text = labelled.unwrap_or_else(|| readable(convention_or_symbol(parameter)));
    owned_label(controller.group.as_ref(), &parameter.symbol, &text)
}

/// The page's `readable`: each word of a label that is not a dotted name is split at each lower-case letter that a capital follows,
/// and `-` and `_` become spaces.
fn readable(text: &str) -> String {
    text.split(' ')
        .map(|word| {
            if word.contains('.') {
                word.to_owned()
            } else {
                split_camel(word, false).replace(['-', '_'], " ")
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// A space goes between a lower-case letter (or, with `digits`, a digit) and the capital that follows it. With `digits` false this is
/// the page's `readable` split; with `digits` true it is the split of `ownedParameterLabel`.
fn split_camel(text: &str, digits: bool) -> String {
    let mut spaced = String::with_capacity(text.len() + 4);
    let mut previous: Option<char> = None;
    for current in text.chars() {
        let joins = previous.is_some_and(|before| {
            before.is_ascii_lowercase() || (digits && before.is_ascii_digit())
        });
        if joins && current.is_ascii_uppercase() {
            spaced.push(' ');
        }
        spaced.push(current);
        previous = Some(current);
    }
    spaced
}

/// The name a setting's row is read from: its convention, or its symbol when the convention is empty.
fn convention_or_symbol(parameter: &Parameter) -> &str {
    if parameter.convention.is_empty() {
        &parameter.symbol
    } else {
        &parameter.convention
    }
}

/// A symbol and its label, as the pairs of the label maps are built.
fn pair(symbol: &str, label: &str) -> (String, String) {
    (symbol.to_owned(), label.to_owned())
}

#[cfg(test)]
#[path = "tune_words_tests.rs"]
mod tests;
