//! The Tune of a PID group (specification 35, C7) on the zones fixtures of `tuning::structures`: a decoupled group designs each member
//! through `D = K⁻¹·diag(K)` and writes every member's gains and every decoupler entry; a paired group designs each member on its own
//! path; and the refusals name their rule. Ported from `control-studio-decoupler.test.ts` and `control-studio-decentralized-pid.test.ts`.

#![allow(clippy::float_cmp)] // The default member and the entries of the group's values are the numbers the design writes.

use serde_json::{json, Value};

use super::{group_failure, run};
use crate::failure::{Cause, NextAction};
use crate::identification::refusal::Refusal;
use crate::loop_definition::LoopDefinition;
use crate::messages::{CandidateOutcome, TuneRequest};
use crate::study::current_model::{CurrentModel, CurrentModelSource};
use crate::study::tune::TuneInputs;
use crate::tuning::requirements::TuningRequirements;
use crate::tuning::structures::fixtures::{decoupler_zones_model, zones_model};
use crate::tuning::structures::pid_group::GroupRefusal;

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
        pins: None,
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
        pins: None,
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
fn a_groups_answer_names_each_channel_by_the_signal_the_loop_binds_to_it() {
    // L19: the page named a member's channels by looking the channel up in the loop's channels and signals (and fell back to the channel
    // identifier). The answer names them, so the page resolves nothing. The decoupled zones bind their heaters to the manipulated variable
    // `P1.Heater1Pct` and their zones to the process variable `P1.Zone1C`; the paired zones' channels `u1` to `y2` are bound to no signal
    // the loop declares, so each is named by its identifier.
    let current = |model| CurrentModel {
        source: CurrentModelSource::Supplied,
        model,
        identify: None,
    };
    let request = request(&json!({}));
    let decoupled = decoupled_loop();
    let model = current(decoupler_zones_model());
    let outcome = run(
        &TuneInputs {
            loop_definition: &decoupled,
            model: Some(&model),
            previous: None,
            request: &request,
            relay: None,
            pins: None,
        },
        Some(&requirements()),
    )
    .expect("the decoupled group is designed");
    let group = outcome.result.group.expect("the answer names the group");
    let named: Vec<(Option<&str>, Option<&str>)> = group
        .members
        .iter()
        .map(|member| (member.input_name.as_deref(), member.output_name.as_deref()))
        .collect();
    assert_eq!(
        named,
        [
            (Some("P1.Heater1Pct"), Some("P1.Zone1C")),
            (Some("P1.Heater1Pct"), Some("P1.Zone1C")),
        ]
    );
    assert_eq!(
        group.decoupler.and_then(|decoupler| decoupler.input_names),
        Some(vec!["P1.Heater1Pct".to_owned(), "P1.Heater1Pct".to_owned()]),
        "the rows of D are named as the members' inputs are"
    );

    let paired = paired_loop();
    let model = current(zones_model());
    let outcome = run(
        &TuneInputs {
            loop_definition: &paired,
            model: Some(&model),
            previous: None,
            request: &request,
            relay: None,
            pins: None,
        },
        Some(&requirements()),
    )
    .expect("the paired group is designed");
    let group = outcome.result.group.expect("the answer names the group");
    assert_eq!(
        (
            group.members[0].input_name.as_deref(),
            group.members[1].output_name.as_deref()
        ),
        (Some("u1"), Some("y2")),
        "a channel the loop binds to no signal is named by its identifier"
    );
}

/// The zones' requirements with the second output's own record: its settling time is `settling_time_seconds`, and the record's own
/// values, the first output's, keep 40 s (B4.3, C7).
fn requirements_with_second_output(settling_time_seconds: f64) -> TuningRequirements {
    serde_json::from_value(json!({
        "referenceChange": 1.0, "maximumError": 0.3, "settlingBand": 0.1,
        "settlingTimeSeconds": 40.0, "durationSeconds": 80.0,
        "channelRequirements": {
            "y2": {
                "referenceChange": 1.0, "maximumError": 0.3, "settlingBand": 0.1,
                "settlingTimeSeconds": settling_time_seconds
            }
        }
    }))
    .expect("the requirements with the second output's own record")
}

#[test]
fn a_paired_member_is_designed_to_its_own_settling_time_and_the_first_keeps_the_record() {
    // C7, step 3: on the zones' path (τ = 7.97 s, no delay), Robust's λ = 4τ/3 leaves 0.24 of the change at 15 s, outside the 0.1 band,
    // so Robust misses; Balanced's λ = 2τ/3 leaves 0.06, inside it. Member two is then Balanced: its Kp = τ/(K·λ) is twice Robust's, and
    // its Ti = τ in both. Member one keeps the record's 40 s, which Robust meets.
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
        pins: None,
    };
    let shared = run(&inputs, Some(&requirements()))
        .expect("the group designed on the record")
        .result
        .group
        .expect("the answer names the group");
    let own = run(&inputs, Some(&requirements_with_second_output(15.0)))
        .expect("the group designed on the second output's own record");
    let own_group = own.result.group.expect("the answer names the group");

    assert_eq!(
        own_group.members[0], shared.members[0],
        "member one holds to the record"
    );
    let robust = &shared.members[1];
    let balanced = &own_group.members[1];
    let proportional_ratio = balanced.gains.proportional_gain / robust.gains.proportional_gain;
    let integral_ratio = balanced.gains.integral_gain / robust.gains.integral_gain;
    // The two-point read of the path finds a delay of the order of 1e-7 s, not exactly zero, and Kp = τ/(K·(λ + θ)): the ratio is
    // two to within that read, about 1e-8.
    assert!(
        (proportional_ratio - 2.0).abs() < 1e-6,
        "Balanced's Kp is twice Robust's: {proportional_ratio}"
    );
    assert!(
        (integral_ratio - 2.0).abs() < 1e-6,
        "Balanced's Ki is twice Robust's: {integral_ratio}"
    );
    assert_eq!(balanced.gains.derivative_gain, 0.0);
    // The values Apply writes are the members' designs.
    assert_eq!(
        own_group.values["P1.Zone2Parameters.ProportionalGain"],
        balanced.gains.proportional_gain
    );
}

#[test]
fn a_member_that_no_simc_response_meets_is_refused_and_its_poles_are_not_placed() {
    // One second is faster than the zones' Fast response (λ = τ/3 = 2.7 s), so no SIMC response meets member two's record. Automatic
    // would place poles for it (C1), but a group member is a SIMC PI (C7), so Tune refuses the group with the shortfall.
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
        pins: None,
    };
    let failure = run(&inputs, Some(&requirements_with_second_output(1.0)))
        .expect_err("no SIMC response meets member two's record");
    let text = failure.detail.unwrap_or(failure.message);
    assert!(
        text.contains("cannot meet the requirements on the model"),
        "{text}"
    );
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
        pins: None,
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
        pins: None,
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
        pins: None,
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

#[test]
fn a_members_requirements_open_requirements_and_the_model_opens_loop_setup() {
    // C7, step 3: a member whose PI cannot meet its own requirements is the engineer's requirements to change, as in a cascade; the
    // model, the pairing and the decoupler are the loop setup's.
    let requirements = group_failure(GroupRefusal::Requirements(Refusal::new(
        "member two misses",
    )));
    assert_eq!(
        (requirements.cause, requirements.next_action),
        (Cause::TaskInfeasible, NextAction::OpenRequirements)
    );
    assert_eq!(requirements.message, "member two misses");
    let model = group_failure(GroupRefusal::Model(Refusal::new("the pairing conflicts")));
    assert_eq!(
        (model.cause, model.next_action),
        (Cause::ModelOrData, NextAction::OpenLoopSetup)
    );
}
