//! The SFC companion executed (`sfc-profile.md`, "Execution of a visual SFC"): each chart's
//! companion and runtime entry, as `trust-lsp.visualCompanion` answers them, compiled and scanned
//! on the runtime at the entry's 20 ms. Every expected value is worked out by hand, scan by scan,
//! from IEC 61131-3 §6.7 as `sfc-profile.md` states it: in a scan the actions run with the steps
//! active at its start, then the transitions clear and the steps evolve, so a step entered in one
//! scan runs its actions in the next. A timer of an action is called in every scan, and a call
//! with `IN` TRUE adds the time since the previous call (specification 08, TON;
//! `docs/IEC_DECISIONS.md` 2026-07-12; `crates/trust-runtime/tests/fb_timers_full.rs`): switched
//! on in scan k, a TON has 20 ms in scan k and reaches a `PT` of n × 20 ms in scan k + n − 1.
//!
//! The tests need the toolchain of specification 44 §4.6 at `TRUST_TOOLCHAIN`.

use trust_runtime::harness::TestHarness;
use trust_runtime::value::{Duration, Value};

/// A chart's companion and runtime entry, compiled.
struct Run {
    harness: TestHarness,
    instance: String,
}

impl Run {
    /// The chart `chart` saved as `<base>.sfc.json`.
    fn new(base: &str, chart: &str) -> Self {
        let files = trust_visual::companion::companion(&format!("{base}.sfc.json"), chart)
            .unwrap_or_else(|error| panic!("a companion: {error:?}"));
        let harness = TestHarness::from_sources(&[
            files.companion_text.as_str(),
            files.runtime_entry_text.as_str(),
        ])
        .unwrap_or_else(|error| {
            panic!(
                "the companion compiles: {error}\n{}\n{}",
                files.companion_text, files.runtime_entry_text
            )
        });
        Self {
            harness,
            instance: format!("fb_{base}"),
        }
    }

    /// One activation of the runtime entry's 20 ms task.
    fn scan(&mut self) {
        self.harness.advance_time(Duration::from_millis(20));
        let result = self.harness.cycle();
        assert!(result.errors.is_empty(), "{:?}", result.errors);
    }

    fn value(&self, name: &str) -> Value {
        let path = format!("{}.{name}", self.instance);
        self.harness
            .try_get_output(&path)
            .unwrap_or_else(|error| panic!("{path}: {error:?}"))
    }

    fn flag(&self, name: &str) -> bool {
        match self.value(name) {
            Value::Bool(value) => value,
            other => panic!("{name} is {other:?}"),
        }
    }

    fn count(&self, name: &str) -> i32 {
        match self.value(name) {
            Value::DInt(value) => value,
            other => panic!("{name} is {other:?}"),
        }
    }

    /// The steps of `steps` that are active.
    fn active<'s>(&self, steps: &[&'s str]) -> Vec<&'s str> {
        steps
            .iter()
            .copied()
            .filter(|step| self.flag(&format!("{step}_active")))
            .collect()
    }

    fn input(&mut self, address: &str, value: bool) {
        self.harness
            .set_direct_input(address, Value::Bool(value))
            .unwrap_or_else(|error| panic!("{address}: {error:?}"));
    }
}

const PARALLEL: &str = r#"{
  "name": "Par",
  "steps": [
    { "id": "start", "name": "Start", "initial": true },
    { "id": "a", "name": "A" }, { "id": "a2", "name": "A2" },
    { "id": "b", "name": "B" }, { "id": "b2", "name": "B2" },
    { "id": "done", "name": "Done" }
  ],
  "transitions": [
    { "id": "t0", "condition": "%IX0.0", "sourceStepId": "start", "targetStepId": "sp" },
    { "id": "t1", "condition": "TRUE", "sourceStepId": "sp", "targetStepId": "a" },
    { "id": "t2", "condition": "TRUE", "sourceStepId": "sp", "targetStepId": "b" },
    { "id": "t3", "condition": "%IX0.4", "sourceStepId": "a", "targetStepId": "a2" },
    { "id": "t4", "condition": "%IX0.3", "sourceStepId": "b", "targetStepId": "b2" },
    { "id": "t5", "condition": "%IX0.1", "sourceStepId": "a2", "targetStepId": "jn" },
    { "id": "t6", "condition": "TRUE", "sourceStepId": "b2", "targetStepId": "jn" },
    { "id": "t7", "condition": "%IX0.2", "sourceStepId": "jn", "targetStepId": "done" }
  ],
  "parallelSplits": [ { "id": "sp", "name": "Split", "branchIds": ["a", "b"] } ],
  "parallelJoins": [ { "id": "jn", "name": "Join", "branchIds": ["a2", "b2"], "nextStepId": "done" } ]
}"#;

#[test]
fn parallel_branches_start_together_and_join_when_every_branch_has_finished() {
    // Table 61 4a and 4c. Scan 1: Start waits for %IX0.0. Scan 2: the split's transition clears
    // and enters A and B together. Scan 3: B moves to B2. Scan 4: the join's conditions hold but
    // A2 is not active, so it waits. Scan 5: A moves to A2; the join was tested against the steps
    // active before this evolution (A2 not yet), so it waits. Scan 6: both last steps are active
    // but A2's link condition %IX0.1 is FALSE. Scan 7: the join clears and enters Done.
    // Catches the old companion (Start was never left), branches entered one at a time, a join
    // that fires when any branch arrives or ignores a branch's condition, and two evolutions in
    // one scan.
    let steps = ["Start", "A", "A2", "B", "B2", "Done"];
    let mut run = Run::new("par", PARALLEL);
    run.scan();
    assert_eq!(run.active(&steps), ["Start"], "scan 1");
    run.input("%IX0.0", true);
    run.scan();
    assert_eq!(run.active(&steps), ["A", "B"], "scan 2");
    run.input("%IX0.3", true);
    run.scan();
    assert_eq!(run.active(&steps), ["A", "B2"], "scan 3");
    run.input("%IX0.2", true);
    run.scan();
    assert_eq!(run.active(&steps), ["A", "B2"], "scan 4");
    run.input("%IX0.4", true);
    run.scan();
    assert_eq!(run.active(&steps), ["A2", "B2"], "scan 5");
    run.scan();
    assert_eq!(run.active(&steps), ["A2", "B2"], "scan 6");
    run.input("%IX0.1", true);
    run.scan();
    assert_eq!(run.active(&steps), ["Done"], "scan 7");
}

#[test]
fn transitions_that_can_clear_together_clear_in_the_same_scan() {
    // §6.7.5: A -> A2 and B -> B2 share the input %IX0.0 and clear in one scan; the join then
    // returns to Start, whose split enters A and B again. Scan 1: {A, B}; scan 2: {A, B};
    // scan 3: {A2, B2}; scan 4: {Start}; scan 5: {A, B}. Catches a companion that clears one
    // transition per scan, and a join or split that does not lead back into a loop.
    let chart = r#"{
      "name": "Loop",
      "steps": [
        { "id": "start", "name": "Start", "initial": true },
        { "id": "a", "name": "A" }, { "id": "a2", "name": "A2" },
        { "id": "b", "name": "B" }, { "id": "b2", "name": "B2" }
      ],
      "transitions": [
        { "id": "t0", "condition": "TRUE", "sourceStepId": "start", "targetStepId": "sp" },
        { "id": "t1", "condition": "TRUE", "sourceStepId": "sp", "targetStepId": "a" },
        { "id": "t2", "condition": "TRUE", "sourceStepId": "sp", "targetStepId": "b" },
        { "id": "t3", "condition": "%IX0.0", "sourceStepId": "a", "targetStepId": "a2" },
        { "id": "t4", "condition": "%IX0.0", "sourceStepId": "b", "targetStepId": "b2" },
        { "id": "t5", "condition": "TRUE", "sourceStepId": "a2", "targetStepId": "jn" },
        { "id": "t6", "condition": "TRUE", "sourceStepId": "b2", "targetStepId": "jn" },
        { "id": "t7", "condition": "TRUE", "sourceStepId": "jn", "targetStepId": "start" }
      ],
      "parallelSplits": [ { "id": "sp", "name": "Split", "branchIds": ["a", "b"] } ],
      "parallelJoins": [ { "id": "jn", "name": "Join", "branchIds": ["a2", "b2"] } ]
    }"#;
    let steps = ["Start", "A", "A2", "B", "B2"];
    let mut run = Run::new("loop", chart);
    let mut seen = Vec::new();
    for scan in 1..=5 {
        if scan == 3 {
            run.input("%IX0.0", true);
        }
        run.scan();
        seen.push(run.active(&steps));
    }
    assert_eq!(
        seen,
        [
            vec!["A", "B"],
            vec!["A", "B"],
            vec!["A2", "B2"],
            vec!["Start"],
            vec!["A", "B"]
        ]
    );
}

#[test]
fn a_selection_takes_the_numbered_transition_then_the_one_drawn_first() {
    // Table 61 2a and 2b. S -> X (drawn first), S -> Y (priority 1) and S -> Z; X, Y and Z return
    // to S on %IX0.3. Scan 1 with X's and Z's conditions TRUE: S -> X wins (drawn before Z), and Z
    // stays inactive. Scan 2: X returns to S. Scan 3 with all three conditions TRUE: the numbered
    // S -> Y wins. Catches a selection that enters two steps at once and a priority ignored.
    let chart = r#"{
      "name": "Sel",
      "steps": [
        { "id": "s", "name": "S", "initial": true }, { "id": "x", "name": "X" },
        { "id": "y", "name": "Y" }, { "id": "z", "name": "Z" }
      ],
      "transitions": [
        { "id": "e0", "condition": "%IX0.0", "sourceStepId": "s", "targetStepId": "x" },
        { "id": "e1", "condition": "%IX0.1", "sourceStepId": "s", "targetStepId": "y", "priority": 1 },
        { "id": "e2", "condition": "%IX0.2", "sourceStepId": "s", "targetStepId": "z" },
        { "id": "e3", "condition": "%IX0.3", "sourceStepId": "x", "targetStepId": "s" },
        { "id": "e4", "condition": "%IX0.3", "sourceStepId": "y", "targetStepId": "s" },
        { "id": "e5", "condition": "%IX0.3", "sourceStepId": "z", "targetStepId": "s" }
      ]
    }"#;
    let steps = ["S", "X", "Y", "Z"];
    let mut run = Run::new("sel", chart);
    run.input("%IX0.0", true);
    run.input("%IX0.2", true);
    run.scan();
    assert_eq!(run.active(&steps), ["X"], "scan 1");
    run.input("%IX0.3", true);
    run.scan();
    assert_eq!(run.active(&steps), ["S"], "scan 2");
    run.input("%IX0.3", false);
    run.input("%IX0.1", true);
    run.scan();
    assert_eq!(run.active(&steps), ["Y"], "scan 3");
}

const ACTIONS: &str = r#"{
  "name": "Acts",
  "steps": [
    { "id": "s0", "name": "S0", "initial": true },
    { "id": "s1", "name": "S1", "actions": [
      { "id": "n", "name": "ActN", "qualifier": "N", "body": "n_runs := n_runs + 1;\nn_q := ActN.Q;" },
      { "id": "s", "name": "ActS", "qualifier": "S", "body": "s_runs := s_runs + 1;\ns_q := ActS.Q;" },
      { "id": "p", "name": "ActP", "qualifier": "P", "body": "p_runs := p_runs + 1;\np_q := ActP.Q;" },
      { "id": "p1", "name": "ActP1", "qualifier": "P1", "body": "p1_runs := p1_runs + 1;\np1_q := ActP1.Q;" },
      { "id": "p0", "name": "ActP0", "qualifier": "P0", "body": "p0_runs := p0_runs + 1;" }
    ] },
    { "id": "s2", "name": "S2" },
    { "id": "s3", "name": "S3", "actions": [ { "id": "r", "name": "ActS", "qualifier": "R" } ] }
  ],
  "transitions": [
    { "id": "t1", "condition": "%IX0.0", "sourceStepId": "s0", "targetStepId": "s1" },
    { "id": "t2", "condition": "%IX0.1", "sourceStepId": "s1", "targetStepId": "s2" },
    { "id": "t3", "condition": "%IX0.2", "sourceStepId": "s2", "targetStepId": "s3" }
  ],
  "variables": [
    { "name": "n_runs", "type": "DINT", "initialValue": "0" },
    { "name": "s_runs", "type": "DINT", "initialValue": "0" },
    { "name": "p_runs", "type": "DINT", "initialValue": "0" },
    { "name": "p1_runs", "type": "DINT", "initialValue": "0" },
    { "name": "p0_runs", "type": "DINT", "initialValue": "0" },
    { "name": "n_q", "type": "BOOL" },
    { "name": "s_q", "type": "BOOL" },
    { "name": "p_q", "type": "BOOL" },
    { "name": "p1_q", "type": "BOOL" }
  ]
}"#;

#[test]
fn actions_run_by_their_qualifiers_with_a_final_scan() {
    // Table 59 and Figure 23 a), counting the runs of each action's statements after each scan
    // (N, S, P, P1, P0). Scans 1-2: S1 is not yet active and nothing runs (no final scan or P0 at
    // the start). S1 is active in scans 3-5: N and S run each scan; P runs in scan 3 with Q TRUE
    // and in scan 4 with Q FALSE (its final scan); P1 runs once in scan 3 with Q FALSE. Scan 6
    // (S2): N runs its final scan with Q FALSE, S keeps running (stored) and P0 runs once. S3's R
    // resets S in scan 9, which is S's final scan, with Q FALSE. Catches every qualifier run as
    // the old N (only while the step is active), a missing final scan, P1 or P0 that run twice,
    // and an action that runs once at start-up.
    let mut run = Run::new("acts", ACTIONS);
    let counts = |run: &Run| {
        [
            run.count("n_runs"),
            run.count("s_runs"),
            run.count("p_runs"),
            run.count("p1_runs"),
            run.count("p0_runs"),
        ]
    };
    let mut seen = Vec::new();
    for scan in 1..=10 {
        match scan {
            2 => run.input("%IX0.0", true),
            5 => run.input("%IX0.1", true),
            8 => run.input("%IX0.2", true),
            _ => {}
        }
        run.scan();
        seen.push(counts(&run));
        match scan {
            3 => {
                assert!(
                    run.flag("n_q") && run.flag("s_q") && run.flag("p_q"),
                    "scan 3"
                );
                assert!(!run.flag("p1_q"), "P1 runs with Q FALSE");
            }
            4 => assert!(!run.flag("p_q"), "P's final scan sees Q FALSE"),
            6 => assert!(!run.flag("n_q"), "N's final scan sees Q FALSE"),
            7 => assert!(run.flag("ActS.Q"), "S is stored after its step"),
            9 => assert!(!run.flag("s_q") && !run.flag("ActS.Q"), "R resets S"),
            _ => {}
        }
    }
    assert_eq!(
        seen,
        [
            [0, 0, 0, 0, 0],
            [0, 0, 0, 0, 0],
            [1, 1, 1, 1, 0],
            [2, 2, 2, 1, 0],
            [3, 3, 2, 1, 0],
            [4, 4, 2, 1, 1],
            [4, 5, 2, 1, 1],
            [4, 6, 2, 1, 1],
            [4, 7, 2, 1, 1],
            [4, 7, 2, 1, 1]
        ]
    );
}

const TIMED: &str = r#"{
  "name": "Timed",
  "steps": [
    { "id": "w", "name": "W", "initial": true },
    { "id": "on", "name": "On", "actions": [
      { "id": "l", "name": "ActL", "qualifier": "L", "time": "T#100ms" },
      { "id": "d", "name": "ActD", "qualifier": "D", "time": "T#100ms" },
      { "id": "sd", "name": "ActSD", "qualifier": "SD", "time": "T#100ms" },
      { "id": "ds", "name": "ActDS", "qualifier": "DS", "time": "T#100ms" },
      { "id": "sl", "name": "ActSL", "qualifier": "SL", "time": 100 }
    ] },
    { "id": "off", "name": "Off" },
    { "id": "rst", "name": "Rst", "actions": [
      { "id": "r1", "name": "ActSD", "qualifier": "R" },
      { "id": "r2", "name": "ActDS", "qualifier": "R" },
      { "id": "r3", "name": "ActSL", "qualifier": "R" }
    ] }
  ],
  "transitions": [
    { "id": "t1", "condition": "%IX0.0", "sourceStepId": "w", "targetStepId": "on" },
    { "id": "t2", "condition": "%IX0.1", "sourceStepId": "on", "targetStepId": "off" },
    { "id": "t3", "condition": "%IX0.2", "sourceStepId": "off", "targetStepId": "rst" }
  ]
}"#;

/// The action flags of L, D, SD, DS and SL, as 0 or 1.
fn timed_flags(run: &Run) -> [u8; 5] {
    ["ActL", "ActD", "ActSD", "ActDS", "ActSL"]
        .map(|action| u8::from(run.flag(&format!("{action}.Q"))))
}

#[test]
fn time_related_qualifiers_limit_and_delay_by_their_duration() {
    // Figure 23 a) with T#100ms (five 20 ms scans). On is entered in scan 1 and active from
    // scan 2, so its timers start in scan 2 and reach 100 ms in scan 6: L is TRUE in scans 2-5
    // and FALSE from 6; D, SD and DS become TRUE in scan 6; SL is TRUE in scans 2-5. On is left in
    // scan 8: from scan 9 L and D are FALSE, SD and DS stay TRUE (stored), SL stays FALSE. Rst,
    // entered in scan 10, resets SD, DS and SL in scan 11. Catches a delay or limit off by a scan,
    // a stored qualifier that ends with its step, and a reset that does not reach a stored timer.
    let mut run = Run::new("timed", TIMED);
    run.input("%IX0.0", true);
    let mut seen = Vec::new();
    for scan in 1..=11 {
        match scan {
            8 => run.input("%IX0.1", true),
            10 => run.input("%IX0.2", true),
            _ => {}
        }
        run.scan();
        seen.push(timed_flags(&run));
    }
    assert_eq!(
        seen,
        [
            [0, 0, 0, 0, 0],
            [1, 0, 0, 0, 1],
            [1, 0, 0, 0, 1],
            [1, 0, 0, 0, 1],
            [1, 0, 0, 0, 1],
            [0, 1, 1, 1, 0],
            [0, 1, 1, 1, 0],
            [0, 1, 1, 1, 0],
            [0, 0, 1, 1, 0],
            [0, 0, 1, 1, 0],
            [0, 0, 0, 0, 0]
        ]
    );
}

#[test]
fn stored_delays_and_limits_keep_their_duration_after_the_step() {
    // On is active only in scans 2 and 3 (left in scan 3). L and D end with the step (D never
    // reached its delay). SD was stored in scan 2 and its delay of 100 ms still runs: TRUE from
    // scan 6. DS ended before its delay and is never stored. SL was stored in scan 2 and stays
    // TRUE until its 100 ms are reached, scans 2-5, after its step ended. Catches the literal T
    // of §6.7.4.6 (t#0s once no association is active), which would set SD in scan 4 and end SL
    // with its step (sfc-profile.md; docs/IEC_DECISIONS.md 2026-10-26).
    let mut run = Run::new("timed", TIMED);
    run.input("%IX0.0", true);
    let mut seen = Vec::new();
    for scan in 1..=8 {
        if scan == 3 {
            run.input("%IX0.1", true);
        }
        run.scan();
        seen.push(timed_flags(&run));
    }
    assert_eq!(
        seen,
        [
            [0, 0, 0, 0, 0],
            [1, 0, 0, 0, 1],
            [1, 0, 0, 0, 1],
            [0, 0, 0, 0, 1],
            [0, 0, 0, 0, 1],
            [0, 0, 1, 0, 0],
            [0, 0, 1, 0, 0],
            [0, 0, 1, 0, 0]
        ]
    );
}

#[test]
fn a_timer_takes_the_duration_of_its_active_association() {
    // Lamp is limited to T#40ms in A and to T#100ms in B, with G between them, whose scans reset
    // the timer. A is active from scan 2: its 40 ms are reached in scan 3, so Lamp is TRUE only in
    // scan 2. B is active from scan 8: its 100 ms are reached in scan 12, so Lamp is TRUE in scans
    // 8-11. Catches one duration for every association.
    let chart = r#"{
      "name": "Dur",
      "steps": [
        { "id": "s0", "name": "S0", "initial": true },
        { "id": "a", "name": "A", "actions": [ { "id": "l1", "name": "Lamp", "qualifier": "L", "time": "T#40ms" } ] },
        { "id": "g", "name": "G" },
        { "id": "b", "name": "B", "actions": [ { "id": "l2", "name": "Lamp", "qualifier": "L", "time": "T#100ms" } ] }
      ],
      "transitions": [
        { "id": "t1", "condition": "%IX0.0", "sourceStepId": "s0", "targetStepId": "a" },
        { "id": "t2", "condition": "%IX0.1", "sourceStepId": "a", "targetStepId": "g" },
        { "id": "t3", "condition": "%IX0.2", "sourceStepId": "g", "targetStepId": "b" }
      ]
    }"#;
    let mut run = Run::new("dur", chart);
    run.input("%IX0.0", true);
    let mut seen = Vec::new();
    for scan in 1..=13 {
        match scan {
            5 => run.input("%IX0.1", true),
            7 => run.input("%IX0.2", true),
            _ => {}
        }
        run.scan();
        seen.push(u8::from(run.flag("Lamp.Q")));
    }
    assert_eq!(seen, [0, 1, 0, 0, 0, 0, 0, 1, 1, 1, 1, 0, 0]);
}

/// The text of the SFC example `file` of `examples/sfc`.
fn example(file: &str) -> String {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../examples/sfc/");
    std::fs::read_to_string(format!("{path}{file}")).expect("example SFC")
}

#[test]
fn the_simple_parallel_example_runs_both_tasks_and_joins_after_the_longer() {
    // examples/sfc/sfc_simple_parallel.sfc: Start sets `ready` in scan 1 and its transition
    // enters TaskA and TaskB; from scan 2 ExecuteTaskA is limited to T#1s (reached in scan 51,
    // its final scan writes `task_a_running` FALSE) and ExecuteTaskB to T#2s (reached in scan
    // 101); the join waits for both and enters End in scan 101, whose action sets `done` in scan
    // 102. Catches an example that does not compile (it called `log`, and used `T#1s` as a
    // condition) or never joins.
    let steps = ["Start", "TaskA", "TaskB", "End"];
    let mut run = Run::new("sfc_simple_parallel", &example("sfc_simple_parallel.sfc"));
    run.scan();
    assert_eq!(run.active(&steps), ["TaskA", "TaskB"], "scan 1");
    for _ in 2..=50 {
        run.scan();
    }
    assert!(
        run.flag("task_a_running") && run.flag("task_b_running"),
        "scan 50"
    );
    run.scan();
    assert!(!run.flag("task_a_running"), "TaskA's 1 s ends in scan 51");
    assert_eq!(
        run.active(&steps),
        ["TaskA", "TaskB"],
        "scan 51: the join waits"
    );
    for _ in 52..=100 {
        run.scan();
    }
    assert_eq!(run.active(&steps), ["TaskA", "TaskB"], "scan 100");
    run.scan();
    assert_eq!(run.active(&steps), ["End"], "scan 101");
    assert!(!run.flag("task_b_running"), "TaskB's 2 s end in scan 101");
    assert!(!run.flag("done"), "scan 101");
    run.scan();
    assert!(run.flag("done"), "scan 102");
}

#[test]
fn the_parallel_process_example_finishes_after_its_longest_branch() {
    // examples/sfc/sfc_parallel_branches_example.sfc: Init (scan 1) and Prepare (scan 2) pass at
    // once; Heat, Mix and Polish are active from scan 3, limited to T#2s, T#3s and T#1500ms, so
    // Polish ends in scan 77, Heat in 102 and Mix in 152, when the join enters Finish; Finish
    // sets `process_complete` in scan 153.
    let steps = ["Init", "Prepare", "Heat", "Mix", "Polish", "Finish"];
    let mut run = Run::new(
        "sfc_parallel_branches_example",
        &example("sfc_parallel_branches_example.sfc"),
    );
    run.scan();
    assert_eq!(run.active(&steps), ["Prepare"], "scan 1");
    run.scan();
    assert_eq!(run.active(&steps), ["Heat", "Mix", "Polish"], "scan 2");
    for scan in 3..=151 {
        run.scan();
        assert_eq!(run.flag("polisher_on"), scan < 77, "scan {scan}");
        assert_eq!(run.flag("heater_on"), scan < 102, "scan {scan}");
        assert!(run.flag("mixer_on"), "scan {scan}");
    }
    assert_eq!(run.active(&steps), ["Heat", "Mix", "Polish"], "scan 151");
    run.scan();
    assert_eq!(run.active(&steps), ["Finish"], "scan 152");
    assert!(!run.flag("mixer_on"), "scan 152");
    run.scan();
    assert!(run.flag("process_complete"), "scan 153");
}

#[test]
fn the_snake_examples_compile_and_wait_for_their_start_button() {
    // examples/sfc/ethercat-snake-*.sfc.json drive EL2008 outputs; without their start button
    // they stay in Init. Catches an example companion that does not compile (the simple snake
    // compared a TIME variable nothing advanced).
    for (file, base) in [
        ("ethercat-snake-simple.sfc.json", "ethercat_snake_simple"),
        (
            "ethercat-snake-advanced.sfc.json",
            "ethercat_snake_advanced",
        ),
    ] {
        let mut run = Run::new(base, &example(file));
        for _ in 0..5 {
            run.scan();
        }
        assert!(run.flag("Init_active"), "{file}");
    }
}
