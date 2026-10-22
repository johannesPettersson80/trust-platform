//! What a project-scripted experiment takes from its baseline (specification 35, C12 Baseline and B3.4 steps 2 and 4; numerical
//! annex N1.3).
//!
//! Control Studio waits for a steady baseline before an experiment it runs itself (B3.4 step 2). A project-scripted experiment that
//! declares a baseline waits for none: its recording starts with its plan, which holds the activation off for that baseline, and the
//! project qualifies its own measurements before it actuates (C12). What a later step takes from a baseline then comes from the
//! samples of the declared baseline: the noise a return to Auto allows for (A6.4), the window the fit takes its operating point from
//! (N1.3) and the baseline window of the record. The baseline runs from the recording's start to the last sample before the plan
//! enables the project: the project reads the activation and reacts in the same execution, so the sample at the instant the plan
//! enables it already holds the first change of the plant input. A project that declares no baseline waits for the steady baseline a
//! campaign waits for, so that its fit has the samples before the first input change that it takes its operating point from.

use crate::experiment::{declared_baseline_ms, ExperimentSample};
use crate::identification::campaign::baseline::{half_range, BaselineSummary};
use crate::identification::connected_run::{fit_window, FitWindow};
use crate::identification::policy::project_scripted;
use crate::identification::settle::MINIMUM_WINDOW_SAMPLES;
use crate::loop_definition::LoopDefinition;
use crate::messages::RecordedChartSample;
use crate::recording::channels::ChannelQuality;

/// The baseline the loop's project-scripted experiment declares, in s: zero when it declares none (see [`declared_baseline_ms`]).
fn declared_baseline_seconds(loop_definition: &LoopDefinition) -> f64 {
    let milliseconds = declared_baseline_ms(loop_definition);
    if milliseconds.is_finite() && milliseconds > 0.0 {
        milliseconds / 1_000.0
    } else {
        0.0
    }
}

/// The time of the last sample of the declared baseline, in s: one sample period before the instant the plan enables the project,
/// whose own sample already holds the first change of the plant input (N2.2). Zero when the declared baseline holds fewer than two
/// samples, which gives no window.
fn declared_baseline_end_seconds(loop_definition: &LoopDefinition) -> f64 {
    let period_seconds = loop_definition.observed_sample_timing.nominal_period_ms / 1_000.0;
    (declared_baseline_seconds(loop_definition) - period_seconds).max(0.0)
}

/// Whether a run of the loop takes the baseline its project declares instead of waiting for a steady one (C12 Baseline, B3.4 step 2):
/// a project-scripted experiment that declares a baseline of at least one sample. An experiment Control Studio runs, and a project that
/// declares no baseline, wait for the steady baseline.
#[must_use]
pub fn takes_the_declared_baseline(loop_definition: &LoopDefinition) -> bool {
    project_scripted(loop_definition).is_some() && declared_baseline_seconds(loop_definition) > 0.0
}

/// The baseline a run of a project-scripted experiment that declares one carries in place of a steady one, or `None` for a run that
/// waits for its steady baseline ([`takes_the_declared_baseline`]). The run journals it with the step and judges nothing by it: a project's
/// experiment is one observation (C12). Its window is the declared baseline. Its mean is the reading the run starts from, because
/// the declared baseline is recorded after the start, and it measures no noise and no range, which the declared baseline's samples
/// give once they are recorded ([`declared_baseline_noise`]).
#[must_use]
pub fn declared_baseline(
    loop_definition: &LoopDefinition,
    sample: Option<&ExperimentSample>,
) -> Option<BaselineSummary> {
    if !takes_the_declared_baseline(loop_definition) {
        return None;
    }
    Some(BaselineSummary {
        mean: sample.map_or(0.0, |sample| sample.process_variable.value),
        noise: 0.0,
        half_range: 0.0,
        spacing_seconds: loop_definition.controller.task_period_ms / 1_000.0,
        window_seconds: declared_baseline_seconds(loop_definition),
        input_mean: None,
        input_noise: None,
    })
}

/// Where the fit of a run's recording starts and where its baseline window ends (B3.4, N1.3). A campaign fits from one window before
/// the end of the steady baseline it waited for (`baseline_end_seconds`, [`fit_window`]), and so does a project that declares no
/// baseline. A project-scripted experiment that declares one waited for none: its window is the declared baseline, from the
/// recording's start to its last sample before the plan enables the project.
#[must_use]
pub fn campaign_fit_window(
    loop_definition: &LoopDefinition,
    baseline_end_seconds: f64,
    baseline: &BaselineSummary,
) -> FitWindow {
    if takes_the_declared_baseline(loop_definition) {
        FitWindow {
            fit_from_seconds: 0.0,
            baseline_end_seconds: declared_baseline_end_seconds(loop_definition),
        }
    } else {
        fit_window(baseline_end_seconds, baseline.window_seconds)
    }
}

/// Half the peak-to-peak range of the measured output over the declared baseline of a project-scripted experiment (N1.3): the noise
/// that the first output of a return to Auto is allowed to move by (A6.4). `samples` are the recording's samples with their process
/// time from its first scan. The baseline runs from the recording's start to its last sample before the plan enables the project
/// ([`campaign_fit_window`]), and its good samples count. `None` for a run that waits for its steady baseline, which gives it (an
/// experiment Control Studio runs, or a project that declares no baseline), and for a declared baseline that holds fewer than the
/// twenty samples a baseline needs (B3.4 step 2).
#[must_use]
pub fn declared_baseline_noise(
    loop_definition: &LoopDefinition,
    samples: &[RecordedChartSample],
) -> Option<f64> {
    if !takes_the_declared_baseline(loop_definition) {
        return None;
    }
    let baseline_end_seconds = declared_baseline_end_seconds(loop_definition);
    if baseline_end_seconds <= 0.0 {
        return None;
    }
    let values: Vec<f64> = samples
        .iter()
        .filter(|sample| {
            sample.elapsed_seconds <= baseline_end_seconds + 1e-9
                && sample.quality == ChannelQuality::Good
        })
        .filter_map(|sample| sample.pv.filter(|value| value.is_finite()))
        .collect();
    (values.len() >= MINIMUM_WINDOW_SAMPLES).then(|| half_range(&values))
}

#[cfg(test)]
#[path = "project_baseline_tests.rs"]
mod tests;
