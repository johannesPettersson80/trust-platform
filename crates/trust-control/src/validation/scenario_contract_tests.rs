#![allow(clippy::float_cmp)] // The window ends and the values are the exact decimal inputs of the TypeScript tests.

use serde_json::json;

use super::*;
use crate::json::decode_value;
use crate::validation::run_validation::{ErrorReference, ErrorStatistic, RunCheck, Window};

fn bounds(id: &str, channel: &str, start: f64, end: f64) -> RunRequirement {
    RunRequirement {
        id: id.to_owned(),
        channel: channel.to_owned(),
        window: Window { start, end },
        check: RunCheck::Bounds {
            minimum: 0.0,
            maximum: 10.0,
        },
    }
}

fn scenario(id: &str, requirements: Vec<RunRequirement>) -> EngineerScenario {
    EngineerScenario {
        id: id.to_owned(),
        model: None,
        operating_outputs: None,
        effects: None,
        initial_values: BTreeMap::new(),
        events: Vec::new(),
        requirements,
    }
}

fn event(at_ms: f64, symbol: &str, value: f64) -> ScenarioEvent {
    ScenarioEvent {
        at_ms,
        symbol: symbol.to_owned(),
        value,
    }
}

/// Whether a campaign holding only `case` is accepted.
fn accepted(case: &EngineerScenario) -> bool {
    valid_engineer_scenarios(std::slice::from_ref(case))
}

#[test]
fn an_editor_scenario_with_one_bounds_check_is_valid() {
    // control-studio-engineer-scenario-editor.test.ts: "adds schema-shaped scenarios" (the scenario the editor adds).
    let check = RunRequirement {
        id: "requirement-1".to_owned(),
        channel: "PV".to_owned(),
        window: Window {
            start: 0.0,
            end: 4.0,
        },
        check: RunCheck::Bounds {
            minimum: 0.0,
            maximum: 10.0,
        },
    };
    assert!(valid_engineer_scenarios(&[scenario(
        "scenario-1",
        vec![check]
    )]));
}

#[test]
fn a_stored_scenario_decodes_with_its_events_and_its_requirement_window() {
    // control-studio-engineer-scenario-editor.test.ts: the first scenario of the raw JSON, without its advanced fields.
    let value = json!({
        "id": "load-step",
        "initialValues": { "Tank": 2 },
        "events": [{ "atMs": 100, "symbol": "Load", "value": 3 }],
        "requirements": [{ "id": "r", "kind": "bounds", "channel": "Tank",
            "window": { "start": 0, "end": 1 }, "minimum": 0, "maximum": 5 }]
    });
    let decoded: EngineerScenario = decode_value(value).expect("a scenario in the stored form");
    assert_eq!(decoded.initial_values.get("Tank"), Some(&2.0));
    assert_eq!(decoded.events, vec![event(100.0, "Load", 3.0)]);
    assert_eq!(
        decoded.requirements[0].window,
        Window {
            start: 0.0,
            end: 1.0
        }
    );
    assert_eq!(
        decoded.requirements[0].check,
        RunCheck::Bounds {
            minimum: 0.0,
            maximum: 5.0
        }
    );
    assert!(decoded.model.is_none() && decoded.effects.is_none());
    assert!(accepted(&decoded));
}

#[test]
fn a_duplicate_scenario_name_is_refused() {
    // control-studio-engineer-scenario-editor.test.ts: "rejects duplicate scenario names".
    let first = scenario("step-response", vec![bounds("r", "PV", 0.0, 4.0)]);
    let second = scenario("step-response", vec![bounds("r", "PV", 0.0, 4.0)]);
    assert!(accepted(&first));
    assert!(!valid_engineer_scenarios(&[first, second]));
}

#[test]
fn the_nominal_identity_and_an_empty_identity_are_refused() {
    assert!(!accepted(&scenario(
        "target-response",
        vec![bounds("r", "PV", 0.0, 4.0)]
    )));
    assert!(!accepted(&scenario("", vec![bounds("r", "PV", 0.0, 4.0)])));
}

#[test]
fn a_campaign_holds_at_most_24_scenarios() {
    let mut scenarios: Vec<EngineerScenario> = (0..24)
        .map(|index| scenario(&format!("case-{index}"), vec![bounds("r", "PV", 0.0, 4.0)]))
        .collect();
    assert!(valid_engineer_scenarios(&scenarios));
    scenarios.push(scenario("case-24", vec![bounds("r", "PV", 0.0, 4.0)]));
    assert!(!valid_engineer_scenarios(&scenarios));
}

#[test]
fn a_scenario_checks_from_one_to_64_requirements() {
    assert!(!accepted(&scenario("none", Vec::new())));
    let sixty_four: Vec<RunRequirement> = (0..64)
        .map(|index| bounds(&format!("r{index}"), "PV", 0.0, 4.0))
        .collect();
    assert!(accepted(&scenario("most", sixty_four.clone())));
    let mut sixty_five = sixty_four;
    sixty_five.push(bounds("r64", "PV", 0.0, 4.0));
    assert!(!accepted(&scenario("too-many", sixty_five)));
}

#[test]
fn events_are_timed_from_zero_and_at_most_12000_of_them() {
    let mut case = scenario("events", vec![bounds("r", "PV", 0.0, 4.0)]);
    case.events = vec![event(0.0, "Load", 3.0)];
    assert!(accepted(&case));
    case.events = vec![event(-1.0, "Load", 3.0)];
    assert!(!accepted(&case));
    case.events = (0..12_000).map(|_| event(100.0, "Load", 3.0)).collect();
    assert!(accepted(&case));
    case.events.push(event(100.0, "Load", 3.0));
    assert!(!accepted(&case));
}

#[test]
fn non_finite_values_and_empty_symbols_are_refused() {
    let mut case = scenario("values", vec![bounds("r", "PV", 0.0, 4.0)]);
    case.initial_values.insert("Tank".to_owned(), 1.0);
    assert!(accepted(&case));
    case.initial_values.insert("Tank".to_owned(), f64::NAN);
    assert!(!accepted(&case));
    case.initial_values.insert("Tank".to_owned(), 1.0);
    case.initial_values.insert(String::new(), 1.0);
    assert!(!accepted(&case));
    case.initial_values.clear();
    case.events = vec![event(f64::NAN, "Load", 1.0)];
    assert!(!accepted(&case));
    case.events = vec![event(0.0, "Load", f64::INFINITY)];
    assert!(!accepted(&case));
    case.events = vec![event(0.0, "", 1.0)];
    assert!(!accepted(&case));
}

#[test]
fn the_latest_check_is_the_check_that_ends_last() {
    // control-studio-scenario-duration.test.ts: the load-trial check that runs until 60 s after a 30 s validation.
    // An earlier check is added to the list to show that the later end wins.
    let late = RunRequirement {
        id: "late-error".to_owned(),
        channel: "Main.PV".to_owned(),
        window: Window {
            start: 20.0,
            end: 60.0,
        },
        check: RunCheck::Error {
            reference: ErrorReference::Channel {
                channel: "Main.SP".to_owned(),
            },
            statistic: ErrorStatistic::Maximum,
            maximum: 0.2,
        },
    };
    let scenarios = [
        scenario("setpoint-step", vec![bounds("early", "Main.PV", 0.0, 30.0)]),
        scenario("load-trial", vec![late]),
    ];
    assert_eq!(
        latest_scenario_check(&scenarios),
        Some(LatestCheck {
            scenario: "load-trial".to_owned(),
            requirement: "late-error".to_owned(),
            end_seconds: 60.0,
        })
    );
}

#[test]
fn a_tie_keeps_the_first_check_in_order() {
    let scenarios = [
        scenario("first", vec![bounds("a", "PV", 0.0, 30.0)]),
        scenario("second", vec![bounds("b", "PV", 0.0, 30.0)]),
    ];
    let check = latest_scenario_check(&scenarios).expect("a check");
    assert_eq!(
        (check.scenario.as_str(), check.requirement.as_str()),
        ("first", "a")
    );
}

#[test]
fn a_check_without_a_finite_end_and_an_empty_list_have_no_latest_check() {
    assert_eq!(latest_scenario_check(&[]), None);
    let open = scenario("open", vec![bounds("r", "PV", 0.0, f64::NAN)]);
    assert_eq!(latest_scenario_check(&[open]), None);
}
