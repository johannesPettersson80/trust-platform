//! Tests of the PID record that the generator binds (`pidLoopGenerator.ts`, `pidBindings.ts`; specification 35, C0, C1 Setup): the bounds that
//! admit the values the plant library's PID pair starts with, the parameters with the `Control.PID` bounds, the seven retained states at
//! the operating point, and the snapshot age. They were part of `pid_loop_tests.rs`, which keeps the generator's other assertions.

#![allow(clippy::float_cmp)]

use std::collections::BTreeMap;

use super::snapshot_age_ms;
use super::test_support::{field, STATE_FIELDS};
use crate::loop_definition::{InitialSource, InitialValue, PostScan, RealType, StateType};
use crate::setup::loop_bindings::MeasuredSignal;
use crate::setup::pid_bindings::{pid_parameters, pid_states};

/// A measured signal that a function takes checked, not as a form draft (`MeasuredSignal`).
fn measured(symbol: &str, unit: &str, minimum: f64, maximum: f64) -> MeasuredSignal {
    MeasuredSignal {
        symbol: symbol.to_string(),
        unit: unit.to_string(),
        minimum,
        maximum,
    }
}

#[test]
fn the_bounds_admit_every_value_the_plant_library_pid_pair_starts_with() {
    // control-studio-loop-setup.test.ts: "the bounds setup declares for a Control.PID record admit every value the Plant library's PID
    // pair starts it with". The pair is `libraries/plant/hmi/pairs/plant-pid.toml`, read at compile time.
    let pair = include_str!("../../../../libraries/plant/hmi/pairs/plant-pid.toml");
    let initial: BTreeMap<String, f64> = pair
        .lines()
        .filter_map(|line| {
            let rest = line.strip_prefix("\"{instance}_Parameters.")?;
            let (name, value) = rest.split_once("\" = ")?;
            let number = value.parse::<f64>().ok()?;
            (!name.is_empty() && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_'))
                .then(|| (name.to_string(), number))
        })
        .collect();
    assert!(
        initial.contains_key("RateMinimum") && initial.contains_key("RateMaximum"),
        "the pair sets the rate limits: {initial:?}"
    );
    let parameters = pid_parameters(
        "P1.Parameters",
        &measured("P1.PV", "°C", 0.0, 100.0),
        &measured("P1.MV", "%", 0.0, 100.0),
    );
    for parameter in parameters {
        let name = field(&parameter.symbol);
        if let Some(value) = initial.get(&name) {
            assert!(
                *value >= parameter.minimum && *value <= parameter.maximum,
                "{name} starts at {value}, inside the declared {}..{}, so Validate can carry it",
                parameter.minimum,
                parameter.maximum
            );
        }
    }
}

#[test]
fn the_pid_parameters_follow_the_control_pid_bounds() {
    // pidBindings.ts `pidParameters`: closed-form bounds for a 0–80 °C measurement and a 0–100 % output (C0).
    let parameters = pid_parameters(
        "P1.Parameters",
        &measured("P1.TemperatureC", "degC", 0.0, 80.0),
        &measured("P1.AppliedPowerPct", "%", 0.0, 100.0),
    );
    let expected = [
        ("ProportionalGain", "%/degC", -1e6, 1e6),
        ("IntegralGain", "%/(degC*s)", -1e6, 1e6),
        ("DerivativeGain", "%*s/degC", -1e6, 1e6),
        ("ProportionalSetpointWeight", "1", 0.0, 1.0),
        ("DerivativeSetpointWeight", "1", 0.0, 1.0),
        ("Bias", "%", -100.0, 100.0),
        ("OutputMinimum", "%", 0.0, 100.0),
        ("OutputMaximum", "%", 0.0, 100.0),
        ("RateMinimum", "%/s", -1e9, 0.0),
        ("RateMaximum", "%/s", 0.0, 1e9),
        ("TrackingTime", "s", 0.001, 1e4),
    ];
    assert_eq!(parameters.len(), expected.len());
    for (parameter, (name, unit, minimum, maximum)) in parameters.iter().zip(expected) {
        assert_eq!(parameter.symbol, format!("P1.Parameters.{name}"));
        assert_eq!(parameter.iec_type, RealType::Lreal);
        assert_eq!(parameter.unit, unit, "{name}");
        assert_eq!(parameter.convention, format!("Control.PID {name}"));
        assert_eq!(
            (parameter.minimum, parameter.maximum),
            (minimum, maximum),
            "{name}"
        );
    }
}

#[test]
fn the_pid_states_are_the_seven_retained_states_at_the_operating_point() {
    // pidBindings.ts `pidStates`: the setpoint and the measurement start at the model output, the two outputs at the model input, the
    // rates and the feedforward at zero, and the rates and the nominal output span the signal spans (C1).
    let states = pid_states(
        "P1.Controller",
        &measured("P1.TemperatureC", "degC", 0.0, 80.0),
        &measured("P1.AppliedPowerPct", "%", 0.0, 100.0),
        0,
    );
    let output = |index| InitialValue::From(InitialSource::ModelOutput { index });
    let input = |index| InitialValue::From(InitialSource::ModelInput { index });
    let expected = [
        ("Setpoint", "degC", 0.0, 80.0, output(0)),
        (
            "SetpointRate",
            "degC/s",
            -80_000.0,
            80_000.0,
            InitialValue::Constant(0.0),
        ),
        ("ProcessValue", "degC", 0.0, 80.0, output(0)),
        (
            "ProcessValueRate",
            "degC/s",
            -80_000.0,
            80_000.0,
            InitialValue::Constant(0.0),
        ),
        (
            "FeedForward",
            "%",
            -100.0,
            100.0,
            InitialValue::Constant(0.0),
        ),
        ("NominalOutput", "%", -1000.0, 1100.0, input(0)),
        ("SaturatedOutput", "%", 0.0, 100.0, input(0)),
    ];
    assert_eq!(states.len(), 7);
    for (state, (name, unit, minimum, maximum, initial)) in states.iter().zip(expected) {
        assert_eq!(state.symbol, format!("P1.Controller.StateValue.{name}"));
        assert_eq!(state.iec_type, StateType::Lreal);
        assert_eq!(state.unit, unit, "{name}");
        assert_eq!((state.minimum, state.maximum), (minimum, maximum), "{name}");
        assert_eq!(state.initial, initial, "{name}");
        assert_eq!(state.post_scan, PostScan::Bounded, "{name}");
    }
    assert_eq!(
        states
            .iter()
            .map(|state| field(&state.symbol))
            .collect::<Vec<_>>(),
        STATE_FIELDS
            .iter()
            .map(|name| (*name).to_string())
            .collect::<Vec<_>>()
    );
}

#[test]
fn the_snapshot_age_is_five_periods_at_least_half_a_second_in_whole_milliseconds() {
    // pidLoopGenerator.ts: Math.max(500, 5 * period), rounded up to whole ms (validation takes an integer from 1 period to 10 s).
    assert_eq!(snapshot_age_ms(100.0), 500);
    assert_eq!(snapshot_age_ms(1_000.0), 5_000);
    assert_eq!(snapshot_age_ms(100.5), 503, "502.5 ms rounded up");
    assert_eq!(
        snapshot_age_ms(2_000.0),
        10_000,
        "the longest age validation accepts"
    );
    assert_eq!(snapshot_age_ms(2_001.0), 10_005);
}
