//! The setup form's model, ported from `control-studio-loop-setup.test.ts` (the form-model assertions: the structure, the signal
//! lists, the modes and booleans, the writes and initializes, and the prefills) and `control-studio-loop-setup.test.ts`'s cascade
//! startup test. The generator assertions of that file belong to the PID generators (`pid_loop_tests.rs`), and its render assertion
//! (JSDOM) is presentation, not ported.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::json;

use super::{
    setup_form_model, OfferedLoopName, PidPrefill, SchedulePointRow, SetupFormDefaults,
    SetupFormLimits, SetupFormWording,
};
use crate::loop_definition::{Action, ModeWhenTrue};
use crate::setup::catalog::{
    CallLink, CascadePair, CompiledCatalog, CompiledController, CompiledSymbol, CompiledTask,
    PidCandidate,
};
use crate::setup::own_law_loop::{DerivativeOn, OwnLawFamily};
use crate::setup::search_loop::{KeptStart, SEARCH_FAMILIES};
use crate::setup::texts::form as words_of;
use crate::tuning::state_controller::StateControllerFamily;

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

#[test]
fn the_form_offers_the_structures_its_program_can_use_and_the_startup_flag_of_a_pid_the_program_initializes(
) {
    // The discover answer names the structures the page offers (B1.3) and the PIDs whose program needs the startup-done flag (C1 Setup).
    let mut initializing = cascade();
    initializing.links.push(call(
        "P1.Outer",
        "Initialize",
        "State",
        "P1.State",
        None,
        &[],
    ));
    let model = setup_form_model(&initializing);
    assert!(model
        .startup_flag_instances
        .contains(&"P1.Outer".to_string()));
    assert!(model.structures.starts_with(&strings(&[
        "pid",
        "cascade",
        "feedforward",
        "smith",
        "schedule",
        "group"
    ])));
    assert_eq!(
        model.structures.last().map(String::as_str),
        Some("identify")
    );
}

#[test]
fn the_form_preselects_the_choices_the_engine_applies() {
    // The page shows these defaults and keeps none of its own (B1.3). Each word reads back as the engine's own type, so a word the
    // engine renames fails here.
    let defaults = setup_form_model(&single_loop()).defaults;
    assert_eq!(
        defaults,
        SetupFormDefaults {
            structure: "pid".to_string(),
            state_family: "LQR".to_string(),
            search_family: "MPC".to_string(),
            search_kept_start: "zero".to_string(),
            mode_when_true: "Auto".to_string(),
            action: "direct".to_string(),
            band_convention: "half-band".to_string(),
            threshold_equality: "at-or-beyond".to_string(),
            own_law: "PI".to_string(),
            trajectory_own_law: "PID".to_string(),
        }
    );
    assert!(
        SEARCH_FAMILIES.contains(&defaults.search_family.as_str()),
        "Tune searches the family the form preselects"
    );
    assert_eq!(
        serde_json::from_value::<StateControllerFamily>(json!(defaults.state_family)).ok(),
        Some(StateControllerFamily::Lqr)
    );
    assert_eq!(
        serde_json::from_value::<KeptStart>(json!(defaults.search_kept_start)).ok(),
        Some(KeptStart::default())
    );
    assert_eq!(
        serde_json::from_value::<ModeWhenTrue>(json!(defaults.mode_when_true)).ok(),
        Some(ModeWhenTrue::Auto)
    );
    assert_eq!(
        serde_json::from_value::<Action>(json!(defaults.action)).ok(),
        Some(Action::Direct)
    );
    assert_eq!(
        serde_json::from_value::<OwnLawFamily>(json!(defaults.own_law)).ok(),
        Some(OwnLawFamily::Pi)
    );
    assert_eq!(
        serde_json::from_value::<OwnLawFamily>(json!(defaults.trajectory_own_law)).ok(),
        Some(OwnLawFamily::Pid)
    );
}

#[test]
fn a_program_of_several_pids_that_do_not_chain_preselects_the_group_structure() {
    let mut several = single_loop();
    several
        .symbols
        .extend(pid_symbols("P2.Controller", "P2.Parameters"));
    several
        .controllers
        .push(controller("P2.Controller", "Control.PID"));
    several.links.push(call(
        "P2.Controller",
        "Configure",
        "Parameters",
        "P2.Parameters",
        None,
        &[],
    ));
    let model = setup_form_model(&several);
    assert_eq!(model.structure, "group");
    assert_eq!(
        model.defaults.structure, "group",
        "the form preselects the structure the program shows, not the single PID's"
    );
    assert_eq!(model.loop_names.len(), 2);
}

#[test]
fn the_form_draws_the_rows_its_generators_accept() {
    assert_eq!(
        setup_form_model(&single_loop()).limits,
        SetupFormLimits {
            further_outputs: 3,
            estimator_states: 3,
            search_coefficients: 4,
            search_kept_values: 3,
            group_members_minimum: 2,
        }
    );
}

#[test]
fn the_gain_schedule_asks_for_its_first_two_points_and_offers_two_more() {
    // C10: a schedule has two to four operating points. The form draws one row for each and the engine marks the required ones, so the
    // page keeps no count and no flag of its own.
    assert_eq!(
        setup_form_model(&single_loop()).schedule_points,
        [true, true, false, false].map(|required| SchedulePointRow { required })
    );
}

/// The single-loop program with `count` library PIDs: the first is the single loop's, and each further one only configures its record.
fn with_pids(count: usize) -> CompiledCatalog {
    let mut catalog = single_loop();
    for number in 2..=count {
        let instance = format!("P{number}.Controller");
        let record = format!("P{number}.Parameters");
        catalog.symbols.extend(pid_symbols(&instance, &record));
        catalog
            .controllers
            .push(controller(&instance, "Control.PID"));
        catalog.links.push(call(
            &instance,
            "Configure",
            "Parameters",
            &record,
            None,
            &[],
        ));
    }
    catalog
}

#[test]
fn the_decoupler_has_a_row_and_a_column_for_each_library_pid_of_the_program() {
    // C7: the members of a group are the program's library PIDs, and the static decoupler D has one row for each plant input and one
    // column for each member, so the matrix is square in the PIDs. A program without a library PID has no decoupler to ask for.
    let shape = |catalog: &CompiledCatalog| {
        let decoupler = setup_form_model(catalog).decoupler;
        (decoupler.rows, decoupler.columns)
    };
    assert_eq!(shape(&with_pids(1)), (1, 1));
    assert_eq!(shape(&with_pids(2)), (2, 2));
    assert_eq!(shape(&with_pids(5)), (5, 5));
    let identify_only = CompiledCatalog {
        symbols: Vec::new(),
        controllers: Vec::new(),
        links: Vec::new(),
        writes: Vec::new(),
    };
    assert_eq!(shape(&identify_only), (0, 0));
}

#[test]
fn the_form_answers_the_schedule_rows_and_the_decoupler_under_the_names_the_page_reads() {
    // The page draws `schedulePoints` and `decoupler` of the discover answer (B1.3) and keeps no count of its own.
    let value = serde_json::to_value(setup_form_model(&with_pids(2))).expect("the model is JSON");
    assert_eq!(
        value["schedulePoints"],
        json!([
            { "required": true },
            { "required": true },
            { "required": false },
            { "required": false }
        ])
    );
    assert_eq!(value["decoupler"], json!({ "rows": 2, "columns": 2 }));
}

#[test]
fn the_form_shows_the_engines_words_for_a_mode_and_for_a_missing_value() {
    assert_eq!(
        setup_form_model(&single_loop()).wording,
        SetupFormWording {
            true_means: "TRUE means".to_string(),
            not_found: "Not found in the code".to_string(),
            not_stated: "Not stated in the code".to_string(),
        }
    );
}

#[test]
fn the_form_starts_each_loop_name_from_its_controllers_last_name_segment() {
    assert_eq!(
        setup_form_model(&single_loop()).loop_names,
        vec![OfferedLoopName {
            instance: "P1.Controller".to_string(),
            name: "Controller".to_string(),
        }]
    );
    let mut plain = single_loop();
    plain.controllers[1].instance = "Heater".to_string();
    assert_eq!(
        setup_form_model(&plain).loop_names,
        vec![OfferedLoopName {
            instance: "Heater".to_string(),
            name: "Heater".to_string(),
        }],
        "an instance without a dot is its own name"
    );
}

#[test]
fn the_form_offers_the_values_each_choice_reads_in_the_order_the_page_lists_them() {
    let choices = super::choices();
    assert_eq!(choices.mode_when_true, strings(&["Auto", "Manual"]));
    assert_eq!(
        choices.band_conventions,
        strings(&["half-band", "full-band"])
    );
    assert_eq!(
        choices.threshold_equalities,
        strings(&["at-or-beyond", "beyond"])
    );
    assert_eq!(
        choices.state_families,
        strings(&["LQR", "state-feedback", "LQG"])
    );
    assert_eq!(choices.own_laws, strings(&["PI", "P", "PID"]));
    assert_eq!(
        choices.search_families,
        strings(&crate::setup::search_loop::SEARCH_FAMILIES)
    );
    assert_eq!(
        choices.kept_starts,
        strings(&["zero", "output", "measurement"])
    );
    assert_eq!(choices.actions, strings(&["direct", "reverse"]));
}

#[test]
fn the_instances_that_can_hold_switching_logic_and_every_real_setting_are_offered() {
    // control-studio-loop-setup-onoff: "offers the instances that can hold switching logic and every REAL setting" (specification 35,
    // B1.3 and C3 Setup). A cold room whose program P1 switches a compressor: a library PID is set up as a PID, and switching logic needs
    // an instance that runs in exactly one cyclic task. Settings are written by Studio directly, so every REAL and LREAL symbol is
    // offered, whatever its I/O address.
    let symbols = vec![
        symbol("P1.RoomC", "LREAL", Some("%IL0")),
        symbol("P1.CompressorPct", "LREAL", Some("%QL0")),
        symbol("P1.SetpointC", "LREAL", Some("%ML0")),
        symbol("P1.ManualPct", "LREAL", Some("%ML8")),
        symbol("P1.AutoMode", "BOOL", None),
        symbol("P1.CoolingOn", "BOOL", None),
        symbol("P1.DwellS", "LREAL", None),
        symbol("P1.HysteresisC", "LREAL", None),
        symbol("P1.MinimumOnS", "LREAL", None),
        symbol("P1.MinimumOffS", "LREAL", None),
    ];
    let catalog = CompiledCatalog {
        symbols,
        controllers: vec![
            controller("P1", "Main"),
            controller("P1.Backup", "Control.PID"),
            CompiledController {
                instance: "P1.Unscheduled".to_string(),
                type_name: "Switch".to_string(),
                tasks: Vec::new(),
            },
        ],
        links: Vec::new(),
        writes: strings(&["P1.CompressorPct", "P1.CoolingOn", "P1.DwellS"]),
    };
    let model = setup_form_model(&catalog);
    assert_eq!(
        model.instances,
        strings(&["P1"]),
        "a library PID is set up as a PID, and switching logic needs one cyclic task"
    );
    assert_eq!(
        model.reals,
        strings(&[
            "P1.CompressorPct",
            "P1.DwellS",
            "P1.HysteresisC",
            "P1.ManualPct",
            "P1.MinimumOffS",
            "P1.MinimumOnS",
            "P1.RoomC",
            "P1.SetpointC",
        ]),
        "settings are written by Studio directly, so they need no I/O address"
    );
    assert!(
        model
            .structures
            .iter()
            .any(|structure| structure == "onoff"),
        "a program with an instance of its own is offered the switching controller: {:?}",
        model.structures
    );
}

/// R3 (B1.3: the page shows the engine's words for each value it offers and keeps none of its own): a program with two library PIDs and
/// an instance of its own offers all twelve structures, and its form model names each structure and each value of each choice with
/// its words (`labels`), and no value it does not offer. Every value has its words in the engine's table, so none is shown bare. The
/// words the specification states are the engine's: a controller action in the direction the output moves, increasing it while the
/// measurement is below the setpoint (`direct`) or above it (`reverse`) (B1.3, B1.4), and a PID law's derivative taken from the error
/// or the measurement (C2), whose values are the draft's own. The wrong behaviour caught: a value the page offers without words, which a
/// label table in the page would drop, or words that drift from the values the draft reads.
#[test]
fn setup_form_model_names_every_choice_it_offers() {
    let model = setup_form_model(&with_pids(2));
    assert_eq!(
        model.structures.len(),
        12,
        "the program offers every structure: {:?}",
        model.structures
    );
    let choices = &model.choices;
    let labels = &model.labels;
    assert_named(
        "structures",
        &model.structures,
        &labels.structures,
        &words_of::STRUCTURES,
    );
    assert_named(
        "modeWhenTrue",
        &choices.mode_when_true,
        &labels.mode_when_true,
        &words_of::MODE_WHEN_TRUE,
    );
    assert_named(
        "bandConventions",
        &choices.band_conventions,
        &labels.band_conventions,
        &words_of::BAND_CONVENTIONS,
    );
    assert_named(
        "thresholdEqualities",
        &choices.threshold_equalities,
        &labels.threshold_equalities,
        &words_of::THRESHOLD_EQUALITIES,
    );
    assert_named(
        "stateFamilies",
        &choices.state_families,
        &labels.state_families,
        &words_of::STATE_FAMILIES,
    );
    assert_named(
        "ownLaws",
        &choices.own_laws,
        &labels.own_laws,
        &words_of::OWN_LAWS,
    );
    assert_named(
        "derivativeOn",
        &choices.derivative_on,
        &labels.derivative_on,
        &words_of::DERIVATIVE_ON,
    );
    assert_named(
        "searchFamilies",
        &choices.search_families,
        &labels.search_families,
        &words_of::SEARCH_FAMILIES,
    );
    assert_named(
        "keptStarts",
        &choices.kept_starts,
        &labels.kept_starts,
        &words_of::KEPT_STARTS,
    );
    assert_named(
        "actions",
        &choices.actions,
        &labels.actions,
        &words_of::ACTIONS,
    );
    assert_eq!(
        labels.actions["direct"],
        "Increase the output while the measurement is below the setpoint"
    );
    assert_eq!(
        labels.actions["reverse"],
        "Increase the output while the measurement is above the setpoint"
    );
    assert_eq!(choices.derivative_on, strings(&["error", "measurement"]));
    for (value, on) in [
        ("error", DerivativeOn::Error),
        ("measurement", DerivativeOn::Measurement),
    ] {
        assert_eq!(
            serde_json::from_value::<DerivativeOn>(json!(value))
                .expect("the draft reads the value"),
            on
        );
    }
    let answer = serde_json::to_value(&model).expect("the model is JSON");
    assert_eq!(
        answer["labels"]["derivativeOn"],
        json!({ "error": "The error", "measurement": "The measurement" }),
        "the page reads the words under the names it uses"
    );
}

/// Checks that the words of the list `name` name exactly the values it offers, each with its words in the engine's `table`, which holds
/// words for exactly those values.
fn assert_named(
    name: &str,
    values: &[String],
    words: &BTreeMap<String, String>,
    table: &[(&str, &str)],
) {
    let offered: BTreeSet<&str> = values.iter().map(String::as_str).collect();
    let named: BTreeSet<&str> = words.keys().map(String::as_str).collect();
    assert_eq!(
        named, offered,
        "{name}: the words name exactly the values offered"
    );
    let tabled: BTreeSet<&str> = table.iter().map(|(value, _)| *value).collect();
    assert_eq!(
        tabled, offered,
        "{name}: every value offered has its words, and no words are stale"
    );
    for (value, text) in table {
        assert_eq!(
            words[*value], *text,
            "{name}: {value} shows the engine's words"
        );
    }
}
