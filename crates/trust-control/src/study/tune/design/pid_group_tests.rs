//! The Tune of a PID group (specification 35, C7) on the zones fixtures of `tuning::structures`: a decoupled group designs each member
//! through `D = K⁻¹·diag(K)` and writes every member's gains and every decoupler entry; a paired group designs each member on its own
//! path; and the refusals name their rule. Ported from `control-studio-decoupler.test.ts` and `control-studio-decentralized-pid.test.ts`.

#![allow(clippy::float_cmp)] // The default member and the entries of the group's values are the numbers the design writes.

use serde_json::{json, Value};

use super::run;
use crate::loop_definition::LoopDefinition;
use crate::messages::{CandidateOutcome, TuneRequest};
use crate::study::current_model::{CurrentModel, CurrentModelSource};
use crate::study::tune::TuneInputs;
use crate::tuning::requirements::TuningRequirements;
use crate::tuning::structures::fixtures::{decoupler_zones_model, zones_model};

/// The loop of a two-member group: each member's gains are its own record's three symbols, and the program's seed is `seed`.
fn group_loop(family: &str, seed: &Value, members: &[(&str, &str, &str, &str)]) -> LoopDefinition {
    let members: Vec<Value> = members
        .iter()
        .map(|(instance, input, output, record)| {
            json!({
                "instance": instance, "type": "Control.PID", "inputChannelId": input, "outputChannelId": output,
                "modeSymbol": "P1.ZonesAuto",
                "parameterSymbols": [format!("{record}.ProportionalGain"), format!("{record}.IntegralGain"), format!("{record}.DerivativeGain")],
            })
        })
        .collect();
    serde_json::from_value(json!({
        "schemaVersion": 2, "id": "loop:zones", "name": "Zones", "revision": "sha256:zones",
        "projectId": "project:zones", "runtimeId": "runtime:zones",
        "controller": {
            "instance": "P1.Zone1", "type": "Control.PID", "revision": "sha256:zones-controller",
            "action": "direct", "taskPeriodMs": 100, "design": "project",
            "modeSignal": "P1.ZonesAuto", "modeWhenTrue": "Auto",
            "project": {
                "family": family, "convention": "project-owned-native-controller",
                "initialization": { "kind": "declared-state-v1", "states": [] },
                "seed": seed, "axes": [],
                "objective": { "requirementId": "P1.Zone1C:target-error", "direction": "minimize" },
                "maximumCandidates": 1, "maximumCycles": 50000, "wallBudgetMs": 60000
            },
            "group": { "members": members }
        },
        "signals": {
            "setpoint": { "symbol": "P1.Zone1SetpointC", "address": "%ML0", "iecType": "LREAL", "direction": "read", "unit": "degC", "range": { "minimum": 0, "maximum": 100 } },
            "processVariable": { "symbol": "P1.Zone1C", "address": "%IL0", "iecType": "LREAL", "direction": "read", "unit": "degC", "range": { "minimum": 0, "maximum": 100 } },
            "manipulatedVariable": { "symbol": "P1.Heater1Pct", "address": "%QL0", "iecType": "LREAL", "direction": "write", "unit": "%", "range": { "minimum": 0, "maximum": 100 } }
        },
        "limits": { "outputMinimum": 0, "outputMaximum": 100 },
        "observedSampleTiming": { "nominalPeriodMs": 100 },
        "parameterBinding": {
            "loop_id": "loop:zones", "task_name": "MainTask",
            "parameters": [
                { "symbol": "P1.Zone1Parameters.ProportionalGain", "iec_type": "LREAL", "unit": "%/degC", "convention": "proportional gain", "minimum": -1_000_000, "maximum": 1_000_000 },
                { "symbol": "P1.Zone1Parameters.IntegralGain", "iec_type": "LREAL", "unit": "%/(degC*s)", "convention": "integral gain", "minimum": -1_000_000, "maximum": 1_000_000 },
                { "symbol": "P1.Zone1Parameters.DerivativeGain", "iec_type": "LREAL", "unit": "%*s/degC", "convention": "derivative gain", "minimum": -1_000_000, "maximum": 1_000_000 },
                { "symbol": "P1.Zone2Parameters.ProportionalGain", "iec_type": "LREAL", "unit": "%/degC", "convention": "proportional gain", "minimum": -1_000_000, "maximum": 1_000_000 },
                { "symbol": "P1.Zone2Parameters.IntegralGain", "iec_type": "LREAL", "unit": "%/(degC*s)", "convention": "integral gain", "minimum": -1_000_000, "maximum": 1_000_000 },
                { "symbol": "P1.Zone2Parameters.DerivativeGain", "iec_type": "LREAL", "unit": "%*s/degC", "convention": "derivative gain", "minimum": -1_000_000, "maximum": 1_000_000 },
                { "symbol": "P1.D11", "iec_type": "LREAL", "unit": "1", "convention": "decoupler entry", "minimum": -1000, "maximum": 1000 },
                { "symbol": "P1.D12", "iec_type": "LREAL", "unit": "1", "convention": "decoupler entry", "minimum": -1000, "maximum": 1000 },
                { "symbol": "P1.D21", "iec_type": "LREAL", "unit": "1", "convention": "decoupler entry", "minimum": -1000, "maximum": 1000 },
                { "symbol": "P1.D22", "iec_type": "LREAL", "unit": "1", "convention": "decoupler entry", "minimum": -1000, "maximum": 1000 }
            ],
            "mode": { "symbol": "P1.ZonesAuto", "manual_value": false }
        },
        "channels": {
            "inputs": [
                { "modelChannelId": "P1.Heater1Pct", "signalId": "manipulatedVariable", "actuator": { "kind": "continuous", "minimum": 0, "maximum": 100 } },
                { "modelChannelId": "P1.Heater2Pct", "signalId": "manipulatedVariable", "actuator": { "kind": "continuous", "minimum": 0, "maximum": 100 } }
            ],
            "outputs": [
                { "modelChannelId": "P1.Zone1C", "signalId": "processVariable", "setpointSignalId": "setpoint" },
                { "modelChannelId": "P1.Zone2C", "signalId": "processVariable", "setpointSignalId": "setpoint" }
            ]
        }
    }))
    .expect("the group loop")
}

/// The decoupler of the zones: `D` by plant input and member, each entry a bound variable of the program.
fn decoupled_loop() -> LoopDefinition {
    group_loop(
        "decoupled-PI",
        &json!({ "kind": "decoupled-pid", "family": "PI", "matrixSymbols": [["P1.D11", "P1.D12"], ["P1.D21", "P1.D22"]] }),
        &[
            (
                "P1.Zone1",
                "P1.Heater1Pct",
                "P1.Zone1C",
                "P1.Zone1Parameters",
            ),
            (
                "P1.Zone2",
                "P1.Heater2Pct",
                "P1.Zone2C",
                "P1.Zone2Parameters",
            ),
        ],
    )
}

/// The paired zones: the two members on the channels `u1`, `u2`, `y1` and `y2` of the paired fixture.
fn paired_loop() -> LoopDefinition {
    group_loop(
        "paired-PI",
        &json!({ "kind": "decentralized-pid", "family": "PI" }),
        &[
            ("P1.Zone1", "u1", "y1", "P1.Zone1Parameters"),
            ("P1.Zone2", "u2", "y2", "P1.Zone2Parameters"),
        ],
    )
}

fn request(extra: &Value) -> TuneRequest {
    let mut fields = json!({ "projectRoot": "/project", "runtimeId": "runtime:zones", "loopId": "loop:zones", "storageFolder": "/storage" });
    for (key, value) in extra.as_object().cloned().unwrap_or_default() {
        fields[key] = value;
    }
    serde_json::from_value(fields).expect("a Tune request")
}

fn requirements() -> TuningRequirements {
    serde_json::from_value(json!({
        "referenceChange": 1.0, "maximumError": 0.3, "settlingBand": 0.1,
        "settlingTimeSeconds": 40.0, "durationSeconds": 80.0
    }))
    .expect("the requirements")
}

#[test]
fn a_decoupled_group_writes_every_member_gain_and_every_decoupler_entry_from_d_equals_k_inverse_diag_k(
) {
    let loop_definition = decoupled_loop();
    let current = CurrentModel {
        source: CurrentModelSource::Supplied,
        model: decoupler_zones_model(),
        identify: None,
    };
    let request = request(&json!({}));
    let inputs = TuneInputs {
        loop_definition: &loop_definition,
        model: Some(&current),
        previous: None,
        request: &request,
        relay: None,
    };
    let outcome = run(&inputs, Some(&requirements())).expect("the decoupled group is designed");

    // K = k·[[0.13, 0.04], [0.04, 0.13]] with k = 0.01/(0.13² − 0.04²), and D = K⁻¹·diag(K) = K₁₁·[[13, −4], [−4, 13]] (C7).
    let k = 0.01 / (0.13 * 0.13 - 0.04 * 0.04);
    let k11 = 0.13 * k;
    let expected = [[13.0 * k11, -4.0 * k11], [-4.0 * k11, 13.0 * k11]];
    let group = outcome
        .result
        .group
        .as_ref()
        .expect("the answer names the group");
    let decoupler = group.decoupler.as_ref().expect("the group has a decoupler");
    for (row, values) in expected.iter().enumerate() {
        for (column, value) in values.iter().enumerate() {
            assert!(
                (decoupler.matrix[row][column] - value).abs() < 1e-9,
                "D[{row}][{column}]"
            );
        }
    }
    assert_eq!(
        group.values.len(),
        10,
        "six member gains and four decoupler entries"
    );
    assert_eq!(group.values["P1.D11"], decoupler.matrix[0][0]);
    assert_eq!(group.values["P1.D22"], decoupler.matrix[1][1]);
    assert_eq!(group.values["P1.Zone1Parameters.DerivativeGain"], 0.0);
    for member in &group.members {
        assert!(
            member.gains.proportional_gain > 0.0 && member.gains.integral_gain > 0.0,
            "{}",
            member.instance
        );
        assert!(
            (member.relative_gain - 1.0).abs() < 1e-3,
            "through the decoupler the relative gains are one"
        );
    }
    let block = decoupler
        .block
        .expect("two members have the block's configuration");
    assert!(
        (block.relative_determinant - 0.529).abs() < 1e-3,
        "the block's relative determinant"
    );
    assert!(!group.applied);
    assert_eq!(outcome.section.group_candidate.as_ref(), Some(group));
    assert_eq!(outcome.result.settings, group.values);
    assert!(matches!(
        outcome.result.outcome,
        CandidateOutcome::Unpredicted { .. }
    ));
}

#[test]
fn a_paired_group_designs_each_member_on_its_own_path_with_no_decoupler() {
    let loop_definition = paired_loop();
    let current = CurrentModel {
        source: CurrentModelSource::Supplied,
        model: zones_model(),
        identify: None,
    };
    let request = request(&json!({}));
    let inputs = TuneInputs {
        loop_definition: &loop_definition,
        model: Some(&current),
        previous: None,
        request: &request,
        relay: None,
    };
    let outcome = run(&inputs, Some(&requirements())).expect("the paired group is designed");
    let group = outcome.result.group.expect("the answer names the group");
    assert!(group.decoupler.is_none());
    assert_eq!(group.values.len(), 6, "each member's three gains");
    for member in &group.members {
        // The zones' own path has the gain 0.225 and the relative gain 1.125 (the decentralized test's numbers).
        assert!(
            (member.path_gain - 0.225).abs() < 1e-3,
            "path gain {}",
            member.path_gain
        );
        assert!(
            (member.relative_gain - 1.125).abs() < 1e-3,
            "relative gain {}",
            member.relative_gain
        );
    }
    assert_eq!(group.members[0].input_channel_id, "u1");
    assert_eq!(group.members[1].output_channel_id, "y2");
}

#[test]
fn a_group_takes_only_automatic_and_no_pins() {
    let loop_definition = decoupled_loop();
    let current = CurrentModel {
        source: CurrentModelSource::Supplied,
        model: decoupler_zones_model(),
        identify: None,
    };
    let named = request(&json!({ "method": "SIMC" }));
    let inputs = TuneInputs {
        loop_definition: &loop_definition,
        model: Some(&current),
        previous: None,
        request: &named,
        relay: None,
    };
    let refusal = run(&inputs, Some(&requirements())).expect_err("a named method is refused");
    // The refusal has two sentences: `Failure::new` keeps the first as `message` and the whole text as `detail`.
    let text = refusal.detail.as_deref().unwrap_or(&refusal.message);
    assert!(text.contains("Choose Automatic"), "{text}");
}

#[test]
fn a_group_without_its_seed_is_refused_by_name() {
    let loop_definition = group_loop(
        "paired-PI",
        &json!({ "kind": "decentralized-pid", "family": "PI" }),
        &[
            ("P1.Zone1", "u1", "y1", "P1.Zone1Parameters"),
            ("P1.Zone2", "u2", "y2", "P1.Zone2Parameters"),
        ],
    );
    let mut without_seed = loop_definition.clone();
    without_seed.controller.project = None;
    let current = CurrentModel {
        source: CurrentModelSource::Supplied,
        model: zones_model(),
        identify: None,
    };
    let request = request(&json!({}));
    let inputs = TuneInputs {
        loop_definition: &without_seed,
        model: Some(&current),
        previous: None,
        request: &request,
        relay: None,
    };
    let refusal = run(&inputs, Some(&requirements())).expect_err("the group needs its seed");
    assert!(
        refusal.message.contains("needs its program to declare"),
        "{}",
        refusal.message
    );
}

#[test]
fn the_isolated_run_of_a_decoupled_group_writes_its_ten_values_each_under_its_own_symbol() {
    // Validate writes the candidate's values as parameter overrides: six member gains and four decoupler entries (C7, B5.3).
    let loop_definition = decoupled_loop();
    let current = CurrentModel {
        source: CurrentModelSource::Supplied,
        model: decoupler_zones_model(),
        identify: None,
    };
    let request = request(&json!({}));
    let inputs = TuneInputs {
        loop_definition: &loop_definition,
        model: Some(&current),
        previous: None,
        request: &request,
        relay: None,
    };
    let outcome = run(&inputs, Some(&requirements())).expect("the decoupled group is designed");
    let group = outcome.result.group.expect("the answer names the group");
    let overrides =
        crate::validation::group_case::overrides(&loop_definition, &group).expect("the overrides");
    assert_eq!(overrides.len(), 10);
    for (symbol, value) in &group.values {
        let written = overrides
            .iter()
            .find(|override_| &override_.symbol == symbol)
            .expect("each value is written");
        assert_eq!(written.value, *value, "{symbol}");
        if symbol.starts_with("P1.D") {
            assert_eq!(written.unit, "1", "a decoupler entry is dimensionless");
        }
    }
    let derivative = overrides
        .iter()
        .find(|override_| override_.symbol == "P1.Zone2Parameters.DerivativeGain")
        .expect("the derivative gain");
    assert_eq!(
        (derivative.minimum, derivative.maximum),
        (-1_000_000.0, 1_000_000.0),
        "the binding's bounds"
    );
}
