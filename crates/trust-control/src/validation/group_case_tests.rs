//! Tests of the cases that a PID group's campaign adds (specification 35, C7; `independentReferenceScenarios`), of the run request that
//! carries a group candidate, and of the start values that a group run takes from its declared profile. The fixture is the decoupled
//! zones loop of `study::tune::design::pid_group_tests`, with its second member on its own channel as `setup::group_loop` declares it
//! (the signals `mv-2`, `pv-2` and `sp-2`). The plan refuses a loop that the Rust engine does not run (`rust_engine_support`), so these
//! tests hold once that gate serves a PID group.

#![allow(clippy::float_cmp)] // The targets are exact sums of the fixture's operating point (35 °C) and its unit step.

use std::collections::BTreeMap;

use serde_json::{json, Value};

use super::independent_reference_cases;
use crate::commissioning::adapters::controller_adapter;
use crate::commissioning::transfer_tracking::TransferTracking;
use crate::loop_definition::LoopDefinition;
use crate::tuning::candidate::{CandidateSource, LawCandidate};
use crate::tuning::group_candidate::GroupCandidate;
use crate::tuning::structures::fixtures::{decoupler_zones_model, requirements};
use crate::validation::engineer_verification_campaign::{
    engineer_verification_campaign, CampaignCase,
};
use crate::validation::engineer_verification_plan::EngineerRequirements;
use crate::validation::model_route::model_route;
use crate::validation::run_requests::{isolated_run_request, RunRequestInput, RunSettings};
use crate::validation::run_validation::{ErrorReference, RunCheck, RunRequirement};

/// The decoupled zones loop: two `Control.PID` members, which drive `P1.Heater1Pct` and `P1.Heater2Pct` and measure `P1.Zone1C` and
/// `P1.Zone2C` against their setpoints, with the program's decoupler `D` (C7).
fn group_loop() -> LoopDefinition {
    LoopDefinition::decode(&group_json().to_string()).expect("the group loop decodes")
}

/// The loop as JSON. The second member's output, measurement and setpoint are auxiliary signals of its own channel, and its output has
/// its own requirements, because the loop's task holds only the first member's process variable. Each member has its Manual command:
/// the controller's own for the first, its channel's for the second (the transfer and the coherence check need both, C7).
fn group_json() -> Value {
    json!({
        "schemaVersion": 2, "id": "loop:zones", "name": "Zones", "revision": "sha256:zones",
        "projectId": "project:zones", "runtimeId": "runtime:zones",
        "controller": {
            "instance": "P1.Zone1", "type": "Control.PID", "revision": "sha256:zones-controller",
            "action": "direct", "taskPeriodMs": 100, "design": "project",
            "modeSignal": "P1.ZonesAuto", "modeWhenTrue": "Auto",
            "manualOutputSignal": "P1.Zone1ManualPct", "manualOutputType": "LREAL",
            "project": {
                "family": "decoupled-PI", "convention": "project-owned-native-controller",
                "initialization": { "kind": "declared-state-v1", "states": retained_states() },
                "seed": { "kind": "decoupled-pid", "family": "PI", "matrixSymbols": [["P1.D11", "P1.D12"], ["P1.D21", "P1.D22"]] },
                "axes": [],
                "objective": { "requirementId": "P1.Zone1C:target-error", "direction": "minimize" },
                "maximumCandidates": 1, "maximumCycles": 50000, "wallBudgetMs": 60000
            },
            "group": { "members": [
                member("P1.Zone1", "P1.Heater1Pct", "P1.Zone1C", "P1.Zone1Parameters"),
                member("P1.Zone2", "P1.Heater2Pct", "P1.Zone2C", "P1.Zone2Parameters")
            ] }
        },
        "signals": {
            "setpoint": { "symbol": "P1.Zone1SetpointC", "address": "%ML0", "iecType": "LREAL", "direction": "read", "unit": "degC", "range": { "minimum": 0, "maximum": 100 } },
            "processVariable": { "symbol": "P1.Zone1C", "address": "%IL0", "iecType": "LREAL", "direction": "read", "unit": "degC", "range": { "minimum": 0, "maximum": 100 } },
            "manipulatedVariable": { "symbol": "P1.Heater1Pct", "address": "%QL0", "iecType": "LREAL", "direction": "write", "unit": "%", "range": { "minimum": 0, "maximum": 100 } },
            "additional": [
                { "id": "mv-2", "channel": "member-2", "role": "MV", "symbol": "P1.Heater2Pct", "address": "%QL1", "iecType": "LREAL", "direction": "write", "unit": "%", "range": { "minimum": 0, "maximum": 100 } },
                { "id": "pv-2", "channel": "member-2", "role": "PV", "symbol": "P1.Zone2C", "address": "%IL1", "iecType": "LREAL", "direction": "read", "unit": "degC", "range": { "minimum": 0, "maximum": 100 } },
                { "id": "sp-2", "channel": "member-2", "role": "SP", "symbol": "P1.Zone2SetpointC", "address": "%ML1", "iecType": "LREAL", "direction": "read", "unit": "degC", "range": { "minimum": 0, "maximum": 100 } }
            ]
        },
        "limits": { "outputMinimum": 0, "outputMaximum": 100 },
        "observedSampleTiming": { "nominalPeriodMs": 100 },
        "parameterBinding": {
            "loop_id": "loop:zones", "task_name": "MainTask",
            "parameters": parameters(),
            "mode": { "symbol": "P1.ZonesAuto", "manual_value": false }
        },
        "channels": {
            "inputs": [
                { "modelChannelId": "P1.Heater1Pct", "signalId": "manipulatedVariable", "actuator": { "kind": "continuous", "minimum": 0, "maximum": 100 } },
                { "modelChannelId": "P1.Heater2Pct", "signalId": "mv-2", "manualOutputSignal": "P1.Zone2ManualPct", "manualOutputType": "LREAL",
                  "actuator": { "kind": "continuous", "minimum": 0, "maximum": 100 } }
            ],
            "outputs": [
                { "modelChannelId": "P1.Zone1C", "signalId": "processVariable", "setpointSignalId": "setpoint" },
                { "modelChannelId": "P1.Zone2C", "signalId": "pv-2", "setpointSignalId": "sp-2",
                  "requirements": { "referenceChange": 1.0, "maximumError": 0.3, "settlingBand": 0.1, "settlingTimeSeconds": 40.0 } }
            ]
        }
    })
}

/// One member as the group declares it: its instance, its channels, the group's mode and its three gains of `record`.
fn member(instance: &str, input: &str, output: &str, record: &str) -> Value {
    json!({
        "instance": instance, "type": "Control.PID", "inputChannelId": input, "outputChannelId": output,
        "modeSymbol": "P1.ZonesAuto",
        "parameterSymbols": [
            format!("{record}.ProportionalGain"), format!("{record}.IntegralGain"), format!("{record}.DerivativeGain")
        ],
    })
}

/// The seven retained states of `Control.PID` in the order the shipped loops declare them (`StateValue`, C7).
const RETAINED_FIELDS: [&str; 7] = [
    "Setpoint",
    "SetpointRate",
    "ProcessValue",
    "ProcessValueRate",
    "FeedForward",
    "NominalOutput",
    "SaturatedOutput",
];

/// Each member's seven retained states, which its program owns (`check_member`, C7): constant initial values, so the declared
/// profile needs no model formula. The fixture's own values are not the run's operating point.
fn retained_states() -> Vec<Value> {
    let mut states = Vec::new();
    for instance in ["P1.Zone1", "P1.Zone2"] {
        for field in RETAINED_FIELDS {
            states.push(json!({
                "symbol": format!("{instance}.StateValue.{field}"), "iec_type": "LREAL", "unit": "%",
                "minimum": -110.0, "maximum": 110.0, "initial": 0.0, "postScan": "bounded"
            }));
        }
    }
    states
}

/// The bound parameters: each member's three gains, then the four decoupler entries.
fn parameters() -> Vec<Value> {
    let mut parameters = gains("P1.Zone1Parameters");
    parameters.extend(gains("P1.Zone2Parameters"));
    parameters.extend(["P1.D11", "P1.D12", "P1.D21", "P1.D22"].map(decoupler_entry));
    parameters
}

/// The three gains of one member's record, bounded by ±1,000,000.
fn gains(record: &str) -> Vec<Value> {
    vec![
        json!({ "symbol": format!("{record}.ProportionalGain"), "iec_type": "LREAL", "unit": "%/degC", "convention": "proportional gain", "minimum": -1_000_000, "maximum": 1_000_000 }),
        json!({ "symbol": format!("{record}.IntegralGain"), "iec_type": "LREAL", "unit": "%/(degC*s)", "convention": "integral gain", "minimum": -1_000_000, "maximum": 1_000_000 }),
        json!({ "symbol": format!("{record}.DerivativeGain"), "iec_type": "LREAL", "unit": "%*s/degC", "convention": "derivative gain", "minimum": -1_000_000, "maximum": 1_000_000 }),
    ]
}

/// One decoupler entry, dimensionless and bounded by ±1,000.
fn decoupler_entry(symbol: &str) -> Value {
    json!({ "symbol": symbol, "iec_type": "LREAL", "unit": "1", "convention": "decoupler entry", "minimum": -1000, "maximum": 1000 })
}

/// The candidate's values: each member's gains and the four decoupler entries, the ten values that Apply writes (C7).
fn group_candidate() -> GroupCandidate {
    let values = [
        ("P1.Zone1Parameters.ProportionalGain", 2.0),
        ("P1.Zone1Parameters.IntegralGain", 0.05),
        ("P1.Zone1Parameters.DerivativeGain", 0.0),
        ("P1.Zone2Parameters.ProportionalGain", 2.0),
        ("P1.Zone2Parameters.IntegralGain", 0.05),
        ("P1.Zone2Parameters.DerivativeGain", 0.0),
        ("P1.D11", 1.2),
        ("P1.D12", -0.4),
        ("P1.D21", -0.4),
        ("P1.D22", 1.2),
    ];
    GroupCandidate {
        applied: false,
        source: CandidateSource::Simc,
        members: Vec::new(),
        decoupler: None,
        values: values
            .into_iter()
            .map(|(symbol, value)| (symbol.to_owned(), value))
            .collect(),
    }
}

/// The task: a unit step, an error of 0.3 at most, a band of 0.1, 40 s to settle, and an 80 s run.
fn task() -> EngineerRequirements {
    requirements(1.0, 0.3, 0.1, 40.0, 80.0)
}

/// The campaign of the group on the decoupler model. The start values are empty: the cases' structure does not read them.
fn campaign(loop_definition: &LoopDefinition, values: &BTreeMap<String, f64>) -> Vec<CampaignCase> {
    engineer_verification_campaign(
        loop_definition,
        &decoupler_zones_model(),
        &task(),
        values,
        &BTreeMap::new(),
    )
    .expect("the group's campaign")
}

/// The requirement of a case with this identity.
fn requirement<'a>(case: &'a CampaignCase, id: &str) -> &'a RunRequirement {
    case.requirements
        .iter()
        .find(|requirement| requirement.id == id)
        .expect("the requirement of the case")
}

#[test]
fn the_group_campaign_has_its_nominal_case_and_one_case_per_controlled_output() {
    let cases = campaign(&group_loop(), &group_candidate().values);
    let ids: Vec<&str> = cases.iter().map(|case| case.id.as_str()).collect();
    assert_eq!(
        ids,
        [
            "target-response",
            "independent-reference:P1.Zone1C",
            "independent-reference:P1.Zone2C",
        ]
    );
}

#[test]
fn the_case_of_member_two_steps_only_its_setpoint_and_holds_the_other_at_its_operating_value() {
    let cases = campaign(&group_loop(), &group_candidate().values);
    // Both zones start at 35 °C, and the step is 1 °C (C7): the stepped setpoint starts at 36 °C, the held one at 35 °C.
    let member_one = &cases[1].plan.initial_values;
    assert_eq!(member_one["P1.Zone1SetpointC"], 36.0);
    assert_eq!(member_one["P1.Zone2SetpointC"], 35.0);
    let member_two = &cases[2].plan.initial_values;
    assert_eq!(member_two["P1.Zone2SetpointC"], 36.0);
    assert_eq!(member_two["P1.Zone1SetpointC"], 35.0);
}

#[test]
fn the_held_member_keeps_its_settling_and_error_checks_at_its_operating_value() {
    let cases = campaign(&group_loop(), &group_candidate().values);
    let case = &cases[2];
    // Member one holds at 35 °C: it must settle there, and its error is taken against its held setpoint (C7).
    let RunCheck::Settling { target, .. } = &requirement(case, "P1.Zone1C:response-deadline").check
    else {
        panic!("member one keeps a settling check");
    };
    assert_eq!(*target, 35.0);
    let RunCheck::Error {
        reference: ErrorReference::Channel { channel },
        ..
    } = &requirement(case, "P1.Zone1C:target-error").check
    else {
        panic!("member one keeps an error check");
    };
    assert_eq!(channel, "P1.Zone1SetpointC");
    // The stepped member keeps its own step to 36 °C.
    let RunCheck::Settling { target, .. } = &requirement(case, "P1.Zone2C:response-deadline").check
    else {
        panic!("member two keeps a settling check");
    };
    assert_eq!(*target, 36.0);
}

#[test]
fn each_run_request_of_the_group_carries_its_ten_values_under_their_symbols() {
    let loop_definition = group_loop();
    let candidate = group_candidate();
    let cases = campaign(&loop_definition, &candidate.values);
    let model = decoupler_zones_model();
    let settings = LawCandidate::Group(Box::new(candidate.clone()));
    for case in &cases {
        let input = RunRequestInput {
            loop_definition: &loop_definition,
            model: &model,
            plan: &case.plan,
            case,
            settings: RunSettings::Candidate(&settings),
            simulation: None,
            request_id: "group-case",
            expected_project_revision: "rev-1",
        };
        let request = isolated_run_request(&input).expect("the run request");
        assert_eq!(request.parameter_overrides.len(), 10, "{}", case.id);
        for (symbol, value) in &candidate.values {
            let written = request
                .parameter_overrides
                .iter()
                .find(|written| &written.symbol == symbol)
                .expect("each value is written");
            assert_eq!(written.value, *value, "{symbol} in {}", case.id);
        }
    }
}

#[test]
fn a_loop_without_a_group_has_no_per_output_cases() {
    let mut loop_definition = group_loop();
    loop_definition.controller.group = None;
    let cases = independent_reference_cases(
        &loop_definition,
        &decoupler_zones_model(),
        &task(),
        &group_candidate().values,
        &BTreeMap::new(),
    )
    .expect("no per-output cases");
    assert!(cases.is_empty());
}

#[test]
fn a_group_run_starts_its_controller_states_at_the_declared_profile_of_its_candidate() {
    let loop_definition = group_loop();
    let model = decoupler_zones_model();
    let values = group_candidate().values;
    let route = model_route(&loop_definition, &model, &values).expect("the group's route");
    let profile = controller_adapter(&loop_definition)
        .expect("the group's adapter")
        .declared_profile(&model, &values)
        .expect("the group's declared profile")
        .expect("a declared profile");
    let binding = profile.binding().expect("the profile's binding");
    let TransferTracking::DeclaredState(tracking) = profile
        .tracking(&binding, true)
        .expect("the profile's tracking")
    else {
        panic!("a declared-state tracking");
    };
    assert_eq!(route.initial_values, tracking.initialization_values);
}

#[test]
fn the_group_write_set_is_its_values_through_the_coefficient_path() {
    // Apply and Validate's entry write a group's values by symbol: `law_coefficients` gives exactly the candidate's values (C7).
    let loop_definition = group_loop();
    let candidate = group_candidate();
    let coefficients = crate::validation::run_requests::law_coefficients(
        &loop_definition,
        &LawCandidate::Group(Box::new(candidate.clone())),
    )
    .expect("the write set");
    assert_eq!(coefficients, Some(candidate.values.clone()));
}
