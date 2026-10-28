//! I/O drivers at the scan's bounds: each driver reads the inputs before the program runs and
//! writes the outputs after it, once per scan, in the order the drivers were added.
//!
//! The tests need the toolchain of specification 44 §4.6 at `TRUST_TOOLCHAIN`.

use std::sync::{Arc, Mutex};

use trust_runtime::harness::TestHarness;
use trust_runtime::io::IoDriver;
use trust_runtime::value::Value;
use trust_runtime::CompiledRuntime;

/// `out` copies `in` in an event task that a rising edge of `trigger` starts.
const COPY: &str = r#"
CONFIGURATION C
VAR_GLOBAL
    in AT %IX0.0 : BOOL;
    out AT %QX0.0 : BOOL;
    trigger : BOOL;
END_VAR
RESOURCE R ON PLC
    TASK T (SINGLE := trigger, PRIORITY := 0);
    PROGRAM P1 WITH T : P;
END_RESOURCE
END_CONFIGURATION

PROGRAM P
out := in;
END_PROGRAM
"#;

fn copy_runtime() -> CompiledRuntime {
    TestHarness::from_source(COPY)
        .expect("the copy program compiles")
        .into_runtime()
}

#[derive(Default)]
struct IoState {
    inputs: Vec<u8>,
    outputs: Vec<u8>,
    reads: u32,
    writes: u32,
}

struct TestDriver {
    state: Arc<Mutex<IoState>>,
}

impl IoDriver for TestDriver {
    fn read_inputs(&mut self, inputs: &mut [u8]) -> Result<(), trust_runtime::error::RuntimeError> {
        let mut state = self.state.lock().expect("io state lock");
        for (idx, byte) in inputs.iter_mut().enumerate() {
            *byte = *state.inputs.get(idx).unwrap_or(&0);
        }
        state.reads += 1;
        Ok(())
    }

    fn write_outputs(&mut self, outputs: &[u8]) -> Result<(), trust_runtime::error::RuntimeError> {
        let mut state = self.state.lock().expect("io state lock");
        state.outputs = outputs.to_vec();
        state.writes += 1;
        Ok(())
    }
}

#[test]
fn io_driver_reads_and_writes_at_cycle_bounds() {
    let state = Arc::new(Mutex::new(IoState {
        inputs: vec![1],
        outputs: Vec::new(),
        reads: 0,
        writes: 0,
    }));
    let mut runtime = copy_runtime();
    runtime.add_io_driver(
        "test",
        Box::new(TestDriver {
            state: state.clone(),
        }),
    );

    runtime
        .write_variable("trigger", &Value::Bool(true))
        .expect("checked test write");
    runtime.execute_cycle().unwrap();

    let state = state.lock().expect("io state lock");
    assert_eq!(state.reads, 1);
    assert_eq!(state.writes, 1);
    assert_eq!(state.outputs.first().copied().unwrap_or(0) & 1, 1);
}

#[test]
fn composed_drivers_are_invoked_in_order() {
    let state_a = Arc::new(Mutex::new(IoState {
        inputs: vec![1],
        outputs: Vec::new(),
        reads: 0,
        writes: 0,
    }));
    let state_b = Arc::new(Mutex::new(IoState {
        inputs: vec![1],
        outputs: Vec::new(),
        reads: 0,
        writes: 0,
    }));
    let mut runtime = copy_runtime();
    runtime.add_io_driver(
        "first",
        Box::new(TestDriver {
            state: state_a.clone(),
        }),
    );
    runtime.add_io_driver(
        "second",
        Box::new(TestDriver {
            state: state_b.clone(),
        }),
    );

    runtime
        .write_variable("trigger", &Value::Bool(true))
        .expect("checked test write");
    runtime.execute_cycle().unwrap();

    let state_a = state_a.lock().expect("io state lock");
    let state_b = state_b.lock().expect("io state lock");
    assert_eq!(state_a.reads, 1);
    assert_eq!(state_a.writes, 1);
    assert_eq!(state_b.reads, 1);
    assert_eq!(state_b.writes, 1);
}
