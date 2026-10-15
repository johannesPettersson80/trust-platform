#![allow(clippy::float_cmp)] // The fixtures hold exact literals: the levels, the bounds and the initial values.

//! Tests of the switching controller's block (C3): the decoder's rules (`decodeOnOffController`), the binding check, the file form's
//! round trip, and the channel shape that the gate serves. The fixture is the cold room of `control-studio-loop-setup-onoff.test.ts`.

use serde_json::{json, Value};

use super::{on_off_channels_raw, BandConvention, OnOffController, ThresholdEquality};
use crate::loop_definition::{
    InitialSource, InitialValue, ModeBinding, Parameter, ParameterBinding, RealType,
};

/// The cold room's block as the TypeScript generator writes it, with `extra` fields laid over it.
fn block_json(extra: &Value) -> Value {
    let mut block = json!({
        "hysteresisSymbol": "P1.HysteresisC",
        "minimumOnTimeSymbol": "P1.MinimumOnS",
        "minimumOffTimeSymbol": "P1.MinimumOffS",
        "bandConvention": "half-band",
        "lowOutput": 0.0,
        "highOutput": 100.0,
        "initialization": {
            "kind": "declared-state-v1",
            "states": [
                {
                    "symbol": "P1.CoolingOn", "iec_type": "BOOL", "unit": "1", "minimum": 0.0, "maximum": 1.0,
                    "initial": { "source": "model-input-is", "index": 0, "value": 100.0 }, "postScan": "bounded"
                },
                {
                    "symbol": "P1.DwellS", "iec_type": "LREAL", "unit": "s", "minimum": 0.0, "maximum": 1_000_000.0,
                    "initial": 0.0, "postScan": "bounded"
                }
            ]
        }
    });
    if let (Some(object), Some(fields)) = (block.as_object_mut(), extra.as_object()) {
        for (key, value) in fields {
            object.insert(key.clone(), value.clone());
        }
    }
    block
}

/// The cold room's parameter binding: the hysteresis in the measurement's unit, and the two minimum times in seconds.
fn binding() -> ParameterBinding {
    let parameter = |symbol: &str, unit: &str| Parameter {
        symbol: symbol.to_owned(),
        iec_type: RealType::Lreal,
        unit: unit.to_owned(),
        convention: "ON/OFF".to_owned(),
        minimum: 0.0,
        maximum: 3600.0,
    };
    ParameterBinding {
        loop_id: "loop:cold-room".to_owned(),
        task_name: "MainTask".to_owned(),
        parameters: vec![
            parameter("P1.HysteresisC", "degC"),
            parameter("P1.MinimumOnS", "s"),
            parameter("P1.MinimumOffS", "s"),
        ],
        mode: ModeBinding {
            symbol: "P1.AutoMode".to_owned(),
            manual_value: false,
        },
    }
}

#[test]
fn the_written_block_decodes_with_its_levels_and_declared_states() {
    let block = OnOffController::from_value(&block_json(&json!({}))).expect("the block decodes");
    assert_eq!(
        (block.low_output, block.high_output),
        (0.0, 100.0),
        "the output levels are the limits"
    );
    assert_eq!(block.band_convention, BandConvention::HalfBand);
    assert_eq!(block.noise_band, None);
    assert_eq!(block.threshold_equality, ThresholdEquality::AtOrBeyond);
    let states = block.initialization.states();
    assert_eq!(states.len(), 2);
    assert_eq!(states[0].symbol, "P1.CoolingOn");
    assert_eq!(
        states[0].initial,
        InitialValue::From(InitialSource::ModelInputIs {
            index: 0,
            value: 100.0
        }),
        "the latch starts ON exactly when the output starts high"
    );
    assert_eq!(
        states[1].initial,
        InitialValue::Constant(0.0),
        "the dwell timer starts at zero"
    );
}

#[test]
fn the_levels_must_be_in_order() {
    let error = OnOffController::from_value(&block_json(&json!({ "lowOutput": 100.0 })))
        .expect_err("low is not below high");
    assert!(
        error.contains("lowOutput"),
        "the refusal names the field: {error}"
    );
}

#[test]
fn an_unknown_band_convention_and_a_negative_noise_band_are_refused() {
    let convention =
        OnOffController::from_value(&block_json(&json!({ "bandConvention": "third-band" })))
            .expect_err("the convention is unknown");
    assert!(convention.contains("bandConvention"), "{convention}");
    let noise = OnOffController::from_value(&block_json(&json!({ "noiseBand": -0.1 })))
        .expect_err("the noise band is negative");
    assert!(noise.contains("noiseBand"), "{noise}");
}

#[test]
fn the_three_symbols_must_be_distinct() {
    let error = OnOffController::from_value(&block_json(
        &json!({ "minimumOffTimeSymbol": "P1.MinimumOnS" }),
    ))
    .expect_err("two symbols are the same");
    assert!(error.contains("distinct"), "{error}");
}

#[test]
fn the_threshold_policy_defaults_to_at_or_beyond_and_round_trips_when_declared() {
    let defaulted = OnOffController::from_value(&block_json(&json!({}))).expect("decodes");
    assert!(
        defaulted.to_value().get("thresholdEquality").is_none(),
        "a file that declares no policy keeps its shape"
    );
    let strict =
        OnOffController::from_value(&block_json(&json!({ "thresholdEquality": "beyond" })))
            .expect("the strict policy decodes");
    assert_eq!(strict.threshold_equality, ThresholdEquality::Beyond);
    assert_eq!(strict.to_value()["thresholdEquality"], json!("beyond"));
    assert_eq!(
        OnOffController::from_value(&strict.to_value()).expect("the written form decodes"),
        strict,
        "the file form reads back to the same block"
    );
}

#[test]
fn the_binding_must_hold_the_three_symbols_with_the_minimum_times_in_seconds() {
    let block = OnOffController::from_value(&block_json(&json!({}))).expect("decodes");
    block
        .check_binding(&binding())
        .expect("the cold room's binding fits");
    let mut missing = binding();
    missing
        .parameters
        .retain(|parameter| parameter.symbol != "P1.MinimumOffS");
    assert!(
        block.check_binding(&missing).is_err(),
        "an unbound minimum OFF time is refused"
    );
    let mut degrees = binding();
    degrees.parameters[2].unit = "degC".to_owned();
    let error = block
        .check_binding(&degrees)
        .expect_err("the OFF time is not in seconds");
    assert!(
        error.contains("P1.MinimumOffS") && error.contains("seconds"),
        "{error}"
    );
}

#[test]
fn a_raw_file_has_the_switching_channel_shape_only_with_its_block_and_one_on_off_input() {
    let channels = json!({
        "inputs": [{
            "modelChannelId": "P1.CompressorPct", "signalId": "manipulatedVariable",
            "actuator": { "kind": "on-off", "minimum": 0.0, "maximum": 100.0, "minimumOnSeconds": 20.0, "minimumOffSeconds": 30.0 }
        }],
        "outputs": [{ "modelChannelId": "P1.RoomC", "signalId": "processVariable", "setpointSignalId": "setpoint" }]
    });
    let raw =
        json!({ "controller": { "onOff": block_json(&json!({})) }, "channels": channels.clone() });
    assert!(
        on_off_channels_raw(&raw),
        "one ON/OFF input and one output, with the block"
    );
    let without_block = json!({ "controller": {}, "channels": channels });
    assert!(
        !on_off_channels_raw(&without_block),
        "the channels need the switching block"
    );
    let mut continuous = raw.clone();
    continuous["channels"]["inputs"][0]["actuator"]["kind"] = json!("continuous");
    assert!(
        !on_off_channels_raw(&continuous),
        "a continuous actuator is not a switching output"
    );
}
