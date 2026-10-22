//! The plant signals that the live selectors of the three-page binding offer (specification 35, B3.6): each side's signals as the
//! loop declares them, in the channels' order, or the loop's manipulated variable and process variable when it declares no channels.
//! The open answer carries them with the list of signals the plots draw (B9.6), and the page offers each side and draws each plot
//! from that answer; it resolves no signal of the loop itself.

use schemars::JsonSchema;
use serde::Serialize;

use super::plot_signals::{plot_signals, PlotSignal};
use crate::identification::connected_run::loop_signal_of;
use crate::loop_definition::LoopDefinition;

/// One plant signal of a live selector: its identifier in the loop, its variable's path and its unit.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SideSignal {
    /// The signal's identifier in the loop: `manipulatedVariable`, `processVariable`, or the signal of a channel.
    pub signal_id: String,
    /// The variable's path.
    pub symbol: String,
    /// The signal's unit; empty means dimensionless.
    pub unit: String,
}

/// The sides of the live selectors (B3.6): the plant inputs and the measured outputs, in the order the selectors list them, and the
/// signals the plots draw.
#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct LiveSides {
    /// The plant inputs, in channel order.
    pub inputs: Vec<SideSignal>,
    /// The measured outputs, in channel order.
    pub outputs: Vec<SideSignal>,
    /// The signals the live plot and the plot of a kept recording draw (B9.6), by the identifier of the project test's Signals selector,
    /// each with its variable, unit and range width. The values of a recorded sample are named by these identifiers (`plotted`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plot_signals: Option<Vec<PlotSignal>>,
}

/// The live sides of a loop (B3.6): the signals its channels bind, in the channels' order, with a channel that names no declared
/// signal left out; for a loop without channels, its manipulated variable as the input and its process variable as the output.
#[must_use]
pub fn live_sides(loop_definition: &LoopDefinition) -> LiveSides {
    let (inputs, outputs): (Vec<SideSignal>, Vec<SideSignal>) = match &loop_definition.channels {
        Some(channels) => (
            channels
                .inputs
                .iter()
                .filter_map(|channel| side_signal(loop_definition, &channel.signal_id))
                .collect(),
            channels
                .outputs
                .iter()
                .filter_map(|channel| side_signal(loop_definition, &channel.signal_id))
                .collect(),
        ),
        None => (
            side_signal(loop_definition, "manipulatedVariable")
                .into_iter()
                .collect(),
            side_signal(loop_definition, "processVariable")
                .into_iter()
                .collect(),
        ),
    };
    LiveSides {
        inputs,
        outputs,
        plot_signals: Some(plot_signals(loop_definition)),
    }
}

/// The loop's signal that an identifier names, as a side's signal (`loop_signal_of`).
fn side_signal(loop_definition: &LoopDefinition, id: &str) -> Option<SideSignal> {
    loop_signal_of(loop_definition, id).map(|signal| SideSignal {
        signal_id: id.to_owned(),
        symbol: signal.symbol,
        unit: signal.unit,
    })
}

#[cfg(test)]
#[path = "live_sides_tests.rs"]
mod tests;
