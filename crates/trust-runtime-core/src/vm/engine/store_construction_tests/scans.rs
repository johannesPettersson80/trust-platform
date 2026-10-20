//! Every program of the bounded-C corpus runs on the flat store as it ran on the `Value` store
//! (specification 34 §6.2, MEM-10 and TIME-08; C1-9d3a, R2; C1-19e).
//!
//! The engine reads and writes the globals and the persistent instances only through the store-access
//! seam. Until the `Value` store was deleted, these tests built every program of the bounded-C corpus on
//! both stores, gave both the same inputs, ran the same scans and compared what the program left behind:
//! every declared variable, the output, marker and hierarchical images, the latched fault, the copy of
//! the storage the debugger reads (C1-17), the result of the scan, and the work and the allocation bytes
//! charged at each phase of it. A program cannot tell the stores apart (specification 34 §6.2), so every
//! line was equal, and the `Value` store's lines were the expected ones. They are recorded now
//! (`expected.rs`), and the flat store's lines are compared with them. A run drives the flat store
//! through a warm and a cold restart as well, because the restart carries the values the scans left.
//!
//! The differences that C1-9b lists are not facts of this comparison: a cell `WSTRING` has no borrowed
//! UTF-8 text but materializes the same `Value`, `snapshot_instance` is not read by a scan, and the
//! program root of a corpus program has a type, so it reads its instance.

use super::expected::Facts;
use super::{
    build, build_served, chip_limits, corpus, declared_values, member, prepare, services,
    EngineState, CORPUS,
};
use crate::bytecode::StorageOwner;
use crate::error::RuntimeError;
use crate::io_address::IoAddress;
use crate::memory::view::StoreAccess;
use crate::memory::InstanceId;
use crate::retain::RestartMode;
use crate::value::{Duration, Value};
use crate::vm::{
    CycleAccess, CycleHooks, CyclePhase, ExecutionServices, PreparationLimits, PreparedModule,
};

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

/// Everything that a program leaves behind and its user can read, one line per fact: every declared
/// variable, the three process images, the latched fault, and the copy of the storage that the debugger
/// reads.
fn left_behind(prepared: &PreparedModule, state: &EngineState<'_>) -> Vec<String> {
    let mut lines = declared_values(prepared, state);
    let data = &*state.data;
    lines.push(format!("outputs {:?}", data.images.outputs));
    lines.push(format!("markers {:?}", data.images.memory));
    lines.push(format!("hierarchical {:?}", data.images.hierarchical));
    lines.push(format!("fault {:?} {:?}", data.fault, data.fault_record));
    lines.extend(projected(state));
    lines
}

/// The copy of the storage that the debugger reads (specification 34 §6.3, specification 11 §6.13.1;
/// C1-17): the projection of the flat store's blocks. One line for each global, in its order, and one
/// for each instance, in identity order, with its type, its parent and its variables in their order,
/// since a reference names a variable by its position.
fn projected(state: &EngineState<'_>) -> Vec<String> {
    let storage = match state.data.reader(state.prepared).project_storage() {
        Ok(storage) => storage,
        Err(error) => return vec![format!("projection refused: {error:?}")],
    };
    let mut lines: Vec<String> = storage
        .globals()
        .iter()
        .map(|(name, value)| format!("projected global {name} = {value:?}"))
        .collect();
    let mut instances: Vec<_> = storage.instances().iter().collect();
    instances.sort_by_key(|(id, _)| **id);
    for (id, data) in instances {
        lines.push(format!(
            "projected instance {} of {}, parent {:?}: {:?}",
            id.0, data.type_name, data.parent, data.variables
        ));
    }
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

/// One program on the flat store, which a test drives scan by scan, and the key its facts are checked
/// under.
struct Run<'a, 'e> {
    key: String,
    name: &'a str,
    prepared: &'a PreparedModule,
    state: EngineState<'e>,
}

impl Run<'_, '_> {
    /// Check the facts `lines` of `stage`, and answer whether they agree.
    fn check(&self, facts: &mut Facts, stage: &str, lines: &[String]) -> bool {
        facts.check(&format!("{}/{stage}", self.key), lines)
    }

    /// Check the store as construction left it.
    fn constructed(&mut self, facts: &mut Facts) -> bool {
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
            declared_values(prepared, &self.state).len(),
            declared,
            "{} constructed",
            self.name
        );
        let lines = left_behind(prepared, &self.state);
        self.check(facts, "constructed", &lines)
    }

    /// Give the program its hierarchical inputs.
    fn stage_hierarchical_inputs(&mut self) {
        for text in hierarchical_inputs(self.name) {
            let address = IoAddress::parse(text).expect("the corpus address parses");
            self.state
                .engineering_operation(|state| {
                    state.set_hierarchical_input(&address, Value::Bool(true))
                })
                .expect("the hierarchical input is staged");
        }
    }

    /// Run scan number `scan`, whatever way it ends, and check what it left. Answers how the scan ended,
    /// and whether its facts agree.
    fn scan_once(
        &mut self,
        facts: &mut Facts,
        scan: i64,
        label: &str,
    ) -> (Result<(), RuntimeError>, bool) {
        drive(&mut self.state, scan);
        let (outcome, mut lines) = run_scan(&mut self.state, Duration::from_millis(10 * scan));
        lines.extend(left_behind(self.prepared, &self.state));
        let agreed = self.check(facts, label, &lines);
        (outcome, agreed)
    }

    /// Run the scans numbered `scans`, and check after each. `stage` prefixes the label of every scan.
    /// Answers whether every scan agreed and ended well, which every corpus program's scans do when it is
    /// healthy.
    fn scans(
        &mut self,
        facts: &mut Facts,
        scans: core::ops::RangeInclusive<i64>,
        stage: &str,
    ) -> bool {
        for scan in scans {
            let label = format!("{stage}scan {scan}");
            let (outcome, agreed) = self.scan_once(facts, scan, &label);
            if let Err(error) = &outcome {
                facts.fail(format!(
                    "{}/{label}: the scan ended with `{error:?}`",
                    self.key
                ));
            }
            if !agreed || outcome.is_err() {
                return false;
            }
        }
        true
    }

    /// Restart the store in `mode` and check how the restart ended and what it left.
    fn restarted(&mut self, facts: &mut Facts, mode: RestartMode, stage: &str) -> bool {
        let mut lines = vec![format!("restart {:?}", self.state.restart(mode))];
        lines.extend(left_behind(self.prepared, &self.state));
        self.check(facts, stage, &lines)
    }

    /// The program ran its body in every scan, so that the agreement is not a store that did nothing: a
    /// corpus program counts its activations in `ticks`, and `blocks` counts the rising edges of its
    /// toggle in its `CTU`, which sees `TRUE` in every odd scan.
    fn ran_the_body(&self) {
        if let Some(ticks) = StoreAccess::read_global(&self.state, "ticks") {
            assert_eq!(
                ticks.materialize().expect("ticks materializes"),
                Value::DInt(20),
                "{}: one activation in each of the {SCANS} scans",
                self.name
            );
        }
        if self.name == "blocks" {
            let Value::Instance(counter) = member(&self.state, InstanceId(0), "counter") else {
                panic!("blocks: the counter is an instance");
            };
            assert_eq!(
                member(&self.state, counter, "CV"),
                Value::Int(10),
                "blocks: the toggle rises in scans 1, 3, ... 19"
            );
        }
    }

    /// Check the store through construction, the scans and a warm and a cold restart with scans between
    /// them, and stop at the first stage that differs.
    fn run(&mut self, facts: &mut Facts) {
        if !self.constructed(facts) {
            return;
        }
        self.stage_hierarchical_inputs();
        if !self.scans(facts, 1..=SCANS, "") {
            return;
        }
        self.ran_the_body();
        let mut next = SCANS + 1;
        for (mode, stage) in [
            (RestartMode::Warm, "after a warm restart"),
            (RestartMode::Cold, "after a cold restart"),
        ] {
            if !self.restarted(facts, mode, stage) {
                return;
            }
            let last = next + SCANS_AFTER_RESTART - 1;
            if !self.scans(facts, next..=last, &format!("{stage}, ")) {
                return;
            }
            next = last + 1;
        }
    }
}

/// Build every corpus program under `limits` with the platform services `services`, run it and check its
/// facts under `key`. Answers the programs that the flat store refused, and the register-tier counts over
/// the corpus: the blocks the interpreter entered and the blocks run in their compiled form (C1-18), both
/// zero without the register tier or its profile.
fn run_corpus(
    facts: &mut Facts,
    key: &str,
    limits: Limits,
    services: &'static dyn ExecutionServices,
) -> (Vec<String>, [u64; 2]) {
    let mut refused = Vec::new();
    let mut register = [0u64; 2];
    for (name, bytes) in CORPUS {
        let prepared = PreparedModule::from_bytes(bytes, limits()).expect("the program prepares");
        let mut data = match build_served(&prepared, services) {
            Ok(data) => data,
            Err(error) => {
                refused.push(format!("{name}: {error:?}"));
                continue;
            }
        };
        let mut run = Run {
            key: format!("{key}/{name}"),
            name,
            prepared: &prepared,
            state: EngineState::borrow(&mut data, &prepared, services),
        };
        run.run(facts);
        let counts = register_counts(&run.state);
        register = [register[0] + counts[0], register[1] + counts[1]];
    }
    (refused, register)
}

/// The register tier's counts in `state`: the blocks its interpreter entered, which its profile counts,
/// and the blocks run in their compiled form (C1-18).
#[cfg(feature = "register-ir")]
fn register_counts(state: &EngineState<'_>) -> [u64; 2] {
    use crate::vm::register_ir::RegisterContext;
    [
        RegisterContext::register_profile(state)
            .snapshot()
            .interpreted_block_entries,
        RegisterContext::tier1_state(state)
            .snapshot()
            .block_executions,
    ]
}

/// Without the register tier no block runs in it.
#[cfg(not(feature = "register-ir"))]
fn register_counts(_state: &EngineState<'_>) -> [u64; 2] {
    [0, 0]
}

/// The corpus under the default limits and, without the register tier, under the chips' limits.
fn every_corpus_program(facts: &mut Facts) {
    let mut passes: Vec<(Limits, &str)> = vec![(PreparationLimits::default, "default limits")];
    if !cfg!(feature = "register-ir") {
        passes.push((chip_limits, "chip limits"));
    }
    for (limits, label) in passes {
        let (refused, _) = run_corpus(
            facts,
            &format!("corpus/{label}"),
            limits,
            &services::LOGICAL_ONLY,
        );
        assert_eq!(refused, Vec::<String>::new(), "{label}");
    }
}

#[test]
fn every_corpus_program_runs_as_the_value_store_ran_it() {
    // Specification 34 §6.2 (MEM-10) and TIME-08, and the C1-9 note's rule for the switch: a program that
    // runs on the flat store reads, writes, faults and is charged exactly as it was on the `Value` store.
    // Each corpus program is built and checked after construction; given the same inputs it runs 20
    // scans, and after every one of them every declared global and every member of every persistent
    // instance, the output, marker and hierarchical images, the result of the scan, the latched fault,
    // and the work left and the allocation bytes charged at each phase of the scan are checked line by
    // line against the `Value` store's recorded lines. The store is then restarted warm and cold, with
    // two scans after each restart, because a warm restart carries the values the 20 scans left in the
    // retained variables. The programs are built under the default limits and, without the register
    // tier, under the chips' construction limits (128 KiB of construction bytes and 50,000 units of
    // work), which the flat store's charge must still fit.
    //
    // A store that read the program root from the `Value` half would find no instance and fault in the
    // first scan; one that applied an input to the `Value` half would leave the cells at their defaults;
    // one that executed a built-in block on the `Value` half would never advance `blocks`' timer and
    // counter (the guard on `blocks` expects the `CTU` to count the 10 rising edges of its toggle); one
    // that journaled a call's output destination in the wrong slot would restore the wrong value; a
    // charge that depended on where a value is held would show on the line of the phase that charged it.
    // The debugger's copy of the storage is checked too (C1-17): a projection of the flat store that left
    // out a global or an instance, put a global or an instance's variable at another position, or kept a
    // flat reference in its address form would differ from the value store's own storage. The guard on
    // `ticks` expects each program to have run its body in all 20 scans.
    //
    // No program is refused: since C1-15 a `STRUCT OVERLAP` lies as its backing, so `structures` and
    // `overlays` run too, and an overlay that kept a member's bits apart, lost a copied structure's gap
    // bits or misplaced a text beside the backing would differ in their values.
    let mut facts = Facts::compared();
    every_corpus_program(&mut facts);
    facts.finish("corpus/");
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

/// Each fault kind of `faults` on a fresh store: two healthy scans, the write, the faulting scan and the
/// scan after it.
fn every_fault_kind(facts: &mut Facts) {
    let prepared = PreparedModule::from_bytes(corpus("faults"), chip_limits())
        .expect("the chips' admission accepts the faults program");
    for (kind, code) in FAULT_KINDS {
        let mut data = build(&prepared).expect("the store constructs");
        let mut run = Run {
            key: format!("faults/kind {kind}"),
            name: "faults",
            prepared: &prepared,
            state: EngineState::borrow(&mut data, &prepared, &services::LOGICAL_ONLY),
        };
        run.constructed(facts);
        run.scans(facts, 1..=2, "");
        run.state
            .engineering_operation(|state| state.write_global("fault_kind", Value::Int(kind)))
            .expect("fault_kind is written");
        let (outcome, _) = run.scan_once(facts, 3, "the faulting scan");
        assert_eq!(
            outcome.map_err(|error| error.stable_code().as_str()),
            Err(code),
            "kind {kind}: the faulting scan"
        );
        let (outcome, _) = run.scan_once(facts, 4, "the scan after the fault");
        assert_eq!(
            outcome.map_err(|error| error.stable_code().as_str()),
            Err("runtime_resource_faulted"),
            "kind {kind}: a latched fault refuses the next scan"
        );
    }
}

#[test]
fn every_fault_kind_faults_as_the_value_store_faulted() {
    // Specification 34 MEM-05 and STATE-04 (the fault record), TIME-08 and the C1-9 note: a program that
    // faults on the flat store faults on the same instruction, with the same error, the same record, the
    // same values as the fault left them, and the same work and allocation charged up to it, as it did on
    // the `Value` store. The program is `faults`: its scan runs the healthy branch until the test writes
    // `fault_kind` through the engineering write, and kind 1 divides by zero, 2 reads outside
    // `ARRAY[1..4]` at index 9, 3 dereferences a null reference, 4 overflows a REAL (1.0E30 squared), 5
    // spins until the work allowance (50,000 units under the chips' limits) is spent, and 6 calls four
    // functions deep where the chips admit a depth of four. Each kind runs on a fresh store: two healthy
    // scans, the write, the faulting scan, and one more scan, which a latched fault refuses
    // (`runtime_resource_faulted`). A store whose work charges differed would spin a different number of
    // times before kind 5 faults, so `spins` would differ; one that moved a fault to another instruction
    // would differ in the record's site.
    let mut facts = Facts::compared();
    every_fault_kind(&mut facts);
    facts.finish("faults/");
}

/// The first two scans of `initializers` at every work allowance from one unit to the work one healthy
/// scan takes.
fn every_initializer_allowance(facts: &mut Facts) {
    let program = corpus("initializers");
    let healthy = {
        let prepared = prepare(program);
        let mut data = build(&prepared).expect("the store constructs");
        let mut state = EngineState::borrow(&mut data, &prepared, &services::LOGICAL_ONLY);
        let (outcome, _) = run_scan(&mut state, Duration::from_millis(10));
        outcome.expect("a healthy scan of initializers");
        // The allowance starts afresh for the program the scan runs.
        prepared.limits.max_work - state.data.resources.work_budget.remaining()
    };
    assert!(healthy > 0, "a scan of initializers charges work");
    facts.check(
        "initializer allowances/healthy",
        &[format!("work of a healthy scan {healthy}")],
    );
    for allowance in 1..=healthy {
        let limits = PreparationLimits {
            max_work: allowance,
            ..PreparationLimits::default()
        };
        let prepared = PreparedModule::from_bytes(program, limits).expect("the program prepares");
        let mut data = build(&prepared).expect("the store constructs");
        let mut run = Run {
            key: format!("initializer allowances/allowance {allowance}"),
            name: "initializers",
            prepared: &prepared,
            state: EngineState::borrow(&mut data, &prepared, &services::LOGICAL_ONLY),
        };
        for scan in 1..=2 {
            // The outcome itself may be a refusal; the facts record it.
            let _ = run.scan_once(facts, scan, &format!("scan {scan}"));
        }
    }
}

#[test]
fn an_initializer_evaluated_in_place_runs_out_of_work_where_the_value_store_did() {
    // Specification 34 TIME-08 and ARCH-05 (C1-13): the flat store stages the initializers of `Pick`'s
    // `window : ARRAY[1..2] OF INT := [INT#7, INT#8]` on the activation stack (the default constructed
    // in place, the seed copied from the variable, the array built where the result lies, the commit a
    // copy), and each step charges the work the value path charged, in the same order. So whatever the
    // scan's work allowance, the flat store ends the first two scans of `initializers` as the `Value`
    // store did: the same result, the same work left and bytes charged at every phase, and the same
    // values, as an allowance spent part way leaves them. The sweep runs from one unit (an allowance of
    // no work is not an admitted profile) to the work one healthy scan takes, which is recorded too. A
    // charge made earlier or later than the value path's would let the flat store run a step further or
    // shorter at some allowance, and the lines of that allowance would differ.
    let mut facts = Facts::compared();
    every_initializer_allowance(&mut facts);
    facts.finish("initializer allowances/");
}

/// Each retaining program at every construction work allowance, from one unit up to the first at which
/// it constructs and restarts warm.
fn every_restart_allowance(facts: &mut Facts) {
    for name in ["retained_blocks", "retained_values", "overlays", "unions"] {
        let program = corpus(name);
        let mut refusals = Vec::new();
        let mut restarted = false;
        for allowance in 1..=200_000 {
            let limits = PreparationLimits {
                max_construction_work: allowance,
                ..PreparationLimits::default()
            };
            let prepared =
                PreparedModule::from_bytes(program, limits).expect("the program prepares");
            let mut data = match build(&prepared) {
                Ok(data) => data,
                Err(error) => {
                    refusals.push(format!("allowance {allowance}: construction `{error:?}`"));
                    continue;
                }
            };
            let mut run = Run {
                key: format!("restart allowances/{name}/allowance {allowance}"),
                name,
                prepared: &prepared,
                state: EngineState::borrow(&mut data, &prepared, &services::LOGICAL_ONLY),
            };
            if !run.scans(facts, 1..=2, "") {
                break;
            }
            let restart = run.state.restart(RestartMode::Warm);
            let warm = restart.is_ok();
            let mut lines = vec![format!("restart {restart:?}")];
            lines.extend(left_behind(&prepared, &run.state));
            if !run.check(facts, "warm restart", &lines) {
                break;
            }
            if warm {
                restarted = true;
                break;
            }
        }
        // The refused constructions are one set: one line for each allowance below construction's work.
        facts.check(&format!("restart allowances/{name}/refused"), &refusals);
        assert!(restarted, "{name}: some allowance restarts the program");
    }
}

#[test]
fn a_warm_restart_copies_retained_cells_as_the_value_store_copied_values() {
    // Specification 34 §11.3, TIME-08 and MEM-12 (C1-14b): a warm restart carries the retained values
    // into the next generation. `retained_blocks` retains a function-block instance `kept`, the reference
    // `follow` to `kept.count`, `total` and its program's `runs`; `retained_values` retains arrays, a
    // structure and texts in globals and in its program; `overlays` retains a `STRUCT OVERLAP` with a text
    // beside its backing and `unions` a union with a text variant (C1-15). The flat store copies them from
    // cell to cell, an instance's members and a flat reference's generation included, and charges each
    // copy as the value store charged the copy of the value. So at every construction work allowance,
    // from one unit up to the first at which the program constructs and restarts warm, the flat store
    // ends as the `Value` store did: construction refused with the same error, or two scans and a warm
    // restart with the same result, the same work and bytes charged at each phase and the same values
    // after it. A copy charged in another order, a reference copied without its new generation, an
    // instance leaf without its new identity or a misplaced byte would make the lines of some allowance
    // differ.
    let mut facts = Facts::compared();
    every_restart_allowance(&mut facts);
    facts.finish("restart allowances/");
}

/// `io` with a byte-string window that ends at a NUL, then one that is no text.
fn byte_string_windows(facts: &mut Facts) {
    let prepared = PreparedModule::from_bytes(corpus("io"), chip_limits())
        .expect("the chips' admission accepts the io program");
    let mut data = build(&prepared).expect("the store constructs");
    let mut run = Run {
        key: "byte strings/io".into(),
        name: "io",
        prepared: &prepared,
        state: EngineState::borrow(&mut data, &prepared, &services::LOGICAL_ONLY),
    };
    run.constructed(facts);
    run.stage_hierarchical_inputs();
    run.scans(facts, 1..=2, "");
    let mut nul = [b'C'; 24];
    nul[..4].copy_from_slice(&[b'A', b'B', 0, 0xFF]);
    let mut invalid = [b'C'; 24];
    invalid[0] = 0xFF;
    for (scan, window, outcome) in [(3, nul, Ok(())), (4, invalid, Err("runtime_type_mismatch"))] {
        drive(&mut run.state, scan);
        run.state.data.images.inputs[12..36].copy_from_slice(&window);
        let (result, mut lines) = run_scan(&mut run.state, Duration::from_millis(10 * scan));
        assert_eq!(
            result.map_err(|error| error.stable_code().as_str()),
            outcome,
            "scan {scan}"
        );
        lines.extend(left_behind(&prepared, &run.state));
        run.check(facts, &format!("scan {scan}"), &lines);
        assert_eq!(
            member(&run.state, InstanceId(0), "label"),
            Value::String("AB".into()),
            "scan {scan}"
        );
        assert_eq!(
            member(&run.state, InstanceId(0), "level"),
            Value::Byte(b'A'),
            "scan {scan}"
        );
    }
}

#[test]
fn a_byte_string_input_ends_at_its_first_nul_and_refuses_invalid_text() {
    // Specification 11 §6.4: reading a byte-string window "stops at the first NUL byte; invalid UTF-8
    // before that terminator is a type error", and input synchronization is transactional: "a read,
    // conversion, undefined target, or null-reference failure leaves all bound program values
    // unchanged". Specification 34 §6.2 and TIME-08: the flat store, which writes a `STRING` input from
    // the image into the text where it lies (C1-16), reads, refuses and charges as the `Value` store did.
    // The program is `io`: `label AT %IB12 : STRING[24]` is bytes 12 to 35 of the input image, and the
    // body copies it to `echo AT %QB8 : STRING[8]`. After two healthy scans, scan 3 gives the window `AB`,
    // a NUL, then 0xFF and letters: the text is `AB`, so a store that read the whole window would refuse
    // the 0xFF, and one that ignored the NUL would keep the letters after it. Scan 4 gives a window that
    // starts with 0xFF: the scan is refused with the type error and no input is written, so `label` keeps
    // `AB`, and `level AT %IB1`, which the drive gives `B` (66) in scan 4, keeps the `A` (65) of scan 3;
    // a store that wrote the inputs sampled before the refused one would hold 66.
    let mut facts = Facts::compared();
    byte_string_windows(&mut facts);
    facts.finish("byte strings/");
}

/// Platform services that run the register tier with its profile on, so that a test sees the tier
/// ran, and with each block's compiled form from its first entry when `compiled` holds (C1-18).
#[cfg(feature = "register-ir")]
struct RegisterServices {
    compiled: bool,
}

#[cfg(feature = "register-ir")]
impl ExecutionServices for RegisterServices {
    fn register_configuration(&self) -> crate::vm::register::RegisterExecutionConfiguration {
        crate::vm::register::RegisterExecutionConfiguration {
            profile_enabled: true,
            tier1_enabled: self.compiled,
            tier1_hot_threshold: 1,
            ..Default::default()
        }
    }

    fn deadline_exceeded(&self) -> bool {
        false
    }

    fn has_wall_clock(&self) -> bool {
        false
    }

    fn current_dt(&self) -> Result<crate::value::DateTimeValue, RuntimeError> {
        Err(RuntimeError::UndefinedFunction("CURRENT_DT".into()))
    }
}

#[cfg(feature = "register-ir")]
static INTERPRETED: RegisterServices = RegisterServices { compiled: false };

#[cfg(feature = "register-ir")]
static COMPILED: RegisterServices = RegisterServices { compiled: true };

/// The corpus with the register tier, interpreted and compiled.
#[cfg(feature = "register-ir")]
fn both_register_forms(facts: &mut Facts) {
    for (services, form, index) in [
        (&INTERPRETED, "interpreted blocks", 0),
        (&COMPILED, "compiled blocks", 1),
    ] {
        let (refused, counts) = run_corpus(
            facts,
            &format!("register tier/{form}"),
            PreparationLimits::default,
            services,
        );
        assert_eq!(refused, Vec::<String>::new(), "{form}");
        assert!(counts[index] > 0, "the flat store ran {form}: {counts:?}");
    }
}

#[cfg(feature = "register-ir")]
#[test]
fn the_register_tier_runs_as_it_ran_on_the_value_store() {
    // Specification 34 §6.2, §6.3 and TIME-08 (C1-18): the hosted register tier runs on the flat store,
    // reading and writing a frame's typed locals on the activation stack and handing a call's result and
    // outputs from the finished frame's bytes to the caller, as the stack tier does (C1-11). Each corpus
    // program runs with the register tier, first interpreting its blocks (a call to a leaf POU runs in
    // place), then running each block's compiled form from its first entry, and is checked as
    // `every_corpus_program_runs_as_the_value_store_ran_it` checks it, charges included, against the
    // `Value` store's recorded lines. A register result read from the frame's own entries would return
    // `Null` from every function on the flat store, and outputs copied back from them would leave the
    // caller's variables `Null`; a register frame kept off the activation stack would leave its callees'
    // frames below their caller's. Two charges are checked too (TIME-08): a flat reference's path copy
    // charged by its empty path instead of the elements of the path it was taken through would charge
    // `arrays` and `copies` less in their compiled blocks, and an operand read from its cells within one
    // operation without the charge of the borrow it stands for would charge `operands` less (its
    // enumerations, WSTRINGs and references; a scalar's copy costs nothing). The counts prove the flat
    // store ran each form: its interpreter entered blocks, and compiled blocks ran.
    let mut facts = Facts::compared();
    both_register_forms(&mut facts);
    facts.finish("register tier/");
}

/// Record the facts of every test of this file.
pub(super) fn record(facts: &mut Facts) {
    every_corpus_program(facts);
    every_fault_kind(facts);
    every_initializer_allowance(facts);
    every_restart_allowance(facts);
    byte_string_windows(facts);
    #[cfg(feature = "register-ir")]
    both_register_forms(facts);
}
