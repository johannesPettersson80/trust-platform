//! The signals the plots draw (specification 35, B9.6, B9.7): the engine's list of the loop's signals by the identifier the plots name
//! them with, and the value each of them takes in a sample of a kept recording. The live plot and the recorded plot draw from the
//! same list (`liveSides`, §0.1a), so the page maps no signal onto the fields of a sample and holds no rule of which value is drawn.
//!
//! The identifiers are those of the project test's Signals selector (B6.1): `sp`, `pv`, `mv` (the output the process received),
//! `requestedMv` (the output the controller asked for, when the loop declares an actuator), then the identifier of each additional
//! signal of the loop.

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::Serialize;

use crate::loop_definition::{LoopDefinition, LoopSignal};
use crate::recording::channels::{
    binding_forced, binding_value, ChannelQuality, RecordedChannel, ACTUATOR_BINDING,
    MANIPULATED_VARIABLE_BINDING, PROCESS_VARIABLE_BINDING, SETPOINT_BINDING,
};
use crate::recording::{RecordedRun, RecordedSample};
use crate::signals::Quality;

/// The identifier of the setpoint among the plotted signals.
pub const SETPOINT: &str = "sp";
/// The identifier of the measurement among the plotted signals.
pub const PROCESS_VALUE: &str = "pv";
/// The identifier of the output the process received: the actuator's position when the loop declares one, else the manipulated variable.
pub const APPLIED_OUTPUT: &str = "mv";
/// The identifier of the output the controller requested, which only a loop with an actuator has apart from the applied output.
pub const REQUESTED_OUTPUT: &str = "requestedMv";

/// What a plotted signal is (B9.6, B9.9): the one vocabulary of the signals the plots draw. The page names a signal by its role and
/// colours it by its kind, and holds no list of identifiers of its own.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum PlotSignalKind {
    /// The setpoint (`sp`).
    Setpoint,
    /// The measurement (`pv`).
    Measurement,
    /// The output the process received (`mv`): the actuator's position when the loop declares one, else the manipulated variable.
    AppliedOutput,
    /// The output the controller requested (`requestedMv`), which only a loop with an actuator lists apart from the applied output.
    RequestedOutput,
    /// An additional signal of the loop, by its own identifier.
    Additional,
}

/// One signal the plots draw: its identifier, its kind, its variable and unit, and the width of its declared range.
#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct PlotSignal {
    /// The identifier the plots name the signal with, and the project test's Signals selector offers it by: `sp`, `pv`, `mv` (the
    /// output the process received), `requestedMv`, or the identifier of an additional signal of the loop.
    pub id: String,
    /// What the signal is, as the identifier says it: the page names and colours the signals by this and reads no identifier.
    pub kind: PlotSignalKind,
    /// The variable's path.
    pub symbol: String,
    /// The signal's unit; empty means dimensionless.
    pub unit: String,
    /// The width of the signal's declared range (the largest value less the smallest), by which the plot frames a flat value.
    pub span: f64,
    /// The symbol the experiment's recorded trace of this signal carries, when it is not `symbol`. The plant input an identification
    /// records is the output the process received, and a loop with an actuator records it under the manipulated variable's symbol.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recorded_as: Option<String>,
}

/// The signals the plots draw for a loop, in the order of the project test's Signals selector (B6.1): the setpoint when the loop has
/// one, the measurement, the applied output, the requested output when the loop declares an actuator, then the additional signals.
#[must_use]
pub fn plot_signals(loop_definition: &LoopDefinition) -> Vec<PlotSignal> {
    let signals = &loop_definition.signals;
    let manipulated = &signals.manipulated_variable;
    let mut listed = Vec::new();
    if let Some(setpoint) = &signals.setpoint {
        listed.push(plot_signal(
            SETPOINT,
            PlotSignalKind::Setpoint,
            setpoint,
            None,
        ));
    }
    listed.push(plot_signal(
        PROCESS_VALUE,
        PlotSignalKind::Measurement,
        &signals.process_variable,
        None,
    ));
    match &signals.actuator {
        Some(actuator) => {
            listed.push(plot_signal(
                APPLIED_OUTPUT,
                PlotSignalKind::AppliedOutput,
                actuator,
                Some(manipulated.symbol.clone()),
            ));
            listed.push(plot_signal(
                REQUESTED_OUTPUT,
                PlotSignalKind::RequestedOutput,
                manipulated,
                None,
            ));
        }
        None => listed.push(plot_signal(
            APPLIED_OUTPUT,
            PlotSignalKind::AppliedOutput,
            manipulated,
            None,
        )),
    }
    listed.extend(
        signals
            .additional
            .iter()
            .flatten()
            .map(|signal| PlotSignal {
                id: signal.id.clone(),
                kind: PlotSignalKind::Additional,
                symbol: signal.symbol.clone(),
                unit: signal.unit.clone(),
                span: signal.range.maximum - signal.range.minimum,
                recorded_as: None,
            }),
    );
    listed
}

fn plot_signal(
    id: &str,
    kind: PlotSignalKind,
    signal: &LoopSignal,
    recorded_as: Option<String>,
) -> PlotSignal {
    PlotSignal {
        id: id.to_owned(),
        kind,
        symbol: signal.symbol.clone(),
        unit: signal.unit.clone(),
        span: signal.range.maximum - signal.range.minimum,
        recorded_as,
    }
}

/// The identifier a recording binds the plotted signal `id` with: the setpoint, the measurement, the actuator or the manipulated
/// variable for the applied output (as `applied_mv` reads it), the manipulated variable for the requested output, and an additional
/// signal by its own identifier.
fn binding_of<'a>(loop_definition: &LoopDefinition, id: &'a str) -> &'a str {
    match id {
        SETPOINT => SETPOINT_BINDING,
        PROCESS_VALUE => PROCESS_VARIABLE_BINDING,
        APPLIED_OUTPUT if loop_definition.signals.actuator.is_some() => ACTUATOR_BINDING,
        APPLIED_OUTPUT | REQUESTED_OUTPUT => MANIPULATED_VARIABLE_BINDING,
        other => other,
    }
}

/// The value each plotted signal takes in `sample` of `run`, by the signal's identifier (B9.6): the value the plot draws, or `None`
/// where it draws a gap. A signal's value is drawn when it is there, finite, not forced, given good quality by the runtime (E4.4) and,
/// for an additional signal, not marked bad by its channel's quality signals (`additional`, the channels of the sample). A missing,
/// bad or forced value is a gap in its own trace and removes no good value of another signal (B9.6), nor is it interpolated (B10.3).
#[must_use]
pub fn recorded_values(
    loop_definition: &LoopDefinition,
    signals: &[PlotSignal],
    run: &RecordedRun,
    sample: &RecordedSample,
    additional: &[RecordedChannel],
) -> BTreeMap<String, Option<f64>> {
    signals
        .iter()
        .map(|signal| {
            let binding = binding_of(loop_definition, &signal.id);
            let (value, quality) =
                if let Some(channel) = additional.iter().find(|channel| channel.id == signal.id) {
                    (channel.value, channel.quality)
                } else {
                    binding_reading(run, sample, binding)
                };
            let drawn = value.filter(|value| {
                quality == ChannelQuality::Good
                    && reported_good(run, sample, binding)
                    && value.is_finite()
            });
            (signal.id.clone(), drawn)
        })
        .collect()
}

/// Whether the runtime gave the value of the binding `id` in `sample` good quality (E4.4): a value it reports forced, bad or missing is
/// not.
fn reported_good(run: &RecordedRun, sample: &RecordedSample, id: &str) -> bool {
    run.bindings
        .iter()
        .position(|binding| binding.id == id)
        .and_then(|index| sample.quality.get(index))
        == Some(&Quality::Good)
}

/// The value of the binding `id` in `sample` and its quality on its own: forced when the binding is forced, bad when it has no value,
/// good otherwise. Another signal's quality does not enter (B9.6).
fn binding_reading(
    run: &RecordedRun,
    sample: &RecordedSample,
    id: &str,
) -> (Option<f64>, ChannelQuality) {
    let value = binding_value(run, sample, id);
    let quality = if binding_forced(run, sample, id) {
        ChannelQuality::Forced
    } else if value.is_none() {
        ChannelQuality::Bad
    } else {
        ChannelQuality::Good
    };
    (value, quality)
}

#[cfg(test)]
#[path = "plot_signals_tests.rs"]
mod tests;
