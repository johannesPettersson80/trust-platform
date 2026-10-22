//! Tests of the gain-schedule generator (`gainScheduleLoopDefinitions`, specification 35, C10 Setup), ported from
//! `control-studio-gain-schedule.test.ts`: one loop per operating point with that point's gains, the refusal of fewer than two or more than
//! four points, and the save of one loop file per point. The form test of that file is the page's and is not ported here. The rules C10
//! adds (the scheduling variable, the order and the tolerances of the points, the startup flag) are tested beside them.

#![allow(clippy::float_cmp)] // The operating points and tolerances are the exact values the setup wrote.

use serde_json::{json, Value};

use super::{
    schedule_loop_definitions, ScheduleLoopDraft, SchedulePointDraft, SchedulingVariableDraft,
};
use crate::failure::Failure;
use crate::json::decode_value;
use crate::loop_definition::{Action, LoopDefinition, LoopScope, ModeWhenTrue};
use crate::setup::catalog::{decode_catalog_answer, CompiledCatalog};
use crate::setup::loop_bindings::FormSignal;
use crate::setup::pid_loop::{ModeChoice, SignalChoice};
use crate::setup::save::{check_setup_targets, parse_setup_draft, plan_setup_save, SetupDraft};

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

/// The tank of `control-studio-gain-schedule.test.ts`: a tank level held by one library PID whose program interpolates its gains from
/// a two-point schedule. The record fields are the ones the program writes, and the startup flag is the program's own.
fn tank_json() -> Value {
    json!({
        "symbols": [
            { "symbol": "P1.LevelM", "type": "LREAL", "address": "%IL0" },
            { "symbol": "P1.SetpointM", "type": "LREAL", "address": "%ML0" },
            { "symbol": "P1.ValvePct", "type": "LREAL", "address": "%QL0" },
            { "symbol": "P1.ManualPct", "type": "LREAL", "address": "%ML8" },
            { "symbol": "P1.Auto", "type": "BOOL" },
            { "symbol": "P1.Started", "type": "BOOL" },
            { "symbol": "P1.KpLow", "type": "LREAL" },
            { "symbol": "P1.KiLow", "type": "LREAL" },
            { "symbol": "P1.KpHigh", "type": "LREAL" },
            { "symbol": "P1.KiHigh", "type": "LREAL" },
            { "symbol": "P1.Parameters.ProportionalGain", "type": "LREAL" },
            { "symbol": "P1.Parameters.IntegralGain", "type": "LREAL" }
        ],
        "controllers": [
            { "instance": "P1", "type": "Main", "tasks": [{ "name": "MainTask", "intervalMs": 100 }] },
            { "instance": "P1.Loop", "type": "Control.PID", "tasks": [{ "name": "MainTask", "intervalMs": 100 }] }
        ],
        "links": [],
        "writes": ["P1.ValvePct", "P1.Parameters.ProportionalGain", "P1.Parameters.IntegralGain"]
    })
}

fn tank() -> CompiledCatalog {
    decode(tank_json())
}

fn signal(symbol: &str, unit: &str, minimum: f64, maximum: f64) -> FormSignal {
    FormSignal {
        symbol: symbol.to_string(),
        unit: unit.to_string(),
        minimum: Some(minimum),
        maximum: Some(maximum),
    }
}

/// The two operating points of the test: Low at 0.09 m and High at 0.49 m, the levels the program's `LowLevelM` and `HighLevelM` name.
fn points() -> Vec<SchedulePointDraft> {
    vec![
        SchedulePointDraft {
            name: "Low".to_string(),
            value: Some(0.09),
            tolerance: Some(0.01),
            proportional_gain: "P1.KpLow".to_string(),
            integral_gain: "P1.KiLow".to_string(),
        },
        SchedulePointDraft {
            name: "High".to_string(),
            value: Some(0.49),
            tolerance: Some(0.01),
            proportional_gain: "P1.KpHigh".to_string(),
            integral_gain: "P1.KiHigh".to_string(),
        },
    ]
}

#[test]
fn a_point_without_its_value_or_its_tolerance_is_refused_by_the_sentence_that_names_it() {
    // The form leaves a blank number null; the generator refuses it under the point's name (C10 Setup).
    let mut without_value = draft();
    without_value.points[0].value = None;
    let failure = schedule_loop_definitions(&tank(), &scope(), &without_value)
        .expect_err("a point with no value is refused");
    assert_eq!(
        failure.message,
        "Enter operating point Low's value of the scheduling variable."
    );

    let mut without_tolerance = draft();
    without_tolerance.points[1].tolerance = None;
    let failure = schedule_loop_definitions(&tank(), &scope(), &without_tolerance)
        .expect_err("a point with no tolerance is refused");
    assert_eq!(
        failure.message,
        "Enter operating point High's tolerance on that value."
    );
}

/// The draft of the tank (`draft` in `control-studio-gain-schedule.test.ts`), with the scheduling variable C10 asks for and the startup
/// flag that C1 Setup requires for a program that writes the PID's record.
fn draft() -> ScheduleLoopDraft {
    ScheduleLoopDraft {
        name: "Tank level".to_string(),
        action: Action::Direct,
        instance: "P1.Loop".to_string(),
        parameter_record: "P1.Parameters".to_string(),
        scheduling: SchedulingVariableDraft {
            symbol: "P1.LevelM".to_string(),
            unit: "m".to_string(),
        },
        process_variable: signal("P1.LevelM", "m", 0.0, 2.0),
        manipulated_variable: signal("P1.ValvePct", "%", 0.0, 100.0),
        setpoint: SignalChoice {
            symbol: "P1.SetpointM".to_string(),
            unit: None,
            minimum: None,
            maximum: None,
        },
        mode: ModeChoice {
            symbol: "P1.Auto".to_string(),
            when_true: ModeWhenTrue::Auto,
        },
        manual_output: "P1.ManualPct".to_string(),
        points: points(),
        experiment_step: None,
        experiment_duration_seconds: None,
        startup_flag: Some("P1.Started".to_string()),
    }
}

/// The project block of a loop as the loop file writes it.
fn project_json(loop_definition: &LoopDefinition) -> Value {
    serde_json::to_value(
        loop_definition
            .controller
            .project
            .as_ref()
            .expect("a schedule loop is a project law"),
    )
    .expect("the block serializes")
}

/// The loop definition as the loop file holds it, decoded again: a loop the setup writes must decode (B1.4).
fn decoded(loop_definition: &LoopDefinition) -> LoopDefinition {
    let text = serde_json::to_string(loop_definition).expect("the loop serializes");
    LoopDefinition::decode(&text).expect("the written loop decodes")
}

#[test]
fn the_schedule_writes_one_loop_per_operating_point_with_that_points_gains() {
    let loops = schedule_loop_definitions(&tank(), &scope(), &draft())
        .expect("the tank's schedule is generated");
    assert_eq!(
        loops
            .iter()
            .map(|loop_definition| (loop_definition.name.as_str(), loop_definition.id.as_str()))
            .collect::<Vec<_>>(),
        vec![
            ("Tank level · Low", "loop:tank-level-low"),
            ("Tank level · High", "loop:tank-level-high"),
        ]
    );
    let expected = [("P1.KpLow", "P1.KiLow"), ("P1.KpHigh", "P1.KiHigh")];
    for (loop_definition, (proportional, integral)) in loops.iter().zip(expected) {
        let project = project_json(loop_definition);
        assert_eq!(project["family"], "gain-scheduled");
        assert_eq!(
            project["seed"],
            json!({
                "kind": "parallel-pid",
                "family": "PI",
                "proportionalSymbol": proportional,
                "integralSymbol": integral,
            })
        );
        assert_eq!(project["objective"]["requirementId"], "target-error");
        assert_eq!(project["maximumCandidates"], 1);
        assert_eq!(project["maximumCycles"], 50_000);
        assert_eq!(project["wallBudgetMs"], 60_000);
        let states = project["initialization"]["states"]
            .as_array()
            .expect("the states are a list");
        assert!(
            states
                .iter()
                .any(|state| state["symbol"] == "P1.Loop.StateValue.NominalOutput"),
            "the PID starts bumplessly"
        );
        let parameters: Vec<(&str, &str)> = loop_definition
            .parameter_binding
            .as_ref()
            .expect("a parameter binding")
            .parameters
            .iter()
            .map(|parameter| (parameter.symbol.as_str(), parameter.unit.as_str()))
            .collect();
        let units = [(proportional, "%/m"), (integral, "%/(m*s)")];
        assert_eq!(parameters, units.to_vec());
        assert!(
            loop_definition.validate(&scope()).is_ok(),
            "the loop validates"
        );
        assert_eq!(decoded(loop_definition).id, loop_definition.id);
    }
}

#[test]
fn every_loop_of_the_schedule_holds_the_whole_table_and_names_its_own_point() {
    let loops = schedule_loop_definitions(&tank(), &scope(), &draft()).expect("generated");
    for (loop_definition, point) in loops.iter().zip(["Low", "High"]) {
        let block = &project_json(loop_definition)["schedule"];
        assert_eq!(block["point"], point);
        assert_eq!(block["parameterRecord"], "P1.Parameters");
        assert_eq!(
            block["scheduling"],
            json!({ "symbol": "P1.LevelM", "unit": "m", "iecType": "LREAL" })
        );
        assert_eq!(
            block["points"],
            json!([
                { "name": "Low", "loopId": "loop:tank-level-low", "value": 0.09, "tolerance": 0.01,
                  "proportionalSymbol": "P1.KpLow", "integralSymbol": "P1.KiLow" },
                { "name": "High", "loopId": "loop:tank-level-high", "value": 0.49, "tolerance": 0.01,
                  "proportionalSymbol": "P1.KpHigh", "integralSymbol": "P1.KiHigh" },
            ])
        );
    }
}

#[test]
fn the_startup_flag_is_an_application_state_of_the_program_as_c1_says() {
    let loops = schedule_loop_definitions(&tank(), &scope(), &draft()).expect("generated");
    let project = project_json(&loops[0]);
    assert_eq!(
        project["initialization"]["applicationStates"][0]["symbol"],
        "P1.Started"
    );
    let states = project["initialization"]["states"]
        .as_array()
        .expect("states");
    assert!(states.iter().all(|state| state["symbol"] != "P1.Started"));
}

#[test]
fn a_schedule_refuses_fewer_than_two_or_more_than_four_operating_points() {
    let mut one = draft();
    one.points.truncate(1);
    let failure = schedule_loop_definitions(&tank(), &scope(), &one)
        .expect_err("one point is not a schedule");
    assert_eq!(
        text_of(&failure),
        "A gain schedule needs two to four operating points."
    );

    let mut five = draft();
    for index in 3..=5 {
        five.points.push(SchedulePointDraft {
            name: format!("Point {index}"),
            value: Some(0.5 + f64::from(index)),
            tolerance: Some(0.01),
            proportional_gain: format!("P1.Kp{index}"),
            integral_gain: format!("P1.Ki{index}"),
        });
    }
    assert!(schedule_loop_definitions(&tank(), &scope(), &five).is_err());
}

#[test]
fn the_values_must_increase_in_the_order_given_and_the_first_point_out_of_order_is_named() {
    let mut reversed = draft();
    reversed.points[0].value = Some(0.49);
    reversed.points[1].value = Some(0.09);
    let failure =
        schedule_loop_definitions(&tank(), &scope(), &reversed).expect_err("High is below Low");
    assert_eq!(
        text_of(&failure),
        "The operating points must increase in the order they are given: High at 0.09 is not above Low at 0.49."
    );
}

#[test]
fn a_point_needs_a_tolerance_above_zero() {
    let mut no_tolerance = draft();
    no_tolerance.points[1].tolerance = Some(0.0);
    let failure = schedule_loop_definitions(&tank(), &scope(), &no_tolerance)
        .expect_err("a tolerance of zero is not one");
    assert_eq!(
        text_of(&failure),
        "Give each operating point a tolerance above zero, in the scheduling variable's unit."
    );
}

#[test]
fn the_scheduling_variable_and_the_gains_are_real_variables_of_the_program() {
    let mut boolean = draft();
    boolean.scheduling.symbol = "P1.Auto".to_string();
    assert!(
        schedule_loop_definitions(&tank(), &scope(), &boolean).is_err(),
        "a BOOL cannot be scheduled"
    );
    let mut missing = draft();
    missing.points[0].proportional_gain = "P1.Missing".to_string();
    assert!(schedule_loop_definitions(&tank(), &scope(), &missing).is_err());
}

#[test]
fn a_program_that_writes_its_record_needs_the_startup_flag() {
    let mut no_flag = draft();
    no_flag.startup_flag = None;
    assert!(
        schedule_loop_definitions(&tank(), &scope(), &no_flag).is_err(),
        "the program writes P1.Parameters, so the startup-done flag is needed"
    );
    assert!(schedule_loop_definitions(&tank(), &scope(), &draft()).is_ok());
}

#[test]
fn a_schedule_draft_is_no_longer_parked_and_saves_one_file_per_point() {
    let json = json!({
        "structure": "schedule",
        "name": "Tank level",
        "action": "direct",
        "instance": "P1.Loop",
        "parameterRecord": "P1.Parameters",
        "scheduling": { "symbol": "P1.LevelM", "unit": "m" },
        "processVariable": { "symbol": "P1.LevelM", "unit": "m", "minimum": 0.0, "maximum": 2.0 },
        "manipulatedVariable": { "symbol": "P1.ValvePct", "unit": "%", "minimum": 0.0, "maximum": 100.0 },
        "setpoint": { "symbol": "P1.SetpointM" },
        "mode": { "symbol": "P1.Auto", "whenTrue": "Auto" },
        "manualOutput": "P1.ManualPct",
        "points": [
            { "name": "Low", "value": 0.09, "tolerance": 0.01, "proportionalGain": "P1.KpLow", "integralGain": "P1.KiLow" },
            { "name": "High", "value": 0.49, "tolerance": 0.01, "proportionalGain": "P1.KpHigh", "integralGain": "P1.KiHigh" }
        ],
        "startupFlag": "P1.Started"
    });
    let draft = parse_setup_draft(&json).expect("the schedule draft parses");
    assert!(
        matches!(draft, SetupDraft::Schedule(_)),
        "a schedule is no longer parked"
    );
    let planned = plan_setup_save(&tank(), &scope(), &draft).expect("the schedule plans");
    assert_eq!(
        planned
            .iter()
            .map(|planned_loop| planned_loop.file_name.as_str())
            .collect::<Vec<_>>(),
        vec![
            "control-loops/tank-level-low.json",
            "control-loops/tank-level-high.json",
        ]
    );
    assert_eq!(
        planned
            .iter()
            .map(|planned_loop| planned_loop.loop_definition.name.as_str())
            .collect::<Vec<_>>(),
        vec!["Tank level · Low", "Tank level · High"]
    );
    assert!(check_setup_targets(&[], &planned).is_ok());
}

#[test]
fn a_schedule_draft_reads_the_camel_case_fields_the_form_sends() {
    let json = json!({
        "name": "Tank level",
        "action": "direct",
        "instance": "P1.Loop",
        "parameterRecord": "P1.Parameters",
        "scheduling": { "symbol": "P1.LevelM", "unit": "m" },
        "processVariable": { "symbol": "P1.LevelM", "unit": "m", "minimum": 0.0, "maximum": 2.0 },
        "manipulatedVariable": { "symbol": "P1.ValvePct", "unit": "%", "minimum": 0.0, "maximum": 100.0 },
        "setpoint": { "symbol": "P1.SetpointM" },
        "mode": { "symbol": "P1.Auto", "whenTrue": "Auto" },
        "manualOutput": "P1.ManualPct",
        "points": [
            { "name": "Low", "value": 0.09, "tolerance": 0.01, "proportionalGain": "P1.KpLow", "integralGain": "P1.KiLow" },
            { "name": "High", "value": 0.49, "tolerance": 0.01, "proportionalGain": "P1.KpHigh", "integralGain": "P1.KiHigh" }
        ],
        "startupFlag": "P1.Started"
    });
    let read: ScheduleLoopDraft = decode_value(json.clone()).expect("the form's draft decodes");
    assert_eq!(read, draft());
    let mut unknown = json;
    unknown["extra"] = json!(1);
    assert!(
        decode_value::<ScheduleLoopDraft>(unknown).is_err(),
        "a field the draft does not declare is refused"
    );
}

/// The tank's schedule as the setup form posts it (B1.3): every operating-point row it draws, each number as the text typed, with the
/// two filled rows `low` and `high` and the rows `third` and `fourth` after them.
fn posted(low: &Value, high: &Value, third: &Value, fourth: &Value) -> ScheduleLoopDraft {
    decode_value(json!({
        "name": "Tank level", "action": "direct", "instance": "P1.Loop", "parameterRecord": "P1.Parameters",
        "scheduling": { "symbol": "P1.LevelM", "unit": "m" },
        "processVariable": { "symbol": "P1.LevelM", "unit": "m", "minimum": "0", "maximum": "2" },
        "manipulatedVariable": { "symbol": "P1.ValvePct", "unit": "%", "minimum": "0", "maximum": "100" },
        "setpoint": { "symbol": "P1.SetpointM" }, "mode": { "symbol": "P1.Auto", "whenTrue": "Auto" },
        "manualOutput": "P1.ManualPct", "points": [low, high, third, fourth], "startupFlag": "P1.Started"
    }))
    .expect("the posted draft decodes")
}

/// B1.3 and C10 Setup: the form posts the four operating-point rows it draws, each number as the text typed. The two rows the engineer
/// filled in, Low at `"0.09"` and High at `"0.49"`, each within `"0.01"`, are the points, and the two blank rows after them are no points:
/// the draft writes the loops of the two-point draft. A row with any entry is a point, so a row that names only a gain variable is
/// refused as a point without its name, and a value typed as `"0,09"` is refused with the sentence that asks for Low's value. The wrong
/// behaviour caught: a blank row the page draws counted as a point, a typed number refused as missing, or a decimal comma read as a number.
#[test]
fn an_operating_point_row_with_no_entry_is_no_point() {
    let blank = json!({ "name": "", "value": "", "tolerance": "", "proportionalGain": "", "integralGain": "" });
    let low = json!({ "name": "Low", "value": "0.09", "tolerance": "0.01", "proportionalGain": "P1.KpLow", "integralGain": "P1.KiLow" });
    let high = json!({ "name": "High", "value": "0.49", "tolerance": "0.01", "proportionalGain": "P1.KpHigh", "integralGain": "P1.KiHigh" });
    assert_eq!(
        schedule_loop_definitions(&tank(), &scope(), &posted(&low, &high, &blank, &blank))
            .expect("the filled rows are the points"),
        schedule_loop_definitions(&tank(), &scope(), &draft()).expect("the two-point draft")
    );

    let gain_only = json!({ "name": "", "value": "", "tolerance": "", "proportionalGain": "P1.KpMid", "integralGain": "" });
    let refusal =
        schedule_loop_definitions(&tank(), &scope(), &posted(&low, &high, &gain_only, &blank))
            .expect_err("a row with an entry is a point");
    assert_eq!(
        text_of(&refusal),
        "Give each operating point its own name and its own two gain variables."
    );

    let mut comma = low.clone();
    comma["value"] = json!("0,09");
    let refusal =
        schedule_loop_definitions(&tank(), &scope(), &posted(&comma, &high, &blank, &blank))
            .expect_err("a decimal comma is no number");
    assert_eq!(
        text_of(&refusal),
        "Enter operating point Low's value of the scheduling variable."
    );
}
