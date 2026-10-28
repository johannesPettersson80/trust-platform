//! Tests of the live sides (B3.6): a loop without channels offers its manipulated and process variables; a loop with channels offers
//! the signals its channels bind, in the channels' order.

use super::{live_sides, SideSignal};
use crate::loop_definition::LoopDefinition;

/// The shipped two-input, two-output loop (lesson 61): its inputs are the manipulated variable and a second heater, its outputs the
/// process variable and a second vessel temperature, each bound by a channel.
fn mimo_loop() -> LoopDefinition {
    LoopDefinition::decode(include_str!(
        "../../../../examples/control/10-multivariable-control/61-mimo-paths/control-loop.json"
    ))
    .expect("lesson 61's loop")
}

#[test]
fn a_loop_with_channels_offers_the_signals_its_channels_bind_in_channel_order() {
    let sides = live_sides(&mimo_loop());
    let inputs: Vec<(&str, &str, &str)> = sides
        .inputs
        .iter()
        .map(|signal| {
            (
                signal.signal_id.as_str(),
                signal.symbol.as_str(),
                signal.unit.as_str(),
            )
        })
        .collect();
    assert_eq!(
        inputs,
        [
            ("manipulatedVariable", "P1.Heater1Pct", "%"),
            ("heater-2", "P1.Heater2Pct", "%"),
        ]
    );
    let outputs: Vec<(&str, &str, &str)> = sides
        .outputs
        .iter()
        .map(|signal| {
            (
                signal.signal_id.as_str(),
                signal.symbol.as_str(),
                signal.unit.as_str(),
            )
        })
        .collect();
    assert_eq!(
        outputs,
        [
            ("processVariable", "P1.Vessel1C", "degC"),
            ("vessel-2", "P1.Vessel2C", "degC"),
        ]
    );
}

#[test]
fn a_loop_without_channels_offers_its_manipulated_variable_and_its_process_variable() {
    let loop_definition = LoopDefinition::decode(include_str!(
        "../../../../examples/control-pid-commissioning/control-loop.json"
    ))
    .expect("the commissioning example's loop");
    assert!(
        loop_definition.channels.is_none(),
        "the example declares no channels"
    );
    let sides = live_sides(&loop_definition);
    let manipulated = &loop_definition.signals.manipulated_variable;
    let process = &loop_definition.signals.process_variable;
    assert_eq!(
        sides.inputs,
        vec![SideSignal {
            signal_id: "manipulatedVariable".to_owned(),
            symbol: manipulated.symbol.clone(),
            unit: manipulated.unit.clone(),
        }]
    );
    assert_eq!(
        sides.outputs,
        vec![SideSignal {
            signal_id: "processVariable".to_owned(),
            symbol: process.symbol.clone(),
            unit: process.unit.clone(),
        }]
    );
}

#[test]
fn the_side_signals_are_among_the_signals_the_plots_draw_with_their_identifier_and_range_width() {
    // The live plot draws a side's signal by looking its symbol up in the list the plots draw from (B9.6): lesson 61's own signals
    // are `pv` (0 to 80) and `mv` (0 to 100), and its second heater and second vessel are additional signals by their identifiers.
    let sides = live_sides(&mimo_loop());
    let plotted = sides
        .plot_signals
        .expect("the open answer lists the signals the plots draw");
    let by_symbol = |symbol: &str| {
        plotted
            .iter()
            .find(|signal| signal.symbol == symbol)
            .map(|signal| (signal.id.as_str(), signal.span))
    };
    assert_eq!(by_symbol("P1.Vessel1C"), Some(("pv", 80.0)));
    assert_eq!(by_symbol("P1.Heater1Pct"), Some(("mv", 100.0)));
    assert_eq!(by_symbol("P1.Vessel2C"), Some(("vessel-2", 80.0)));
    assert_eq!(by_symbol("P1.Heater2Pct"), Some(("heater-2", 100.0)));
    for side in sides.inputs.iter().chain(&sides.outputs) {
        assert!(
            by_symbol(&side.symbol).is_some(),
            "{} is a side signal the plots can draw",
            side.symbol
        );
    }
}
