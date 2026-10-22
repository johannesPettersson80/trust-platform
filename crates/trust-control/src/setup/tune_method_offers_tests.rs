//! Tests of the tuning-method offers of the loop setup (specification 35, B4.2, C0, C2, C4, C6 to C10, N5.R): the methods a controller takes
//! with their titles and words, the settings each family takes, the methods a study offers the Tune page, and the loops that Tune designs
//! by Automatic alone. They were part of the discovery tests, which keep the discovery and the controller compatibility.

#![allow(clippy::float_cmp)]

use std::collections::BTreeMap;

use serde_json::{json, Value};

use super::study_tune_methods;
use crate::commissioning::fixture_loops::smith_predictor_loop;
use crate::loop_definition::{
    AntiWindup, AntiWindupMethod, ControllerLaw, LoopDefinition, Parameter, RealType,
};
use crate::setup::discovery::{adapter_compatibility, ControllerCompatibility};
use crate::setup::discovery_test_support::{
    loop_of, simc_amigo_manual, simc_amigo_manual_titles, tank_json,
};

/// The symbols of a list, sorted, as the page compares a family's settings.
fn sorted_symbols(symbols: &[&str]) -> Vec<String> {
    let mut sorted: Vec<String> = symbols.iter().map(|symbol| (*symbol).to_owned()).collect();
    sorted.sort();
    sorted
}

/// The families of an accepted loop and their settings, each list sorted (`setupDiscover`'s `familySettings`).
fn sorted_family_settings(loop_definition: &LoopDefinition) -> BTreeMap<String, Vec<String>> {
    match adapter_compatibility(loop_definition) {
        ControllerCompatibility::Accepted {
            family_settings, ..
        } => family_settings
            .into_iter()
            .map(|(family, mut symbols)| {
                symbols.sort();
                (family, symbols)
            })
            .collect(),
        other => panic!("a Control.PID loop is accepted, not {other:?}"),
    }
}

#[test]
fn the_study_offers_the_relay_rule_beside_manual_only_with_a_qualified_record() {
    use crate::messages::TuneMethod;
    use crate::tuning::relay_rule::RelayCycleEvidence;
    let loop_definition = LoopDefinition::decode(include_str!(
        "../../../../examples/control-pid-commissioning/control-loop.json"
    ))
    .expect("the commissioning example's loop");
    let qualified = RelayCycleEvidence::Qualified {
        cycles: 4,
        output_amplitude: 1.0,
        response_amplitude: 0.5,
        period_seconds: 2.0,
        ultimate_gain: 3.0,
        proportional_gain: 1.35,
        integral_time_seconds: 1.5,
        integral_gain: 0.9,
    };
    let not_qualified = RelayCycleEvidence::NotQualified {
        reason: "Relay cycle amplitude or period is not identifiable.".to_owned(),
    };
    let offered = |relay: Option<&RelayCycleEvidence>| {
        study_tune_methods(&loop_definition, relay)
            .expect("an accepted PID loop")
            .0
    };
    let named = vec![TuneMethod::Simc, TuneMethod::Amigo, TuneMethod::Manual];
    assert_eq!(offered(None), named);
    assert_eq!(offered(Some(&not_qualified)), named);
    assert_eq!(
        offered(Some(&qualified)),
        vec![
            TuneMethod::Simc,
            TuneMethod::Amigo,
            TuneMethod::Relay,
            TuneMethod::Manual
        ]
    );
    let (_, titles) =
        study_tune_methods(&loop_definition, Some(&qualified)).expect("an accepted PID loop");
    let titles = titles.expect("the offered methods have titles");
    assert_eq!(titles["relay"], "Relay rule (Ziegler–Nichols PI)");
    assert_eq!(titles["manual"], "Manual");
}

#[test]
fn a_control_pid_loop_names_the_settings_each_family_takes_as_the_page_did() {
    // The page's rule before the engine named the settings (manualTuneSettings.ts): Kp for every family; Ki for PI and PID; Kd for PD
    // and PID; the tracking gain and β for PI and PID; γ for PD and PID; Tf for every family where the law declares it.
    const KP: &str = "P1.Parameters.ProportionalGain";
    const KI: &str = "P1.Parameters.IntegralGain";
    const KD: &str = "P1.Parameters.DerivativeGain";
    const BETA: &str = "P1.Parameters.ProportionalSetpointWeight";
    const GAMMA: &str = "P1.Parameters.DerivativeSetpointWeight";
    const TF: &str = "P1.Parameters.MeasurementFilterTime";
    const TRACKING: &str = "P1.Parameters.TrackingTime";
    let mut loop_definition = LoopDefinition::decode(include_str!(
        "../../../../examples/control-pid-commissioning/control-loop.json"
    ))
    .expect("the commissioning example's loop");

    // The shipped law declares Kp, Ki and Kd only.
    assert_eq!(
        sorted_family_settings(&loop_definition),
        BTreeMap::from([
            ("P".to_owned(), sorted_symbols(&[KP])),
            ("PI".to_owned(), sorted_symbols(&[KP, KI])),
            ("PD".to_owned(), sorted_symbols(&[KP, KD])),
            ("PID".to_owned(), sorted_symbols(&[KP, KI, KD])),
        ]),
        "the shipped PID lessons keep the page's settings"
    );

    // Every optional field declared and bound: each one appears under the families that take it, and Tf under every family.
    if let Some(ControllerLaw::Pid(pid)) = &mut loop_definition.controller.law {
        pid.proportional_setpoint_weight_symbol = Some(BETA.to_owned());
        pid.derivative_setpoint_weight_symbol = Some(GAMMA.to_owned());
        pid.measurement_filter_time_symbol = Some(TF.to_owned());
        pid.anti_windup = Some(AntiWindup {
            method: AntiWindupMethod::BackCalculation,
            tracking_gain_symbol: TRACKING.to_owned(),
        });
    }
    if let Some(binding) = &mut loop_definition.parameter_binding {
        binding.parameters.push(Parameter {
            symbol: TF.to_owned(),
            iec_type: RealType::Real,
            unit: "s".to_owned(),
            convention: "Control.PID MeasurementFilterTime".to_owned(),
            minimum: 0.0,
            maximum: 10.0,
        });
    }
    assert_eq!(
        sorted_family_settings(&loop_definition),
        BTreeMap::from([
            ("P".to_owned(), sorted_symbols(&[KP, TF])),
            (
                "PI".to_owned(),
                sorted_symbols(&[KP, KI, BETA, TF, TRACKING])
            ),
            ("PD".to_owned(), sorted_symbols(&[KP, KD, GAMMA, TF])),
            (
                "PID".to_owned(),
                sorted_symbols(&[KP, KI, KD, BETA, GAMMA, TF, TRACKING])
            ),
        ]),
        "each optional field is named by the families that take it"
    );
}

#[test]
fn a_control_pid_law_offers_simc_amigo_and_manual_in_that_order_with_their_titles() {
    // C4: a Control.PID law takes the two formula methods and the manual settings. Its relay rule needs a qualified record of the study,
    // which setup does not read, so setup does not offer it.
    let loop_definition = LoopDefinition::decode(include_str!(
        "../../../../examples/control-pid-commissioning/control-loop.json"
    ))
    .expect("the commissioning example's loop");
    let answer = adapter_compatibility(&loop_definition);
    let ControllerCompatibility::Accepted {
        methods,
        method_titles,
        ..
    } = &answer
    else {
        panic!("a Control.PID loop is accepted, not {answer:?}");
    };
    assert_eq!(
        methods,
        &Some(vec![
            crate::messages::TuneMethod::Simc,
            crate::messages::TuneMethod::Amigo,
            crate::messages::TuneMethod::Manual,
        ])
    );
    assert_eq!(
        method_titles,
        &Some(BTreeMap::from([
            (
                "SIMC".to_owned(),
                "Simple internal model control (SIMC)".to_owned()
            ),
            (
                "AMIGO".to_owned(),
                "Approximate M-constrained integral gain optimization (AMIGO)".to_owned()
            ),
            ("manual".to_owned(), "Manual".to_owned()),
        ]))
    );
    // The page receives the same names and titles.
    let page = serde_json::to_value(&answer).expect("the answer encodes");
    assert_eq!(page["methods"], json!(["SIMC", "AMIGO", "manual"]));
    assert_eq!(
        page["methodTitles"]["SIMC"],
        "Simple internal model control (SIMC)"
    );
    assert_eq!(page["methodTitles"]["manual"], "Manual");
}

#[test]
fn the_word_of_each_method_is_the_name_the_method_has_in_its_json() {
    use crate::messages::TuneMethod;
    for method in [
        TuneMethod::Simc,
        TuneMethod::Amigo,
        TuneMethod::Relay,
        TuneMethod::Manual,
        TuneMethod::StateSpace,
    ] {
        assert_eq!(
            serde_json::to_value(method).expect("a method encodes"),
            json!(super::method_word(method))
        );
    }
}

/// Lesson 89's finite search (C5) is offered as its declared family with its coefficient variables, recommended, and with no named
/// method: Automatic runs its search.
#[test]
fn a_search_law_is_offered_as_its_declared_family_with_automatic_alone() {
    let lesson_89 = include_str!(
        "../../../../editors/vscode/media/examples/control/07-classical-stability-and-frequency-design/89-mechanical-resonance/control-loop.json"
    );
    let loop_definition = LoopDefinition::decode(lesson_89).expect("lesson 89's loop decodes");
    assert_eq!(
        adapter_compatibility(&loop_definition),
        ControllerCompatibility::Accepted {
            families: vec!["PID".to_string()],
            family_settings: BTreeMap::from([(
                "PID".to_string(),
                vec![
                    "P1.NotchFrequencyHz".to_string(),
                    "P1.NotchQuality".to_string()
                ]
            )]),
            recommended_family: Some("PID".to_string()),
            methods: Some(Vec::new()),
            method_titles: None,
        }
    );
}

#[test]
fn a_proportional_law_offers_simc_amigo_and_manual_with_their_titles() {
    // C4 and B4.2: a proportional law takes the same named methods as a PID law, and its one family is P.
    let mut value = tank_json("loop:tank");
    value["parameterBinding"] = json!({
        "loop_id": "loop:tank", "task_name": "MainTask",
        "parameters": [
            { "symbol": "Main.Kp", "iec_type": "LREAL", "unit": "%/%", "convention": "proportional gain", "minimum": 0, "maximum": 100 }
        ],
        "mode": { "symbol": "Main.Auto", "manual_value": false }
    });
    let mut proportional = loop_of(&value);
    proportional.controller.law = Some(ControllerLaw::Proportional {
        gain_symbol: "Main.Kp".to_string(),
    });
    let answer = adapter_compatibility(&proportional);
    let ControllerCompatibility::Accepted {
        families,
        methods,
        method_titles,
        ..
    } = &answer
    else {
        panic!("a proportional law with its parameter record is accepted, not {answer:?}");
    };
    assert_eq!(families, &vec!["P".to_string()]);
    assert_eq!(methods, &Some(simc_amigo_manual()));
    assert_eq!(method_titles, &Some(simc_amigo_manual_titles()));
    // The page receives the same names and titles.
    let page = serde_json::to_value(&answer).expect("the answer encodes");
    assert_eq!(page["methods"], json!(["SIMC", "AMIGO", "manual"]));
    assert_eq!(page["methodTitles"]["manual"], "Manual");
}

#[test]
fn a_state_law_offers_the_state_space_design_alone() {
    // C4: a state-feedback law takes the state-space design alone, so the page offers no SIMC, AMIGO or Manual for it.
    let loop_definition = LoopDefinition::decode(include_str!(
        "../../../../editors/vscode/media/examples/control/11-state-space-estimation-and-predictive-control/68-state-feedback/control-loop.json"
    ))
    .expect("lesson 68's loop");
    let answer = adapter_compatibility(&loop_definition);
    let ControllerCompatibility::Accepted {
        methods,
        method_titles,
        ..
    } = &answer
    else {
        panic!("a state law is accepted, not {answer:?}");
    };
    assert_eq!(
        methods,
        &Some(vec![crate::messages::TuneMethod::StateSpace])
    );
    assert_eq!(
        method_titles,
        &Some(BTreeMap::from([(
            "state-space".to_owned(),
            "State-space design".to_owned()
        )]))
    );
}

/// The page's offer of a loop that Tune designs by Automatic alone (B4.2, C4): its families, and no named method and no title. Tune
/// refuses each named method for such a loop, so the page shows Automatic alone.
fn assert_automatic_alone(loop_definition: &LoopDefinition, families: &[&str]) {
    let answer = adapter_compatibility(loop_definition);
    let ControllerCompatibility::Accepted {
        families: offered,
        methods,
        method_titles,
        ..
    } = &answer
    else {
        panic!("the loop is accepted, not {answer:?}");
    };
    let expected: Vec<String> = families
        .iter()
        .map(|family| (*family).to_string())
        .collect();
    assert_eq!(offered, &expected);
    assert_eq!(methods, &Some(Vec::<crate::messages::TuneMethod>::new()));
    assert!(method_titles.is_none(), "{method_titles:?}");
    let page = serde_json::to_value(&answer).expect("the answer encodes");
    assert_eq!(page["methods"], json!([]));
    assert!(page.get("methodTitles").is_none());
}

/// Lesson 84's feedforward (C8): Tune designs it by Automatic alone, and `check_request` refuses every named method for it.
#[test]
fn a_feedforward_program_law_is_offered_automatic_alone() {
    let loop_definition = LoopDefinition::decode(include_str!(
        "../../../../editors/vscode/media/examples/control/14-integrated-capstones/84-independent/control-loop.json"
    ))
    .expect("lesson 84's loop");
    assert_automatic_alone(&loop_definition, &["PI"]);
}

/// The Smith predictor of the shared commissioning fixtures (C9): its PI is designed by Automatic alone, as `check_request` gives.
#[test]
fn a_smith_predictor_is_offered_automatic_alone() {
    assert_automatic_alone(&smith_predictor_loop(), &["PI"]);
}

/// A cascade of two PIs (C6): Automatic designs both, and `check_request` refuses every named method for it.
#[test]
fn a_cascade_is_offered_automatic_alone() {
    assert_automatic_alone(&cascade_loop(), &["PI"]);
}

/// A gain schedule's point (C10): `point_requests` refuses every named method for the schedule. Its PI seed of the gain-scheduled
/// family offers P and PI.
#[test]
fn a_gain_scheduled_loop_is_offered_automatic_alone() {
    assert_automatic_alone(&gain_scheduled_loop(), &["P", "PI"]);
}

/// A switching controller (C3): `check_on_off` refuses every named method, and the controller offers no family.
#[test]
fn a_switching_controller_is_offered_automatic_alone() {
    assert_automatic_alone(&cold_room_loop(), &[]);
}

/// A PID group (C7): `pid_group::check` refuses every named method, and the group's program offers no family.
#[test]
fn a_pid_group_is_offered_automatic_alone() {
    assert_automatic_alone(&paired_group_loop(), &[]);
}

/// Lesson 90's trajectory law (C11): Tune takes SIMC, AMIGO and Manual for its PID, as for a PID law, so the page keeps the three.
#[test]
fn a_trajectory_program_law_keeps_the_named_methods() {
    let loop_definition = LoopDefinition::decode(include_str!(
        "../../../../editors/vscode/media/examples/control/09-industrial-control-structures/90-trajectory-feedforward/control-loop.json"
    ))
    .expect("lesson 90's loop");
    let answer = adapter_compatibility(&loop_definition);
    let ControllerCompatibility::Accepted {
        methods,
        method_titles,
        ..
    } = &answer
    else {
        panic!("a trajectory law is accepted, not {answer:?}");
    };
    assert_eq!(methods, &Some(simc_amigo_manual()));
    assert_eq!(method_titles, &Some(simc_amigo_manual_titles()));
}

/// A tank loop whose program block is `project`, with its parameters bound to `parameters`: the base of the program fixtures below.
fn program_loop(id: &str, project: Value, parameters: &[Value]) -> LoopDefinition {
    let mut value = tank_json(id);
    value["controller"]["design"] = json!("project");
    value["controller"]["project"] = project;
    value["parameterBinding"] = json!({
        "loop_id": id, "task_name": "MainTask", "parameters": parameters,
        "mode": { "symbol": "Main.Auto", "manual_value": false }
    });
    loop_of(&value)
}

/// A bound real gain of a program's law over a wide range, as the Tune tests bind them.
fn real_gain(symbol: &str) -> Value {
    json!({ "symbol": symbol, "iec_type": "LREAL", "unit": "1", "convention": "gain", "minimum": -1e6, "maximum": 1e6 })
}

/// A cascade of two PIs (C6), each named by its three gain variables, which are bound as parameters (`project_cascade_tests`).
fn cascade_loop() -> LoopDefinition {
    const GAINS: [&str; 6] = [
        "Main.InnerParameters.ProportionalGain",
        "Main.InnerParameters.IntegralGain",
        "Main.InnerParameters.DerivativeGain",
        "Main.OuterParameters.ProportionalGain",
        "Main.OuterParameters.IntegralGain",
        "Main.OuterParameters.DerivativeGain",
    ];
    let project = json!({
        "family": "cascade", "convention": "project-owned-native-controller",
        "initialization": { "kind": "declared-state-v1", "states": [] },
        "seed": {
            "kind": "cascade-pid", "family": "PI",
            "inner": { "instance": "Main.Inner", "proportionalSymbol": GAINS[0],
                "integralSymbol": GAINS[1], "derivativeSymbol": GAINS[2] },
            "outer": { "instance": "Main.Outer", "proportionalSymbol": GAINS[3],
                "integralSymbol": GAINS[4], "derivativeSymbol": GAINS[5] }
        },
        "axes": [], "objective": { "requirementId": "Main.VesselC:target-error", "direction": "minimize" },
        "maximumCandidates": 1, "maximumCycles": 50000, "wallBudgetMs": 60000
    });
    let parameters: Vec<Value> = GAINS.into_iter().map(real_gain).collect();
    program_loop("loop:vessel", project, &parameters)
}

/// The point `Low` of a gain schedule (C10): a PI of the gain-scheduled family, with the table of its two operating points
/// (`gain_schedule_tests`).
fn gain_scheduled_loop() -> LoopDefinition {
    let project = json!({
        "family": "gain-scheduled", "convention": "project-owned-native-controller",
        "initialization": { "kind": "declared-state-v1", "states": [] },
        "seed": { "kind": "parallel-pid", "family": "PI", "proportionalSymbol": "P1.KpLow", "integralSymbol": "P1.KiLow" },
        "schedule": {
            "parameterRecord": "P1.Parameters",
            "scheduling": { "symbol": "P1.LevelM", "unit": "m", "iecType": "LREAL" },
            "point": "Low",
            "points": [
                { "name": "Low", "loopId": "loop:tank-level-low", "value": 0.09, "tolerance": 0.01,
                    "proportionalSymbol": "P1.KpLow", "integralSymbol": "P1.KiLow" },
                { "name": "High", "loopId": "loop:tank-level-high", "value": 0.49, "tolerance": 0.01,
                    "proportionalSymbol": "P1.KpHigh", "integralSymbol": "P1.KiHigh" }
            ]
        },
        "axes": [], "objective": { "requirementId": "target-error", "direction": "minimize" },
        "maximumCandidates": 4, "maximumCycles": 50000, "wallBudgetMs": 60000
    });
    let parameters: Vec<Value> = ["P1.KpLow", "P1.KiLow", "P1.KpHigh", "P1.KiHigh"]
        .into_iter()
        .map(real_gain)
        .collect();
    program_loop("loop:tank-level-low", project, &parameters)
}

/// The cold room of a switching controller (C3) with a half-band hysteresis, the equipment's minimum times and its on-off channel
/// (`study/tune/design/on_off_tests`).
fn cold_room_loop() -> LoopDefinition {
    let actuator = json!({
        "kind": "on-off", "minimum": 0.0, "maximum": 100.0,
        "minimumOnSeconds": 20.0, "minimumOffSeconds": 30.0
    });
    let value = json!({
        "schemaVersion": 2, "id": "loop:cold-room", "name": "Cold room", "revision": "sha256:cold-1",
        "projectId": "project:setup", "runtimeId": "runtime-1",
        "controller": {
            "instance": "P1", "type": "Main", "revision": "sha256:controller-1", "action": "reverse",
            "taskPeriodMs": 100, "design": "project", "modeSignal": "P1.AutoMode", "modeWhenTrue": "Auto",
            "manualOutputSignal": "P1.ManualPct", "manualOutputType": "LREAL",
            "onOff": {
                "hysteresisSymbol": "P1.HysteresisC", "minimumOnTimeSymbol": "P1.MinimumOnS",
                "minimumOffTimeSymbol": "P1.MinimumOffS", "bandConvention": "half-band",
                "lowOutput": 0.0, "highOutput": 100.0,
                "initialization": { "kind": "declared-state-v1", "states": [
                    { "symbol": "P1.CoolingOn", "iec_type": "BOOL", "unit": "1", "minimum": 0.0, "maximum": 1.0,
                      "initial": { "source": "model-input-is", "index": 0, "value": 100.0 }, "postScan": "bounded" },
                    { "symbol": "P1.DwellS", "iec_type": "LREAL", "unit": "s", "minimum": 0.0, "maximum": 1_000_000.0,
                      "initial": 0.0, "postScan": "bounded" }
                ] }
            }
        },
        "parameterBinding": {
            "loop_id": "loop:cold-room", "task_name": "MainTask",
            "parameters": [
                { "symbol": "P1.HysteresisC", "iec_type": "LREAL", "unit": "degC", "convention": "ON/OFF hysteresis", "minimum": 0.0, "maximum": 30.0 },
                { "symbol": "P1.MinimumOnS", "iec_type": "LREAL", "unit": "s", "convention": "ON/OFF minimum ON time", "minimum": 0.0, "maximum": 3600.0 },
                { "symbol": "P1.MinimumOffS", "iec_type": "LREAL", "unit": "s", "convention": "ON/OFF minimum OFF time", "minimum": 0.0, "maximum": 3600.0 }
            ],
            "mode": { "symbol": "P1.AutoMode", "manual_value": false }
        },
        "signals": {
            "setpoint": { "symbol": "P1.SetpointC", "iecType": "LREAL", "direction": "read", "unit": "degC", "range": { "minimum": -10, "maximum": 20 } },
            "processVariable": { "symbol": "P1.RoomC", "address": "%IL0", "iecType": "LREAL", "direction": "read", "unit": "degC", "range": { "minimum": -10, "maximum": 20 } },
            "manipulatedVariable": { "symbol": "P1.CompressorPct", "address": "%QL0", "iecType": "LREAL", "direction": "write", "unit": "%", "range": { "minimum": 0, "maximum": 100 } }
        },
        "limits": { "outputMinimum": 0, "outputMaximum": 100 },
        "observedSampleTiming": { "nominalPeriodMs": 100 },
        "channels": {
            "inputs": [{ "modelChannelId": "P1.CompressorPct", "signalId": "manipulatedVariable", "actuator": actuator }],
            "outputs": [{ "modelChannelId": "P1.RoomC", "signalId": "processVariable", "setpointSignalId": "setpoint" }]
        }
    });
    LoopDefinition::decode(&value.to_string()).expect("the cold room loop decodes")
}

/// The two zones of a decentralized PI group (C7), each member's gains bound as its record's three symbols (`pid_group_tests`).
fn paired_group_loop() -> LoopDefinition {
    let members: Vec<Value> = [
        ("P1.Zone1", "u1", "y1", "P1.Zone1Parameters"),
        ("P1.Zone2", "u2", "y2", "P1.Zone2Parameters"),
    ]
    .iter()
    .map(|(instance, input, output, record)| {
        json!({
            "instance": instance, "type": "Control.PID", "inputChannelId": input, "outputChannelId": output,
            "modeSymbol": "P1.ZonesAuto",
            "parameterSymbols": [format!("{record}.ProportionalGain"), format!("{record}.IntegralGain"), format!("{record}.DerivativeGain")],
        })
    })
    .collect();
    serde_json::from_value(json!({
        "schemaVersion": 2, "id": "loop:zones", "name": "Zones", "revision": "sha256:zones",
        "projectId": "project:zones", "runtimeId": "runtime:zones",
        "controller": {
            "instance": "P1.Zone1", "type": "Control.PID", "revision": "sha256:zones-controller",
            "action": "direct", "taskPeriodMs": 100, "design": "project",
            "modeSignal": "P1.ZonesAuto", "modeWhenTrue": "Auto",
            "project": {
                "family": "PI", "convention": "project-owned-native-controller",
                "initialization": { "kind": "declared-state-v1", "states": [] },
                "seed": { "kind": "decentralized-pid", "family": "PI" }, "axes": [],
                "objective": { "requirementId": "P1.Zone1C:target-error", "direction": "minimize" },
                "maximumCandidates": 1, "maximumCycles": 50000, "wallBudgetMs": 60000
            },
            "group": { "members": members }
        },
        "signals": {
            "setpoint": { "symbol": "P1.Zone1SetpointC", "address": "%ML0", "iecType": "LREAL", "direction": "read", "unit": "degC", "range": { "minimum": 0, "maximum": 100 } },
            "processVariable": { "symbol": "P1.Zone1C", "address": "%IL0", "iecType": "LREAL", "direction": "read", "unit": "degC", "range": { "minimum": 0, "maximum": 100 } },
            "manipulatedVariable": { "symbol": "P1.Heater1Pct", "address": "%QL0", "iecType": "LREAL", "direction": "write", "unit": "%", "range": { "minimum": 0, "maximum": 100 } }
        },
        "limits": { "outputMinimum": 0, "outputMaximum": 100 },
        "observedSampleTiming": { "nominalPeriodMs": 100 },
        "parameterBinding": {
            "loop_id": "loop:zones", "task_name": "MainTask",
            "parameters": [
                { "symbol": "P1.Zone1Parameters.ProportionalGain", "iec_type": "LREAL", "unit": "%/degC", "convention": "proportional gain", "minimum": -1_000_000, "maximum": 1_000_000 },
                { "symbol": "P1.Zone1Parameters.IntegralGain", "iec_type": "LREAL", "unit": "%/(degC*s)", "convention": "integral gain", "minimum": -1_000_000, "maximum": 1_000_000 },
                { "symbol": "P1.Zone1Parameters.DerivativeGain", "iec_type": "LREAL", "unit": "%*s/degC", "convention": "derivative gain", "minimum": -1_000_000, "maximum": 1_000_000 },
                { "symbol": "P1.Zone2Parameters.ProportionalGain", "iec_type": "LREAL", "unit": "%/degC", "convention": "proportional gain", "minimum": -1_000_000, "maximum": 1_000_000 },
                { "symbol": "P1.Zone2Parameters.IntegralGain", "iec_type": "LREAL", "unit": "%/(degC*s)", "convention": "integral gain", "minimum": -1_000_000, "maximum": 1_000_000 },
                { "symbol": "P1.Zone2Parameters.DerivativeGain", "iec_type": "LREAL", "unit": "%*s/degC", "convention": "derivative gain", "minimum": -1_000_000, "maximum": 1_000_000 },
                { "symbol": "P1.D11", "iec_type": "LREAL", "unit": "1", "convention": "decoupler entry", "minimum": -1000, "maximum": 1000 },
                { "symbol": "P1.D12", "iec_type": "LREAL", "unit": "1", "convention": "decoupler entry", "minimum": -1000, "maximum": 1000 },
                { "symbol": "P1.D21", "iec_type": "LREAL", "unit": "1", "convention": "decoupler entry", "minimum": -1000, "maximum": 1000 },
                { "symbol": "P1.D22", "iec_type": "LREAL", "unit": "1", "convention": "decoupler entry", "minimum": -1000, "maximum": 1000 }
            ],
            "mode": { "symbol": "P1.ZonesAuto", "manual_value": false }
        },
        "channels": {
            "inputs": [
                { "modelChannelId": "P1.Heater1Pct", "signalId": "manipulatedVariable", "actuator": { "kind": "continuous", "minimum": 0, "maximum": 100 } },
                { "modelChannelId": "P1.Heater2Pct", "signalId": "manipulatedVariable", "actuator": { "kind": "continuous", "minimum": 0, "maximum": 100 } }
            ],
            "outputs": [
                { "modelChannelId": "P1.Zone1C", "signalId": "processVariable", "setpointSignalId": "setpoint" },
                { "modelChannelId": "P1.Zone2C", "signalId": "processVariable", "setpointSignalId": "setpoint" }
            ]
        }
    }))
    .expect("the group loop")
}
