// Protocol annex E2.3 (the CODESYS toolkit's S9): a transfer binds the members of a loop that a CODESYS project keeps in a GVL
// when the loop's task owns each global it writes (specification 44 §5.7): no program writes it, or only the programs that task
// runs do. A global that another task's program writes, or one that a function block writes from two tasks' programs, is refused
// with its reason. A guard, which the transfer only reads, may be any declared member.

/// A CODESYS-style tank: its mode, setpoint, measurement and valve in the GVL `GVL_IO`, its law in `Tank`, which task `Fast`
/// runs; `Other`, in task `Slow`, writes `lrSlowValve` and `lrSlowLimit`, and the function block `FB_Drive` writes
/// `lrBlockValve` from both programs. `lrSlowLimit`, `lrBlockValve` and `OtherInst.rLimit` keep their values from scan to scan.
const GVL_LOOP_SOURCE: &str = r#"
NAMESPACE GVL_IO
VAR_GLOBAL
    bManual : BOOL := TRUE;
    lrSetpoint : LREAL := 10.0;
    lrLevel : LREAL := 10.0;
    lrManualValve : LREAL := 2.0;
    lrValve : LREAL;
    lrSlowValve : LREAL;
    lrSlowLimit : LREAL;
    lrBlockValve : LREAL;
END_VAR
END_NAMESPACE

FUNCTION_BLOCK FB_Drive
GVL_IO.lrBlockValve := 1.0;
END_FUNCTION_BLOCK

PROGRAM Tank
VAR
    Kp : LREAL := 2.0;
    Drive : FB_Drive;
END_VAR
IF GVL_IO.bManual THEN
    GVL_IO.lrValve := GVL_IO.lrManualValve;
ELSE
    GVL_IO.lrValve := Kp * (GVL_IO.lrSetpoint - GVL_IO.lrLevel);
END_IF;
Drive();
END_PROGRAM

PROGRAM Other
VAR
    rLimit : LREAL := 4.0;
    Drive : FB_Drive;
END_VAR
GVL_IO.lrSlowValve := GVL_IO.lrSlowValve + 1.0;
GVL_IO.lrSlowLimit := 5.0;
Drive();
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

/// The tank's loop in `GVL_IO`, run by `task`, with `output` as its output. Its guards are the level, which no program writes, a
/// limit that task `Slow` writes, the valve that `FB_Drive` writes and a member of `OtherInst`: a transfer only reads them.
fn gvl_transfer_binding(task: &str, output: &str) -> serde_json::Value {
    let number = |symbol: &str, unit: &str| json!({"symbol":symbol,"iec_type":"LREAL","unit":unit,"minimum":-20,"maximum":20});
    json!({"loop_id":"gvl-loop","task_name":task,"mode":{"symbol":"GVL_IO.bManual","manual_value":true},
        "states":[number("GVL_IO.lrSetpoint","m"),number("GVL_IO.lrManualValve","V")],
        "guards":[number("GVL_IO.lrLevel","m"),number("GVL_IO.lrSlowLimit","m"),number("GVL_IO.lrBlockValve","V"),
            number("OtherInst.rLimit","m")],
        "output":number(output,"V")})
}

fn gvl_transfer_status(state: &ControlState, binding: &serde_json::Value) -> ControlResponse {
    handle_request_value(
        json!({"id":8800,"type":"control.transfer.status","params":{"binding":binding}}),
        state,
        None,
    )
}

/// Worked from the project: task `Fast` (index 0) owns `lrValve`, so its symbol's owner byte is 1; `Slow` (index 1) owns
/// `lrSlowValve`, byte 2; no program writes `bManual`, byte 128; `FB_Drive`, which programs of both tasks call, writes
/// `lrBlockValve`, so no task owns it, byte 0
/// (specification 44 §5.7). The transfer of the loop in task `Fast` then reads Manual and the valve's 2.0, and to Auto after the
/// setpoint 11 gives the law's 2·(11 − 10) = 2. Its guards, which another task, a function block and another program write or
/// hold, are only read. Catches a GVL loop refused as "transfer members must belong to one compiled program" (the toolkit's S9),
/// a guard held to the loop's task as what it writes is, and owner bytes in the wrong place or order.
#[test]
fn a_transfer_binds_a_gvl_loop_whose_task_owns_its_globals() {
    let mut state = simulated_control_state(GVL_LOOP_SOURCE);
    let mut harness = simulated_harness(GVL_LOOP_SOURCE);
    state.debug = studio_debug(&mut harness);
    assert!(harness.cycle().errors.is_empty());
    // A task with an interval runs first one interval after the start: at 100 ms both have run once.
    harness.advance_time(crate::value::Duration::from_millis(100));
    assert!(harness.cycle().errors.is_empty());
    let owners: Vec<u8> = harness.runtime().with_program_view(|program| {
        let image = program.program_image();
        [
            "GVL_IO.LRVALVE",
            "GVL_IO.LRSLOWVALVE",
            "GVL_IO.BMANUAL",
            "GVL_IO.LRBLOCKVALVE",
        ]
        .iter()
        .map(|name| image.find_symbol(name).map_or(255, |(_, symbol)| symbol.owner))
        .collect()
    });
    assert_eq!(owners, [1, 2, 128, 0]);

    let binding = gvl_transfer_binding("Fast", "GVL_IO.lrValve");
    let before = gvl_transfer_status(&state, &binding);
    assert!(before.ok, "{:?}", before.error);
    let before = before.result.unwrap();
    assert_eq!(before["values"]["GVL_IO.bManual"], true, "Manual reads TRUE");
    assert_eq!(before["values"]["GVL_IO.lrValve"], 2.0);
    // The guards as the tasks' first run left them: `Other` wrote 5.0, `FB_Drive` 1.0, and `rLimit` keeps its 4.0.
    assert_eq!(before["values"]["GVL_IO.lrSlowLimit"], 5.0);
    assert_eq!(before["values"]["GVL_IO.lrBlockValve"], 1.0);
    assert_eq!(before["values"]["OtherInst.rLimit"], 4.0);
    let apply = handle_request_value(
        json!({"id":8801,"type":"control.transfer.apply","params":{
        "transaction_id":"gvl-transfer-1","binding":binding,
        "expected_program_revision":before["program_revision"],"expected_binding_revision":before["binding_revision"],
        "expected_values":before["values"],"values":{"GVL_IO.lrSetpoint":11.0,"GVL_IO.lrManualValve":2.0},
        "manual":false,"maximum_output_change":0.000001,"source":"simulation","physical_approved":false}}),
        &state,
        None,
    );
    assert!(apply.ok, "{:?}", apply.error);
    // The transfer commits at the boundary of task `Fast`'s next run, 10 ms later; `Slow` next runs at 200 ms.
    harness.advance_time(crate::value::Duration::from_millis(10));
    assert!(harness.cycle().errors.is_empty());
    let after = gvl_transfer_status(&state, &binding).result.unwrap();
    assert_eq!(after["status"], "applied", "{after}");
    assert_eq!(after["values"]["GVL_IO.bManual"], false, "Auto reads FALSE");
    assert_eq!(after["values"]["GVL_IO.lrValve"], 2.0);
}

/// Worked from the project: the loop in task `Slow` may not hold `lrValve`, which `Fast` writes; the loop in `Fast` may not hold
/// `lrSlowValve`, which `Slow` writes, nor `lrBlockValve`, which `FB_Drive` writes from both tasks, as its output, nor `lrSlowLimit`
/// as a state.
/// Each refusal names the global and why. Catches a transfer that writes a global another task writes at the same time, and one
/// that a block in any task writes.
#[test]
fn a_transfer_refuses_a_global_its_task_does_not_own() {
    let (state, _harness) = {
        let mut state = simulated_control_state(GVL_LOOP_SOURCE);
        let mut harness = simulated_harness(GVL_LOOP_SOURCE);
        state.debug = studio_debug(&mut harness);
        assert!(harness.cycle().errors.is_empty());
        (state, harness)
    };
    let mut slow_state = gvl_transfer_binding("Fast", "GVL_IO.lrValve");
    slow_state["guards"] = json!([{"symbol":"GVL_IO.lrLevel","iec_type":"LREAL","unit":"m","minimum":-20,"maximum":20}]);
    slow_state["states"][1]["symbol"] = json!("GVL_IO.lrSlowLimit");
    let output = |task: &str, symbol: &str| {
        let mut binding = gvl_transfer_binding(task, symbol);
        // The output is no guard as well.
        binding["guards"] = json!([{"symbol":"GVL_IO.lrLevel","iec_type":"LREAL","unit":"m","minimum":-20,"maximum":20}]);
        binding
    };
    for (binding, reason) in [
        (
            output("Slow", "GVL_IO.lrValve"),
            "transfer member GVL_IO.lrValve is a global that task Fast writes, not the loop's task Slow",
        ),
        (
            output("Fast", "GVL_IO.lrSlowValve"),
            "transfer member GVL_IO.lrSlowValve is a global that task Slow writes, not the loop's task Fast",
        ),
        (
            output("Fast", "GVL_IO.lrBlockValve"),
            "transfer member GVL_IO.lrBlockValve is a global that no single task owns",
        ),
        (
            slow_state,
            "transfer member GVL_IO.lrSlowLimit is a global that task Slow writes, not the loop's task Fast",
        ),
    ] {
        let response = gvl_transfer_status(&state, &binding);
        assert!(!response.ok, "{binding}");
        assert!(refusal_text(&response).contains(reason), "{}", refusal_text(&response));
    }
}
