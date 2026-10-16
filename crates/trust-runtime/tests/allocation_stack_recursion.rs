//! Spec 34 MEM-02, MEM-03, MEM-08, HOST-04 and section 6.2.2 (B-R4): recursion of one occupied POU
//! (the shared stack path) at the maximum admitted call depth completes on a 512 KiB native stack,
//! one frame past that depth refuses with `CallStackOverflow`, and the deepest typed expression the
//! generator emits completes on the same stack. Construction and scans run on the small stack; the
//! compiler runs on a separate 64 MiB authoring thread, because it is not the controller.
//!
//! Specification 34 documents no host thread stack minimum, so 512 KiB is the explicit small stack.
//! Passing this test is not a whole-profile worst-case native-stack proof (section 6.2.2).
//! Stack overflow aborts the whole test binary, so this recursion lives in its own binary.
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

/// `Demo.Recur(level)` returns `level + 1`. Each level above zero re-enters the occupied POU.
fn recursion_source(level: i32) -> String {
    format!(
        r#"VAR_GLOBAL answer : DINT := DINT#0; END_VAR
NAMESPACE Demo
FUNCTION Recur : DINT
VAR_INPUT level : DINT; END_VAR
IF level = DINT#0 THEN
    Recur := DINT#1;
ELSE
    Recur := Demo.Recur(level := level - DINT#1) + DINT#1;
END_IF;
END_FUNCTION
END_NAMESPACE
PROGRAM Main
answer := Demo.Recur(level := DINT#{level});
END_PROGRAM
"#
    )
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
        let mut runtime = Runtime::from_bytes(&bytes, limits).expect("admit the recursion");
        runtime.advance_time(Duration::from_millis(10));
        let refused =
            runtime.execute_cycle().err() == Some(VmTrap::CallStackOverflow.into_runtime_error());
        refused && runtime.faulted()
    })
}

/// Spec 34 MEM-08 and HOST-04: the deepest recursion the default profile admits, `max_call_depth`
/// frames (the program plus `level + 1` activations of the occupied `Recur`), completes on the small
/// stack and returns the exact result.
#[test]
fn maximum_admitted_recursion_depth_completes_on_a_small_stack() {
    let depth = PreparationLimits::default().max_call_depth;
    let level = i32::try_from(depth - 2).expect("the admitted depth fits DINT");
    let bytes = build(&recursion_source(level));
    let answer = on_small_stack(move || {
        let mut runtime =
            Runtime::from_bytes(&bytes, PreparationLimits::default()).expect("admit the recursion");
        runtime.advance_time(Duration::from_millis(10));
        runtime
            .execute_cycle()
            .expect("the admitted recursion completes");
        assert!(!runtime.faulted(), "the admitted recursion must not fault");
        let Some(Value::DInt(answer)) = runtime.storage().get_global("answer") else {
            panic!("answer global");
        };
        *answer
    });
    assert_eq!(answer, level + 1);
}

/// Spec 34 MEM-02: one frame past the admitted recursion depth refuses with `CallStackOverflow` on
/// the same small stack, and the controller is faulted rather than overflowed.
#[test]
fn one_frame_past_the_admitted_recursion_depth_refuses_on_the_small_stack() {
    let depth = PreparationLimits::default().max_call_depth;
    let level = i32::try_from(depth - 2).expect("the admitted depth fits DINT");
    let bytes = build(&recursion_source(level));
    assert!(
        refused_on_small_stack(bytes, depth - 1),
        "a recursion one frame deeper than the admitted depth must refuse with CallStackOverflow"
    );
}

/// Spec 34 HOST-04 and MEM-03: the deepest typed expression the generator emits (operator depth
/// four, where a literal or variable is depth zero, as in `st_program_gen` MAX_EXPR_DEPTH) evaluates
/// on the small stack, from construction through the scan.
#[test]
fn deepest_typed_expression_completes_on_a_small_stack() {
    let bytes = build(
        "VAR_GLOBAL seed : DINT := DINT#7; answer : DINT := DINT#0; END_VAR\n\
         PROGRAM Main\n\
         answer := ((((seed + DINT#1) * DINT#2) - DINT#3) / DINT#1);\n\
         END_PROGRAM",
    );
    let answer = on_small_stack(move || {
        let mut runtime = Runtime::from_bytes(&bytes, PreparationLimits::default())
            .expect("admit the expression");
        runtime.advance_time(Duration::from_millis(10));
        runtime
            .execute_cycle()
            .expect("the deepest expression evaluates");
        assert!(!runtime.faulted(), "the deepest expression must not fault");
        let Some(Value::DInt(answer)) = runtime.storage().get_global("answer") else {
            panic!("answer global");
        };
        *answer
    });
    // ((7 + 1) * 2 - 3) / 1
    assert_eq!(answer, 13);
}
