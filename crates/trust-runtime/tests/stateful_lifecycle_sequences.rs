//! Stateful lifecycle sequences (scope H assurance, checklist A08).
//!
//! Each property runs a generated sequence of operations against one installed controller and against
//! the explicit model in `support/lifecycle_model.rs`. The model is written from spec 10 sections 4.7
//! and 4.8, spec 11 sections 6.6, 6.7, 6.7.1, 6.9 and 6.9.2, and spec 34 HOST-09, UPDATE-01, UPDATE-02
//! and UPDATE-07. After every operation the test compares the result and every observable that the
//! model defines. A refused operation must also leave the whole observable state and the installed
//! module unchanged.
//!
//! The sequences pin these rules of spec 11 (the section is in brackets):
//! - a scan that faults after it applied its boundary writes keeps them, and the fault removes the
//!   forces without writing a replacement value (6.6, state after a faulting scan);
//! - a second queued write to one target replaces the first, also when the later one is refused when
//!   the boundary applies it (6.9.2);
//! - checked writes, queued writes, forces and releases are accepted while the resource is faulted,
//!   a refused scan leaves them pending, and the commit of a recovery clears them (6.9.2);
//! - a restart, an online change and a retain save between scans leave the debugger hold in place
//!   (6.9);
//! - a restart reinitializes a located variable and carries its output image over (6.7, directly
//!   represented variables at restart).
//!
//! The driver does not generate a scan under a debugger hold, which waits inside the cycle until the
//! hold is released (`debugger_hold_operations.rs` covers it with a second thread); a pause or a retain
//! save of a faulted resource, which the specifications do not define; and stale handles across
//! replacement (scope C).
//!
//! The compiled debugger answers no query of a pause armed between scans, so the `held` observable is
//! the driver's record of the pause it armed and resumed, and the installed module changes exactly
//! when an online change commits, which the driver counts.
//!
//! The seed is fixed, so a run replays the same sequences. `PROPTEST_CASES` overrides the default of
//! 64 cases. A failing case is saved in the `proptest-regressions` store and replayed first.
//!
//! The tests need the toolchain of specification 44 §4.6 at `TRUST_TOOLCHAIN`.

#![cfg(feature = "full")]

#[path = "support/lifecycle_model.rs"]
mod lifecycle_model;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;
use std::time::{Duration as WallDuration, Instant};

use lifecycle_model::{Checkpoint, Fault, Model, Op, Outcome, View};
use proptest::collection::vec;
use proptest::prelude::*;
use proptest::strategy::Union;
use proptest::test_runner::{FileFailurePersistence, RngSeed};
use trust_runtime::debug::{DebugControl, ForcedVarTarget};
use trust_runtime::error::RuntimeError;
use trust_runtime::harness::{SourceFile, TestHarness};
use trust_runtime::native::{build, Options, Target, Toolchain};
use trust_runtime::retain::RetainStore;
use trust_runtime::value::{Duration, Value};
use trust_runtime::{RestartMode, RetainSnapshot};

/// Seed for the generated sequences ("trustSEQ").
const SEED: u64 = 0x7472_7573_7453_4551;
/// Cases when `PROPTEST_CASES` is unset.
const DEFAULT_CASES: u32 = 64;

/// Program A, installed at the start. `Main.r` and `g_r` are retained. `out_bit` is the process
/// output `%QX0.0`. `lamp` is the output `%QX0.1` with the initial value TRUE, which no program assigns.
const SOURCE_A: &str = r#"
VAR_GLOBAL RETAIN
    g_r : INT := INT#5;
END_VAR
VAR_GLOBAL CONSTANT
    ident : INT := INT#1;
END_VAR
VAR_GLOBAL
    i : INT := INT#0;
    d : INT(0..10) := INT#1;
    out_bit AT %QX0.0 : BOOL := FALSE;
    lamp AT %QX0.1 : BOOL := TRUE;
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

/// Program B, a replacement with the same declarations, another `ident` constant and other arithmetic.
const SOURCE_B: &str = r#"
VAR_GLOBAL RETAIN
    g_r : INT := INT#5;
END_VAR
VAR_GLOBAL CONSTANT
    ident : INT := INT#2;
END_VAR
VAR_GLOBAL
    i : INT := INT#0;
    d : INT(0..10) := INT#1;
    out_bit AT %QX0.0 : BOOL := FALSE;
    lamp AT %QX0.1 : BOOL := TRUE;
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

/// Bytes that do not decode as a program image.
const MALFORMED_IMAGE: [u8; 4] = [0x00, 0x01, 0x02, 0x03];

/// The compiled candidate, built once per test binary.
struct Candidates {
    b: Vec<u8>,
}

fn candidates() -> &'static Candidates {
    static CANDIDATES: OnceLock<Candidates> = OnceLock::new();
    CANDIDATES.get_or_init(|| {
        let toolchain = Toolchain::find()
            .expect("TRUST_TOOLCHAIN names the toolchain of specification 44 §4.6");
        let target = Target::this_cpu().expect("a computer target");
        let work =
            std::env::temp_dir().join(format!("trust-lifecycle-candidate-{}", std::process::id()));
        let built = build(
            &[SourceFile::new(SOURCE_B)],
            &Options {
                target,
                hooks: true,
            },
            &toolchain,
            &work,
        );
        let _ = std::fs::remove_dir_all(&work);
        Candidates {
            b: built.expect("candidate B compiles").image,
        }
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

/// The installed controller under test, its debugger handle and its durable store, with the pause
/// the driver armed and the online changes that committed.
struct Sut {
    harness: TestHarness,
    debug: DebugControl,
    store: Arc<Mutex<RetainSnapshot>>,
    armed: bool,
    installs: usize,
}

impl Sut {
    fn new() -> Self {
        let mut harness = TestHarness::from_source(SOURCE_A).expect("program A compiles");
        let runtime = harness.runtime_mut();
        let debug = DebugControl::new();
        runtime.set_debug_control(debug.clone());
        let store = Arc::new(Mutex::new(RetainSnapshot::default()));
        runtime
            .set_retain_store(
                Some(Box::new(MemoryStore {
                    content: Arc::clone(&store),
                })),
                None,
            )
            .expect("the retain store is set");
        Self {
            harness,
            debug,
            store,
            armed: false,
            installs: 0,
        }
    }

    /// Run one operation. The flag tells whether the installed module changed.
    fn apply(&mut self, op: Op) -> (Result<(), RuntimeError>, bool) {
        let before = self.installs;
        let result = self.run(op);
        (result, before != self.installs)
    }

    fn run(&mut self, op: Op) -> Result<(), RuntimeError> {
        let runtime = self.harness.runtime_mut();
        match op {
            Op::Scan => runtime.execute_cycle(),
            Op::WriteI(value) => runtime.write_variable("i", &Value::Int(value)),
            Op::WriteD(value) => runtime.write_variable("d", &Value::Int(value)),
            Op::WriteLamp(value) => runtime.write_variable("lamp", &Value::Bool(value)),
            Op::EnqueueI(value) => {
                self.debug
                    .try_enqueue_global_write("i", Value::Int(value))
                    .expect("the write queue has room");
                Ok(())
            }
            Op::EnqueueD(value) => {
                self.debug
                    .try_enqueue_global_write("d", Value::Int(value))
                    .expect("the write queue has room");
                Ok(())
            }
            Op::ForceI(value) => {
                self.debug
                    .try_force_global("i", Value::Int(value))
                    .expect("the force queue has room");
                Ok(())
            }
            Op::ForceD(value) => {
                self.debug
                    .try_force_global("d", Value::Int(value))
                    .expect("the force queue has room");
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
                self.debug
                    .compiled()
                    .expect("the runtime set its compiled debugger")
                    .pause();
                self.armed = true;
                Ok(())
            }
            Op::Resume => {
                self.debug
                    .compiled()
                    .expect("the runtime set its compiled debugger")
                    .resume();
                self.armed = false;
                Ok(())
            }
            Op::RestartWarm => runtime.restart(RestartMode::Warm),
            Op::RestartCold => runtime.restart(RestartMode::Cold),
            Op::RestartWarmFromStore => runtime.restart_with_retain_store(RestartMode::Warm),
            Op::SaveStore => runtime.save_retain_store(),
            Op::OnlineChangeB => {
                let result = runtime.apply_online_change_bytes(&candidates().b);
                if result.is_ok() {
                    self.installs += 1;
                }
                result
            }
            Op::OnlineChangeMalformed => runtime.apply_online_change_bytes(&MALFORMED_IMAGE),
            Op::AdvanceTime(ms) => {
                let now = runtime.current_time().as_nanos();
                runtime.set_current_time(Duration::from_nanos(
                    now + Duration::from_millis(ms).as_nanos(),
                ));
                Ok(())
            }
        }
    }

    /// Read every observable that the model defines. Reads do not change the controller.
    fn view(&self) -> Result<View, String> {
        let runtime = self.harness.runtime();
        let (store_entries, store) = stored(&self.store);
        Ok(View {
            ident: int_output(&self.harness, "ident")?,
            cycles: runtime.cycle_counter(),
            faulted: runtime.faulted(),
            last_fault: runtime.last_fault().as_ref().map(fault_of),
            held: self.armed,
            time_ms: runtime.current_time().as_millis(),
            r: int_output(&self.harness, "r")?,
            g_r: int_output(&self.harness, "g_r")?,
            n: int_output(&self.harness, "n")?,
            q: int_output(&self.harness, "q")?,
            i: int_output(&self.harness, "i")?,
            d: int_output(&self.harness, "d")?,
            out: image_bool(&self.harness, "%QX0.0")?,
            lamp: bool_output(&self.harness, "lamp")?,
            lamp_out: image_bool(&self.harness, "%QX0.1")?,
            forced_i: forced_int(&self.debug, "i")?,
            forced_d: forced_int(&self.debug, "d")?,
            store_entries,
            store,
        })
    }
}

fn int_output(harness: &TestHarness, name: &str) -> Result<i16, String> {
    match harness.get_output(name) {
        Some(Value::Int(value)) => Ok(value),
        other => Err(format!("{name} is not an INT: {other:?}")),
    }
}

fn bool_output(harness: &TestHarness, name: &str) -> Result<bool, String> {
    match harness.get_output(name) {
        Some(Value::Bool(value)) => Ok(value),
        other => Err(format!("{name} is not a BOOL: {other:?}")),
    }
}

/// One bit of the published output image.
fn image_bool(harness: &TestHarness, address: &str) -> Result<bool, String> {
    match harness.get_direct_output(address) {
        Ok(Value::Bool(value)) => Ok(value),
        other => Err(format!("{address} is not a BOOL image: {other:?}")),
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
        RuntimeError::InvalidBundle(_) => Outcome::BytecodeRejected,
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

/// The longest a sequence may take. A sequence runs in milliseconds, so the limit matters only when an
/// operation blocks the single-threaded driver for good.
const SEQUENCE_LIMIT: WallDuration = WallDuration::from_secs(120);

/// A hang guard for one sequence. The driver restarts, replaces and saves while the debugger holds the
/// scan, which does not block because statement hooks stay off while a candidate is constructed. If
/// that ever changed, the driver would wait for itself with no timeout, so a sequence that does not
/// finish within [`SEQUENCE_LIMIT`] ends the test binary with a message instead of hanging.
struct HangGuard {
    finished: Arc<AtomicBool>,
}

impl HangGuard {
    fn arm(ops: &[Op]) -> Self {
        let finished = Arc::new(AtomicBool::new(false));
        let flag = Arc::clone(&finished);
        let sequence = format!("{ops:?}");
        thread::spawn(move || {
            let deadline = Instant::now() + SEQUENCE_LIMIT;
            while Instant::now() < deadline {
                if flag.load(Ordering::SeqCst) {
                    return;
                }
                thread::sleep(WallDuration::from_millis(50));
            }
            eprintln!(
                "a lifecycle sequence did not finish within {SEQUENCE_LIMIT:?}; an operation \
                 blocks the driver (a debugger hold?): {sequence}"
            );
            std::process::exit(101);
        });
        Self { finished }
    }
}

impl Drop for HangGuard {
    fn drop(&mut self) {
        self.finished.store(true, Ordering::SeqCst);
    }
}

/// Run one sequence. A generated sequence may contain an operation that the driver does not issue in
/// the current state, because the single-threaded driver cannot (see the file notes); it skips that
/// operation. An exact sequence is written by hand to pin a rule, so an operation that the driver
/// would skip fails the test instead of letting it pass without its step.
fn run_sequence(ops: &[Op], exact: bool) -> Result<(), String> {
    let _guard = HangGuard::arm(ops);
    let mut sut = Sut::new();
    let mut model = Model::initial();
    compare(&sut, &model, "initial state")?;
    for (step, &op) in ops.iter().enumerate() {
        if model.allows(op) {
            apply_checked(&mut sut, &mut model, op, step)?;
        } else if exact {
            return Err(format!(
                "step {step}: the driver does not issue {op:?} in this state"
            ));
        }
    }
    Ok(())
}

/// Run a hand-written sequence in which the driver issues every step.
fn run_exact(ops: &[Op]) {
    run_sequence(ops, true).expect("the controller follows the model");
}

/// Generated operations. Weights favour scans and the writes that drive the fault and the outputs.
fn op_strategy() -> impl Strategy<Value = Op> {
    Union::new_weighted(vec![
        (8, Just(Op::Scan).boxed()),
        (3, (-50i16..=50).prop_map(Op::WriteI).boxed()),
        (2, (-5i16..=20).prop_map(Op::WriteD).boxed()),
        (2, any::<bool>().prop_map(Op::WriteLamp).boxed()),
        (2, (-50i16..=50).prop_map(Op::EnqueueI).boxed()),
        (2, (-5i16..=20).prop_map(Op::EnqueueD).boxed()),
        (2, (-50i16..=50).prop_map(Op::ForceI).boxed()),
        (1, (0i16..=10).prop_map(Op::ForceD).boxed()),
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
        if let Err(message) = run_sequence(&ops, false) {
            prop_assert!(false, "{}\n{:?}", message, ops);
        }
    }
}

/// A queued write accepted before a cold restart is not applied by the next scan (spec 11 6.9.2).
#[test]
fn queued_write_is_not_replayed_after_a_cold_restart() {
    run_exact(&[Op::EnqueueI(9), Op::RestartCold, Op::Scan, Op::Scan]);
}

/// Pause and resume keep a queued write and an active force (spec 11 6.9.2).
#[test]
fn pause_and_resume_keep_a_queued_write_and_a_force() {
    run_exact(&[
        Op::EnqueueI(9),
        Op::ForceD(3),
        Op::Pause,
        Op::Resume,
        Op::Scan,
        Op::Scan,
    ]);
}

/// Refused candidates keep a queued write and an active force (spec 11 6.7.1, spec 34 UPDATE-01).
#[test]
fn refused_candidates_keep_a_queued_write_and_a_force() {
    run_exact(&[
        Op::EnqueueI(9),
        Op::ForceD(2),
        Op::OnlineChangeMalformed,
        Op::Scan,
    ]);
}

/// Repeated scans on a faulted resource are refused until a restart recovers it (spec 10 4.7, 11 6.6).
#[test]
fn repeated_faults_are_refused_until_a_restart_recovers() {
    run_exact(&[
        Op::WriteD(0),
        Op::Scan,
        Op::Scan,
        Op::AdvanceTime(3),
        Op::Scan,
        Op::RestartWarm,
        Op::Scan,
    ]);
}

/// A scan that faults after it applied a queued write keeps the write: nothing the scan stored before
/// the fault is undone, and the consumed write is not applied again (spec 11 6.6, state after a
/// faulting scan).
#[test]
fn scan_that_faults_after_a_queued_write_keeps_the_write() {
    run_exact(&[
        Op::WriteD(0),
        Op::EnqueueI(7),
        Op::Scan,
        Op::Scan,
        Op::RestartWarm,
        Op::Scan,
    ]);
}

/// A scan that faults after it applied a force keeps the forced value, and the fault removes the force
/// without writing a replacement value (spec 11 6.6 state after a faulting scan, 6.9.2).
#[test]
fn scan_that_faults_after_a_force_keeps_the_forced_value_and_removes_the_force() {
    run_exact(&[
        Op::WriteD(0),
        Op::ForceI(5),
        Op::ForceD(0),
        Op::Scan,
        Op::RestartCold,
        Op::Scan,
    ]);
}

/// A queued write can itself cause the fault. It stays applied, and the next scan is refused (spec 11
/// 6.6 state after a faulting scan).
#[test]
fn queued_write_that_causes_the_fault_stays_applied() {
    run_exact(&[
        Op::EnqueueD(0),
        Op::Scan,
        Op::Scan,
        Op::RestartWarm,
        Op::Scan,
    ]);
}

/// A queued write that the boundary refuses does not rescue the scan from the fault (spec 11 6.9.2).
#[test]
fn refused_queued_write_leaves_the_fault_condition_in_place() {
    run_exact(&[Op::WriteD(0), Op::EnqueueD(50), Op::Scan, Op::RestartWarm]);
}

/// A second queued write to one target replaces the first in place, so the target takes the last value
/// (spec 11 6.8.1, 6.9.2).
#[test]
fn second_queued_write_to_one_target_replaces_the_first() {
    run_exact(&[
        Op::EnqueueI(3),
        Op::EnqueueI(7),
        Op::Scan,
        Op::Scan,
        Op::EnqueueI(-2),
        Op::EnqueueI(4),
        Op::EnqueueI(1),
        Op::Scan,
    ]);
}

/// The replacement drops the earlier value for good: when the later queued write is refused at the
/// boundary, the target keeps its value and does not take the earlier one (spec 11 6.9.2).
#[test]
fn replaced_queued_write_does_not_return_when_the_later_one_is_refused() {
    run_exact(&[
        Op::EnqueueD(5),
        Op::EnqueueD(50),
        Op::Scan,
        Op::EnqueueD(6),
        Op::Scan,
    ]);
}

/// A force on the same target wins over a queued write at the boundary, and a release writes no
/// replacement value (spec 10 4.7 step 2, spec 11 6.9.2).
#[test]
fn force_wins_over_a_queued_write_to_the_same_target() {
    run_exact(&[
        Op::EnqueueI(4),
        Op::ForceI(9),
        Op::Scan,
        Op::ReleaseI,
        Op::Scan,
    ]);
}

/// Writes, queued writes, forces and releases are accepted while the resource is faulted. A scan
/// request is refused and leaves them pending, and the commit of the recovery clears them, so they
/// are never replayed into the recovered lifecycle (spec 11 6.9.2).
#[test]
fn writes_and_forces_accepted_while_faulted_are_cleared_by_the_recovery_commit() {
    run_exact(&[
        Op::WriteD(0),
        Op::Scan,
        Op::WriteI(3),
        Op::WriteLamp(false),
        Op::EnqueueI(8),
        Op::ForceI(6),
        Op::ForceD(2),
        Op::ReleaseD,
        Op::ForceD(4),
        Op::Scan,
        Op::RestartWarm,
        Op::Scan,
        Op::Scan,
    ]);
}

/// An online change recovers a faulted resource and clears what was accepted while it was faulted
/// (spec 11 6.6, 6.9.2). It keeps every value (spec 11 6.7.1, decision OC-1), the divisor 0 that
/// faulted the resource included, so B, which does not handle it, faults again at its first scan.
#[test]
fn online_change_recovery_clears_what_was_accepted_while_faulted() {
    run_exact(&[
        Op::WriteD(0),
        Op::Scan,
        Op::EnqueueI(8),
        Op::ForceD(4),
        Op::OnlineChangeB,
        Op::Scan,
        Op::Scan,
    ]);
}

/// A recovery that fails commits nothing: what was accepted while the resource was faulted stays
/// pending, and the resource stays faulted (spec 11 6.7.1, 6.9.2).
#[test]
fn failed_recovery_leaves_what_was_accepted_while_faulted_pending() {
    run_exact(&[
        Op::WriteD(0),
        Op::Scan,
        Op::EnqueueI(8),
        Op::ForceI(6),
        Op::OnlineChangeMalformed,
        Op::Scan,
        Op::ReleaseI,
        Op::RestartCold,
        Op::Scan,
    ]);
}

/// A checked write while faulted is accepted or refused by its ordinary rules, changes only its
/// destination, and neither clears the fault nor resumes execution (spec 11 6.9.2, spec 34 HOST-09).
#[test]
fn checked_write_while_faulted_changes_only_its_destination() {
    run_exact(&[
        Op::WriteD(0),
        Op::Scan,
        Op::WriteI(11),
        Op::WriteD(99),
        Op::WriteD(2),
        Op::Scan,
        Op::AdvanceTime(4),
        Op::RestartWarm,
        Op::Scan,
    ]);
}

/// A restart, an online change and a retain save between scans leave the debugger hold in place, and a
/// restart or an online change that commits clears the queued writes and forces as always (spec 11
/// 6.9; spec 13 reload).
#[test]
fn restart_and_replacement_between_scans_leave_the_debugger_hold_in_place() {
    run_exact(&[
        Op::EnqueueI(9),
        Op::ForceD(3),
        Op::Pause,
        Op::RestartWarm,
        Op::EnqueueI(2),
        Op::ForceI(5),
        Op::OnlineChangeB,
        Op::SaveStore,
        Op::RestartWarmFromStore,
        Op::RestartCold,
        Op::Resume,
        Op::Scan,
        Op::Scan,
    ]);
}

/// Refused candidates under a hold change neither the controller nor the hold (spec 11 6.7.1, 6.9).
#[test]
fn refused_candidates_under_a_hold_leave_the_hold_and_the_mutations_in_place() {
    run_exact(&[
        Op::EnqueueI(9),
        Op::ForceD(3),
        Op::Pause,
        Op::OnlineChangeMalformed,
        Op::Resume,
        Op::Scan,
    ]);
}

/// Checked writes, queued writes, forces and releases are accepted under a hold and behave as without
/// one (spec 11 6.9).
#[test]
fn writes_and_forces_under_a_hold_behave_as_without_a_hold() {
    run_exact(&[
        Op::Pause,
        Op::WriteI(5),
        Op::WriteLamp(false),
        Op::EnqueueD(4),
        Op::ForceI(2),
        Op::ReleaseI,
        Op::WriteD(99),
        Op::Resume,
        Op::Scan,
    ]);
}

/// A restart reinitializes the variable that is located at an output and carries the output image
/// over: the image keeps the last published value until a scan completes (spec 11 6.7, directly
/// represented variables at restart; spec 34 10). The same holds for both restart modes and a restart
/// from the store; an online change carries the image over too and keeps the variable's value
/// (spec 11 6.7.1, decision OC-1). The first sequence has TRUE images before the restart
/// (`WriteI(8)` makes `out` TRUE), so a restart that zeroed the images would show at once. The second
/// has a FALSE `lamp` image while the variable is TRUE again after the restart, so a restart that
/// wrote the initializer into the image would show.
#[test]
fn restart_reinitializes_a_located_variable_and_carries_its_output_image_over() {
    for restart in [
        Op::RestartWarm,
        Op::RestartCold,
        Op::RestartWarmFromStore,
        Op::OnlineChangeB,
    ] {
        run_exact(&[Op::WriteI(8), Op::Scan, restart, Op::Scan]);
        run_exact(&[
            Op::WriteI(8),
            Op::Scan,
            Op::WriteLamp(false),
            Op::Scan,
            restart,
            Op::Scan,
        ]);
    }
}

/// A scan that faults publishes nothing, so the image of a located variable keeps its last value, and
/// a recovery then reinitializes the variable and carries the image over (spec 11 6.6, 6.7).
#[test]
fn faulting_scan_does_not_publish_a_located_variable() {
    run_exact(&[
        Op::Scan,
        Op::WriteLamp(false),
        Op::WriteD(0),
        Op::Scan,
        Op::RestartCold,
        Op::Scan,
    ]);
}
