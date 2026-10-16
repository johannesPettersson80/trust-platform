//! Stateful lifecycle sequences (scope H assurance, checklist A08).
//!
//! Each property runs a generated sequence of operations against one installed controller and against
//! the explicit model in `support/lifecycle_model.rs`. The model is written from spec 10 sections 4.7
//! and 4.8, spec 11 sections 6.6, 6.7, 6.7.1, 6.9 and 6.9.2, and spec 34 HOST-09, UPDATE-01, UPDATE-02
//! and UPDATE-07. After every operation the test compares the result and every observable that the
//! model defines. A refused operation must also leave the whole observable state and the installed
//! module unchanged.
//!
//! The model does not generate operations whose outcome the specifications leave open: a scan, a
//! restart, a replacement or a save while the debugger holds the scan; writes, forces and queued writes
//! while faulted; a second queued write to one target before a scan; a scan that fails after it applied
//! queued writes or forces; `clear_fault`; and stale handles across replacement (scope C).
//!
//! The seed is fixed, so a run replays the same sequences. `PROPTEST_CASES` overrides the default of
//! 64 cases. A failing case is saved in the `proptest-regressions` store and replayed first.

#![cfg(feature = "full")]

#[path = "support/lifecycle_model.rs"]
mod lifecycle_model;

use std::sync::{Arc, Mutex, OnceLock};

use lifecycle_model::{Checkpoint, Fault, Model, Op, Outcome, View};
use proptest::collection::vec;
use proptest::prelude::*;
use proptest::strategy::Union;
use proptest::test_runner::{FileFailurePersistence, RngSeed};
use trust_runtime::debug::{DebugControl, ForcedVarTarget};
use trust_runtime::error::RuntimeError;
use trust_runtime::harness::{bytecode_bytes_from_source, TestHarness};
use trust_runtime::retain::RetainStore;
use trust_runtime::value::{Duration, Value};
use trust_runtime::{RestartMode, RetainSnapshot};

/// Seed for the generated sequences ("trustSEQ").
const SEED: u64 = 0x7472_7573_7453_4551;
/// Cases when `PROPTEST_CASES` is unset.
const DEFAULT_CASES: u32 = 64;

/// Program A, installed at the start. `Main.r` and `g_r` are retained. `out_bit` is the process
/// output `%QX0.0`.
const SOURCE_A: &str = r#"
VAR_GLOBAL RETAIN
    g_r : INT := INT#5;
END_VAR
VAR_GLOBAL
    ident : INT := INT#1;
    i : INT := INT#0;
    d : INT(0..10) := INT#1;
    out_bit AT %QX0.0 : BOOL := FALSE;
END_VAR

PROGRAM Main
VAR RETAIN
    r : INT := INT#1;
END_VAR
VAR
    n : INT := INT#2;
    q : INT := INT#0;
END_VAR
q := INT#100 / d;
r := r + INT#1;
g_r := g_r + INT#1;
n := n + i;
out_bit := n >= INT#10;
END_PROGRAM
"#;

/// Program B, a compatible replacement: the same declarations with another `ident` and other arithmetic.
const SOURCE_B: &str = r#"
VAR_GLOBAL RETAIN
    g_r : INT := INT#5;
END_VAR
VAR_GLOBAL
    ident : INT := INT#2;
    i : INT := INT#0;
    d : INT(0..10) := INT#1;
    out_bit AT %QX0.0 : BOOL := FALSE;
END_VAR

PROGRAM Main
VAR RETAIN
    r : INT := INT#1;
END_VAR
VAR
    n : INT := INT#2;
    q : INT := INT#0;
END_VAR
q := INT#100 / d;
r := r + INT#10;
g_r := g_r + INT#2;
n := n + i * INT#2;
out_bit := n >= INT#4;
END_PROGRAM
"#;

/// Program C, an incompatible replacement: the retained `Main.r` changes from INT to BOOL.
const SOURCE_C: &str = r#"
VAR_GLOBAL RETAIN
    g_r : INT := INT#5;
END_VAR
VAR_GLOBAL
    ident : INT := INT#1;
    i : INT := INT#0;
    d : INT(0..10) := INT#1;
    out_bit AT %QX0.0 : BOOL := FALSE;
END_VAR

PROGRAM Main
VAR RETAIN
    r : BOOL := FALSE;
END_VAR
VAR
    n : INT := INT#2;
    q : INT := INT#0;
END_VAR
q := INT#100 / d;
r := NOT r;
g_r := g_r + INT#1;
n := n + i;
out_bit := n >= INT#10;
END_PROGRAM
"#;

/// Bytes that do not decode as bytecode.
const MALFORMED_BYTECODE: [u8; 4] = [0x00, 0x01, 0x02, 0x03];

/// The compiled candidates, built once per test binary.
struct Candidates {
    b: Vec<u8>,
    c: Vec<u8>,
}

fn candidates() -> &'static Candidates {
    static CANDIDATES: OnceLock<Candidates> = OnceLock::new();
    CANDIDATES.get_or_init(|| Candidates {
        b: bytecode_bytes_from_source(SOURCE_B).expect("candidate B compiles"),
        c: bytecode_bytes_from_source(SOURCE_C).expect("candidate C compiles"),
    })
}

/// The durable retain store. The test keeps a second handle to read what was saved.
struct MemoryStore {
    content: Arc<Mutex<RetainSnapshot>>,
}

impl RetainStore for MemoryStore {
    fn load(&self) -> Result<RetainSnapshot, RuntimeError> {
        Ok(self.content.lock().expect("store lock").clone())
    }

    fn store(&self, snapshot: &RetainSnapshot) -> Result<(), RuntimeError> {
        *self.content.lock().expect("store lock") = snapshot.clone();
        Ok(())
    }
}

/// The installed controller under test, its debugger handle and its durable store.
struct Sut {
    harness: TestHarness,
    debug: DebugControl,
    store: Arc<Mutex<RetainSnapshot>>,
}

impl Sut {
    fn new() -> Self {
        let mut harness = TestHarness::from_source(SOURCE_A).expect("program A compiles");
        let runtime = harness.runtime_mut();
        let debug = runtime.enable_debug();
        let store = Arc::new(Mutex::new(RetainSnapshot::default()));
        runtime.set_retain_store(
            Some(Box::new(MemoryStore {
                content: Arc::clone(&store),
            })),
            None,
        );
        Self {
            harness,
            debug,
            store,
        }
    }

    /// Run one operation. The flag tells whether the installed module changed.
    fn apply(&mut self, op: Op) -> (Result<(), RuntimeError>, bool) {
        let before = installed_identity(&self.harness);
        let result = self.run(op);
        let after = installed_identity(&self.harness);
        (result, before != after)
    }

    fn run(&mut self, op: Op) -> Result<(), RuntimeError> {
        let runtime = self.harness.runtime_mut();
        match op {
            Op::Scan => runtime.execute_cycle(),
            Op::WriteI(value) => runtime.write_global("i", Value::Int(value)),
            Op::WriteD(value) => runtime.write_global("d", Value::Int(value)),
            Op::EnqueueI(value) => {
                self.debug.enqueue_global_write("i", Value::Int(value));
                Ok(())
            }
            Op::EnqueueD(value) => {
                self.debug.enqueue_global_write("d", Value::Int(value));
                Ok(())
            }
            Op::ForceI(value) => {
                self.debug.force_global("i", Value::Int(value));
                Ok(())
            }
            Op::ForceD(value) => {
                self.debug.force_global("d", Value::Int(value));
                Ok(())
            }
            Op::ReleaseI => {
                self.debug.release_global("i");
                Ok(())
            }
            Op::ReleaseD => {
                self.debug.release_global("d");
                Ok(())
            }
            Op::Pause => {
                self.debug.pause();
                Ok(())
            }
            Op::Resume => {
                self.debug.continue_run();
                Ok(())
            }
            Op::RestartWarm => runtime.restart(RestartMode::Warm),
            Op::RestartCold => runtime.restart(RestartMode::Cold),
            Op::RestartWarmFromStore => runtime.restart_with_retain_store(RestartMode::Warm),
            Op::SaveStore => runtime.save_retain_store(),
            Op::OnlineChangeB => runtime
                .apply_online_change_bytes(&candidates().b, None)
                .map(|_| ()),
            Op::OnlineChangeMalformed => runtime
                .apply_online_change_bytes(&MALFORMED_BYTECODE, None)
                .map(|_| ()),
            Op::OnlineChangeIncompatible => runtime
                .apply_online_change_bytes(&candidates().c, None)
                .map(|_| ()),
            Op::AdvanceTime(ms) => {
                runtime.advance_time(Duration::from_millis(ms));
                Ok(())
            }
        }
    }

    /// Read every observable that the model defines. Reads do not change the controller.
    fn view(&self) -> Result<View, String> {
        let runtime = self.harness.runtime();
        let out = match self.harness.get_direct_output("%QX0.0") {
            Ok(Value::Bool(value)) => value,
            other => return Err(format!("%QX0.0 is not a BOOL image: {other:?}")),
        };
        let (store_entries, store) = stored(&self.store);
        Ok(View {
            ident: int_output(&self.harness, "ident")?,
            cycles: runtime.cycle_counter(),
            faulted: runtime.faulted(),
            last_fault: runtime.last_fault().map(fault_of),
            held: self.debug.is_paused(),
            time_ms: runtime.current_time().as_millis(),
            r: int_output(&self.harness, "r")?,
            g_r: int_output(&self.harness, "g_r")?,
            n: int_output(&self.harness, "n")?,
            q: int_output(&self.harness, "q")?,
            i: int_output(&self.harness, "i")?,
            d: int_output(&self.harness, "d")?,
            out,
            forced_i: forced_int(&self.debug, "i")?,
            forced_d: forced_int(&self.debug, "d")?,
            store_entries,
            store,
        })
    }
}

/// The address of the installed, admitted module. It changes only when a replacement is installed.
fn installed_identity(harness: &TestHarness) -> usize {
    harness.runtime().prepared() as *const _ as usize
}

fn int_output(harness: &TestHarness, name: &str) -> Result<i16, String> {
    match harness.get_output(name) {
        Some(Value::Int(value)) => Ok(value),
        other => Err(format!("{name} is not an INT: {other:?}")),
    }
}

fn forced_int(debug: &DebugControl, name: &str) -> Result<Option<i16>, String> {
    match debug.forced_var_value(&ForcedVarTarget::Global(name.into())) {
        None => Ok(None),
        Some(Value::Int(value)) => Ok(Some(value)),
        Some(other) => Err(format!("forced {name} is not an INT: {other:?}")),
    }
}

/// The number of store entries and the checkpoint they form. A checkpoint needs both keys.
fn stored(store: &Mutex<RetainSnapshot>) -> (usize, Option<Checkpoint>) {
    let snapshot = store.lock().expect("store lock").clone();
    let values = snapshot.values();
    let checkpoint = match (values.get("g_r"), values.get("@program/Main/r")) {
        (Some(Value::Int(g_r)), Some(Value::Int(r))) => Some(Checkpoint { g_r: *g_r, r: *r }),
        _ => None,
    };
    (values.len(), checkpoint)
}

fn fault_of(error: &RuntimeError) -> Fault {
    match error {
        RuntimeError::DivisionByZero => Fault::DivisionByZero,
        _ => Fault::Other,
    }
}

/// Map a product error to the outcome the specifications name. Any other error maps to
/// `CandidateRejected`, which the model accepts only for a refused candidate.
fn outcome_of(error: &RuntimeError) -> Outcome {
    match error {
        RuntimeError::ResourceFaulted => Outcome::ResourceFaulted,
        RuntimeError::DivisionByZero => Outcome::DivisionByZero,
        RuntimeError::SubrangeViolation { .. } => Outcome::SubrangeViolation,
        RuntimeError::Bytecode { .. } => Outcome::BytecodeRejected,
        _ => Outcome::CandidateRejected,
    }
}

fn check_outcome(
    label: &str,
    expected: Outcome,
    result: &Result<(), RuntimeError>,
) -> Result<(), String> {
    let observed = match result {
        Ok(()) => Outcome::Accepted,
        Err(error) => outcome_of(error),
    };
    let agrees = match expected {
        Outcome::CandidateRejected => observed != Outcome::Accepted,
        _ => observed == expected,
    };
    if agrees {
        Ok(())
    } else {
        Err(format!(
            "{label}: model expects {expected:?}, product returned {result:?}"
        ))
    }
}

fn compare(sut: &Sut, model: &Model, label: &str) -> Result<(), String> {
    let observed = sut.view()?;
    let expected = model.view();
    if observed == expected {
        Ok(())
    } else {
        Err(format!(
            "{label}: product {observed:?} differs from model {expected:?}"
        ))
    }
}

/// Apply one operation that the model defines, then check the result, the installed module and
/// every observable. A refused operation must also leave every observable unchanged; a scan that
/// faults is not a refusal, because it latches the fault the model describes (spec 11 6.6).
fn apply_checked(sut: &mut Sut, model: &mut Model, op: Op, step: usize) -> Result<(), String> {
    let label = format!("step {step}: {op:?}");
    let before = sut.view()?;
    let expected = model.step(op);
    let (result, installed_changed) = sut.apply(op);
    check_outcome(&label, expected.outcome, &result)?;
    if installed_changed != expected.installed_changed {
        return Err(format!(
            "{label}: installed module changed = {installed_changed}, model expects {}",
            expected.installed_changed
        ));
    }
    if !matches!(
        expected.outcome,
        Outcome::Accepted | Outcome::DivisionByZero
    ) {
        let after = sut.view()?;
        if after != before {
            return Err(format!(
                "{label}: refused operation changed the controller from {before:?} to {after:?}"
            ));
        }
    }
    compare(sut, model, &label)
}

/// Run one sequence. The operations the model does not define in the current state are skipped.
fn run_sequence(ops: &[Op]) -> Result<(), String> {
    let mut sut = Sut::new();
    let mut model = Model::initial();
    compare(&sut, &model, "initial state")?;
    for (step, &op) in ops.iter().enumerate() {
        if model.allows(op) {
            apply_checked(&mut sut, &mut model, op, step)?;
        }
    }
    Ok(())
}

/// Generated operations. Weights favour scans and the writes that drive the fault and the outputs.
fn op_strategy() -> impl Strategy<Value = Op> {
    Union::new_weighted(vec![
        (8, Just(Op::Scan).boxed()),
        (3, (-50i16..=50).prop_map(Op::WriteI).boxed()),
        (2, (-5i16..=20).prop_map(Op::WriteD).boxed()),
        (2, (-50i16..=50).prop_map(Op::EnqueueI).boxed()),
        (1, (1i16..=10).prop_map(Op::EnqueueD).boxed()),
        (2, (-50i16..=50).prop_map(Op::ForceI).boxed()),
        (1, (1i16..=10).prop_map(Op::ForceD).boxed()),
        (1, Just(Op::ReleaseI).boxed()),
        (1, Just(Op::ReleaseD).boxed()),
        (1, Just(Op::Pause).boxed()),
        (1, Just(Op::Resume).boxed()),
        (1, Just(Op::RestartWarm).boxed()),
        (1, Just(Op::RestartCold).boxed()),
        (1, Just(Op::RestartWarmFromStore).boxed()),
        (2, Just(Op::SaveStore).boxed()),
        (2, Just(Op::OnlineChangeB).boxed()),
        (1, Just(Op::OnlineChangeMalformed).boxed()),
        (1, Just(Op::OnlineChangeIncompatible).boxed()),
        (2, (1i64..=5).prop_map(Op::AdvanceTime).boxed()),
    ])
}

fn cases() -> u32 {
    std::env::var("PROPTEST_CASES")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(DEFAULT_CASES)
}

fn config() -> ProptestConfig {
    ProptestConfig {
        cases: cases(),
        rng_seed: RngSeed::Fixed(SEED),
        max_shrink_iters: 4096,
        failure_persistence: Some(Box::new(FileFailurePersistence::WithSource(
            "proptest-regressions",
        ))),
        ..ProptestConfig::default()
    }
}

proptest! {
    #![proptest_config(config())]

    /// Each generated sequence of 1 to 24 operations agrees with the model after every operation.
    /// A refused operation leaves the controller and the installed module unchanged.
    #[test]
    fn sequences_follow_the_lifecycle_model(ops in vec(op_strategy(), 1..25)) {
        if let Err(message) = run_sequence(&ops) {
            prop_assert!(false, "{}\n{:?}", message, ops);
        }
    }
}

/// A queued write accepted before a cold restart is not applied by the next scan (spec 11 6.9.2).
#[test]
fn queued_write_is_not_replayed_after_a_cold_restart() {
    run_sequence(&[Op::EnqueueI(9), Op::RestartCold, Op::Scan, Op::Scan])
        .expect("the controller follows the model");
}

/// Pause and resume keep a queued write and an active force (spec 11 6.9.2).
#[test]
fn pause_and_resume_keep_a_queued_write_and_a_force() {
    run_sequence(&[
        Op::EnqueueI(9),
        Op::ForceD(3),
        Op::Pause,
        Op::Resume,
        Op::Scan,
        Op::Scan,
    ])
    .expect("the controller follows the model");
}

/// Refused candidates keep a queued write and an active force (spec 11 6.7.1, spec 34 UPDATE-01).
#[test]
fn refused_candidates_keep_a_queued_write_and_a_force() {
    run_sequence(&[
        Op::EnqueueI(9),
        Op::ForceD(2),
        Op::OnlineChangeMalformed,
        Op::OnlineChangeIncompatible,
        Op::Scan,
    ])
    .expect("the controller follows the model");
}

/// Repeated scans on a faulted resource are refused until a restart recovers it (spec 10 4.7, 11 6.6).
#[test]
fn repeated_faults_are_refused_until_a_restart_recovers() {
    run_sequence(&[
        Op::WriteD(0),
        Op::Scan,
        Op::Scan,
        Op::AdvanceTime(3),
        Op::Scan,
        Op::RestartWarm,
        Op::Scan,
    ])
    .expect("the controller follows the model");
}

/// The bookkeeping of the fault latch alone: the fault is cleared and no value, output or time changes.
/// The test does not scan after the clear. Whether a cleared resource may resume is an open question.
#[test]
fn clear_fault_changes_only_the_fault_latch() {
    let mut sut = Sut::new();
    let mut model = Model::initial();
    apply_checked(&mut sut, &mut model, Op::WriteD(0), 0).expect("the write is accepted");
    apply_checked(&mut sut, &mut model, Op::Scan, 1).expect("the scan latches the fault");
    sut.harness.runtime_mut().clear_fault();
    let mut expected = model.view();
    expected.faulted = false;
    expected.last_fault = None;
    assert_eq!(
        sut.view().expect("the controller can be read"),
        expected,
        "clear_fault changes only the fault latch"
    );
}
