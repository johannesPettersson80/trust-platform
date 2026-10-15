//! Tests of the search-law generator (`searchLawLoopGenerator.ts`): the loop assertions of `control-studio-loop-setup-search-law.test.ts`
//! on its oven, the refusals of its generator and of the form (`loopSetupForm.ts`), and the decode of the written loop by the loop
//! contract. The startup-done flag is an application state (C1, C5 Setup), which the TypeScript test does not pin: the test follows
//! the specification (`applicationStates`), and the difference is named in the report.

#![allow(clippy::float_cmp)]

use serde_json::{json, Value};

use crate::failure::Failure;
use crate::json::decode_value;
use crate::loop_definition::{
    InitialValue, LoopDefinition, LoopScope, ObjectiveDirection, RealType,
};
use crate::setup::catalog::{decode_catalog_answer, CompiledCatalog};
use crate::setup::save::{check_setup_targets, parse_setup_draft, plan_setup_save, SetupDraft};
use crate::setup::search_loop::{search_loop_definition, SearchLawLoopDraft};

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

#[allow(clippy::needless_pass_by_value)] // A test fixture: it takes the JSON literals its callers write in place.
fn decode(control: Value) -> CompiledCatalog {
    decode_catalog_answer(&json!({ "ok": true, "processImage": {}, "controlSymbols": control }))
        .expect("the compiled answer decodes")
        .control
}

/// The oven of `control-studio-loop-setup-search-law.test.ts`: the program computes a law whose coefficients no formula designs.
fn oven() -> CompiledCatalog {
    decode(json!({
        "symbols": [
            { "symbol": "P1.OvenC", "type": "LREAL", "address": "%IL0" },
            { "symbol": "P1.HeaterPct", "type": "LREAL", "address": "%QL0" },
            { "symbol": "P1.SetpointC", "type": "LREAL", "address": "%ML0" },
            { "symbol": "P1.ManualPct", "type": "LREAL", "address": "%ML8" },
            { "symbol": "P1.AutoMode", "type": "BOOL" },
            { "symbol": "P1.Started", "type": "BOOL" },
            { "symbol": "P1.Weight", "type": "LREAL" },
            { "symbol": "P1.Horizon", "type": "REAL" },
            { "symbol": "P1.Integral", "type": "LREAL" },
            { "symbol": "P1.Filtered", "type": "LREAL" }
        ],
        "controllers": [
            { "instance": "P1", "type": "Main", "tasks": [{ "name": "MainTask", "intervalMs": 100 }] }
        ],
        "links": [],
        "writes": ["P1.HeaterPct", "P1.Integral", "P1.Filtered", "P1.Started"]
    }))
}

/// The searched law of the oven, as the form sends it: two coefficients with their value lists, two kept values and a startup flag.
fn oven_json() -> Value {
    json!({
        "name": "Oven MPC",
        "action": "direct",
        "family": "MPC",
        "instance": "P1",
        "processVariable": { "symbol": "P1.OvenC", "unit": "degC", "minimum": 0, "maximum": 300 },
        "manipulatedVariable": { "symbol": "P1.HeaterPct", "unit": "%", "minimum": 0, "maximum": 100 },
        "setpoint": { "symbol": "P1.SetpointC" },
        "mode": { "symbol": "P1.AutoMode", "whenTrue": "Auto" },
        "manualOutput": "P1.ManualPct",
        "coefficients": [
            { "symbol": "P1.Weight", "unit": "1", "values": "0.5, 1, 2" },
            { "symbol": "P1.Horizon", "unit": "s", "values": "0.1; 5" }
        ],
        "states": [
            { "symbol": "P1.Integral", "unit": "%", "start": "output" },
            { "symbol": "P1.Filtered", "unit": "degC", "start": "measurement" }
        ],
        "startupFlag": "P1.Started"
    })
}

fn draft_of(value: Value) -> SearchLawLoopDraft {
    decode_value(value).expect("the draft decodes")
}

/// The refusal a draft gets on the oven, as its whole sentence.
fn refusal(value: Value) -> String {
    let failure = search_loop_definition(&oven(), &scope(), &draft_of(value))
        .expect_err("the draft is refused");
    text_of(&failure)
}

/// The draft with one edit applied to it.
fn edited(edit: impl FnOnce(&mut Value)) -> Value {
    let mut value = oven_json();
    edit(&mut value);
    value
}

#[test]
fn the_search_law_writes_a_project_controller_that_tries_every_combination_on_the_actual_st() {
    // control-studio-loop-setup-search-law.test.ts: "writes a project controller that searches every combination of the coefficient
    // values on the actual ST".
    let loop_definition = search_loop_definition(&oven(), &scope(), &draft_of(oven_json()))
        .expect("the searched law is written");
    let text = serde_json::to_string(&loop_definition).expect("the loop serializes");
    let decoded = LoopDefinition::decode(&text).expect("the written loop decodes");
    decoded
        .rust_engine_support()
        .expect("the engine serves a searched law");
    loop_definition
        .validate(&scope())
        .expect("the loop passes the checks that need no runtime");
    let project = loop_definition
        .controller
        .project
        .as_ref()
        .expect("a project controller");
    assert_eq!(project.family, "MPC");
    assert!(project.seed.is_none(), "a searched law has no seed");
    let axes: Vec<(&str, &[f64])> = project
        .axes
        .iter()
        .map(|axis| (axis.symbol.as_str(), axis.values.as_slice()))
        .collect();
    assert_eq!(
        axes,
        [
            ("P1.Weight", [0.5, 1.0, 2.0].as_slice()),
            ("P1.Horizon", [f64::from(0.1_f32), 5.0].as_slice())
        ],
        "a REAL variable's values are rounded to the nearest REAL"
    );
    assert!(matches!(
        project.objective.direction,
        ObjectiveDirection::Minimize
    ));
    assert_eq!(project.objective.requirement_id, "target-error");
    assert_eq!(project.maximum_candidates, 64);
    let parameters: Vec<(&str, &str, f64, f64)> = loop_definition
        .parameter_binding
        .as_ref()
        .expect("a parameter record")
        .parameters
        .iter()
        .map(|parameter| {
            (
                parameter.symbol.as_str(),
                parameter.unit.as_str(),
                parameter.minimum,
                parameter.maximum,
            )
        })
        .collect();
    assert_eq!(
        parameters,
        [
            ("P1.Weight", "1", 0.5, 2.0),
            ("P1.Horizon", "s", f64::from(0.1_f32), 5.0)
        ]
    );
    assert!(matches!(
        loop_definition
            .parameter_binding
            .as_ref()
            .map(|binding| binding.parameters[1].iec_type),
        Some(RealType::Real)
    ));
}

#[test]
fn the_kept_values_start_as_validation_starts_them_and_the_startup_flag_is_an_application_state() {
    // searchLawLoopGenerator.ts's `retained` and C5 Setup: the output's start is bounded by ten output spans, the measurement's by one
    // measurement span; the startup-done flag is an application state that starts TRUE (C1).
    let loop_definition = search_loop_definition(&oven(), &scope(), &draft_of(oven_json()))
        .expect("the searched law is written");
    let project = loop_definition
        .controller
        .project
        .as_ref()
        .expect("a project controller");
    let kept: Vec<(&str, &str, f64, f64)> = project
        .initialization
        .states()
        .iter()
        .filter(|state| !project.is_application_state(&state.symbol))
        .map(|state| {
            (
                state.symbol.as_str(),
                state.unit.as_str(),
                state.minimum,
                state.maximum,
            )
        })
        .collect();
    assert_eq!(
        kept,
        [
            ("P1.Integral", "%", -1000.0, 1100.0),
            ("P1.Filtered", "degC", -300.0, 600.0)
        ]
    );
    assert_eq!(project.application_states, vec!["P1.Started".to_string()]);
    let flag = project
        .initialization
        .states()
        .iter()
        .find(|state| state.symbol == "P1.Started")
        .expect("the flag is declared");
    assert_eq!(
        flag.initial,
        InitialValue::Constant(1.0),
        "the flag starts TRUE"
    );
}

#[test]
fn a_searched_law_refuses_the_coefficients_it_cannot_search() {
    // searchLawLoopGenerator.ts's refusals, in the order the generator makes them.
    assert_eq!(
        refusal(edited(|value| value["coefficients"] = json!([]))),
        "Choose at least one coefficient Tune may search."
    );
    let five: Vec<Value> = (1..=5)
        .map(|index| json!({ "symbol": format!("P1.C{index}"), "unit": "1", "values": "1" }))
        .collect();
    assert_eq!(
        refusal(edited(|value| value["coefficients"] = json!(five))),
        "Tune searches at most four coefficients."
    );
    assert_eq!(
        refusal(edited(|value| {
            value["coefficients"] = json!([
                { "symbol": "P1.Weight", "unit": "1", "values": "1, 2, 3, 4, 5, 6, 7, 8, 9" },
                { "symbol": "P1.Horizon", "unit": "s", "values": "1, 2, 3, 4, 5, 6, 7, 8" }
            ]);
        })),
        "Tune tries at most 64 combinations; these values make 72."
    );
    assert_eq!(
        refusal(edited(
            |value| value["states"] =
                json!([{ "symbol": "P1.Weight", "unit": "1", "start": "zero" }])
        )),
        "Choose a different variable for each coefficient and each value the program keeps."
    );
    assert_eq!(
        refusal(edited(
            |value| value["coefficients"][0]["values"] = json!("")
        )),
        "Enter the values of P1.Weight Tune may try, separated by commas."
    );
}

#[test]
fn the_form_names_a_value_list_it_cannot_read_and_the_combination_limit() {
    // loopSetupForm.ts's search draft: a value the form cannot read, then more than 64 combinations.
    assert_eq!(
        refusal(edited(
            |value| value["coefficients"][0]["values"] = json!("0.5, fast")
        )),
        "Enter the values of P1.Weight Tune may try, separated by commas."
    );
    assert_eq!(
        refusal(edited(|value| value["coefficients"][0]["unit"] = json!(""))),
        "Enter the unit of P1.Weight."
    );
    assert_eq!(
        refusal(edited(|value| value["mode"]["symbol"] = json!(""))),
        "Choose the Auto/Manual signal."
    );
    assert_eq!(
        refusal(edited(|value| value["family"] = json!("bogus"))),
        "Choose which kind of law the program computes."
    );
}

#[test]
fn the_search_law_names_the_instance_it_lacks_and_the_startup_flag_it_needs() {
    assert_eq!(
        refusal(edited(|value| value["instance"] = json!(""))),
        "Choose the instance that computes the law."
    );
    assert_eq!(
        refusal(edited(|value| value["instance"] = json!("P1.Missing"))),
        "P1.Missing is not a compiled program or function block instance."
    );
    let catalog = decode(json!({
        "symbols": [
            { "symbol": "P1.OvenC", "type": "LREAL", "address": "%IL0" },
            { "symbol": "P1.HeaterPct", "type": "LREAL", "address": "%QL0" },
            { "symbol": "P1.SetpointC", "type": "LREAL", "address": "%ML0" },
            { "symbol": "P1.ManualPct", "type": "LREAL", "address": "%ML8" },
            { "symbol": "P1.AutoMode", "type": "BOOL" },
            { "symbol": "P1.Started", "type": "BOOL" },
            { "symbol": "P1.Weight", "type": "LREAL" },
            { "symbol": "P1.Horizon", "type": "REAL" },
            { "symbol": "P1.Integral", "type": "LREAL" },
            { "symbol": "P1.Filtered", "type": "LREAL" }
        ],
        "controllers": [
            { "instance": "P1", "type": "Main", "tasks": [{ "name": "MainTask", "intervalMs": 100 }] }
        ],
        "links": [],
        "writes": ["P1.HeaterPct", "P1.Integral", "P1.Filtered", "P1.Weight"]
    }));
    let without_flag = edited(|value| value["startupFlag"] = Value::Null);
    let failure = search_loop_definition(&catalog, &scope(), &draft_of(without_flag))
        .expect_err("the program writes a coefficient, so it needs the flag");
    assert_eq!(
        text_of(&failure),
        "The program writes P1.Weight in its own code. Choose the flag that marks its startup as done, so validation keeps the values it starts from."
    );
}

#[test]
fn the_searched_law_saves_as_one_loop_file_named_from_its_loop() {
    // control-studio-loop-setup-search-law.test.ts: "saves the searched law as one loop file": the plan names the file from the loop id.
    let parsed =
        parse_setup_draft(&with_structure(oven_json())).expect("the searched draft decodes");
    assert!(
        matches!(parsed, SetupDraft::Search(_)),
        "the search structure is served"
    );
    let planned = plan_setup_save(&oven(), &scope(), &parsed).expect("the searched law is planned");
    assert_eq!(planned.len(), 1);
    assert_eq!(planned[0].file_name, "control-loops/oven-mpc.json");
    let axes: Vec<&str> = planned[0]
        .loop_definition
        .controller
        .project
        .as_ref()
        .expect("a project controller")
        .axes
        .iter()
        .map(|axis| axis.symbol.as_str())
        .collect();
    assert_eq!(axes, ["P1.Weight", "P1.Horizon"]);
    check_setup_targets(&[], &planned).expect("nothing collides in an empty project");
}

/// The searched draft with its structure, as the setup form sends it.
fn with_structure(mut value: Value) -> Value {
    value["structure"] = json!("search");
    value
}
