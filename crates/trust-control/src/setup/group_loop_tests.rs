//! Tests of the PID group generator (`pidGroupGenerator.ts`): the setup assertions of `control-studio-loop-setup-group.test.ts` and of
//! `control-studio-decoupler.test.ts` on their two-zone program, the refusals of the `TypeScript`, and the save's reading of a group. The
//! Tune prediction, the identification channels and the form of those files are other stages, and are not ported here.

#![allow(clippy::float_cmp)]

use serde_json::{json, Value};

use super::{group_loop_definition, GroupLoopDraft, GroupMemberDraft};
use crate::failure::Failure;
use crate::identification::connected_run::input_steps;
use crate::json::decode_value;
use crate::loop_definition::{
    Action, Design, Execution, ExperimentKind, InitialSource, InitialValue, LoopDefinition,
    LoopScope, ModeWhenTrue, ProjectSeedBlock,
};
use crate::setup::catalog::{decode_catalog_answer, CompiledCatalog, PID_PARAMETERS};
use crate::setup::loop_bindings::FormSignal;
use crate::setup::pid_loop::{ModeChoice, SignalChoice};
use crate::setup::save::{parse_setup_draft, plan_setup_save, SetupDraft};
use crate::setup::texts::configure::not_compiled_bool;
use crate::setup::texts::generators::{program_initializes, program_writes, startup_flag_needed};
use crate::setup::texts::save::parked_label;

/// The seven retained states of each `Control.PID` of the fixture (`STATE_FIELDS` of the `TypeScript` test).
const STATE_FIELDS: [&str; 7] = [
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
        project_id: "project:setup".to_string(),
        runtime_id: "runtime-1".to_string(),
    }
}

/// The sentence a refusal shows: its whole text, which is the detail when it says more than its first sentence.
fn text_of(failure: &Failure) -> String {
    failure
        .detail
        .clone()
        .unwrap_or_else(|| failure.message.clone())
}

fn signal(symbol: &str, unit: &str, minimum: f64, maximum: f64) -> FormSignal {
    FormSignal {
        symbol: symbol.to_string(),
        unit: unit.to_string(),
        minimum: Some(minimum),
        maximum: Some(maximum),
    }
}

/// The symbols of one `Control.PID`: its record fields and its seven retained states (`pidSymbols`).
fn pid_symbols(instance: &str, record: &str) -> Vec<Value> {
    PID_PARAMETERS
        .iter()
        .map(|field| json!({ "symbol": format!("{record}.{field}"), "type": "LREAL" }))
        .chain(STATE_FIELDS.iter().map(
            |field| json!({ "symbol": format!("{instance}.StateValue.{field}"), "type": "LREAL" }),
        ))
        .collect()
}

/// One zone: its measurement, its heater output, its setpoint, its manual output and its PID's symbols (`zone` of the fixture).
fn zone(number: usize) -> Vec<Value> {
    let offset = 8 * (number - 1);
    let mut symbols = vec![
        json!({ "symbol": format!("P1.Zone{number}C"), "type": "LREAL", "address": format!("%IL{offset}") }),
        json!({ "symbol": format!("P1.Heater{number}Pct"), "type": "LREAL", "address": format!("%QL{offset}") }),
        json!({ "symbol": format!("P1.Zone{number}SetpointC"), "type": "LREAL", "address": format!("%ML{offset}") }),
        json!({ "symbol": format!("P1.Zone{number}ManualPct"), "type": "LREAL", "address": format!("%ML{}", 16 + offset) }),
    ];
    symbols.extend(pid_symbols(
        &format!("P1.Zone{number}"),
        &format!("P1.Zone{number}Parameters"),
    ));
    symbols
}

/// The links of the fixture: each zone's parameter record, and its update, which names its heater, its measurement, its setpoint and its
/// manual output.
fn links() -> Value {
    links_with_outputs(|number| format!("P1.Heater{number}Pct"))
}

/// The links of the fixture, with each zone's `Update` output named by `output`.
fn links_with_outputs(output: impl Fn(usize) -> String) -> Value {
    let mut entries = Vec::new();
    for number in 1..=2_usize {
        entries.push(json!({
            "instance": format!("P1.Zone{number}"), "method": "Configure", "argument": "Parameters",
            "record": format!("P1.Zone{number}Parameters"), "output": null, "fields": {}
        }));
        entries.push(json!({
            "instance": format!("P1.Zone{number}"), "method": "Update", "argument": "Input",
            "record": format!("P1.Zone{number}Input"), "output": output(number),
            "fields": {
                "ManualOutput": [format!("P1.Zone{number}ManualPct")],
                "ProcessValue": [format!("P1.Zone{number}C")],
                "Setpoint": [format!("P1.Zone{number}SetpointC")]
            }
        }));
    }
    Value::Array(entries)
}

/// The compiled program of the two zones: a library PID per zone, the first in `MainTask` and the second in `second_task`, and the
/// signals the two share (`zones` of `control-studio-loop-setup-group.test.ts`).
fn catalog_with_second_task(second_task: &str) -> CompiledCatalog {
    compiled_program(second_task, &links())
}

/// The compiled program of [`catalog_with_second_task`], with the call links that `links` names.
fn compiled_program(second_task: &str, links: &Value) -> CompiledCatalog {
    let mut symbols = zone(1);
    symbols.extend(zone(2));
    symbols.extend([
        json!({ "symbol": "P1.ZonesAuto", "type": "BOOL" }),
        json!({ "symbol": "P1.Initialized", "type": "BOOL" }),
        json!({ "symbol": "P1.D11", "type": "LREAL" }),
        json!({ "symbol": "P1.D12", "type": "LREAL" }),
        json!({ "symbol": "P1.D21", "type": "LREAL" }),
        json!({ "symbol": "P1.D22", "type": "LREAL" }),
    ]);
    let control = json!({
        "symbols": symbols,
        "controllers": [
            { "instance": "P1", "type": "Main", "tasks": [{ "name": "MainTask", "intervalMs": 100 }] },
            { "instance": "P1.Zone1", "type": "Control.PID", "tasks": [{ "name": "MainTask", "intervalMs": 100 }] },
            { "instance": "P1.Zone2", "type": "Control.PID", "tasks": [{ "name": second_task, "intervalMs": 100 }] }
        ],
        "links": links,
        "writes": ["P1.Heater1Pct", "P1.Heater2Pct", "P1.Initialized"]
    });
    decode_catalog_answer(&json!({ "ok": true, "processImage": {}, "controlSymbols": control }))
        .expect("the compiled answer decodes")
        .control
}

fn catalog() -> CompiledCatalog {
    catalog_with_second_task("MainTask")
}

/// The compiled program whose controllers write their outputs to `P1.Request<n>Pct`, so that behind a decoupler the program writes the
/// plant inputs `P1.Heater<n>Pct` from `D·v`, as `control-studio-decoupler.test.ts` does.
fn controlled_catalog() -> CompiledCatalog {
    compiled_program(
        "MainTask",
        &links_with_outputs(|number| format!("P1.Request{number}Pct")),
    )
}

/// A zone's member as the `TypeScript` test drafts it: a 20 to 60 °C measurement, a 0 to 100 % output, its setpoint and its manual output.
fn member(number: usize) -> GroupMemberDraft {
    GroupMemberDraft {
        instance: format!("P1.Zone{number}"),
        parameter_record: format!("P1.Zone{number}Parameters"),
        process_variable: signal(&format!("P1.Zone{number}C"), "degC", 20.0, 60.0),
        manipulated_variable: signal(&format!("P1.Heater{number}Pct"), "%", 0.0, 100.0),
        setpoint: SignalChoice {
            symbol: format!("P1.Zone{number}SetpointC"),
            unit: None,
            minimum: None,
            maximum: None,
        },
        manual_output: format!("P1.Zone{number}ManualPct"),
        experiment_step: None,
    }
}

/// The group draft of `control-studio-loop-setup-group.test.ts`: a group step of 10 for 40 s, and the startup flag.
fn draft() -> GroupLoopDraft {
    GroupLoopDraft {
        name: "Zone temperatures".to_string(),
        action: Action::Direct,
        mode: ModeChoice {
            symbol: "P1.ZonesAuto".to_string(),
            when_true: ModeWhenTrue::Auto,
        },
        experiment_step: Some(10.0),
        experiment_duration_seconds: Some(40.0),
        startup_flag: Some("P1.Initialized".to_string()),
        members: vec![member(1), member(2)],
        decoupler: None,
    }
}

/// The decoupler of `control-studio-decoupler.test.ts`: `D[i][j]` for plant input `i` and member `j`.
fn decoupler() -> Vec<Vec<String>> {
    vec![
        vec!["P1.D11".to_string(), "P1.D12".to_string()],
        vec!["P1.D21".to_string(), "P1.D22".to_string()],
    ]
}

fn generate(draft: &GroupLoopDraft) -> LoopDefinition {
    group_loop_definition(&catalog(), &scope(), draft).expect("the group is generated")
}

/// The group as the `TypeScript` form posts it (`PidGroupDraft`, without the project and the runtime, which the save removes).
fn draft_json() -> Value {
    json!({
        "name": "Zone temperatures",
        "action": "direct",
        "mode": { "symbol": "P1.ZonesAuto", "whenTrue": "Auto" },
        "experimentStep": 10,
        "experimentDurationSeconds": 40,
        "startupFlag": "P1.Initialized",
        "members": [
            {
                "instance": "P1.Zone1", "parameterRecord": "P1.Zone1Parameters",
                "processVariable": { "symbol": "P1.Zone1C", "unit": "degC", "minimum": 20, "maximum": 60 },
                "manipulatedVariable": { "symbol": "P1.Heater1Pct", "unit": "%", "minimum": 0, "maximum": 100 },
                "setpoint": { "symbol": "P1.Zone1SetpointC" }, "manualOutput": "P1.Zone1ManualPct"
            },
            {
                "instance": "P1.Zone2", "parameterRecord": "P1.Zone2Parameters",
                "processVariable": { "symbol": "P1.Zone2C", "unit": "degC", "minimum": 20, "maximum": 60 },
                "manipulatedVariable": { "symbol": "P1.Heater2Pct", "unit": "%", "minimum": 0, "maximum": 100 },
                "setpoint": { "symbol": "P1.Zone2SetpointC" }, "manualOutput": "P1.Zone2ManualPct",
                "experimentStep": 5
            }
        ],
        "decoupler": [["P1.D11", "P1.D12"], ["P1.D21", "P1.D22"]]
    })
}

#[test]
fn the_two_member_group_is_written_read_back_and_accepted() {
    // control-studio-loop-setup-group.test.ts: "writes one group loop whose members are tuned together from the group model".
    let loop_definition = generate(&draft());
    let text = serde_json::to_string(&loop_definition).expect("the loop serializes");
    let decoded = LoopDefinition::decode(&text).expect("the loop file is read");
    decoded.validate(&scope()).expect("the loop is accepted");
    assert_eq!(decoded.id, "loop:zone-temperatures");
    assert_eq!(decoded.controller.design, Some(Design::Project));
    assert_eq!(decoded.controller.instance, "P1.Zone1");
    assert!(
        decoded.controller.pid_initialization.is_none(),
        "the group's states are in its project block, not a library PID's initialization"
    );
}

#[test]
fn the_members_are_named_on_their_channels_and_share_the_mode() {
    let loop_definition = generate(&draft());
    let group = loop_definition
        .controller
        .group
        .as_ref()
        .expect("the loop declares its group");
    let members = group
        .members
        .iter()
        .map(|member| {
            (
                member.instance.as_str(),
                member.input_channel_id.as_str(),
                member.output_channel_id.as_str(),
                member.mode_symbol.as_str(),
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(
        members,
        [
            ("P1.Zone1", "P1.Heater1Pct", "P1.Zone1C", "P1.ZonesAuto"),
            ("P1.Zone2", "P1.Heater2Pct", "P1.Zone2C", "P1.ZonesAuto"),
        ]
    );
    let owned = |index: usize| {
        group.members[index]
            .application_state_symbols
            .clone()
            .unwrap_or_default()
    };
    assert_eq!(
        owned(0).len(),
        8,
        "seven retained states and the startup flag"
    );
    assert_eq!(owned(0).last().map(String::as_str), Some("P1.Initialized"));
    assert_eq!(
        owned(1).len(),
        7,
        "the second member owns its seven states only"
    );
}

#[test]
fn each_member_is_designed_from_the_group_model_with_no_declared_grid() {
    // control-studio-loop-setup-group.test.ts: the decentralized seed, and no tuning axes.
    let loop_definition = generate(&draft());
    let project = loop_definition
        .controller
        .project
        .as_ref()
        .expect("the law is the program's own");
    assert_eq!(project.family, "paired-PI");
    assert_eq!(project.seed, Some(ProjectSeedBlock::Decentralized));
    assert!(
        project.axes.is_empty(),
        "each member is designed from the group model (Specification 35 §17.9), not from a declared grid"
    );
}

#[test]
fn only_the_gains_are_bound_and_each_member_brings_its_own() {
    let loop_definition = generate(&draft());
    let binding = loop_definition
        .parameter_binding
        .as_ref()
        .expect("the loop binds its records");
    let symbols = binding
        .parameters
        .iter()
        .map(|parameter| parameter.symbol.as_str())
        .collect::<Vec<_>>();
    assert_eq!(
        symbols,
        [
            "P1.Zone1Parameters.ProportionalGain",
            "P1.Zone1Parameters.IntegralGain",
            "P1.Zone1Parameters.DerivativeGain",
            "P1.Zone2Parameters.ProportionalGain",
            "P1.Zone2Parameters.IntegralGain",
            "P1.Zone2Parameters.DerivativeGain",
        ],
        "only the gains are bound, so validation stays within its recorded channels"
    );
    assert_eq!(binding.mode.symbol, "P1.ZonesAuto");
    assert!(!binding.mode.manual_value, "TRUE means Auto");
}

#[test]
fn each_member_starts_with_its_seven_states_and_the_startup_flag_follows_them() {
    let loop_definition = generate(&draft());
    let project = loop_definition
        .controller
        .project
        .as_ref()
        .expect("the law is the program's own");
    let states = project.initialization.states();
    assert_eq!(
        states.len(),
        15,
        "seven retained states per member and the startup flag"
    );
    assert_eq!(states[7].symbol, "P1.Zone2.StateValue.Setpoint");
    assert_eq!(states[14].symbol, "P1.Initialized");
}

#[test]
fn the_second_member_steps_its_own_manual_output_and_the_group_step_sets_the_experiment() {
    // control-studio-loop-setup-group.test.ts: the manual output of member 2, an amplitude of 10 and a duration of 40.
    let loop_definition = generate(&draft());
    let channels = loop_definition
        .channels
        .as_ref()
        .expect("the loop has its channels");
    assert_eq!(
        channels.inputs[1].manual_output_signal.as_deref(),
        Some("P1.Zone2ManualPct")
    );
    let experiment = loop_definition
        .experiment
        .as_ref()
        .expect("the group step sets the experiment");
    assert_eq!(
        (experiment.amplitude, experiment.duration_seconds),
        (10.0, Some(40.0))
    );
}

#[test]
fn the_first_members_step_never_sets_the_loop_experiment() {
    // Decision D-15: the loop's experiment step is the group's only. A member's own step is that member's channel step.
    let mut changed = draft();
    changed.members[0].experiment_step = Some(5.0);
    let loop_definition = generate(&changed);
    let experiment = loop_definition
        .experiment
        .as_ref()
        .expect("the group step sets the experiment");
    assert_eq!(
        experiment.amplitude, 10.0,
        "the group step, not the first member's"
    );
    let channels = loop_definition
        .channels
        .as_ref()
        .expect("the loop has its channels");
    let steps: Vec<Option<f64>> = channels
        .inputs
        .iter()
        .map(|input| input.experiment_step)
        .collect();
    assert_eq!(
        steps,
        [Some(5.0), Some(10.0)],
        "a member's own step, else the group's step"
    );
}

#[test]
fn per_member_steps_are_each_inputs_step_and_the_loop_asks_for_the_automatic_request() {
    // Specification 35 §17.9: a member's own step is its input's step; the loop's experiment is the group's alone (B3.4).
    let mut changed = draft();
    changed.experiment_step = None;
    changed.experiment_duration_seconds = None;
    changed.members[0].experiment_step = Some(10.0);
    changed.members[1].experiment_step = Some(5.0);
    let loop_definition = generate(&changed);
    let channels = loop_definition
        .channels
        .as_ref()
        .expect("the loop has its channels");
    let steps: Vec<Option<f64>> = channels
        .inputs
        .iter()
        .map(|input| input.experiment_step)
        .collect();
    assert_eq!(steps, [Some(10.0), Some(5.0)], "Specification 35 §17.9");
    let experiment = loop_definition
        .experiment
        .as_ref()
        .expect("a member's step asks for the automatic request");
    assert_eq!(
        (
            experiment.kind,
            experiment.amplitude,
            experiment.duration_seconds
        ),
        (ExperimentKind::Observation, 0.0, None),
        "without a group step the loop is the automatic request (B3.4)"
    );
    let text = serde_json::to_string(&loop_definition).expect("the loop serializes");
    LoopDefinition::decode(&text).expect("the loop file is read");
}

#[test]
fn a_group_of_one_member_is_refused() {
    // control-studio-loop-setup-group.test.ts: "refuses members that do not belong to one compiled program", the first case.
    let mut changed = draft();
    changed.members.truncate(1);
    let failure =
        group_loop_definition(&catalog(), &scope(), &changed).expect_err("one member is refused");
    assert!(
        text_of(&failure).contains("at least two"),
        "{}",
        text_of(&failure)
    );
}

#[test]
fn a_controller_named_twice_is_refused() {
    // control-studio-loop-setup-group.test.ts: the second case, a controller chosen for both members.
    let mut changed = draft();
    changed.members[1].instance = "P1.Zone1".to_string();
    let failure = group_loop_definition(&catalog(), &scope(), &changed)
        .expect_err("a repeated member is refused");
    assert!(text_of(&failure).contains("once"), "{}", text_of(&failure));
}

#[test]
fn a_member_that_is_not_a_control_pid_is_refused() {
    let mut changed = draft();
    changed.members[1].instance = "P1".to_string();
    let failure = group_loop_definition(&catalog(), &scope(), &changed)
        .expect_err("a member that is not a PID is refused");
    assert!(
        text_of(&failure).contains("is not a compiled Control.PID instance"),
        "{}",
        text_of(&failure)
    );
}

#[test]
fn members_in_different_tasks_are_refused() {
    let failure = group_loop_definition(&catalog_with_second_task("FastTask"), &scope(), &draft())
        .expect_err("members in two tasks are refused");
    assert!(
        text_of(&failure).contains("same task"),
        "{}",
        text_of(&failure)
    );
}

#[test]
fn a_decoupled_group_starts_each_pid_at_its_share_of_the_plant_inputs() {
    // control-studio-decoupler.test.ts: the decoupled seed, the units of the decoupler, and the initial sources of the outputs.
    let mut changed = draft();
    changed.experiment_step = None;
    changed.experiment_duration_seconds = None;
    changed.startup_flag = None;
    changed.decoupler = Some(decoupler());
    let loop_definition = group_loop_definition(&controlled_catalog(), &scope(), &changed)
        .expect("the decoupled group is generated");
    let text = serde_json::to_string(&loop_definition).expect("the loop serializes");
    LoopDefinition::decode(&text).expect("the loop file is read");
    let project = loop_definition
        .controller
        .project
        .as_ref()
        .expect("the law is the program's own");
    assert_eq!(project.family, "decoupled-PI");
    match &project.seed {
        Some(ProjectSeedBlock::Decoupled(seed)) => assert_eq!(seed.matrix_symbols, decoupler()),
        other => panic!("the seed names the decoupler, not {other:?}"),
    }
    let states = project.initialization.states();
    let initial = |symbol: &str| {
        states
            .iter()
            .find(|state| state.symbol == symbol)
            .map(|state| state.initial.clone())
    };
    assert_eq!(
        initial("P1.Zone2.StateValue.NominalOutput"),
        Some(InitialValue::From(InitialSource::DecoupledOutput {
            index: 1,
            matrix_symbols: decoupler(),
        }))
    );
    assert_eq!(
        initial("P1.Zone1.StateValue.SaturatedOutput"),
        Some(InitialValue::From(InitialSource::DecoupledOutput {
            index: 0,
            matrix_symbols: decoupler(),
        }))
    );
    let binding = loop_definition
        .parameter_binding
        .as_ref()
        .expect("the loop binds its records");
    let decoupler_units = binding
        .parameters
        .iter()
        .skip(6)
        .map(|parameter| (parameter.symbol.as_str(), parameter.unit.as_str()))
        .collect::<Vec<_>>();
    assert_eq!(
        decoupler_units,
        [
            ("P1.D11", "1"),
            ("P1.D12", "1"),
            ("P1.D21", "1"),
            ("P1.D22", "1")
        ]
    );
}

#[test]
fn a_decoupler_that_is_not_square_or_names_a_variable_twice_is_refused() {
    let shapes = [
        vec![vec!["P1.D11".to_string()]],
        vec![
            vec!["P1.D11".to_string(), "P1.D11".to_string()],
            vec!["P1.D21".to_string(), "P1.D22".to_string()],
        ],
    ];
    for shape in shapes {
        let mut changed = draft();
        changed.decoupler = Some(shape);
        let failure = group_loop_definition(&catalog(), &scope(), &changed)
            .expect_err("the decoupler is refused");
        assert!(
            text_of(&failure).contains("one variable per plant input and member"),
            "{}",
            text_of(&failure)
        );
    }
}

#[test]
fn the_draft_reads_the_typescript_field_names() {
    let value = draft_json();
    let read: GroupLoopDraft = decode_value(value.clone()).expect("the draft decodes");
    assert_eq!(read.experiment_step, Some(10.0));
    assert_eq!(read.members[1].experiment_step, Some(5.0));
    assert_eq!(read.decoupler, Some(decoupler()));
    let mut unknown = value;
    unknown["extra"] = json!(1);
    assert!(
        decode_value::<GroupLoopDraft>(unknown).is_err(),
        "a field the draft does not declare is refused"
    );
}

#[test]
fn the_save_reads_the_group_structure_and_plans_one_loop_file() {
    assert!(
        parked_label("group").is_none(),
        "a PID group is no longer parked"
    );
    let mut value = draft_json();
    value["structure"] = json!("group");
    let parsed = parse_setup_draft(&value).expect("the group structure is read");
    assert!(
        matches!(parsed, SetupDraft::Group(_)),
        "the structure is a group"
    );
    // The draft's decoupler writes the plant inputs from D·v, so the program's controllers write their own outputs elsewhere: the
    // decoupled fixture (`controlled_catalog`), not the paired one whose outputs are the plant inputs (C7, specification 35 §17.9).
    let planned =
        plan_setup_save(&controlled_catalog(), &scope(), &parsed).expect("the group is planned");
    let files: Vec<&str> = planned.iter().map(|plan| plan.file_name.as_str()).collect();
    assert_eq!(files, ["control-loops/zone-temperatures.json"]);
}

#[test]
fn each_members_own_step_is_its_inputs_identification_step() {
    // control-studio-loop-setup-group.test.ts: "gives each member's input its own identification step, which Identify uses for that
    // input", with its numbers: a step of 10 and of 5 on the two members, and a duration of 40 s with no group step.
    let mut changed = draft();
    changed.experiment_step = None;
    changed.members[0].experiment_step = Some(10.0);
    changed.members[1].experiment_step = Some(5.0);
    let loop_definition = generate(&changed);
    loop_definition
        .validate(&scope())
        .expect("the loop is accepted");
    let text = serde_json::to_string(&loop_definition).expect("the loop serializes");
    LoopDefinition::decode(&text).expect("the loop file is read");
    let channels = loop_definition
        .channels
        .as_ref()
        .expect("the loop has its channels");
    let steps: Vec<Option<f64>> = channels
        .inputs
        .iter()
        .map(|input| input.experiment_step)
        .collect();
    assert_eq!(steps, [Some(10.0), Some(5.0)], "Specification 35 §17.9");
    let experiment = loop_definition
        .experiment
        .as_ref()
        .expect("the duration asks for an experiment");
    assert_eq!(
        (
            experiment.kind,
            experiment.amplitude,
            experiment.duration_seconds
        ),
        (ExperimentKind::Observation, 0.0, Some(40.0))
    );
    let io = json!({
        "memory": [
            { "name": "P1.Zone1ManualPct", "address": "%ML16" },
            { "name": "P1.Zone2ManualPct", "address": "%ML24" }
        ]
    });
    let stepped: Vec<(String, ExperimentKind, f64)> = input_steps(&loop_definition, &io)
        .expect("the two inputs are stepped")
        .into_iter()
        .map(|step| {
            let experiment = step
                .loop_definition
                .experiment
                .expect("each input is stepped on the experiment");
            (step.input_symbol, experiment.kind, experiment.amplitude)
        })
        .collect();
    assert_eq!(
        stepped,
        [
            ("P1.Heater1Pct".to_string(), ExperimentKind::MvStep, 10.0),
            ("P1.Heater2Pct".to_string(), ExperimentKind::MvStep, 5.0),
        ],
        "each input is stepped at its own step, as a step of that input"
    );
}

#[test]
fn a_group_step_without_member_steps_is_an_mv_step_of_that_amplitude_for_its_duration() {
    // The group's own step (Specification 35 §17.9): an output step of its amplitude, held for the duration given.
    let loop_definition = generate(&draft());
    let experiment = loop_definition
        .experiment
        .as_ref()
        .expect("the group step sets the experiment");
    assert_eq!(
        (
            experiment.execution,
            experiment.kind,
            experiment.amplitude,
            experiment.duration_seconds,
            experiment.seed
        ),
        (
            Execution::ControlStudio,
            ExperimentKind::MvStep,
            10.0,
            Some(40.0),
            1
        )
    );
    let text = serde_json::to_string(&loop_definition).expect("the loop serializes");
    LoopDefinition::decode(&text).expect("the loop file is read");
}

#[test]
fn a_group_step_without_a_duration_leaves_the_duration_to_identify() {
    let mut changed = draft();
    changed.experiment_duration_seconds = None;
    let loop_definition = generate(&changed);
    let experiment = loop_definition
        .experiment
        .as_ref()
        .expect("the group step sets the experiment");
    assert_eq!(
        (
            experiment.kind,
            experiment.amplitude,
            experiment.duration_seconds
        ),
        (ExperimentKind::MvStep, 10.0, None),
        "Identify names the duration while it is missing (B3.4 step 1)"
    );
}

#[test]
fn a_duration_without_a_step_writes_an_observation_of_amplitude_zero() {
    // The automatic request (B3.4): an observation has no amplitude, and Identify chooses the excitation.
    let mut changed = draft();
    changed.experiment_step = None;
    let loop_definition = generate(&changed);
    let experiment = loop_definition
        .experiment
        .as_ref()
        .expect("the duration asks for an experiment");
    assert_eq!(
        (
            experiment.kind,
            experiment.amplitude,
            experiment.duration_seconds
        ),
        (ExperimentKind::Observation, 0.0, Some(40.0))
    );
    let text = serde_json::to_string(&loop_definition).expect("the loop serializes");
    LoopDefinition::decode(&text).expect("the loop file is read");
}

#[test]
fn a_group_with_no_step_no_duration_and_no_member_step_writes_no_experiment() {
    let mut changed = draft();
    changed.experiment_step = None;
    changed.experiment_duration_seconds = None;
    let loop_definition = generate(&changed);
    assert!(
        loop_definition.experiment.is_none(),
        "Identify chooses the experiment (B3.4)"
    );
}

#[test]
fn a_duration_outside_the_identification_limits_is_refused() {
    let mut changed = draft();
    changed.experiment_duration_seconds = Some(0.5);
    let failure = group_loop_definition(&catalog(), &scope(), &changed)
        .expect_err("a duration under 1 s is refused");
    assert!(
        text_of(&failure).contains("identification duration"),
        "{}",
        text_of(&failure)
    );
}

#[test]
fn a_startup_flag_that_is_not_a_compiled_bool_is_refused() {
    let mut changed = draft();
    changed.startup_flag = Some("P1.Zone1C".to_string());
    let failure = group_loop_definition(&catalog(), &scope(), &changed)
        .expect_err("a REAL variable is refused as the startup flag");
    assert_eq!(text_of(&failure), not_compiled_bool("P1.Zone1C"));
}

#[test]
fn a_startup_flag_that_is_a_signal_of_the_loop_is_refused() {
    // The Auto/Manual signal is a BOOL, so only the signal rule refuses it.
    let mut changed = draft();
    changed.startup_flag = Some("P1.ZonesAuto".to_string());
    let failure = group_loop_definition(&catalog(), &scope(), &changed)
        .expect_err("the Auto/Manual signal is refused as the startup flag");
    assert!(
        text_of(&failure).contains("one of the loop's signals"),
        "{}",
        text_of(&failure)
    );
}

#[test]
fn a_program_that_writes_a_seeded_value_needs_the_startup_flag() {
    // check_startup (C1): the program writes the first member's setpoint, which validation seeds, and the draft names no flag.
    let mut program = catalog();
    program.writes.push("P1.Zone1SetpointC".to_string());
    let mut changed = draft();
    changed.startup_flag = None;
    let failure = group_loop_definition(&program, &scope(), &changed)
        .expect_err("a seeded value the program writes needs the flag");
    assert_eq!(
        text_of(&failure),
        startup_flag_needed(&program_writes("P1.Zone1SetpointC"))
    );
}

#[test]
fn a_program_that_initializes_a_member_needs_the_startup_flag() {
    // check_startup (C1): the program calls Initialize on the second member's PID, and the draft names no flag.
    let mut entries = links().as_array().cloned().unwrap_or_default();
    entries.push(json!({
        "instance": "P1.Zone2", "method": "Initialize", "argument": "Parameters",
        "record": "P1.Zone2Parameters", "output": null, "fields": {}
    }));
    let program = compiled_program("MainTask", &Value::Array(entries));
    let mut changed = draft();
    changed.startup_flag = None;
    let failure = group_loop_definition(&program, &scope(), &changed)
        .expect_err("a member the program initializes needs the flag");
    assert_eq!(
        text_of(&failure),
        startup_flag_needed(&program_initializes("P1.Zone2"))
    );
}

#[test]
fn a_member_whose_plant_input_is_its_controllers_output_behind_a_decoupler_is_refused() {
    // Specification 35 §17.9: behind a decoupler a member's output is the plant input that the program writes from D·v, not its
    // controller's own output. The fixture's controllers write their outputs to P1.Heater<n>Pct, which are the plant inputs themselves.
    let mut changed = draft();
    changed.decoupler = Some(decoupler());
    let failure = group_loop_definition(&catalog(), &scope(), &changed)
        .expect_err("a plant input that is its controller's output is refused");
    assert!(
        text_of(&failure).contains("Behind a decoupler"),
        "{}",
        text_of(&failure)
    );
    assert_eq!(
        failure.field.as_deref(),
        Some("group.0.manipulatedVariable"),
        "the refusal names the first member's plant input"
    );
}

#[test]
fn the_same_draft_without_a_decoupler_is_accepted_because_the_plant_input_is_the_controllers_output(
) {
    // A paired group drives each plant input with its controller's own output, so the decoupler's rule does not apply to it.
    let loop_definition = generate(&draft());
    let text = serde_json::to_string(&loop_definition).expect("the loop serializes");
    LoopDefinition::decode(&text).expect("the loop file is read");
}

#[test]
fn a_decoupled_draft_whose_plant_inputs_differ_from_the_controllers_outputs_is_accepted() {
    let mut changed = draft();
    changed.decoupler = Some(decoupler());
    let loop_definition = group_loop_definition(&controlled_catalog(), &scope(), &changed)
        .expect("the plant inputs are written from D·v, not the controllers' outputs");
    let text = serde_json::to_string(&loop_definition).expect("the loop serializes");
    LoopDefinition::decode(&text).expect("the loop file is read");
}
