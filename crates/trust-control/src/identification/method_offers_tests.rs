use serde_json::json;

use super::*;
use crate::loop_definition::{AuxiliarySignal, Execution};

/// The PID loop of the offer tests: a setpoint, a measured output and a manipulated variable, a Manual command, a mode binding and a
/// declared positive relay direction, on a Control Studio observation.
fn pid_loop() -> LoopDefinition {
    serde_json::from_value(json!({
        "schemaVersion": 2,
        "id": "pid",
        "name": "PID",
        "revision": "sha256:pid",
        "projectId": "test",
        "runtimeId": "test",
        "controller": {
            "instance": "Main.PID",
            "type": "Control.PID",
            "revision": "sha256:controller",
            "action": "direct",
            "taskPeriodMs": 100.0,
            "manualOutputSignal": "Main.PID_Manual",
            "manualOutputType": "REAL"
        },
        "observedSampleTiming": { "nominalPeriodMs": 100.0 },
        "signals": {
            "setpoint": {
                "symbol": "Main.SP", "iecType": "LREAL", "direction": "read",
                "unit": "degC", "range": { "minimum": 0.0, "maximum": 100.0 }
            },
            "processVariable": {
                "symbol": "Main.PV", "iecType": "LREAL", "direction": "read",
                "unit": "degC", "range": { "minimum": 0.0, "maximum": 100.0 }
            },
            "manipulatedVariable": {
                "symbol": "Main.MV", "iecType": "LREAL", "direction": "write",
                "unit": "%", "range": { "minimum": 0.0, "maximum": 100.0 }
            }
        },
        "limits": { "outputMinimum": 0.0, "outputMaximum": 100.0 },
        "parameterBinding": {
            "loop_id": "pid",
            "task_name": "MainTask",
            "parameters": [],
            "mode": { "symbol": "Main.Run", "manual_value": true }
        },
        "experiment": {
            "execution": "control-studio", "kind": "observation", "amplitude": 10.0,
            "relayDirection": "positive", "durationSeconds": 60.0, "seed": 1
        }
    }))
    .expect("the fixture loop decodes")
}

/// An additional signal of the loop in the setpoint role, with the given symbol and unit.
fn setpoint_candidate(symbol: &str, unit: &str) -> AuxiliarySignal {
    serde_json::from_value(json!({
        "id": "sp2", "channel": "reference", "role": "SP", "symbol": symbol,
        "iecType": "LREAL", "direction": "read", "unit": unit,
        "range": { "minimum": 0.0, "maximum": 100.0 }
    }))
    .expect("the fixture signal decodes")
}

fn offer_of<'a>(offers: &'a [IdentifyMethodOffer], value: &str) -> &'a IdentifyMethodOffer {
    offers
        .iter()
        .find(|offer| offer.value == value)
        .expect("the method is offered")
}

#[test]
fn a_loop_offers_its_methods_in_the_page_order_and_can_run_each_of_them() {
    let offers = identify_method_offers(&pid_loop());
    let values: Vec<&str> = offers.iter().map(|offer| offer.value.as_str()).collect();
    assert_eq!(
        values,
        [
            "automatic",
            "mv_step",
            "mv_pulse",
            "setpoint_step",
            "relay",
            "prbs",
            "multisine",
            "recorded-fit",
            "supplied-model",
        ]
    );
    assert!(offers
        .iter()
        .all(|offer| offer.available && offer.reason.is_none()));
}

#[test]
fn the_labels_are_the_words_of_the_method_list() {
    let offers = identify_method_offers(&pid_loop());
    let labels: Vec<&str> = offers.iter().map(|offer| offer.label.as_str()).collect();
    assert_eq!(
        labels,
        [
            "Automatic",
            "Step response",
            "Pulse response",
            "Setpoint step",
            "Feedback relay",
            "Pseudo-random binary sequence (PRBS)",
            "Multisine",
            "Recorded-data fit",
            "Supplied plant model",
        ]
    );
}

#[test]
fn the_project_experiment_is_offered_for_a_project_scripted_loop_only() {
    let mut loop_definition = pid_loop();
    assert!(!identify_method_offers(&loop_definition)
        .iter()
        .any(|offer| offer.value == "project"));
    loop_definition
        .experiment
        .as_mut()
        .expect("the fixture declares its experiment")
        .execution = Execution::ProjectScripted;
    let offers = identify_method_offers(&loop_definition);
    let project = offer_of(&offers, "project");
    assert_eq!(project.label, "Project experiment");
    assert!(project.available);
    assert_eq!(
        offers.last().map(|offer| offer.value.as_str()),
        Some("project")
    );
}

#[test]
fn a_refused_method_carries_the_sentence_that_says_why() {
    let mut loop_definition = pid_loop();
    loop_definition
        .experiment
        .as_mut()
        .expect("the fixture declares its experiment")
        .relay_direction = None;
    let offers = identify_method_offers(&loop_definition);
    let relay = offer_of(&offers, "relay");
    assert!(!relay.available);
    assert_eq!(
        relay.reason.as_deref(),
        Some("Relay identification needs an explicitly declared process direction.")
    );
    assert!(offer_of(&offers, "automatic").available);
}

#[test]
fn a_loop_without_a_setpoint_offers_the_setpoint_step_when_a_reference_signal_of_the_process_unit_exists(
) {
    let mut loop_definition = pid_loop();
    loop_definition.signals.setpoint = None;
    let refused = identify_method_offers(&loop_definition);
    let step = offer_of(&refused, "setpoint_step");
    assert!(!step.available);
    assert_eq!(
        step.reason.as_deref(),
        Some("Setpoint-step identification needs the configured reference signal.")
    );

    loop_definition.signals.additional = Some(vec![setpoint_candidate("Main.SP2", "degC")]);
    assert!(offer_of(&identify_method_offers(&loop_definition), "setpoint_step").available);

    loop_definition.signals.additional = Some(vec![setpoint_candidate("Main.SP2", "%")]);
    assert!(!offer_of(&identify_method_offers(&loop_definition), "setpoint_step").available);
}

#[test]
fn an_offer_serializes_its_reason_only_when_the_loop_cannot_run_the_method() {
    let available = offer("automatic", "Automatic", None, true);
    assert_eq!(
        serde_json::to_value(&available).expect("the offer serializes"),
        json!({ "value": "automatic", "label": "Automatic", "available": true, "records": true })
    );
    let refused = offer("relay", "Feedback relay", Some("A reason."), true);
    assert_eq!(
        serde_json::to_value(&refused).expect("the offer serializes"),
        json!({ "value": "relay", "label": "Feedback relay", "available": false, "reason": "A reason.", "records": true })
    );
}

/// B3.2, A8.1: the page sends a chosen method only when the study records it as an acquisition setting. Every live method does,
/// whether or not the loop can run it (a refused relay is still a setting the engine records or refuses by its own rule); the
/// recorded-data fit, the supplied model and the project test start no campaign of the study's inputs, so they carry no `records`.
/// Catches a page whose list of view-only methods differs from the engine's (a project test posted as an acquisition setting, or a
/// live method never recorded), and a refused live method that the offers stop marking as recorded.
#[test]
fn method_offers_say_which_methods_the_study_records() {
    let mut loop_definition = pid_loop();
    loop_definition
        .experiment
        .as_mut()
        .expect("the fixture declares its experiment")
        .execution = Execution::ProjectScripted;
    let recorded: Vec<(String, bool)> = identify_method_offers(&loop_definition)
        .into_iter()
        .map(|offer| (offer.value, offer.records))
        .collect();
    let expected = [
        ("automatic", true),
        ("mv_step", true),
        ("mv_pulse", true),
        ("setpoint_step", true),
        ("relay", true),
        ("prbs", true),
        ("multisine", true),
        ("recorded-fit", false),
        ("supplied-model", false),
        ("project", false),
    ];
    assert_eq!(
        recorded,
        expected.map(|(value, records)| (value.to_owned(), records))
    );
    // A live method the loop cannot run is still a method the study records: the engine refuses the setting by its own rule.
    loop_definition
        .experiment
        .as_mut()
        .expect("the fixture declares its experiment")
        .relay_direction = None;
    let offers = identify_method_offers(&loop_definition);
    let relay = offer_of(&offers, "relay");
    assert!(!relay.available);
    assert!(relay.records);
    // The recorded-data fit and the supplied model are never marked, and they do not appear in a serialized offer.
    let view =
        serde_json::to_value(offer_of(&offers, "recorded-fit")).expect("the offer serializes");
    assert_eq!(
        view,
        json!({ "value": "recorded-fit", "label": "Recorded-data fit", "available": true })
    );
}

/// B3.2: the Method list's word of a recorded method is the word of its offer, and the method the loop declares is shown as
/// Automatic. Catches a list that cannot show the method the study holds, or a page-side table that names it differently.
#[test]
fn a_recorded_method_is_shown_under_the_word_of_its_offer() {
    use crate::identification::acquisition_settings::AcquisitionMethod::{
        Automatic, Configured, Multisine, MvPulse, MvStep, Prbs, Relay, SetpointStep,
    };
    let words = [
        Configured,
        Automatic,
        MvStep,
        MvPulse,
        SetpointStep,
        Relay,
        Prbs,
        Multisine,
    ]
    .map(method_word);
    assert_eq!(
        words,
        [
            "automatic",
            "automatic",
            "mv_step",
            "mv_pulse",
            "setpoint_step",
            "relay",
            "prbs",
            "multisine"
        ]
    );
    let offered: Vec<String> = identify_method_offers(&pid_loop())
        .into_iter()
        .filter(|offer| offer.records)
        .map(|offer| offer.value)
        .collect();
    assert_eq!(
        offered,
        [
            "automatic",
            "mv_step",
            "mv_pulse",
            "setpoint_step",
            "relay",
            "prbs",
            "multisine"
        ],
        "each recorded method's offer carries the word the Method list shows it under"
    );
}
