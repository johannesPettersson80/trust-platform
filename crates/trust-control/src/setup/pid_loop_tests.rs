//! Tests of the PID loop generator (`pidLoopGenerator.ts`): the setup assertions of `control-studio-loop-setup.test.ts`,
//! `control-studio-generated-loop.test.ts`, `control-studio-symbolic-signals.test.ts`, `control-studio-library-pid.test.ts` and
//! `control-studio-domain-neutral.test.ts` on their fixtures, closed-form checks of the bounds and the states, and the spec rules
//! that the TypeScript does not hold (C1 Setup: the own measurement is never computed; the startup-done flag is required).

#![allow(clippy::float_cmp)]

use std::collections::BTreeMap;

use serde_json::{json, Value};

use super::{pid_loop_definition, snapshot_age_ms, ModeChoice, PidLoopDraft, SignalChoice};
use crate::commissioning::library_pid::library_pid_initialization;
use crate::failure::Failure;
use crate::json::decode_value;
use crate::loop_definition::{
    Action, ControllerLaw, InitialSource, InitialValue, InitializedState, LoopDefinition,
    LoopScope, ModeWhenTrue, ParameterBinding, PidInitialization, PostScan, RealType, StateType,
};
use crate::setup::catalog::{decode_catalog_answer, CompiledCatalog, PID_PARAMETERS};
use crate::setup::loop_bindings::{FormSignal, MeasuredSignal};
use crate::setup::pid_bindings::{pid_parameters, pid_states, startup_state};

const STATE_FIELDS: [&str; 7] = [
    "Setpoint",
    "SetpointRate",
    "ProcessValue",
    "ProcessValueRate",
    "FeedForward",
    "NominalOutput",
    "SaturatedOutput",
];

const OWN: &str = "Main.TIC101.StateValue.ProcessValue";

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

/// A measured signal that a function takes checked, not as a form draft (`MeasuredSignal`).
fn measured(symbol: &str, unit: &str, minimum: f64, maximum: f64) -> MeasuredSignal {
    MeasuredSignal {
        symbol: symbol.to_string(),
        unit: unit.to_string(),
        minimum,
        maximum,
    }
}

/// The last segment of a parameter's path: the record field.
fn field(symbol: &str) -> String {
    symbol.rsplit('.').next().unwrap_or_default().to_string()
}

#[allow(clippy::needless_pass_by_value)] // A test fixture: it takes the JSON literals its callers write in place.
fn decode(control: Value) -> CompiledCatalog {
    decode_catalog_answer(&json!({ "ok": true, "processImage": {}, "controlSymbols": control }))
        .expect("the compiled answer decodes")
        .control
}

fn task() -> Value {
    json!([{ "name": "MainTask", "intervalMs": 100 }])
}

/// The symbols of one Control.PID: its record fields, and its seven retained states (`pidSymbols`).
fn pid_symbols(instance: &str, record: &str) -> Vec<Value> {
    PID_PARAMETERS
        .iter()
        .map(|field| json!({ "symbol": format!("{record}.{field}"), "type": "LREAL" }))
        .chain(STATE_FIELDS.iter().map(
            |field| json!({ "symbol": format!("{instance}.StateValue.{field}"), "type": "LREAL" }),
        ))
        .collect()
}

/// The single-loop program of `singleLoop` (`control-studio-loop-setup.test.ts`): a heater whose PID runs in `MainTask`.
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
        "writes": ["P1.SafeProcessValueC", "P1.AppliedPowerPct"]
    })
}

fn single_loop() -> CompiledCatalog {
    decode(single_loop_json())
}

/// The heater the TypeScript tests set up on `singleLoop`: a 0–80 °C measurement, a 0–100 % output, an identification step of 20 % for 8 s.
fn heater_draft() -> PidLoopDraft {
    PidLoopDraft {
        name: "Heater temperature".to_string(),
        instance: "P1.Controller".to_string(),
        action: Action::Direct,
        parameter_record: "P1.Parameters".to_string(),
        process_variable: signal("P1.TemperatureC", "degC", 0.0, 80.0),
        manipulated_variable: signal("P1.AppliedPowerPct", "%", 0.0, 100.0),
        setpoint: SignalChoice {
            symbol: "P1.SetpointC".to_string(),
            unit: None,
            minimum: None,
            maximum: None,
        },
        mode: ModeChoice {
            symbol: "P1.AutoMode".to_string(),
            when_true: ModeWhenTrue::Auto,
        },
        manual_output: "P1.ManualOutput".to_string(),
        experiment_step: Some(20.0),
        experiment_duration_seconds: Some(8.0),
        startup_flag: None,
    }
}

/// The compiled facts of a loop that Create PLC code wrote (`generated`, `control-studio-generated-loop.test.ts`): the temperature reaches
/// the PID through a 4–20 mA input, the manual output and the setpoint have no address, the Auto/Manual signal is TRUE in Manual, and the
/// manual output follows the output in Auto.
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

/// The draft the TypeScript tests use on the generated program: the PID's own measurement and the Manual output that follows the output.
fn generated_draft(manual_output: &str) -> PidLoopDraft {
    PidLoopDraft {
        name: "Tank temperature".to_string(),
        instance: "Main.TIC101".to_string(),
        action: Action::Direct,
        parameter_record: "Main.TIC101_Parameters".to_string(),
        process_variable: signal(OWN, "degC", 0.0, 100.0),
        manipulated_variable: signal("Main.TIC101_Output", "%", 0.0, 100.0),
        setpoint: SignalChoice {
            symbol: "Main.TIC101_Setpoint".to_string(),
            unit: None,
            minimum: None,
            maximum: None,
        },
        mode: ModeChoice {
            symbol: "Main.TIC101_Manual".to_string(),
            when_true: ModeWhenTrue::Manual,
        },
        manual_output: manual_output.to_string(),
        experiment_step: None,
        experiment_duration_seconds: None,
        startup_flag: Some("Main.TIC101_Initialized".to_string()),
    }
}

/// The program of the symbolic-signal tests (`catalog` in `control-studio-symbolic-signals.test.ts`): the PID output is an internal
/// variable the program limits onto the heater, and the controller's measurement is computed from the addressed temperature.
fn symbolic_json() -> Value {
    let mut symbols = vec![
        json!({ "symbol": "P1.TemperatureC", "type": "LREAL", "address": "%IL8" }),
        json!({ "symbol": "P1.FilteredC", "type": "LREAL" }),
        json!({ "symbol": "P1.OperatorC", "type": "LREAL" }),
        json!({ "symbol": "P1.ControllerPct", "type": "LREAL" }),
        json!({ "symbol": "P1.HeaterPct", "type": "LREAL", "address": "%QL0" }),
        json!({ "symbol": "P1.SetpointC", "type": "LREAL" }),
        json!({ "symbol": "P1.ManualOutput", "type": "LREAL", "address": "%ML72" }),
        json!({ "symbol": "P1.AutoMode", "type": "BOOL", "address": "%MX96.0" }),
    ];
    for field in PID_PARAMETERS {
        symbols.push(json!({ "symbol": format!("P1.Parameters.{field}"), "type": "LREAL" }));
    }
    for field in STATE_FIELDS {
        symbols.push(
            json!({ "symbol": format!("P1.Controller.StateValue.{field}"), "type": "LREAL" }),
        );
    }
    json!({
        "symbols": symbols,
        "controllers": [
            { "instance": "P1", "type": "Main", "tasks": task() },
            { "instance": "P1.Controller", "type": "Control.PID", "tasks": task() }
        ],
        "links": [],
        "writes": ["P1.ControllerPct", "P1.HeaterPct", "P1.FilteredC"]
    })
}

fn symbolic_draft(process_variable: &str) -> PidLoopDraft {
    PidLoopDraft {
        name: "Heater".to_string(),
        instance: "P1.Controller".to_string(),
        action: Action::Direct,
        parameter_record: "P1.Parameters".to_string(),
        process_variable: signal(process_variable, "degC", 0.0, 100.0),
        manipulated_variable: signal("P1.ControllerPct", "%", 0.0, 100.0),
        setpoint: SignalChoice {
            symbol: "P1.SetpointC".to_string(),
            unit: None,
            minimum: None,
            maximum: None,
        },
        mode: ModeChoice {
            symbol: "P1.AutoMode".to_string(),
            when_true: ModeWhenTrue::Auto,
        },
        manual_output: "P1.ManualOutput".to_string(),
        experiment_step: Some(10.0),
        experiment_duration_seconds: Some(20.0),
        startup_flag: None,
    }
}

/// The two-PID cascade (`cascade` in `control-studio-cascade-fixtures.ts`): the outer output feeds the inner setpoint.
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

#[test]
fn the_bounds_admit_every_value_the_plant_library_pid_pair_starts_with() {
    // control-studio-loop-setup.test.ts: "the bounds setup declares for a Control.PID record admit every value the Plant library's PID
    // pair starts it with". The pair is `libraries/plant/hmi/pairs/plant-pid.toml`, read at compile time.
    let pair = include_str!("../../../../libraries/plant/hmi/pairs/plant-pid.toml");
    let initial: BTreeMap<String, f64> = pair
        .lines()
        .filter_map(|line| {
            let rest = line.strip_prefix("\"{instance}_Parameters.")?;
            let (name, value) = rest.split_once("\" = ")?;
            let number = value.parse::<f64>().ok()?;
            (!name.is_empty() && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_'))
                .then(|| (name.to_string(), number))
        })
        .collect();
    assert!(
        initial.contains_key("RateMinimum") && initial.contains_key("RateMaximum"),
        "the pair sets the rate limits: {initial:?}"
    );
    let parameters = pid_parameters(
        "P1.Parameters",
        &measured("P1.PV", "°C", 0.0, 100.0),
        &measured("P1.MV", "%", 0.0, 100.0),
    );
    for parameter in parameters {
        let name = field(&parameter.symbol);
        if let Some(value) = initial.get(&name) {
            assert!(
                *value >= parameter.minimum && *value <= parameter.maximum,
                "{name} starts at {value}, inside the declared {}..{}, so Validate can carry it",
                parameter.minimum,
                parameter.maximum
            );
        }
    }
}

#[test]
fn the_generated_pid_loop_is_accepted_and_meets_the_contracts_it_is_tuned_by() {
    // control-studio-loop-setup.test.ts: "generates a Control.PID loop that the existing tuning and application contracts accept".
    let loop_definition = pid_loop_definition(&single_loop(), &scope(), &heater_draft())
        .expect("the heater PID is generated");
    loop_definition
        .validate(&scope())
        .expect("the generated loop is accepted");
    let experiment = loop_definition
        .experiment
        .as_ref()
        .expect("the chosen step and duration are the experiment");
    assert_eq!(
        (experiment.amplitude, experiment.duration_seconds),
        (20.0, Some(8.0)),
        "the chosen identification step and duration"
    );
    library_pid_initialization(&loop_definition)
        .expect("all seven retained states are bound with their units and coordinates");
    assert_eq!(loop_definition.controller.type_name, "Control.PID");
    assert_eq!(loop_definition.controller.task_period_ms, 100.0);
    assert_eq!(
        (
            loop_definition.controller.mode_signal.as_deref(),
            loop_definition.controller.mode_when_true,
            loop_definition.controller.manual_output_signal.as_deref(),
            loop_definition.controller.manual_output_address.as_deref(),
        ),
        (
            Some("P1.AutoMode"),
            Some(ModeWhenTrue::Auto),
            Some("P1.ManualOutput"),
            Some("%ML72"),
        )
    );
    let Some(ControllerLaw::Pid(law)) = &loop_definition.controller.law else {
        panic!("a PID loop declares a PID law");
    };
    assert_eq!(law.parameter_record_symbol, "P1.Parameters");
    let binding: &ParameterBinding = loop_definition
        .parameter_binding
        .as_ref()
        .expect("a PID loop binds its record");
    let units: BTreeMap<String, String> = binding
        .parameters
        .iter()
        .map(|parameter| (field(&parameter.symbol), parameter.unit.clone()))
        .collect();
    assert_eq!(
        [
            units["ProportionalGain"].as_str(),
            units["IntegralGain"].as_str(),
            units["DerivativeGain"].as_str(),
            units["RateMaximum"].as_str(),
            units["TrackingTime"].as_str(),
        ],
        ["%/degC", "%/(degC*s)", "%*s/degC", "%/s", "s"]
    );
    assert_eq!(
        (
            loop_definition.signals.process_variable.address.as_deref(),
            loop_definition
                .signals
                .manipulated_variable
                .address
                .as_deref(),
            loop_definition
                .signals
                .setpoint
                .as_ref()
                .and_then(|setpoint| setpoint.address.as_deref()),
        ),
        (Some("%IL8"), Some("%QL0"), Some("%ML0"))
    );
    assert_eq!(
        loop_definition
            .experiment
            .as_ref()
            .map(|experiment| experiment.execution),
        Some(crate::loop_definition::Execution::ControlStudio),
        "Studio's own identification methods drive the plant"
    );
    assert!(
        !binding.mode.manual_value,
        "the Auto signal is FALSE in Manual"
    );
}

#[test]
fn the_output_bias_is_bounded_as_an_offset() {
    // control-studio-loop-setup.test.ts: "bounds the output bias as an offset, so a zero bias stays valid when the output range excludes zero".
    let draft = PidLoopDraft {
        name: "Outer".to_string(),
        manipulated_variable: signal("P1.AppliedPowerPct", "degC", 20.0, 80.0),
        experiment_step: Some(6.0),
        experiment_duration_seconds: Some(30.0),
        ..heater_draft()
    };
    let loop_definition =
        pid_loop_definition(&single_loop(), &scope(), &draft).expect("the outer PID is generated");
    let binding = loop_definition
        .parameter_binding
        .as_ref()
        .expect("a PID loop binds its record");
    let bias = binding
        .parameters
        .iter()
        .find(|parameter| parameter.symbol == "P1.Parameters.Bias")
        .expect("the bias is a record field");
    assert_eq!(
        (bias.minimum, bias.maximum),
        (-60.0, 60.0),
        "the bias may shift the output by at most its span either way"
    );
}

#[test]
fn the_startup_flag_is_an_application_state_that_validation_starts_true() {
    // control-studio-loop-setup.test.ts: "asks for the startup flag when the program writes its own PID parameters, and declares it";
    // control-studio-library-pid.test.ts: "carries declared application startup state alongside the seven library states".
    let mut value = single_loop_json();
    value["symbols"]
        .as_array_mut()
        .expect("the symbols are a list")
        .push(json!({ "symbol": "P1.Initialized", "type": "BOOL" }));
    value["writes"] = json!([
        "P1.AppliedPowerPct",
        "P1.Initialized",
        "P1.Parameters.IntegralGain",
        "P1.Parameters.ProportionalGain"
    ]);
    let catalog = decode(value);
    let draft = PidLoopDraft {
        startup_flag: Some("P1.Initialized".to_string()),
        experiment_step: Some(10.0),
        experiment_duration_seconds: Some(60.0),
        ..heater_draft()
    };
    let loop_definition = pid_loop_definition(&catalog, &scope(), &draft)
        .expect("the flag is declared, so the program's writes are allowed");
    loop_definition
        .validate(&scope())
        .expect("the loop with a startup flag is accepted");
    library_pid_initialization(&loop_definition).expect("the library states are bound");
    let Some(PidInitialization::ControlPid {
        application_states, ..
    }) = &loop_definition.controller.pid_initialization
    else {
        panic!("a PID loop declares the control-pid-v1 initialization");
    };
    assert_eq!(
        application_states,
        &Some(vec![InitializedState {
            symbol: "P1.Initialized".to_string(),
            iec_type: StateType::Bool,
            unit: "1".to_string(),
            minimum: 0.0,
            maximum: 1.0,
            initial: InitialValue::Constant(1.0),
            post_scan: PostScan::Unchanged,
        }]),
        "validation starts with the program's startup already done, so its startup code keeps the candidate"
    );
    assert_eq!(
        startup_state("P1.Initialized"),
        application_states.as_ref().expect("one state")[0]
    );
}

#[test]
fn a_startup_flag_that_is_not_a_bool_is_refused_for_a_pid_loop() {
    // C1 Setup: the startup-done flag is a BOOL the program sets. A REAL variable named as the flag is refused under the form's field.
    let catalog = decode(generated_json());
    let mut draft = generated_draft("Main.TIC101_ManualOutput");
    draft.startup_flag = Some("Main.EH101_Power".to_string());
    let failure = pid_loop_definition(&catalog, &scope(), &draft)
        .expect_err("a REAL variable is not a startup-done flag");
    assert_eq!(
        failure.message,
        "Main.EH101_Power is not a compiled BOOL symbol."
    );
    assert_eq!(failure.field.as_deref(), Some("startupFlag"));
}

#[test]
fn a_program_that_writes_its_parameter_record_needs_the_startup_flag() {
    // C1 Setup: "When the program writes one of the loop's seeded values itself ... setup requires the BOOL that marks the startup as
    // done." The TypeScript form enforced it in the webview; the generator now refuses a draft that names no flag.
    let mut value = single_loop_json();
    value["writes"] = json!([
        "P1.AppliedPowerPct",
        "P1.Parameters.IntegralGain",
        "P1.Parameters.ProportionalGain"
    ]);
    let failure = pid_loop_definition(&decode(value), &scope(), &heater_draft())
        .expect_err("a written parameter needs the startup-done flag");
    assert_eq!(
        text_of(&failure),
        "The program writes P1.Parameters.IntegralGain in its own code. Choose the flag that marks its startup as done, so validation keeps the values it starts from."
    );
}

#[test]
fn a_program_that_initializes_the_pid_needs_the_startup_flag() {
    // C1 Setup: "or calls Initialize on the loop's PID, setup requires the BOOL that marks the startup as done."
    let mut value = single_loop_json();
    value["symbols"]
        .as_array_mut()
        .expect("the symbols are a list")
        .push(json!({ "symbol": "P1.Initialized", "type": "BOOL" }));
    value["links"]
        .as_array_mut()
        .expect("the links are a list")
        .push(json!({ "instance": "P1.Controller", "method": "Initialize", "argument": "State", "record": "P1.State",
                      "output": null, "fields": {}, "guardedBy": "P1.Initialized" }));
    let catalog = decode(value);
    let failure = pid_loop_definition(&catalog, &scope(), &heater_draft())
        .expect_err("an initialized PID needs the startup-done flag");
    assert_eq!(
        text_of(&failure),
        "The program initializes P1.Controller in its own code. Choose the flag that marks its startup as done, so validation keeps the values it starts from."
    );
    let flagged = PidLoopDraft {
        startup_flag: Some("P1.Initialized".to_string()),
        ..heater_draft()
    };
    pid_loop_definition(&catalog, &scope(), &flagged).expect("with the flag the loop is generated");
}

#[test]
fn a_program_that_writes_no_seeded_value_needs_no_startup_flag() {
    // control-studio-symbolic-signals.test.ts: the program writes its controller output and its measurement, not a seeded value.
    let loop_definition = pid_loop_definition(
        &decode(symbolic_json()),
        &scope(),
        &symbolic_draft("P1.OperatorC"),
    )
    .expect("no seeded value is written, so no flag is needed");
    loop_definition
        .validate(&scope())
        .expect("the symbolic loop is accepted");
}

#[test]
fn the_generated_loop_on_the_pids_own_measurement_follows_its_output() {
    // control-studio-generated-loop.test.ts: "generates a loop on the PID's own measurement whose manual output follows the output".
    let catalog = decode(generated_json());
    let loop_definition = pid_loop_definition(
        &catalog,
        &scope(),
        &generated_draft("Main.TIC101_ManualOutput"),
    )
    .expect("the loop on the PID's own measurement is generated");
    loop_definition
        .validate(&scope())
        .expect("the generated loop is accepted");
    assert_eq!(loop_definition.signals.process_variable.symbol, OWN);
    assert_eq!(loop_definition.signals.process_variable.address, None);
    assert_eq!(
        (
            loop_definition.controller.manual_output_signal.as_deref(),
            loop_definition.controller.manual_output_address.as_deref(),
            loop_definition.controller.manual_output_follows_output,
        ),
        (Some("Main.TIC101_ManualOutput"), None, Some(true))
    );
    let binding = loop_definition
        .parameter_binding
        .as_ref()
        .expect("a PID loop binds its record");
    assert!(binding.mode.manual_value, "TIC101_Manual is TRUE in Manual");
    let elsewhere =
        pid_loop_definition(&catalog, &scope(), &generated_draft("Main.TIC101_Setpoint"))
            .expect("the other manual output is generated");
    assert_eq!(
        elsewhere.controller.manual_output_follows_output, None,
        "only the variable the code copies the output into follows it"
    );
}

#[test]
fn the_pids_own_measurement_is_never_refused_as_computed() {
    // C1 Setup: "The measurement ... is never refused as computed." The TypeScript refused a written measurement; the own measurement is
    // the one signal the program cannot compute for Studio, so it is read whatever the program writes.
    let mut value = generated_json();
    value["writes"]
        .as_array_mut()
        .expect("the writes are a list")
        .push(json!(OWN));
    let loop_definition = pid_loop_definition(
        &decode(value),
        &scope(),
        &generated_draft("Main.TIC101_ManualOutput"),
    )
    .expect("the own measurement is accepted even though the program writes it");
    assert_eq!(loop_definition.signals.process_variable.symbol, OWN);
    assert_eq!(
        loop_definition.signals.process_variable.direction,
        crate::loop_definition::Direction::Read
    );
}

#[test]
fn a_program_computed_measurement_is_refused_and_a_read_one_is_accepted() {
    // control-studio-symbolic-signals.test.ts: "sets up a loop on a controller output and setpoint that have no I/O address", "refuses a
    // measurement the program computes, which a simulated measurement could not reach", and "accepts a measurement without an address".
    let catalog = decode(symbolic_json());
    let loop_definition =
        pid_loop_definition(&catalog, &scope(), &symbolic_draft("P1.TemperatureC"))
            .expect("the output and the setpoint have no address and are generated");
    loop_definition
        .validate(&scope())
        .expect("the loop is accepted");
    assert_eq!(
        (
            loop_definition.signals.manipulated_variable.symbol.as_str(),
            loop_definition
                .signals
                .manipulated_variable
                .address
                .as_deref(),
            loop_definition
                .signals
                .setpoint
                .as_ref()
                .and_then(|setpoint| setpoint.address.as_deref()),
        ),
        ("P1.ControllerPct", None, None)
    );
    let failure = pid_loop_definition(&catalog, &scope(), &symbolic_draft("P1.FilteredC"))
        .expect_err("a computed measurement is refused");
    let text = text_of(&failure);
    assert!(
        text.starts_with("P1.FilteredC is computed by the program")
            && text.contains("overwritten")
            && text.contains("measurement it is computed from"),
        "{text}"
    );
    let read = pid_loop_definition(&catalog, &scope(), &symbolic_draft("P1.OperatorC"))
        .expect("a variable the program only reads is accepted");
    read.validate(&scope()).expect("the loop is accepted");
    assert_eq!(read.signals.process_variable.address, None);
}

#[test]
fn the_inner_loop_of_a_two_file_cascade_is_generated_and_accepted() {
    // control-studio-loop-setup.test.ts: "validates the inner loop of a cascade declared as two loop files with the outer PID held in
    // Manual at the target". Only the generator is ported; the verification plan is V3.
    let draft = PidLoopDraft {
        name: "Vessel temperature · inner loop".to_string(),
        instance: "P1.Inner".to_string(),
        parameter_record: "P1.InnerParameters".to_string(),
        process_variable: signal("P1.JacketC", "degC", 0.0, 90.0),
        manipulated_variable: signal("P1.HeaterPct", "%", 0.0, 100.0),
        setpoint: SignalChoice {
            symbol: "P1.OuterManual".to_string(),
            unit: None,
            minimum: None,
            maximum: None,
        },
        mode: ModeChoice {
            symbol: "P1.InnerAuto".to_string(),
            when_true: ModeWhenTrue::Auto,
        },
        manual_output: "P1.InnerManual".to_string(),
        experiment_step: Some(10.0),
        experiment_duration_seconds: Some(3.0),
        ..heater_draft()
    };
    let loop_definition = pid_loop_definition(&decode(cascade_json()), &scope(), &draft)
        .expect("the inner loop is generated");
    loop_definition
        .validate(&scope())
        .expect("the inner loop is accepted");
    assert_eq!(loop_definition.id, "loop:vessel-temperature-inner-loop");
}

#[test]
fn the_setup_writes_no_experiment_the_engineer_did_not_set() {
    // control-studio-loop-setup.test.ts: "sets up a loop without an identification step or duration, leaving them to Identify and Limits (B1.3)".
    let draft = PidLoopDraft {
        experiment_step: None,
        experiment_duration_seconds: None,
        ..heater_draft()
    };
    let loop_definition =
        pid_loop_definition(&single_loop(), &scope(), &draft).expect("the loop is generated");
    assert!(
        loop_definition.experiment.is_none(),
        "setup writes no experiment values the engineer did not set"
    );
}

#[test]
fn the_pid_generator_refuses_a_non_pid_several_tasks_and_a_boolean_manual_output() {
    // pidLoopGenerator.ts: the instance is a compiled Control.PID in exactly one cyclic task, and the manual output is REAL or LREAL.
    let not_a_pid = PidLoopDraft {
        instance: "P1".to_string(),
        ..heater_draft()
    };
    let failure = pid_loop_definition(&single_loop(), &scope(), &not_a_pid)
        .expect_err("P1 is the program, not a PID");
    assert_eq!(
        text_of(&failure),
        "P1 is not a compiled Control.PID instance."
    );

    let mut two_tasks = single_loop_json();
    two_tasks["controllers"][1]["tasks"] = json!([
        { "name": "MainTask", "intervalMs": 100 },
        { "name": "Fast", "intervalMs": 10 }
    ]);
    let failure = pid_loop_definition(&decode(two_tasks), &scope(), &heater_draft())
        .expect_err("the PID runs in two tasks");
    assert_eq!(
        text_of(&failure),
        "P1.Controller must run in exactly one cyclic task."
    );

    let boolean_output = PidLoopDraft {
        manual_output: "P1.AutoMode".to_string(),
        ..heater_draft()
    };
    let failure = pid_loop_definition(&single_loop(), &scope(), &boolean_output)
        .expect_err("a BOOL manual output is refused");
    assert_eq!(text_of(&failure), "P1.AutoMode must be REAL or LREAL.");
}

#[test]
fn the_pid_parameters_follow_the_control_pid_bounds() {
    // pidBindings.ts `pidParameters`: closed-form bounds for a 0–80 °C measurement and a 0–100 % output (C0).
    let parameters = pid_parameters(
        "P1.Parameters",
        &measured("P1.TemperatureC", "degC", 0.0, 80.0),
        &measured("P1.AppliedPowerPct", "%", 0.0, 100.0),
    );
    let expected = [
        ("ProportionalGain", "%/degC", -1e6, 1e6),
        ("IntegralGain", "%/(degC*s)", -1e6, 1e6),
        ("DerivativeGain", "%*s/degC", -1e6, 1e6),
        ("ProportionalSetpointWeight", "1", 0.0, 1.0),
        ("DerivativeSetpointWeight", "1", 0.0, 1.0),
        ("Bias", "%", -100.0, 100.0),
        ("OutputMinimum", "%", 0.0, 100.0),
        ("OutputMaximum", "%", 0.0, 100.0),
        ("RateMinimum", "%/s", -1e9, 0.0),
        ("RateMaximum", "%/s", 0.0, 1e9),
        ("TrackingTime", "s", 0.001, 1e4),
    ];
    assert_eq!(parameters.len(), expected.len());
    for (parameter, (name, unit, minimum, maximum)) in parameters.iter().zip(expected) {
        assert_eq!(parameter.symbol, format!("P1.Parameters.{name}"));
        assert_eq!(parameter.iec_type, RealType::Lreal);
        assert_eq!(parameter.unit, unit, "{name}");
        assert_eq!(parameter.convention, format!("Control.PID {name}"));
        assert_eq!(
            (parameter.minimum, parameter.maximum),
            (minimum, maximum),
            "{name}"
        );
    }
}

#[test]
fn the_pid_states_are_the_seven_retained_states_at_the_operating_point() {
    // pidBindings.ts `pidStates`: the setpoint and the measurement start at the model output, the two outputs at the model input, the
    // rates and the feedforward at zero, and the rates and the nominal output span the signal spans (C1).
    let states = pid_states(
        "P1.Controller",
        &measured("P1.TemperatureC", "degC", 0.0, 80.0),
        &measured("P1.AppliedPowerPct", "%", 0.0, 100.0),
        0,
    );
    let output = |index| InitialValue::From(InitialSource::ModelOutput { index });
    let input = |index| InitialValue::From(InitialSource::ModelInput { index });
    let expected = [
        ("Setpoint", "degC", 0.0, 80.0, output(0)),
        (
            "SetpointRate",
            "degC/s",
            -80_000.0,
            80_000.0,
            InitialValue::Constant(0.0),
        ),
        ("ProcessValue", "degC", 0.0, 80.0, output(0)),
        (
            "ProcessValueRate",
            "degC/s",
            -80_000.0,
            80_000.0,
            InitialValue::Constant(0.0),
        ),
        (
            "FeedForward",
            "%",
            -100.0,
            100.0,
            InitialValue::Constant(0.0),
        ),
        ("NominalOutput", "%", -1000.0, 1100.0, input(0)),
        ("SaturatedOutput", "%", 0.0, 100.0, input(0)),
    ];
    assert_eq!(states.len(), 7);
    for (state, (name, unit, minimum, maximum, initial)) in states.iter().zip(expected) {
        assert_eq!(state.symbol, format!("P1.Controller.StateValue.{name}"));
        assert_eq!(state.iec_type, StateType::Lreal);
        assert_eq!(state.unit, unit, "{name}");
        assert_eq!((state.minimum, state.maximum), (minimum, maximum), "{name}");
        assert_eq!(state.initial, initial, "{name}");
        assert_eq!(state.post_scan, PostScan::Bounded, "{name}");
    }
    assert_eq!(
        states
            .iter()
            .map(|state| field(&state.symbol))
            .collect::<Vec<_>>(),
        STATE_FIELDS
            .iter()
            .map(|name| (*name).to_string())
            .collect::<Vec<_>>()
    );
}

#[test]
fn the_snapshot_age_is_five_periods_at_least_half_a_second_in_whole_milliseconds() {
    // pidLoopGenerator.ts: Math.max(500, 5 * period), rounded up to whole ms (validation takes an integer from 1 period to 10 s).
    assert_eq!(snapshot_age_ms(100.0), 500);
    assert_eq!(snapshot_age_ms(1_000.0), 5_000);
    assert_eq!(snapshot_age_ms(100.5), 503, "502.5 ms rounded up");
    assert_eq!(
        snapshot_age_ms(2_000.0),
        10_000,
        "the longest age validation accepts"
    );
    assert_eq!(snapshot_age_ms(2_001.0), 10_005);
}

#[test]
fn a_task_slower_than_two_seconds_gives_an_age_the_loop_refuses() {
    // The age would be above the 10 s bound that validation and libraryPidInitialization.ts both hold.
    let mut value = single_loop_json();
    value["controllers"][1]["tasks"] = json!([{ "name": "MainTask", "intervalMs": 2_001 }]);
    let loop_definition = pid_loop_definition(&decode(value), &scope(), &heater_draft())
        .expect("the generator writes the age the formula gives");
    loop_definition
        .validate(&scope())
        .expect_err("a snapshot age above 10 s is refused");
}

#[test]
fn a_dimensionless_signal_entered_as_one_keeps_its_unit_as_one() {
    // The form asks for a unit, and a dimensionless signal's unit is entered as `1`: the parameters and the states say `1`, as the
    // loop reader writes it (C0 needs a unit).
    let draft = PidLoopDraft {
        process_variable: signal("P1.TemperatureC", "1", 0.0, 80.0),
        ..heater_draft()
    };
    let loop_definition =
        pid_loop_definition(&single_loop(), &scope(), &draft).expect("the loop is generated");
    loop_definition
        .validate(&scope())
        .expect("the dimensionless loop is accepted");
    assert_eq!(loop_definition.signals.process_variable.unit, "1");
    let binding = loop_definition
        .parameter_binding
        .as_ref()
        .expect("a PID loop binds its record");
    assert_eq!(binding.parameters[0].unit, "%/1");
}

#[test]
fn the_generated_loop_survives_the_loop_file() {
    // The loop file the engine writes is the loop the generator made, and the reader accepts it unchanged.
    let loop_definition = pid_loop_definition(&single_loop(), &scope(), &heater_draft())
        .expect("the loop is generated");
    let text = serde_json::to_string(&loop_definition).expect("the loop serializes");
    let decoded = LoopDefinition::decode(&text).expect("the loop file is read");
    assert_eq!(decoded, loop_definition);
}

#[test]
fn the_draft_reads_the_typescript_field_names() {
    // The engine decodes the form's draft with the names the TypeScript form sends (camelCase, `whenTrue`).
    let value = json!({
        "name": "Heater temperature",
        "instance": "P1.Controller",
        "action": "direct",
        "parameterRecord": "P1.Parameters",
        "processVariable": { "symbol": "P1.TemperatureC", "unit": "degC", "minimum": 0, "maximum": 80 },
        "manipulatedVariable": { "symbol": "P1.AppliedPowerPct", "unit": "%", "minimum": 0, "maximum": 100 },
        "setpoint": { "symbol": "P1.SetpointC" },
        "mode": { "symbol": "P1.AutoMode", "whenTrue": "Auto" },
        "manualOutput": "P1.ManualOutput",
        "experimentStep": 20,
        "experimentDurationSeconds": 8,
        "startupFlag": null
    });
    let draft: PidLoopDraft = decode_value(value.clone()).expect("the draft decodes");
    assert_eq!(draft, heater_draft());
    let mut unknown = value;
    unknown["extra"] = json!(1);
    assert!(
        decode_value::<PidLoopDraft>(unknown).is_err(),
        "a field the draft does not declare is refused"
    );
}
