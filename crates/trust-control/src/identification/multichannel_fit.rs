//! The multichannel fit of a recording (`multichannelIdentification.ts`, `fitMultichannelRecordings`, and `identifyStateModel` of
//! `identifiedStateModel.ts`; specification 35, C7 and C8). A loop whose channels name two or more plant inputs or measured
//! outputs is identified into one discrete state-space model. Its recording, or its estimation and independent records, are placed
//! on an even grid; each sample holds the inputs and then the outputs; and the multichannel ARX fit of `trust-control-math`
//! (`identify::arx`) gives the model of the first order that its free-running check accepts. The model's channels are the loop's
//! model channel identifiers, and each refusal is the sentence the `TypeScript` threw.
//!
//! The one-input, one-output fits are `recorded_model`'s. The gate still parks a loop with channels (checklist V7), so this fit is
//! reached by its tests until the multichannel shapes are served.

use serde_json::json;
use trust_control_math::identify::arx::{self, Comparison, Outcome, Rejection};
use trust_control_math::model::OperatingPoint;
use trust_control_math::sampling::{evenly_sampled_grid, interpolate, EvenGrid};

use crate::identification::automatic::{good_value, FitComparison, GeneralIdentificationFit};
use crate::identification::connected_record::{
    RuntimeVerificationResult, ValidationRun, ValidationSample, ValidationValue,
};
use crate::identification::evidence::ModelFitPoint;
use crate::identification::independent_validation::{
    digest, recording_fingerprint, ValidationPredictionSeries,
};
use crate::identification::recorded_model::{maximum_normalized_error, policy_of};
use crate::identification::refusal::Refusal;
use crate::loop_definition::{channel_signal, ChannelSignal, LoopChannels, LoopDefinition};
use crate::model::{
    GeneralModel, ModelChannel, ModelIdentity, ModelSource, ModelValidation, OperatingRange,
};
use crate::recording::channels::ChannelQuality;

/// The fewest scans a record may hold (`evenlySampled`).
const MINIMUM_SCANS: usize = 30;

/// The model order the fit tries up to when the policy names none (`identifyStateModel`'s `maximumOrder = 4`).
const DEFAULT_MAXIMUM_ORDER: usize = 4;

/// The multichannel model fitted from one recording, or from a recording and its independent record: the model, the records it
/// was fitted from, and the fit the page shows.
#[derive(Debug, Clone, PartialEq)]
pub struct MultichannelIdentification<E> {
    /// The discrete state-space model, with the loop's model channel identifiers.
    pub model: GeneralModel,
    /// The records the model was fitted from: the estimation record, then the independent record when there is one.
    pub runs: Vec<RuntimeVerificationResult<E>>,
    /// The comparisons of the orders, and the predictions of the selected model on the records it was checked on.
    pub fit: GeneralIdentificationFit,
}

/// A loop channel bound to its loop signal. A measured output is interpolated between scans; an input is held.
struct Bound<'a> {
    id: &'a str,
    signal: ChannelSignal<'a>,
    output: bool,
}

/// Fits the multichannel model of a loop from one recording, or from two independent records (`fitMultichannelRecordings`). The
/// inputs come first in the model, then the outputs, each in the loop's order. The loop's policy gives the error limit (`0.15` by
/// default) and the maximum order (`4` by default); the rest of the rules are those of the fit in `trust-control-math`.
///
/// # Errors
/// The loop has no channels or breaks a binding rule; the records are not one or two independent records; a record is too short,
/// has a gap, or lacks a channel; a sample of a channel is not good; or no order meets the error limit on the comparison record.
pub fn fit_multichannel_model<E: Clone>(
    loop_definition: &LoopDefinition,
    runs: &[RuntimeVerificationResult<E>],
) -> Result<MultichannelIdentification<E>, Refusal> {
    let channels = loop_definition
        .channels
        .as_ref()
        .ok_or_else(|| Refusal::new("Ordered model channels are required."))?;
    channels
        .check_bindings(loop_definition)
        .map_err(Refusal::new)?;
    let (estimation, validation) = records(runs)?;
    let bound = bind(loop_definition, channels)?;
    let estimation_grid = grid_of(&estimation.run, None)?;
    let step = estimation_grid.step_seconds;
    if !step.is_finite() || step <= 0.0 {
        return Err(Refusal::new(
            "Identification requires a positive sample interval.",
        ));
    }
    let comparison = validation.map_or(&estimation.run, |run| &run.run);
    let comparison_grid = grid_of(comparison, Some(step))?;
    check_channels(&estimation.run, &bound)?;
    check_channels(comparison, &bound)?;
    let train = rows(&estimation.run, &estimation_grid, &bound)?;
    let held = rows(comparison, &comparison_grid, &bound)?;
    let inputs = channels.inputs.len();
    let fitted = arx::fit(
        &train,
        &held,
        inputs,
        maximum_order(loop_definition),
        maximum_normalized_error(loop_definition),
    )
    .map_err(|rejection| refusal(rejection, validation.is_some()))?;
    let model = state_space_model(
        &fitted,
        &bound,
        inputs,
        step,
        &estimation.run,
        validation.map(|run| &run.run),
    );
    let times: Vec<f64> = comparison_grid
        .points
        .iter()
        .map(|point| point.time_seconds)
        .collect();
    let series = predictions(
        &fitted,
        &held,
        &times,
        &bound,
        inputs,
        comparison,
        &model.identity().revision,
    );
    let (validation_predictions, estimation_predictions) = if validation.is_some() {
        (series, Vec::new())
    } else {
        (Vec::new(), series)
    };
    Ok(MultichannelIdentification {
        model,
        runs: runs.to_vec(),
        fit: GeneralIdentificationFit {
            comparisons: fit_comparisons(&fitted.comparisons, validation.is_some()),
            validation_predictions,
            estimation_predictions,
        },
    })
}

/// The estimation record and, when there are two records, the independent one. Two records must be distinct, and of one program
/// and model revision (`identifyStateModel`, `fitMultichannelRecordings`).
fn records<E>(
    runs: &[RuntimeVerificationResult<E>],
) -> Result<
    (
        &RuntimeVerificationResult<E>,
        Option<&RuntimeVerificationResult<E>>,
    ),
    Refusal,
> {
    match runs {
        [estimation] => Ok((estimation, None)),
        [estimation, validation] => {
            let distinct = estimation.run.id != validation.run.id;
            let same_revisions = estimation.project_revision == validation.project_revision
                && estimation.run.conditions.program_revision
                    == validation.run.conditions.program_revision
                && estimation.run.conditions.model_revision
                    == validation.run.conditions.model_revision;
            if distinct && same_revisions {
                Ok((estimation, Some(validation)))
            } else {
                Err(Refusal::new(
                    "Independent records must have distinct identities and the same plant/program revision.",
                ))
            }
        }
        _ => Err(Refusal::new(
            "One estimation recording or two independent recordings are required.",
        )),
    }
}

/// The loop's channels bound to their signals: the inputs in order, then the outputs in order.
fn bind<'a>(
    loop_definition: &'a LoopDefinition,
    channels: &'a LoopChannels,
) -> Result<Vec<Bound<'a>>, Refusal> {
    let inputs = channels.inputs.iter().map(|input| {
        (
            input.model_channel_id.as_str(),
            input.signal_id.as_str(),
            false,
        )
    });
    let outputs = channels.outputs.iter().map(|output| {
        (
            output.model_channel_id.as_str(),
            output.signal_id.as_str(),
            true,
        )
    });
    inputs
        .chain(outputs)
        .map(|(id, signal_id, output)| -> Result<Bound<'a>, Refusal> {
            let signal = channel_signal(loop_definition, signal_id).ok_or_else(|| {
                Refusal::new(format!(
                    "Channel binding {signal_id} does not identify a declared signal."
                ))
            })?;
            Ok(Bound { id, signal, output })
        })
        .collect()
}

/// The grid of a record: its scans held or interpolated onto `step`, or onto their median spacing when no step is given. A record
/// holds at least thirty scans and two timing samples, and its scans may not have gaps (`evenlySampled`).
fn grid_of(run: &ValidationRun, step: Option<f64>) -> Result<EvenGrid, Refusal> {
    if run.conditions.sample_times_seconds.len() < 2 {
        return Err(Refusal::new(
            "Identification requires complete sample timing for both independent records.",
        ));
    }
    if run.samples.len() < MINIMUM_SCANS {
        return Err(Refusal::new(
            "Identification requires complete sample coverage.",
        ));
    }
    let times: Vec<f64> = run
        .samples
        .iter()
        .map(|sample| sample.time_seconds)
        .collect();
    evenly_sampled_grid(&times, step, MINIMUM_SCANS)
        .map_err(|_| Refusal::new("Identification samples have gaps or inconsistent timing."))
}

/// Each bound channel is in the record with the same unit (`read` in `identifyStateModel`).
fn check_channels(run: &ValidationRun, bound: &[Bound<'_>]) -> Result<(), Refusal> {
    for channel in bound {
        let symbol = channel.signal.symbol;
        let present = run
            .channels
            .iter()
            .any(|item| item.id == symbol && item.unit == channel.signal.unit);
        if !present {
            return Err(Refusal::new(format!(
                "Missing identification channel {symbol} or incompatible unit."
            )));
        }
    }
    Ok(())
}

/// The rows of the fit: one per grid point, holding the inputs and then the outputs, each a good value.
fn rows(
    run: &ValidationRun,
    grid: &EvenGrid,
    bound: &[Bound<'_>],
) -> Result<Vec<Vec<f64>>, Refusal> {
    grid.points
        .iter()
        .map(|point| {
            let held = &run.samples[point.index];
            let after = if point.fraction > 0.0 {
                run.samples.get(point.index + 1)
            } else {
                None
            };
            bound
                .iter()
                .map(|channel| value_at(held, after, point.fraction, channel))
                .collect::<Result<Vec<f64>, Refusal>>()
        })
        .collect()
}

/// One channel's good value at a grid point (`read`): the held scan's value, or for a measured output the value interpolated
/// toward the next scan. A missing or not good value is refused.
fn value_at(
    held: &ValidationSample,
    after: Option<&ValidationSample>,
    fraction: f64,
    channel: &Bound<'_>,
) -> Result<f64, Refusal> {
    let symbol = channel.signal.symbol;
    let sampled = held.values.get(symbol).copied().map(|value| {
        let next = after.and_then(|sample| sample.values.get(symbol).copied());
        sampled_value(value, next, fraction, channel.output)
    });
    sampled
        .as_ref()
        .and_then(good_value)
        .ok_or_else(|| Refusal::new(format!("Unqualified identification data on {symbol}.")))
}

/// A channel's value at a grid point (`evenlySampled`): an output is interpolated toward the next scan when both values are
/// numbers, and takes the next scan's quality when its own is good; an input, or an output without a next value, is held.
fn sampled_value(
    held: ValidationValue,
    after: Option<ValidationValue>,
    fraction: f64,
    output: bool,
) -> ValidationValue {
    match (output, held.value, after) {
        (
            true,
            Some(value),
            Some(ValidationValue {
                value: Some(next),
                quality,
            }),
        ) => ValidationValue {
            value: Some(interpolate(value, next, fraction)),
            quality: if held.quality == ChannelQuality::Good {
                quality
            } else {
                held.quality
            },
        },
        _ => held,
    }
}

/// The state space of the fitted order, in the companion form the fit gives: its states are named and ranged by their channel, its
/// channels are the loop's, its operating point is the first estimation sample, and its evidence is the records' fingerprints.
fn state_space_model(
    fitted: &arx::Fit,
    bound: &[Bound<'_>],
    inputs: usize,
    sample_time: f64,
    estimation: &ValidationRun,
    validation: Option<&ValidationRun>,
) -> GeneralModel {
    let outputs = bound.len() - inputs;
    let states: Vec<ModelChannel> = (0..fitted.model.states())
        .map(|index| {
            history_state(
                index,
                bound,
                inputs,
                outputs,
                fitted.order,
                &fitted.operating_point,
            )
        })
        .collect();
    let mut evidence = vec![recording_fingerprint(estimation)];
    evidence.extend(validation.map(recording_fingerprint));
    let system = fitted.model.clone();
    let revision = digest(&json!({
        "a": &system.a,
        "b": &system.b,
        "c": &system.c,
        "d": &system.d,
        "sampleTime": sample_time,
        "estimation": &estimation.id,
        "validation": validation.map(|run| run.id.as_str()),
    }));
    GeneralModel::DiscreteStateSpace {
        identity: ModelIdentity {
            revision,
            source: ModelSource::Fitted,
            validation: if validation.is_some() {
                ModelValidation::Validated
            } else {
                ModelValidation::Unvalidated
            },
            evidence,
            inputs: bound[..inputs].iter().map(model_channel).collect(),
            outputs: bound[inputs..].iter().map(model_channel).collect(),
            delay_seconds: 0.0,
            operating_point: Some(OperatingPoint {
                inputs: fitted.operating_point[..inputs].to_vec(),
                outputs: fitted.operating_point[inputs..].to_vec(),
            }),
        },
        states,
        system,
        sample_time_seconds: sample_time,
    }
}

/// The name, unit and operating range of one state of the companion form: an output state is an output at one of its lags, and an
/// input state an input at one of its lags (`identifyStateModel`). Its range is its channel's range less the operating point.
fn history_state(
    index: usize,
    bound: &[Bound<'_>],
    inputs: usize,
    outputs: usize,
    order: usize,
    origin: &[f64],
) -> ModelChannel {
    let output_states = outputs * order;
    let global = if index < output_states {
        inputs + index % outputs
    } else {
        (index - output_states) % inputs
    };
    let channel = &bound[global];
    ModelChannel {
        id: format!("history-{index}"),
        unit: channel.signal.unit.to_owned(),
        operating_range: OperatingRange {
            minimum: channel.signal.range.minimum - origin[global],
            maximum: channel.signal.range.maximum - origin[global],
        },
    }
}

/// A loop channel as a model channel: its identifier, its signal's unit and its signal's range.
fn model_channel(channel: &Bound<'_>) -> ModelChannel {
    ModelChannel {
        id: channel.id.to_owned(),
        unit: channel.signal.unit.to_owned(),
        operating_range: OperatingRange {
            minimum: channel.signal.range.minimum,
            maximum: channel.signal.range.maximum,
        },
    }
}

/// The free-running predictions of each output on the comparison record, with the time of each sample and its measured value
/// (`validationPredictions`, `estimationPredictions`). The first prediction is for the sample at index `order`.
fn predictions(
    fitted: &arx::Fit,
    held: &[Vec<f64>],
    times: &[f64],
    bound: &[Bound<'_>],
    inputs: usize,
    comparison: &ValidationRun,
    model_revision: &str,
) -> Vec<ValidationPredictionSeries> {
    bound[inputs..]
        .iter()
        .enumerate()
        .map(|(output, channel)| ValidationPredictionSeries {
            channel_id: channel.id.to_owned(),
            unit: channel.signal.unit.to_owned(),
            recording_id: comparison.id.clone(),
            model_revision: model_revision.to_owned(),
            points: fitted.predictions[output]
                .iter()
                .enumerate()
                .map(|(step, predicted)| {
                    let index = fitted.order + step;
                    let measured = held[index][inputs + output];
                    ModelFitPoint {
                        time_seconds: times[index],
                        measured,
                        predicted: *predicted,
                        residual: measured - *predicted,
                    }
                })
                .collect(),
        })
        .collect()
}

/// The comparisons the fit made, as the page shows them: each order with its normalised errors and the sentence that says whether
/// it is adequate, which depends on whether the comparison record is independent.
fn fit_comparisons(comparisons: &[Comparison], independent: bool) -> Vec<FitComparison> {
    comparisons
        .iter()
        .map(|compared| match &compared.outcome {
            Outcome::Indistinguishable => FitComparison {
                order: compared.order,
                normalized_errors: None,
                reason: comparison_reason(compared, independent).to_owned(),
                adequate: false,
            },
            Outcome::Evaluated {
                normalized_errors,
                adequate,
            } => FitComparison {
                order: compared.order,
                normalized_errors: Some(normalized_errors.clone()),
                reason: reason(*adequate, independent).to_owned(),
                adequate: *adequate,
            },
        })
        .collect()
}

/// The sentence of one compared order (`identifyStateModel`'s `reason`).
fn comparison_reason(compared: &Comparison, independent: bool) -> &'static str {
    match &compared.outcome {
        Outcome::Indistinguishable => {
            "Parameters are not distinguishable with the supplied excitation."
        }
        Outcome::Evaluated { adequate, .. } => reason(*adequate, independent),
    }
}

/// The sentence of an evaluated order: whether it meets the error limit, on the independent record or on the estimation record.
fn reason(adequate: bool, independent: bool) -> &'static str {
    match (independent, adequate) {
        (true, true) => "Every held-out output meets the declared error limit.",
        (true, false) => "Independent free-running predictions exceed the declared error limit.",
        (false, true) => "Estimation fit meets the error limit; independent validation is pending.",
        (false, false) => "Estimation fit exceeds the error limit.",
    }
}

/// The refusal of a fit that gives no model, in the words `identifyStateModel` throws. When no order is adequate, each order
/// that was compared is named with its reason.
fn refusal(rejection: Rejection, independent: bool) -> Refusal {
    match rejection {
        Rejection::InvalidLimit => {
            Refusal::new("Invalid identification adequacy or model-order limit.")
        }
        Rejection::InvalidRecords => {
            Refusal::new("Identification requires complete sample coverage.")
        }
        Rejection::Unrepresentable => {
            Refusal::new("The identified model is not a valid state space.")
        }
        Rejection::NoAdequateOrder { comparisons } => {
            let orders: Vec<String> = comparisons
                .iter()
                .map(|compared| {
                    format!(
                        "Order {}: {}",
                        compared.order,
                        comparison_reason(compared, independent)
                    )
                })
                .collect();
            Refusal::new(format!(
                "No identifiable adequate state model. {}",
                orders.join(" ")
            ))
        }
    }
}

/// The largest model order the policy allows, or the default.
fn maximum_order(loop_definition: &LoopDefinition) -> usize {
    policy_of(loop_definition)
        .and_then(|policy| policy.maximum_order)
        .map_or(DEFAULT_MAXIMUM_ORDER, usize::from)
}

#[cfg(test)]
#[path = "multichannel_fit_tests.rs"]
mod tests;
