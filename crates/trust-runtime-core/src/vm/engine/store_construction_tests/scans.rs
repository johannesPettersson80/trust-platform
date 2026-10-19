//! The cell store and the `Value` store run the same scans (specification 34 §6.2, MEM-10 and TIME-08;
//! C1-9d3a, R2).
//!
//! The engine reads and writes the globals and the persistent instances only through the store-access
//! seam, whichever store it holds. These tests build every program of the bounded-C corpus on both
//! stores, give both the same inputs, run the same scans and compare what the program leaves behind:
//! every declared variable, the output, marker and hierarchical images, the latched fault, the result of
//! the scan, and the work and the allocation bytes charged at each phase of it. A program cannot tell the
//! stores apart (specification 34 §6.2), so every line is equal, and the `Value` store's line is the
//! expected one. A comparison drives both stores through a warm and a cold restart as well, because the
//! restart carries the values the scans left.
//!
//! The differences that C1-9b lists are not facts of this comparison: a cell `WSTRING` has no borrowed
//! UTF-8 text but materializes the same `Value`, `snapshot_instance` is not read by a scan, and the
//! program root of a corpus program has a type, so it reads its instance on both stores.

use super::{
    build, chip_limits, corpus, declared_values, member, prepare, services, EngineState, StoreKind,
    CORPUS,
};
use crate::bytecode::StorageOwner;
use crate::error::RuntimeError;
use crate::io_address::IoAddress;
use crate::memory::view::StoreAccess;
use crate::memory::InstanceId;
use crate::retain::RestartMode;
use crate::value::{Duration, Value};
use crate::vm::{CycleAccess, CycleHooks, CyclePhase, PreparationLimits, PreparedModule};

/// The scans each program runs before it is restarted, 10 ms apart: one activation of its task each.
const SCANS: i64 = 20;

/// The scans each program runs after a restart.
const SCANS_AFTER_RESTART: i64 = 2;

/// The limits a corpus pass prepares its programs under.
type Limits = fn() -> PreparationLimits;

/// Records, at every phase of a scan that the engine reports, the work left in the scan's allowance and
/// the allocation bytes charged so far (TIME-08). The work allowance restarts for each program a task
/// runs, so the points between the programs carry what each of them charged.
#[derive(Default)]
struct ChargeLog {
    points: Vec<(String, usize, usize)>,
}

impl CycleHooks for ChargeLog {
    // The default scan: no output hook, so the callbacks after the output sample do not run.
    fn has_output_hooks(&self) -> bool {
        false
    }

    fn on_phase(
        &mut self,
        phase: CyclePhase,
        access: &mut CycleAccess<'_, '_>,
    ) -> Result<(), RuntimeError> {
        let resources = &access.engine.data.resources;
        self.points.push((
            format!("{phase:?}"),
            resources.work_budget.remaining(),
            resources.construction_bytes.get(),
        ));
        Ok(())
    }
}

/// Everything that a program leaves behind and its user can read, one line per fact, in an order both
/// stores share: every declared variable, the three process images, and the latched fault.
fn facts(prepared: &PreparedModule, state: &EngineState<'_>) -> Vec<String> {
    let mut lines = declared_values(prepared, state);
    let data = &*state.data;
    lines.push(format!("outputs {:?}", data.images.outputs));
    lines.push(format!("markers {:?}", data.images.memory));
    lines.push(format!("hierarchical {:?}", data.images.hierarchical));
    lines.push(format!("fault {:?} {:?}", data.fault, data.fault_record));
    lines
}

/// Run one scan at `now`, and answer its result with the lines that describe how it ended and what it
/// charged: one for its result, one for each phase it reported and one for its end.
fn run_scan(state: &mut EngineState<'_>, now: Duration) -> (Result<(), RuntimeError>, Vec<String>) {
    let mut log = ChargeLog::default();
    let result = state.execute_cycle_with_hooks(now, &mut log);
    let mut lines = vec![format!("result {result:?}")];
    for (phase, work_left, bytes) in &log.points {
        lines.push(format!(
            "charged at {phase}: work left {work_left}, bytes {bytes}"
        ));
    }
    let resources = &state.data.resources;
    lines.push(format!(
        "charged at the end: work left {}, bytes {}",
        resources.work_budget.remaining(),
        resources.construction_bytes.get()
    ));
    (result, lines)
}

/// Give the input image the sample of scan number `scan`: every byte is one of four ASCII letters, so each
/// bound bit toggles from scan to scan and every `STRING` window stays valid text.
fn drive(state: &mut EngineState<'_>, scan: i64) {
    let step = usize::try_from(scan).expect("a scan number is not negative");
    for (index, byte) in state.data.images.inputs.iter_mut().enumerate() {
        let letter = u8::try_from((step + index) % 4).expect("a remainder of 4 fits a byte");
        *byte = b'A' + letter;
    }
}

/// The hierarchical inputs a corpus program binds: `io` binds `remote AT %IX1.2.3 : BOOL`, the one of the
/// corpus. They are given the value TRUE before the first scan.
fn hierarchical_inputs(name: &str) -> &'static [&'static str] {
    if name == "io" {
        &["%IX1.2.3"]
    } else {
        &[]
    }
}

/// Both stores of one program, which a comparison drives scan by scan.
struct Pair<'a, 'e> {
    name: &'a str,
    prepared: &'a PreparedModule,
    values: EngineState<'e>,
    cells: EngineState<'e>,
    differences: Vec<String>,
}

impl Pair<'_, '_> {
    /// Record each line on which the two stores' facts differ, and answer whether they agree.
    fn agree(&mut self, stage: &str, expected: &[String], actual: &[String]) -> bool {
        let before = self.differences.len();
        if expected.len() != actual.len() {
            self.differences.push(format!(
                "{} {stage}: the Value store reports {} facts and the cell store {}",
                self.name,
                expected.len(),
                actual.len()
            ));
        }
        for (expected, actual) in expected.iter().zip(actual) {
            if expected != actual {
                self.differences.push(format!(
                    "{} {stage}: values `{expected}` / cells `{actual}`",
                    self.name
                ));
            }
        }
        self.differences.len() == before
    }

    /// Compare the stores as construction left them.
    fn constructed(&mut self) -> bool {
        let prepared = self.prepared;
        // Every global, and every persistent instance with its type, its parent and its members, is one
        // line: a comparison of empty lists would prove nothing.
        let declared = prepared.declarations(StorageOwner::Global, None).len()
            + prepared
                .roots
                .entries
                .iter()
                .filter_map(|root| root.template_pou_id)
                .map(|pou| {
                    1 + prepared
                        .declarations(StorageOwner::Instance, Some(pou))
                        .len()
                })
                .sum::<usize>();
        assert_eq!(
            declared_values(prepared, &self.values).len(),
            declared,
            "{} constructed",
            self.name
        );
        let expected = facts(prepared, &self.values);
        let actual = facts(prepared, &self.cells);
        self.agree("constructed", &expected, &actual)
    }

    /// Give both stores the hierarchical inputs of the program.
    fn stage_hierarchical_inputs(&mut self) {
        for text in hierarchical_inputs(self.name) {
            let address = IoAddress::parse(text).expect("the corpus address parses");
            for state in [&mut self.values, &mut self.cells] {
                state
                    .engineering_operation(|state| {
                        state.set_hierarchical_input(&address, Value::Bool(true))
                    })
                    .expect("the hierarchical input is staged");
            }
        }
    }

    /// Run scan number `scan` on both stores, whatever way it ends, and compare what they left. Answers
    /// how the scan ended on the `Value` store, and whether the two stores agree.
    fn scan_once(&mut self, scan: i64, label: &str) -> (Result<(), RuntimeError>, bool) {
        drive(&mut self.values, scan);
        drive(&mut self.cells, scan);
        let now = Duration::from_millis(10 * scan);
        let (outcome, mut expected) = run_scan(&mut self.values, now);
        expected.extend(facts(self.prepared, &self.values));
        let (_, mut actual) = run_scan(&mut self.cells, now);
        actual.extend(facts(self.prepared, &self.cells));
        let agreed = self.agree(label, &expected, &actual);
        (outcome, agreed)
    }

    /// Run the scans numbered `scans` on both stores, and compare after each. `stage` prefixes the
    /// label of every scan. Answers whether every scan agreed and ended well on the `Value` store, which
    /// every corpus program does when it is healthy.
    fn scans(&mut self, scans: core::ops::RangeInclusive<i64>, stage: &str) -> bool {
        for scan in scans {
            let label = format!("{stage}scan {scan}");
            let (outcome, agreed) = self.scan_once(scan, &label);
            if let Err(error) = &outcome {
                self.differences.push(format!(
                    "{} {label}: the Value store's own scan ended with `{error:?}`",
                    self.name
                ));
            }
            if !agreed || outcome.is_err() {
                return false;
            }
        }
        true
    }

    /// Restart both stores in `mode` and compare how the restarts ended and what they left.
    fn restarted(&mut self, mode: RestartMode, stage: &str) -> bool {
        let mut expected = vec![format!("restart {:?}", self.values.restart(mode))];
        expected.extend(facts(self.prepared, &self.values));
        let mut actual = vec![format!("restart {:?}", self.cells.restart(mode))];
        actual.extend(facts(self.prepared, &self.cells));
        self.agree(stage, &expected, &actual)
    }

    /// The cell store ran the body of the program in every scan, so that the agreement is not two stores
    /// that did nothing: a corpus program counts its activations in `ticks`, and `blocks` counts the
    /// rising edges of its toggle in its `CTU`, which sees `TRUE` in every odd scan.
    fn ran_the_body(&self) {
        let ticks = StoreAccess::read_global(&self.cells, "ticks");
        if let Some(ticks) = ticks {
            assert_eq!(
                ticks.materialize().expect("ticks materializes"),
                Value::DInt(20),
                "{}: one activation in each of the {SCANS} scans",
                self.name
            );
        }
        if self.name == "blocks" {
            let Value::Instance(counter) = member(&self.cells, InstanceId(0), "counter") else {
                panic!("blocks: the counter is an instance");
            };
            assert_eq!(
                member(&self.cells, counter, "CV"),
                Value::Int(10),
                "blocks: the toggle rises in scans 1, 3, ... 19"
            );
        }
    }

    /// Compare the stores through construction, the scans and a warm and a cold restart with scans
    /// between them, and stop at the first stage on which they differ.
    fn run(&mut self) {
        if !self.constructed() {
            return;
        }
        self.stage_hierarchical_inputs();
        if !self.scans(1..=SCANS, "") {
            return;
        }
        self.ran_the_body();
        let mut next = SCANS + 1;
        for (mode, stage) in [
            (RestartMode::Warm, "after a warm restart"),
            (RestartMode::Cold, "after a cold restart"),
        ] {
            if !self.restarted(mode, stage) {
                return;
            }
            let last = next + SCANS_AFTER_RESTART - 1;
            if !self.scans(next..=last, &format!("{stage}, ")) {
                return;
            }
            next = last + 1;
        }
    }
}

/// Build every corpus program on both stores under `limits` and compare them through their scans. The
/// first list holds one line for every fact that differs, and the second the programs that the cell store
/// refused.
fn compare_corpus(limits: Limits) -> (Vec<String>, Vec<String>) {
    let mut differences = Vec::new();
    let mut refused = Vec::new();
    for (name, bytes) in CORPUS {
        let prepared = PreparedModule::from_bytes(bytes, limits()).expect("the program prepares");
        let mut values =
            build(&prepared, StoreKind::Values).expect("the Value store constructs the program");
        let mut cells = match build(&prepared, StoreKind::Cells) {
            Ok(cells) => cells,
            Err(error) => {
                refused.push(format!("{name}: {error:?}"));
                continue;
            }
        };
        let mut pair = Pair {
            name,
            prepared: &prepared,
            values: EngineState::borrow(&mut values, &prepared, &services::LOGICAL_ONLY),
            cells: EngineState::borrow(&mut cells, &prepared, &services::LOGICAL_ONLY),
            differences: Vec::new(),
        };
        pair.run();
        differences.append(&mut pair.differences);
    }
    (differences, refused)
}

#[test]
fn cells_and_values_agree_after_every_scan() {
    // Specification 34 §6.2 (MEM-10) and TIME-08, and the C1-9 note's rule for the switch: a program that
    // runs on the cell store reads, writes, faults and is charged exactly as it is on the `Value` store.
    // Each corpus program is built on both stores and compared after construction; given the same inputs
    // it runs 20 scans, and after every one of them every declared global and every member of every
    // persistent instance, the output, marker and hierarchical images, the result of the scan, the latched
    // fault, and the work left and the allocation bytes charged at each phase of the scan are compared
    // line by line with the `Value` store's. Both stores are then restarted warm and cold, with two scans
    // after each restart, because a warm restart carries the values the 20 scans left in the retained
    // variables. The programs are built under the default limits and, without the register tier, under the
    // chips' construction limits (128 KiB of construction bytes and 50,000 units of work), which the cell
    // store's charge must still fit.
    //
    // A cell store that read the program root from the `Value` half would find no instance and fault in
    // the first scan; one that applied an input to the `Value` half would leave the cells at their
    // defaults; one that executed a built-in block on the `Value` half would never advance `blocks`' timer
    // and counter (the guard on `blocks` expects the `CTU` to count the 10 rising edges of its toggle); one
    // that journaled a call's output destination in the wrong slot would restore the wrong value; a
    // charge that depended on where a value is held would show on the line of the phase that charged it.
    // The guard on `ticks` expects each program to have run its body in all 20 scans.
    //
    // No program is refused: since C1-15 a `STRUCT OVERLAP` lies as its backing, so `structures` and
    // `overlays` run on both stores too, and an overlay that kept a member's bits apart, lost a copied
    // structure's gap bits or misplaced a text beside the backing would differ in their values.
    let mut passes: Vec<(Limits, &str)> = vec![(PreparationLimits::default, "default limits")];
    if !cfg!(feature = "register-ir") {
        passes.push((chip_limits, "chip limits"));
    }
    for (limits, label) in passes {
        let (differences, refused) = compare_corpus(limits);
        assert_eq!(
            differences,
            Vec::<String>::new(),
            "{label}: the cells differ from the Value store"
        );
        assert_eq!(refused, Vec::<String>::new(), "{label}");
    }
}

/// The fault kinds of the corpus program `faults` (`faults/src/main.st`: the test writes `fault_kind`
/// between scans) and the stable code each ends its scan with.
const FAULT_KINDS: [(i16, &str); 6] = [
    (1, "runtime_division_by_zero"),
    (2, "runtime_index_out_of_bounds"),
    (3, "runtime_null_reference"),
    (4, "runtime_overflow"),
    (5, "runtime_execution_timeout"),
    (6, "vm_call_stack_overflow"),
];

#[test]
fn cells_and_values_fault_alike() {
    // Specification 34 MEM-05 and STATE-04 (the fault record), TIME-08 and the C1-9 note: a program that
    // faults on the cell store faults on the same instruction, with the same error, the same record, the
    // same values as the fault left them, and the same work and allocation charged up to it. The program
    // is `faults`: its scan runs the healthy branch until the test writes `fault_kind` through the
    // engineering write, and kind 1 divides by zero, 2 reads outside `ARRAY[1..4]` at index 9, 3
    // dereferences a null reference, 4 overflows a REAL (1.0E30 squared), 5 spins until the work
    // allowance (50,000 units under the chips' limits) is spent, and 6 calls four functions deep where
    // the chips admit a depth of four. Each kind runs on a fresh pair of stores: two healthy scans, the
    // write, the faulting scan, and one more scan, which a latched fault refuses (`runtime_resource_faulted`).
    // A cell store whose work charges differ would spin a different number of times before kind 5 faults,
    // so `spins` would differ; one that moved a fault to another instruction would differ in the record's
    // site.
    let prepared = PreparedModule::from_bytes(corpus("faults"), chip_limits())
        .expect("the chips' admission accepts the faults program");
    let mut differences = Vec::new();
    for (kind, code) in FAULT_KINDS {
        let mut values = build(&prepared, StoreKind::Values).expect("the Value store constructs");
        let mut cells = build(&prepared, StoreKind::Cells).expect("the cell store constructs");
        let mut pair = Pair {
            name: "faults",
            prepared: &prepared,
            values: EngineState::borrow(&mut values, &prepared, &services::LOGICAL_ONLY),
            cells: EngineState::borrow(&mut cells, &prepared, &services::LOGICAL_ONLY),
            differences: Vec::new(),
        };
        assert!(pair.constructed(), "kind {kind}: construction");
        assert!(pair.scans(1..=2, ""), "kind {kind}: the healthy scans");
        for state in [&mut pair.values, &mut pair.cells] {
            state
                .engineering_operation(|state| state.write_global("fault_kind", Value::Int(kind)))
                .expect("fault_kind is written");
        }
        let (outcome, _) = pair.scan_once(3, &format!("kind {kind}, the faulting scan"));
        assert_eq!(
            outcome.map_err(|error| error.stable_code().as_str()),
            Err(code),
            "kind {kind}: the Value store's faulting scan"
        );
        let (outcome, _) = pair.scan_once(4, &format!("kind {kind}, the scan after the fault"));
        assert_eq!(
            outcome.map_err(|error| error.stable_code().as_str()),
            Err("runtime_resource_faulted"),
            "kind {kind}: a latched fault refuses the next scan"
        );
        differences.append(&mut pair.differences);
    }
    assert_eq!(
        differences,
        Vec::<String>::new(),
        "the cells fault differently from the Value store"
    );
}

#[test]
fn an_initializer_evaluated_in_place_runs_out_of_work_where_the_value_store_does() {
    // Specification 34 TIME-08 and ARCH-05 (C1-13): the cell store stages the initializers of `Pick`'s
    // `window : ARRAY[1..2] OF INT := [INT#7, INT#8]` on the activation stack (the default constructed
    // in place, the seed copied from the variable, the array built where the result lies, the commit a
    // copy), and each step charges the work the value path charges, in the same order. So whatever the
    // scan's work allowance, both stores end the first two scans of `initializers` alike: the same
    // result, the same work left and bytes charged at every phase, and the same values, as an
    // allowance spent part way leaves them. The sweep runs from one unit (an allowance of no work is
    // not an admitted profile) to the work one healthy scan takes. A charge made earlier or later
    // than the value path's would let one store run a step further than the other at some allowance,
    // and the lines of that allowance would differ.
    let program = corpus("initializers");
    let healthy = {
        let prepared = prepare(program);
        let mut values = build(&prepared, StoreKind::Values).expect("the Value store constructs");
        let mut state = EngineState::borrow(&mut values, &prepared, &services::LOGICAL_ONLY);
        let (outcome, _) = run_scan(&mut state, Duration::from_millis(10));
        outcome.expect("a healthy scan of initializers");
        // The allowance starts afresh for the program the scan runs.
        prepared.limits.max_work - state.data.resources.work_budget.remaining()
    };
    assert!(healthy > 0, "a scan of initializers charges work");
    let mut differences = Vec::new();
    for allowance in 1..=healthy {
        let limits = PreparationLimits {
            max_work: allowance,
            ..PreparationLimits::default()
        };
        let prepared = PreparedModule::from_bytes(program, limits).expect("the program prepares");
        let mut values = build(&prepared, StoreKind::Values).expect("the Value store constructs");
        let mut cells = build(&prepared, StoreKind::Cells).expect("the cell store constructs");
        let mut pair = Pair {
            name: "initializers",
            prepared: &prepared,
            values: EngineState::borrow(&mut values, &prepared, &services::LOGICAL_ONLY),
            cells: EngineState::borrow(&mut cells, &prepared, &services::LOGICAL_ONLY),
            differences: Vec::new(),
        };
        for scan in 1..=2 {
            // Each disagreement is recorded; the outcome itself may be a refusal.
            let _ = pair.scan_once(scan, &format!("allowance {allowance}, scan {scan}"));
        }
        differences.append(&mut pair.differences);
    }
    assert_eq!(
        differences,
        Vec::<String>::new(),
        "the cells spend their work differently from the Value store"
    );
}

#[test]
fn a_warm_restart_copies_retained_cells_where_the_value_store_copies_values() {
    // Specification 34 §11.3, TIME-08 and MEM-12 (C1-14b): a warm restart carries the retained values
    // into the next generation. `retained_blocks` retains a function-block instance `kept`, the
    // reference `follow` to `kept.count`, `total` and its program's `runs`; `retained_values` retains
    // arrays, a structure and texts in globals and in its program; `overlays` retains a `STRUCT
    // OVERLAP` with a text beside its backing and `unions` a union with a text variant (C1-15). The
    // cell store copies them from
    // cell to cell, an instance's members and a flat reference's generation included, and charges
    // each copy as the value store charges the copy of the value. So at every construction work
    // allowance, from one unit up to the first at which both stores construct the program and restart
    // it warm, both stores end alike: construction refused with the same error, or two scans and a
    // warm restart with the same result, the same work and bytes charged at each phase and the same
    // values after it. A copy charged in another order, a reference copied without its new
    // generation, an instance leaf without its new identity or a misplaced byte would make the lines
    // of some allowance differ.
    let mut differences = Vec::new();
    for name in ["retained_blocks", "retained_values", "overlays", "unions"] {
        let program = corpus(name);
        let mut restarted = false;
        for allowance in 1..=200_000 {
            let limits = PreparationLimits {
                max_construction_work: allowance,
                ..PreparationLimits::default()
            };
            let prepared =
                PreparedModule::from_bytes(program, limits).expect("the program prepares");
            let built = (
                build(&prepared, StoreKind::Values),
                build(&prepared, StoreKind::Cells),
            );
            let (mut values, mut cells) = match built {
                (Ok(values), Ok(cells)) => (values, cells),
                (Err(values), Err(cells)) => {
                    if values != cells {
                        differences.push(format!(
                            "{name} allowance {allowance}: construction refused with \
                             `{values:?}` / `{cells:?}`"
                        ));
                    }
                    continue;
                }
                (values, cells) => {
                    differences.push(format!(
                        "{name} allowance {allowance}: construction `{:?}` / `{:?}`",
                        values.err(),
                        cells.err()
                    ));
                    break;
                }
            };
            let mut pair = Pair {
                name,
                prepared: &prepared,
                values: EngineState::borrow(&mut values, &prepared, &services::LOGICAL_ONLY),
                cells: EngineState::borrow(&mut cells, &prepared, &services::LOGICAL_ONLY),
                differences: Vec::new(),
            };
            let healthy = pair.scans(1..=2, &format!("allowance {allowance}, "));
            let mut warm = false;
            if healthy {
                let restart = pair.values.restart(RestartMode::Warm);
                warm = restart.is_ok();
                let mut expected = vec![format!("restart {restart:?}")];
                expected.extend(facts(&prepared, &pair.values));
                let mut actual = vec![format!(
                    "restart {:?}",
                    pair.cells.restart(RestartMode::Warm)
                )];
                actual.extend(facts(&prepared, &pair.cells));
                pair.agree(
                    &format!("allowance {allowance}, warm restart"),
                    &expected,
                    &actual,
                );
            }
            differences.append(&mut pair.differences);
            if !differences.is_empty() {
                break;
            }
            if warm {
                restarted = true;
                break;
            }
        }
        assert!(
            restarted || !differences.is_empty(),
            "{name}: some allowance restarts the program"
        );
    }
    assert_eq!(
        differences,
        Vec::<String>::new(),
        "the cells restart differently from the Value store"
    );
}

#[test]
fn a_byte_string_input_ends_at_its_first_nul_and_refuses_invalid_text_alike() {
    // Specification 11 §6.4: reading a byte-string window "stops at the first NUL byte; invalid UTF-8
    // before that terminator is a type error", and input synchronization is transactional: "a read,
    // conversion, undefined target, or null-reference failure leaves all bound program values
    // unchanged". Specification 34 §6.2 and TIME-08: the cell store, which writes a `STRING` input from
    // the image into the text where it lies (C1-16), reads, refuses and charges as the `Value` store.
    // The program is `io`: `label AT %IB12 : STRING[24]` is bytes 12 to 35 of the input image, and the
    // body copies it to `echo AT %QB8 : STRING[8]`. After two healthy scans, scan 3 gives the window `AB`,
    // a NUL, then 0xFF and letters: the text is `AB`, so a store that read the whole window would refuse
    // the 0xFF, and one that ignored the NUL would keep the letters after it. Scan 4 gives a window that
    // starts with 0xFF: both stores refuse the scan with the type error and write no input, so `label`
    // keeps `AB`, and `level AT %IB1`, which the drive gives `B` (66) in scan 4, keeps the `A` (65) of
    // scan 3; a store that wrote the inputs sampled before the refused one would hold 66.
    let prepared = PreparedModule::from_bytes(corpus("io"), chip_limits())
        .expect("the chips' admission accepts the io program");
    let mut values = build(&prepared, StoreKind::Values).expect("the Value store constructs");
    let mut cells = build(&prepared, StoreKind::Cells).expect("the cell store constructs");
    let mut pair = Pair {
        name: "io",
        prepared: &prepared,
        values: EngineState::borrow(&mut values, &prepared, &services::LOGICAL_ONLY),
        cells: EngineState::borrow(&mut cells, &prepared, &services::LOGICAL_ONLY),
        differences: Vec::new(),
    };
    assert!(pair.constructed(), "construction: {:?}", pair.differences);
    pair.stage_hierarchical_inputs();
    assert!(
        pair.scans(1..=2, ""),
        "the healthy scans: {:?}",
        pair.differences
    );
    let mut nul = [b'C'; 24];
    nul[..4].copy_from_slice(&[b'A', b'B', 0, 0xFF]);
    let mut invalid = [b'C'; 24];
    invalid[0] = 0xFF;
    for (scan, window, outcome) in [(3, nul, Ok(())), (4, invalid, Err("runtime_type_mismatch"))] {
        let mut lines = [Vec::new(), Vec::new()];
        for (state, lines) in [&mut pair.values, &mut pair.cells]
            .into_iter()
            .zip(&mut lines)
        {
            drive(state, scan);
            state.data.images.inputs[12..36].copy_from_slice(&window);
            let (result, scanned) = run_scan(state, Duration::from_millis(10 * scan));
            assert_eq!(
                result.map_err(|error| error.stable_code().as_str()),
                outcome,
                "scan {scan}"
            );
            *lines = scanned;
            lines.extend(facts(&prepared, state));
        }
        let [expected, actual] = &lines;
        assert!(
            pair.agree(&format!("scan {scan}"), expected, actual),
            "{:?}",
            pair.differences
        );
        for (store, state) in [("Value store", &pair.values), ("cell store", &pair.cells)] {
            assert_eq!(
                member(state, InstanceId(0), "label"),
                Value::String("AB".into()),
                "{store}, scan {scan}"
            );
            assert_eq!(
                member(state, InstanceId(0), "level"),
                Value::Byte(b'A'),
                "{store}, scan {scan}"
            );
        }
    }
}
