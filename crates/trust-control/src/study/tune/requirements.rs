//! The requirement record Tune designs to (specification 35, B4.3; Tune page specification, "Control requirements";
//! `automaticEngineerRequirements.ts`): the values the study's previous Tune section holds, the method and the response of
//! the request, and every missing test value, which Automatic fills from the model, the loop's limits and the model's
//! operating point. The policy that decides the values is [`fill_requirements`]; this module gathers its evidence.
//!
//! The policy also reads the identification recording: the recorded range of the input and of the output, the first
//! recorded values when the model has no operating point, the recording's length for a model without a decay rate, and the
//! baseline noise. Tune is not passed the recording yet, so these come from the model and the loop alone; the noise is zero.

#![allow(clippy::module_name_repetitions)] // `requirements_for` names the record the module builds.

use trust_control_math::frequency::FrequencyModel;
use trust_control_math::linalg::ComplexValue;
use trust_control_math::model::{OperatingPoint, Timing};

use crate::identification::plant::IdentifiedModel;
use crate::identification::refusal::Refusal;
use crate::loop_definition::LoopDefinition;
use crate::messages::{TuneChoice, TuneMethod, TuneRequest};
use crate::model::{GeneralModel, ModelChannel};
use crate::study::current_model::CurrentModel;
use crate::study::inputs::InputsSection;
use crate::study::tune::model::tuning_model;
use crate::study::tune::TuneSection;
use crate::tuning::automatic_criteria::{fill_requirements, FillEvidence, SuppliedRequirements};
use crate::tuning::candidate_margins::{loop_value, magnitude, plant_transfer};
use crate::tuning::objective::{TuningMethod, TuningObjective};
use crate::tuning::requirements::TuningRequirements;

/// The test's low frequency as a fraction of the inverse model timescale (`0.001 / timescale`).
const LOW_FREQUENCY_FACTOR: f64 = 0.001;
/// The timescale's multiple of the task period (`10 · period`).
const PERIOD_MULTIPLE: f64 = 10.0;
/// The timescale's multiple of the dead time (`2 · delay`).
const DELAY_MULTIPLE: f64 = 2.0;
/// The decay rate, in 1/s, that a pole must exceed to give the timescale its time (`rate > 1e-8`).
const MINIMUM_POLE_RATE: f64 = 1e-8;

/// The refusal of a model without its operating point (`automaticEngineerRequirements.ts`).
const NEEDS_OPERATING_POINT: &str =
    "The model needs its operating point before automatic tuning setup.";
/// The refusal of a model whose low-frequency response cannot be calculated (`automaticEngineerRequirements.ts`).
const NO_CHANNEL_RESPONSE: &str =
    "The model has no usable channel response for automatic test settings.";
/// The refusal of a model that does not have one input and one output (`connectedModelChannels` of `loopChannels.ts`).
const UNMAPPED_CHANNELS: &str = "Map every model input and output in loop channel bindings.";

/// The requirement record Tune designs to (B4.3): every value the previous Tune section holds, the request's method, the
/// request's response when it declares one, and each missing test value filled by Automatic from the evidence of the
/// model and the loop ([`fill_requirements`]). A value the record marks as generated is filled again, so it follows the
/// current model; every other value is kept exactly. The Automatic method declares no response of its own, so without a
/// response in the request it is filled as More robust (Tune page, "Control requirements"), whatever an earlier run of a
/// formula method declared; a formula method keeps the declared response.
///
/// The record depends on the model alone, not on the recording it was fitted from: the record-dependent evidence of the policy
/// (the recorded ranges, the operating values of a model without an operating point, the recording's length and the baseline
/// noise) is not used.
///
/// `held` is the requirements the study holds when the engineer recorded them (the inputs of `controlStudio/inputs`, B4.3). They
/// are the engineer's values, kept as given, and they take the place of the earlier Tune section's record: the generated values
/// of that record are not carried over, because the engineer now owns every value the record holds. Without `held`, the earlier
/// section's record carries over as before.
///
/// # Errors
/// Returns a [`Refusal`] when the model does not have one input and one output, has no finite operating point or no
/// usable low-frequency response, or when [`fill_requirements`] refuses the record: a stored criteria record that is not
/// valid, a supplied value that is not finite, an operating point outside its limits, no visible test change, a test change
/// that leaves the identified range, or a validation window longer than the runtime supports.
pub(crate) fn requirements_for(
    loop_definition: &LoopDefinition,
    model: &IdentifiedModel,
    general: &GeneralModel,
    previous: Option<&TuneSection>,
    held: Option<&TuningRequirements>,
    request: &TuneRequest,
) -> Result<TuningRequirements, Refusal> {
    requirements_with(
        loop_definition,
        model,
        general,
        previous,
        held,
        request.method,
        request.choice,
    )
}

/// [`requirements_for`] with the method and the declared response of the request, which are the only parts of a request the
/// record reads. The Identify answer calls it with the method and the response the study's inputs name
/// ([`study_requirements`]), so both answers fill one record by one rule.
pub(crate) fn requirements_with(
    loop_definition: &LoopDefinition,
    model: &IdentifiedModel,
    general: &GeneralModel,
    previous: Option<&TuneSection>,
    held: Option<&TuningRequirements>,
    method: Option<TuneMethod>,
    choice: Option<TuneChoice>,
) -> Result<TuningRequirements, Refusal> {
    let evidence = fill_evidence(loop_definition, model, general)?;
    let recorded = held.or_else(|| previous.map(|section| &section.requirements));
    let supplied = supplied_requirements(
        recorded,
        method.and_then(TuneMethod::formula),
        choice.map(TuneChoice::objective),
    );
    // A response the request declares is the engineer's value, so it is no longer the one Tune generated before (B4.3).
    let criteria = recorded
        .and_then(|requirements| requirements.automatic_criteria.as_ref())
        .map(|criteria| {
            let mut criteria = criteria.clone();
            if choice.is_some() {
                criteria.fields.retain(|field| field != "tuningObjective");
            }
            criteria
        });
    let mut filled = fill_requirements(&evidence, supplied, criteria.as_ref())?;
    // Automatic fills the record's own values. Each controlled output's record is the engineer's, so the filled record keeps it as
    // given: the validation's currency compares the requirements the study holds (`ValidateSection::is_current`).
    filled.channel_requirements = recorded.and_then(|record| record.channel_requirements.clone());
    Ok(filled)
}

/// The requirements the study's current model needs now, as Tune designs to them (B4.3; the answers name them): the values the
/// study records as given, the method and the declared response its inputs name (the request that the automatic run's Tune is
/// given from the same inputs), and Automatic's values for the rest, listed in `automaticCriteria`. `None` when Automatic cannot
/// form the record or the record breaks its rules: Tune then names the missing prerequisite when it runs (B4.7).
#[must_use]
pub(crate) fn study_requirements(
    loop_definition: &LoopDefinition,
    current: &CurrentModel,
    previous: Option<&TuneSection>,
    inputs: Option<&InputsSection>,
) -> Option<TuningRequirements> {
    let model = tuning_model(current).ok()?;
    let requirements = requirements_with(
        loop_definition,
        &model,
        &current.model,
        previous,
        inputs.and_then(|inputs| inputs.requirements.as_ref()),
        inputs.and_then(|inputs| inputs.method),
        inputs.and_then(InputsSection::declared_response),
    )
    .ok()?;
    requirements.validate().ok()?;
    Some(requirements)
}

/// The values Tune keeps (B4.3; Tune page, "Control requirements"): every value of the previous record, the request's
/// method in place of the record's, and the request's response when it declares one. The Automatic method (`method` is
/// `None`) without a declared response leaves the response to the policy, which fills it as More robust; a formula method
/// without one keeps the record's response.
fn supplied_requirements(
    recorded: Option<&TuningRequirements>,
    method: Option<TuningMethod>,
    choice: Option<TuningObjective>,
) -> SuppliedRequirements {
    let mut supplied = recorded.map_or_else(SuppliedRequirements::default, recorded_values);
    supplied.tuning_method = method;
    if choice.is_some() || method.is_none() {
        supplied.tuning_objective = choice;
    }
    supplied
}

/// The values a stored requirement record holds, each as a supplied value. [`fill_requirements`] fills again those the
/// record marks as generated.
fn recorded_values(requirements: &TuningRequirements) -> SuppliedRequirements {
    SuppliedRequirements {
        reference_change: Some(requirements.reference_change),
        maximum_error: Some(requirements.maximum_error),
        settling_band: Some(requirements.settling_band),
        settling_time_seconds: Some(requirements.settling_time_seconds),
        duration_seconds: Some(requirements.duration_seconds),
        constant_input_disturbance: requirements.constant_input_disturbance,
        measurement_noise_band: requirements.measurement_noise_band,
        gain_uncertainty_fraction: requirements.gain_uncertainty_fraction,
        transfer_tolerance: requirements.transfer_tolerance,
        maximum_overshoot_percent: requirements.maximum_overshoot_percent,
        tuning_method: requirements.tuning_method,
        tuning_objective: requirements.tuning_objective,
    }
}

/// The numbers Automatic fills the test from (B4.3; `automaticEngineerRequirements.ts`): the model's revision and dead
/// time, the loop's measurement and output with their units, the task period, the model timescale, the operating point,
/// the output and input ranges, the output's rate limits, and the model's response at the test's low frequency. A rate
/// limit the loop does not declare is no limit: the input then has its whole headroom and takes no time to slew, where a
/// declared zero stops it. The baseline noise needs the recording, which Tune is not passed, so it is zero.
fn fill_evidence(
    loop_definition: &LoopDefinition,
    model: &IdentifiedModel,
    general: &GeneralModel,
) -> Result<FillEvidence, Refusal> {
    let identity = general.identity();
    let ([input_channel], [output_channel]) =
        (identity.inputs.as_slice(), identity.outputs.as_slice())
    else {
        return Err(Refusal::new(UNMAPPED_CHANNELS));
    };
    let (operating_input, operating_output) = operating_values(identity.operating_point.as_ref())?;
    let period = loop_definition.controller.task_period_ms / 1000.0;
    let delay = model.dead_time_seconds.unwrap_or(0.0);
    let timescale = timescale_seconds(model, period, delay);
    let response = low_frequency_response(model, delay, timescale)?;
    let (output_minimum, output_maximum) = output_range(loop_definition, output_channel);
    let (input_minimum, input_maximum) = input_range(loop_definition, input_channel);
    let signals = &loop_definition.signals;
    let limits = &loop_definition.limits;
    Ok(FillEvidence {
        model_revision: identity.revision.clone(),
        output_symbol: signals.process_variable.symbol.clone(),
        output_unit: signals.process_variable.unit.clone(),
        input_symbol: signals.manipulated_variable.symbol.clone(),
        period_seconds: period,
        delay_seconds: delay,
        timescale_seconds: timescale,
        operating_output,
        operating_input,
        output_minimum,
        output_maximum,
        input_minimum,
        input_maximum,
        input_rate_up_per_s: limits.rate_up_per_s.unwrap_or(f64::INFINITY),
        input_rate_down_per_s: limits.rate_down_per_s.unwrap_or(f64::INFINITY),
        low_frequency_real: response.real,
        low_frequency_magnitude: magnitude(response),
        baseline_noise: 0.0,
    })
}

/// The model's operating input and output (`model.operatingPoint`), each finite. The policy falls back on the first
/// recorded values, which Tune is not passed, so a model without its operating point is refused.
fn operating_values(point: Option<&OperatingPoint>) -> Result<(f64, f64), Refusal> {
    point
        .and_then(|point| point.inputs.first().zip(point.outputs.first()))
        .map(|(&input, &output)| (input, output))
        .filter(|(input, output)| input.is_finite() && output.is_finite())
        .ok_or_else(|| Refusal::new(NEEDS_OPERATING_POINT))
}

/// The model timescale (Tune page: "a conservative model timescale, resolved delay and sample period"): the longest of ten
/// task periods, twice the dead time, and the time `1/|p|` of each pole `p = −1/τ` of the model whose decay rate exceeds
/// `10⁻⁸` per second. An integrator's pole at zero gives no time. For a model without such a pole the policy also takes a
/// tenth of the recording's length, which needs the recording; without it, its fallback is one task period, which ten
/// task periods cover.
fn timescale_seconds(model: &IdentifiedModel, period: f64, delay: f64) -> f64 {
    [
        model.time_constant_seconds,
        model.second_time_constant_seconds,
    ]
    .into_iter()
    .flatten()
    .filter_map(|time_constant| {
        let rate = time_constant.recip().abs();
        (rate.is_finite() && rate > MINIMUM_POLE_RATE).then_some(rate.recip())
    })
    .fold(
        (PERIOD_MULTIPLE * period).max(DELAY_MULTIPLE * delay),
        f64::max,
    )
}

/// The model's response at the test's low frequency `ω = 0.001 / timescale`: `G(jω)` of the tuning model's transfer
/// function with its dead time, for example `K·e^(−jωL)/(jωT + 1)` for a first-order form, `K′·e^(−jωL)/(jω)` for an
/// integrating one, and the product of the two lags for a second-order one.
fn low_frequency_response(
    model: &IdentifiedModel,
    delay: f64,
    timescale: f64,
) -> Result<ComplexValue, Refusal> {
    let plant = FrequencyModel::Transfer {
        transfer: plant_transfer(model),
        timing: Timing::Continuous,
        delay_seconds: delay,
    };
    loop_value(&plant, LOW_FREQUENCY_FACTOR / timescale)
        .map_err(|_| Refusal::new(NO_CHANNEL_RESPONSE))
}

/// The outputs a test may reach (`automaticEngineerRequirements.ts`): the measurement's signal range, within the model
/// output's operating range and the setpoint's signal range. The recorded range of the measurement, which the policy also
/// takes, needs the recording.
fn output_range(loop_definition: &LoopDefinition, channel: &ModelChannel) -> (f64, f64) {
    let signals = &loop_definition.signals;
    let measurement = signals.process_variable.range;
    let setpoint = signals.setpoint.as_ref().map(|setpoint| setpoint.range);
    (
        measurement
            .minimum
            .max(channel.operating_range.minimum)
            .max(setpoint.map_or(f64::NEG_INFINITY, |range| range.minimum)),
        measurement
            .maximum
            .min(channel.operating_range.maximum)
            .min(setpoint.map_or(f64::INFINITY, |range| range.maximum)),
    )
}

/// The inputs the actuator and the model admit (`automaticEngineerRequirements.ts`): the manipulated variable's signal
/// range, within the model input's operating range and the output limits the loop declares. A limit the loop does not
/// declare bounds nothing. The recorded range of the input, which the policy also takes, needs the recording.
fn input_range(loop_definition: &LoopDefinition, channel: &ModelChannel) -> (f64, f64) {
    let limits = &loop_definition.limits;
    let signal = loop_definition.signals.manipulated_variable.range;
    (
        signal
            .minimum
            .max(channel.operating_range.minimum)
            .max(limits.output_minimum.unwrap_or(f64::NEG_INFINITY)),
        signal
            .maximum
            .min(channel.operating_range.maximum)
            .min(limits.output_maximum.unwrap_or(f64::INFINITY)),
    )
}

#[cfg(test)]
#[path = "requirements_tests.rs"]
mod tests;
