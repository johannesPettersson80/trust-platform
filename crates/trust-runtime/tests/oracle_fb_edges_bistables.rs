//! Independent oracle for the edge detectors R_TRIG and F_TRIG (with the aliases DIFU and DIFD)
//! and the bistables SR and RS, including the long input names of IEC Table 43.
//!
//! Authority: IEC 61131-3 Ed.3 section 6.6.3.5.2, Table 43 (bistable bodies; the initial state of
//! Q1 is the default BOOL value, FALSE) and section 6.6.3.5.3, Table 44 (the R_TRIG and F_TRIG
//! bodies, and the NOTE on the first execution after a cold restart). Specification 08 sections 2
//! and 3 restate them: SR is set dominant, RS is reset dominant, a trigger's Q is one execution
//! wide, each trigger keeps its own sampled phase per instance, and DIFU and DIFD execute the same
//! state machines as R_TRIG and F_TRIG.
//!
//! The reference models below are the Table 43 and Table 44 bodies written out in Rust. They
//! share no code with the runtime. Random inputs come from a fixed-seed generator, and every
//! output is compared after every executed call. A sequence stops at its first mismatch; every
//! mismatch of every sequence is collected and reported together.

use trust_runtime::harness::TestHarness;
use trust_runtime::value::Value;
use trust_runtime::RestartMode;

const STEPS_PER_SEQUENCE: usize = 1_000;
const SEQUENCES_PER_LANE: u64 = 4;

/// Deterministic SplitMix64 generator, so every input vector is reproducible.
struct Rng(u64);

impl Rng {
    fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    fn per_mille(&mut self, chance: u64) -> bool {
        self.next_u64() % 1000 < chance
    }
}

/// The body of Table 44: `Q := CLK AND NOT M; M := CLK;` for R_TRIG and
/// `Q := NOT CLK AND NOT M; M := NOT CLK;` for F_TRIG, with M FALSE on a cold start.
#[derive(Clone, Copy, Debug)]
struct Trigger {
    falling: bool,
    memory: bool,
}

impl Trigger {
    fn new(falling: bool) -> Self {
        Self {
            falling,
            memory: false,
        }
    }

    fn step(&mut self, clk: bool) -> bool {
        if self.falling {
            let q = !clk && !self.memory;
            self.memory = !clk;
            q
        } else {
            let q = clk && !self.memory;
            self.memory = clk;
            q
        }
    }

    /// Table 44 NOTE: a cold restart leaves M FALSE, so the first execution sees a fresh instance.
    fn cold_restart(&mut self) {
        self.memory = false;
    }
}

#[derive(Clone, Copy, Debug)]
enum Bistable {
    /// Table 43 SR: `Q1 := S1 OR (NOT R AND Q1)`.
    Sr,
    /// Table 43 RS: `Q1 := NOT R1 AND (S OR Q1)`.
    Rs,
}

/// The stored Q1 of one bistable instance. It starts FALSE and holds when neither input dominates.
#[derive(Clone, Copy, Debug)]
struct Latch {
    kind: Bistable,
    q1: bool,
}

impl Latch {
    /// `set` is S1 for SR and S for RS. `reset` is R for SR and R1 for RS.
    fn step(&mut self, set: bool, reset: bool) -> bool {
        self.q1 = match self.kind {
            Bistable::Sr => set || (!reset && self.q1),
            Bistable::Rs => !reset && (set || self.q1),
        };
        self.q1
    }
}

fn edge_source(block: &str) -> String {
    format!(
        "PROGRAM Main\n\
         VAR\n\
             edge_inst : {block};\n\
             clk_sig : BOOL;\n\
             q_out : BOOL;\n\
         END_VAR\n\
         edge_inst(CLK := clk_sig, Q => q_out);\n\
         END_PROGRAM\n"
    )
}

fn latch_source(block: &str, set_pin: &str, reset_pin: &str) -> String {
    format!(
        "PROGRAM Main\n\
         VAR\n\
             latch_inst : {block};\n\
             set_sig : BOOL;\n\
             reset_sig : BOOL;\n\
             q_out : BOOL;\n\
         END_VAR\n\
         latch_inst({set_pin} := set_sig, {reset_pin} := reset_sig, Q1 => q_out);\n\
         END_PROGRAM\n"
    )
}

/// Runs seeded CLK sequences through one edge block, with cold restarts, against the Table 44 body.
/// Returns the number of calls that agreed with the model; mismatches go to `ledger`.
fn run_edge_lane(block: &str, falling: bool, ledger: &mut Vec<String>) -> usize {
    let source = edge_source(block);
    let mut executed = 0;
    for sequence in 0..SEQUENCES_PER_LANE {
        let label = format!("{block} sequence {sequence}");
        let mut harness = match TestHarness::from_source(&source) {
            Ok(harness) => harness,
            Err(error) => {
                ledger.push(format!("{label} does not compile: {error}"));
                continue;
            }
        };
        let mut rng = Rng(0xED6E_0000 + u64::from(falling) * 1_000 + sequence);
        let mut model = Trigger::new(falling);
        let mut clk = false;
        for step in 0..STEPS_PER_SEQUENCE {
            if rng.per_mille(400) {
                clk = !clk;
            }
            if rng.per_mille(10) {
                if let Err(error) = harness.restart(RestartMode::Cold) {
                    ledger.push(format!("{label} step {step}: cold restart: {error:?}"));
                    break;
                }
                model.cold_restart();
            }
            harness.set_input("clk_sig", Value::Bool(clk));
            let result = harness.cycle();
            if !result.errors.is_empty() {
                ledger.push(format!(
                    "{label} step {step} with CLK={clk}: cycle errors {:?}",
                    result.errors
                ));
                break;
            }
            let expected = model.step(clk);
            let q = harness.get_output("q_out");
            if q != Some(Value::Bool(expected)) {
                ledger.push(format!(
                    "{label} step {step} with CLK={clk}: expected Q={expected}, runtime Q={q:?}"
                ));
                break;
            }
            executed += 1;
        }
    }
    executed
}

/// Runs seeded set and reset sequences through one bistable, against the Table 43 body. Returns
/// the number of calls that agreed with the model; mismatches go to `ledger`.
fn run_latch_lane(
    block: &str,
    set_pin: &str,
    reset_pin: &str,
    kind: Bistable,
    ledger: &mut Vec<String>,
) -> usize {
    let source = latch_source(block, set_pin, reset_pin);
    let is_rs = matches!(kind, Bistable::Rs);
    let mut executed = 0;
    for sequence in 0..SEQUENCES_PER_LANE {
        let label = format!("{block} ({set_pin}, {reset_pin}) sequence {sequence}");
        let mut harness = match TestHarness::from_source(&source) {
            Ok(harness) => harness,
            Err(error) => {
                ledger.push(format!("{label} does not compile: {error}"));
                continue;
            }
        };
        let mut rng = Rng(0x1A7C_0000 + u64::from(is_rs) * 1_000 + sequence);
        let mut model = Latch { kind, q1: false };
        let mut set = false;
        let mut reset = false;
        for step in 0..STEPS_PER_SEQUENCE {
            if rng.per_mille(400) {
                set = !set;
            }
            if rng.per_mille(400) {
                reset = !reset;
            }
            harness.set_input("set_sig", Value::Bool(set));
            harness.set_input("reset_sig", Value::Bool(reset));
            let result = harness.cycle();
            if !result.errors.is_empty() {
                ledger.push(format!(
                    "{label} step {step} with {set_pin}={set}, {reset_pin}={reset}: \
                     cycle errors {:?}",
                    result.errors
                ));
                break;
            }
            let expected = model.step(set, reset);
            let q = harness.get_output("q_out");
            if q != Some(Value::Bool(expected)) {
                ledger.push(format!(
                    "{label} step {step} with {set_pin}={set}, {reset_pin}={reset}: \
                     expected Q1={expected}, runtime Q1={q:?}"
                ));
                break;
            }
            executed += 1;
        }
    }
    executed
}

/// Asserts that no lane recorded a mismatch and that enough calls were generated.
fn assert_clean(name: &str, executed: usize, ledger: &[String]) {
    assert!(
        ledger.is_empty(),
        "{name}: {} sequence(s) diverged from the reference model ({executed} calls agreed):\n{}",
        ledger.len(),
        ledger.join("\n")
    );
    assert!(
        executed >= 3_000,
        "{name}: only {executed} executed calls were generated"
    );
}

#[test]
fn oracle_r_trig_matches_table_44() {
    let mut ledger = Vec::new();
    let executed = run_edge_lane("R_TRIG", false, &mut ledger);
    assert_clean("R_TRIG", executed, &ledger);
}

#[test]
fn oracle_f_trig_matches_table_44() {
    let mut ledger = Vec::new();
    let executed = run_edge_lane("F_TRIG", true, &mut ledger);
    assert_clean("F_TRIG", executed, &ledger);
}

#[test]
fn oracle_difu_and_difd_aliases_match_table_44() {
    let mut ledger = Vec::new();
    let executed =
        run_edge_lane("DIFU", false, &mut ledger) + run_edge_lane("DIFD", true, &mut ledger);
    assert_clean("DIFU/DIFD", executed, &ledger);
}

#[test]
fn oracle_sr_and_rs_match_table_43() {
    let mut ledger = Vec::new();
    let executed = run_latch_lane("SR", "S1", "R", Bistable::Sr, &mut ledger)
        + run_latch_lane("RS", "S", "R1", Bistable::Rs, &mut ledger);
    assert_clean("SR/RS", executed, &ledger);
}

/// IEC Table 43 rows 1b and 2b give the long input names SET1/RESET and SET/RESET1. The HIR
/// accepts them as aliases (trust-hir type_check/calls/standard_fbs.rs), so the runtime must bind
/// them to the same bodies. The runtime binder matches registry parameter names only.
#[test]
fn oracle_sr_and_rs_long_input_names_match_table_43() {
    let mut ledger = Vec::new();
    let executed = run_latch_lane("SR", "SET1", "RESET", Bistable::Sr, &mut ledger)
        + run_latch_lane("RS", "SET", "RESET1", Bistable::Rs, &mut ledger);
    assert_clean("SR/RS long names", executed, &ledger);
}

/// Table 44 NOTE and spec 08 section 3 (Cold Restart Behavior): after a cold restart, an R_TRIG
/// with CLK TRUE and an F_TRIG with CLK FALSE each give Q TRUE on their first execution. Later
/// executions with the same CLK give Q FALSE.
#[test]
fn oracle_cold_restart_first_execution_rules() {
    let mut harness = TestHarness::from_source(
        "PROGRAM Main\n\
         VAR\n\
             rising : R_TRIG;\n\
             falling : F_TRIG;\n\
             clk_sig : BOOL;\n\
             q_rise : BOOL;\n\
             q_fall : BOOL;\n\
         END_VAR\n\
         rising(CLK := clk_sig, Q => q_rise);\n\
         falling(CLK := clk_sig, Q => q_fall);\n\
         END_PROGRAM\n",
    )
    .unwrap_or_else(|error| panic!("edge fixture must compile: {error}"));
    harness.set_input("clk_sig", Value::Bool(true));
    harness.cycle();
    harness.assert_eq("q_rise", Value::Bool(true));
    harness.assert_eq("q_fall", Value::Bool(false));
    harness.cycle();
    harness.assert_eq("q_rise", Value::Bool(false));
    harness.assert_eq("q_fall", Value::Bool(false));

    harness
        .restart(RestartMode::Cold)
        .unwrap_or_else(|error| panic!("cold restart: {error:?}"));
    harness.set_input("clk_sig", Value::Bool(false));
    harness.cycle();
    harness.assert_eq("q_rise", Value::Bool(false));
    harness.assert_eq("q_fall", Value::Bool(true));
    harness.cycle();
    harness.assert_eq("q_rise", Value::Bool(false));
    harness.assert_eq("q_fall", Value::Bool(false));
}
