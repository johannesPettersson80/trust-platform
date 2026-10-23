//! Independent oracle for the standard timer function blocks TP, TON and TOF.
//!
//! Authority: IEC 61131-3 Ed.3 section 6.6.3.5.5, Table 46 and Figure 15 (the overloaded names
//! and the explicit *_TIME and *_LTIME names share one state machine, and the effect of a PT
//! change during timing is implementer specific), and specification 08 section 5 (scan-step
//! contract, TP, TON and TOF rules, runtime boundary contract). The scan-step policy recorded in
//! docs/IEC_DECISIONS.md (2026-07-12) is applied as written there: an executed call is one scan
//! step; PT is sampled on every executed call; the first call after creation or restart adds zero
//! elapsed time; a call adds the runtime clock advance since the previous executed call, together
//! with the inputs sampled on that call; a stationary or backward clock adds zero.
//!
//! The reference model below is written from those clauses and shares no code with the runtime.
//! Input vectors come from a fixed-seed generator. Q and ET are compared with the model after
//! every executed call, and the runtime clock is compared after every advance. A sequence stops at
//! its first mismatch, because the states have diverged from that call on. Every mismatch of every
//! sequence is collected and reported together.

use std::collections::BTreeMap;

use trust_runtime::harness::TestHarness;
use trust_runtime::value::{Duration, Value};
use trust_runtime::RestartMode;

const STEPS_PER_SEQUENCE: usize = 500;
const SEQUENCES_PER_LANE: u64 = 6;
const MIN_RULE_HITS: u64 = 5;

/// Runtime clock readings stay within +-CLOCK_LIMIT, so every advance and every elapsed
/// contribution is a representable nanosecond value and the runtime clock never saturates.
const CLOCK_LIMIT: i128 = (1 << 62) - 1;
/// Steps that would take the clock beyond +-CLOCK_HALF reverse direction, keeping it near zero.
const CLOCK_HALF: i128 = 1 << 61;

/// Preset values: zero and negative presets (normalised to zero), one nanosecond, milliseconds
/// and seconds, the TIME range limits, and a value near the top of the range.
const PT_SET: [i128; 13] = [
    0,
    0,
    1,
    1_000_000,
    10_000_000,
    1_000_000_000,
    3_000_000_000,
    -1,
    -10_000_000,
    i64::MIN as i128,
    i64::MAX as i128,
    i64::MAX as i128 - 1,
    1 << 62,
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    Tp,
    Ton,
    Tof,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Family {
    Time,
    LTime,
}

/// The overloaded base name (TP, TON, TOF) or the explicit family name (TP_TIME, TON_LTIME, ...).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Form {
    Overloaded,
    Explicit,
}

#[derive(Clone, Copy, Debug)]
struct Lane {
    kind: Kind,
    family: Family,
    form: Form,
}

impl Lane {
    fn index(self) -> u64 {
        self.kind as u64 * 4 + self.family as u64 * 2 + self.form as u64
    }
}

fn base_name(kind: Kind) -> &'static str {
    match kind {
        Kind::Tp => "TP",
        Kind::Ton => "TON",
        Kind::Tof => "TOF",
    }
}

fn block_name(kind: Kind, family: Family, form: Form) -> String {
    let base = base_name(kind);
    match (form, family) {
        (Form::Overloaded, _) => base.to_string(),
        (Form::Explicit, Family::Time) => format!("{base}_TIME"),
        (Form::Explicit, Family::LTime) => format!("{base}_LTIME"),
    }
}

fn family_type(family: Family) -> &'static str {
    match family {
        Family::Time => "TIME",
        Family::LTime => "LTIME",
    }
}

/// One timer instance with its PT input and its Q and ET outputs, as Structured Text.
fn program(block: &str, ty: &str) -> String {
    format!(
        "PROGRAM Main\n\
         VAR\n\
             timer_inst : {block};\n\
             in_sig : BOOL;\n\
             pt_sig : {ty};\n\
             q_out : BOOL;\n\
             et_out : {ty};\n\
         END_VAR\n\
         timer_inst(IN := in_sig, PT := pt_sig, Q => q_out, ET => et_out);\n\
         END_PROGRAM\n"
    )
}

fn to_i64(value: i128) -> i64 {
    i64::try_from(value).expect("value is within the nanosecond range")
}

fn duration_value(family: Family, nanos: i128) -> Value {
    let duration = Duration::from_nanos(to_i64(nanos));
    match family {
        Family::Time => Value::Time(duration),
        Family::LTime => Value::LTime(duration),
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

/// Picks the next PT: mostly from the boundary set, sometimes a small random value.
fn next_pt(rng: &mut Rng) -> i128 {
    if rng.per_mille(900) {
        PT_SET[usize::try_from(rng.below(PT_SET.len() as u64)).expect("index")]
    } else {
        rng.range(-1_000_000, 100_000_000)
    }
}

/// Picks the next clock step from the boundary categories of the brief, clamped so the runtime
/// clock stays inside +-CLOCK_LIMIT.
fn next_dt(rng: &mut Rng, clock: i128, pt: i128) -> i128 {
    let step = match rng.below(100) {
        0..=9 => 0,
        10..=39 => rng.range(1, 5_000_000),
        40..=49 => pt,
        50..=59 => pt - 1,
        60..=69 => pt + 1,
        70..=79 => rng.range(1, 50_000_000),
        80..=84 => -rng.range(1, 5_000_000),
        85..=89 => -pt,
        _ => rng.range(CLOCK_HALF, CLOCK_LIMIT),
    };
    let step = if (clock + step).abs() > CLOCK_HALF {
        -step
    } else {
        step
    };
    step.clamp(-CLOCK_LIMIT - clock, CLOCK_LIMIT - clock)
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

/// The phase of a timer interval (IEC Figure 15 and spec 08 section 5).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Phase {
    /// Nothing is timing: no interval was armed since creation or restart.
    Idle,
    /// An interval is timing: a TP pulse, or a TOF off-delay after a high sample.
    Running,
    /// The interval completed: ET holds the threshold that was reached.
    Expired,
}

/// Reference state of one timer instance, written from IEC Table 46, Figure 15 and spec 08
/// section 5. The runtime is not consulted.
#[derive(Clone, Copy, Debug)]
struct Model {
    kind: Kind,
    /// Runtime clock of the previous executed call; `None` before the first call and after restart.
    baseline: Option<i128>,
    phase: Phase,
    prev_in: bool,
    q: bool,
    et: i128,
}

impl Model {
    fn new(kind: Kind) -> Self {
        Self {
            kind,
            baseline: None,
            phase: Phase::Idle,
            prev_in: false,
            q: false,
            et: 0,
        }
    }

    fn restart(&mut self) {
        *self = Self::new(self.kind);
    }

    /// One executed call at runtime clock `now`, with IN and PT sampled on that call. Returns the
    /// expected Q and ET.
    fn step(
        &mut self,
        now: i128,
        input: bool,
        pt_raw: i128,
        coverage: &mut Coverage,
    ) -> (bool, i128) {
        let elapsed = match self.baseline {
            None => 0,
            Some(previous) => {
                if now < previous {
                    coverage.hit("clock: backward");
                } else if now == previous {
                    coverage.hit("clock: stationary");
                }
                (now - previous).max(0)
            }
        };
        self.baseline = Some(now);
        // PT <= T#0s is zero for TP, TON and TOF (spec 08 section 5, IEC_DECISIONS 2026-07-12).
        let pt = pt_raw.max(0);
        let rising = input && !self.prev_in;
        match self.kind {
            Kind::Ton => self.ton(input, pt, elapsed, coverage),
            Kind::Tof => self.tof(input, pt, elapsed, coverage),
            Kind::Tp => self.tp(input, rising, pt, elapsed, coverage),
        }
        self.prev_in = input;
        (self.q, self.et)
    }

    /// Adds this call's elapsed contribution to ET, saturating at PT, and reports whether PT has
    /// been reached on this call.
    fn accumulate(&mut self, elapsed: i128, pt: i128, coverage: &mut Coverage) -> bool {
        if self.et + elapsed > pt {
            coverage.hit("elapsed saturates at PT");
        }
        self.et = (self.et + elapsed).min(pt);
        self.et >= pt
    }

    /// TON (Figure 15b): Q follows IN after IN has been high for PT; ET resets when IN is low.
    fn ton(&mut self, input: bool, pt: i128, elapsed: i128, coverage: &mut Coverage) {
        if input {
            let was_done = self.q;
            let reached = self.accumulate(elapsed, pt, coverage);
            if was_done && !reached {
                coverage.hit("ton: Q recomputed after PT increase");
            }
            self.q = reached;
        } else {
            if self.et > 0 {
                coverage.hit("ton: IN low resets ET");
            }
            self.et = 0;
            self.q = false;
        }
    }

    /// TOF (Figure 15c): a high sample sets Q and resets ET; after a falling sample Q stays TRUE
    /// while ET runs to PT; at expiry ET holds PT until the next high sample.
    fn tof(&mut self, input: bool, pt: i128, elapsed: i128, coverage: &mut Coverage) {
        if input {
            if self.phase != Phase::Idle {
                coverage.hit("tof: IN high rearms");
            }
            self.phase = Phase::Running;
            self.q = true;
            self.et = 0;
            return;
        }
        match self.phase {
            Phase::Idle => {
                self.q = false;
                self.et = 0;
            }
            Phase::Running => {
                if self.accumulate(elapsed, pt, coverage) {
                    coverage.hit("tof: expires");
                    self.phase = Phase::Expired;
                    self.q = false;
                } else {
                    self.q = true;
                }
            }
            Phase::Expired => {
                if pt != self.et {
                    coverage.hit("tof: ET held under PT change");
                }
                self.q = false;
            }
        }
    }

    /// TP (Figure 15a): a sampled rising edge starts a pulse of length PT; a falling input does not
    /// cancel it; a new rising edge restarts it. Once it has expired, Q stays FALSE and ET stays
    /// at PT. The first later sampled-low call resets ET to zero and rearms the edge (spec 08
    /// section 5). A pulse that expires on a call where IN is low therefore still shows ET = PT on
    /// that call; the reset is on a later call.
    fn tp(&mut self, input: bool, rising: bool, pt: i128, elapsed: i128, coverage: &mut Coverage) {
        if rising {
            if self.phase == Phase::Running {
                coverage.hit("tp: retrigger while pulsing");
            }
            self.et = 0;
            if pt == 0 {
                // A zero preset has no positive-duration pulse, so Q stays FALSE.
                coverage.hit("tp: zero preset rising");
                self.phase = Phase::Expired;
                self.q = false;
            } else {
                coverage.hit("tp: rising starts pulse");
                self.phase = Phase::Running;
                self.pulse(input, pt, elapsed, coverage);
            }
            return;
        }
        match self.phase {
            Phase::Idle => {
                self.q = false;
                self.et = 0;
            }
            Phase::Running => {
                if !input {
                    coverage.hit("tp: falling does not cancel");
                }
                self.pulse(input, pt, elapsed, coverage);
            }
            Phase::Expired => {
                self.q = false;
                if !input {
                    coverage.hit("tp: rearm on low");
                    self.et = 0;
                    self.phase = Phase::Idle;
                }
            }
        }
    }

    fn pulse(&mut self, input: bool, pt: i128, elapsed: i128, coverage: &mut Coverage) {
        if self.accumulate(elapsed, pt, coverage) {
            coverage.hit(if input {
                "tp: expires while IN high"
            } else {
                "tp: expires while IN low"
            });
            self.phase = Phase::Expired;
            self.q = false;
        } else {
            self.q = true;
        }
    }
}

/// Rules that the random vectors of one timer block must exercise at least `MIN_RULE_HITS` times.
fn required_rules(kind: Kind) -> Vec<&'static str> {
    let mut rules = vec![
        "clock: backward",
        "clock: stationary",
        "elapsed saturates at PT",
        "restart",
    ];
    rules.extend(match kind {
        Kind::Tp => vec![
            "tp: rising starts pulse",
            "tp: retrigger while pulsing",
            "tp: falling does not cancel",
            "tp: expires while IN high",
            "tp: expires while IN low",
            "tp: zero preset rising",
            "tp: rearm on low",
        ],
        Kind::Ton => vec![
            "ton: IN low resets ET",
            "ton: Q recomputed after PT increase",
        ],
        Kind::Tof => vec![
            "tof: IN high rearms",
            "tof: expires",
            "tof: ET held under PT change",
        ],
    });
    rules
}

/// One generated sequence: a fresh runtime, the reference model and the generator state.
struct Sequence {
    lane: Lane,
    label: String,
    harness: TestHarness,
    model: Model,
    rng: Rng,
    clock: i128,
    pt: i128,
    input: bool,
}

impl Sequence {
    fn start(lane: Lane, sequence: u64, source: &str, label: String) -> Result<Self, String> {
        let harness = TestHarness::from_source(source)
            .map_err(|error| format!("{label} does not compile: {error}"))?;
        let mut rng = Rng(0xA27_0000_0000 + lane.index() * 1_000 + sequence);
        let pt = next_pt(&mut rng);
        Ok(Self {
            lane,
            label,
            harness,
            model: Model::new(lane.kind),
            rng,
            clock: 0,
            pt,
            input: false,
        })
    }

    /// One executed call: advance the clock, maybe restart, drive IN and PT, cycle, and compare.
    /// Returns the first mismatch of this call.
    fn step(&mut self, step: usize, coverage: &mut Coverage) -> Result<(), String> {
        let context = format!("{} step {step}", self.label);
        if self.rng.per_mille(300) {
            self.input = !self.input;
        }
        if self.rng.per_mille(200) {
            self.pt = next_pt(&mut self.rng);
        }
        let dt = next_dt(&mut self.rng, self.clock, self.pt);
        self.clock += dt;
        self.harness.advance_time(Duration::from_nanos(to_i64(dt)));
        let runtime_clock = self.harness.current_time();
        let expected_clock = Duration::from_nanos(to_i64(self.clock));
        if runtime_clock != expected_clock {
            return Err(format!(
                "{context}: runtime clock {runtime_clock:?}, expected {expected_clock:?}"
            ));
        }
        if self.rng.per_mille(5) {
            let mode = if self.rng.per_mille(500) {
                RestartMode::Warm
            } else {
                RestartMode::Cold
            };
            self.harness
                .restart(mode)
                .map_err(|error| format!("{context}: restart failed: {error:?}"))?;
            self.model.restart();
            coverage.hit("restart");
        }
        self.harness.set_input("in_sig", Value::Bool(self.input));
        self.harness
            .set_input("pt_sig", duration_value(self.lane.family, self.pt));
        let result = self.harness.cycle();
        if !result.errors.is_empty() {
            return Err(format!(
                "{context}: cycle errors {:?} with IN={}, PT={} ns",
                result.errors, self.input, self.pt
            ));
        }
        let (expected_q, expected_et) = self.model.step(self.clock, self.input, self.pt, coverage);
        let q = self.harness.get_output("q_out");
        let et = self.harness.get_output("et_out");
        if q != Some(Value::Bool(expected_q))
            || et != Some(duration_value(self.lane.family, expected_et))
        {
            return Err(format!(
                "{context}: IN={}, PT={} ns, clock={} ns: expected Q={expected_q}, \
                 ET={expected_et} ns; runtime Q={q:?}, ET={et:?}",
                self.input, self.pt, self.clock
            ));
        }
        Ok(())
    }
}

/// Runs the seeded sequences of one lane through the runtime and the model. Returns the number of
/// executed calls that agreed with the model. Mismatches are appended to `ledger`.
fn run_lane(lane: Lane, coverage: &mut Coverage, ledger: &mut Vec<String>) -> usize {
    let block = block_name(lane.kind, lane.family, lane.form);
    let source = program(&block, family_type(lane.family));
    let mut executed = 0;
    for sequence in 0..SEQUENCES_PER_LANE {
        let label = format!("{block} ({lane:?}) sequence {sequence}");
        let mut run = match Sequence::start(lane, sequence, &source, label) {
            Ok(run) => run,
            Err(message) => {
                ledger.push(message);
                continue;
            }
        };
        for step in 0..STEPS_PER_SEQUENCE {
            if let Err(message) = run.step(step, coverage) {
                ledger.push(message);
                break;
            }
            executed += 1;
        }
    }
    executed
}

/// Runs all four TIME and LTIME, explicit and overloaded lanes of one block kind.
fn run_kind(kind: Kind) {
    let mut coverage = Coverage::default();
    let mut ledger = Vec::new();
    let mut executed = 0;
    for (family, form) in [
        (Family::Time, Form::Explicit),
        (Family::LTime, Form::Explicit),
        (Family::Time, Form::Overloaded),
        (Family::LTime, Form::Overloaded),
    ] {
        executed += run_lane(Lane { kind, family, form }, &mut coverage, &mut ledger);
    }
    assert!(
        ledger.is_empty(),
        "{kind:?}: {} sequence(s) diverged from the reference model ({executed} calls agreed):\n{}",
        ledger.len(),
        ledger.join("\n")
    );
    assert!(
        executed >= 3_000,
        "{kind:?}: only {executed} executed calls were generated"
    );
    for rule in required_rules(kind) {
        assert!(
            coverage.hits(rule) >= MIN_RULE_HITS,
            "{kind:?}: rule '{rule}' was exercised {} times",
            coverage.hits(rule)
        );
    }
}

#[test]
fn oracle_tp_matches_reference_model_for_time_and_ltime() {
    run_kind(Kind::Tp);
}

#[test]
fn oracle_ton_matches_reference_model_for_time_and_ltime() {
    run_kind(Kind::Ton);
}

#[test]
fn oracle_tof_matches_reference_model_for_time_and_ltime() {
    run_kind(Kind::Tof);
}

/// With PT <= T#0s the blocks have no positive-duration interval. Each row is one call: the
/// milliseconds advanced before the call, and IN on the call. The expected values are worked out
/// from the rules: TON completes on a high call, TOF expires on the first low call, TP never pulses.
#[test]
fn oracle_zero_and_negative_preset_boundaries() {
    const CALLS: [(i64, bool); 6] = [
        (0, false),
        (5, true),
        (5, true),
        (5, false),
        (5, false),
        (5, true),
    ];
    const TON_AND_TOF: [(bool, i64); 6] = [
        (false, 0),
        (true, 0),
        (true, 0),
        (false, 0),
        (false, 0),
        (true, 0),
    ];
    const TP: [(bool, i64); 6] = [(false, 0); 6];
    let mut ledger = Vec::new();
    for kind in [Kind::Tp, Kind::Ton, Kind::Tof] {
        let expected = if kind == Kind::Tp { TP } else { TON_AND_TOF };
        for pt_ms in [0_i64, -5] {
            let block = block_name(kind, Family::Time, Form::Explicit);
            let label = format!("{block} PT={pt_ms} ms");
            let mut harness = match TestHarness::from_source(&program(&block, "TIME")) {
                Ok(harness) => harness,
                Err(error) => {
                    ledger.push(format!("{label} does not compile: {error}"));
                    continue;
                }
            };
            harness.set_input("pt_sig", Value::Time(Duration::from_millis(pt_ms)));
            for call in 0..CALLS.len() {
                let (advance_ms, input) = CALLS[call];
                let (expected_q, expected_et_ms) = expected[call];
                harness.advance_time(Duration::from_millis(advance_ms));
                harness.set_input("in_sig", Value::Bool(input));
                let result = harness.cycle();
                let q = harness.get_output("q_out");
                let et = harness.get_output("et_out");
                if !result.errors.is_empty()
                    || q != Some(Value::Bool(expected_q))
                    || et != Some(Value::Time(Duration::from_millis(expected_et_ms)))
                {
                    ledger.push(format!(
                        "{label} call {call} (IN={input}): expected Q={expected_q}, \
                         ET={expected_et_ms} ms; runtime Q={q:?}, ET={et:?}, errors={:?}",
                        result.errors
                    ));
                    break;
                }
            }
        }
    }
    assert!(ledger.is_empty(), "{}", ledger.join("\n"));
}

/// Elapsed time saturates at PT without wrapping, even when one contribution is almost the whole
/// i64 nanosecond range. A wrapping sum would make ET negative. A backward clock adds nothing and
/// does not reduce ET (spec 08 section 5, runtime boundary contract).
#[test]
fn oracle_elapsed_saturates_at_pt_without_wrap_near_the_range_limit() {
    let mut harness = TestHarness::from_source(&program("TON_TIME", "TIME"))
        .unwrap_or_else(|error| panic!("TON_TIME must compile: {error}"));
    harness.set_input("in_sig", Value::Bool(true));
    harness.set_input("pt_sig", Value::Time(Duration::from_millis(100)));
    // (clock advance in ns, expected Q, expected ET in ns)
    let steps: [(i64, bool, i64); 5] = [
        (-(1 << 62), false, 0),
        (5, false, 5),
        (i64::MAX - 1, true, 100_000_000),
        (-(i64::MAX - 1), true, 100_000_000),
        (1_000_000, true, 100_000_000),
    ];
    let mut clock: i64 = 0;
    for (index, (advance, expected_q, expected_et)) in steps.into_iter().enumerate() {
        harness.advance_time(Duration::from_nanos(advance));
        clock += advance;
        assert_eq!(
            harness.current_time(),
            Duration::from_nanos(clock),
            "step {index}: runtime clock"
        );
        let result = harness.cycle();
        assert!(
            result.errors.is_empty(),
            "step {index}: {:?}",
            result.errors
        );
        assert_eq!(
            harness.get_output("q_out"),
            Some(Value::Bool(expected_q)),
            "step {index}: Q at clock {clock} ns"
        );
        assert_eq!(
            harness.get_output("et_out"),
            Some(Value::Time(Duration::from_nanos(expected_et))),
            "step {index}: ET at clock {clock} ns"
        );
    }
}
