use serde_json::json;

use super::*;
use crate::identification::texts::method_refusal_text;

/// The PID loop of the method tests: a setpoint, a measured output and a manipulated variable, a Manual command, a
/// mode binding and a declared positive relay direction.
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

fn refusal(loop_definition: &LoopDefinition, method: AcquisitionMethod) -> Option<MethodRefusal> {
    match method_applicability(loop_definition, method) {
        Applicability::Available => None,
        Applicability::Unavailable(reason) => Some(reason),
    }
}

#[test]
fn relay_reports_the_missing_declared_process_direction() {
    let mut loop_definition = pid_loop();
    if let Some(experiment) = loop_definition.experiment.as_mut() {
        experiment.relay_direction = None;
    }
    assert_eq!(
        refusal(&loop_definition, AcquisitionMethod::Relay),
        Some(MethodRefusal::RelayDirection)
    );
    assert!(
        method_refusal_text(MethodRefusal::RelayDirection).contains("process direction"),
        "the sentence names the direction"
    );
}

#[test]
fn frequency_excitation_is_refused_for_an_on_off_actuator() {
    let mut loop_definition = pid_loop();
    loop_definition.controller.on_off = Some(crate::loop_definition::OnOffController::from_value(&json!({
        "hysteresisSymbol": "P1.HysteresisC", "minimumOnTimeSymbol": "P1.MinimumOnS", "minimumOffTimeSymbol": "P1.MinimumOffS",
        "bandConvention": "half-band", "lowOutput": -20.0, "highOutput": 20.0,
        "initialization": { "kind": "declared-state-v1", "states": [{
            "symbol": "P1.CoolingOn", "iec_type": "BOOL", "unit": "1", "minimum": 0.0, "maximum": 1.0,
            "initial": { "source": "model-input-is", "index": 0, "value": 20.0 }, "postScan": "bounded"
        }] }
    }))
    .expect("the ON/OFF block decodes"));
    assert_eq!(
        refusal(&loop_definition, AcquisitionMethod::Prbs),
        Some(MethodRefusal::FrequencyOnOff)
    );
    assert_eq!(
        refusal(&loop_definition, AcquisitionMethod::Multisine),
        Some(MethodRefusal::FrequencyOnOff)
    );
}

#[test]
fn a_setpoint_step_needs_the_configured_reference_and_automatic_always_applies() {
    let mut loop_definition = pid_loop();
    assert_eq!(
        refusal(&loop_definition, AcquisitionMethod::SetpointStep),
        None
    );
    loop_definition.signals.setpoint = None;
    assert_eq!(
        refusal(&loop_definition, AcquisitionMethod::SetpointStep),
        Some(MethodRefusal::SetpointReference)
    );
    assert_eq!(
        refusal(&loop_definition, AcquisitionMethod::Automatic),
        None
    );
}

#[test]
fn an_open_loop_method_needs_the_manual_command_and_the_mode_binding() {
    let mut loop_definition = pid_loop();
    assert_eq!(refusal(&loop_definition, AcquisitionMethod::MvStep), None);
    loop_definition.controller.manual_output_signal = None;
    assert_eq!(
        refusal(&loop_definition, AcquisitionMethod::MvStep),
        Some(MethodRefusal::OpenLoopBindings)
    );
    assert_eq!(
        refusal(&loop_definition, AcquisitionMethod::Relay),
        Some(MethodRefusal::RelayManualOwnership),
        "a relay needs the same command and mode ownership"
    );
}

#[test]
fn configured_applies_the_declared_method_or_falls_back_to_automatic() {
    let mut loop_definition = pid_loop();
    if let Some(experiment) = loop_definition.experiment.as_mut() {
        experiment.kind = ExperimentKind::Relay;
    }
    assert_eq!(
        refusal(&loop_definition, AcquisitionMethod::Configured),
        None
    );
    if let Some(experiment) = loop_definition.experiment.as_mut() {
        experiment.relay_direction = None;
        experiment.kind = ExperimentKind::LoadStep;
    }
    assert_eq!(
        refusal(&loop_definition, AcquisitionMethod::Configured),
        None,
        "a load step is not offered as a method, so the configured method is automatic"
    );
}
