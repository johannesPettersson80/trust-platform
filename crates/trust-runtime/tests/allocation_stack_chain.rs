//! Spec 34 MEM-02, MEM-03 and section 6.2.2 (B-R4): the maximum admitted call depth of distinct POUs
//! completes on a 512 KiB native stack, and one frame past that depth refuses with
//! `CallStackOverflow` without overflowing the stack. Construction and scans both run on the small
//! stack. The compiler runs on a separate 64 MiB authoring thread, because it is not the controller.
//!
//! Specification 34 documents no host thread stack minimum, so 512 KiB is the explicit small stack.
//! Passing this test is not a whole-profile worst-case native-stack proof (section 6.2.2).
//! Stack overflow aborts the whole test binary, so this chain lives in its own binary.
#![cfg(feature = "full")]

use trust_runtime::bytecode::BytecodeVersion;
use trust_runtime::harness::CompileSession;
use trust_runtime::value::{Duration, Value};
use trust_runtime::Runtime;
use trust_runtime_core::vm::{PreparationLimits, VmTrap};

/// Native stack of the thread that constructs and runs the controller.
const SMALL_STACK_BYTES: usize = 512 * 1024;

/// Wraps the application in one resource with a 10 ms periodic task, and compiles it on an
/// authoring thread with a generous stack. Compiler recursion is outside the controller's native
/// stack budget, so the controller only ever receives bytes.
fn build(body: &str) -> Vec<u8> {
    const AUTHORING_STACK_BYTES: usize = 64 * 1024 * 1024;
    let source = format!(
        "{body}\nCONFIGURATION Conf RESOURCE Controller ON PLC TASK Tick (INTERVAL := T#10ms, PRIORITY := 0); PROGRAM Plant WITH Tick : Main; END_RESOURCE END_CONFIGURATION"
    );
    std::thread::Builder::new()
        .stack_size(AUTHORING_STACK_BYTES)
        .spawn(move || {
            CompileSession::from_source(source)
                .build_bytecode_module_for_version(BytecodeVersion::SOURCE_FREE)
                .expect("accepted ST source")
                .encode()
                .expect("encode STBC")
        })
        .expect("spawn the authoring thread")
        .join()
        .expect("the authoring thread returns without panicking")
}

/// `Main` calls `Nested_0`, which calls `Nested_1`, through `functions` distinct POUs. The last POU
/// returns 1, so `Nested_0` returns `functions`, and `Main` stores `2 * functions - 1` in `answer`
/// through a typed expression four operator levels deep.
fn chain_source(functions: usize) -> String {
    let mut source = String::from("VAR_GLOBAL answer : DINT := DINT#0; END_VAR\n");
    for index in 0..functions {
        source.push_str(&format!("FUNCTION Nested_{index} : DINT\n"));
        if index + 1 == functions {
            source.push_str(&format!("Nested_{index} := DINT#1;\n"));
        } else {
            source.push_str(&format!(
                "Nested_{index} := DINT#1 + Nested_{}();\n",
                index + 1
            ));
        }
        source.push_str("END_FUNCTION\n");
    }
    source.push_str(
        "PROGRAM Main\nanswer := ((((Nested_0() + DINT#1) * DINT#2) - DINT#3) / DINT#1);\nEND_PROGRAM\n",
    );
    source
}

/// Runs `work` on a thread with an explicit native stack of `SMALL_STACK_BYTES`.
fn on_small_stack<T: Send + 'static>(work: impl FnOnce() -> T + Send + 'static) -> T {
    std::thread::Builder::new()
        .name("small-stack-controller".to_owned())
        .stack_size(SMALL_STACK_BYTES)
        .spawn(work)
        .expect("spawn the small-stack thread")
        .join()
        .expect("the small-stack thread returns without panicking")
}

/// Admits the bytes with `max_call_depth` on the small stack and reports whether the first scan
/// refuses with `CallStackOverflow` and leaves the controller faulted.
fn refused_on_small_stack(bytes: Vec<u8>, max_call_depth: usize) -> bool {
    on_small_stack(move || {
        let limits = PreparationLimits {
            max_call_depth,
            ..PreparationLimits::default()
        };
        let mut runtime = Runtime::from_bytes(&bytes, limits).expect("admit the chain");
        runtime.advance_time(Duration::from_millis(10));
        let refused =
            runtime.execute_cycle().err() == Some(VmTrap::CallStackOverflow.into_runtime_error());
        refused && runtime.faulted()
    })
}

/// Spec 34 MEM-02 and section 6.2.2: the deepest chain the default profile admits, `max_call_depth`
/// frames (the program plus `max_call_depth - 1` distinct POUs), runs to completion on the small
/// stack, from construction through the scan, and yields the exact expression result.
#[test]
fn maximum_admitted_depth_of_distinct_pous_completes_on_a_small_stack() {
    let depth = PreparationLimits::default().max_call_depth;
    let functions = depth - 1;
    let bytes = build(&chain_source(functions));
    let answer = on_small_stack(move || {
        let mut runtime =
            Runtime::from_bytes(&bytes, PreparationLimits::default()).expect("admit the chain");
        runtime.advance_time(Duration::from_millis(10));
        runtime
            .execute_cycle()
            .expect("the admitted chain completes");
        assert!(!runtime.faulted(), "the admitted chain must not fault");
        let Some(Value::DInt(answer)) = runtime.storage().get_global("answer") else {
            panic!("answer global");
        };
        *answer
    });
    let expected = 2 * i32::try_from(functions).expect("the chain length fits DINT") - 1;
    assert_eq!(answer, expected);
}

/// Spec 34 MEM-02: one frame past the admitted depth refuses with `CallStackOverflow` on the same
/// small stack, and the controller is faulted rather than overflowed.
#[test]
fn one_frame_past_the_admitted_depth_refuses_on_the_small_stack() {
    let depth = PreparationLimits::default().max_call_depth;
    let bytes = build(&chain_source(depth - 1));
    assert!(
        refused_on_small_stack(bytes, depth - 1),
        "a chain one frame deeper than the admitted depth must refuse with CallStackOverflow"
    );
}
