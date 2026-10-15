//! The state budget of a loop (`check_state_budget`): each controller instance holds at most `MAXIMUM_STATES` declared states.

use serde_json::{json, Value};

use super::super::tests::complete_loop;
use crate::loop_definition::LoopDefinition;

/// One declared state of the given symbol: a bounded LREAL that starts at 0. A declared state writes its type as `iec_type`, as the
/// shipped loop files do (`InitializedState`).
fn state(symbol: &str) -> Value {
    json!({
        "symbol": symbol, "iec_type": "LREAL", "unit": "%", "minimum": -100.0, "maximum": 100.0,
        "initial": 0.0, "postScan": "bounded"
    })
}

/// `count` declared states, named under `prefix`.
fn states(prefix: &str, count: usize) -> Vec<Value> {
    (0..count)
        .map(|index| state(&format!("{prefix}.S{index}")))
        .collect()
}

/// Member `index` of a PID group, on its own channels and gain variables.
fn member(index: usize) -> Value {
    json!({
        "instance": format!("P1.Zone{index}"), "type": "Control.PID",
        "inputChannelId": format!("u{index}"), "outputChannelId": format!("y{index}"),
        "modeSymbol": "P1.ZonesAuto",
        "parameterSymbols": [
            format!("P1.Zone{index}Parameters.ProportionalGain"),
            format!("P1.Zone{index}Parameters.IntegralGain"),
            format!("P1.Zone{index}Parameters.DerivativeGain"),
        ],
    })
}

/// The complete loop with its program's `states` and, when `members` is given, a PID group of that many members. It is decoded
/// without the checks of `LoopDefinition::decode`, so that only the state budget is under test.
fn loop_with(members: Option<usize>, states: &[Value]) -> LoopDefinition {
    let mut value = complete_loop();
    value["controller"]["project"] = json!({
        "family": "paired-PI",
        "convention": "project-owned-native-controller",
        "initialization": { "kind": "declared-state-v1", "states": states },
        "seed": { "kind": "decentralized-pid", "family": "PI" },
        "axes": [],
        "objective": { "requirementId": "target-error", "direction": "minimize" },
        "maximumCandidates": 1,
        "maximumCycles": 50000,
        "wallBudgetMs": 60000,
    });
    if let Some(members) = members {
        let members: Vec<Value> = (1..=members).map(member).collect();
        value["controller"]["group"] = json!({ "members": members });
    }
    crate::json::decode_value(value).expect("the loop decodes")
}

#[test]
fn a_group_of_sixteen_members_declares_113_states_within_the_limit() {
    // Each member's seven states and one startup state: 16 × 7 + 1 = 113 states, within 32 per member (512).
    let mut declared: Vec<Value> = (1..=16)
        .flat_map(|index| states(&format!("P1.Zone{index}.StateValue"), 7))
        .collect();
    declared.push(state("P1.Started"));
    assert_eq!(declared.len(), 113);
    assert_eq!(loop_with(Some(16), &declared).check_state_budget(), Ok(()));
}

#[test]
fn a_loop_without_a_group_holds_at_most_32_states() {
    assert_eq!(
        loop_with(None, &states("Main.S", 32)).check_state_budget(),
        Ok(()),
        "32 states fit one instance"
    );
    let refusal = loop_with(None, &states("Main.S", 33))
        .check_state_budget()
        .expect_err("33 states do not fit one instance");
    assert!(refusal.contains("declares 33"), "{refusal}");
    assert!(refusal.contains("at most 32 each"), "{refusal}");
}

#[test]
fn a_group_holds_32_states_per_member() {
    assert_eq!(
        loop_with(Some(5), &states("P1.S", 36)).check_state_budget(),
        Ok(()),
        "36 states fit 5 members"
    );
    assert_eq!(
        loop_with(Some(2), &states("P1.S", 64)).check_state_budget(),
        Ok(()),
        "64 states fit 2 members"
    );
    assert!(
        loop_with(Some(2), &states("P1.S", 65))
            .check_state_budget()
            .is_err(),
        "65 states do not fit 2 members"
    );
}

#[test]
fn a_library_pid_counts_its_retained_and_application_states_with_the_program_states() {
    let mut value = complete_loop();
    value["controller"]["pidInitialization"] = json!({
        "kind": "control-pid-v1",
        "maximumSnapshotAgeMs": 100,
        "states": states("Main.Pid", 20),
        "applicationStates": states("Main.App", 13),
    });
    let loop_definition: LoopDefinition =
        crate::json::decode_value(value).expect("the loop decodes");
    assert!(
        loop_definition.check_state_budget().is_err(),
        "20 + 13 = 33 states do not fit one instance"
    );
}
