//! Tests of the declared verification scenarios that `controlStudio/inputs` records and `controlStudio/open` answers (specification 35,
//! B4.3, B9.9, A8.1 and A8.4): a declared list comes back as declared, a change of the list invalidates the validations, and the engine
//! refuses a scenario without an identity, a check that ends after the validation and an event between two task samples, and names a
//! scenario that has no name. The fixtures are those of `inputs_action_tests.rs`, and no runtime runs here.

use trust_control::validation::scenario_contract::EngineerScenario;

use super::*;

#[test]
fn the_declared_scenarios_are_answered_by_open_as_declared() {
    let project = Project::new("inputs-scenarios");
    let declared = InputsSection {
        scenarios: Some(Vec::new()),
        ..InputsSection::default()
    };
    answer(&LoopClaims::default(), project.inputs_request(declared)).expect("recorded");
    let studio = ControlStudio::new(Arc::new(Ignore));
    let opened = studio
        .execute(OPEN, &[project.open_request()])
        .expect("answered");
    assert_eq!(
        opened["result"]["inputs"]["scenarios"],
        json!([]),
        "a declared list, even an empty one, comes back as declared: {opened}"
    );
}

#[test]
fn an_invalid_scenario_is_refused_with_its_next_action_and_nothing_is_recorded() {
    let project = Project::new("inputs-bad-scenario");
    let unnamed: EngineerScenario = serde_json::from_value(json!({
        "id": "",
        "initialValues": {},
        "events": [],
        "requirements": [],
    }))
    .expect("the scenario decodes; its empty identity is refused");
    let refused = answer(
        &LoopClaims::default(),
        project.inputs_request(InputsSection {
            scenarios: Some(vec![unnamed]),
            ..InputsSection::default()
        }),
    )
    .expect_err("a scenario without an identity");
    assert_eq!(refused.next_action, NextAction::OpenRequirements);
    let studio = ControlStudio::new(Arc::new(Ignore));
    let opened = studio
        .execute(OPEN, &[project.open_request()])
        .expect("answered");
    assert!(
        opened["result"]["inputs"].get("scenarios").is_none(),
        "the refused list is not recorded: {opened}"
    );
}

/// One declared scenario that the campaign accepts, named `id`.
fn declared_scenario(id: &str) -> EngineerScenario {
    serde_json::from_value(json!({
        "id": id,
        "initialValues": { "Main.PV": 2 },
        "events": [{ "atMs": 100, "symbol": "Main.SP", "value": 3 }],
        "requirements": [{ "id": "r", "kind": "bounds", "channel": "Main.PV",
            "window": { "start": 0, "end": 1 }, "minimum": 0, "maximum": 5 }]
    }))
    .expect("a scenario in the stored form")
}

#[test]
fn a_change_of_the_declared_scenarios_is_reported_as_invalidating_the_validations() {
    let project = Project::new("inputs-scenario-change");
    let claims = LoopClaims::default();
    let with_scenarios = |scenarios: Vec<EngineerScenario>| InputsSection {
        scenarios: Some(scenarios),
        ..InputsSection::default()
    };
    let first = answer(
        &claims,
        project.inputs_request(with_scenarios(vec![declared_scenario("load-step")])),
    )
    .expect("recorded");
    assert_eq!(
        first["invalidated"]["validation"],
        json!(true),
        "scenarios declared where none were held are a change (A8.4)"
    );
    let same = answer(
        &claims,
        project.inputs_request(with_scenarios(vec![declared_scenario("load-step")])),
    )
    .expect("recorded");
    assert_eq!(
        same["invalidated"]["validation"],
        json!(false),
        "the same scenario list invalidates nothing"
    );
    let other = answer(
        &claims,
        project.inputs_request(with_scenarios(vec![declared_scenario("noise")])),
    )
    .expect("recorded");
    assert_eq!(
        other["invalidated"]["validation"],
        json!(true),
        "another scenario list is a change (A8.4)"
    );
}

/// A declared scenario whose one check ends at `end` seconds, as the requirements page declares it (B9.9).
fn scenario_until(end: f64) -> EngineerScenario {
    serde_json::from_value(json!({
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
            "window": { "start": 20.0, "end": end }
        }]
    }))
    .expect("the scenario decodes")
}

#[test]
fn a_check_that_ends_after_the_validation_duration_is_refused_under_the_duration() {
    let project = Project::new("inputs-check-beyond-duration");
    // The requirements validate for 90 s, and the check ends at 120 s: the refusal names the duration it needs (B4.3).
    let refused = answer(
        &LoopClaims::default(),
        project.inputs_request(InputsSection {
            requirements: Some(requirements(2.0)),
            scenarios: Some(vec![scenario_until(120.0)]),
            ..InputsSection::default()
        }),
    )
    .expect_err("a check until 120 s cannot run in a 90 s validation");
    assert_eq!(refused.field.as_deref(), Some("durationSeconds"));
    assert_eq!(refused.next_action, NextAction::OpenRequirements);
    assert_eq!(
        refused.message,
        "load-trial checks late-error until 120 s, after the 90 s validation."
    );
    let study = storage::open_study(
        &project.root,
        &project.loop_definition.id,
        &project.loop_definition.revision,
    )
    .expect("the study");
    assert_eq!(
        study.section::<InputsSection>().expect("read"),
        None,
        "a refused change is not kept"
    );
}

#[test]
fn a_check_that_ends_within_the_validation_duration_is_recorded() {
    let project = Project::new("inputs-check-within-duration");
    answer(
        &LoopClaims::default(),
        project.inputs_request(InputsSection {
            requirements: Some(requirements(2.0)),
            scenarios: Some(vec![scenario_until(90.0)]),
            ..InputsSection::default()
        }),
    )
    .expect("a check that ends at the duration lies inside the validation");
}

#[test]
fn a_scenario_event_between_two_task_samples_is_refused_when_the_list_changes() {
    // The example loop's task runs every 100 ms (B4.3). The page's list reaches the engine as typed: an event at 150 ms is refused
    // under the scenarios' field and nothing is recorded; an event at 200 ms is a sample and is kept.
    let project = Project::new("inputs-scenario-off-grid");
    let change = |at_ms: &str| {
        json!({
            "referenceChange": 1.0,
            "maximumError": 0.3,
            "settlingBand": 0.1,
            "settlingTimeSeconds": 40.0,
            "durationSeconds": 80,
            "scenarioDefinitions": [{
                "id": "load-step",
                "initialValues": [],
                "events": [{ "atMs": at_ms, "symbol": "P1.Setpoint", "value": "3" }],
                "requirements": [{
                    "id": "r", "kind": "bounds", "channel": "P1.ProcessValue",
                    "window": { "start": 0.0, "end": 4.0 }, "minimum": 0, "maximum": 80
                }]
            }]
        })
    };
    let refused = answer(
        &LoopClaims::default(),
        project.change_request(&change("150")),
    )
    .expect_err("150 ms lies between the samples of the 100 ms task");
    assert_eq!(refused.field.as_deref(), Some("scenarios"));
    assert_eq!(
        refused.message,
        "Scenario load-step writes an event at 150 ms, which is not a multiple of the 100 ms task period."
    );
    let study = storage::open_study(
        &project.root,
        &project.loop_definition.id,
        &project.loop_definition.revision,
    )
    .expect("the study");
    assert_eq!(
        study.section::<InputsSection>().expect("read"),
        None,
        "a refused change is not kept"
    );
    let recorded = answer(
        &LoopClaims::default(),
        project.change_request(&change("200")),
    )
    .expect("an event on a sample is recorded");
    assert_eq!(
        recorded["inputs"]["scenarios"][0]["events"][0]["atMs"].as_f64(),
        Some(200.0)
    );
}

#[test]
fn a_scenario_without_a_name_is_named_by_the_engine_when_the_list_changes() {
    // B4.3: the page adds the engine's template, which has no name; the engine gives the first identity no other scenario holds.
    let project = Project::new("inputs-scenario-unnamed");
    let change = json!({
        "referenceChange": 1.0,
        "maximumError": 0.3,
        "settlingBand": 0.1,
        "settlingTimeSeconds": 40.0,
        "durationSeconds": 80,
        "scenarioDefinitions": [
            { "id": "", "initialValues": [], "events": [], "requirements": [{
                "id": "r", "kind": "bounds", "channel": "P1.ProcessValue",
                "window": { "start": 0.0, "end": 4.0 }, "minimum": 0, "maximum": 80 }] },
            { "id": "scenario-1", "initialValues": [], "events": [], "requirements": [{
                "id": "r", "kind": "bounds", "channel": "P1.ProcessValue",
                "window": { "start": 0.0, "end": 4.0 }, "minimum": 0, "maximum": 80 }] }
        ]
    });
    let recorded = answer(&LoopClaims::default(), project.change_request(&change))
        .expect("the list is recorded with a name for each scenario");
    assert_eq!(
        recorded["inputs"]["scenarios"][0]["id"],
        json!("scenario-2")
    );
    assert_eq!(
        recorded["inputs"]["scenarios"][1]["id"],
        json!("scenario-1")
    );
}
