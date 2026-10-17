//! Spec 34 ARCH-05, MEM-04 and MEM-09 (bounded operation, slice C1-1): the allocator calls that
//! every scan makes, split at the engine's own cycle boundaries, for the artifacts the boards run
//! and for the generated bounded-C corpus in `crates/trust-runtime/tests/fixtures/bounded-c/`.
//! Each corpus fixture is its own project folder holding `src/main.st` and the generated
//! `program.stbc`.
//!
//! A counting global allocator records `alloc`, `realloc` and `dealloc` on this thread only while
//! `execute_cycle_with_hooks` runs. Its hooks install no output callbacks, so the engine takes the
//! path of `execute_cycle`, which the chips call. Each run on `CEILINGS` must stay within its
//! allocator-call ceiling in every scan from its first listed scan; every other run is measured and
//! printed, so the later C1 slices lower the ceilings to zero as their allocations go. A ceiling
//! only ever goes down, an entry is only added or moved to an earlier first scan, and C1-19 requires
//! a ceiling of zero for every run from its first scan. The register tier (`register-ir`) is a
//! hosted executor that no chip links; with it the ceilings are printed, not asserted.
//!
//! The corpus is generated on the builder, never written by hand:
//!
//! ```sh
//! cargo run --locked -p trust-runtime --example bounded_c_fixtures -- crates/trust-runtime/tests/fixtures/bounded-c
//! ```
//!
//! Run the tests with `-- --nocapture` to see the counts.
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::fmt;
use std::path::Path;
use trust_runtime_core::{
    bytecode::ValidationLimits,
    error::RuntimeError,
    io_address::IoAddress,
    retain::RetainSnapshot,
    value::{DateTimeValue, Duration, Value},
    vm::{
        CycleAccess, CycleHooks, CyclePhase, ExecutionServices, PreparationLimits, PreparedModule,
        RuntimeState,
    },
};

/// Allocator calls counted while a scan runs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Calls {
    alloc: u64,
    realloc: u64,
    dealloc: u64,
}

impl Calls {
    const ZERO: Self = Self {
        alloc: 0,
        realloc: 0,
        dealloc: 0,
    };

    fn add(&mut self, other: Self) {
        self.alloc += other.alloc;
        self.realloc += other.realloc;
        self.dealloc += other.dealloc;
    }

    fn count(self) -> u64 {
        self.alloc + self.realloc + self.dealloc
    }
}

impl fmt::Display for Calls {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{}/{}/{}",
            self.alloc, self.realloc, self.dealloc
        )
    }
}

thread_local! {
    static ARMED: Cell<bool> = const { Cell::new(false) };
    static COUNTED: Cell<Calls> = const { Cell::new(Calls::ZERO) };
}

/// Count one call on this thread while it is armed. Only const-initialized `Cell`s are
/// touched, so counting never allocates, blocks or unwinds.
fn record(call: fn(&mut Calls)) {
    let _ = ARMED.try_with(|armed| {
        if armed.get() {
            let _ = COUNTED.try_with(|counted| {
                let mut calls = counted.get();
                call(&mut calls);
                counted.set(calls);
            });
        }
    });
}

struct CountingAllocator;

// SAFETY: every operation forwards the caller's unchanged layout, pointer and new size to
// `System`; the counting touches only const-initialized thread-local `Cell`s.
unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        record(|calls| calls.alloc = calls.alloc.saturating_add(1));
        // SAFETY: the caller supplies a valid allocation layout.
        unsafe { System.alloc(layout) }
    }

    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        record(|calls| calls.realloc = calls.realloc.saturating_add(1));
        // SAFETY: `System` returned `pointer` for `layout`; `size` is the caller's new size.
        unsafe { System.realloc(pointer, layout, size) }
    }

    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        record(|calls| calls.dealloc = calls.dealloc.saturating_add(1));
        // SAFETY: `pointer` and `layout` are the pair `System` returned.
        unsafe { System.dealloc(pointer, layout) }
    }
}

#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

/// Counting on this thread from creation until drop, starting from zero.
struct Armed;

impl Armed {
    fn new() -> Self {
        COUNTED.with(|counted| counted.set(Calls::ZERO));
        ARMED.with(|armed| armed.set(true));
        Self
    }
}

impl Drop for Armed {
    fn drop(&mut self) {
        ARMED.with(|armed| armed.set(false));
    }
}

/// The calls counted since the last take.
fn take_counted() -> Calls {
    COUNTED.with(|counted| counted.replace(Calls::ZERO))
}

/// The run phases of one scan, split at the engine's cycle boundaries (`CyclePhase`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Phase {
    /// Up to input sampling: the fault latch, logical time, allowance and deadline checks.
    Entry,
    /// Input and marker image sampling.
    Input,
    /// Task readiness and ordering, and the gaps between tasks.
    Schedule,
    /// Program and task function-block bodies.
    Tasks,
    /// Retain capture, then output and marker publication.
    Output,
}

const PHASES: [Phase; 5] = [
    Phase::Entry,
    Phase::Input,
    Phase::Schedule,
    Phase::Tasks,
    Phase::Output,
];

impl Phase {
    fn name(self) -> &'static str {
        match self {
            Self::Entry => "entry",
            Self::Input => "input",
            Self::Schedule => "schedule",
            Self::Tasks => "tasks",
            Self::Output => "output",
        }
    }

    /// The phase that a boundary opens.
    fn opened_by(boundary: CyclePhase) -> Self {
        match boundary {
            CyclePhase::CycleStart => Self::Entry,
            CyclePhase::BeforeInputSample => Self::Input,
            CyclePhase::AfterInputSample
            | CyclePhase::TaskOverrun { .. }
            | CyclePhase::TaskEnd(_)
            | CyclePhase::BackgroundEnd(_) => Self::Schedule,
            CyclePhase::TaskStart(_) | CyclePhase::BackgroundStart(_) => Self::Tasks,
            CyclePhase::BeforeOutputSample
            | CyclePhase::AfterOutputSample
            | CyclePhase::CycleEnd => Self::Output,
        }
    }
}

/// Cycle hooks that only split the counted calls at each boundary. Without output hooks the
/// engine publishes exactly as `execute_cycle` does.
struct PhaseMeter {
    phase: Phase,
    calls: [Calls; 5],
    activations: u32,
    /// Allocate and free once after every boundary, so a test can prove the attribution.
    probe: bool,
}

impl PhaseMeter {
    fn enter(&mut self, next: Phase) {
        self.calls[self.phase as usize].add(take_counted());
        self.phase = next;
    }
}

impl CycleHooks for PhaseMeter {
    fn has_output_hooks(&self) -> bool {
        false
    }

    fn on_phase(
        &mut self,
        boundary: CyclePhase,
        _access: &mut CycleAccess<'_, '_>,
    ) -> Result<(), RuntimeError> {
        self.enter(Phase::opened_by(boundary));
        if matches!(
            boundary,
            CyclePhase::TaskStart(_) | CyclePhase::BackgroundStart(_)
        ) {
            self.activations += 1;
        }
        if self.probe {
            drop(std::hint::black_box(Box::new(0u8)));
        }
        Ok(())
    }
}

/// One counted scan. A failing scan counts its fault latch in the phase that failed.
struct Scan {
    at_ms: i64,
    /// The stable code of the scan's error, if it failed.
    outcome: Option<&'static str>,
    calls: [Calls; 5],
    activations: u32,
}

impl Scan {
    fn sum(&self) -> Calls {
        let mut sum = Calls::ZERO;
        for calls in self.calls {
            sum.add(calls);
        }
        sum
    }
}

/// Run one scan at `at_ms` with counting armed only inside `execute_cycle_with_hooks`.
fn measure(state: &mut RuntimeState<'_>, at_ms: i64, probe: bool) -> Scan {
    let now = Duration::from_millis(at_ms);
    let mut meter = PhaseMeter {
        phase: Phase::Entry,
        calls: [Calls::ZERO; 5],
        activations: 0,
        probe,
    };
    let result = {
        let _armed = Armed::new();
        state.execute_cycle_with_hooks(now, &mut meter)
    };
    let last = meter.phase;
    meter.enter(last);
    Scan {
        at_ms,
        outcome: result.err().map(|error| error.stable_code().as_str()),
        calls: meter.calls,
        activations: meter.activations,
    }
}

/// The chips' execution services (`crates/trust-board-fixture/src/services.rs`): no UTC clock
/// and a deadline that a fault case can expire. A run may keep a retain store that saves nothing.
struct Services {
    expired: Cell<bool>,
    retain_store: bool,
}

impl Services {
    fn new(retain_store: bool) -> Self {
        Self {
            expired: Cell::new(false),
            retain_store,
        }
    }
}

impl ExecutionServices for Services {
    fn deadline_exceeded(&self) -> bool {
        self.expired.get()
    }
    fn has_wall_clock(&self) -> bool {
        false
    }
    fn has_retain_store(&self) -> bool {
        self.retain_store
    }
    fn save_retain_snapshot(&self, _snapshot: &RetainSnapshot) -> Result<(), RuntimeError> {
        Ok(())
    }
    fn current_dt(&self) -> Result<DateTimeValue, RuntimeError> {
        Err(RuntimeError::ProfileUnsupported("no UTC clock".into()))
    }
}

/// The chips' admission (`crates/trust-board-fixture/src/services.rs`, `limits`).
/// The limits each fixture run is prepared under: the chips' admission limits on the chips' stack
/// executor. The register tier (`register-ir`) is a hosted executor that no chip links, and its
/// lowering needs more preparation than the chips allow, so with it the fixture runs are prepared
/// under the hosted defaults; their ceilings are printed, not asserted.
fn run_limits() -> PreparationLimits {
    if cfg!(feature = "register-ir") {
        PreparationLimits::default()
    } else {
        chip_limits()
    }
}

/// The chips' admission limits. The fault cases always use them: a case such as `call_depth`
/// faults because of them.
fn chip_limits() -> PreparationLimits {
    PreparationLimits {
        max_artifact_bytes: 16 * 1024,
        max_preparation_bytes: 512 * 1024,
        max_preparation_work: 1_000_000,
        validation: ValidationLimits {
            max_scratch_bytes: 48 * 1024,
            max_work: 500_000,
        },
        max_construction_values: 4096,
        max_construction_bytes: 128 * 1024,
        max_process_image_bytes: 64,
        max_work: 50_000,
        max_construction_work: 50_000,
        max_call_depth: 4,
    }
}

const SAVED_MAIN: &[u8] =
    include_bytes!("../../trust-runtime/tests/fixtures/portability/stbc-2.0/program-v2.stbc");
const SAVED_NUMERIC: &[u8] =
    include_bytes!("../../trust-runtime/tests/fixtures/portability/stbc-2.0/numeric-v2.stbc");
const SAVED_GPIO: &[u8] =
    include_bytes!("../../trust-runtime/tests/fixtures/portability/f401/gpio.stbc");

const CORPUS: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../trust-runtime/tests/fixtures/bounded-c"
);
const GENERATE: &str = "cargo run --locked -p trust-runtime --example bounded_c_fixtures -- \
                        crates/trust-runtime/tests/fixtures/bounded-c";

/// Where a fixture's artifact comes from.
#[derive(Clone, Copy)]
enum Artifact {
    /// Bytes the boards run, saved in the repository.
    Saved(&'static [u8]),
    /// `<name>/program.stbc` of the generated corpus.
    Corpus,
}

/// One measured application and how the test drives it.
#[derive(Clone, Copy)]
struct Fixture {
    name: &'static str,
    artifact: Artifact,
    /// Logical time of the first scan; the others follow 10 ms apart.
    first_ms: i64,
    scans: i64,
    /// Hierarchical inputs given the value TRUE once, before the first scan.
    hierarchical_inputs: &'static [&'static str],
    /// The services keep a retain store, so every scan captures retained values (census S9).
    retain_store: bool,
}

impl Fixture {
    const fn corpus(name: &'static str) -> Self {
        Self {
            name,
            artifact: Artifact::Corpus,
            first_ms: 10,
            scans: 20,
            hierarchical_inputs: &[],
            retain_store: false,
        }
    }

    const fn saved(name: &'static str, bytes: &'static [u8], first_ms: i64, scans: i64) -> Self {
        Self {
            artifact: Artifact::Saved(bytes),
            first_ms,
            scans,
            ..Self::corpus(name)
        }
    }
}

/// Every measured application. Each corpus program runs in a 10 ms task, so each of its scans
/// activates it once. The saved main and numeric artifacts keep the boards' schedule: 101 scans
/// from 0 ms to 1,000 ms with a 25 ms task.
static FIXTURES: [Fixture; 17] = [
    Fixture::saved("saved/main", SAVED_MAIN, 0, 101),
    Fixture::saved("saved/numeric", SAVED_NUMERIC, 0, 101),
    Fixture::saved("saved/gpio", SAVED_GPIO, 10, 20),
    Fixture::corpus("scalar"),
    Fixture::corpus("gpio"),
    Fixture {
        hierarchical_inputs: &["%IX1.2.3"],
        ..Fixture::corpus("io")
    },
    Fixture::corpus("calls"),
    Fixture::corpus("blocks"),
    Fixture::corpus("methods"),
    Fixture::corpus("instances"),
    Fixture::corpus("arrays"),
    Fixture::corpus("structures"),
    Fixture::corpus("references"),
    Fixture::corpus("strings"),
    Fixture::corpus("initializers"),
    Fixture::corpus("sizeof"),
    Fixture {
        retain_store: true,
        ..Fixture::corpus("retain")
    },
];

/// How a fault case makes its scan fail.
#[derive(Clone, Copy)]
enum Trigger {
    /// Select `fault_kind` in the corpus' faults program before the scan.
    Program(i16),
    /// Expire the physical deadline before the scan.
    Deadline,
    /// Scan at an earlier logical time than the last scan (census S10). This is refused without
    /// latching a fault.
    TimeBackwards,
}

/// One faulting scan of the corpus' faults program and the stable code it must end with.
struct FaultCase {
    name: &'static str,
    trigger: Trigger,
    code: &'static str,
}

static FAULT_CASES: [FaultCase; 8] = [
    FaultCase {
        name: "faults/division",
        trigger: Trigger::Program(1),
        code: "runtime_division_by_zero",
    },
    FaultCase {
        name: "faults/index",
        trigger: Trigger::Program(2),
        code: "runtime_index_out_of_bounds",
    },
    FaultCase {
        name: "faults/null",
        trigger: Trigger::Program(3),
        code: "runtime_null_reference",
    },
    FaultCase {
        name: "faults/overflow",
        trigger: Trigger::Program(4),
        code: "runtime_overflow",
    },
    FaultCase {
        name: "faults/work",
        trigger: Trigger::Program(5),
        code: "runtime_execution_timeout",
    },
    FaultCase {
        name: "faults/call_depth",
        trigger: Trigger::Program(6),
        code: "vm_call_stack_overflow",
    },
    FaultCase {
        name: "faults/deadline",
        trigger: Trigger::Deadline,
        code: "runtime_execution_timeout",
    },
    FaultCase {
        name: "faults/time_backwards",
        trigger: Trigger::TimeBackwards,
        code: "runtime_invalid_config",
    },
];

/// Runs whose allocator calls in every scan, from the given scan on, stay within a ceiling, where
/// 0 is the first scan after construction. Zero is the goal (spec 34 MEM-04): a ceiling only goes
/// down. The scalar program still allocates and frees one buffer in the tasks phase of every scan
/// (measured on the builder, batch c1-2, 17 October 2026); C1-2 reserves it at construction and
/// lowers the ceiling to zero. Its first scan is left out: it may still grow the operand and frame
/// stacks and, with `std`, the storage field caches (spec 34 MEM-09 first-use growth).
const CEILINGS: [(&str, usize, Calls); 1] = [(
    "scalar",
    1,
    Calls {
        alloc: 1,
        realloc: 0,
        dealloc: 1,
    },
)];

/// The ceilings are asserted on the chips' stack executor and printed with the register tier.
const CEILINGS_ASSERTED: bool = cfg!(not(feature = "register-ir"));

fn ceiling_of(name: &str) -> Option<(usize, Calls)> {
    CEILINGS
        .iter()
        .find(|(entry, _, _)| *entry == name)
        .map(|(_, from, ceiling)| (*from, *ceiling))
}

/// Every scan of a listed run that made more allocator calls than its ceiling, from its first
/// listed scan on.
fn ceiling_violations(name: &str, scans: &[Scan]) -> Vec<String> {
    let Some((from, ceiling)) = ceiling_of(name) else {
        return Vec::new();
    };
    if !CEILINGS_ASSERTED {
        return Vec::new();
    }
    if scans.len() <= from {
        return vec![format!("{name}: the listed run has no scan {from}")];
    }
    let mut violations = Vec::new();
    for (index, scan) in scans.iter().enumerate().skip(from) {
        let calls = scan.sum();
        if calls.alloc > ceiling.alloc
            || calls.realloc > ceiling.realloc
            || calls.dealloc > ceiling.dealloc
        {
            violations.push(format!(
                "{name}: scan {index} at {} ms made {calls} alloc/realloc/dealloc calls, above \
                 its ceiling of {ceiling} ({})",
                scan.at_ms,
                phase_fields(&scan.calls)
            ));
        }
    }
    violations
}

/// `<phase>=<alloc>/<realloc>/<dealloc>` for each phase.
fn phase_fields(calls: &[Calls; 5]) -> String {
    PHASES
        .iter()
        .zip(calls)
        .map(|(phase, phase_calls)| format!("{}={phase_calls}", phase.name()))
        .collect::<Vec<_>>()
        .join(",")
}

/// Print one run: its first scan, the later scans, each phase over all scans, and every scan's
/// number of calls.
fn report(name: &str, scans: &[Scan]) {
    let mut phases = [Calls::ZERO; 5];
    let mut later = Calls::ZERO;
    for (index, scan) in scans.iter().enumerate() {
        for (sum, calls) in phases.iter_mut().zip(scan.calls) {
            sum.add(calls);
        }
        if index > 0 {
            later.add(scan.sum());
        }
    }
    let first = scans.first().map_or(Calls::ZERO, Scan::sum);
    let activations: u32 = scans.iter().map(|scan| scan.activations).sum();
    let per_scan = scans
        .iter()
        .map(|scan| scan.sum().count().to_string())
        .collect::<Vec<_>>()
        .join(" ");
    let ceiling = match ceiling_of(name) {
        None => "none".to_owned(),
        Some((from, ceiling)) if CEILINGS_ASSERTED => {
            format!("{ceiling} asserted from scan {from}")
        }
        Some((from, ceiling)) => format!("{ceiling} printed from scan {from} (register-ir)"),
    };
    eprintln!(
        "scan-allocations,{name},scans={},activations={activations},first={first},\
         later={later},{},ceiling={ceiling},per_scan={per_scan}",
        scans.len(),
        phase_fields(&phases)
    );
}

/// The generated `<name>/program.stbc`. A missing artifact fails with the command that generates
/// the corpus, so the test never passes without it.
fn corpus_artifact(name: &str) -> Result<Vec<u8>, String> {
    let path = Path::new(CORPUS).join(name).join("program.stbc");
    std::fs::read(&path).map_err(|error| {
        format!(
            "{} is missing ({error}); generate the bounded-C corpus on the builder, from the \
             workspace root: {GENERATE}",
            path.display()
        )
    })
}

/// Every corpus fixture folder the tests measure.
fn corpus_names() -> impl Iterator<Item = &'static str> {
    FIXTURES
        .iter()
        .filter(|fixture| matches!(fixture.artifact, Artifact::Corpus))
        .map(|fixture| fixture.name)
        .chain(["faults"])
}

/// Entries of the corpus directory that are not a fixture folder that a test measures.
fn unmeasured_entries() -> Vec<String> {
    let entries = std::fs::read_dir(CORPUS).unwrap_or_else(|error| {
        panic!(
            "the bounded-C corpus {CORPUS} is missing ({error}); generate it on the builder, \
             from the workspace root: {GENERATE}"
        )
    });
    let mut unmeasured = Vec::new();
    for entry in entries {
        let entry = entry.expect("readable corpus directory entry");
        let name = entry.file_name();
        let name = name.to_string_lossy();
        let folder = entry.file_type().is_ok_and(|kind| kind.is_dir());
        if !folder || !corpus_names().any(|known| known == name.as_ref()) {
            unmeasured.push(name.to_string());
        }
    }
    unmeasured
}

/// Prepare and construct a fixture outside the counted window, then count every scan. Before
/// each scan every input byte is set to alternating ASCII, so each bound bit toggles and every
/// STRING window stays valid text.
fn run_fixture(fixture: &Fixture) -> Result<Vec<Scan>, String> {
    let bytes = match fixture.artifact {
        Artifact::Saved(bytes) => bytes.to_vec(),
        Artifact::Corpus => corpus_artifact(fixture.name)?,
    };
    let prepared = PreparedModule::from_bytes(&bytes, run_limits())
        .map_err(|error| format!("preparation refused: {error}"))?;
    let services = Services::new(fixture.retain_store);
    let mut state = prepared
        .instantiate_with_services(0, &services)
        .map_err(|error| format!("construction refused: {error}"))?;
    for address in fixture.hierarchical_inputs {
        IoAddress::parse(address)
            .and_then(|parsed| state.set_hierarchical_input(&parsed, Value::Bool(true)))
            .map_err(|error| format!("hierarchical input {address} refused: {error}"))?;
    }
    let mut scans = Vec::new();
    for index in 0..fixture.scans {
        state
            .inputs_mut()
            .fill(if index % 2 == 0 { b'A' } else { b'B' });
        let scan = measure(&mut state, fixture.first_ms + 10 * index, false);
        if let Some(code) = scan.outcome {
            return Err(format!("scan {index} at {} ms failed: {code}", scan.at_ms));
        }
        scans.push(scan);
    }
    let activations: u32 = scans.iter().map(|scan| scan.activations).sum();
    if activations == 0 {
        return Err("no scan activated a task".to_owned());
    }
    // A corpus program counts its own activations in `ticks`, so the scans ran its body.
    if let Some(ticks) = state.storage().get_global("ticks")
        && *ticks != Value::DInt(activations as i32)
    {
        return Err(format!(
            "ticks is {ticks:?} after {activations} task activations"
        ));
    }
    Ok(scans)
}

/// Two healthy scans, the faulting scan and one more scan of a fresh engine, all counted.
fn run_fault_case(prepared: &PreparedModule, case: &FaultCase) -> Result<Vec<Scan>, String> {
    let services = Services::new(false);
    let mut state = prepared
        .instantiate_with_services(0, &services)
        .map_err(|error| format!("construction refused: {error}"))?;
    let mut scans = Vec::new();
    for at_ms in [10, 20] {
        let scan = measure(&mut state, at_ms, false);
        if let Some(code) = scan.outcome {
            return Err(format!("the healthy scan at {at_ms} ms failed: {code}"));
        }
        scans.push(scan);
    }
    let at_ms = match case.trigger {
        Trigger::Program(kind) => {
            state
                .write_global("fault_kind", Value::Int(kind))
                .map_err(|error| format!("fault_kind write refused: {error}"))?;
            30
        }
        Trigger::Deadline => {
            services.expired.set(true);
            30
        }
        Trigger::TimeBackwards => 15,
    };
    let faulting = measure(&mut state, at_ms, false);
    services.expired.set(false);
    if faulting.outcome != Some(case.code) {
        return Err(format!(
            "the faulting scan ended with {:?}, not {}",
            faulting.outcome, case.code
        ));
    }
    eprintln!(
        "scan-allocations-fault,{},code={},faulting={},{}",
        case.name,
        case.code,
        faulting.sum(),
        phase_fields(&faulting.calls)
    );
    scans.push(faulting);
    // A latched fault refuses the next scan; a time step backwards latches nothing.
    let after = measure(&mut state, 40, false);
    let expected =
        (!matches!(case.trigger, Trigger::TimeBackwards)).then_some("runtime_resource_faulted");
    if after.outcome != expected {
        return Err(format!(
            "the scan after the fault ended with {:?}, not {expected:?}",
            after.outcome
        ));
    }
    scans.push(after);
    Ok(scans)
}

/// Spec 34 ARCH-05, MEM-04 and MEM-09: every fixture runs its scans under the chips' admission
/// limits, the zero-list runs make no allocator call, and every run's counts are printed.
#[test]
fn listed_runs_stay_within_their_allocator_ceilings_and_every_fixture_reports_its_scans() {
    let unmeasured = unmeasured_entries();
    assert!(
        unmeasured.is_empty(),
        "the corpus holds entries that no test measures: {unmeasured:?}"
    );
    for (name, _, _) in CEILINGS {
        assert!(
            FIXTURES.iter().any(|fixture| fixture.name == name)
                || FAULT_CASES.iter().any(|case| case.name == name),
            "the ceiling entry {name} names no run"
        );
    }
    eprintln!(
        "scan-allocations-legend,calls=alloc/realloc/dealloc,phases=entry input schedule tasks \
         output,per_scan=allocator calls of each scan"
    );
    let mut failures = Vec::new();
    for fixture in &FIXTURES {
        match run_fixture(fixture) {
            Ok(scans) => {
                report(fixture.name, &scans);
                failures.extend(ceiling_violations(fixture.name, &scans));
            }
            Err(error) => failures.push(format!("{}: {error}", fixture.name)),
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// Spec 34 ARCH-05 and MEM-04 (fault and limit paths): each fault case's scan ends with its
/// stable code, a latched fault refuses the next scan, and every scan's counts are printed.
#[test]
fn every_fault_case_ends_with_its_stable_code_and_reports_its_scans() {
    let bytes = corpus_artifact("faults").unwrap_or_else(|error| panic!("{error}"));
    let prepared = PreparedModule::from_bytes(&bytes, chip_limits())
        .expect("the chips' admission accepts the faults program");
    let mut failures = Vec::new();
    for case in &FAULT_CASES {
        match run_fault_case(&prepared, case) {
            Ok(scans) => {
                report(case.name, &scans);
                failures.extend(ceiling_violations(case.name, &scans));
            }
            Err(error) => failures.push(format!("{}: {error}", case.name)),
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// The meter counts this thread's allocator calls exactly and only while armed, and gives each
/// call to the phase that makes it: a probe that allocates once after every boundary adds one
/// call pair to the phase each boundary opens. Uses the saved board GPIO artifact, so it runs
/// without the corpus.
#[test]
// The second push must grow the vector: the test counts exactly that reallocation.
#[allow(clippy::vec_init_then_push)]
fn the_meter_counts_armed_calls_exactly_and_in_the_phase_that_makes_them() {
    let growth = {
        let _armed = Armed::new();
        let mut values = Vec::<u64>::with_capacity(1);
        values.push(1);
        values.push(2);
        drop(std::hint::black_box(values));
        take_counted()
    };
    assert_eq!(
        growth,
        Calls {
            alloc: 1,
            realloc: 1,
            dealloc: 1
        }
    );
    let outside = Box::new(7u64);
    let idle = {
        let _armed = Armed::new();
        take_counted()
    };
    drop(outside);
    assert_eq!(idle, Calls::ZERO);

    let prepared = PreparedModule::from_bytes(SAVED_GPIO, run_limits())
        .expect("the chips' admission accepts the board GPIO artifact");
    let services = Services::new(false);
    let mut state = prepared
        .instantiate_with_services(0, &services)
        .expect("the board GPIO artifact constructs");
    for at_ms in [10, 20, 30] {
        assert_eq!(measure(&mut state, at_ms, false).outcome, None);
    }
    let plain = measure(&mut state, 40, false);
    let probed = measure(&mut state, 50, true);
    assert_eq!((plain.outcome, probed.outcome), (None, None));
    assert_eq!((plain.activations, probed.activations), (1, 1));
    // CycleStart opens entry, BeforeInputSample input, AfterInputSample and TaskEnd schedule,
    // TaskStart tasks and BeforeOutputSample output.
    let boundaries = [1, 1, 2, 1, 1];
    for ((phase, (without, with)), opened) in PHASES
        .iter()
        .zip(plain.calls.iter().zip(probed.calls))
        .zip(boundaries)
    {
        let expected = Calls {
            alloc: without.alloc + opened,
            realloc: without.realloc,
            dealloc: without.dealloc + opened,
        };
        assert_eq!(with, expected, "phase {}", phase.name());
    }
}
