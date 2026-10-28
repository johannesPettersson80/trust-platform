//! The three-page binding route's rules, ported from `threePageBindingRoute.ts` (specification 35, B1.2, B1.3 and C0): the selection's
//! shape, the family it asks for, and the project that holds the loop. The rules are checked here on values; the message protocol,
//! the replies and the session are not ported (the session's own test is in `control-studio-host-bindings.test.ts`, see
//! `discovery_tests.rs`).

use serde_json::json;

use super::{
    check_family, check_selection, controller_belongs_to_equipment, identify_project,
    is_controller_family, ProjectLoopFiles, CONTROLLER_FAMILIES,
};
use crate::loop_definition::LoopDefinition;
use crate::setup::discovery::{
    BindingSelection, ControllerCompatibility, Discovery, LoopFileRead, LoopFileText,
};

fn selection(inputs: usize, outputs: usize) -> BindingSelection {
    BindingSelection {
        loop_id: "loop:tank".to_string(),
        controller_instance: "Main.TankController".to_string(),
        input_ids: (0..inputs).map(|index| format!("in{index}")).collect(),
        output_ids: (0..outputs).map(|index| format!("out{index}")).collect(),
    }
}

/// A loop of project `project:a` in `runtime:local`, with its controller and its signals.
fn loop_definition() -> LoopDefinition {
    let value = json!({
        "schemaVersion": 2, "id": "loop:tank", "name": "Tank level", "revision": "sha256:loop-1",
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

/// A loop file that declares the given identity, as a project's file does.
fn declaring(id: &str, project: &str, runtime: &str) -> LoopFileText {
    LoopFileText {
        configuration_path: "control-loop.json".to_string(),
        read: LoopFileRead::Text(
            json!({ "id": id, "projectId": project, "runtimeId": runtime }).to_string(),
        ),
    }
}

fn project(root: &str, files: Vec<LoopFileText>) -> ProjectLoopFiles {
    ProjectLoopFiles {
        root: root.to_string(),
        files,
    }
}

#[test]
fn a_selection_names_one_to_sixteen_channels_on_each_side() {
    // threePageBindingRoute.ts's `selection`: `input.length > 0 && input.length <= 16` on both sides, each identity a non-empty text.
    assert!(check_selection(&selection(1, 1), None).is_ok());
    assert!(
        check_selection(&selection(16, 16), None).is_ok(),
        "sixteen is inside the bound"
    );
    assert!(
        check_selection(&selection(0, 1), None).is_err(),
        "no input is refused"
    );
    assert!(
        check_selection(&selection(1, 0), None).is_err(),
        "no output is refused"
    );
    assert!(
        check_selection(&selection(17, 1), None).is_err(),
        "seventeen is outside the bound"
    );
    let mut empty_identity = selection(1, 1);
    empty_identity.input_ids[0] = String::new();
    assert!(
        check_selection(&empty_identity, None).is_err(),
        "an empty identity is refused"
    );
}

#[test]
fn a_chosen_family_must_be_a_controller_family() {
    // threePageBindingRoute.ts: `payload.family !== undefined && !isControllerFamily(payload.family)` is refused with the same sentence.
    assert!(check_selection(&selection(1, 1), Some("PID")).is_ok());
    assert!(
        check_selection(&selection(1, 1), Some("pid")).is_err(),
        "families are named exactly"
    );
    assert!(check_selection(&selection(1, 1), Some("bogus")).is_err());
    let refusal = check_selection(&selection(1, 1), Some("bogus")).expect_err("refused");
    assert_eq!(
        refusal.message,
        "Choose a declared controller and its complete signal mapping."
    );
}

#[test]
fn the_controller_families_are_the_typescript_list() {
    // controllerFamilies.ts: four connected families, then the nineteen project families, in that order.
    assert_eq!(CONTROLLER_FAMILIES.len(), 23);
    assert_eq!(
        &CONTROLLER_FAMILIES[..4],
        ["ON/OFF", "state-feedback", "LQR", "LQG"]
    );
    assert_eq!(CONTROLLER_FAMILIES[4..8], ["P", "PI", "PD", "PID"]);
    assert_eq!(CONTROLLER_FAMILIES[22], "lead-lag");
    assert!(is_controller_family("PID"));
    assert!(!is_controller_family("Pid"));
}

#[test]
fn a_family_is_accepted_only_when_the_selected_loops_adapter_realizes_it() {
    // threePageBindingRoute.ts: "Choose a configuration supported by that controller's declared adapter." (C0).
    let target = crate::setup::discovery::BindingTarget {
        configuration_path: "control-loop.json".to_string(),
        loop_definition: loop_definition(),
        controller: crate::setup::discovery::DiscoveredController {
            instance: "Main.TankController".to_string(),
            type_name: "Control.PID".to_string(),
            tasks: Vec::new(),
        },
        inputs: Vec::new(),
        outputs: Vec::new(),
        controller_compatibility: ControllerCompatibility::Accepted {
            recommended_family: None,
            methods: None,
            method_titles: None,
            families: vec![
                "P".to_string(),
                "PI".to_string(),
                "PD".to_string(),
                "PID".to_string(),
            ],
            family_settings: std::collections::BTreeMap::new(),
        },
    };
    let discovery = Discovery {
        targets: vec![target],
        rejected: Vec::new(),
    };
    let chosen = BindingSelection {
        loop_id: "loop:tank".to_string(),
        controller_instance: "Main.TankController".to_string(),
        input_ids: vec!["manipulatedVariable".to_string()],
        output_ids: vec!["processVariable".to_string()],
    };
    assert!(check_family(&discovery, &chosen, "PI").is_ok());
    let refusal =
        check_family(&discovery, &chosen, "LQR").expect_err("LQR is not realized by this adapter");
    assert_eq!(
        refusal.message,
        "Choose a configuration supported by that controller's declared adapter."
    );
    let mut other = chosen;
    other.loop_id = "loop:other".to_string();
    assert!(
        check_family(&discovery, &other, "PI").is_err(),
        "a loop that is not listed realizes nothing"
    );
}

#[test]
fn the_only_project_is_the_loops_project_whatever_its_files_say() {
    // threePageBindingRoute.ts `project`: a single workspace project is the project.
    let only = [project("/workspace/a", Vec::new())];
    let found = identify_project(&loop_definition(), &only).expect("the only project");
    assert_eq!(found.root, "/workspace/a");
}

#[test]
fn among_several_projects_the_one_whose_loop_files_declare_the_loop_is_chosen() {
    // threePageBindingRoute.ts `project`: exactly one project must hold a file that declares the loop's id, project and runtime.
    let tank = loop_definition();
    let holds = project(
        "/workspace/a",
        vec![declaring("loop:tank", "project:a", "runtime:local")],
    );
    let other = project(
        "/workspace/b",
        vec![declaring("loop:tank", "project:b", "runtime:local")],
    );
    let projects = [other.clone(), holds.clone()];
    assert_eq!(
        identify_project(&tank, &projects)
            .expect("one project holds the loop")
            .root,
        "/workspace/a"
    );

    let both = [
        holds.clone(),
        project(
            "/workspace/c",
            vec![declaring("loop:tank", "project:a", "runtime:local")],
        ),
    ];
    assert!(
        identify_project(&tank, &both).is_err(),
        "two projects declaring the loop are ambiguous"
    );
    let elsewhere = project(
        "/workspace/c",
        vec![declaring("loop:other", "project:a", "runtime:local")],
    );
    let none = [other, elsewhere];
    assert!(
        identify_project(&tank, &none).is_err(),
        "no project declaring the loop is refused"
    );
}

#[test]
fn a_file_that_is_too_large_or_not_json_declares_nothing_for_the_project_choice() {
    // threePageBindingRoute.ts: a file over the bound is skipped, and an unreadable one is left to the discovery.
    let tank = loop_definition();
    let too_large = LoopFileText {
        configuration_path: "control-loops/big.json".to_string(),
        read: LoopFileRead::TooLarge { bytes: 600_000 },
    };
    let broken = LoopFileText {
        configuration_path: "control-loops/broken.json".to_string(),
        read: LoopFileRead::Text("{ invalid".to_string()),
    };
    let projects = [
        project("/workspace/a", vec![too_large, broken]),
        project(
            "/workspace/b",
            vec![declaring("loop:tank", "project:a", "runtime:local")],
        ),
    ];
    assert_eq!(
        identify_project(&tank, &projects)
            .expect("only one project declares the loop")
            .root,
        "/workspace/b"
    );
}

#[test]
fn a_controller_belongs_to_an_equipment_when_it_is_the_equipment_or_one_of_its_members_without_case(
) {
    assert!(controller_belongs_to_equipment(
        "Main.TankPID",
        "Main.TankPID"
    ));
    assert!(controller_belongs_to_equipment(
        "main.tankpid",
        "Main.TankPID"
    ));
    assert!(controller_belongs_to_equipment(
        "Main.TankPID.Inner",
        "Main.TankPID"
    ));
    // A shared prefix is not membership: the equipment's path must end at a dot.
    assert!(!controller_belongs_to_equipment(
        "Main.TankPID2",
        "Main.TankPID"
    ));
    assert!(!controller_belongs_to_equipment(
        "Main.Tank",
        "Main.TankPID"
    ));
    assert!(!controller_belongs_to_equipment("Main", "Main.TankPID"));
}
