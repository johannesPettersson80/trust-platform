//! Tests of the multichannel fit (`multichannel_fit`; specification 35, C7 and C8): a two-input recording is fitted into one discrete
//! state-space model on the loop's channels; an independent record is checked; and a record or a loop that does not qualify is
//! refused with its sentence. Each record is generated from a known difference equation that starts at rest.

use serde_json::json;

use super::fit_multichannel_model;
use crate::identification::connected_record::RuntimeVerificationResult;
use crate::identification::independent_validation::{
    RequirementStatus, ValidationPredictionSeries,
};
use crate::identification::record_fixtures::{build, Row, Spec, Verdict};
use crate::loop_definition::LoopDefinition;
use crate::model::{GeneralModel, ModelValidation};

/// A loop with a heater (the manipulated variable), a measured load (an additional disturbance, stepped through its own Manual
/// command), and a water temperature (the process variable). The controller is a PID with its gains.
fn loop_with_channels() -> LoopDefinition {
    let value = json!({
        "schemaVersion": 2, "id": "loop:tank", "name": "Tank", "revision": "sha256:loop-1",
        "projectId": "project:sha256:abc", "runtimeId": "runtime:local",
        "controller": {
            "instance": "Main.TankController", "type": "Control.PID", "revision": "sha256:controller-1",
            "action": "direct", "taskPeriodMs": 10, "modeSignal": "%MX0.0", "trackingSignal": "%ML24",
            "law": { "kind": "pid", "parameterRecordSymbol": "Main.Parameters", "proportionalGainSymbol": "Main.Parameters.Kp",
                "integralGainSymbol": "Main.Parameters.Ki", "derivativeGainSymbol": "Main.Parameters.Kd" }
        },
        "parameterBinding": {
            "loop_id": "loop:tank", "task_name": "MainTask",
            "parameters": [
                { "symbol": "Main.Parameters.Kp", "iec_type": "LREAL", "unit": "%/%", "convention": "proportional gain", "minimum": 0, "maximum": 100 },
                { "symbol": "Main.Parameters.Ki", "iec_type": "LREAL", "unit": "%/(%*s)", "convention": "integral gain", "minimum": 0, "maximum": 100 },
                { "symbol": "Main.Parameters.Kd", "iec_type": "LREAL", "unit": "%*s/%", "convention": "derivative gain", "minimum": 0, "maximum": 100 }
            ],
            "mode": { "symbol": "Main.Auto", "manual_value": false }
        },
        "signals": {
            "setpoint": { "symbol": "LevelSP", "address": "%ML0", "iecType": "LREAL", "direction": "read", "unit": "%", "range": { "minimum": 0, "maximum": 100 } },
            "processVariable": { "symbol": "LevelPV", "address": "%IL0", "iecType": "LREAL", "direction": "read", "unit": "%", "range": { "minimum": 0, "maximum": 100 } },
            "manipulatedVariable": { "symbol": "ValveMV", "address": "%QL0", "iecType": "LREAL", "direction": "write", "unit": "%", "range": { "minimum": 0, "maximum": 100 } },
            "additional": [
                { "id": "load", "channel": "load", "role": "disturbance", "symbol": "Main.Load", "address": "%IL16", "iecType": "LREAL",
                  "direction": "read", "unit": "%", "range": { "minimum": 0, "maximum": 100 } }
            ]
        },
        "limits": { "outputMinimum": 0, "outputMaximum": 100, "rateUpPerS": 20, "rateDownPerS": 25 },
        "observedSampleTiming": { "nominalPeriodMs": 10, "maximumJitterMs": 1 },
        "channels": {
            "inputs": [
                { "modelChannelId": "heater", "signalId": "manipulatedVariable",
                  "actuator": { "kind": "continuous", "minimum": 0, "maximum": 100 } },
                { "modelChannelId": "load", "signalId": "load", "manualOutputSignal": "Main.LoadCommand", "manualOutputType": "LREAL",
                  "actuator": { "kind": "continuous", "minimum": 0, "maximum": 50 }, "experimentStep": 10 }
            ],
            "outputs": [ { "modelChannelId": "water", "signalId": "processVariable" } ]
        },
        "plantId": "tank", "operatingPoint": "Normal production"
    });
    LoopDefinition::decode(&value.to_string()).expect("the loop with its channels decodes")
}

/// A deterministic excitation in `[-1, 1)`, starting at rest: a linear congruential sequence with a zero first sample.
#[allow(clippy::cast_precision_loss)]
fn excitation(seed: u64, count: usize) -> Vec<f64> {
    let mut state = seed;
    (0..count)
        .map(|index| {
            state = state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            if index == 0 {
                return 0.0;
            }
            let uniform = (state >> 11) as f64 / (1_u64 << 53) as f64;
            2.0 * uniform - 1.0
        })
        .collect()
}

/// The rows of one recording at 0.1 s: the heater, the load and the water temperature of `y(k+1) = a·y(k) + g₁·u(k) + g₂·v(k)`,
/// from rest.
#[allow(clippy::needless_range_loop, clippy::cast_precision_loss)]
fn record_rows(seed: u64, a: f64, gains: [f64; 2], count: usize) -> Vec<Row> {
    let heater = excitation(seed, count);
    let load = excitation(seed + 1, count);
    let mut water = vec![0.0; count];
    for k in 0..count - 1 {
        water[k + 1] = a * water[k] + gains[0] * heater[k] + gains[1] * load[k];
    }
    (0..count)
        .map(|k| {
            (
                k as f64 * 0.1,
                u64::try_from(k).unwrap_or_default(),
                vec![Some(heater[k]), Some(load[k]), Some(water[k])],
            )
        })
        .collect()
}

/// A recording of the loop's channels, under the symbols and units the loop declares.
fn record(id: &str, rows: &[Row]) -> RuntimeVerificationResult<Verdict> {
    build(&Spec {
        id,
        project_revision: "sha256:project",
        model_revision: "connected-plant:tank",
        kind: "setpoint_step",
        channels: &[("ValveMV", "%"), ("Main.Load", "%"), ("LevelPV", "%")],
        rows,
        baseline: None,
        verdict: RequirementStatus::Pass,
    })
}

#[test]
fn a_two_input_recording_is_fitted_into_one_state_space_model_on_the_loop_channels() {
    let estimation = record("estimation", &record_rows(7, 0.8, [0.1, 0.2], 200));
    let identified = fit_multichannel_model(&loop_with_channels(), &[estimation])
        .expect("an exact first-order record is identified");
    let GeneralModel::DiscreteStateSpace {
        identity,
        states,
        system,
        sample_time_seconds,
    } = &identified.model
    else {
        panic!(
            "expected a discrete state-space model, got {:?}",
            identified.model
        );
    };
    let inputs: Vec<&str> = identity
        .inputs
        .iter()
        .map(|channel| channel.id.as_str())
        .collect();
    assert_eq!(inputs, vec!["heater", "load"]);
    assert_eq!(identity.outputs[0].id, "water");
    assert_eq!(identity.validation, ModelValidation::Unvalidated);
    assert_eq!(identity.evidence.len(), 1);
    // The first order is the adequate one: one state, the output's coefficient and the two input coefficients.
    assert_eq!(states.len(), 1);
    assert!((system.a[0][0] - 0.8).abs() < 1e-6);
    assert!((system.b[0][0] - 0.1).abs() < 1e-6);
    assert!((system.b[0][1] - 0.2).abs() < 1e-6);
    assert!((sample_time_seconds - 0.1).abs() < 1e-9);
    // The estimation record is the comparison record: its free run is on the record and matches it.
    assert_eq!(
        identified.fit.validation_predictions,
        [] as [ValidationPredictionSeries; 0]
    );
    let series = &identified.fit.estimation_predictions;
    assert_eq!(series.len(), 1);
    assert_eq!(series[0].points.len(), 199);
    assert!(series[0]
        .points
        .iter()
        .all(|point| point.residual.abs() < 1e-6));
    assert_eq!(
        identified.fit.comparisons[0].reason,
        "Estimation fit meets the error limit; independent validation is pending."
    );
}

#[test]
fn an_independent_record_that_the_model_predicts_validates_it() {
    let estimation = record("estimation", &record_rows(7, 0.8, [0.1, 0.2], 200));
    let validation = record("validation", &record_rows(11, 0.8, [0.1, 0.2], 200));
    let identified = fit_multichannel_model(&loop_with_channels(), &[estimation, validation])
        .expect("an independent record of the same plant identifies and validates the model");
    assert_eq!(
        identified.model.identity().validation,
        ModelValidation::Validated
    );
    assert_eq!(identified.model.identity().evidence.len(), 2);
    assert_eq!(
        identified.fit.estimation_predictions,
        [] as [ValidationPredictionSeries; 0]
    );
    assert_eq!(identified.fit.validation_predictions.len(), 1);
    assert_eq!(identified.runs.len(), 2);
    assert_eq!(
        identified.fit.comparisons[0].reason,
        "Every held-out output meets the declared error limit."
    );
}

#[test]
fn an_independent_record_of_another_plant_is_refused_above_the_error_limit() {
    let estimation = record("estimation", &record_rows(7, 0.8, [0.1, 0.2], 200));
    let validation = record("validation", &record_rows(11, -0.5, [0.9, 0.1], 200));
    let refusal = fit_multichannel_model(&loop_with_channels(), &[estimation, validation])
        .expect_err("a model above the error limit on the independent record is refused");
    assert!(
        refusal.0.starts_with(
            "No identifiable adequate state model. Order 1: Independent free-running predictions exceed the declared error limit."
        ),
        "{}",
        refusal.0
    );
}

#[test]
fn the_records_the_loop_and_the_sample_count_are_refused_when_they_do_not_qualify() {
    let rows = record_rows(7, 0.8, [0.1, 0.2], 200);
    let twin = fit_multichannel_model(
        &loop_with_channels(),
        &[record("estimation", &rows), record("estimation", &rows)],
    )
    .err()
    .map(|refusal| refusal.0)
    .unwrap_or_default();
    assert_eq!(
        twin,
        "Independent records must have distinct identities and the same plant/program revision."
    );
    let mut bare = loop_with_channels();
    bare.channels = None;
    let no_channels = fit_multichannel_model(&bare, &[record("estimation", &rows)])
        .err()
        .map(|refusal| refusal.0)
        .unwrap_or_default();
    assert_eq!(no_channels, "Ordered model channels are required.");
    let short = record("short", &record_rows(7, 0.8, [0.1, 0.2], 20));
    let coverage = fit_multichannel_model(&loop_with_channels(), &[short])
        .err()
        .map(|refusal| refusal.0)
        .unwrap_or_default();
    assert_eq!(
        coverage,
        "Identification requires complete sample coverage."
    );
}
