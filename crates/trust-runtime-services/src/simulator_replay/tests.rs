//! `simulator.replay` (specification 11 §6.8.1, "Replaying a trace"; specification 48 RU-60): what
//! was replayed and the answer. A computer's refusal on the wall clock is tested with its control
//! state, which keeps uploads.

use alloc::vec;
use serde_json::json;
use trust_runtime_core::value::Value;

use super::{Difference, FirstDifference, OutputDifference, ReplayKind, ReplayReport};

#[test]
fn what_was_replayed_comes_from_the_sources_hash_and_the_target() {
    // RU-60, "What was replayed": the same sources for the same target are the same program; the
    // same sources for another target are the same program for another device; other sources are
    // a changed program, whatever the target. Catches a kind judged by the target alone, and a
    // changed program taken as the same for its target.
    let (sources, other) = ([1; 32], [2; 32]);
    assert_eq!(
        ReplayKind::of(&sources, 7, &sources, 7),
        ReplayKind::SameProgram
    );
    assert_eq!(
        ReplayKind::of(&sources, 7, &sources, 8),
        ReplayKind::SameSourcesOtherTarget
    );
    assert_eq!(
        ReplayKind::of(&sources, 7, &other, 7),
        ReplayKind::ChangedProgram
    );
    assert_eq!(
        ReplayKind::of(&sources, 7, &other, 8),
        ReplayKind::ChangedProgram
    );
}

#[test]
fn the_answer_holds_what_specification_11_lists() {
    // Specification 11 §6.8.1: the answer is {"matched", "cycles", "kind", "first_difference",
    // "later_differing_cycles", "notes"}; the first difference holds the cycle, its logical time and
    // each differing output (path, address, the trace's value and the build's), or the two faults.
    // Catches a member missing or misnamed, and `matched` that does not follow the first difference.
    let matched = ReplayReport {
        cycles: 3,
        kind: ReplayKind::SameProgram,
        first_difference: None,
        later_differing_cycles: 0,
        notes: vec![],
    };
    assert_eq!(
        matched.to_json(),
        json!({"matched": true, "cycles": 3, "kind": "same_program",
            "first_difference": null, "later_differing_cycles": 0, "notes": []})
    );
    let differing = ReplayReport {
        cycles: 5,
        kind: ReplayKind::ChangedProgram,
        first_difference: Some(FirstDifference {
            cycle: 2,
            time_ns: 20_000_000,
            difference: Difference::Outputs(vec![OutputDifference {
                path: "Main.Speed".into(),
                address: "%QW2".into(),
                trace: Some(Value::Int(5)),
                build: Some(Value::Int(6)),
            }]),
        }),
        later_differing_cycles: 1,
        notes: vec!["the build has no task Slow".into()],
    };
    assert_eq!(
        differing.to_json(),
        json!({"matched": false, "cycles": 5, "kind": "changed_program",
            "first_difference": {"cycle": 2, "time_ns": 20_000_000, "outputs": [
                {"path": "Main.Speed", "address": "%QW2", "trace": 5, "build": 6}]},
            "later_differing_cycles": 1, "notes": ["the build has no task Slow"]})
    );
    let faulted = ReplayReport {
        first_difference: Some(FirstDifference {
            cycle: 4,
            time_ns: 40_000_000,
            difference: Difference::Faults {
                trace: Some("division by zero".into()),
                build: None,
            },
        }),
        ..matched
    };
    assert_eq!(
        faulted.to_json()["first_difference"],
        json!({"cycle": 4, "time_ns": 40_000_000,
            "faults": {"trace": "division by zero", "build": null}})
    );
}
