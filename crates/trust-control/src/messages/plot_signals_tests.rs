#![allow(clippy::float_cmp)] // Declared range widths are compared exactly on purpose.

//! Tests of the signals the plots draw (specification 35, B9.6, B6.1): the list is the project test's Signals selector, with each
//! signal's variable, unit and range width, and a loop with an actuator records the applied output under the manipulated variable.

use serde_json::json;

use super::{plot_signals, PlotSignal, PlotSignalKind};
use crate::identification::project_test::project_test_form;
use crate::loop_definition::{Direction, LoopDefinition, LoopSignal, Range};
use crate::recording::fixtures::loop_with_disturbance;

/// The fixture loop with an actuator `Valve.Position` over `minimum` to `maximum` percent.
fn with_actuator(minimum: f64, maximum: f64) -> LoopDefinition {
    let mut loop_definition = loop_with_disturbance();
    loop_definition.signals.actuator = Some(LoopSignal {
        symbol: "Valve.Position".to_owned(),
        address: Some("%IL4".to_owned()),
        iec_type: "LREAL".to_owned(),
        direction: Direction::Read,
        unit: "%".to_owned(),
        range: Range { minimum, maximum },
    });
    loop_definition
}

fn listed(signals: &[PlotSignal]) -> Vec<(&str, &str, &str, f64)> {
    signals
        .iter()
        .map(|signal| {
            (
                signal.id.as_str(),
                signal.symbol.as_str(),
                signal.unit.as_str(),
                signal.span,
            )
        })
        .collect()
}

#[test]
fn the_plots_draw_the_loops_signals_by_the_identifiers_of_the_signals_selector() {
    // The fixture loop declares a setpoint, a measurement, a manipulated variable and the feed flow, each over 0 to 100 percent.
    let signals = plot_signals(&loop_with_disturbance());
    assert_eq!(
        listed(&signals),
        [
            ("sp", "LevelSP", "%", 100.0),
            ("pv", "LevelPV", "%", 100.0),
            ("mv", "ValveMV", "%", 100.0),
            ("feed-flow", "FeedFlow", "%", 100.0),
        ]
    );
    assert!(
        signals.iter().all(|signal| signal.recorded_as.is_none()),
        "without an actuator every trace carries its signal's own symbol"
    );
}

#[test]
fn an_actuator_is_the_applied_output_and_the_manipulated_variable_the_requested_one() {
    // The actuator ranges over -10 to 30, so its range is 40 wide; the manipulated variable keeps its 0 to 100.
    let signals = plot_signals(&with_actuator(-10.0, 30.0));
    assert_eq!(
        listed(&signals),
        [
            ("sp", "LevelSP", "%", 100.0),
            ("pv", "LevelPV", "%", 100.0),
            ("mv", "Valve.Position", "%", 40.0),
            ("requestedMv", "ValveMV", "%", 100.0),
            ("feed-flow", "FeedFlow", "%", 100.0),
        ]
    );
    // An identification records the plant input, which is the applied output, under the manipulated variable's symbol.
    let recorded: Vec<(&str, Option<&str>)> = signals
        .iter()
        .map(|signal| (signal.id.as_str(), signal.recorded_as.as_deref()))
        .collect();
    assert_eq!(
        recorded,
        [
            ("sp", None),
            ("pv", None),
            ("mv", Some("ValveMV")),
            ("requestedMv", None),
            ("feed-flow", None),
        ]
    );
}

#[test]
fn each_plotted_signal_names_its_kind() {
    // B9.6, B9.9: the page names a signal by its role and colours it by its kind, and holds no list of identifiers. The kinds follow the
    // identifiers by hand: `sp` the setpoint, `pv` the measurement, `mv` the output the process received, `requestedMv` the output the
    // controller asked for, and every further signal of the loop an additional one, whatever its identifier.
    let kinds = |loop_definition: &LoopDefinition| -> Vec<(String, PlotSignalKind)> {
        plot_signals(loop_definition)
            .into_iter()
            .map(|signal| (signal.id, signal.kind))
            .collect()
    };
    assert_eq!(
        kinds(&loop_with_disturbance()),
        [
            ("sp".to_owned(), PlotSignalKind::Setpoint),
            ("pv".to_owned(), PlotSignalKind::Measurement),
            ("mv".to_owned(), PlotSignalKind::AppliedOutput),
            ("feed-flow".to_owned(), PlotSignalKind::Additional),
        ]
    );
    // With an actuator `requestedMv` is a kind of its own, which a list of three fixed identifiers (sp, pv, mv) would call an additional
    // signal in one place and a fixed one in another.
    assert_eq!(
        kinds(&with_actuator(-10.0, 30.0)),
        [
            ("sp".to_owned(), PlotSignalKind::Setpoint),
            ("pv".to_owned(), PlotSignalKind::Measurement),
            ("mv".to_owned(), PlotSignalKind::AppliedOutput),
            ("requestedMv".to_owned(), PlotSignalKind::RequestedOutput),
            ("feed-flow".to_owned(), PlotSignalKind::Additional),
        ]
    );
    // The page reads the kind as a camelCase word.
    let words: Vec<serde_json::Value> = plot_signals(&with_actuator(-10.0, 30.0))
        .iter()
        .map(|signal| serde_json::to_value(signal).expect("the signal encodes")["kind"].clone())
        .collect();
    assert_eq!(
        words,
        [
            "setpoint",
            "measurement",
            "appliedOutput",
            "requestedOutput",
            "additional"
        ]
    );
}

#[test]
fn a_loop_without_a_setpoint_lists_no_setpoint() {
    let mut loop_definition = loop_with_disturbance();
    loop_definition.signals.setpoint = None;
    let ids: Vec<String> = plot_signals(&loop_definition)
        .into_iter()
        .map(|signal| signal.id)
        .collect();
    assert_eq!(ids, ["pv", "mv", "feed-flow"]);
}

#[test]
fn the_plots_and_the_project_tests_signals_selector_list_the_same_signals_in_the_same_order() {
    // A project-scripted loop with an actuator and an additional signal: the selector the project test offers and the list the plots
    // draw from are one list of identifiers, so a selected identifier always names a plotted signal.
    let mut value = serde_json::to_value(with_actuator(0.0, 100.0)).expect("the fixture encodes");
    value["experiment"] = json!({
        "execution": "project-scripted", "kind": "observation", "amplitude": 0.0, "seed": 1
    });
    let loop_definition: LoopDefinition =
        serde_json::from_value(value).expect("the project-scripted fixture decodes");
    let form = project_test_form(&loop_definition).expect("a project-scripted loop has a form");
    let offered: Vec<&str> = form
        .signals
        .iter()
        .map(|signal| signal.id.as_str())
        .collect();
    let plotted = plot_signals(&loop_definition);
    let drawn: Vec<&str> = plotted.iter().map(|signal| signal.id.as_str()).collect();
    assert_eq!(offered, drawn);
    assert_eq!(drawn, ["sp", "pv", "mv", "requestedMv", "feed-flow"]);
}
