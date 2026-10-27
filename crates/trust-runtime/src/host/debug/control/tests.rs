use super::*;
use serde_json::json;

use crate::harness::{CompileSession, SourceFile, TestHarness};

mod event_buffer_bound;
mod mutation_contract;
mod state_stream_contract;

/// The commissioning example on the real Control.PID library, compiled for this computer, with a
/// control attached and one scan run at time zero, which publishes the program's state
/// (specification 44 §8.8): `MainTask`, every 100 ms, is not due yet, so the program keeps its
/// starting values.
fn commissioning_runtime() -> (TestHarness, DebugControl) {
    let mut harness = TestHarness::from_session(CompileSession::from_sources(vec![
        SourceFile::with_path(
            "Control/control.st",
            include_str!("../../../../../../libraries/control/src/control.st"),
        ),
        SourceFile::with_path(
            "src/main.st",
            include_str!("../../../../../../examples/control-pid-commissioning/src/Main.st"),
        ),
    ]))
    .expect("compile real Control.PID fixture");
    let control = DebugControl::new();
    harness.runtime_mut().set_debug_control(control.clone());
    let cycle = harness.cycle();
    assert!(cycle.errors.is_empty(), "first scan: {:?}", cycle.errors);
    (harness, control)
}

/// The value at `path` in the program as the last scan, or a task boundary, left it.
fn program_value(harness: &TestHarness, path: &str) -> Value {
    harness
        .runtime()
        .with_program_view(|program| program.value(path))
        .unwrap_or_else(|| panic!("{path} names a value"))
}

/// Specification 35 A6.8: a parameter set is written whole at its task's boundary and confirmed
/// after the task; a set the program changed after the commit is rejected, keeping the program's
/// value.
#[test]
fn nested_control_pid_parameter_transaction_adopts_and_rejects_post_st_write() {
    let (mut harness, control) = commissioning_runtime();
    let binding = json!({
        "loop_id": "nested-pid",
        "task_name": "MainTask",
        "parameters": [{
            "symbol": "P1.Controller.ParametersValue.ProportionalGain",
            "iec_type": "LREAL",
            "unit": "gain",
            "convention": "positive gain",
            "minimum": 0.0,
            "maximum": 10.0
        }],
        "mode": {"symbol": "P1.AutoMode", "manual_value": false}
    });
    let status = control
        .parameter_status(json!({"binding": binding}))
        .expect("read nested parameter status");
    let apply = control
        .parameter_apply(
            json!({
                    "transaction_id": "nested-apply",
                    "binding": binding,
                    "expected_program_revision": status["program_revision"],
                    "expected_binding_revision": status["binding_revision"],
                    "expected_active_revision": status["active_revision"],
                    "expected_values": status["values"],
            "values": {"P1.Controller.ParametersValue.ProportionalGain": 2.5},
                    "source": "simulation",
                    "physical_approved": false
                }),
            false,
            false,
        )
        .expect("stage nested parameter transaction");
    assert_eq!(apply["status"], "pending");
    // The boundary of `MainTask`: the program at its start and at its end, through the program
    // access seam the scan's access points hand over (specification 44 §13.3).
    harness
        .runtime_mut()
        .with_program(|program| control.commit_parameters("MainTask", program, false));
    assert_eq!(
        program_value(&harness, "P1.Controller.ParametersValue.ProportionalGain"),
        crate::value::Value::LReal(2.5)
    );
    assert_eq!(
        program_value(&harness, "P1.Controller.ParametersValue.IntegralGain"),
        crate::value::Value::LReal(0.0),
        "the nested sibling the set does not name keeps its value"
    );
    harness
        .runtime_mut()
        .with_program(|program| control.confirm_parameters("MainTask", program));
    // The next scan publishes the confirmed values; `MainTask` is not due at time zero.
    assert!(harness.cycle().errors.is_empty());
    let adopted = control
        .parameter_status(json!({"binding": binding}))
        .expect("read adopted nested parameter status");
    assert_eq!(adopted["status"], "applied");
    assert_eq!(
        adopted["values"]["P1.Controller.ParametersValue.ProportionalGain"],
        2.5
    );

    let status = adopted;
    control
        .parameter_apply(
            json!({
                "transaction_id": "nested-reject",
                "binding": binding,
                "expected_program_revision": status["program_revision"],
                "expected_binding_revision": status["binding_revision"],
                "expected_active_revision": status["active_revision"],
                "expected_values": status["values"],
                "values": {"P1.Controller.ParametersValue.ProportionalGain": 3.5},
                "source": "simulation",
                "physical_approved": false
            }),
            false,
            false,
        )
        .expect("stage second nested parameter transaction");
    harness
        .runtime_mut()
        .with_program(|program| control.commit_parameters("MainTask", program, false));
    // The program sets the member itself after the commit, as its task would.
    harness.runtime_mut().with_program(|program| {
        let at = program
            .locate("P1.Controller.ParametersValue.ProportionalGain")
            .expect("resolve second nested parameter");
        program
            .write_at(&at, &crate::value::Value::LReal(2.5))
            .expect("the program's own write");
    });
    harness
        .runtime_mut()
        .with_program(|program| control.confirm_parameters("MainTask", program));
    assert!(harness.cycle().errors.is_empty());
    let rejected = control
        .parameter_status(json!({"binding": binding}))
        .expect("read rejected nested parameter status");
    assert_eq!(rejected["status"], "rejected");
    assert_eq!(
        rejected["values"]["P1.Controller.ParametersValue.ProportionalGain"],
        2.5
    );
}

/// Protocol annex E7.2 and E7.4: status reports the values of the last completed scan and never an
/// outcome those values do not show. A set its task's end confirmed is reported `applied`, with the
/// value it wrote, once the scan that ran the task is published; between the task's end and that
/// publication status still reports it `pending`, with the value the published scan holds, 0.0, the
/// example's initial gain. Catches a record that turns `applied` at the task's end while status
/// still reads the scan before the commit, which a client polling for `applied` would take as the
/// confirmed value.
#[test]
fn a_confirmed_parameter_set_is_reported_applied_once_the_scan_that_confirmed_it_is_published() {
    let (mut harness, control) = commissioning_runtime();
    let gain = "P1.Controller.ParametersValue.ProportionalGain";
    let binding = json!({
        "loop_id": "published-pid",
        "task_name": "MainTask",
        "parameters": [{
            "symbol": gain,
            "iec_type": "LREAL",
            "unit": "gain",
            "convention": "positive gain",
            "minimum": 0.0,
            "maximum": 10.0
        }],
        "mode": {"symbol": "P1.AutoMode", "manual_value": false}
    });
    let before = control
        .parameter_status(json!({"binding": binding}))
        .expect("read the parameter status");
    assert_eq!(before["values"][gain], 0.0, "{before}");
    control
        .parameter_apply(
            json!({
                "transaction_id": "published-apply",
                "binding": binding,
                "expected_program_revision": before["program_revision"],
                "expected_binding_revision": before["binding_revision"],
                "expected_active_revision": before["active_revision"],
                "expected_values": before["values"],
                "values": {gain: 2.5},
                "source": "simulation",
                "physical_approved": false
            }),
            false,
            false,
        )
        .expect("stage the parameter set");
    // `MainTask`'s start and end, through the program access seam (specification 44 §13.3); the
    // task's programs leave the gain alone.
    harness
        .runtime_mut()
        .with_program(|program| control.commit_parameters("MainTask", program, false));
    harness
        .runtime_mut()
        .with_program(|program| control.confirm_parameters("MainTask", program));
    assert_eq!(
        program_value(&harness, gain),
        crate::value::Value::LReal(2.5)
    );
    let confirmed_unpublished = control
        .parameter_status(json!({"binding": binding}))
        .expect("read the status before the scan's end");
    assert_eq!(confirmed_unpublished["status"], "pending");
    assert_eq!(confirmed_unpublished["values"][gain], 0.0);
    assert_eq!(
        confirmed_unpublished["active_revision"],
        before["active_revision"]
    );
    // The scan's end publishes the program the task left.
    assert!(harness.cycle().errors.is_empty());
    let published = control
        .parameter_status(json!({"binding": binding}))
        .expect("read the status after the scan's end");
    assert_eq!(published["status"], "applied");
    assert_eq!(published["values"][gain], 2.5);
    assert_eq!(published["previous_values"][gain], 0.0);
}

/// Specification 35 A6.3: a controller transfer is one transaction at its owning task's boundary:
/// its states are written before the task runs, the members it does not write keep their values,
/// and it is applied once the scan that ran it completes.
#[test]
fn nested_control_pid_transfer_transaction_adopts_state_at_task_boundary() {
    let (mut harness, control) = commissioning_runtime();
    let binding = json!({
        "loop_id": "nested-transfer",
        "task_name": "MainTask",
        "mode": {"symbol": "P1.AutoMode", "manual_value": false},
        "states": [{
            "symbol": "P1.Controller.StateValue.NominalOutput",
            "iec_type": "LREAL",
            "unit": "output",
            "minimum": -100.0,
            "maximum": 100.0
        }],
        "guards": [{
            "symbol": "P1.Controller.StateValue.ProcessValue",
            "iec_type": "LREAL",
            "unit": "process",
            "minimum": -100.0,
            "maximum": 100.0
        }],
        "output": {
            "symbol": "P1.Controller.StateValue.SaturatedOutput",
            "iec_type": "LREAL",
            "unit": "output",
            "minimum": -100.0,
            "maximum": 100.0
        },
        "additional_outputs": [],
        "tracking_outputs": {}
    });
    let status = control
        .transfer_status(json!({"binding": binding}))
        .expect("read nested transfer status");
    let initial_process_value = status["values"]["P1.Controller.StateValue.ProcessValue"]
        .as_f64()
        .expect("initial nested guard value");
    control
        .transfer_apply(
            json!({
                "transaction_id": "nested-transfer-apply",
                "binding": binding,
                "expected_program_revision": status["program_revision"],
                "expected_binding_revision": status["binding_revision"],
                "expected_values": status["values"],
                "values": {"P1.Controller.StateValue.NominalOutput": 7.5},
                "manual": true,
                "maximum_output_change": 100.0,
                "source": "simulation",
                "physical_approved": false,
                "tracking": null
            }),
            false,
        )
        .expect("stage nested transfer transaction");
    // The boundary of `MainTask`, through the program access seam (specification 44 §13.3).
    harness
        .runtime_mut()
        .with_program(|program| control.commit_transfer("MainTask", program, false));
    assert_eq!(
        program_value(&harness, "P1.Controller.StateValue.NominalOutput"),
        crate::value::Value::LReal(7.5)
    );
    assert_eq!(
        program_value(&harness, "P1.Controller.StateValue.ProcessValue"),
        crate::value::Value::LReal(initial_process_value)
    );
    harness
        .runtime_mut()
        .with_program(|program| control.confirm_transfer_task("MainTask", program));
    // The scan's end, after its outputs were committed.
    control.finish_transfer_cycle();
    let confirmed = control
        .transfer_status(json!({"binding": binding}))
        .expect("read confirmed nested transfer status");
    assert_eq!(confirmed["status"], "applied");
}

/// Specification 35, control signal reads: a symbol path names a scalar leaf inside a structure
/// member of a function block instance of a program, which is read and written at its place. A
/// resolution that stops at the structure, or reads its first field's bytes as the leaf, fails here.
#[test]
fn resolves_scalar_leaf_inside_nested_controller_struct() {
    let mut harness = TestHarness::from_source(
        r#"
TYPE PIDParameters : STRUCT
    IntegralGain : LREAL := 0.5;
    ProportionalGain : LREAL := 2.5;
END_STRUCT END_TYPE
FUNCTION_BLOCK PIDBlock
VAR ParametersValue : PIDParameters; END_VAR
END_FUNCTION_BLOCK
PROGRAM Main
VAR Controller : PIDBlock; END_VAR
END_PROGRAM
"#,
    )
    .expect("compile the nested controller fixture");
    harness.runtime_mut().with_program(|program| {
        let path = "Main.Controller.ParametersValue.ProportionalGain";
        let resolved = parameters::resolve_path(&*program, path)
            .expect("nested controller scalar leaf should resolve");
        assert_eq!(resolved.value, crate::value::Value::LReal(2.5));
        program
            .write_at(&resolved.at, &crate::value::Value::LReal(3.5))
            .expect("write the leaf at its place");
        let observed = parameters::resolve_path(&*program, path)
            .expect("nested controller scalar leaf should remain resolvable");
        assert_eq!(observed.value, crate::value::Value::LReal(3.5));
        assert_eq!(observed.at, resolved.at);
        assert_eq!(
            parameters::resolve_path(&*program, "Main.Controller.ParametersValue.IntegralGain")
                .expect("the sibling field")
                .value,
            crate::value::Value::LReal(0.5),
            "the write reaches only its own field"
        );
    });
}
