use std::sync::mpsc::{self, RecvTimeoutError};
use std::time::{Duration as StdDuration, Instant};

use trust_runtime::error::RuntimeError;
use trust_runtime::execution_backend::ExecutionBackend;
use trust_runtime::harness::{bytecode_bytes_from_source, CompileSession, TestHarness};
use trust_runtime::io::IoDriver;
use trust_runtime::scheduler::{ResourceCommand, ResourceRunner, StdClock};
use trust_runtime::value::{Duration, Value};

fn vm_harness(source: &str) -> TestHarness {
    let mut harness = TestHarness::from_source(source).expect("compile harness");
    let bytes = bytecode_bytes_from_source(source).expect("build bytecode");
    harness
        .runtime_mut()
        .apply_bytecode_bytes(&bytes, None)
        .expect("apply bytecode");
    harness
        .runtime_mut()
        .set_execution_backend(ExecutionBackend::BytecodeVm)
        .expect("select vm backend");
    harness
        .runtime_mut()
        .restart(trust_runtime::RestartMode::Cold)
        .expect("restart runtime");
    harness
}

fn assert_numeric_output(harness: &TestHarness, name: &str, expected: i64) {
    let value = numeric_output(harness, name).unwrap_or_else(|| panic!("missing output '{name}'"));
    assert_eq!(value, expected, "unexpected value for '{name}'");
}

fn numeric_output(harness: &TestHarness, name: &str) -> Option<i64> {
    let actual = harness.get_output(name)?;
    Some(match actual {
        Value::Int(v) => i64::from(v),
        Value::DInt(v) => i64::from(v),
        Value::LInt(v) => v,
        other => panic!("unexpected numeric type for '{name}': {other:?}"),
    })
}

#[derive(Debug)]
struct SleepOnReadDriver {
    entered: mpsc::Sender<()>,
    release: mpsc::Receiver<()>,
    reads: usize,
}

impl SleepOnReadDriver {
    fn new(entered: mpsc::Sender<()>, release: mpsc::Receiver<()>) -> Self {
        Self {
            entered,
            release,
            reads: 0,
        }
    }
}

impl IoDriver for SleepOnReadDriver {
    fn read_inputs(&mut self, _inputs: &mut [u8]) -> Result<(), RuntimeError> {
        if self.reads == 0 {
            let _ = self.entered.send(());
            let _ = self.release.recv_timeout(StdDuration::from_secs(2));
        }
        self.reads = self.reads.saturating_add(1);
        Ok(())
    }

    fn write_outputs(&mut self, _outputs: &[u8]) -> Result<(), RuntimeError> {
        Ok(())
    }
}

#[test]
fn hot_reload_cycle_boundary_contract_holds_for_vm_reload() {
    let source_v1 = r#"
PROGRAM Main
VAR
    count : INT := 0;
END_VAR
count := count + 1;
END_PROGRAM
"#;

    let source_v2 = r#"
PROGRAM Main
VAR
    count : INT := 0;
END_VAR
count := count + 10;
END_PROGRAM
"#;

    let session_v1 = CompileSession::from_source(source_v1);
    let session_v2 = CompileSession::from_source(source_v2);
    let mut runtime = session_v1.build_runtime().expect("build runtime");
    let initial_bytes = session_v1
        .build_bytecode_bytes()
        .expect("build bytecode v1");
    let updated_bytes = session_v2
        .build_bytecode_bytes()
        .expect("build bytecode v2");

    runtime
        .apply_bytecode_bytes(&initial_bytes, None)
        .expect("apply bytecode v1");
    runtime
        .set_execution_backend(ExecutionBackend::BytecodeVm)
        .expect("select vm backend");
    runtime
        .restart(trust_runtime::RestartMode::Cold)
        .expect("restart runtime");
    let (read_entered_tx, read_entered_rx) = mpsc::channel();
    let (release_read_tx, release_read_rx) = mpsc::channel();
    runtime.add_io_driver(
        "sleep-on-first-read",
        Box::new(SleepOnReadDriver::new(read_entered_tx, release_read_rx)),
    );

    let runner = ResourceRunner::new(runtime, StdClock::new(), Duration::from_millis(1));
    let mut handle = runner.spawn("hot-reload-cycle-boundary").expect("spawn");
    let control = handle.control();

    read_entered_rx
        .recv_timeout(StdDuration::from_secs(1))
        .expect("runtime should enter the first blocking input read");

    let (tx, rx) = mpsc::channel();
    let queued_at = Instant::now();
    control
        .send_command(ResourceCommand::ReloadBytecode {
            bytes: updated_bytes,
            respond_to: tx,
        })
        .expect("queue reload command");

    let early = rx.recv_timeout(StdDuration::from_millis(40));
    assert!(
        matches!(early, Err(RecvTimeoutError::Timeout)),
        "reload should not complete while cycle is in-flight"
    );
    release_read_tx
        .send(())
        .expect("release first blocking input read");

    let result = rx
        .recv_timeout(StdDuration::from_secs(2))
        .expect("reload response");
    assert!(result.is_ok(), "reload failed: {result:?}");
    assert!(
        queued_at.elapsed() >= StdDuration::from_millis(40),
        "reload should complete only after cycle-boundary handoff"
    );

    handle.stop();
    handle.join().expect("join resource thread");
}

#[test]
fn hot_reload_migrates_retain_and_resets_nonretain_and_instances() {
    let source_v1 = r#"
CONFIGURATION Conf
VAR_GLOBAL RETAIN
    g_ret : INT := 1;
END_VAR
VAR_GLOBAL
    g_plain : INT := 2;
    g_fb : CounterFb;
END_VAR
PROGRAM P1 : Main;
END_CONFIGURATION

FUNCTION_BLOCK CounterFb
VAR
    count : INT := 0;
END_VAR
END_FUNCTION_BLOCK

PROGRAM Main
VAR RETAIN
    p_ret : INT := 3;
END_VAR
VAR
    p_plain : INT := 4;
END_VAR
END_PROGRAM
"#;

    let source_v2 = r#"
CONFIGURATION Conf
VAR_GLOBAL RETAIN
    g_ret : INT := 1;
END_VAR
VAR_GLOBAL
    g_plain : INT := 2;
    g_fb : CounterFb;
END_VAR
PROGRAM P1 : Main;
END_CONFIGURATION

FUNCTION_BLOCK CounterFb
VAR
    count : INT := 0;
END_VAR
END_FUNCTION_BLOCK

PROGRAM Main
VAR RETAIN
    p_ret : INT := 3;
END_VAR
VAR
    p_plain : INT := 4;
END_VAR
END_PROGRAM
"#;

    let mut harness = vm_harness(source_v1);

    harness.set_input("g_ret", Value::Int(30));
    harness.set_input("g_plain", Value::Int(40));
    harness.set_input("p_ret", Value::Int(50));
    harness.set_input("p_plain", Value::Int(60));
    let g_fb_before = match harness.runtime().storage().get_global("g_fb") {
        Some(Value::Instance(id)) => *id,
        other => panic!("expected g_fb instance before reload, got {other:?}"),
    };
    harness
        .runtime_mut()
        .storage_mut()
        .set_instance_var(g_fb_before, "count", Value::Int(7));

    let bytes_v2 = bytecode_bytes_from_source(source_v2).expect("build bytecode v2");
    harness
        .runtime_mut()
        .apply_online_change_bytes(&bytes_v2, None)
        .expect("apply online change");

    assert_numeric_output(&harness, "g_ret", 30);
    assert_numeric_output(&harness, "g_plain", 2);
    assert_numeric_output(&harness, "p_ret", 50);
    assert_numeric_output(&harness, "p_plain", 4);
    let g_fb_after = match harness.runtime().storage().get_global("g_fb") {
        Some(Value::Instance(id)) => *id,
        other => panic!("expected g_fb instance after reload, got {other:?}"),
    };
    let count_after = harness
        .runtime()
        .storage()
        .get_instance_var(g_fb_after, "count")
        .cloned();
    assert!(
        matches!(
            count_after,
            Some(Value::Int(0)) | Some(Value::DInt(0)) | Some(Value::LInt(0))
        ),
        "expected recreated instance count to reset to 0, got {count_after:?}"
    );
}

#[test]
fn hot_reload_rebinds_instance_backed_io_after_warm_restart() {
    let source_v1 = r#"
PROGRAM Main
VAR
    q AT %QX0.0 : BOOL;
END_VAR
q := FALSE;
END_PROGRAM
"#;

    let source_v2 = r#"
PROGRAM Main
VAR
    q AT %QX0.0 : BOOL;
END_VAR
q := TRUE;
END_PROGRAM
"#;

    let session_v1 = CompileSession::from_source(source_v1);
    let session_v2 = CompileSession::from_source(source_v2);
    let mut runtime = session_v1.build_runtime().expect("build runtime");
    let bytes_v1 = session_v1
        .build_bytecode_bytes()
        .expect("build bytecode v1");
    let bytes_v2 = session_v2
        .build_bytecode_bytes()
        .expect("build bytecode v2");

    runtime
        .apply_bytecode_bytes(&bytes_v1, None)
        .expect("apply bytecode v1");
    runtime
        .set_execution_backend(ExecutionBackend::BytecodeVm)
        .expect("select vm backend");
    runtime
        .restart(trust_runtime::RestartMode::Cold)
        .expect("restart runtime");
    runtime.execute_cycle().expect("execute initial cycle");

    let before = runtime.io().snapshot();
    assert_eq!(before.outputs.len(), 1, "expected one output binding");
    assert_eq!(
        before.outputs[0].address.area,
        trust_runtime::memory::IoArea::Output
    );
    assert_eq!(before.outputs[0].address.byte, 0);
    assert_eq!(before.outputs[0].address.bit, 0);
    assert!(matches!(
        before.outputs[0].value,
        trust_runtime::io::IoSnapshotValue::Value(Value::Bool(false))
    ));

    runtime
        .apply_online_change_bytes(&bytes_v2, None)
        .expect("apply online change");
    runtime.execute_cycle().expect("execute post-reload cycle");

    let after = runtime.io().snapshot();
    assert_eq!(
        after.outputs.len(),
        1,
        "expected one output binding after reload"
    );
    assert!(matches!(
        after.outputs[0].value,
        trust_runtime::io::IoSnapshotValue::Value(Value::Bool(true))
    ));
}

#[test]
fn hot_reload_changed_body_restarts_at_entrypoint_policy() {
    let source_v1 = r#"
PROGRAM Main
VAR
    count : INT := 0;
END_VAR
count := count + 1;
END_PROGRAM
"#;

    let source_v2 = r#"
PROGRAM Main
VAR
    count : INT := 0;
END_VAR
count := count + 10;
END_PROGRAM
"#;

    let mut harness = vm_harness(source_v1);
    harness.cycle();
    harness.cycle();
    harness.cycle();
    assert_numeric_output(&harness, "count", 3);
    assert_eq!(harness.runtime().cycle_counter(), 3);

    let bytes_v2 = bytecode_bytes_from_source(source_v2).expect("build bytecode v2");
    harness
        .runtime_mut()
        .apply_online_change_bytes(&bytes_v2, None)
        .expect("apply online change");

    assert_eq!(harness.runtime().cycle_counter(), 0);
    assert_numeric_output(&harness, "count", 0);

    let cycle = harness.cycle();
    assert!(
        cycle.errors.is_empty(),
        "unexpected errors after reload: {:?}",
        cycle.errors
    );
    assert_numeric_output(&harness, "count", 10);
}

#[test]
fn hot_reload_invalid_module_reports_deterministic_error() {
    let source = r#"
PROGRAM Main
VAR
    count : INT := 0;
END_VAR
count := count + 1;
END_PROGRAM
"#;

    let runtime = vm_harness(source).into_runtime();
    let runner = ResourceRunner::new(runtime, StdClock::new(), Duration::from_millis(1));
    let mut handle = runner.spawn("hot-reload-invalid-bytecode").expect("spawn");
    let control = handle.control();
    let (tx, rx) = mpsc::channel();

    control
        .send_command(ResourceCommand::ReloadBytecode {
            bytes: vec![0x00, 0x01, 0x02, 0x03],
            respond_to: tx,
        })
        .expect("send reload command");

    let result = rx
        .recv_timeout(StdDuration::from_secs(2))
        .expect("reload response");
    let err = result.expect_err("reload should fail");
    assert!(
        matches!(err, RuntimeError::InvalidBytecode(_)),
        "expected InvalidBytecode, got {err:?}"
    );

    handle.stop();
    handle.join().expect("join resource thread");
}
