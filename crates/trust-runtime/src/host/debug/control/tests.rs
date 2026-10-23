use super::*;
use serde_json::json;
use std::sync::mpsc::{channel, Receiver};
use std::thread;
use std::time::Duration;

mod event_buffer_bound;
mod lifecycle_contract;
mod mutation_contract;
mod state_stream_contract;

fn recv_stop(stop_rx: &Receiver<DebugStop>, timeout: Duration, label: &str) -> DebugStop {
    stop_rx
        .recv_timeout(timeout)
        .unwrap_or_else(|err| panic!("{label}: {err:?}"))
}

#[test]
fn breakpoint_clears_pending_pause() {
    let control = DebugControl::new();
    let location = SourceLocation::new(0, 0, 5);
    control.set_breakpoints_for_file(0, vec![DebugBreakpoint::new(location)]);

    let (stop_tx, stop_rx) = channel();
    control.set_stop_sender(stop_tx);

    {
        let (lock, _) = &*control.state;
        let mut state = lock.lock().expect("debug state poisoned");
        state.pending_stop = Some(DebugStopReason::Pause);
        state.mode = DebugMode::Running;
    }

    let mut hook = control.clone();
    let handle = thread::spawn(move || {
        hook.on_statement(Some(&location), 0);
    });

    let stop = recv_stop(&stop_rx, Duration::from_secs(2), "breakpoint stop");
    assert_eq!(stop.reason, DebugStopReason::Breakpoint);

    control.continue_run();
    handle.join().unwrap();

    let (lock, _) = &*control.state;
    let state = lock.lock().expect("debug state poisoned");
    assert!(state.pending_stop.is_none());
}

#[test]
fn pause_after_continue_while_waiting_emits_pause_stop() {
    let control = DebugControl::new();
    let location = SourceLocation::new(0, 0, 5);
    control.set_breakpoints_for_file(0, vec![DebugBreakpoint::new(location)]);

    let (stop_tx, stop_rx) = channel();
    control.set_stop_sender(stop_tx);

    let mut hook = control.clone();
    let handle = thread::spawn(move || {
        hook.on_statement(Some(&location), 0);
    });

    let first = recv_stop(&stop_rx, Duration::from_secs(2), "breakpoint stop");
    assert_eq!(first.reason, DebugStopReason::Breakpoint);

    // Race window: continue and immediately pause again while the first hook may still be waking.
    control.continue_run();
    control.pause();

    // Always drive a fresh statement boundary after pause so a pending pause stop is consumed
    // deterministically even if the first hook already exited after continue.
    // Use a different location so this fallback path cannot hit the original breakpoint.
    let second_location = SourceLocation::new(1, 0, 5);
    let mut second_hook = control.clone();
    let second_handle = thread::spawn(move || {
        second_hook.on_statement(Some(&second_location), 0);
    });

    let second = recv_stop(&stop_rx, Duration::from_secs(2), "pause stop");
    assert_eq!(second.reason, DebugStopReason::Pause);

    control.continue_run();
    handle.join().expect("hook thread joins");
    second_handle.join().expect("second hook thread joins");
}

#[test]
fn nested_control_pid_parameter_transaction_adopts_and_rejects_post_st_write() {
    let mut runtime = crate::harness::CompileSession::from_sources(vec![
        crate::harness::SourceFile::with_path(
            "Control/control.st",
            include_str!("../../../../../../libraries/control/src/control.st"),
        ),
        crate::harness::SourceFile::with_path(
            "src/tests.st",
            include_str!("../../../../../../examples/control-pid-commissioning/src/Main.st"),
        ),
    ])
    .build_runtime()
    .expect("compile real Control.PID fixture");
    let control = runtime.enable_debug();
    runtime.execute_cycle().expect("execute Control.PID task");
    let metadata = runtime.metadata_snapshot();
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
    {
        let (lock, _) = &*control.state;
        lock.lock().expect("debug state").snapshot = Some(crate::debug::DebugSnapshot {
            storage_abi: runtime.prepared().storage_abi(),
            storage: runtime.storage().clone(),
            now: crate::value::Duration::from_millis(100),
        });
    }
    let status = control
        .parameter_status(json!({"binding": binding}), &metadata)
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
            &metadata,
            false,
            false,
        )
        .expect("stage nested parameter transaction");
    assert_eq!(apply["status"], "pending");
    let mut task_storage = runtime.storage().clone();
    control.commit_parameters("MainTask", &mut task_storage, &metadata, false, &[]);
    let proportional = parameters::resolve_path(
        &task_storage,
        "P1.Controller.ParametersValue.ProportionalGain",
    )
    .expect("committed nested parameter")
    .value;
    assert_eq!(proportional, crate::value::Value::LReal(2.5));
    let sibling =
        parameters::resolve_path(&task_storage, "P1.Controller.ParametersValue.IntegralGain")
            .expect("preserved nested sibling")
            .value;
    assert_eq!(sibling, crate::value::Value::LReal(0.0));
    control.confirm_parameters(
        "MainTask",
        &mut task_storage,
        crate::value::Duration::from_millis(200),
        crate::value::StorageAbi::default(),
    );
    let adopted = control
        .parameter_status(json!({"binding": binding}), &metadata)
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
            &metadata,
            false,
            false,
        )
        .expect("stage second nested parameter transaction");
    control.commit_parameters("MainTask", &mut task_storage, &metadata, false, &[]);
    let reference = parameters::resolve_path(
        &task_storage,
        "P1.Controller.ParametersValue.ProportionalGain",
    )
    .expect("resolve second nested parameter")
    .reference;
    assert!(task_storage.write_by_ref(reference, crate::value::Value::LReal(2.5)));
    control.confirm_parameters(
        "MainTask",
        &mut task_storage,
        crate::value::Duration::from_millis(300),
        crate::value::StorageAbi::default(),
    );
    let rejected = control
        .parameter_status(json!({"binding": binding}), &metadata)
        .expect("read rejected nested parameter status");
    assert_eq!(rejected["status"], "rejected");
    assert_eq!(
        rejected["values"]["P1.Controller.ParametersValue.ProportionalGain"],
        2.5
    );
}

#[test]
fn nested_control_pid_transfer_transaction_adopts_state_at_task_boundary() {
    let mut runtime = crate::harness::CompileSession::from_sources(vec![
        crate::harness::SourceFile::with_path(
            "Control/control.st",
            include_str!("../../../../../../libraries/control/src/control.st"),
        ),
        crate::harness::SourceFile::with_path(
            "src/main.st",
            include_str!("../../../../../../examples/control-pid-commissioning/src/Main.st"),
        ),
    ])
    .build_runtime()
    .expect("compile real Control.PID fixture");
    let control = runtime.enable_debug();
    runtime.execute_cycle().expect("execute Control.PID task");
    let metadata = runtime.metadata_snapshot();
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
    {
        let (lock, _) = &*control.state;
        lock.lock().expect("debug state").snapshot = Some(crate::debug::DebugSnapshot {
            storage_abi: runtime.prepared().storage_abi(),
            storage: runtime.storage().clone(),
            now: crate::value::Duration::from_millis(100),
        });
    }
    let status = control
        .transfer_status(json!({"binding": binding}), &metadata)
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
            &metadata,
            false,
        )
        .expect("stage nested transfer transaction");
    let mut task_storage = runtime.storage().clone();
    control.commit_transfer("MainTask", &mut task_storage, &metadata, false, &[]);
    assert_eq!(
        parameters::resolve_path(&task_storage, "P1.Controller.StateValue.NominalOutput")
            .expect("committed nested state")
            .value,
        crate::value::Value::LReal(7.5)
    );
    let process_value =
        parameters::resolve_path(&task_storage, "P1.Controller.StateValue.ProcessValue")
            .expect("preserved nested guard")
            .value;
    assert_eq!(
        process_value,
        crate::value::Value::LReal(initial_process_value)
    );
    control.confirm_transfer_task("MainTask", &mut task_storage);
    control.finish_transfer_cycle(
        &task_storage,
        crate::value::Duration::from_millis(200),
        crate::value::StorageAbi::default(),
    );
    let confirmed = control
        .transfer_status(json!({"binding": binding}), &metadata)
        .expect("read confirmed nested transfer status");
    assert_eq!(confirmed["status"], "applied");
}
