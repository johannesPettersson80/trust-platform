//! Tests of the mapping answer (specification 35, B1.2 and B3.6): the loop the panel shows wins, one other loop is chosen, several
//! others are ambiguous, no loop is unmatched, a recording's selection says whether it changed, and each choice names a family its
//! adapter realizes.

use std::collections::BTreeMap;

use serde_json::json;

use super::{
    match_mapping, own_mapping, MappingOption, MappingOutcome, MappingRequest, AMBIGUOUS, UNMATCHED,
};
use crate::loop_definition::{LoopDefinition, Range};
use crate::recording::opened_channels::RecordingSignalSelection;
use crate::setup::catalog::CompiledTask;
use crate::setup::discovery::{
    BindingSelection, BindingTarget, ControllerCompatibility, DiscoveredController, Discovery,
    SelectableSignal,
};

/// A loop of the project `project:a` in `runtime:local`, with the id given.
fn loop_of(id: &str) -> LoopDefinition {
    let value = json!({
        "schemaVersion": 2, "id": id, "name": id, "revision": "sha256:loop-1",
        "projectId": "project:a", "runtimeId": "runtime:local",
        "controller": { "instance": "Main.TankController", "type": "Control.PID", "revision": "sha256:controller-1",
            "action": "direct", "taskPeriodMs": 10 },
        "signals": {
            "setpoint": { "symbol": "Main.SP", "address": "%ML0", "iecType": "LREAL", "direction": "read",
                "unit": "%", "range": { "minimum": 0, "maximum": 100 } },
            "processVariable": { "symbol": "Main.PV", "address": "%IL0", "iecType": "LREAL", "direction": "read",
                "unit": "%", "range": { "minimum": 0, "maximum": 100 } },
            "manipulatedVariable": { "symbol": "Main.MV", "address": "%QL0", "iecType": "LREAL", "direction": "write",
                "unit": "%", "range": { "minimum": 0, "maximum": 100 } }
        },
        "limits": { "outputMinimum": 0, "outputMaximum": 100 },
        "observedSampleTiming": { "nominalPeriodMs": 10 }
    });
    LoopDefinition::decode(&value.to_string()).expect("the fixture is a complete loop definition")
}

fn signal(id: &str, symbol: &str) -> SelectableSignal {
    SelectableSignal {
        id: id.to_owned(),
        symbol: symbol.to_owned(),
        address: None,
        iec_type: "LREAL".to_owned(),
        unit: "%".to_owned(),
        bounds: Range {
            minimum: 0.0,
            maximum: 100.0,
        },
        channel: None,
        role: "MV".to_owned(),
        order: 0,
    }
}

fn accepted(families: &[&str]) -> ControllerCompatibility {
    ControllerCompatibility::Accepted {
        families: families.iter().map(|family| (*family).to_owned()).collect(),
        family_settings: BTreeMap::new(),
        recommended_family: None,
        methods: None,
        method_titles: None,
    }
}

/// A discovered loop whose input and output are the given symbols, under the controller instance and with the compatibility given.
fn target(
    id: &str,
    instance: &str,
    inputs: &[&str],
    outputs: &[&str],
    compatibility: ControllerCompatibility,
) -> BindingTarget {
    BindingTarget {
        configuration_path: format!("control-loops/{id}.json"),
        loop_definition: loop_of(id),
        controller: DiscoveredController {
            instance: instance.to_owned(),
            type_name: "Control.PID".to_owned(),
            tasks: vec![CompiledTask {
                name: "MainTask".to_owned(),
                interval_ms: 10.0,
            }],
        },
        inputs: inputs
            .iter()
            .enumerate()
            .map(|(order, symbol)| signal(&format!("in{order}"), symbol))
            .collect(),
        outputs: outputs
            .iter()
            .enumerate()
            .map(|(order, symbol)| signal(&format!("out{order}"), symbol))
            .collect(),
        controller_compatibility: compatibility,
    }
}

fn discovery(targets: Vec<BindingTarget>) -> Discovery {
    Discovery {
        targets,
        rejected: Vec::new(),
    }
}

fn request(loop_id: &str, inputs: &[&str], outputs: &[&str]) -> MappingRequest {
    MappingRequest {
        loop_id: loop_id.to_owned(),
        inputs: inputs.iter().map(|symbol| (*symbol).to_owned()).collect(),
        outputs: outputs.iter().map(|symbol| (*symbol).to_owned()).collect(),
        recording_selection: None,
    }
}

#[test]
fn the_loop_the_panel_shows_wins_and_only_the_other_loops_families_are_offered() {
    let found = discovery(vec![
        target(
            "loop:tank",
            "Main.TankController",
            &["Main.MV"],
            &["Main.PV"],
            accepted(&["P", "PI"]),
        ),
        target(
            "loop:matching",
            "Main.MatchingPID",
            &["Main.MV"],
            &["Main.PV"],
            accepted(&["PI"]),
        ),
    ]);
    let answer = match_mapping(&found, &request("loop:tank", &["Main.MV"], &["Main.PV"]));
    assert_eq!(answer.outcome, MappingOutcome::Current);
    assert_eq!(answer.sentence, "");
    assert!(!answer.blocks_run, "the loop the panel shows can run");
    assert_eq!(
        answer.selection, None,
        "the current loop is not chosen again"
    );
    let labels: Vec<(&str, &str)> = answer
        .options
        .iter()
        .map(|option| (option.loop_id.as_str(), option.label.as_str()))
        .collect();
    assert_eq!(labels, [("loop:matching", "Main.MatchingPID · PI")]);
}

#[test]
fn one_other_loop_with_the_signals_is_chosen_with_its_selection() {
    let found = discovery(vec![target(
        "loop:other",
        "Main.OtherPID",
        &["Main.MV"],
        &["Main.PV"],
        accepted(&["P", "PID"]),
    )]);
    let answer = match_mapping(&found, &request("loop:tank", &["Main.MV"], &["Main.PV"]));
    assert_eq!(answer.outcome, MappingOutcome::Other);
    assert_eq!(answer.sentence, "");
    assert!(
        !answer.blocks_run,
        "the one other loop is chosen, so the mapping does not stop the work"
    );
    assert_eq!(
        answer.selection,
        Some(BindingSelection {
            loop_id: "loop:other".to_owned(),
            controller_instance: "Main.OtherPID".to_owned(),
            input_ids: vec!["in0".to_owned()],
            output_ids: vec!["out0".to_owned()],
        })
    );
    assert_eq!(
        answer.options.len(),
        2,
        "one choice per family the adapter realizes"
    );
    assert_eq!(answer.options[1].family, "PID");
}

#[test]
fn several_other_loops_without_the_current_one_are_ambiguous_and_say_so() {
    let found = discovery(vec![
        target(
            "loop:a",
            "Main.A",
            &["Main.MV"],
            &["Main.PV"],
            accepted(&["P"]),
        ),
        target(
            "loop:b",
            "Main.B",
            &["Main.MV"],
            &["Main.PV"],
            accepted(&["P"]),
        ),
    ]);
    let answer = match_mapping(&found, &request("loop:tank", &["Main.MV"], &["Main.PV"]));
    assert_eq!(answer.outcome, MappingOutcome::Ambiguous);
    assert_eq!(answer.sentence, AMBIGUOUS);
    assert!(
        answer.blocks_run,
        "an ambiguous mapping stops the work until a controller is chosen"
    );
    assert_eq!(answer.selection, None);
    assert_eq!(answer.options.len(), 2);
}

#[test]
fn a_mapping_that_no_loop_has_is_unmatched_and_names_the_mapping_to_choose() {
    let found = discovery(vec![target(
        "loop:other",
        "Main.OtherPID",
        &["Main.Elsewhere"],
        &["Main.PV"],
        accepted(&["P"]),
    )]);
    let answer = match_mapping(&found, &request("loop:tank", &["Main.MV"], &["Main.PV"]));
    assert_eq!(answer.outcome, MappingOutcome::Unmatched);
    assert_eq!(answer.sentence, UNMATCHED);
    assert!(
        answer.blocks_run,
        "a mapping that no loop has stops the work"
    );
    assert_eq!(answer.options, [] as [MappingOption; 0]);
}

#[test]
fn a_side_counts_each_symbol_once_in_any_order_and_a_partial_side_matches_nothing() {
    let found = discovery(vec![target(
        "loop:tank",
        "Main.TankController",
        &["Main.A", "Main.B"],
        &["Main.PV"],
        accepted(&["P"]),
    )]);
    let reversed = match_mapping(
        &found,
        &request("loop:tank", &["Main.B", "Main.A"], &["Main.PV"]),
    );
    assert_eq!(
        reversed.outcome,
        MappingOutcome::Current,
        "the order of the symbols is not the mapping"
    );
    let partial = match_mapping(&found, &request("loop:tank", &["Main.A"], &["Main.PV"]));
    assert_eq!(partial.outcome, MappingOutcome::Unmatched);
}

#[test]
fn a_loop_whose_adapter_realizes_no_family_is_chosen_but_offers_no_choice() {
    let found = discovery(vec![target(
        "loop:other",
        "Main.OtherPID",
        &["Main.MV"],
        &["Main.PV"],
        ControllerCompatibility::Rejected {
            reason: "No adapter.".to_owned(),
        },
    )]);
    let answer = match_mapping(&found, &request("loop:tank", &["Main.MV"], &["Main.PV"]));
    assert_eq!(answer.outcome, MappingOutcome::Other);
    assert!(answer.selection.is_some());
    assert_eq!(answer.options, [] as [MappingOption; 0]);
}

#[test]
fn a_recording_selection_says_whether_the_selectors_changed_it() {
    let found = discovery(Vec::new());
    let recorded = RecordingSignalSelection {
        inputs: vec!["appliedMv".to_owned()],
        outputs: vec!["pv".to_owned()],
    };
    let mut same = request("loop:tank", &["appliedMv"], &["pv"]);
    same.recording_selection = Some(recorded.clone());
    let answer = match_mapping(&found, &same);
    assert_eq!(answer.outcome, MappingOutcome::Recorded);
    assert!(
        !answer.changed,
        "the selectors show the recording's own selection"
    );
    assert!(
        !answer.blocks_run,
        "a recording's selection is not a loop mapping, so it blocks nothing"
    );

    let mut changed = request("loop:tank", &["appliedMv"], &["appliedMv"]);
    changed.recording_selection = Some(recorded);
    let answer = match_mapping(&found, &changed);
    assert_eq!(answer.outcome, MappingOutcome::Recorded);
    assert!(
        answer.changed,
        "an output that the recording does not hold is a change"
    );
}

#[test]
fn the_choices_of_the_shown_loop_are_the_other_loops_that_bind_its_own_signals_and_nothing_blocks()
{
    // The panel names no mapping: the loop it shows has exactly its own signals, whatever its selectors hold (B1.2, B3.6).
    let found = discovery(vec![
        target(
            "loop:tank",
            "Main.TankController",
            &["Main.MV"],
            &["Main.PV"],
            accepted(&["P", "PI"]),
        ),
        target(
            "loop:matching",
            "Main.MatchingPID",
            &["Main.MV"],
            &["Main.PV"],
            accepted(&["PI", "PID"]),
        ),
        target(
            "loop:elsewhere",
            "Main.ElsewherePID",
            &["Main.OtherMV"],
            &["Main.PV"],
            accepted(&["P"]),
        ),
    ]);
    let answer = own_mapping(&found, "loop:tank").expect("the discovery lists the loop");
    assert_eq!(answer.outcome, MappingOutcome::Current);
    assert_eq!(answer.sentence, "");
    assert!(!answer.blocks_run, "the loop the panel shows can run");
    assert_eq!(
        answer.selection, None,
        "the current loop is not chosen again"
    );
    let labels: Vec<(&str, &str)> = answer
        .options
        .iter()
        .map(|option| (option.loop_id.as_str(), option.label.as_str()))
        .collect();
    assert_eq!(
        labels,
        [
            ("loop:matching", "Main.MatchingPID · PI"),
            ("loop:matching", "Main.MatchingPID · PID"),
        ],
        "a loop with other plant signals is no choice"
    );
}

#[test]
fn a_loop_the_discovery_does_not_list_has_no_mapping_of_its_own() {
    let found = discovery(vec![target(
        "loop:tank",
        "Main.TankController",
        &["Main.MV"],
        &["Main.PV"],
        accepted(&["PI"]),
    )]);
    assert_eq!(own_mapping(&found, "loop:missing"), None);
}
