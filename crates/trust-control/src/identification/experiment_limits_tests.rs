#![allow(clippy::float_cmp)]

use serde_json::{json, Value};

use super::*;
use crate::loop_definition::RelayDirection;
use crate::setup::loop_bindings::duration_seconds_of_text;
use crate::study::inputs::InputsSection;
use crate::study::StudySection;

/// A single-channel loop: a valve (the manipulated variable, 0 to 100 %) and a level (the process variable, 0 to 100 %), whose output
/// limits are `limits` and whose experiment declares `duration_seconds` when it is given.
fn single_channel_loop(limits: &Value, duration_seconds: Option<f64>) -> LoopDefinition {
    let mut experiment = json!({
        "execution": "control-studio", "kind": "mv_step", "amplitude": 5.0, "seed": 1
    });
    if let Some(seconds) = duration_seconds {
        experiment["durationSeconds"] = json!(seconds);
    }
    serde_json::from_value(json!({
        "schemaVersion": 2, "id": "level", "name": "Level", "revision": "sha256:level",
        "projectId": "project", "runtimeId": "runtime",
        "controller": {
            "instance": "Main.Level", "type": "Control.PID", "revision": "sha256:controller",
            "action": "direct", "taskPeriodMs": 100.0,
            "manualOutputSignal": "Main.ManualValve", "manualOutputType": "REAL"
        },
        "observedSampleTiming": { "nominalPeriodMs": 100.0 },
        "signals": {
            "processVariable": { "symbol": "Main.Level", "iecType": "REAL", "direction": "read", "unit": "%",
                "range": { "minimum": 0.0, "maximum": 100.0 } },
            "manipulatedVariable": { "symbol": "Main.Valve", "iecType": "REAL", "direction": "write", "unit": "%",
                "range": { "minimum": 0.0, "maximum": 100.0 } }
        },
        "limits": limits,
        "experiment": experiment
    }))
    .expect("the fixture loop decodes")
}

/// A two-input loop: the valve (the manipulated variable, with an actuator from 5 to 80 %) and a measured load (kW, 0 to 50), and two
/// measured outputs. The loop's output limits are 10 to 90 %.
fn two_input_loop() -> LoopDefinition {
    serde_json::from_value(json!({
        "schemaVersion": 2, "id": "heater", "name": "Heater", "revision": "sha256:heater",
        "projectId": "project", "runtimeId": "runtime",
        "controller": {
            "instance": "P1.Controller", "type": "Control.PID", "revision": "sha256:controller",
            "action": "direct", "taskPeriodMs": 100.0,
            "manualOutputSignal": "P1.ManualPct", "manualOutputType": "LREAL"
        },
        "observedSampleTiming": { "nominalPeriodMs": 100.0 },
        "signals": {
            "processVariable": { "symbol": "LevelPV", "iecType": "LREAL", "direction": "read", "unit": "%",
                "range": { "minimum": 0.0, "maximum": 100.0 } },
            "manipulatedVariable": { "symbol": "ValveMV", "iecType": "LREAL", "direction": "write", "unit": "%",
                "range": { "minimum": 0.0, "maximum": 100.0 } },
            "additional": [
                { "id": "load", "channel": "load", "role": "disturbance", "symbol": "P1.LoadKw", "iecType": "LREAL",
                  "direction": "read", "unit": "kW", "range": { "minimum": 0.0, "maximum": 50.0 } },
                { "id": "temperature", "channel": "temperature", "role": "PV", "symbol": "P1.TempC", "iecType": "LREAL",
                  "direction": "read", "unit": "degC", "range": { "minimum": -20.0, "maximum": 120.0 } }
            ]
        },
        "limits": { "outputMinimum": 10.0, "outputMaximum": 90.0 },
        "experiment": { "execution": "control-studio", "kind": "mv_step", "amplitude": 5.0, "durationSeconds": 120.0, "seed": 1 },
        "channels": {
            "inputs": [
                { "modelChannelId": "valve", "signalId": "manipulatedVariable", "manualOutputSignal": "P1.ManualPct",
                  "manualOutputType": "LREAL", "actuator": { "kind": "continuous", "minimum": 5.0, "maximum": 80.0 } },
                { "modelChannelId": "load", "signalId": "load", "manualOutputSignal": "P1.LoadKw",
                  "manualOutputType": "LREAL", "actuator": { "kind": "continuous", "minimum": 0.0, "maximum": 50.0 } }
            ],
            "outputs": [
                { "modelChannelId": "level", "signalId": "processVariable" },
                { "modelChannelId": "temperature", "signalId": "temperature" }
            ]
        }
    }))
    .expect("the fixture loop decodes")
}

#[test]
fn a_single_channel_loop_starts_from_its_declared_ranges_narrowed_by_the_output_limits() {
    let limits = experiment_limits(&single_channel_loop(
        &json!({ "outputMinimum": 10.0, "outputMaximum": 90.0 }),
        Some(60.0),
    ));
    assert_eq!(limits.duration_seconds, Some(60.0));
    assert_eq!(
        limits.inputs,
        [ExperimentRange {
            symbol: "Main.Valve".into(),
            unit: "%".into(),
            minimum: 10.0,
            maximum: 90.0
        }]
    );
    assert_eq!(
        limits.outputs,
        [ExperimentRange {
            symbol: "Main.Level".into(),
            unit: "%".into(),
            minimum: 0.0,
            maximum: 100.0
        }]
    );
}

#[test]
fn a_loop_without_output_limits_keeps_the_declared_range_of_its_input() {
    let limits = experiment_limits(&single_channel_loop(&json!({}), None));
    assert_eq!(limits.inputs[0].minimum, 0.0);
    assert_eq!(limits.inputs[0].maximum, 100.0);
    assert_eq!(limits.duration_seconds, None);
}

#[test]
fn an_actuator_range_narrows_its_channel_and_the_output_limits_narrow_the_manipulated_variable() {
    let limits = experiment_limits(&two_input_loop());
    let valve = &limits.inputs[0];
    assert_eq!(
        (valve.symbol.as_str(), valve.minimum, valve.maximum),
        ("ValveMV", 10.0, 80.0)
    );
    let load = &limits.inputs[1];
    assert_eq!(
        (
            load.symbol.as_str(),
            load.unit.as_str(),
            load.minimum,
            load.maximum
        ),
        ("P1.LoadKw", "kW", 0.0, 50.0)
    );
}

#[test]
fn the_outputs_keep_their_declared_ranges_in_channel_order() {
    let limits = experiment_limits(&two_input_loop());
    let symbols: Vec<&str> = limits
        .outputs
        .iter()
        .map(|output| output.symbol.as_str())
        .collect();
    assert_eq!(symbols, ["LevelPV", "P1.TempC"]);
    assert_eq!(
        (limits.outputs[1].minimum, limits.outputs[1].maximum),
        (-20.0, 120.0)
    );
    assert_eq!(limits.duration_seconds, Some(120.0));
}

#[test]
fn the_limits_serialize_in_camel_case_and_omit_what_the_loop_does_not_declare() {
    let value = serde_json::to_value(experiment_limits(&single_channel_loop(&json!({}), None)))
        .expect("the limits serialize");
    assert_eq!(
        value,
        json!({
            "inputs": [{ "symbol": "Main.Valve", "unit": "%", "minimum": 0.0, "maximum": 100.0 }],
            "outputs": [{ "symbol": "Main.Level", "unit": "%", "minimum": 0.0, "maximum": 100.0 }],
            "amplitude": 5.0,
            "reviewDurationSeconds": 0.0
        })
    );
}

#[test]
fn the_review_defaults_are_the_declared_amplitude_relay_direction_and_duration() {
    let mut loop_definition = single_channel_loop(&json!({}), Some(60.0));
    let experiment = loop_definition
        .experiment
        .as_mut()
        .expect("the fixture declares its experiment");
    experiment.relay_direction = Some(RelayDirection::Positive);
    let limits = experiment_limits(&loop_definition);
    assert_eq!(limits.amplitude, 5.0);
    assert_eq!(limits.relay_direction, Some(RelayDirection::Positive));
    assert_eq!(limits.review_duration_seconds, 60.0);
    let undeclared = experiment_limits(&single_channel_loop(&json!({}), None));
    assert_eq!(
        undeclared.review_duration_seconds, 0.0,
        "zero is the engine's value for a loop that declares no duration"
    );
    assert_eq!(undeclared.relay_direction, None);
}

/// The acquisition record a page change or an earlier engine stored, from its JSON.
fn record(value: &Value) -> AcquisitionSettings {
    serde_json::from_value(value.clone()).expect("the record decodes")
}

/// The symbol and the range of each signal in `ranges`, in the order the Limits editor lists them.
fn rows(ranges: &[ExperimentRange]) -> Vec<(&str, f64, f64)> {
    ranges
        .iter()
        .map(|range| (range.symbol.as_str(), range.minimum, range.maximum))
        .collect()
}

/// B3.2, A8.6: the limits the open answer gives hold every plant input and measured output of the loop, each with the range the study
/// records for it and the declared range where the study records none. Catches a page that merges the study over the defaults itself and
/// maps only the first range of each side (the other signals of a multichannel loop then keep their defaults), and an answer that
/// lists only the signals the record names.
#[test]
fn limits_answer_lists_every_plant_signal_range() {
    let loop_definition = two_input_loop();
    // Without a record the answer is the loop's declared limits.
    let declared = held_experiment_limits(&loop_definition, None);
    assert_eq!(declared, experiment_limits(&loop_definition));
    // The record names the valve and the temperature; the load and the level are not named.
    let held = held_experiment_limits(
        &loop_definition,
        Some(&record(&json!({
            "method": "mv_step", "durationSeconds": 90, "amplitude": 5,
            "inputRanges": { "ValveMV": { "minimum": 20, "maximum": 60 } },
            "outputRanges": { "P1.TempC": { "minimum": 10, "maximum": 50 } }
        }))),
    );
    assert_eq!(
        rows(&held.inputs),
        [("ValveMV", 20.0, 60.0), ("P1.LoadKw", 0.0, 50.0)],
        "the valve takes the recorded range and the load keeps its declared one"
    );
    assert_eq!(
        rows(&held.outputs),
        [("LevelPV", 0.0, 100.0), ("P1.TempC", 10.0, 50.0)],
        "the temperature takes the recorded range and the level keeps its declared one"
    );
    assert_eq!(held.duration_seconds, Some(90.0), "the recorded duration");
    assert_eq!(
        (
            held.amplitude,
            held.relay_direction,
            held.review_duration_seconds
        ),
        (5.0, None, 120.0),
        "the amplitude and the review's duration stay the declared ones"
    );
    assert_eq!(
        held.inputs
            .iter()
            .map(|range| range.unit.as_str())
            .collect::<Vec<_>>(),
        ["%", "kW"],
        "each signal keeps its unit"
    );
}

/// B3.2, B9.9: the Limits editor shows the maximum duration as the engine's words and sends back what the engineer typed, so the words
/// must read back as exactly the duration they show. The page once formatted the number itself, which rounds: a duration of 89.999 s was
/// shown as `90 s` and stored as 90. The words below are worked out by hand from the rule of B9.9 (seconds below 2 min, minutes below
/// 2 h, then hours and minutes, a remainder in the next smaller unit): 130.5 s is 2 min and 10.5 s, 9000 s is 2 h and 30 min.
#[test]
fn the_limits_answer_writes_each_duration_in_words_that_read_back_exactly() {
    for (seconds, words) in [
        (0.0005, "0.0005 s"),
        (47.123, "47.123 s"),
        (89.999, "89.999 s"),
        (90.0, "90 s"),
        (120.0, "2 min"),
        (130.5, "2 min 10.5 s"),
        (900.0, "15 min"),
        (4_500.0, "75 min"),
        (5_400.0, "90 min"),
        (7_200.0, "2 h"),
        (9_000.0, "2 h 30 min"),
    ] {
        // The loop's declared duration, and the duration the study records over it, are written the same way.
        let declared = experiment_limits(&single_channel_loop(&json!({}), Some(seconds)));
        assert_eq!(
            declared.duration_words.as_deref(),
            Some(words),
            "{seconds} s declared"
        );
        let held = held_experiment_limits(
            &single_channel_loop(&json!({}), None),
            Some(&record(&json!({
                "method": "mv_step", "durationSeconds": seconds, "amplitude": 5
            }))),
        );
        assert_eq!(
            held.duration_words.as_deref(),
            Some(words),
            "{seconds} s recorded"
        );
        // The page sends the words back as they are shown; the engine reads them as the very number they stand for.
        assert_eq!(
            duration_seconds_of_text(words),
            Some(seconds),
            "{words} reads back as {seconds} s"
        );
        let wire = serde_json::to_value(&held).expect("the limits are written");
        assert_eq!(wire["durationWords"], words);
    }
    // A loop that declares no duration, and a study that records none, have no words: the editor then asks for it.
    let none = experiment_limits(&single_channel_loop(&json!({}), None));
    assert_eq!(
        (none.duration_seconds, none.duration_words.as_deref()),
        (None, None)
    );
    let wire = serde_json::to_value(&none).expect("the limits are written");
    assert!(wire.get("durationWords").is_none(), "{wire}");
}

/// A bound that no finite number reads cannot replace a declared range: the editor shows the declared one.
#[test]
fn a_recorded_bound_that_is_no_finite_number_leaves_the_declared_range() {
    let loop_definition = two_input_loop();
    let mut unreadable = record(&json!({
        "method": "mv_step", "durationSeconds": 90, "amplitude": 5,
        "inputRanges": { "ValveMV": { "minimum": 20, "maximum": 60 } }
    }));
    if let Some(range) = unreadable
        .input_ranges
        .as_mut()
        .and_then(|ranges| ranges.get_mut("ValveMV"))
    {
        range.maximum = f64::NAN;
    }
    let held = held_experiment_limits(&loop_definition, Some(&unreadable));
    assert_eq!(rows(&held.inputs)[0], ("ValveMV", 10.0, 80.0));
}

/// B3.2, A8.6: a study stored before ranges were kept by signal holds the range of the manipulated variable and of the measured
/// output as `inputRange` and `outputRange`, a `reuseRecordings` flag and the method the loop declares. It still reads: the single
/// ranges are the primary signals' (and not the first listed other signal's), and a range named by symbol takes precedence. Catches a
/// record that stops decoding, a single range put on the wrong signal, and a symbol range that loses to the single one.
#[test]
fn an_acquisition_record_of_the_old_form_still_reads() {
    let stored = json!({
        "acquisition": {
            "method": "configured", "durationSeconds": 45, "amplitude": 0, "reuseRecordings": false,
            "inputRange": { "minimum": 30, "maximum": 70 },
            "outputRange": { "minimum": 5, "maximum": 95 }
        }
    });
    let section: InputsSection =
        serde_json::from_value(stored).expect("the stored inputs of the earlier form decode");
    section
        .check()
        .expect("the stored inputs of the earlier form pass their checks");
    let loop_definition = two_input_loop();
    let held = held_experiment_limits(&loop_definition, section.acquisition.as_ref());
    assert_eq!(
        rows(&held.inputs),
        [("ValveMV", 30.0, 70.0), ("P1.LoadKw", 0.0, 50.0)]
    );
    assert_eq!(
        rows(&held.outputs),
        [("LevelPV", 5.0, 95.0), ("P1.TempC", -20.0, 120.0)]
    );
    assert_eq!(held.duration_seconds, Some(45.0));
    // The form that keeps ranges by signal wins over the single range of the same signal.
    let both = record(&json!({
        "method": "automatic", "durationSeconds": 45, "amplitude": 0,
        "inputRange": { "minimum": 30, "maximum": 70 },
        "inputRanges": { "ValveMV": { "minimum": 35, "maximum": 65 } }
    }));
    let held = held_experiment_limits(&loop_definition, Some(&both));
    assert_eq!(rows(&held.inputs)[0], ("ValveMV", 35.0, 65.0));
}
