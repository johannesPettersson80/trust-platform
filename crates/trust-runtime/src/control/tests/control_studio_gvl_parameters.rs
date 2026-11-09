// Protocol annex E2.3 and E7.1 (the CODESYS toolkit's S9: "parameters live in GVLs"): a parameter set writes the parameters
// and mode a CODESYS project keeps in GVLs when the loop's task owns each global (specification 44 §5.7), and refuses one that
// another task or no single task owns with its reason. An experiment steps a writable global as it steps a program's variable.

/// A CODESYS-style loop: its mode and manual valve in `GVL_IO`, its gains in `GVL_Par`, its law in `Tank`, which task `Fast`
/// runs. `Other`, in task `Slow`, writes `lrKd`, and the function block `FB_Tune` writes `lrKf`; no program writes the rest.
const GVL_PARAMETERS_SOURCE: &str = r#"
NAMESPACE GVL_IO
VAR_GLOBAL
    bManual : BOOL := TRUE;
    lrManualValve : LREAL := 2.0;
END_VAR
END_NAMESPACE

NAMESPACE GVL_Par
VAR_GLOBAL
    lrKp : LREAL := 2.0;
    lrKi : LREAL := 0.5;
    lrKd : LREAL;
    lrKf : LREAL;
END_VAR
END_NAMESPACE

FUNCTION_BLOCK FB_Tune
GVL_Par.lrKf := 0.25;
END_FUNCTION_BLOCK

PROGRAM Tank
VAR
    Observed : LREAL;
    Tune : FB_Tune;
END_VAR
Observed := GVL_Par.lrKp + GVL_Par.lrKi;
Tune();
END_PROGRAM

PROGRAM Other
GVL_Par.lrKd := 1.0;
END_PROGRAM

CONFIGURATION Plant
RESOURCE Res ON PLC
    TASK Fast (INTERVAL := T#10ms, PRIORITY := 1);
    TASK Slow (INTERVAL := T#100ms, PRIORITY := 2);
    PROGRAM TankInst WITH Fast : Tank;
    PROGRAM OtherInst WITH Slow : Other;
END_RESOURCE
END_CONFIGURATION
"#;

/// The loop's parameter set in task `Fast`, with the gains `symbols` of `GVL_Par`.
fn gvl_parameter_binding(symbols: &[&str]) -> serde_json::Value {
    let parameters: Vec<serde_json::Value> = symbols
        .iter()
        .map(|symbol| json!({"symbol":format!("GVL_Par.{symbol}"),"iec_type":"LREAL","unit":"1","convention":"parallel","minimum":0,"maximum":10}))
        .collect();
    json!({"loop_id":"gvl-parameters","task_name":"Fast","parameters":parameters,
        "mode":{"symbol":"GVL_IO.bManual","manual_value":true}})
}

fn gvl_parameter_status(state: &ControlState, binding: &serde_json::Value) -> ControlResponse {
    handle_request_value(
        json!({"id":8900,"type":"control.parameters.status","params":{"binding":binding}}),
        state,
        None,
    )
}

/// Worked from the project: no program writes `lrKp` or `lrKi`, so task `Fast` may hold them. The set writes Kp 4 and Ki 1 at
/// `Fast`'s first run, 10 ms after the start, before the task executes, so `Tank` computes 4 + 1 = 5. Catches a GVL parameter
/// refused as "parameter root is not a compiled program" (the toolkit's S9), and a set committed outside its task's boundary.
#[test]
fn a_parameter_set_writes_the_gvl_parameters_its_task_owns() {
    let (state, mut harness) = generic_fixture_for(GVL_PARAMETERS_SOURCE);
    let binding = gvl_parameter_binding(&["lrKp", "lrKi"]);
    let status = gvl_parameter_status(&state, &binding);
    assert!(status.ok, "{:?}", status.error);
    let status = status.result.unwrap();
    assert_eq!(status["values"]["GVL_Par.lrKp"], 2.0);
    assert_eq!(status["values"]["GVL_Par.lrKi"], 0.5);
    let apply = handle_request_value(
        json!({"id":8901,"type":"control.parameters.apply","params":{
            "transaction_id":"gvl-parameters-1","binding":binding,"expected_program_revision":status["program_revision"],
            "expected_binding_revision":status["binding_revision"],"expected_active_revision":status["active_revision"],
            "expected_values":status["values"],"values":{"GVL_Par.lrKp":4.0,"GVL_Par.lrKi":1.0},"source":"simulation",
            "physical_approved":false
        }}),
        &state,
        None,
    );
    assert!(apply.ok, "{:?}", apply.error);
    // A task with an interval runs first one interval after the start.
    harness.advance_time(crate::value::Duration::from_millis(10));
    assert!(harness.cycle().errors.is_empty());
    let after = gvl_parameter_status(&state, &binding).result.unwrap();
    assert_eq!(after["status"], "applied", "{after}");
    assert_eq!(studio_value(&harness, "TankInst.Observed"), Value::LReal(5.0));
}

/// Worked from the project: `lrKd` is written by `Other`, which task `Slow` runs, and `lrKf` by the function block `FB_Tune`,
/// which any task may run; a set of the loop in task `Fast` may hold neither. Each refusal names the global and why. Catches a set
/// that writes a global another task writes at the same time.
#[test]
fn a_parameter_set_refuses_a_gvl_parameter_its_task_does_not_own() {
    let (state, _harness) = generic_fixture_for(GVL_PARAMETERS_SOURCE);
    for (symbol, reason) in [
        (
            "lrKd",
            "parameter GVL_Par.lrKd is a global that task Slow writes, not the loop's task Fast",
        ),
        (
            "lrKf",
            "parameter GVL_Par.lrKf is a global that no single task owns",
        ),
    ] {
        let response = gvl_parameter_status(&state, &gvl_parameter_binding(&["lrKp", symbol]));
        assert!(!response.ok, "{symbol}");
        assert!(refusal_text(&response).contains(reason), "{}", refusal_text(&response));
    }
}

/// Worked from the project: an output step of the loop in task `Fast` writes the manual valve that `GVL_IO` keeps, as it would
/// write a program's variable. Catches a GVL target refused as "name a variable of a compiled program".
#[test]
fn an_experiment_steps_a_gvl_output() {
    let state = control_test_state(GVL_PARAMETERS_SOURCE);
    state.connect_experiments();
    state.experiments.record_simulated_target(true);
    with_fresh_scan(&state);
    let started = handle_request_value(
        json!({"id": 8902, "type": "control.experiment.start", "params": {
            "experiment_id": "gvl-step-1", "source": "simulation", "kind": "output_step", "task": "Fast",
            "symbol": "GVL_IO.lrManualValve", "value": 4.0, "restoration_value": 2.0, "minimum": 0, "maximum": 10,
            "duration_ms": 5_000, "expected_scan": 7,
            "mode_binding": {"symbol": "GVL_IO.bManual", "manual_value": true}
        }}),
        &state,
        None,
    );
    assert!(started.ok, "start failed: {:?}", started.error);
    let result = started.result.unwrap();
    assert_eq!(result["status"], "running");
    assert_eq!(result["symbol"], "GVL_IO.lrManualValve");
}
