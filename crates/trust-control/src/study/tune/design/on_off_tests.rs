#![allow(clippy::float_cmp)]
// The expected settings are exact: the rounded minimum times are whole multiples of the task period.

//! Tests of the switching controller's design (C3, D-8, D-9): the numbers of `control-studio-loop-setup-onoff.test.ts` and
//! `control-studio-controller-mappings.test.ts` (the half band from the settling band, the equipment minimums as the floor of the minimum
//! times), and the refusals the design owns (an undeclared equipment limit, a band with no design inside the error limit).

use std::collections::BTreeMap;

use serde_json::{json, Value};

use super::design;
use crate::commissioning::fixture_loops::requirements as task_requirements;
use crate::loop_definition::LoopDefinition;
use crate::tuning::requirements::TuningRequirements;

/// The cold room of the TypeScript tests: a program that switches a compressor between 0 and 100 %, with a 100 ms task. Its channel's
/// actuator is `actuator`, and its hysteresis band convention is `convention`.
pub(crate) fn cold_room(actuator: &Value, convention: &str) -> LoopDefinition {
    let value = json!({
        "schemaVersion": 2, "id": "loop:cold-room", "name": "Cold room", "revision": "sha256:cold-1",
        "projectId": "project:setup", "runtimeId": "runtime-1",
        "controller": {
            "instance": "P1", "type": "Main", "revision": "sha256:controller-1", "action": "reverse",
            "taskPeriodMs": 100, "design": "project", "modeSignal": "P1.AutoMode", "modeWhenTrue": "Auto",
            "manualOutputSignal": "P1.ManualPct", "manualOutputType": "LREAL",
            "onOff": {
                "hysteresisSymbol": "P1.HysteresisC", "minimumOnTimeSymbol": "P1.MinimumOnS",
                "minimumOffTimeSymbol": "P1.MinimumOffS", "bandConvention": convention,
                "lowOutput": 0.0, "highOutput": 100.0,
                "initialization": { "kind": "declared-state-v1", "states": [
                    { "symbol": "P1.CoolingOn", "iec_type": "BOOL", "unit": "1", "minimum": 0.0, "maximum": 1.0,
                      "initial": { "source": "model-input-is", "index": 0, "value": 100.0 }, "postScan": "bounded" },
                    { "symbol": "P1.DwellS", "iec_type": "LREAL", "unit": "s", "minimum": 0.0, "maximum": 1_000_000.0,
                      "initial": 0.0, "postScan": "bounded" }
                ] }
            }
        },
        "parameterBinding": {
            "loop_id": "loop:cold-room", "task_name": "MainTask",
            "parameters": [
                { "symbol": "P1.HysteresisC", "iec_type": "LREAL", "unit": "degC", "convention": "ON/OFF hysteresis", "minimum": 0.0, "maximum": 30.0 },
                { "symbol": "P1.MinimumOnS", "iec_type": "LREAL", "unit": "s", "convention": "ON/OFF minimum ON time", "minimum": 0.0, "maximum": 3600.0 },
                { "symbol": "P1.MinimumOffS", "iec_type": "LREAL", "unit": "s", "convention": "ON/OFF minimum OFF time", "minimum": 0.0, "maximum": 3600.0 }
            ],
            "mode": { "symbol": "P1.AutoMode", "manual_value": false }
        },
        "signals": {
            "setpoint": { "symbol": "P1.SetpointC", "iecType": "LREAL", "direction": "read", "unit": "degC", "range": { "minimum": -10, "maximum": 20 } },
            "processVariable": { "symbol": "P1.RoomC", "address": "%IL0", "iecType": "LREAL", "direction": "read", "unit": "degC", "range": { "minimum": -10, "maximum": 20 } },
            "manipulatedVariable": { "symbol": "P1.CompressorPct", "address": "%QL0", "iecType": "LREAL", "direction": "write", "unit": "%", "range": { "minimum": 0, "maximum": 100 } }
        },
        "limits": { "outputMinimum": 0, "outputMaximum": 100 },
        "observedSampleTiming": { "nominalPeriodMs": 100 },
        "channels": {
            "inputs": [{ "modelChannelId": "P1.CompressorPct", "signalId": "manipulatedVariable", "actuator": actuator }],
            "outputs": [{ "modelChannelId": "P1.RoomC", "signalId": "processVariable", "setpointSignalId": "setpoint" }]
        }
    });
    LoopDefinition::decode(&value.to_string()).expect("the cold room loop decodes")
}

/// The on-off actuator of the cold room: 0 and 100 %, with the equipment's minimum times when `minimums` gives them.
pub(crate) fn actuator(minimums: Option<(f64, f64)>) -> Value {
    let mut actuator = json!({ "kind": "on-off", "minimum": 0.0, "maximum": 100.0 });
    if let Some((on, off)) = minimums {
        actuator["minimumOnSeconds"] = json!(on);
        actuator["minimumOffSeconds"] = json!(off);
    }
    actuator
}

/// The process requirements of the test: a settling band of 0.5 degC inside an error limit of 1.5 degC (the TypeScript's numbers).
fn requirements(settling_band: f64, maximum_error: f64) -> TuningRequirements {
    let mut requirements: TuningRequirements = task_requirements();
    requirements.settling_band = settling_band;
    requirements.maximum_error = maximum_error;
    requirements
}

#[test]
fn the_half_band_comes_from_the_settling_band_and_the_minimum_times_from_the_equipment() {
    let loop_definition = cold_room(&actuator(Some((20.0, 30.0))), "half-band");
    let candidate =
        design(&loop_definition, &requirements(0.5, 1.5), None).expect("the design holds");
    assert!(!candidate.applied, "Tune never applies a candidate");
    assert_eq!(
        candidate.values.get("P1.HysteresisC"),
        Some(&0.5),
        "Tune sets the half band from the settling band"
    );
    assert_eq!(
        candidate.values.get("P1.MinimumOnS"),
        Some(&20.0),
        "a compressor's restart protection is never shortened"
    );
    assert_eq!(candidate.values.get("P1.MinimumOffS"), Some(&30.0));
}

#[test]
fn a_full_band_declares_twice_the_half_width() {
    let loop_definition = cold_room(&actuator(Some((20.0, 20.0))), "full-band");
    let candidate =
        design(&loop_definition, &requirements(0.5, 1.5), None).expect("the design holds");
    assert_eq!(
        candidate.values.get("P1.HysteresisC"),
        Some(&1.0),
        "the declared width is the separation of the thresholds"
    );
}

#[test]
fn a_zero_equipment_minimum_is_a_declaration_and_still_takes_the_task_period() {
    let loop_definition = cold_room(&actuator(Some((0.0, 0.0))), "half-band");
    let candidate =
        design(&loop_definition, &requirements(0.5, 1.5), None).expect("zero is declared");
    assert_eq!(
        candidate.values.get("P1.MinimumOnS"),
        Some(&0.1),
        "a dwell is at least one task period (100 ms)"
    );
    assert_eq!(candidate.values.get("P1.MinimumOffS"), Some(&0.1));
}

#[test]
fn an_undeclared_equipment_minimum_is_refused_and_never_taken_as_zero() {
    let loop_definition = cold_room(&actuator(None), "half-band");
    let failure = design(&loop_definition, &requirements(0.5, 1.5), None)
        .expect_err("the limits are undeclared");
    assert!(
        failure
            .message
            .contains("Declare the switched equipment's minimum ON and OFF times"),
        "{}",
        failure.message
    );
}

#[test]
fn the_pinned_settings_replace_the_designed_ones_after_the_design() {
    // The TypeScript's pinned-settings test (`control-studio-controller-mappings.test.ts`): the pins hold exactly. The TypeScript pins
    // 0.04 s and 0.06 s on its 20 ms period; this loop's period is 100 ms, so its pinned dwells are whole periods (0.5 s and 1.0 s).
    let loop_definition = cold_room(&actuator(Some((0.0, 0.0))), "half-band");
    let pins = BTreeMap::from([
        ("P1.HysteresisC".to_owned(), 0.8),
        ("P1.MinimumOnS".to_owned(), 0.5),
        ("P1.MinimumOffS".to_owned(), 1.0),
    ]);
    let candidate = design(&loop_definition, &requirements(0.2, 1.0), Some(&pins))
        .expect("the pinned design holds");
    assert_eq!(
        candidate.values, pins,
        "the pins are the settings Tune writes"
    );
}

#[test]
fn the_designed_hysteresis_is_the_settling_band_and_the_dwells_are_whole_task_periods() {
    // The TypeScript's design test (`control-studio-controller-mappings.test.ts`), with this loop's 100 ms task period.
    let loop_definition = cold_room(&actuator(Some((0.0, 0.0))), "half-band");
    let candidate =
        design(&loop_definition, &requirements(0.2, 1.0), None).expect("the design holds");
    assert!(
        candidate.values["P1.HysteresisC"] >= 0.2,
        "the band is at least the settling band"
    );
    for symbol in ["P1.MinimumOnS", "P1.MinimumOffS"] {
        let periods = candidate.values[symbol] / 0.1;
        assert!(
            (periods - periods.round()).abs() < 1e-9,
            "{symbol} is a whole number of task periods"
        );
    }
}

#[test]
fn a_pin_outside_its_binding_or_of_another_symbol_is_refused_and_names_its_symbol() {
    let loop_definition = cold_room(&actuator(Some((0.0, 0.0))), "half-band");
    // The hysteresis is bound to 0 to 30 degC: 31 degC is refused, with the symbol as its field.
    let outside = BTreeMap::from([("P1.HysteresisC".to_owned(), 31.0)]);
    let failure = design(&loop_definition, &requirements(0.2, 1.0), Some(&outside))
        .expect_err("31 degC is outside its binding");
    assert_eq!(failure.field.as_deref(), Some("P1.HysteresisC"));
    // A setpoint is not one of the switching settings.
    let other = BTreeMap::from([("P1.SetpointC".to_owned(), 5.0)]);
    let failure = design(&loop_definition, &requirements(0.2, 1.0), Some(&other))
        .expect_err("the setpoint is not a switching setting");
    assert_eq!(failure.field.as_deref(), Some("P1.SetpointC"));
}

#[test]
fn a_pinned_minimum_time_below_the_equipment_minimum_is_refused_and_names_its_symbol() {
    // C3: a dwell is never shorter than the switched equipment's own limit, so a pin below it is refused, whichever time it is.
    let loop_definition = cold_room(&actuator(Some((20.0, 30.0))), "half-band");
    let short_on = BTreeMap::from([("P1.MinimumOnS".to_owned(), 15.0)]);
    let failure = design(&loop_definition, &requirements(0.2, 1.0), Some(&short_on))
        .expect_err("15 s is below the equipment's 20 s");
    assert_eq!(failure.field.as_deref(), Some("P1.MinimumOnS"));
    assert!(
        failure.message.contains("minimum ON time"),
        "{}",
        failure.message
    );
    let short_off = BTreeMap::from([("P1.MinimumOffS".to_owned(), 10.0)]);
    let failure = design(&loop_definition, &requirements(0.2, 1.0), Some(&short_off))
        .expect_err("10 s is below the equipment's 30 s");
    assert_eq!(failure.field.as_deref(), Some("P1.MinimumOffS"));
    assert!(
        failure.message.contains("minimum OFF time"),
        "{}",
        failure.message
    );
}

#[test]
fn a_valid_pinned_dwell_at_or_above_the_equipment_minimum_is_kept_in_whole_task_periods() {
    // 40 s and 30 s are above the equipment's 20 s and at its 30 s, and each is a whole number of the 100 ms task periods.
    let loop_definition = cold_room(&actuator(Some((20.0, 30.0))), "half-band");
    let pins = BTreeMap::from([
        ("P1.MinimumOnS".to_owned(), 40.0),
        ("P1.MinimumOffS".to_owned(), 30.0),
    ]);
    let candidate = design(&loop_definition, &requirements(0.2, 1.0), Some(&pins))
        .expect("the pinned dwells are valid");
    assert_eq!(candidate.values["P1.MinimumOnS"], 40.0);
    assert_eq!(candidate.values["P1.MinimumOffS"], 30.0);
}

#[test]
fn a_pinned_dwell_that_is_not_whole_task_periods_is_rounded_up_as_a_designed_one_is() {
    // With no equipment limit, 0.25 s rounds up to three periods of 100 ms, and 0.04 s to one period.
    let loop_definition = cold_room(&actuator(Some((0.0, 0.0))), "half-band");
    let pins = BTreeMap::from([
        ("P1.MinimumOnS".to_owned(), 0.25),
        ("P1.MinimumOffS".to_owned(), 0.04),
    ]);
    let candidate = design(&loop_definition, &requirements(0.2, 1.0), Some(&pins))
        .expect("the pinned dwells are valid");
    assert!(
        (candidate.values["P1.MinimumOnS"] - 0.3).abs() < 1e-9,
        "0.25 s rounds up to 0.3 s"
    );
    assert!(
        (candidate.values["P1.MinimumOffS"] - 0.1).abs() < 1e-9,
        "0.04 s rounds up to 0.1 s"
    );
}

#[test]
fn undeclared_equipment_refuses_tune_with_or_without_pins() {
    // D-9: an undeclared equipment minimum is never taken as zero, so a pin does not make the design possible.
    let loop_definition = cold_room(&actuator(None), "half-band");
    let pins = BTreeMap::from([
        ("P1.MinimumOnS".to_owned(), 20.0),
        ("P1.MinimumOffS".to_owned(), 30.0),
    ]);
    let without = design(&loop_definition, &requirements(0.2, 1.0), None)
        .expect_err("the equipment minimums are undeclared");
    assert!(
        without
            .message
            .contains("Declare the switched equipment's minimum ON and OFF times"),
        "{}",
        without.message
    );
    let with = design(&loop_definition, &requirements(0.2, 1.0), Some(&pins))
        .expect_err("the equipment minimums are undeclared, with pins");
    assert!(
        with.message
            .contains("Declare the switched equipment's minimum ON and OFF times"),
        "{}",
        with.message
    );
}

#[test]
fn a_band_that_leaves_no_design_inside_the_error_limit_is_refused() {
    let loop_definition = cold_room(&actuator(Some((20.0, 20.0))), "half-band");
    let failure = design(&loop_definition, &requirements(1.5, 1.5), None)
        .expect_err("the band fills the error limit");
    assert!(
        failure.message.contains("leaves no switching design"),
        "{}",
        failure.message
    );
}
