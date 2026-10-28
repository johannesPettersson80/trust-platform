//! The manual method (specification 35, B4.2 and B4.5; numerical annex N5.9 and C0): the controller settings the engineer
//! enters are the candidate. They are checked against the bounds of their parameters, merged with the pinned values, and
//! recorded with the rules of every other candidate (`candidate_record`, C0 and C1). A preview gives the same candidate and
//! stores nothing (B4.5). Nothing here reaches a runtime (B4.1).

use std::collections::BTreeMap;

use crate::failure::{Cause, Failure, NextAction};
use crate::loop_definition::{LoopDefinition, Parameter};
use crate::messages::{TuneMethod, TuneRequest};
use crate::study::tune::design::Design;
use crate::study::tune::record::{candidate_record, record_shaping, LoopLimits};
use crate::study::tune::settings_refused;
use crate::tuning::candidate::{CandidateSource, LawCandidate};
use crate::tuning::objective::TuningObjective;
use crate::tuning::project_candidate::{
    project_candidate, served_seed, served_trajectory, trajectory_candidate,
};
use crate::tuning::project_seed::TrajectoryFeedforwardSeed;
use crate::tuning::settings_pins::{
    apply_pins, mapped_parameters, merge_manual_entry, ControllerSettingsPins, GainKey,
    MappedParameter,
};
use crate::tuning::structure::{ParallelGains, Structure};
use crate::tuning::texts;

/// The first setting of an entry that the loop does not bind, or whose value lies outside the bounds of its parameter or is
/// not finite, with the symbol of its setting and its sentence (B4.5; `outOfBounds` of `manualPreview.ts`). The symbol names the
/// setting's field, under which the page shows the refusal. The settings are read in the order of their symbols.
#[must_use]
pub fn first_outside_bounds(
    loop_definition: &LoopDefinition,
    settings: &BTreeMap<String, f64>,
) -> Option<(String, String)> {
    let parameters: &[Parameter] = match &loop_definition.parameter_binding {
        Some(binding) => &binding.parameters,
        None => &[],
    };
    settings.iter().find_map(|(symbol, value)| {
        let Some(parameter) = parameters
            .iter()
            .find(|parameter| &parameter.symbol == symbol)
        else {
            return Some((symbol.clone(), texts::not_a_setting(symbol)));
        };
        if value.is_nan() {
            let sentence = texts::setting_not_a_number(
                symbol,
                parameter.minimum,
                parameter.maximum,
                &parameter.unit,
            );
            return Some((symbol.clone(), sentence));
        }
        let inside =
            value.is_finite() && *value >= parameter.minimum && *value <= parameter.maximum;
        (!inside).then(|| {
            let sentence = texts::outside_bounds(
                symbol,
                parameter.minimum,
                parameter.maximum,
                &parameter.unit,
                *value,
            );
            (symbol.clone(), sentence)
        })
    })
}

/// The sentence of the first setting that [`first_outside_bounds`] refuses.
#[must_use]
pub fn outside_bounds(
    loop_definition: &LoopDefinition,
    settings: &BTreeMap<String, f64>,
) -> Option<String> {
    first_outside_bounds(loop_definition, settings).map(|(_, sentence)| sentence)
}

/// The candidate of a request that uses the manual method (B4.2, B4.5): the settings it enters, for the structure the request
/// fixes or, when it fixes none, the family of the pins (`pins`, resolved for the design) or the structure its entered gains give (C0).
/// `None` for any other method.
///
/// # Errors
/// The failure of [`manual_candidate`], or the manual method without the settings it takes.
pub(crate) fn candidate_of(
    loop_definition: &LoopDefinition,
    request: &TuneRequest,
    pins: Option<&ControllerSettingsPins>,
) -> Result<Option<LawCandidate>, Failure> {
    if request.method != Some(TuneMethod::Manual) {
        return Ok(None);
    }
    let Some(entered) = &request.settings else {
        return Err(Failure::new(
            texts::MANUAL_NEEDS_SETTINGS,
            Cause::Settings,
            NextAction::None,
        ));
    };
    let family = request
        .structure
        .or(pins.map(|pins| pins.family))
        .unwrap_or_else(|| structure_of_entry(&mapped_parameters(loop_definition), entered));
    manual_candidate(loop_definition, family, entered, pins).map(Some)
}

/// The candidate of entered settings for a structure (B4.2, C0, C1): every setting the structure needs must be entered, a
/// pinned value keeps its value, and the bounds and pins apply to the gains. The record is then written by
/// `candidate_record`, as every design writes it; a program's own law takes the gains and their seed variables instead (C2).
/// The gains that the structure does not take are left at zero.
///
/// # Errors
/// The settings refusals of [`merge_manual_entry`] and [`apply_pins`] (cause Settings, no next action), and the record's
/// refusals, which [`candidate_record`] gives.
pub(crate) fn manual_candidate(
    loop_definition: &LoopDefinition,
    family: Structure,
    entered: &BTreeMap<String, f64>,
    pins: Option<&ControllerSettingsPins>,
) -> Result<LawCandidate, Failure> {
    let mapped = mapped_parameters(loop_definition);
    let pinned = pins.map_or_else(BTreeMap::new, |pins| pins.values.clone());
    let values =
        merge_manual_entry(family, entered, &pinned, &mapped).map_err(settings_refused())?;
    let applied = apply_pins(
        &loop_definition.revision,
        family,
        &keyed_values(&values, &mapped),
        pins,
        &mapped,
    )
    .map_err(settings_refused())?;
    let gains = family_gains(family, &applied);
    if let Some(trajectory) = served_trajectory(loop_definition) {
        let feedforward = trajectory_feedforward(trajectory, &values)?;
        return Ok(LawCandidate::Project(trajectory_candidate(
            trajectory,
            CandidateSource::Manual,
            gains,
            feedforward,
        )));
    }
    if let Some(seed) = served_seed(loop_definition) {
        return Ok(LawCandidate::Project(project_candidate(
            seed,
            CandidateSource::Manual,
            gains,
        )));
    }
    candidate_record(
        gains,
        CandidateSource::Manual,
        &LoopLimits::of(loop_definition),
        record_shaping(loop_definition, &applied),
        applied.get(&GainKey::BackCalculationGain).copied(),
    )
    .map(LawCandidate::Pid)
}

/// The design of an entered candidate, for the answer of a run (B4.4): its own gains and structure, the manual settings'
/// reason and description, and no formula's `λ`.
#[must_use]
pub(crate) fn design_of(candidate: &LawCandidate, objective: TuningObjective) -> Option<Design> {
    Some(Design {
        source: CandidateSource::Manual,
        structure: candidate.structure()?,
        gains: candidate.gains()?,
        objective,
        closed_loop_time_seconds: None,
        attempts: Vec::new(),
        reason: texts::MANUAL_READY.to_owned(),
        description: Some(texts::MANUAL_DESCRIPTION.to_owned()),
    })
}

/// The rate and acceleration feedforward gains of a trajectory-following law's manual entry (C11): both must be entered, or pinned, as
/// finite numbers. A missing one refuses the candidate, so Apply never writes a zero that the engineer did not enter.
fn trajectory_feedforward(
    seed: &TrajectoryFeedforwardSeed,
    values: &BTreeMap<String, f64>,
) -> Result<(f64, f64), Failure> {
    let entered = |symbol: &str| {
        values
            .get(symbol)
            .copied()
            .filter(|value| value.is_finite())
    };
    match (
        entered(&seed.rate_symbol),
        entered(&seed.acceleration_symbol),
    ) {
        (Some(rate), Some(acceleration)) => Ok((rate, acceleration)),
        _ => Err(Failure::new(
            texts::REQUIRED_SETTING_MISSING,
            Cause::Settings,
            NextAction::None,
        )),
    }
}

/// The values of the bound parameters, by the record field each holds (C0).
fn keyed_values(
    values: &BTreeMap<String, f64>,
    mapped: &[MappedParameter],
) -> BTreeMap<GainKey, f64> {
    mapped
        .iter()
        .filter_map(|parameter| {
            values
                .get(&parameter.symbol)
                .map(|value| (parameter.key, *value))
        })
        .collect()
}

/// The structure that entered gains give when none is fixed (C0): integral action for a nonzero integral gain, and
/// derivative action for a nonzero derivative gain.
fn structure_of_entry(mapped: &[MappedParameter], entered: &BTreeMap<String, f64>) -> Structure {
    let keyed = keyed_values(entered, mapped);
    let value = |key: GainKey| keyed.get(&key).copied().unwrap_or(0.0);
    Structure::of_gains(value(GainKey::IntegralGain), value(GainKey::DerivativeGain))
}

/// The gains of a structure's record (C0): the proportional gain always, the integral gain for a structure with integral
/// action, and the derivative gain for a structure with derivative action; the others are zero.
fn family_gains(family: Structure, applied: &BTreeMap<GainKey, f64>) -> ParallelGains {
    let value = |key: GainKey| applied.get(&key).copied().unwrap_or(0.0);
    ParallelGains {
        proportional_gain: value(GainKey::ProportionalGain),
        integral_gain: if family.has_integral() {
            value(GainKey::IntegralGain)
        } else {
            0.0
        },
        derivative_gain: if family.has_derivative() {
            value(GainKey::DerivativeGain)
        } else {
            0.0
        },
    }
}

#[cfg(test)]
#[path = "manual_tests.rs"]
mod tests;
