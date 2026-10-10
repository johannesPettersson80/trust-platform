//! Tests of the verification campaign (`engineerVerificationCampaign.ts` and `campaignStatus`). The test of the declared check
//! beyond the run is ported from control-studio-scenario-duration.test.ts. The other refusals and the case order are the
//! rules of the same `TypeScript` function, on the engineer fixture of the suite (control-loop.json, as its `pidLoop`
//! builds it). The campaign status is tested on its verdicts (`campaign_verdict`), since a whole run is not needed.

#![allow(clippy::float_cmp)] // The overrides are the literals the scenario JSON carries, compared exactly on purpose.

use std::collections::BTreeMap;

use serde_json::{json, Value};

use super::*;
use crate::failure::{Cause, Failure, NextAction};
use crate::identification::independent_validation::RequirementStatus;
use crate::loop_definition::LoopDefinition;
use crate::model::{decode_general_model, GeneralModel};
use crate::validation::engineer_verification_plan::{engineer_requirements, EngineerRequirements};
use crate::validation::run_validation::RunCheck;
use crate::validation::texts;

const FIXTURE: &str = include_str!(
    "../../../../editors/vscode/src/test/fixtures/control-studio-engineer/control-loop.json"
);

/// The engineer fixture as the suite's `pidLoop` builds it: a first-order proportional loop with the PID law and the
/// parameters `Main.Kp`, `Main.Ki` and `Main.Kd`. Its task period is 20 ms.
fn fixture_loop() -> LoopDefinition {
    let mut value: Value = crate::json::parse_text(FIXTURE).expect("the fixture is JSON");
    // The Rust engine refuses a declared analysis (`rust_engine_support`, a parked feature): the suite's loop drops it.
    if let Some(fields) = value.as_object_mut() {
        fields.remove("analysis");
        fields.remove("analysisParameterSymbols");
    }
    value["controller"]["type"] = json!("Control.PID");
    value["controller"]["design"] = json!("pid");
    value["controller"]["law"] = json!({
        "kind": "pid",
        "parameterRecordSymbol": "Main.Parameters",
        "parameterRevisionSymbol": "Main.ParameterRevision",
        "proportionalGainSymbol": "Main.Kp",
        "integralGainSymbol": "Main.Ki",
        "derivativeGainSymbol": "Main.Kd"
    });
    value["parameterBinding"]["parameters"] = json!([
        { "symbol": "Main.Kp", "iec_type": "LREAL", "unit": "V/m", "convention": "proportional gain", "minimum": 0, "maximum": 100 },
        { "symbol": "Main.Ki", "iec_type": "LREAL", "unit": "V/(m*s)", "convention": "integral gain", "minimum": 0, "maximum": 100 },
        { "symbol": "Main.Kd", "iec_type": "LREAL", "unit": "V*s/m", "convention": "derivative gain", "minimum": 0, "maximum": 100 }
    ]);
    LoopDefinition::decode(&value.to_string()).expect("the fixture decodes")
}

/// The fixture loop with declared verification scenarios.
fn loop_with_scenarios(scenarios: Value) -> LoopDefinition {
    let mut loop_definition = fixture_loop();
    loop_definition.verification_scenarios = Some(scenarios);
    loop_definition
}

/// The suite's campaign model: a first-order process with a 0.5 s delay, validated as `model-4`.
fn fixture_model() -> GeneralModel {
    decode_general_model(&json!({
        "kind": "transfer-function",
        "numerator": [1.5],
        "denominator": [4.0, 1.0],
        "delaySeconds": 0.5,
        "revision": "model-4",
        "source": "provided",
        "validation": "validated",
        "evidence": ["declared-fixture"],
        "inputs": [{ "id": "u", "unit": "V", "operatingRange": { "minimum": -20.0, "maximum": 20.0 } }],
        "outputs": [{ "id": "y", "unit": "m", "operatingRange": { "minimum": -20.0, "maximum": 20.0 } }]
    }))
    .expect("the fixture model decodes")
}

/// The suite's campaign task: a unit reference change, a 0.2 m error limit, a 0.1 m settling band, 15 s to settle, and
/// a 30 s validation.
fn fixture_task() -> EngineerRequirements {
    engineer_requirements(&json!({
        "referenceChange": 1.0,
        "maximumError": 0.2,
        "settlingBand": 0.1,
        "settlingTimeSeconds": 15.0,
        "durationSeconds": 30.0
    }))
    .expect("the fixture requirements decode")
}

/// The gains the parameter binding names.
fn fixture_gains() -> BTreeMap<String, f64> {
    BTreeMap::from([
        ("Main.Kp".to_owned(), 1.0),
        ("Main.Ki".to_owned(), 0.2),
        ("Main.Kd".to_owned(), 0.0),
    ])
}

/// A declared bounds requirement on a channel, held over the whole 30 s run.
fn bounds_requirement(id: &str, channel: &str) -> Value {
    json!({
        "id": id,
        "kind": "bounds",
        "channel": channel,
        "window": { "start": 0.0, "end": 30.0 },
        "minimum": -20.0,
        "maximum": 20.0
    })
}

fn campaign(
    loop_definition: &LoopDefinition,
    task: &EngineerRequirements,
) -> Result<Vec<CampaignCase>, Failure> {
    engineer_verification_campaign(
        loop_definition,
        &fixture_model(),
        task,
        &fixture_gains(),
        &BTreeMap::new(),
    )
}

/// The campaign is refused with `text`, for `cause`, and the next action opens the requirements.
fn assert_refused(result: Result<Vec<CampaignCase>, Failure>, text: &str, cause: Cause) {
    let failure = result.expect_err("the campaign is refused");
    assert_eq!(failure.message, text);
    assert_eq!(failure.cause, cause);
    assert_eq!(failure.next_action, NextAction::OpenRequirements);
}

#[test]
fn the_target_response_comes_first_and_the_envelope_cases_follow_in_their_order() {
    let mut task = fixture_task();
    task.constant_input_disturbance = Some(0.5);
    task.measurement_noise_band = Some(0.05);
    task.gain_uncertainty_fraction = Some(0.2);
    let cases = campaign(&fixture_loop(), &task).expect("the campaign is valid");
    let ids: Vec<&str> = cases.iter().map(|case| case.id.as_str()).collect();
    assert_eq!(
        ids,
        [
            "target-response",
            "load-rejection",
            "measurement-noise",
            "lower-gain",
            "upper-gain"
        ]
    );
    assert!(cases[0].effects.is_empty() && cases[0].events.is_empty());
}

/// Ported from control-studio-scenario-duration.test.ts ("refuses a declared check beyond the validation run with the
/// duration it needs"): a check that ends after the run is refused, and the refusal names the duration it needs. Its fix
/// is in Requirements, not in Tune.
#[test]
fn a_declared_check_beyond_the_run_names_the_duration_it_needs() {
    let loop_definition = loop_with_scenarios(json!([{
        "id": "load-trial",
        "initialValues": {},
        "events": [],
        "requirements": [{
            "id": "late-error",
            "kind": "error",
            "channel": "Main.PV",
            "reference": { "channel": "Main.SP" },
            "statistic": "maximum",
            "maximum": 0.2,
            "window": { "start": 20.0, "end": 60.0 }
        }]
    }]));
    let failure = campaign(&loop_definition, &fixture_task())
        .expect_err("a check until 60 s cannot run in a 30 s validation");
    assert_eq!(
        failure.message,
        "load-trial checks late-error until 60 s, after the 30 s validation."
    );
    assert_eq!(
        failure.detail,
        Some(texts::scenario_check_after_run(
            "load-trial",
            "late-error",
            60.0,
            30.0
        ))
    );
    assert_eq!(failure.cause, Cause::Settings);
    assert_eq!(failure.next_action, NextAction::OpenRequirements);
}

#[test]
fn a_declared_scenario_cannot_write_a_frozen_controller_parameter() {
    let loop_definition = loop_with_scenarios(json!([{
        "id": "retune",
        "initialValues": { "Main.Kp": 2.0 },
        "events": [],
        "requirements": [bounds_requirement("bounds-1", "Main.PV")]
    }]));
    assert_refused(
        campaign(&loop_definition, &fixture_task()),
        &texts::scenario_overwrites_parameters("retune"),
        Cause::Settings,
    );
}

#[test]
fn a_declared_event_falls_on_the_task_grid_inside_the_run() {
    // The task period is 20 ms: 25 ms is off the grid, and 40 s lies after the 30 s run.
    let off_grid = loop_with_scenarios(json!([{
        "id": "late-event",
        "initialValues": {},
        "events": [{ "atMs": 25.0, "symbol": "Main.SP", "value": 1.0 }],
        "requirements": [bounds_requirement("bounds-1", "Main.PV")]
    }]));
    assert_refused(
        campaign(&off_grid, &fixture_task()),
        &texts::scenario_events_off_grid("late-event"),
        Cause::Settings,
    );
    let after_run = loop_with_scenarios(json!([{
        "id": "late-event",
        "initialValues": {},
        "events": [{ "atMs": 40_000.0, "symbol": "Main.SP", "value": 1.0 }],
        "requirements": [bounds_requirement("bounds-1", "Main.PV")]
    }]));
    assert_refused(
        campaign(&after_run, &fixture_task()),
        &texts::scenario_events_off_grid("late-event"),
        Cause::Settings,
    );
}

#[test]
fn a_declared_scenario_cannot_reuse_an_automatic_case_identity() {
    let mut task = fixture_task();
    task.gain_uncertainty_fraction = Some(0.2);
    let loop_definition = loop_with_scenarios(json!([{
        "id": "lower-gain",
        "initialValues": {},
        "events": [],
        "requirements": [bounds_requirement("bounds-1", "Main.PV")]
    }]));
    assert_refused(
        campaign(&loop_definition, &task),
        &texts::scenario_duplicates_automatic(),
        Cause::Settings,
    );
}

#[test]
fn a_declared_case_keeps_its_overrides_events_effects_and_its_own_requirements() {
    let loop_definition = loop_with_scenarios(json!([{
        "id": "step",
        "initialValues": { "Main.PV": 1.0 },
        "events": [{ "atMs": 1000.0, "symbol": "Main.SP", "value": 2.0 }],
        "effects": [{
            "id": "d1",
            "target": "process_input",
            "channel": 0,
            "mode": "add",
            "startSeconds": 0.0,
            "generator": "constant",
            "value": 0.5
        }],
        "requirements": [bounds_requirement("bounds-1", "Main.PV")]
    }]));
    let cases = campaign(&loop_definition, &fixture_task()).expect("the campaign is valid");
    assert_eq!(cases.len(), 2);
    let step = cases
        .last()
        .expect("the declared case follows the target case");
    assert_eq!(step.id, "step");
    assert_eq!(step.initial_overrides.get("Main.PV"), Some(&1.0));
    assert_eq!(step.events.len(), 1);
    assert_eq!(step.effects.len(), 1);
    assert_eq!(step.effects[0].id, "d1");
    assert!(step
        .requirements
        .iter()
        .any(|requirement| requirement.id == "bounds-1"));
    assert!(!step.requirements.iter().any(|requirement| matches!(
        requirement.check,
        RunCheck::Error { .. } | RunCheck::Settling { .. }
    )));
}

#[test]
fn a_supplied_model_must_decode_and_an_operating_range_needs_its_own_model() {
    let invalid = loop_with_scenarios(json!([{
        "id": "bad-model",
        "model": {},
        "initialValues": {},
        "events": [],
        "requirements": [bounds_requirement("bounds-1", "Main.PV")]
    }]));
    assert_refused(
        campaign(&invalid, &fixture_task()),
        &texts::scenario_model_invalid("bad-model"),
        Cause::ModelOrData,
    );
    let range = loop_with_scenarios(json!([{
        "id": "range",
        "operatingOutputs": [1.0],
        "initialValues": {},
        "events": [],
        "requirements": [bounds_requirement("bounds-1", "Main.PV")]
    }]));
    assert_refused(
        campaign(&range, &fixture_task()),
        &texts::scenario_operating_model_missing("range"),
        Cause::ModelOrData,
    );
}

#[test]
fn malformed_declared_scenarios_are_an_invalid_campaign() {
    let malformed = loop_with_scenarios(json!([{ "id": "x" }]));
    assert_refused(
        campaign(&malformed, &fixture_task()),
        &texts::scenarios_invalid(),
        Cause::Settings,
    );
}

#[test]
fn an_envelope_refusal_stops_the_campaign_before_any_run_starts() {
    let mut task = fixture_task();
    task.measurement_noise_band = Some(-0.1);
    assert_refused(
        campaign(&fixture_loop(), &task),
        &texts::noise_bounds_invalid(),
        Cause::Settings,
    );
}

fn verdict<'a>(id: &'a str, revision: &'a str, status: RequirementStatus) -> Verdict<'a> {
    Verdict {
        id,
        revision,
        status,
    }
}

#[test]
fn a_campaign_passes_when_each_expected_case_passes_once_on_one_revision() {
    let expected = vec!["target-response".to_owned(), "lower-gain".to_owned()];
    let results = [
        verdict("target-response", "r1", RequirementStatus::Pass),
        verdict("lower-gain", "r1", RequirementStatus::Pass),
    ];
    assert_eq!(
        campaign_verdict(&expected, &results),
        RequirementStatus::Pass
    );
}

#[test]
fn a_failed_case_fails_the_campaign_even_when_other_cases_are_missing() {
    let expected = vec!["target-response".to_owned(), "lower-gain".to_owned()];
    let results = [verdict("target-response", "r1", RequirementStatus::Fail)];
    assert_eq!(
        campaign_verdict(&expected, &results),
        RequirementStatus::Fail
    );
}

#[test]
fn a_campaign_is_inconclusive_when_a_case_is_missing_or_not_passing() {
    let expected = vec!["target-response".to_owned(), "lower-gain".to_owned()];
    let missing = [verdict("target-response", "r1", RequirementStatus::Pass)];
    assert_eq!(
        campaign_verdict(&expected, &missing),
        RequirementStatus::Inconclusive
    );
    let not_passing = [
        verdict("target-response", "r1", RequirementStatus::Pass),
        verdict("lower-gain", "r1", RequirementStatus::Inconclusive),
    ];
    assert_eq!(
        campaign_verdict(&expected, &not_passing),
        RequirementStatus::Inconclusive
    );
}

#[test]
fn a_campaign_is_inconclusive_without_cases_on_repeated_ids_or_mixed_revisions() {
    assert_eq!(campaign_verdict(&[], &[]), RequirementStatus::Inconclusive);
    let expected = vec!["a".to_owned(), "b".to_owned()];
    let repeated = [
        verdict("a", "r1", RequirementStatus::Pass),
        verdict("a", "r1", RequirementStatus::Pass),
    ];
    assert_eq!(
        campaign_verdict(&expected, &repeated),
        RequirementStatus::Inconclusive
    );
    let mixed = [
        verdict("a", "r1", RequirementStatus::Pass),
        verdict("b", "r2", RequirementStatus::Pass),
    ];
    assert_eq!(
        campaign_verdict(&expected, &mixed),
        RequirementStatus::Inconclusive
    );
}
