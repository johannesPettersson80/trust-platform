//! Independent oracle for the counter function blocks CTU, CTD and CTUD and their typed variants.
//!
//! Authority: IEC 61131-3 Ed.3 section 6.6.3.5.4, Table 45 (CU, CD and the edge evaluated by the
//! data type; the NOTE that PVmin and PVmax are implementer specific), and specification 08
//! section 4 (behaviour and the Counter Runtime Conformance Contract). truST's PVmax and PVmin
//! are the maximum and minimum of each declared integer type, and a counter saturates there
//! without wrapping. The 15 October decision is applied: the edge memory of each count input is
//! updated on every executed call, including a call whose reset or load dominates the count.
//!
//! The reference model below is written from those clauses and shares no code with the runtime.
//! Two paths are compared. The generic and typed forms run as Structured Text through TestHarness
//! with random inputs. The storage path (`execute_builtin_in_storage`) starts CV close to the
//! limit, so saturation is reached in a few calls for every declared type. A sequence stops at its
//! first mismatch; every mismatch of every sequence is collected and reported together.

use std::collections::BTreeMap;

use trust_runtime::harness::TestHarness;
use trust_runtime::memory::VariableStorage;
use trust_runtime::stdlib::fbs::{execute_builtin_in_storage, BuiltinFbKind};
use trust_runtime::value::{Duration, Value};

const STEPS_PER_SEQUENCE: usize = 400;
const SEQUENCES_PER_LANE: u64 = 6;
const STORAGE_SEQUENCES: u64 = 4;
const STORAGE_STEPS: usize = 300;
const MIN_RULE_HITS: u64 = 5;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Block {
    Ctu,
    Ctd,
    Ctud,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Ty {
    Int,
    Dint,
    Lint,
    Udint,
    Ulint,
}

impl Ty {
    const ALL: [Ty; 5] = [Ty::Int, Ty::Dint, Ty::Lint, Ty::Udint, Ty::Ulint];

    fn st_name(self) -> &'static str {
        match self {
            Ty::Int => "INT",
            Ty::Dint => "DINT",
            Ty::Lint => "LINT",
            Ty::Udint => "UDINT",
            Ty::Ulint => "ULINT",
        }
    }

    fn suffix(self) -> &'static str {
        match self {
            Ty::Int => "_INT",
            Ty::Dint => "_DINT",
            Ty::Lint => "_LINT",
            Ty::Udint => "_UDINT",
            Ty::Ulint => "_ULINT",
        }
    }

    /// PVmin and PVmax of the declared type (IEC Table 45 NOTE; spec 08 section 4).
    fn limits(self) -> (i128, i128) {
        match self {
            Ty::Int => (i128::from(i16::MIN), i128::from(i16::MAX)),
            Ty::Dint => (i128::from(i32::MIN), i128::from(i32::MAX)),
            Ty::Lint => (i128::from(i64::MIN), i128::from(i64::MAX)),
            Ty::Udint => (0, i128::from(u32::MAX)),
            Ty::Ulint => (0, i128::from(u64::MAX)),
        }
    }

    fn value(self, v: i128) -> Value {
        match self {
            Ty::Int => Value::Int(i16::try_from(v).expect("INT range")),
            Ty::Dint => Value::DInt(i32::try_from(v).expect("DINT range")),
            Ty::Lint => Value::LInt(i64::try_from(v).expect("LINT range")),
            Ty::Udint => Value::UDInt(u32::try_from(v).expect("UDINT range")),
            Ty::Ulint => Value::ULInt(u64::try_from(v).expect("ULINT range")),
        }
    }
}

/// A typed variant (`CTU_DINT`) or the generic form, which takes INT by default.
#[derive(Clone, Copy, Debug)]
struct Lane {
    block: Block,
    ty: Ty,
    typed: bool,
}

impl Lane {
    fn index(self) -> u64 {
        let block = match self.block {
            Block::Ctu => 0,
            Block::Ctd => 1,
            Block::Ctud => 2,
        };
        block * 10 + self.ty as u64 + if self.typed { 0 } else { 5 }
    }
}

fn block_base(block: Block) -> &'static str {
    match block {
        Block::Ctu => "CTU",
        Block::Ctd => "CTD",
        Block::Ctud => "CTUD",
    }
}

fn block_name(lane: Lane) -> String {
    if lane.typed {
        format!("{}{}", block_base(lane.block), lane.ty.suffix())
    } else {
        block_base(lane.block).to_string()
    }
}

/// The Structured Text program for one lane. Outputs are `q_out` (CTU, CTD) or `qu_out` and
/// `qd_out` (CTUD), and `cv_out`. PV is `pv_sig`, of the lane's type.
fn program(lane: Lane) -> String {
    let name = block_name(lane);
    let ty = lane.ty.st_name();
    match lane.block {
        Block::Ctu => format!(
            "PROGRAM Main\n\
             VAR\n\
                 ctr : {name};\n\
                 cu_sig : BOOL;\n\
                 r_sig : BOOL;\n\
                 pv_sig : {ty};\n\
                 q_out : BOOL;\n\
                 cv_out : {ty};\n\
             END_VAR\n\
             ctr(CU := cu_sig, R := r_sig, PV := pv_sig, Q => q_out, CV => cv_out);\n\
             END_PROGRAM\n"
        ),
        Block::Ctd => format!(
            "PROGRAM Main\n\
             VAR\n\
                 ctr : {name};\n\
                 cd_sig : BOOL;\n\
                 ld_sig : BOOL;\n\
                 pv_sig : {ty};\n\
                 q_out : BOOL;\n\
                 cv_out : {ty};\n\
             END_VAR\n\
             ctr(CD := cd_sig, LD := ld_sig, PV := pv_sig, Q => q_out, CV => cv_out);\n\
             END_PROGRAM\n"
        ),
        Block::Ctud => format!(
            "PROGRAM Main\n\
             VAR\n\
                 ctr : {name};\n\
                 cu_sig : BOOL;\n\
                 cd_sig : BOOL;\n\
                 r_sig : BOOL;\n\
                 ld_sig : BOOL;\n\
                 pv_sig : {ty};\n\
                 qu_out : BOOL;\n\
                 qd_out : BOOL;\n\
                 cv_out : {ty};\n\
             END_VAR\n\
             ctr(CU := cu_sig, CD := cd_sig, R := r_sig, LD := ld_sig, PV := pv_sig, \
                 QU => qu_out, QD => qd_out, CV => cv_out);\n\
             END_PROGRAM\n"
        ),
    }
}

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

    fn below(&mut self, bound: u64) -> u64 {
        self.next_u64() % bound
    }

    fn per_mille(&mut self, chance: u64) -> bool {
        self.below(1000) < chance
    }

    /// Uniform value in the inclusive range `low..=high`.
    fn range(&mut self, low: i128, high: i128) -> i128 {
        let span = u128::try_from(high - low + 1).expect("non-empty range");
        low + i128::try_from(u128::from(self.next_u64()) % span).expect("span fits in i128")
    }
}

/// A preset value: the limits of the type and their neighbours, zero and small values, or a
/// uniform random value of the type.
fn next_pv(rng: &mut Rng, ty: Ty) -> i128 {
    let (min, max) = ty.limits();
    match rng.below(12) {
        0 => min,
        1 => min + 1,
        2 => 0,
        3 => 1,
        4 => 2,
        5 => 3,
        6 => 7,
        7 => max - 1,
        8 => max,
        9 => 10,
        _ => rng.range(min, max),
    }
}

/// A one-to-three-call pulse: starts with chance 4 per cent, then holds for `remaining` calls.
fn pulse(rng: &mut Rng, remaining: &mut u64) -> bool {
    if *remaining > 0 {
        *remaining -= 1;
        return true;
    }
    if rng.per_mille(40) {
        *remaining = rng.below(3);
        return true;
    }
    false
}

/// The input signals of one executed call. Unused signals of a block are ignored by it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Signals {
    cu: bool,
    cd: bool,
    r: bool,
    ld: bool,
    pv: i128,
}

/// Generates held and toggled count inputs, occasional reset and load pulses, and preset changes.
struct Driver {
    cu: bool,
    cd: bool,
    r_hold: u64,
    ld_hold: u64,
    pv: i128,
}

impl Driver {
    fn new(rng: &mut Rng, ty: Ty) -> Self {
        Self {
            cu: false,
            cd: false,
            r_hold: 0,
            ld_hold: 0,
            pv: next_pv(rng, ty),
        }
    }

    fn next(&mut self, rng: &mut Rng, ty: Ty) -> Signals {
        if rng.per_mille(350) {
            self.cu = !self.cu;
        }
        if rng.per_mille(350) {
            self.cd = !self.cd;
        }
        let r = pulse(rng, &mut self.r_hold);
        let ld = pulse(rng, &mut self.ld_hold);
        if rng.per_mille(80) {
            self.pv = next_pv(rng, ty);
        }
        Signals {
            cu: self.cu,
            cd: self.cd,
            r,
            ld,
            pv: self.pv,
        }
    }
}

/// Outputs of one executed call. CTU and CTD set `q`; CTUD sets `qu` and `qd`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Outputs {
    q: bool,
    qu: bool,
    qd: bool,
    cv: i128,
}

/// Counts how often each named boundary rule was exercised by the generated vectors.
#[derive(Default)]
struct Coverage(BTreeMap<&'static str, u64>);

impl Coverage {
    fn hit(&mut self, rule: &'static str) {
        *self.0.entry(rule).or_insert(0) += 1;
    }

    fn hits(&self, rule: &'static str) -> u64 {
        self.0.get(rule).copied().unwrap_or(0)
    }
}

/// Reference state of one counter instance, written from IEC Table 45 and spec 08 section 4.
#[derive(Clone, Copy, Debug)]
struct Model {
    block: Block,
    limits: (i128, i128),
    cv: i128,
    prev_cu: bool,
    prev_cd: bool,
}

impl Model {
    /// A fresh instance: CV = 0 and both edge memories FALSE.
    fn new(block: Block, ty: Ty) -> Self {
        Self::with_cv(block, ty, 0)
    }

    fn with_cv(block: Block, ty: Ty, cv: i128) -> Self {
        Self {
            block,
            limits: ty.limits(),
            cv,
            prev_cu: false,
            prev_cd: false,
        }
    }

    /// One executed call. Each count input is evaluated as a rising edge against its memory, and
    /// the memory is updated on every call, including calls where reset or load dominates.
    fn step(&mut self, input: Signals, coverage: &mut Coverage) -> Outputs {
        let (min, max) = self.limits;
        match self.block {
            Block::Ctu => {
                let edge = input.cu && !self.prev_cu;
                self.prev_cu = input.cu;
                if input.r {
                    if edge {
                        coverage.hit("ctu: reset dominates an edge");
                    }
                    self.cv = 0;
                } else if edge && self.cv < max {
                    self.cv += 1;
                    coverage.hit("counted");
                } else if edge {
                    coverage.hit("saturated at limit");
                }
                Outputs {
                    q: self.cv >= input.pv,
                    qu: false,
                    qd: false,
                    cv: self.cv,
                }
            }
            Block::Ctd => {
                let edge = input.cd && !self.prev_cd;
                self.prev_cd = input.cd;
                if input.ld {
                    if edge {
                        coverage.hit("ctd: load dominates an edge");
                    }
                    self.cv = input.pv;
                } else if edge && self.cv > min {
                    self.cv -= 1;
                    coverage.hit("counted");
                } else if edge {
                    coverage.hit("saturated at limit");
                }
                Outputs {
                    q: self.cv <= 0,
                    qu: false,
                    qd: false,
                    cv: self.cv,
                }
            }
            Block::Ctud => {
                let up = input.cu && !self.prev_cu;
                let down = input.cd && !self.prev_cd;
                self.prev_cu = input.cu;
                self.prev_cd = input.cd;
                if input.r {
                    if up || down {
                        coverage.hit("ctud: reset dominates an edge");
                    }
                    self.cv = 0;
                } else if input.ld {
                    if up || down {
                        coverage.hit("ctud: load dominates an edge");
                    }
                    self.cv = input.pv;
                } else if up && down {
                    // IF NOT (CU AND CD) THEN ... : simultaneous rising edges leave CV unchanged.
                    coverage.hit("ctud: simultaneous edges cancel");
                } else if up {
                    if self.cv < max {
                        self.cv += 1;
                        coverage.hit("counted");
                    } else {
                        coverage.hit("saturated at limit");
                    }
                } else if down {
                    if self.cv > min {
                        self.cv -= 1;
                        coverage.hit("counted");
                    } else {
                        coverage.hit("saturated at limit");
                    }
                }
                Outputs {
                    q: false,
                    qu: self.cv >= input.pv,
                    qd: self.cv <= 0,
                    cv: self.cv,
                }
            }
        }
    }
}

/// Rules that the vectors of one counter block must exercise at least `MIN_RULE_HITS` times.
fn required_rules(block: Block) -> Vec<&'static str> {
    let mut rules = vec!["counted", "saturated at limit"];
    rules.push(match block {
        Block::Ctu => "ctu: reset dominates an edge",
        Block::Ctd => "ctd: load dominates an edge",
        Block::Ctud => "ctud: reset dominates an edge",
    });
    if block == Block::Ctud {
        rules.push("ctud: load dominates an edge");
        rules.push("ctud: simultaneous edges cancel");
    }
    rules
}

fn set_st_inputs(harness: &mut TestHarness, lane: Lane, signals: Signals) {
    harness.set_input("pv_sig", lane.ty.value(signals.pv));
    match lane.block {
        Block::Ctu => {
            harness.set_input("cu_sig", Value::Bool(signals.cu));
            harness.set_input("r_sig", Value::Bool(signals.r));
        }
        Block::Ctd => {
            harness.set_input("cd_sig", Value::Bool(signals.cd));
            harness.set_input("ld_sig", Value::Bool(signals.ld));
        }
        Block::Ctud => {
            harness.set_input("cu_sig", Value::Bool(signals.cu));
            harness.set_input("cd_sig", Value::Bool(signals.cd));
            harness.set_input("r_sig", Value::Bool(signals.r));
            harness.set_input("ld_sig", Value::Bool(signals.ld));
        }
    }
}

/// The Structured Text outputs against the model. Returns a description of the mismatch.
fn st_outputs_match(harness: &TestHarness, lane: Lane, expected: Outputs) -> Result<(), String> {
    let cv = harness.get_output("cv_out");
    let q = harness.get_output("q_out");
    let qu = harness.get_output("qu_out");
    let qd = harness.get_output("qd_out");
    let flags_match = match lane.block {
        Block::Ctu | Block::Ctd => q == Some(Value::Bool(expected.q)),
        Block::Ctud => qu == Some(Value::Bool(expected.qu)) && qd == Some(Value::Bool(expected.qd)),
    };
    if cv == Some(lane.ty.value(expected.cv)) && flags_match {
        Ok(())
    } else {
        Err(format!(
            "expected {expected:?}; runtime CV={cv:?}, Q={q:?}, QU={qu:?}, QD={qd:?}"
        ))
    }
}

/// Runs the seeded sequences of one typed or generic lane through TestHarness. Returns the number
/// of calls that agreed with the model. Mismatches are appended to `ledger`.
fn run_st_lane(lane: Lane, coverage: &mut Coverage, ledger: &mut Vec<String>) -> usize {
    let name = block_name(lane);
    let source = program(lane);
    let mut executed = 0;
    for sequence in 0..SEQUENCES_PER_LANE {
        let label = format!("{name} sequence {sequence}");
        let mut harness = match TestHarness::from_source(&source) {
            Ok(harness) => harness,
            Err(error) => {
                ledger.push(format!("{label} does not compile: {error}"));
                continue;
            }
        };
        let mut rng = Rng(0xC7A_0000_0000 + lane.index() * 1_000 + sequence);
        let mut driver = Driver::new(&mut rng, lane.ty);
        let mut model = Model::new(lane.block, lane.ty);
        for step in 0..STEPS_PER_SEQUENCE {
            let signals = driver.next(&mut rng, lane.ty);
            set_st_inputs(&mut harness, lane, signals);
            let result = harness.cycle();
            if !result.errors.is_empty() {
                ledger.push(format!(
                    "{label} step {step} with inputs {signals:?}: cycle errors {:?}",
                    result.errors
                ));
                break;
            }
            let expected = model.step(signals, coverage);
            if let Err(message) = st_outputs_match(&harness, lane, expected) {
                ledger.push(format!(
                    "{label} step {step} with inputs {signals:?}: {message}"
                ));
                break;
            }
            executed += 1;
        }
    }
    executed
}

/// The inputs of a block as named variables of a native instance.
fn storage_inputs(block: Block, ty: Ty, signals: Signals) -> Vec<(&'static str, Value)> {
    let pv = ("PV", ty.value(signals.pv));
    match block {
        Block::Ctu => vec![
            ("CU", Value::Bool(signals.cu)),
            ("R", Value::Bool(signals.r)),
            pv,
        ],
        Block::Ctd => vec![
            ("CD", Value::Bool(signals.cd)),
            ("LD", Value::Bool(signals.ld)),
            pv,
        ],
        Block::Ctud => vec![
            ("CU", Value::Bool(signals.cu)),
            ("CD", Value::Bool(signals.cd)),
            ("R", Value::Bool(signals.r)),
            ("LD", Value::Bool(signals.ld)),
            pv,
        ],
    }
}

/// Runs the native counter from a CV near the limit it counts toward, so saturation is reached
/// quickly for every declared type. CTD and every second CTUD sequence count down. Mismatches are
/// appended to `ledger`.
fn run_storage_lane(
    block: Block,
    ty: Ty,
    coverage: &mut Coverage,
    ledger: &mut Vec<String>,
) -> usize {
    let (min, max) = ty.limits();
    let kind = match block {
        Block::Ctu => BuiltinFbKind::Ctu,
        Block::Ctd => BuiltinFbKind::Ctd,
        Block::Ctud => BuiltinFbKind::Ctud,
    };
    let mut executed = 0;
    for sequence in 0..STORAGE_SEQUENCES {
        let counting_down = block == Block::Ctd || (block == Block::Ctud && sequence % 2 == 1);
        let preset = if counting_down { min + 2 } else { max - 2 };
        let label = format!(
            "{} native {} sequence {sequence} from CV {preset}",
            block_base(block),
            ty.st_name()
        );
        let mut rng = Rng(0x5704_0000_0000
            + Lane {
                block,
                ty,
                typed: true,
            }
            .index()
                * 100
            + sequence);
        let mut driver = Driver::new(&mut rng, ty);
        let mut model = Model::with_cv(block, ty, preset);
        let mut storage = VariableStorage::default();
        let instance = storage.create_instance(block_base(block));
        assert!(storage.set_instance_var(instance, "CV", ty.value(preset)));
        for step in 0..STORAGE_STEPS {
            let signals = driver.next(&mut rng, ty);
            for (name, value) in storage_inputs(block, ty, signals) {
                assert!(storage.set_instance_var(instance, name, value));
            }
            if let Err(error) =
                execute_builtin_in_storage(&mut storage, Duration::ZERO, instance, kind)
            {
                ledger.push(format!(
                    "{label} step {step} with inputs {signals:?}: execution failed: {error:?}"
                ));
                break;
            }
            let expected = model.step(signals, coverage);
            let cv_ok = storage.get_instance_var(instance, "CV") == Some(&ty.value(expected.cv));
            let flags_ok = match block {
                Block::Ctu | Block::Ctd => {
                    storage.get_instance_var(instance, "Q") == Some(&Value::Bool(expected.q))
                }
                Block::Ctud => {
                    storage.get_instance_var(instance, "QU") == Some(&Value::Bool(expected.qu))
                        && storage.get_instance_var(instance, "QD")
                            == Some(&Value::Bool(expected.qd))
                }
            };
            if !cv_ok || !flags_ok {
                ledger.push(format!(
                    "{label} step {step} with inputs {signals:?}: expected {expected:?}; \
                     runtime CV={:?}, Q={:?}, QU={:?}, QD={:?}",
                    storage.get_instance_var(instance, "CV"),
                    storage.get_instance_var(instance, "Q"),
                    storage.get_instance_var(instance, "QU"),
                    storage.get_instance_var(instance, "QD"),
                ));
                break;
            }
            executed += 1;
        }
    }
    executed
}

/// Runs every typed variant and the generic form of one block, through both paths.
fn run_block(block: Block) {
    let mut coverage = Coverage::default();
    let mut ledger = Vec::new();
    let mut executed = 0;
    for ty in Ty::ALL {
        executed += run_st_lane(
            Lane {
                block,
                ty,
                typed: true,
            },
            &mut coverage,
            &mut ledger,
        );
        executed += run_storage_lane(block, ty, &mut coverage, &mut ledger);
    }
    executed += run_st_lane(
        Lane {
            block,
            ty: Ty::Int,
            typed: false,
        },
        &mut coverage,
        &mut ledger,
    );
    assert!(
        ledger.is_empty(),
        "{block:?}: {} sequence(s) diverged from the reference model ({executed} calls agreed):\n{}",
        ledger.len(),
        ledger.join("\n")
    );
    assert!(
        executed >= 3_000,
        "{block:?}: only {executed} executed calls were generated"
    );
    for rule in required_rules(block) {
        assert!(
            coverage.hits(rule) >= MIN_RULE_HITS,
            "{block:?}: rule '{rule}' was exercised {} times",
            coverage.hits(rule)
        );
    }
}

#[test]
fn oracle_ctu_family_matches_reference_model() {
    run_block(Block::Ctu);
}

#[test]
fn oracle_ctd_family_matches_reference_model() {
    run_block(Block::Ctd);
}

#[test]
fn oracle_ctud_family_matches_reference_model() {
    run_block(Block::Ctud);
}

/// IEC Table 45 CU is an R_EDGE input, and spec 08 section 4 (15 October decision) says the edge
/// memory follows CU on a call whose reset dominates. A CU held high through a reset release is
/// not counted again. Expected CV after each call: 1, 0, 1, 0, 0.
#[test]
fn oracle_ctu_edge_memory_is_updated_on_reset_calls() {
    let mut harness = TestHarness::from_source(&program(Lane {
        block: Block::Ctu,
        ty: Ty::Int,
        typed: true,
    }))
    .unwrap_or_else(|error| panic!("CTU_INT must compile: {error}"));
    harness.set_input("pv_sig", Value::Int(2));
    // (CU, R, expected CV)
    let calls = [
        (true, false, 1),
        (false, true, 0),
        (true, false, 1),
        (true, true, 0),
        (true, false, 0),
    ];
    for (call, (cu, r, expected_cv)) in calls.into_iter().enumerate() {
        harness.set_input("cu_sig", Value::Bool(cu));
        harness.set_input("r_sig", Value::Bool(r));
        let result = harness.cycle();
        assert!(result.errors.is_empty(), "call {call}: {:?}", result.errors);
        harness.assert_eq("cv_out", Value::Int(expected_cv));
        harness.assert_eq("q_out", Value::Bool(false));
    }
}

/// CU is low on the reset call. The reset call still records CU in the edge memory, so the next
/// call with CU high is a new rising edge and counts. Expected CV: 1, 0, 1.
#[test]
fn oracle_ctu_reset_call_records_the_count_input_it_sees() {
    let mut harness = TestHarness::from_source(&program(Lane {
        block: Block::Ctu,
        ty: Ty::Dint,
        typed: true,
    }))
    .unwrap_or_else(|error| panic!("CTU_DINT must compile: {error}"));
    harness.set_input("pv_sig", Value::DInt(100));
    // (CU, R, expected CV)
    let calls = [(true, false, 1), (false, true, 0), (true, false, 1)];
    for (call, (cu, r, expected_cv)) in calls.into_iter().enumerate() {
        harness.set_input("cu_sig", Value::Bool(cu));
        harness.set_input("r_sig", Value::Bool(r));
        let result = harness.cycle();
        assert!(result.errors.is_empty(), "call {call}: {:?}", result.errors);
        harness.assert_eq("cv_out", Value::DInt(expected_cv));
    }
}

/// CTUD: a load while CU is held high leaves CV at PV, and the held CU is not counted again when
/// the load releases. Simultaneous rising edges cancel. Expected CV: 1, 5, 5, 5, 5, 5, 6.
#[test]
fn oracle_ctud_edge_memory_is_updated_on_load_and_edges_cancel() {
    let mut harness = TestHarness::from_source(&program(Lane {
        block: Block::Ctud,
        ty: Ty::Int,
        typed: true,
    }))
    .unwrap_or_else(|error| panic!("CTUD_INT must compile: {error}"));
    harness.set_input("pv_sig", Value::Int(5));
    // (CU, CD, R, LD, expected CV, expected QU)
    let calls = [
        (true, false, false, false, 1, false),
        (true, false, false, true, 5, true),
        (true, false, false, false, 5, true),
        (false, false, false, false, 5, true),
        (true, true, false, false, 5, true),
        (false, false, false, false, 5, true),
        (true, false, false, false, 6, true),
    ];
    for (call, (cu, cd, r, ld, expected_cv, expected_qu)) in calls.into_iter().enumerate() {
        harness.set_input("cu_sig", Value::Bool(cu));
        harness.set_input("cd_sig", Value::Bool(cd));
        harness.set_input("r_sig", Value::Bool(r));
        harness.set_input("ld_sig", Value::Bool(ld));
        let result = harness.cycle();
        assert!(result.errors.is_empty(), "call {call}: {:?}", result.errors);
        harness.assert_eq("cv_out", Value::Int(expected_cv));
        harness.assert_eq("qu_out", Value::Bool(expected_qu));
        harness.assert_eq("qd_out", Value::Bool(false));
    }
}

/// CTD: a load on the call where CD rises dominates that edge and leaves CV at PV. CD held high
/// after the load is not counted again. Expected CV: 3, 3, 3, 2 with Q FALSE throughout.
#[test]
fn oracle_ctd_edge_memory_is_updated_on_load() {
    let mut harness = TestHarness::from_source(&program(Lane {
        block: Block::Ctd,
        ty: Ty::Int,
        typed: true,
    }))
    .unwrap_or_else(|error| panic!("CTD_INT must compile: {error}"));
    // (CD, LD, PV, expected CV)
    let calls = [
        (true, true, 3, 3),
        (true, false, 3, 3),
        (false, false, 3, 3),
        (true, false, 3, 2),
    ];
    for (call, (cd, ld, pv, expected_cv)) in calls.into_iter().enumerate() {
        harness.set_input("cd_sig", Value::Bool(cd));
        harness.set_input("ld_sig", Value::Bool(ld));
        harness.set_input("pv_sig", Value::Int(pv));
        let result = harness.cycle();
        assert!(result.errors.is_empty(), "call {call}: {:?}", result.errors);
        harness.assert_eq("cv_out", Value::Int(expected_cv));
        harness.assert_eq("q_out", Value::Bool(false));
    }
}
