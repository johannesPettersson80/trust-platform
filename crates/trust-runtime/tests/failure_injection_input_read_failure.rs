//! Brief item 8 (audit A10.26): a read-side input failure leaves every bound
//! PLC input value unchanged. Spec 11, "Process-image interface and binding
//! transaction" (docs/specs/11-runtime-engine.md lines 451-456): a read failure
//! "leaves all bound program values unchanged". The first driver writes its bytes into the
//! shared input image before the second driver fails, so this also proves that
//! those raw bytes never reach program storage and that no output is committed.
//!
//! The tests need the toolchain of specification 44 §4.6 at TRUST_TOOLCHAIN.

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use trust_runtime::error::RuntimeError;
use trust_runtime::harness::TestHarness;
use trust_runtime::io::{IoAddress, IoDriver};
use trust_runtime::value::Value;
use trust_runtime::watchdog::FaultPolicy;
use trust_runtime::CompiledRuntime;

/// Samples the first `%IX` bit from a shared flag and logs every output image.
#[derive(Debug)]
struct FirstInputDriver {
    value: Arc<AtomicBool>,
    reads: Arc<AtomicUsize>,
    output_images: Arc<Mutex<Vec<Vec<u8>>>>,
}

impl IoDriver for FirstInputDriver {
    fn read_inputs(&mut self, inputs: &mut [u8]) -> Result<(), RuntimeError> {
        self.reads.fetch_add(1, Ordering::SeqCst);
        inputs[0] = u8::from(self.value.load(Ordering::SeqCst));
        Ok(())
    }

    fn write_outputs(&mut self, outputs: &[u8]) -> Result<(), RuntimeError> {
        self.output_images
            .lock()
            .expect("output log lock")
            .push(outputs.to_vec());
        Ok(())
    }
}

/// Sets the second `%IX` bit, or fails its read while `fail` is set.
#[derive(Debug)]
struct SecondInputDriver {
    fail: Arc<AtomicBool>,
    reads: Arc<AtomicUsize>,
}

impl IoDriver for SecondInputDriver {
    fn read_inputs(&mut self, inputs: &mut [u8]) -> Result<(), RuntimeError> {
        self.reads.fetch_add(1, Ordering::SeqCst);
        if self.fail.load(Ordering::SeqCst) {
            return Err(RuntimeError::IoTransport(
                "second input transport failure".into(),
            ));
        }
        inputs[1] = 1;
        Ok(())
    }

    fn write_outputs(&mut self, _outputs: &[u8]) -> Result<(), RuntimeError> {
        Ok(())
    }
}

/// Two bound inputs (`%IX0.0`, `%IX1.0`) and one bound output (`%QX0.0`).
fn bound_runtime() -> CompiledRuntime {
    TestHarness::from_source(
        r#"
VAR_GLOBAL
    first AT %IX0.0 : BOOL;
    second AT %IX1.0 : BOOL;
    led AT %QX0.0 : BOOL;
END_VAR

PROGRAM Main
END_PROGRAM
"#,
    )
    .expect("the bound program compiles")
    .into_runtime()
}

/// The output image's byte.
fn output_byte(runtime: &CompiledRuntime) -> Result<Value, RuntimeError> {
    runtime.read_io(&IoAddress::parse("%QB0").expect("output byte address"))
}

#[test]
fn failed_second_input_read_keeps_every_bound_input_and_commits_no_output() {
    let first_value = Arc::new(AtomicBool::new(true));
    let first_reads = Arc::new(AtomicUsize::new(0));
    let output_images: Arc<Mutex<Vec<Vec<u8>>>> = Arc::new(Mutex::new(Vec::new()));
    let second_fail = Arc::new(AtomicBool::new(false));
    let second_reads = Arc::new(AtomicUsize::new(0));

    let mut runtime = bound_runtime();
    // FaultPolicy::Halt applies no safe-state write, so the only output
    // activity in the failed cycle could come from the cycle itself.
    runtime.set_fault_policy(FaultPolicy::Halt);
    runtime.add_io_driver(
        "first-input",
        Box::new(FirstInputDriver {
            value: Arc::clone(&first_value),
            reads: Arc::clone(&first_reads),
            output_images: Arc::clone(&output_images),
        }),
    );
    runtime.add_io_driver(
        "second-input",
        Box::new(SecondInputDriver {
            fail: Arc::clone(&second_fail),
            reads: Arc::clone(&second_reads),
        }),
    );
    runtime
        .write_variable("led", &Value::Bool(true))
        .expect("checked output source");

    // Cycle 1: both reads succeed, so every bound value and the output commit.
    runtime
        .execute_cycle()
        .expect("healthy cycle samples both inputs");
    assert_eq!(runtime.read_variable("first"), Ok(Value::Bool(true)));
    assert_eq!(runtime.read_variable("second"), Ok(Value::Bool(true)));
    assert_eq!(output_byte(&runtime), Ok(Value::Byte(1)));
    assert_eq!(
        output_images.lock().expect("output log lock").as_slice(),
        &[vec![1u8]]
    );

    // Cycle 2: the first driver now reads FALSE into the raw image, the second
    // driver fails, and the output source changes to FALSE. Nothing may commit.
    first_value.store(false, Ordering::SeqCst);
    second_fail.store(true, Ordering::SeqCst);
    runtime
        .write_variable("led", &Value::Bool(false))
        .expect("checked output source");
    let error = runtime
        .execute_cycle()
        .expect_err("the failed input read must fail the cycle");
    assert_eq!(
        error,
        RuntimeError::IoTransport("second input transport failure".into())
    );

    assert_eq!(
        runtime.read_variable("first"),
        Ok(Value::Bool(true)),
        "bound first input keeps the previous cycle value"
    );
    assert_eq!(
        runtime.read_variable("second"),
        Ok(Value::Bool(true)),
        "bound second input keeps the previous cycle value"
    );
    assert_eq!(
        output_byte(&runtime),
        Ok(Value::Byte(1)),
        "no output image is committed by the failed cycle"
    );
    assert_eq!(
        output_images.lock().expect("output log lock").len(),
        1,
        "no driver output write happens in the failed cycle"
    );
    assert_eq!(first_reads.load(Ordering::SeqCst), 2);
    assert_eq!(second_reads.load(Ordering::SeqCst), 2);
    assert!(runtime.faulted());
    assert_eq!(
        runtime.last_fault(),
        Some(RuntimeError::IoTransport(
            "second input transport failure".into()
        ))
    );
}
