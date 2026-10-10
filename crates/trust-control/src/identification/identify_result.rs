//! What an identification gives the page and the command line (specification 35, A8.4 and B3.8; Identify page specification,
//! "Identified model and plot"): the signals an identification used, the readable result of a stored Identify section with
//! its response plot, and the evidence the study gives the stages.
//!
//! The result is read from the stored section, never from a fit run again (A8.6): the model type and the key numbers come
//! from its presentation ([`identified_model_type_text`], [`identified_model_key_numbers`]), and the plot from the traces of
//! the experiment and the predicted response the presentation keeps.

use crate::experiment::ControlExperimentKind;
use crate::identification::experiment_trace::ExperimentTrace;
use crate::identification::presentation::{
    identified_model_key_numbers, identified_model_type_text,
};
use crate::loop_definition::LoopDefinition;
use crate::messages::{IdentifyPlot, IdentifyResult, PlotRole, PlotTrace};
use crate::stages::{StageAvailability, StageEvidence};
use crate::study::identify::{IdentifySection, IdentifySignals};

/// What the text adds to the model type of a provisional model (B3.5).
const PROVISIONAL_LABEL: &str = " · Provisional";

/// The signals an identification of `kind` uses on `loop_definition` (B3.2): the manipulated variable as the plant input,
/// the measurement as the measured output, and the setpoint, by symbol, only for a setpoint step (closed-loop
/// identification, B3.3).
#[must_use]
pub fn identify_signals(
    loop_definition: &LoopDefinition,
    kind: ControlExperimentKind,
) -> IdentifySignals {
    let signals = &loop_definition.signals;
    IdentifySignals {
        inputs: vec![signals.manipulated_variable.symbol.clone()],
        outputs: vec![signals.process_variable.symbol.clone()],
        setpoint: signals
            .setpoint
            .as_ref()
            .filter(|_| kind == ControlExperimentKind::SetpointStep)
            .map(|setpoint| setpoint.symbol.clone()),
    }
}

/// The result of the identification a section stores (B3.8): its model type, equation and key numbers, whether it is
/// provisional, the readable text, the response plot, the recording the model was fitted from, and `stages`. The text is the
/// model type, " · Provisional" for a provisional model, then the equation and the key numbers, each on its own line.
#[must_use]
pub fn identify_result(
    section: &IdentifySection,
    stages: Vec<StageAvailability>,
) -> IdentifyResult {
    let shown = &section.presentation.presentation;
    let model_type = identified_model_type_text(shown);
    let key_numbers = identified_model_key_numbers(shown);
    let label = if section.provisional {
        PROVISIONAL_LABEL
    } else {
        ""
    };
    let text = format!("{model_type}{label}\n{}\n{key_numbers}", shown.equation);
    IdentifyResult {
        model_type,
        equation: shown.equation.clone(),
        key_numbers,
        provisional: section.provisional,
        text,
        plot: response_plot(section),
        recording_id: section.estimation_recording_id.clone(),
        stages,
    }
}

/// The evidence of a study of `loop_definition` that sets the stages' status: the loop is bound, a model exists when the
/// study's Identify section is current for the loop (A8.4), and `has_recording` says whether the study holds a recording.
#[must_use]
pub fn study_evidence(
    loop_definition: &LoopDefinition,
    identify: Option<&IdentifySection>,
    has_recording: bool,
) -> StageEvidence {
    StageEvidence {
        loop_bound: true,
        has_model: identify.is_some_and(|section| section.is_current_for(loop_definition)),
        has_recording,
        ..StageEvidence::default()
    }
}

/// One trace before it is placed on the plot's time base: its role, symbol and unit, and its values at its own times.
struct Series<'a> {
    role: PlotRole,
    symbol: &'a str,
    unit: &'a str,
    points: Vec<(f64, Option<f64>)>,
}

/// The response plot of a section (B3.8, B9.7): the applied plant inputs and the measured outputs of the experiment, then the
/// model's prediction, on the times of the recording. Without the experiment's traces the measured output is the measured
/// value of each fitted point, as the page drew it. Nothing is interpolated or resampled.
fn response_plot(section: &IdentifySection) -> IdentifyPlot {
    let comparison = &section.presentation;
    let shown = &comparison.presentation;
    let fitted = &shown.predicted_response;
    let predicted_output = comparison
        .output_id
        .as_deref()
        .or_else(|| {
            section
                .selected_model
                .identity()
                .outputs
                .first()
                .map(|channel| channel.id.as_str())
        })
        .unwrap_or_default();
    let mut series = Vec::new();
    if let Some(experiment) = &comparison.experiment {
        series.extend(
            experiment
                .inputs
                .iter()
                .map(|trace| traced(PlotRole::PlantInput, trace)),
        );
        series.extend(
            experiment
                .outputs
                .iter()
                .map(|trace| traced(PlotRole::Measurement, trace)),
        );
    } else if !fitted.is_empty() {
        series.push(Series {
            role: PlotRole::Measurement,
            symbol: section
                .signals
                .outputs
                .first()
                .map_or(predicted_output, String::as_str),
            unit: &shown.units.output,
            points: fitted
                .iter()
                .map(|point| (point.time_seconds, Some(point.measured)))
                .collect(),
        });
    }
    if !fitted.is_empty() {
        series.push(Series {
            role: PlotRole::Prediction,
            symbol: predicted_output,
            unit: &shown.units.output,
            points: fitted
                .iter()
                .map(|point| (point.time_seconds, Some(point.predicted)))
                .collect(),
        });
    }
    on_one_time_base(series)
}

/// The series of an experiment trace.
fn traced(role: PlotRole, trace: &ExperimentTrace) -> Series<'_> {
    Series {
        role,
        symbol: &trace.id,
        unit: &trace.unit,
        points: trace
            .points
            .iter()
            .map(|point| (point.time_seconds, point.value))
            .collect(),
    }
}

/// The series on one time base: every finite time any of them has, in increasing order, and each series' value at each of
/// those times, `None` where it has none (B10.3: a missing value stays missing).
fn on_one_time_base(series: Vec<Series<'_>>) -> IdentifyPlot {
    let mut time_seconds: Vec<f64> = series
        .iter()
        .flat_map(|trace| trace.points.iter().map(|(time, _)| *time))
        .filter(|time| time.is_finite())
        .collect();
    time_seconds.sort_by(f64::total_cmp);
    time_seconds.dedup_by(|later, earlier| later.total_cmp(earlier).is_eq());
    let traces = series
        .into_iter()
        .map(|trace| {
            let mut values = vec![None; time_seconds.len()];
            for (time, value) in trace.points {
                if let Ok(index) = time_seconds.binary_search_by(|probe| probe.total_cmp(&time)) {
                    values[index] = value.filter(|value| value.is_finite());
                }
            }
            PlotTrace {
                role: trace.role,
                symbol: trace.symbol.to_owned(),
                unit: trace.unit.to_owned(),
                values,
            }
        })
        .collect();
    IdentifyPlot {
        time_seconds,
        traces,
    }
}

#[cfg(test)]
#[path = "identify_result_tests.rs"]
pub(crate) mod tests;
