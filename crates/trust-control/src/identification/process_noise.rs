//! The measured noise of the output as single executions see it (`processNoiseEvidence.ts`; specification 35, A6.4 and
//! A4.2). It is three standard deviations of the measured output about its mean over the steady baseline, times the square
//! root of the executions each recorded sample averages (A4.2). The baseline is the window the request names (B3.4, step
//! 2), or for a recording without one the samples before the first input change.

use crate::experiment::{baseline_end_seconds, experiment_start_seconds};
use crate::identification::connected_record::RuntimeVerificationResult;
use crate::identification::independent_validation::recording_fingerprint;
use crate::loop_definition::LoopDefinition;
use crate::model::GeneralModel;

/// Fewer baseline samples than this give no noise (`baseline.length < 10`).
const MINIMUM_BASELINE_SAMPLES: usize = 10;

/// One sample of the steady baseline: its time and the measured output.
#[derive(Debug, Clone, Copy, PartialEq)]
struct BaselineSample {
    time_seconds: f64,
    output: f64,
}

/// The measured process noise of the model's recording (`measuredProcessNoise`): the first of `runs` that the model was
/// fitted from, looked up by its fingerprint. The noise is zero without a baseline (A6.4): when there is no model, no such
/// recording, or fewer than ten baseline samples.
#[must_use]
pub fn measured_process_noise<E>(
    loop_definition: &LoopDefinition,
    model: Option<&GeneralModel>,
    runs: &[RuntimeVerificationResult<E>],
) -> f64 {
    let Some(model) = model else {
        return 0.0;
    };
    let evidence = &model.identity().evidence;
    let Some(record) = runs
        .iter()
        .find(|item| evidence.contains(&recording_fingerprint(&item.run)))
    else {
        return 0.0;
    };
    let baseline = baseline_samples(loop_definition, record);
    if baseline.len() < MINIMUM_BASELINE_SAMPLES {
        return 0.0;
    }
    let count = count_f64(baseline.len());
    let mean = baseline.iter().map(|sample| sample.output).sum::<f64>() / count;
    // The sample variance: the squares about the mean, over `n − 1`.
    let variance = baseline
        .iter()
        .map(|sample| (sample.output - mean).powi(2))
        .sum::<f64>()
        / (count - 1.0);
    let times: Vec<f64> = baseline.iter().map(|sample| sample.time_seconds).collect();
    let executions = executions_per_sample(loop_definition, &times);
    3.0 * variance.sqrt() * executions.sqrt()
}

/// The baseline samples of a record. A record that names its steady window takes the samples from its start to its end
/// (the wait before the window never counts). Otherwise the samples are taken while the input holds its first value, from
/// the first sample on.
fn baseline_samples<E>(
    loop_definition: &LoopDefinition,
    record: &RuntimeVerificationResult<E>,
) -> Vec<BaselineSample> {
    let process = loop_definition.signals.process_variable.symbol.as_str();
    let manipulated = loop_definition.signals.manipulated_variable.symbol.as_str();
    let request = &record.request;
    let mut samples = Vec::new();
    if let Some(end) =
        baseline_end_seconds(request.baseline_start_seconds, request.baseline_end_seconds)
    {
        let start = experiment_start_seconds(request.baseline_start_seconds);
        for sample in &record.run.samples {
            if sample.time_seconds > end + 1e-9 {
                break;
            }
            let Some(output) = sample.values.get(process).and_then(|value| value.value) else {
                continue;
            };
            if sample.time_seconds >= start - 1e-9 {
                samples.push(BaselineSample {
                    time_seconds: sample.time_seconds,
                    output,
                });
            }
        }
        return samples;
    }
    let mut held: Option<f64> = None;
    for sample in &record.run.samples {
        let input = sample.values.get(manipulated).and_then(|value| value.value);
        let output = sample.values.get(process).and_then(|value| value.value);
        let (Some(input), Some(output)) = (input, output) else {
            break;
        };
        let first = *held.get_or_insert(input);
        if (input - first).abs() > 1e-9 * first.abs().max(1.0) {
            break;
        }
        samples.push(BaselineSample {
            time_seconds: sample.time_seconds,
            output,
        });
    }
    samples
}

/// A sample count as a float. The counts here are sample counts, far below 2^53, so the conversion is exact.
#[allow(clippy::cast_precision_loss)]
fn count_f64(count: usize) -> f64 {
    count as f64
}

/// The executions one recorded sample averages (`executionsPerSample`): the median spacing of the sample times, in
/// milliseconds, over the loop's task period, rounded, and at least one (A4.2, A4.3).
fn executions_per_sample(loop_definition: &LoopDefinition, times: &[f64]) -> f64 {
    let mut spacings: Vec<f64> = times.windows(2).map(|pair| pair[1] - pair[0]).collect();
    spacings.sort_by(f64::total_cmp);
    let median = spacings.get(spacings.len() / 2).copied().unwrap_or(0.0);
    (median * 1000.0 / loop_definition.controller.task_period_ms)
        .round()
        .max(1.0)
}

#[cfg(test)]
#[path = "process_noise_tests.rs"]
mod tests;
