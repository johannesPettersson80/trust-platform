//! Tests of the compiled catalog: the TypeScript assertions of `control-studio-loop-setup.test.ts`, `control-studio-generated-loop.test.ts`
//! on their catalog fixtures (`control-studio-cascade-fixtures.ts` for the cascade), and the decode rules of `normalizeCatalog`.

#![allow(clippy::float_cmp)]

use serde_json::{json, Value};

use super::{
    cascade_pairs, decode_catalog_answer, own_measurement, pid_candidates, pid_parameter_records,
    CascadePair, CompiledCatalog, CompiledTask, ModeEvidence, PidCandidate,
    MAXIMUM_CATALOG_ENTRIES, PID_PARAMETERS,
};
use crate::failure::Failure;
use crate::loop_definition::ModeWhenTrue;

const STATE_FIELDS: [&str; 7] = [
    "Setpoint",
    "SetpointRate",
    "ProcessValue",
    "ProcessValueRate",
    "FeedForward",
    "NominalOutput",
    "SaturatedOutput",
];

/// The sentence a refusal shows: its whole text, which is the detail when it says more than its first sentence.
fn text_of(failure: &Failure) -> String {
    failure
        .detail
        .clone()
        .unwrap_or_else(|| failure.message.clone())
}

/// Decodes a compiled control-symbol list inside an answer that is ok (the `controlSymbols` payload).
#[allow(clippy::needless_pass_by_value)] // A test fixture: it takes the JSON literals its callers write in place.
fn decode(control: Value) -> CompiledCatalog {
    decode_catalog_answer(&json!({ "ok": true, "processImage": {}, "controlSymbols": control }))
        .expect("the compiled answer decodes")
        .control
}

/// The sentence the answer is refused with.
fn refused(answer: &Value) -> String {
    text_of(&decode_catalog_answer(answer).expect_err("the answer is refused"))
}

fn task() -> Value {
    json!([{ "name": "MainTask", "intervalMs": 100 }])
}

/// The symbols of one Control.PID: its eleven record fields, and its seven retained states (`pidSymbols`).
fn pid_symbols(instance: &str, record: &str) -> Vec<Value> {
    PID_PARAMETERS
        .iter()
        .map(|field| json!({ "symbol": format!("{record}.{field}"), "type": "LREAL" }))
        .chain(STATE_FIELDS.iter().map(
            |field| json!({ "symbol": format!("{instance}.StateValue.{field}"), "type": "LREAL" }),
        ))
        .collect()
}

/// The compiled facts of a single-loop program (`singleLoop` in `control-studio-loop-setup.test.ts`).
fn single_loop_json() -> Value {
    let mut symbols = vec![
        json!({ "symbol": "P1.TemperatureC", "type": "LREAL", "address": "%IL8" }),
        json!({ "symbol": "P1.AppliedPowerPct", "type": "LREAL", "address": "%QL0" }),
        json!({ "symbol": "P1.SetpointC", "type": "LREAL", "address": "%ML0" }),
        json!({ "symbol": "P1.AutoMode", "type": "BOOL", "address": "%MX96.0" }),
        json!({ "symbol": "P1.ManualOutput", "type": "LREAL", "address": "%ML72" }),
        json!({ "symbol": "P1.SafeProcessValueC", "type": "LREAL" }),
    ];
    symbols.extend(pid_symbols("P1.Controller", "P1.Parameters"));
    json!({
        "symbols": symbols,
        "controllers": [
            { "instance": "P1", "type": "Main", "tasks": task() },
            { "instance": "P1.Controller", "type": "Control.PID", "tasks": task() }
        ],
        "links": [
            { "instance": "P1.Controller", "method": "Configure", "argument": "Parameters", "record": "P1.Parameters", "output": null, "fields": {} },
            { "instance": "P1.Controller", "method": "Update", "argument": "Input", "record": "P1.Input", "output": "P1.AppliedPowerPct",
              "fields": { "ManualOutput": ["P1.ManualOutput"], "ProcessValue": ["P1.SafeProcessValueC"], "Setpoint": ["P1.SetpointC"],
                          "TrackingOutput": ["P1.ManualOutput"] } }
        ],
        // The program computes the checked measurement from the temperature and writes the PID output.
        "writes": ["P1.SafeProcessValueC", "P1.AppliedPowerPct"]
    })
}

/// The compiled facts of a two-PID cascade (`cascade` in `control-studio-cascade-fixtures.ts`): the outer output feeds the inner setpoint.
fn cascade_json() -> Value {
    let mut symbols = vec![
        json!({ "symbol": "P1.VesselC", "type": "LREAL", "address": "%IL0" }),
        json!({ "symbol": "P1.JacketC", "type": "LREAL", "address": "%IL8" }),
        json!({ "symbol": "P1.VesselSetpointC", "type": "LREAL", "address": "%ML0" }),
        json!({ "symbol": "P1.JacketSetpointC", "type": "LREAL", "address": "%ML8" }),
        json!({ "symbol": "P1.HeaterPct", "type": "LREAL", "address": "%QL0" }),
        json!({ "symbol": "P1.OuterAuto", "type": "BOOL", "address": "%MX40.0" }),
        json!({ "symbol": "P1.InnerAuto", "type": "BOOL", "address": "%MX40.1" }),
        json!({ "symbol": "P1.OuterManual", "type": "LREAL", "address": "%ML16" }),
        json!({ "symbol": "P1.InnerManual", "type": "LREAL", "address": "%ML24" }),
    ];
    symbols.extend(pid_symbols("P1.Outer", "P1.OuterParameters"));
    symbols.extend(pid_symbols("P1.Inner", "P1.InnerParameters"));
    json!({
        "symbols": symbols,
        "controllers": [
            { "instance": "P1", "type": "Main", "tasks": task() },
            { "instance": "P1.Inner", "type": "Control.PID", "tasks": task() },
            { "instance": "P1.Outer", "type": "Control.PID", "tasks": task() }
        ],
        "links": [
            { "instance": "P1.Outer", "method": "Configure", "argument": "Parameters", "record": "P1.OuterParameters", "output": null, "fields": {} },
            { "instance": "P1.Inner", "method": "Configure", "argument": "Parameters", "record": "P1.InnerParameters", "output": null, "fields": {} },
            { "instance": "P1.Outer", "method": "Update", "argument": "Input", "record": "P1.OuterInput", "output": "P1.JacketSetpointC",
              "fields": { "ManualOutput": ["P1.OuterManual"], "ProcessValue": ["P1.VesselC"], "Setpoint": ["P1.VesselSetpointC"] } },
            { "instance": "P1.Inner", "method": "Update", "argument": "Input", "record": "P1.InnerInput", "output": "P1.HeaterPct",
              "fields": { "ManualOutput": ["P1.InnerManual"], "ProcessValue": ["P1.JacketC"], "Setpoint": ["P1.JacketSetpointC"] } }
        ]
    })
}

/// The compiled facts of a loop that Create PLC code wrote (`generated` in `control-studio-generated-loop.test.ts`): the temperature
/// reaches the PID through a 4–20 mA input, the manual output and setpoint have no address, the Auto/Manual signal is TRUE in Manual, and
/// the manual output follows the output in Auto.
fn generated_json() -> Value {
    let mut symbols = vec![
        json!({ "symbol": "Main.TT101_Signal", "type": "LREAL", "address": "%IL8" }),
        json!({ "symbol": "Main.TT101.Value", "type": "LREAL" }),
        json!({ "symbol": "Main.EH101_Power", "type": "LREAL", "address": "%QL8" }),
        json!({ "symbol": "Main.EH101.AutoPower", "type": "LREAL" }),
        json!({ "symbol": "Main.TIC101_Setpoint", "type": "LREAL" }),
        json!({ "symbol": "Main.TIC101_ManualOutput", "type": "LREAL" }),
        json!({ "symbol": "Main.TIC101_Output", "type": "LREAL" }),
        json!({ "symbol": "Main.TIC101_Manual", "type": "BOOL" }),
        json!({ "symbol": "Main.TIC101_Initialized", "type": "BOOL" }),
    ];
    for field in ["Setpoint", "ProcessValue", "ManualOutput", "CycleTime"] {
        symbols.push(json!({ "symbol": format!("Main.TIC101_Input.{field}"), "type": "LREAL" }));
    }
    symbols.extend(pid_symbols("Main.TIC101", "Main.TIC101_Parameters"));
    // Control.PID's own copy of the record its Configure receives.
    for field in PID_PARAMETERS {
        symbols.push(
            json!({ "symbol": format!("Main.TIC101.ParametersValue.{field}"), "type": "LREAL" }),
        );
    }
    json!({
        "symbols": symbols,
        "controllers": [
            { "instance": "Main", "type": "Main", "tasks": task() },
            { "instance": "Main.EH101", "type": "Plant.FB_Heater", "tasks": task() },
            { "instance": "Main.TIC101", "type": "Control.PID", "tasks": task() },
            { "instance": "Main.TT101", "type": "Plant.FB_TemperatureInput", "tasks": task() }
        ],
        "links": [
            { "instance": "Main.TIC101", "method": "Configure", "argument": "Parameters", "record": "Main.TIC101_Parameters", "output": null, "fields": {} },
            { "instance": "Main.TIC101", "method": "Initialize", "argument": "State", "record": "Main.TIC101_Start", "output": null,
              "fields": { "Setpoint": ["Main.TIC101_Setpoint"] }, "guardedBy": "Main.TIC101_Initialized" },
            { "instance": "Main.TIC101", "method": "Update", "argument": "Input", "record": "Main.TIC101_Input", "output": "Main.TIC101_Output",
              "fields": { "ManualOutput": ["Main.TIC101_ManualOutput"], "ProcessValue": ["Main.TT101"], "Setpoint": ["Main.TIC101_Setpoint"] },
              "mode": { "symbol": "Main.TIC101_Manual", "whenTrue": "Manual" },
              "copiesOfOutput": ["Main.TIC101_Start.NominalOutput", "Main.TIC101_ManualOutput", "Main.EH101.AutoPower"] }
        ],
        "writes": ["Main.EH101.AutoPower", "Main.TIC101_Initialized", "Main.TIC101_Input.CycleTime", "Main.TIC101_Input.ManualOutput",
                   "Main.TIC101_Input.ProcessValue", "Main.TIC101_Input.Setpoint", "Main.TIC101_ManualOutput", "Main.TIC101_Output"]
    })
}

const OWN_MEASUREMENT: &str = "Main.TIC101.StateValue.ProcessValue";

#[test]
fn the_generated_loops_links_keep_their_startup_flag_mode_and_output_copies() {
    // control-studio-generated-loop.test.ts: "reads the startup flag, the Auto/Manual signal and the output copies from the compiled links".
    let catalog = decode(generated_json());
    let links = &catalog.links;
    assert_eq!(
        links
            .iter()
            .map(|link| link.method.as_str())
            .collect::<Vec<_>>(),
        vec!["Configure", "Initialize", "Update"]
    );
    assert_eq!(links[0].guarded_by, None);
    assert_eq!(
        links[1].guarded_by.as_deref(),
        Some("Main.TIC101_Initialized")
    );
    assert_eq!(links[1].mode, None);
    assert_eq!(links[2].guarded_by, None);
    assert_eq!(
        links[2].mode,
        Some(ModeEvidence {
            symbol: "Main.TIC101_Manual".to_string(),
            when_true: ModeWhenTrue::Manual,
        })
    );
    assert_eq!(
        links[2].copies_of_output,
        vec![
            "Main.TIC101_Start.NominalOutput",
            "Main.TIC101_ManualOutput",
            "Main.EH101.AutoPower"
        ]
    );
    assert_eq!(
        pid_candidates(&catalog),
        vec![PidCandidate {
            instance: "Main.TIC101".to_string(),
            task: Some(CompiledTask {
                name: "MainTask".to_string(),
                interval_ms: 100.0,
            }),
            parameter_record: Some("Main.TIC101_Parameters".to_string()),
            output: Some("Main.TIC101_Output".to_string()),
            setpoint: Some("Main.TIC101_Setpoint".to_string()),
            process_value: Some("Main.TT101".to_string()),
            manual_output: Some("Main.TIC101_ManualOutput".to_string()),
            own_measurement: Some(OWN_MEASUREMENT.to_string()),
            mode: Some(ModeEvidence {
                symbol: "Main.TIC101_Manual".to_string(),
                when_true: ModeWhenTrue::Manual,
            }),
            startup_flag: Some("Main.TIC101_Initialized".to_string()),
            manual_output_follows_output: true,
        }],
        "the PID's own calls are the evidence, and the manual output follows the output"
    );
    assert_eq!(
        pid_parameter_records(&catalog),
        vec!["Main.TIC101_Parameters"],
        "the PID's own copy is not offered: Configure overwrites it from the program's record every scan"
    );
}

#[test]
fn a_pid_is_prefilled_from_its_own_calls_without_guessing_from_names() {
    // control-studio-loop-setup.test.ts: "prefills each library PID from its compiled calls without guessing from names".
    assert_eq!(
        pid_candidates(&decode(single_loop_json())),
        vec![PidCandidate {
            instance: "P1.Controller".to_string(),
            task: Some(CompiledTask {
                name: "MainTask".to_string(),
                interval_ms: 100.0,
            }),
            parameter_record: Some("P1.Parameters".to_string()),
            output: Some("P1.AppliedPowerPct".to_string()),
            setpoint: Some("P1.SetpointC".to_string()),
            process_value: Some("P1.SafeProcessValueC".to_string()),
            manual_output: Some("P1.ManualOutput".to_string()),
            own_measurement: Some("P1.Controller.StateValue.ProcessValue".to_string()),
            mode: None,
            startup_flag: None,
            manual_output_follows_output: false,
        }],
        "the PID's own measurement is the value its Update received"
    );
}

#[test]
fn a_record_missing_one_field_is_not_offered_and_a_cascade_offers_both_records() {
    // control-studio-loop-setup.test.ts: "offers only complete Control.PID parameter records".
    let mut partial = single_loop_json();
    partial["symbols"]
        .as_array_mut()
        .expect("the symbols are a list")
        .push(json!({ "symbol": "P1.Other.ProportionalGain", "type": "LREAL" }));
    assert_eq!(
        pid_parameter_records(&decode(partial)),
        vec!["P1.Parameters"],
        "a record missing any of the eleven numeric fields is not offered"
    );
    assert_eq!(
        pid_parameter_records(&decode(cascade_json())),
        vec!["P1.InnerParameters", "P1.OuterParameters"]
    );
}

#[test]
fn a_cascade_is_one_pids_output_feeding_anothers_setpoint() {
    // control-studio-loop-setup.test.ts: "recognises a cascade when one PID's output feeds another's setpoint".
    assert_eq!(
        cascade_pairs(&pid_candidates(&decode(cascade_json()))),
        vec![CascadePair {
            outer: "P1.Outer".to_string(),
            inner: "P1.Inner".to_string(),
        }]
    );
    assert_eq!(
        cascade_pairs(&pid_candidates(&decode(single_loop_json()))),
        vec![]
    );
}

#[test]
fn the_entry_limit_counts_every_raw_entry_of_a_list() {
    // The bounded entry limit (A7 and the discovery's limit): exactly the limit decodes, one more is refused, and malformed entries count.
    let at_limit: Vec<Value> = (0..MAXIMUM_CATALOG_ENTRIES)
        .map(|index| json!({ "symbol": format!("P1.V{index}"), "type": "BOOL" }))
        .collect();
    assert_eq!(
        decode(json!({ "symbols": at_limit, "controllers": [] }))
            .symbols
            .len(),
        MAXIMUM_CATALOG_ENTRIES
    );
    let over: Vec<Value> = (0..=MAXIMUM_CATALOG_ENTRIES).map(|_| json!({})).collect();
    assert_eq!(
        refused(&json!({ "ok": true, "controlSymbols": { "symbols": over, "controllers": [] } })),
        "Compiled control-symbol catalog exceeds its bounded entry limit."
    );
}

#[test]
fn the_process_image_limit_applies_to_each_list() {
    let signals = |count: usize, prefix: &str| -> Vec<Value> {
        (0..count)
            .map(|index| json!({ "symbol": format!("{prefix}{index}"), "address": "%IX0.0", "type": "BOOL" }))
            .collect()
    };
    let full = json!({
        "ok": true,
        "processImage": {
            "commands": signals(MAXIMUM_CATALOG_ENTRIES, "C"),
            "feedbacks": signals(MAXIMUM_CATALOG_ENTRIES, "F"),
            "memory": signals(MAXIMUM_CATALOG_ENTRIES, "M")
        },
        "controlSymbols": { "symbols": [], "controllers": [] }
    });
    assert_eq!(
        decode_catalog_answer(&full)
            .expect("three lists at the limit decode")
            .signals
            .len(),
        3 * MAXIMUM_CATALOG_ENTRIES,
        "each list is bounded on its own"
    );
    let over = json!({
        "ok": true,
        "processImage": { "commands": signals(MAXIMUM_CATALOG_ENTRIES + 1, "C") },
        "controlSymbols": { "symbols": [], "controllers": [] }
    });
    assert_eq!(
        refused(&over),
        "Compiled process-image catalog exceeds its bounded entry limit."
    );
}

#[test]
fn the_controller_limit_applies_to_the_controller_list() {
    let controllers: Vec<Value> = (0..=MAXIMUM_CATALOG_ENTRIES)
        .map(|index| json!({ "instance": format!("P{index}"), "type": "Main", "tasks": [] }))
        .collect();
    assert_eq!(
        refused(
            &json!({ "ok": true, "controlSymbols": { "symbols": [], "controllers": controllers } })
        ),
        "Compiled control-symbol catalog exceeds its bounded entry limit."
    );
}

#[test]
fn malformed_entries_are_dropped_and_never_repaired() {
    let catalog = decode(json!({
        "symbols": [
            { "symbol": "P1.A", "type": "LREAL", "address": "" },
            { "symbol": "P1.B" },
            { "symbol": "", "type": "LREAL" },
            "not an object",
            { "symbol": "P1.C", "type": "BOOL", "address": "%MX0.0" }
        ],
        "controllers": [
            { "instance": "P1", "type": "Main", "tasks": [
                { "name": "T", "intervalMs": 100 }, { "name": "", "intervalMs": 5 }, { "intervalMs": 5 }] },
            { "instance": "P2", "type": "Main", "tasks": "not a list" }
        ],
        "links": [
            { "instance": "P1", "method": "Update", "argument": "Input", "record": "R", "output": 5, "fields": {} },
            { "instance": "P1", "method": "Update", "argument": "Input", "record": "R", "output": "P1.Out",
              "fields": { "Setpoint": ["P1.S"], "ManualOutput": ["P1.M", ""] },
              "mode": { "symbol": "P1.Auto", "whenTrue": "Off" },
              "copiesOfOutput": ["P1.M", 3, ""] }
        ],
        "writes": ["P1.B", "P1.A", "P1.A", 7, ""]
    }));
    assert_eq!(
        catalog
            .symbols
            .iter()
            .map(|entry| entry.symbol.as_str())
            .collect::<Vec<_>>(),
        vec!["P1.A", "P1.C"],
        "a symbol without a type, or without a name, is dropped"
    );
    assert_eq!(
        catalog.symbols[0].address, None,
        "an empty address is no address"
    );
    assert_eq!(catalog.symbols[1].address.as_deref(), Some("%MX0.0"));
    assert_eq!(
        catalog.controllers.len(),
        1,
        "P2's tasks are not a list, so P2 is dropped"
    );
    assert_eq!(
        catalog.controllers[0].tasks,
        vec![CompiledTask {
            name: "T".to_string(),
            interval_ms: 100.0,
        }],
        "a task without a name, or without an interval, is dropped and the controller kept"
    );
    assert_eq!(
        catalog.links.len(),
        1,
        "the link whose output is a number is dropped"
    );
    let link = &catalog.links[0];
    assert_eq!(link.output.as_deref(), Some("P1.Out"));
    assert_eq!(
        link.fields.keys().map(String::as_str).collect::<Vec<_>>(),
        vec!["Setpoint"],
        "a field with an empty source is dropped whole"
    );
    assert_eq!(
        link.mode, None,
        "a mode that is neither Auto nor Manual is dropped and the call is kept"
    );
    assert_eq!(link.copies_of_output, vec!["P1.M"]);
    assert_eq!(
        catalog.writes,
        vec!["P1.A", "P1.B"],
        "writes are sorted and without repetition"
    );
}

#[test]
fn a_signal_listed_twice_needs_one_identity() {
    let signal = |symbol: &str, address: &str, kind: &str| json!({ "symbol": symbol, "address": address, "type": kind });
    let answer = |memory: Value| {
        json!({
            "ok": true,
            "processImage": { "commands": [signal("P1.X", "%IX0.0", "BOOL")], "memory": memory },
            "controlSymbols": { "symbols": [], "controllers": [] }
        })
    };
    assert_eq!(
        decode_catalog_answer(&answer(json!([signal("P1.X", "%IX0.0", "bool")])))
            .expect("one identity, listed twice, decodes")
            .signals
            .len(),
        1,
        "the type is compared without regard to case"
    );
    assert_eq!(
        refused(&answer(json!([signal("P1.X", "%IX0.1", "BOOL")]))),
        "Compiled signal P1.X has conflicting identities."
    );
    assert_eq!(
        refused(&answer(json!([signal("P1.X", "%IX0.0", "BYTE")]))),
        "Compiled signal P1.X has conflicting identities."
    );
}

#[test]
fn a_symbol_or_an_instance_with_two_types_is_refused() {
    assert_eq!(
        refused(&json!({ "ok": true, "controlSymbols": { "symbols": [
            { "symbol": "P1.X", "type": "LREAL" }, { "symbol": "P1.X", "type": "BOOL" }], "controllers": [] } })),
        "Compiled symbol P1.X has conflicting types."
    );
    assert_eq!(
        decode(json!({ "symbols": [{ "symbol": "P1.X", "type": "lreal" }, { "symbol": "P1.X", "type": "LREAL" }],
                       "controllers": [] }))
        .symbols
        .len(),
        2,
        "symbol types are compared without regard to case"
    );
    assert_eq!(
        refused(
            &json!({ "ok": true, "controlSymbols": { "symbols": [], "controllers": [
            { "instance": "P1", "type": "Main", "tasks": [] }, { "instance": "P1", "type": "Other", "tasks": [] }] } })
        ),
        "Compiled controller P1 has conflicting types."
    );
    assert_eq!(
        refused(
            &json!({ "ok": true, "controlSymbols": { "symbols": [], "controllers": [
            { "instance": "P1", "type": "Main", "tasks": [] }, { "instance": "P1", "type": "main", "tasks": [] }] } })
        ),
        "Compiled controller P1 has conflicting types.",
        "controller types are compared exactly"
    );
}

#[test]
fn a_compile_failure_is_refused_with_the_compilers_text() {
    assert_eq!(
        refused(&json!({ "ok": false, "error": "The project does not compile." })),
        "The project does not compile."
    );
    assert_eq!(
        refused(&json!({ "ok": false })),
        "Compiled control catalog is unavailable."
    );
    assert_eq!(
        refused(&json!({ "ok": null, "controlSymbols": { "symbols": [], "controllers": [] } })),
        "Compiled control catalog is unavailable.",
        "an ok that is not true refuses the answer"
    );
    assert_eq!(
        refused(&json!([])),
        "Compiled control catalog is unavailable.",
        "an answer that is not an object refuses"
    );
    assert!(
        decode_catalog_answer(&json!({ "controlSymbols": { "symbols": [], "controllers": [] } }))
            .is_ok(),
        "an answer without an ok field is read"
    );
}

#[test]
fn a_catalog_without_its_control_symbol_lists_is_not_compiled() {
    assert_eq!(
        refused(&json!({ "ok": true, "controlSymbols": { "symbols": [] } })),
        "Compile the project successfully before configuring its loop."
    );
    assert_eq!(
        refused(&json!({ "ok": true })),
        "Compile the project successfully before configuring its loop."
    );
}

#[test]
fn the_own_measurement_is_the_pids_process_value_state() {
    assert_eq!(
        own_measurement("P1.Controller"),
        "P1.Controller.StateValue.ProcessValue"
    );
}
