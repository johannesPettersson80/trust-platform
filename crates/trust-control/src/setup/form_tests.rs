//! The setup form's model, ported from `control-studio-loop-setup.test.ts` (the form-model assertions: the structure, the signal
//! lists, the modes and booleans, the writes and initializes, and the prefills) and `control-studio-loop-setup.test.ts`'s cascade
//! startup test. The generator assertions of that file belong to the PID generators (`pid_loop_tests.rs`), and its render assertion
//! (JSDOM) is presentation, not ported.

use super::{setup_form_model, PidPrefill};
use crate::setup::catalog::{
    CallLink, CascadePair, CompiledCatalog, CompiledController, CompiledSymbol, CompiledTask,
    PidCandidate,
};

/// The parameter fields of a `Control.PID` record, in the order the `TypeScript` fixtures declare them (`PARAMETER_FIELDS`).
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

/// The state fields of a library PID (`STATE_FIELDS`).
const STATES: [&str; 7] = [
    "Setpoint",
    "SetpointRate",
    "ProcessValue",
    "ProcessValueRate",
    "FeedForward",
    "NominalOutput",
    "SaturatedOutput",
];

fn symbol(name: &str, iec_type: &str, address: Option<&str>) -> CompiledSymbol {
    CompiledSymbol {
        symbol: name.to_string(),
        iec_type: iec_type.to_string(),
        address: address.map(str::to_string),
    }
}

/// The compiled symbols of one library PID's record and its states (`pidSymbols`).
fn pid_symbols(instance: &str, record: &str) -> Vec<CompiledSymbol> {
    PARAMETERS
        .iter()
        .map(|field| symbol(&format!("{record}.{field}"), "LREAL", None))
        .chain(
            STATES
                .iter()
                .map(|field| symbol(&format!("{instance}.StateValue.{field}"), "LREAL", None)),
        )
        .collect()
}

/// A compiled controller that runs in `MainTask` every 100 ms.
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

/// A compiled call: `instance.method(argument := record)` whose output is `output`, and whose fields read the named symbols.
fn call(
    instance: &str,
    method: &str,
    argument: &str,
    record: &str,
    output: Option<&str>,
    fields: &[(&str, &str)],
) -> CallLink {
    CallLink {
        instance: instance.to_string(),
        method: method.to_string(),
        argument: argument.to_string(),
        record: record.to_string(),
        output: output.map(str::to_string),
        fields: fields
            .iter()
            .map(|(field, symbol)| ((*field).to_string(), vec![(*symbol).to_string()]))
            .collect(),
        guarded_by: None,
        mode: None,
        copies_of_output: Vec::new(),
    }
}

fn strings(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| (*value).to_string()).collect()
}

/// The single-loop program of the `TypeScript` fixture: one library PID, its record, its measurement computed by the program.
fn single_loop() -> CompiledCatalog {
    let mut symbols = vec![
        symbol("P1.TemperatureC", "LREAL", Some("%IL8")),
        symbol("P1.AppliedPowerPct", "LREAL", Some("%QL0")),
        symbol("P1.SetpointC", "LREAL", Some("%ML0")),
        symbol("P1.AutoMode", "BOOL", Some("%MX96.0")),
        symbol("P1.ManualOutput", "LREAL", Some("%ML72")),
        symbol("P1.SafeProcessValueC", "LREAL", None),
    ];
    symbols.extend(pid_symbols("P1.Controller", "P1.Parameters"));
    CompiledCatalog {
        symbols,
        controllers: vec![
            controller("P1", "Main"),
            controller("P1.Controller", "Control.PID"),
        ],
        links: vec![
            call(
                "P1.Controller",
                "Configure",
                "Parameters",
                "P1.Parameters",
                None,
                &[],
            ),
            call(
                "P1.Controller",
                "Update",
                "Input",
                "P1.Input",
                Some("P1.AppliedPowerPct"),
                &[
                    ("ManualOutput", "P1.ManualOutput"),
                    ("ProcessValue", "P1.SafeProcessValueC"),
                    ("Setpoint", "P1.SetpointC"),
                    ("TrackingOutput", "P1.ManualOutput"),
                ],
            ),
        ],
        // The program computes the checked measurement from the temperature and writes the PID output.
        writes: strings(&["P1.SafeProcessValueC", "P1.AppliedPowerPct"]),
    }
}

/// The two-PID cascade of the `TypeScript` fixture: the outer output is the inner setpoint.
fn cascade() -> CompiledCatalog {
    let mut symbols = vec![
        symbol("P1.VesselC", "LREAL", Some("%IL0")),
        symbol("P1.JacketC", "LREAL", Some("%IL8")),
        symbol("P1.VesselSetpointC", "LREAL", Some("%ML0")),
        symbol("P1.JacketSetpointC", "LREAL", Some("%ML8")),
        symbol("P1.HeaterPct", "LREAL", Some("%QL0")),
        symbol("P1.OuterAuto", "BOOL", Some("%MX40.0")),
        symbol("P1.InnerAuto", "BOOL", Some("%MX40.1")),
        symbol("P1.OuterManual", "LREAL", Some("%ML16")),
        symbol("P1.InnerManual", "LREAL", Some("%ML24")),
    ];
    symbols.extend(pid_symbols("P1.Outer", "P1.OuterParameters"));
    symbols.extend(pid_symbols("P1.Inner", "P1.InnerParameters"));
    CompiledCatalog {
        symbols,
        controllers: vec![
            controller("P1", "Main"),
            controller("P1.Inner", "Control.PID"),
            controller("P1.Outer", "Control.PID"),
        ],
        links: vec![
            call(
                "P1.Outer",
                "Configure",
                "Parameters",
                "P1.OuterParameters",
                None,
                &[],
            ),
            call(
                "P1.Inner",
                "Configure",
                "Parameters",
                "P1.InnerParameters",
                None,
                &[],
            ),
            call(
                "P1.Outer",
                "Update",
                "Input",
                "P1.OuterInput",
                Some("P1.JacketSetpointC"),
                &[
                    ("ManualOutput", "P1.OuterManual"),
                    ("ProcessValue", "P1.VesselC"),
                    ("Setpoint", "P1.VesselSetpointC"),
                ],
            ),
            call(
                "P1.Inner",
                "Update",
                "Input",
                "P1.InnerInput",
                Some("P1.HeaterPct"),
                &[
                    ("ManualOutput", "P1.InnerManual"),
                    ("ProcessValue", "P1.JacketC"),
                    ("Setpoint", "P1.JacketSetpointC"),
                ],
            ),
        ],
        writes: Vec::new(),
    }
}

/// The prefill of a PID whose record, output, setpoint, measurement and manual output the form offers, and nothing else.
fn expected_prefill(
    instance: &str,
    record: &str,
    manipulated: &str,
    setpoint: &str,
    process_value: &str,
    manual: &str,
) -> PidPrefill {
    PidPrefill {
        instance: instance.to_string(),
        parameter_record: Some(record.to_string()),
        manipulated_variable: Some(manipulated.to_string()),
        setpoint: Some(setpoint.to_string()),
        process_value: Some(process_value.to_string()),
        manual_output: Some(manual.to_string()),
        mode: None,
        mode_when_true: None,
        startup_flag: None,
        computed: None,
    }
}

#[test]
fn a_single_pid_program_opens_the_pid_structure_with_only_the_usable_choices() {
    // control-studio-loop-setup.test.ts: "opens the setup form on the structure the compiled program shows, with only usable choices".
    let single = setup_form_model(&single_loop());
    assert_eq!(single.structure, "pid");
    assert_eq!(
        single
            .pids
            .iter()
            .map(|pid| pid.instance.as_str())
            .collect::<Vec<_>>(),
        ["P1.Controller"]
    );
    assert_eq!(
        single.addressed_real,
        strings(&[
            "P1.AppliedPowerPct",
            "P1.ManualOutput",
            "P1.SetpointC",
            "P1.TemperatureC",
        ]),
        "the REAL/LREAL signals with an I/O address"
    );
    assert_eq!(
        single.signals,
        strings(&[
            "P1.AppliedPowerPct",
            "P1.ManualOutput",
            "P1.SafeProcessValueC",
            "P1.SetpointC",
            "P1.TemperatureC",
        ]),
        "outputs and setpoints may be program variables without an address; the PID's own records and states are not signals"
    );
    assert_eq!(
        single.measurements,
        strings(&["P1.Controller.StateValue.ProcessValue", "P1.TemperatureC"]),
        "a simulated plant writes inputs and program variables the program only reads, and gives the PID its own measurement; the program computes P1.SafeProcessValueC itself"
    );
    assert_eq!(single.booleans, strings(&["P1.AutoMode"]));
    assert_eq!(single.records, strings(&["P1.Parameters"]));
    assert_eq!(
        single.instances,
        strings(&["P1"]),
        "a non-PID task instance is offered as a program instance"
    );
    assert_eq!(
        single.prefill,
        Some(expected_prefill(
            "P1.Controller",
            "P1.Parameters",
            "P1.AppliedPowerPct",
            "P1.SetpointC",
            "P1.Controller.StateValue.ProcessValue",
            "P1.ManualOutput",
        )),
        "the PID's own measurement is prefilled: what its Update received, whatever the program computed it from"
    );
}

#[test]
fn a_program_of_two_pids_whose_outputs_chain_opens_the_cascade_structure() {
    // control-studio-loop-setup.test.ts: the cascade part of "opens the setup form on the structure the compiled program shows".
    let pair = setup_form_model(&cascade());
    assert_eq!(pair.structure, "cascade");
    assert_eq!(
        pair.cascade,
        Some(CascadePair {
            outer: "P1.Outer".to_string(),
            inner: "P1.Inner".to_string(),
        })
    );
    assert_eq!(
        pair.records,
        strings(&["P1.InnerParameters", "P1.OuterParameters"]),
        "the records of both complete library PIDs, in name order"
    );
    assert_eq!(pair.prefill, None, "a cascade has no single PID prefill");
}

#[test]
fn a_program_without_a_library_pid_opens_the_identify_structure() {
    // control-studio-loop-setup.test.ts: "a program without a library PID is set up for identification".
    let mut catalog = single_loop();
    catalog.symbols.truncate(5);
    catalog.controllers.clear();
    catalog.links.clear();
    let model = setup_form_model(&catalog);
    assert_eq!(model.structure, "identify");
    assert_eq!(model.pids, [] as [PidCandidate; 0]);
    assert_eq!(model.prefills, [] as [PidPrefill; 0]);
    assert_eq!(model.prefill, None);
}

#[test]
fn an_auto_manual_signal_is_a_bool_without_an_address_and_booleans_keep_every_bool() {
    // control-studio-loop-setup.test.ts: "offers as Auto/Manual signal only BOOLs without an I/O address, which Apply can switch".
    // The render assertion (JSDOM over loopSetupTemplate.ts) is presentation and is not ported.
    let mut catalog = single_loop();
    catalog.symbols.push(symbol("P1.Enabled", "BOOL", None));
    let model = setup_form_model(&catalog);
    assert_eq!(
        model.modes,
        strings(&["P1.Enabled"]),
        "the controller transfer switches only an unaddressed mode (controller-transfer contract)"
    );
    assert_eq!(
        model.booleans,
        strings(&["P1.AutoMode", "P1.Enabled"]),
        "a startup flag or a switching state may be addressed"
    );
}

#[test]
fn the_writes_keep_the_compiler_order_and_the_pid_outputs_are_not_measurements() {
    // control-studio-loop-setup.test.ts: "asks for the startup flag when the program writes its own PID parameters, and declares it".
    let mut writes = single_loop();
    writes.symbols.push(symbol("P1.Initialized", "BOOL", None));
    writes.writes = strings(&[
        "P1.AppliedPowerPct",
        "P1.Initialized",
        "P1.Parameters.IntegralGain",
        "P1.Parameters.ProportionalGain",
    ]);
    assert_eq!(
        setup_form_model(&writes).writes,
        strings(&[
            "P1.AppliedPowerPct",
            "P1.Initialized",
            "P1.Parameters.IntegralGain",
            "P1.Parameters.ProportionalGain",
        ]),
        "a program that writes its own parameter record would overwrite a validated candidate"
    );
    assert_eq!(
        setup_form_model(&single_loop()).writes,
        strings(&["P1.SafeProcessValueC", "P1.AppliedPowerPct"]),
        "neither the PID output nor the measurement the program computes is a value validation seeds"
    );
}

#[test]
fn a_cascade_whose_program_initializes_its_controller_reports_the_writes_and_the_initialized_pid() {
    // control-studio-loop-setup.test.ts: "declares the startup flag on a cascade whose program initializes its controllers" (the form part).
    let mut initializing = cascade();
    initializing
        .symbols
        .push(symbol("P1.Initialized", "BOOL", None));
    initializing.writes = strings(&[
        "P1.HeaterPct",
        "P1.Initialized",
        "P1.JacketSetpointC",
        "P1.OuterManual",
    ]);
    initializing.links.push(call(
        "P1.Outer",
        "Initialize",
        "State",
        "P1.State",
        None,
        &[],
    ));
    let model = setup_form_model(&initializing);
    assert_eq!(
        model.writes,
        strings(&[
            "P1.HeaterPct",
            "P1.Initialized",
            "P1.JacketSetpointC",
            "P1.OuterManual",
        ]),
        "startup code that sets a manual output or initializes a controller replaces the state validation starts from"
    );
    assert_eq!(model.initializes, strings(&["P1.Outer"]));
}

#[test]
fn every_library_pid_is_prefilled_from_its_own_calls_so_any_of_several_controllers_can_be_chosen() {
    // control-studio-loop-setup.test.ts: "prefills every library PID from its own calls, so any of several controllers can be chosen".
    let model = setup_form_model(&cascade());
    assert_eq!(
        model.prefills,
        vec![
            expected_prefill(
                "P1.Inner",
                "P1.InnerParameters",
                "P1.HeaterPct",
                "P1.JacketSetpointC",
                "P1.Inner.StateValue.ProcessValue",
                "P1.InnerManual",
            ),
            expected_prefill(
                "P1.Outer",
                "P1.OuterParameters",
                "P1.JacketSetpointC",
                "P1.VesselSetpointC",
                "P1.Outer.StateValue.ProcessValue",
                "P1.OuterManual",
            ),
        ],
        "each controller's record, signals and manual output come from its own Configure and Update calls"
    );
    assert_eq!(model.prefill, None, "a cascade preselects no single PID");
}

#[test]
fn a_manual_output_of_a_pid_is_offered_when_the_form_lists_its_signal() {
    // The manual output of each PID is offered by name, with no address needed (Specification 35, §17.1): the form lists the REAL
    // and the prefill names it. Here the manual output has no address, and the prefill still offers it.
    let mut catalog = single_loop();
    for entry in &mut catalog.symbols {
        if entry.symbol == "P1.ManualOutput" {
            entry.address = None;
        }
    }
    let model = setup_form_model(&catalog);
    let prefill = model.prefill.expect("the single PID is prefilled");
    assert_eq!(prefill.manual_output.as_deref(), Some("P1.ManualOutput"));
    assert!(
        !model
            .addressed_real
            .contains(&"P1.ManualOutput".to_string()),
        "the manual output is no longer an addressed signal"
    );
    assert!(model.signals.contains(&"P1.ManualOutput".to_string()));
}
