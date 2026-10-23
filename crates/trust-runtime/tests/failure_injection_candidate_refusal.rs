//! Failure injection: a candidate refused at each reachable preparation or installation step
//! leaves the live controller unchanged.
//!
//! Spec 10 section 4.7 and 4.8 and spec 34 UPDATE-01 and LOAD-03/06/09 (section 4.3.1 HOST-09):
//! a refused candidate keeps the live forces, queued debug writes, published outputs, cycle
//! counter and prepared module identity, and the live controller still runs afterwards.
//!
//! Steps with no integration seam (native import resolution, retained migration on a live
//! store, host binding preparation, construction work limit) are not covered here.
#[path = "common/bytecode_helpers.rs"]
mod bytecode_helpers;

use trust_runtime::bytecode::{BytecodeModule, BytecodeVersion};
use trust_runtime::debug::{DebugControl, ForcedVarTarget};
use trust_runtime::error::{RuntimeError, StableErrorCode};
use trust_runtime::execution_backend::ExecutionBackend;
use trust_runtime::harness::{bytecode_bytes_from_source, CompileSession, TestHarness};
use trust_runtime::retain::RetainStore;
use trust_runtime::runtime_core::vm::{PreparationLimits, PreparedModule};
use trust_runtime::value::Value;
use trust_runtime::{RestartMode, RetainSnapshot, Runtime};

const LIVE_SOURCE: &str = r#"
VAR_GLOBAL count : INT := INT#0; hold : INT := INT#0; END_VAR
PROGRAM Main
VAR published AT %QW0 : INT; END_VAR
count := count + INT#1;
published := count;
END_PROGRAM
"#;

const CANDIDATE_SOURCE: &str = r#"
VAR_GLOBAL count : INT := INT#0; hold : INT := INT#0; END_VAR
PROGRAM Main
VAR published AT %QW0 : INT; END_VAR
count := count + INT#10;
published := count;
END_PROGRAM
"#;

/// Same as the candidate with extra declarations, so its artifact is strictly larger.
const LARGER_CANDIDATE_SOURCE: &str = r#"
VAR_GLOBAL count : INT := INT#0; hold : INT := INT#0; spare : DINT := DINT#5; END_VAR
PROGRAM Main
VAR published AT %QW0 : INT; extra : DINT := DINT#9; END_VAR
count := count + INT#10;
published := count;
END_PROGRAM
"#;

struct FailingLoadStore;

impl RetainStore for FailingLoadStore {
    fn load(&self) -> Result<RetainSnapshot, RuntimeError> {
        Err(RuntimeError::RetainStore(
            "injected retain load failure".into(),
        ))
    }

    fn store(&self, _snapshot: &RetainSnapshot) -> Result<(), RuntimeError> {
        Ok(())
    }
}

/// A live VM controller after one cycle, with count forced to 99 and a queued write to hold.
struct Live {
    harness: TestHarness,
    debug: DebugControl,
}

fn live() -> Live {
    let mut harness = TestHarness::from_source(LIVE_SOURCE).expect("compile live source");
    let bytes = bytecode_bytes_from_source(LIVE_SOURCE).expect("build live bytecode");
    harness
        .runtime_mut()
        .apply_bytecode_bytes(&bytes, None)
        .expect("apply live bytecode");
    harness
        .runtime_mut()
        .set_execution_backend(ExecutionBackend::BytecodeVm)
        .expect("select vm backend");
    harness
        .runtime_mut()
        .restart(RestartMode::Cold)
        .expect("cold restart live runtime");
    let first = harness.cycle();
    assert!(
        first.errors.is_empty(),
        "live cycle failed: {:?}",
        first.errors
    );
    let debug = harness.runtime_mut().enable_debug();
    debug.force_global("count", Value::Int(99));
    debug.enqueue_global_write("hold", Value::Int(7));
    Live { harness, debug }
}

/// Everything a refused candidate must leave untouched.
struct Observed {
    forced_count: Option<Value>,
    outputs: Vec<u8>,
    cycle_counter: u64,
    prepared: *const PreparedModule,
}

fn observe(runtime: &Runtime, debug: &DebugControl) -> Observed {
    Observed {
        forced_count: debug.forced_var_value(&ForcedVarTarget::Global("count".into())),
        outputs: runtime.io().outputs().to_vec(),
        cycle_counter: runtime.cycle_counter(),
        prepared: runtime.prepared() as *const PreparedModule,
    }
}

fn assert_unchanged(before: &Observed, after: &Observed, label: &str) {
    assert_eq!(
        after.forced_count, before.forced_count,
        "{label}: the active force changed"
    );
    assert_eq!(
        after.outputs, before.outputs,
        "{label}: the published output image changed"
    );
    assert_eq!(
        after.cycle_counter, before.cycle_counter,
        "{label}: the cycle counter changed"
    );
    assert!(
        std::ptr::eq(after.prepared, before.prepared),
        "{label}: the prepared module identity changed"
    );
}

/// Run one live cycle after a refusal: the queued write must apply and the force must hold.
fn assert_live_runtime_still_runs(live: &mut Live, label: &str) {
    assert_eq!(
        live.harness.runtime().io().outputs().to_vec(),
        vec![1u8, 0u8],
        "{label}: the published image must still be cycle 1"
    );
    let cycle = live.harness.cycle();
    assert!(
        cycle.errors.is_empty(),
        "{label}: the live runtime must still run: {:?}",
        cycle.errors
    );
    assert_eq!(
        live.harness.runtime().storage().get_global("hold"),
        Some(&Value::Int(7)),
        "{label}: the queued write must survive the refused candidate"
    );
    assert_eq!(
        live.debug
            .forced_var_value(&ForcedVarTarget::Global("count".into())),
        Some(Value::Int(99)),
        "{label}: the force must survive the refused candidate"
    );
}

fn refused_bytecode(live: &mut Live, bytes: &[u8]) -> RuntimeError {
    match live.harness.runtime_mut().apply_bytecode_bytes(bytes, None) {
        Ok(()) => panic!("the candidate must be refused"),
        Err(error) => error,
    }
}

fn candidate_bytes() -> Vec<u8> {
    bytecode_bytes_from_source(CANDIDATE_SOURCE).expect("build candidate bytecode")
}

fn source_free_bytes(source: &str) -> Vec<u8> {
    CompileSession::from_source(source)
        .build_bytecode_module_for_version(BytecodeVersion::SOURCE_FREE)
        .expect("accepted source")
        .encode()
        .unwrap()
}

#[test]
fn retain_load_refusal_of_an_online_change_keeps_the_live_runtime() {
    let mut live = live();
    let before = observe(live.harness.runtime(), &live.debug);
    assert_eq!(before.outputs, vec![1u8, 0u8]);
    live.harness
        .runtime_mut()
        .set_retain_store(Some(Box::new(FailingLoadStore)), None);
    let bytes = candidate_bytes();
    let error = match live
        .harness
        .runtime_mut()
        .apply_online_change_bytes(&bytes, None)
    {
        Ok(_) => panic!("a candidate behind a failing retain load must be refused"),
        Err(error) => error,
    };
    assert!(
        matches!(error, RuntimeError::RetainStore(_)),
        "the retain load is the first step of an online change: {error:?}"
    );
    assert!(
        error.to_string().contains("injected retain load failure"),
        "the injected store failure must reach the caller: {error}"
    );
    assert_unchanged(
        &before,
        &observe(live.harness.runtime(), &live.debug),
        "retain load refusal",
    );
    assert_live_runtime_still_runs(&mut live, "retain load refusal");
}

#[test]
fn bad_magic_candidate_is_refused_at_decode_with_the_live_state_unchanged() {
    let mut live = live();
    let before = observe(live.harness.runtime(), &live.debug);
    let mut bytes = candidate_bytes();
    bytes[0] = 0x00;
    let error = refused_bytecode(&mut live, &bytes);
    assert_eq!(error.stable_code(), StableErrorCode::BytecodeInvalidMagic);
    assert_unchanged(
        &before,
        &observe(live.harness.runtime(), &live.debug),
        "bad magic",
    );
    assert_live_runtime_still_runs(&mut live, "bad magic");
}

#[test]
fn truncated_candidate_is_refused_with_exactly_the_decoder_error() {
    let mut live = live();
    let before = observe(live.harness.runtime(), &live.debug);
    let bytes = candidate_bytes();
    let truncated = &bytes[..bytes.len() / 2];
    let Err(decode_error) = BytecodeModule::decode(truncated) else {
        panic!("a truncated candidate must not decode");
    };
    let error = refused_bytecode(&mut live, truncated);
    assert_eq!(
        error,
        RuntimeError::from(decode_error),
        "a truncated candidate must fail with the decoder's own refusal"
    );
    assert_unchanged(
        &before,
        &observe(live.harness.runtime(), &live.debug),
        "truncated candidate",
    );
    assert_live_runtime_still_runs(&mut live, "truncated candidate");
}

#[test]
fn context_less_1_1_candidate_is_refused_as_a_profile_mismatch() {
    let mut live = live();
    let before = observe(live.harness.runtime(), &live.debug);
    let bytes = bytecode_helpers::base_module()
        .encode()
        .expect("encode 1.1 artifact");
    let error = refused_bytecode(&mut live, &bytes);
    assert_eq!(
        error.stable_code(),
        StableErrorCode::VmBytecodeDecode,
        "a context-less 1.1 artifact is not executable under the installed profile"
    );
    assert_unchanged(
        &before,
        &observe(live.harness.runtime(), &live.debug),
        "profile mismatch",
    );
    assert_live_runtime_still_runs(&mut live, "profile mismatch");
}

#[test]
fn candidate_over_the_admitted_artifact_limit_is_refused_at_preparation() {
    let live_bytes = source_free_bytes(LIVE_SOURCE);
    let candidate = source_free_bytes(LARGER_CANDIDATE_SOURCE);
    let limits = PreparationLimits {
        max_artifact_bytes: live_bytes.len() + 1,
        ..Default::default()
    };
    assert!(
        candidate.len() > limits.max_artifact_bytes,
        "the candidate must exceed the live admission limit"
    );
    let mut runtime = match Runtime::from_bytes(&live_bytes, limits) {
        Ok(runtime) => runtime,
        Err(error) => panic!("the live controller must be admitted: {error:?}"),
    };
    let debug = runtime.enable_debug();
    runtime.execute_cycle().expect("live cycle");
    debug.force_global("count", Value::Int(99));
    debug.enqueue_global_write("hold", Value::Int(7));
    let before = observe(&runtime, &debug);
    let error = match runtime.apply_bytecode_bytes(&candidate, None) {
        Ok(()) => panic!("an oversized candidate must be refused at preparation"),
        Err(error) => error,
    };
    assert_eq!(error, RuntimeError::PreparationLimit);
    assert_unchanged(&before, &observe(&runtime, &debug), "admission limit");
    runtime.execute_cycle().expect("live cycle after refusal");
    assert_eq!(
        runtime.storage().get_global("hold"),
        Some(&Value::Int(7)),
        "the queued write must survive the refused candidate"
    );
    assert_eq!(
        debug.forced_var_value(&ForcedVarTarget::Global("count".into())),
        Some(Value::Int(99)),
        "the force must survive the refused candidate"
    );
}
