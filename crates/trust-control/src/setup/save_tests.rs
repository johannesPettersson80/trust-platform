//! The setup save, ported from `control-studio-loop-setup-save.test.ts` (the save rules) and the save refusals of
//! `loopSetupSave.ts`/`projectLoop.ts` (specification 35, B1.3 and §0.1b). The cascade, ON/OFF and state-feedback saves are parked
//! until stage V7: they are ported as refusals by name. The write assertions (files, `wx`) belong to the engine.

use serde_json::{json, Value};

use super::{
    check_setup_targets, parse_setup_draft, plan_setup_save, validation_reason_code,
    ExistingLoopFile, LoopFileId, PlannedLoop, SetupDraft,
};
use crate::failure::{Cause, Failure, NextAction};
use crate::loop_definition::{LoopRejection, LoopScope};
use crate::setup::catalog::{
    CallLink, CompiledCatalog, CompiledController, CompiledSymbol, CompiledTask,
};

const PARAMETERS: [&str; 11] = [
    "ProportionalGain",
    "IntegralGain",
    "DerivativeGain",
    "ProportionalSetpointWeight",
    "DerivativeSetpointWeight",
    "Bias",
    "OutputMinimum",
    "OutputMaximum",
    "RateMinimum",
    "RateMaximum",
    "TrackingTime",
];

const STATES: [&str; 7] = [
    "Setpoint",
    "SetpointRate",
    "ProcessValue",
    "ProcessValueRate",
    "FeedForward",
    "NominalOutput",
    "SaturatedOutput",
];

fn scope() -> LoopScope {
    LoopScope {
        project_id: "project:save".to_string(),
        runtime_id: "runtime-1".to_string(),
    }
}

fn symbol(name: &str, iec_type: &str, address: Option<&str>) -> CompiledSymbol {
    CompiledSymbol {
        symbol: name.to_string(),
        iec_type: iec_type.to_string(),
        address: address.map(str::to_string),
    }
}

fn controller(instance: &str, type_name: &str) -> CompiledController {
    CompiledController {
        instance: instance.to_string(),
        type_name: type_name.to_string(),
        tasks: vec![CompiledTask {
            name: "MainTask".to_string(),
            interval_ms: 100.0,
        }],
    }
}

/// The cascade program of `control-studio-loop-setup-save.test.ts`: an outer and an inner PID, each with its record and states.
fn save_catalog() -> CompiledCatalog {
    let mut symbols = vec![
        symbol("P1.VesselC", "LREAL", Some("%IL0")),
        symbol("P1.JacketC", "LREAL", Some("%IL8")),
        symbol("P1.VesselSetpointC", "LREAL", Some("%ML0")),
        symbol("P1.JacketSetpointC", "LREAL", Some("%ML8")),
        symbol("P1.HeaterPct", "LREAL", Some("%QL0")),
        symbol("P1.OuterAuto", "BOOL", None),
        symbol("P1.InnerAuto", "BOOL", None),
        symbol("P1.OuterManual", "LREAL", Some("%ML16")),
        symbol("P1.InnerManual", "LREAL", Some("%ML24")),
    ];
    for name in ["Outer", "Inner"] {
        for field in PARAMETERS {
            symbols.push(symbol(
                &format!("P1.{name}Parameters.{field}"),
                "LREAL",
                None,
            ));
        }
        for field in STATES {
            symbols.push(symbol(
                &format!("P1.{name}.StateValue.{field}"),
                "LREAL",
                None,
            ));
        }
    }
    CompiledCatalog {
        symbols,
        controllers: vec![
            controller("P1", "Main"),
            controller("P1.Inner", "Control.PID"),
            controller("P1.Outer", "Control.PID"),
        ],
        links: vec![
            CallLink {
                instance: "P1.Outer".to_string(),
                method: "Update".to_string(),
                argument: "Input".to_string(),
                record: "P1.OuterInput".to_string(),
                output: Some("P1.JacketSetpointC".to_string()),
                fields: [(
                    "Setpoint".to_string(),
                    vec!["P1.VesselSetpointC".to_string()],
                )]
                .into_iter()
                .collect(),
                guarded_by: None,
                mode: None,
                copies_of_output: Vec::new(),
            },
            CallLink {
                instance: "P1.Inner".to_string(),
                method: "Update".to_string(),
                argument: "Input".to_string(),
                record: "P1.InnerInput".to_string(),
                output: Some("P1.HeaterPct".to_string()),
                fields: [(
                    "Setpoint".to_string(),
                    vec!["P1.JacketSetpointC".to_string()],
                )]
                .into_iter()
                .collect(),
                guarded_by: None,
                mode: None,
                copies_of_output: Vec::new(),
            },
        ],
        writes: Vec::new(),
    }
}

/// The text of a failure, whole: its detail when it says more than its message.
fn full_text(failure: &Failure) -> String {
    failure
        .detail
        .clone()
        .unwrap_or_else(|| failure.message.clone())
}

fn identify_draft() -> Value {
    json!({
        "structure": "identify",
        "name": "Jacket response",
        "manipulatedVariable": { "symbol": "P1.HeaterPct", "unit": "%", "minimum": 0, "maximum": 100 },
        "processVariable": { "symbol": "P1.JacketC", "unit": "degC", "minimum": 0, "maximum": 90 },
        "experimentDurationSeconds": 120,
    })
}

#[test]
fn an_identify_only_draft_plans_one_open_loop_on_the_chosen_input_and_output() {
    // control-studio-loop-setup-save.test.ts: "writes an identify-only loop that records the chosen plant input and output".
    let draft = parse_setup_draft(&identify_draft()).expect("the identify draft decodes");
    let planned =
        plan_setup_save(&save_catalog(), &scope(), &draft).expect("the identify loop is planned");
    assert_eq!(planned.len(), 1, "one identify loop");
    let loop_definition = &planned[0].loop_definition;
    assert_eq!(
        planned[0].file_name, "control-loops/jacket-response.json",
        "the file is named from the loop id"
    );
    assert_eq!(
        (
            loop_definition.topology,
            loop_definition.controller.instance.as_str(),
            loop_definition.controller.type_name.as_str(),
            loop_definition.signals.setpoint.is_none(),
        ),
        (
            Some(crate::loop_definition::Topology::OpenLoop),
            "P1",
            "Main",
            true,
        ),
        "an open loop with no setpoint, on the program's own controller instance"
    );
    assert_eq!(
        (
            loop_definition.signals.process_variable.address.as_deref(),
            loop_definition
                .signals
                .manipulated_variable
                .address
                .as_deref(),
        ),
        (Some("%IL8"), Some("%QL0")),
        "the measurement and the output keep their addresses"
    );
    assert_eq!(
        loop_definition.project_id, "project:save",
        "the scope gives the project"
    );
    assert_eq!(
        loop_definition.runtime_id, "runtime-1",
        "the scope gives the runtime"
    );
}

#[test]
fn a_cascade_draft_is_refused_by_name_until_the_rust_engine_runs_cascades() {
    // control-studio-loop-setup-save.test.ts: "writes a cascade as one loop file..." is parked (stage V7, C6): the save refuses it.
    let draft = parse_setup_draft(&json!({ "structure": "cascade", "name": "Vessel temperature" }))
        .expect("the structure is known");
    assert!(matches!(&draft, SetupDraft::Parked { structure } if structure == "cascade"));
    let refusal = plan_setup_save(&save_catalog(), &scope(), &draft)
        .expect_err("a parked structure is refused");
    assert_eq!(
        full_text(&refusal),
        "A cascade is not yet supported by the Rust engine."
    );
    assert_eq!(refusal.cause, Cause::ControllerCapability);
    assert_eq!(refusal.next_action, NextAction::None);
}

#[test]
fn the_on_off_and_state_feedback_drafts_are_refused_by_name() {
    // control-studio-loop-setup-save.test.ts: "writes an ON/OFF loop ..." and "writes a state-feedback loop ..." are parked (V7, C3, C4).
    for (structure, label) in [
        ("onoff", "An ON/OFF controller"),
        ("state-feedback", "State feedback"),
    ] {
        let draft = parse_setup_draft(&json!({ "structure": structure, "name": "Cold room" }))
            .expect("the structure is known");
        let refusal = plan_setup_save(&save_catalog(), &scope(), &draft)
            .expect_err("a parked structure is refused");
        assert_eq!(
            full_text(&refusal),
            format!("{label} is not yet supported by the Rust engine.")
        );
    }
}

#[test]
fn a_draft_without_a_structure_is_a_pid_loop_and_an_unknown_structure_is_refused() {
    // loopSetupSave.ts: a draft without a structure is the PID's form; §0.1b: an unknown structure is never guessed.
    let pid = parse_setup_draft(&json!({
        "name": "Heater temperature", "instance": "P1.Controller", "action": "direct", "parameterRecord": "P1.Parameters",
        "processVariable": { "symbol": "P1.TemperatureC", "unit": "degC", "minimum": 0, "maximum": 80 },
        "manipulatedVariable": { "symbol": "P1.AppliedPowerPct", "unit": "%", "minimum": 0, "maximum": 100 },
        "setpoint": { "symbol": "P1.SetpointC" }, "mode": { "symbol": "P1.AutoMode", "whenTrue": "Auto" },
        "manualOutput": "P1.ManualOutput",
    }))
    .expect("a PID draft decodes");
    assert!(
        matches!(pid, SetupDraft::Pid(_)),
        "no structure means a PID loop"
    );
    let unknown = parse_setup_draft(&json!({ "structure": "bogus" }))
        .expect_err("an unknown structure is refused");
    assert_eq!(
        full_text(&unknown),
        "bogus is not a loop setup structure this version knows."
    );
    assert_eq!(unknown.cause, Cause::Implementation);
    assert_eq!(unknown.next_action, NextAction::OpenLoopSetup);
}

#[test]
fn a_draft_with_a_field_the_structure_does_not_declare_is_refused_naming_the_field() {
    // §0.1b and A11.1: a field the form does not send is refused with its name, never ignored.
    let refusal =
        parse_setup_draft(&json!({ "structure": "identify", "name": "Jacket", "bogus": 1 }))
            .expect_err("an extra field is refused");
    assert!(
        full_text(&refusal).contains("bogus"),
        "the sentence names the field: {}",
        full_text(&refusal)
    );
    assert!(
        full_text(&refusal).starts_with("The identify setup draft is not complete"),
        "{}",
        full_text(&refusal)
    );
}

#[test]
fn a_draft_that_is_not_an_object_or_has_no_text_structure_is_refused() {
    assert_eq!(
        full_text(&parse_setup_draft(&json!([1, 2])).expect_err("an array is not a draft")),
        "The setup draft must be a JSON object."
    );
    assert_eq!(
        full_text(
            &parse_setup_draft(&json!({ "structure": 3 }))
                .expect_err("a number is not a structure")
        ),
        "The setup draft's structure must be a text."
    );
}

#[test]
fn an_existing_loop_file_of_the_same_name_is_never_overwritten() {
    // control-studio-loop-setup-save.test.ts: "an existing loop file is never overwritten" (`/already exists/`).
    let draft = parse_setup_draft(&identify_draft()).expect("the identify draft decodes");
    let planned =
        plan_setup_save(&save_catalog(), &scope(), &draft).expect("the identify loop is planned");
    let existing = [ExistingLoopFile {
        file_name: "control-loops/jacket-response.json".to_string(),
        id: LoopFileId::Id("loop:jacket-response".to_string()),
    }];
    let refusal =
        check_setup_targets(&existing, &planned).expect_err("an existing file is refused");
    assert_eq!(
        full_text(&refusal),
        "A loop named Jacket response already exists in this project."
    );
    assert_eq!(refusal.next_action, NextAction::OpenLoopSetup);
}

#[test]
fn a_planned_id_that_a_root_or_folder_file_declares_is_refused_naming_that_file() {
    // projectLoop.ts assertControlLoopIdAvailable: "Duplicate control loop ID … in <file>. Choose a different loop name."
    let planned = plan_setup_save(
        &save_catalog(),
        &scope(),
        &parse_setup_draft(&identify_draft()).expect("the identify draft decodes"),
    )
    .expect("the identify loop is planned");
    let root = [ExistingLoopFile {
        file_name: "control-loop.json".to_string(),
        id: LoopFileId::Id("loop:jacket-response".to_string()),
    }];
    let refusal = check_setup_targets(&root, &planned).expect_err("a duplicate id is refused");
    assert_eq!(
        full_text(&refusal),
        "Duplicate control loop ID loop:jacket-response in control-loop.json. Choose a different loop name."
    );
}

#[test]
fn an_unreadable_loop_file_stops_the_save_and_names_its_repair() {
    // projectLoop.ts: "Repair <file> before adding a control loop: its JSON is invalid." (the engine reports invalid JSON and an
    // unreadable file alike as Unreadable).
    let planned = plan_setup_save(
        &save_catalog(),
        &scope(),
        &parse_setup_draft(&identify_draft()).expect("the identify draft decodes"),
    )
    .expect("the identify loop is planned");
    let existing = [ExistingLoopFile {
        file_name: "control-loops/broken.json".to_string(),
        id: LoopFileId::Unreadable,
    }];
    let refusal =
        check_setup_targets(&existing, &planned).expect_err("an unreadable file is refused");
    assert_eq!(
        full_text(&refusal),
        "Repair broken.json before adding a control loop: its JSON is invalid or it cannot be read."
    );
}

#[test]
fn a_file_without_an_id_or_a_root_file_named_like_the_slug_does_not_block_the_save() {
    // The file names compare as project-relative paths, so `jacket-response.json` at the root never collides with
    // `control-loops/jacket-response.json`, and a file without an id declares no loop.
    let planned = plan_setup_save(
        &save_catalog(),
        &scope(),
        &parse_setup_draft(&identify_draft()).expect("the identify draft decodes"),
    )
    .expect("the identify loop is planned");
    let existing = [
        ExistingLoopFile {
            file_name: "jacket-response.json".to_string(),
            id: LoopFileId::NoId,
        },
        ExistingLoopFile {
            file_name: "control-loops/other.json".to_string(),
            id: LoopFileId::Id("loop:other".to_string()),
        },
    ];
    check_setup_targets(&existing, &planned).expect("nothing collides");
}

#[test]
fn the_refusals_follow_the_order_of_the_planned_loops_and_the_first_one_is_reported() {
    // All or none: the check decides every planned loop before anything is written, and the first refusal is the one reported.
    let mut second = plan_setup_save(
        &save_catalog(),
        &scope(),
        &parse_setup_draft(&identify_draft()).expect("the identify draft decodes"),
    )
    .expect("the identify loop is planned")
    .remove(0);
    second.file_name = "control-loops/second.json".to_string();
    second.loop_definition.id = "loop:second".to_string();
    second.loop_definition.name = "Second".to_string();
    let first = PlannedLoop {
        file_name: "control-loops/first.json".to_string(),
        loop_definition: second.loop_definition.clone(),
    };
    let existing = [ExistingLoopFile {
        file_name: "control-loops/second.json".to_string(),
        id: LoopFileId::NoId,
    }];
    let refusal = check_setup_targets(&existing, &[first, second])
        .expect_err("the second planned file exists");
    assert_eq!(
        full_text(&refusal),
        "A loop named Second already exists in this project."
    );
}

#[test]
fn the_rejection_names_map_to_the_rules_of_the_typescript_discovery() {
    // loopSetupSave.ts and hostBindingDiscovery.ts: each rule's name, shown in words, as the sentence reads.
    assert_eq!(
        validation_reason_code(&LoopRejection::IncompatibleUnits),
        "incompatible_units"
    );
    assert_eq!(
        validation_reason_code(&LoopRejection::AmbiguousSignalAddress(
            "a".into(),
            "b".into()
        )),
        "ambiguous_signal_address"
    );
    assert_eq!(
        validation_reason_code(&LoopRejection::CrossRuntime),
        "cross_runtime_loop"
    );
    assert_eq!(
        validation_reason_code(&LoopRejection::UnsupportedVersion(1)),
        "unsupported_version"
    );
}
