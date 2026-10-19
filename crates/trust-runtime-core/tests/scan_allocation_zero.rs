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
    retain::{RestartMode, RetainSnapshot},
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
            trace_call();
        }
    });
}

thread_local! {
    /// Whether the tracing test records where each counted call comes from.
    static TRACING: Cell<bool> = const { Cell::new(false) };
    /// Set while a trace is captured: the capture's own allocations are neither counted nor traced.
    static IN_TRACE: Cell<bool> = const { Cell::new(false) };
    static TRACES: std::cell::RefCell<Vec<String>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// Record the engine frames that made the counted call, when the tracing test asked for them.
fn trace_call() {
    if !TRACING.try_with(Cell::get).unwrap_or(false) || IN_TRACE.try_with(Cell::get).unwrap_or(true)
    {
        return;
    }
    let _ = IN_TRACE.try_with(|flag| flag.set(true));
    let _ = ARMED.try_with(|armed| {
        let was = armed.replace(false);
        let trace = std::backtrace::Backtrace::force_capture().to_string();
        let mut names = trace
            .lines()
            .filter(|line| line.contains("trust_runtime_core::") && !line.contains(" at "))
            .map(|line| {
                line.trim()
                    .split_once(": ")
                    .map_or(line.trim(), |(_, name)| name)
            });
        // The innermost frame names what allocates; the engine's own functions then say why.
        let frames = names
            .next()
            .into_iter()
            .chain(
                names
                    .filter(|name| {
                        name.starts_with("trust_runtime_core::")
                            || name.starts_with("<trust_runtime_core::")
                    })
                    .take(8),
            )
            .collect::<Vec<_>>()
            .join(" <- ");
        let _ = TRACES.try_with(|traces| traces.borrow_mut().push(frames));
        armed.set(was);
    });
    let _ = IN_TRACE.try_with(|flag| flag.set(false));
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
/// and a deadline that a fault case can expire. A run may keep a retain store that persists
/// through the shared capture.
struct Services {
    expired: Cell<bool>,
    retain_store: bool,
    retain: std::cell::RefCell<trust_runtime_core::retain::capture::RetainCapture>,
}

impl Services {
    fn new(retain_store: bool) -> Self {
        let mut retain = trust_runtime_core::retain::capture::RetainCapture::new();
        // The chips' policy: once the retained values are dirty, a scan is due to capture (census
        // row S9). The capture refills its reserved payload buffer in place, so after the first
        // scan a scan allocates nothing (spec 34 MEM-04).
        retain.configure(Some(Duration::ZERO), Duration::ZERO);
        Self {
            expired: Cell::new(false),
            retain_store,
            retain: std::cell::RefCell::new(retain),
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
    fn retain_capture(
        &self,
    ) -> Option<&std::cell::RefCell<trust_runtime_core::retain::capture::RetainCapture>> {
        self.retain_store.then_some(&self.retain)
    }
    fn save_retain_payload(&self, _payload: &[u8]) -> Result<(), RuntimeError> {
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
static FIXTURES: [Fixture; 25] = [
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
    // Every program with retained variables keeps a retain store, as a board does, so each of
    // its scans captures them (C1-19c4).
    Fixture {
        retain_store: true,
        ..Fixture::corpus("retained_blocks")
    },
    Fixture {
        retain_store: true,
        ..Fixture::corpus("retained_values")
    },
    Fixture::corpus("copies"),
    Fixture {
        retain_store: true,
        ..Fixture::corpus("overlays")
    },
    Fixture {
        retain_store: true,
        ..Fixture::corpus("unions")
    },
    Fixture::corpus("operands"),
    Fixture::corpus("results"),
    Fixture::corpus("bindings"),
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
/// down. C1-2 reserves the stack executor's buffer, the program activation's local slots and the
/// process-image staging at construction, so the scalar program (one buffer per scan before, batch
/// c1-2, 17 October 2026) and both GPIO artifacts (8 calls per scan in input and output staging)
/// make none. C1-3b resolves each native call once, at preparation, so a call no longer builds an
/// uppercase type-name key or a named-argument index vector; C1-3c gives a call frame the payload of
/// its depth from construction's reservation, takes a user function's or method's locals from those
/// payloads and reads an edge-affected call's inputs and restores from reserved lists (census A3,
/// A4, A5, C5). The call fixtures still allocate their argument and output vectors (census C1,
/// C4, C7 to C11), which the call-scratch slice removes. C1-4 reserves initializer evaluation,
/// construction frames, the lifetime check and `SIZEOF` at construction, so the `sizeof` fixture
/// makes none from its first scan; the `initializers` fixture's own bookkeeping makes none, and its
/// remaining calls per scan are the function and method call scratch. The first scan is left out:
/// with `std` it may still fill the storage field caches (spec 34 MEM-09 first-use growth).
///
/// C3-1b (spec 34 MEM-05, STATE-04) adds the corpus `faults` cases: a faulting scan latches a
/// fixed-size record built from the error by reference, so it allocates nothing once the engine is
/// warm. Their ceiling starts at the third scan, the one that faults; the two healthy scans before
/// it are not the measured scan, and the healthy branch's dynamic index still allocates its
/// reference path (census C1-3). The five cases that reach their fault inside one activation make
/// no call: division, index, null, overflow and work. `faults/call_depth` builds four function
/// activations before its fifth call is refused, so its ceiling records the call setup's remaining
/// allocations (census C1, removed by the call-scratch slice), not the latch's.
///
/// C1-3c2 reserves each call depth's binding scratch at construction, so a call takes
/// its argument list, parameter flags, output bindings and prepared targets and outputs from that
/// scratch (census C1, C3, C4, C5, C7). This slice gives the scratch a standard-library value kind,
/// so a standard function's arguments and a split function's outputs bind into the call depth's
/// reserved values and its named arguments match without building an uppercase name (census C9,
/// C10), and the call fixtures' later scans allocate only what the later slices leave: a reference
/// output target's copied path (C8) and the output journal's destination root snapshots (C11); the
/// text functions' own results (C12, C13) do not appear, because the fixture calls no text
/// function. Their ceilings were 34, 13 and 12 allocator calls, measured on the builder. The first
/// scan is left out: with `std` it may still fill the storage field caches (spec 34 MEM-09 first-use
/// growth).
///
/// C1-10 keeps the path and index buffers of the references a scan builds and takes them back when
/// a load or a store through a reference is done, so a dynamic index or member, a static path and a
/// loaded reference variable allocate nothing once the engine is warm (spec 34 ARCH-05): `calls`
/// falls to 28, `methods` to 9 and `initializers` to 19 allocator calls; the `arrays` fixture went
/// from 62 to 6 and `references` from 18 to 7 on the flat store.
///
/// C1-10c leaves an array or a structure of scalars where it lies until a store copies its bytes,
/// and gives the path buffers of a stored reference back when a write replaces it, and of the
/// references that `=`, `<>` and a refused `?=` consume. On the flat store `arrays` and
/// `references` then make no allocator call after the first scan and `retained_blocks` makes 2;
/// the `copies` fixture's 30 per scan are its function call and its structure that holds a text
/// (C1-11, C1-12), measured on the builder on 19 October 2026.
///
/// C1-11 gives each activation its presence bits from the reserved frame payloads (census A4), finds a
/// method's or a function block's type by name in any case without an upper-case copy, matches named
/// arguments without an index vector (census C6), keeps every call's locals on the activation stack and
/// makes a reference to a whole writable persistent target flat, so a stored, copied or compared
/// reference holds no path. On the value store `calls` falls to 20, `methods` to 1 and
/// `initializers` to 17; on the flat store `calls` to 1, `blocks` and `retained_blocks` to 0,
/// `methods` to 1, `instances` to 16 (its per-call class instance, C1-11c), `initializers` to 17 and
/// `copies` to 8, measured on the builder on 19 October 2026.
///
/// C1-11c puts the function-block and class instances a call creates on the activation stack, keeps
/// the instance lists of the frames that ended for the next frame and walks a promoted value's
/// instances in a reserved set: on the flat store `instances` falls from 16 to 2, the staged result
/// of the initializer that creates the instance (C1-13), and on the value store from 16 to 10,
/// measured on the builder on 19 October 2026.
///
/// C1-12a and C1-12b leave a text, and a structure that holds one, where it lies, and run the
/// built-in text functions on their arguments where they lie, their results in the operand stack's
/// text area, a variable argument read where its target lies: on the flat store `strings` falls
/// from 40 to 10 allocator calls and from 14 to 1 reallocation per scan, `copies` from 8 to 5 and
/// `io` from 8 to 7. C1-12c runs the conversions from and to texts the same way, loads `WSTRING`
/// constants into the text area and replaces a written character in its leaf: `strings` makes no
/// allocator call after its first scan, measured on the builder on 19 October 2026.
///
/// C1-13a gives the path buffers of the references a finished activation's locals hold back to the
/// engine and keeps the staged initializer slot's buffer in the scratch: `instances` makes no
/// allocator call after its first scan on the flat store, and `initializers` falls from 17 to 13 on
/// both stores (its array initializer, C1-13b), measured on the builder on 19 October 2026.
///
/// C1-11e pushes the frame of a call that hands it its own locals without a reserved payload, so
/// the payloads reserved per call depth cover the deepest call: on both stores `methods` makes no
/// allocator call after its first scan, `calls` falls from 20 to 19 on the value store and from 1 to
/// 0 on the flat store, `initializers` from 13 to 12 and, on the flat store, `copies` from 5 to 4,
/// measured on the builder on 19 October 2026.
///
/// C1-13b1 builds the raw arrays and structures of initializer bodies in kept containers, which the
/// construction that consumes a raw gives back: `initializers` falls from 12 to 9 on both stores,
/// measured on the builder on 19 October 2026.
///
/// C1-13b2 stages the result of a frame variable's array, structure or text initializer on the
/// activation stack of the flat store: its default is constructed there, its seed copied, the typed
/// value its body builds written there, and the commit copies its bytes into the variable. On the
/// flat store `initializers` makes no allocator call after its first scan, measured on the builder
/// on 19 October 2026.
///
/// C1-14b adds `retained_values`, whose retained arrays, structure and texts a warm restart carries as
/// their bytes. On the flat store its scans write each of them in place; the first scan through its
/// text branch, scan 1, grows the text buffers once, so its ceiling of zero starts at scan 2,
/// measured on the builder on 19 October 2026.
///
/// C1-15 places a `STRUCT OVERLAP` on the flat store as its backing, and C1-15b reads and writes its
/// leaves, copies it and the structures nested in it, and builds the default of a function's local
/// overlay, where they lie. Left on the flat store for `structures` and `overlays`: the boxes of
/// their enumeration values (7 calls each way per scan), measured on the builder on 19 October 2026.
/// `unions` lays each union out as a structure its variants: its scans write them in place, and the
/// first scan through its text branch, scan 2, grows the text buffers once, so its ceiling of zero
/// starts at scan 3.
///
/// C1-18 adds `operands`, whose comparisons of globals the register tier reads within one
/// operation.
///
/// C1-19a pushes an enumeration as its type and number on the flat store, so its loads, stores and
/// comparisons build no boxed value: `structures` falls from 7 to 0, `overlays` from 7 to 1 (a call
/// target's path) and `copies` from 4 to 3 (a structure read as a value); `operands` makes 2 (its
/// WSTRINGs read as values for `=`), measured on the builder on 19 October 2026.
///
/// C1-19b reads a call target's value without copying its path, compares two texts with `=` and
/// `<>` where they lie, and copies the bytes of an array or a structure that a call leaves on the
/// operand stack into the stack's temporary area, where the store after the call finds them: on the
/// flat store `copies`, `overlays` and `operands` make no allocator call after their first scan.
///
/// C1-19c reserves at construction what the first scans grew: the locals and presence bits of every
/// call level, the execution buffers of initializer bodies, the instance bookkeeping of a call
/// that creates instances, the output journal of a native call, the hierarchical image keys, the
/// text a character write edits and the retain capture. On the flat store `scalar`, `gpio`, the
/// saved `gpio` and `numeric`, `sizeof`, `methods`, `instances` and `retain` make no allocator call
/// in any scan, the first included, measured on the builder on 19 October 2026.
///
/// C1-19c2 bounds, by a walk of every body at preparation, the texts and copies the operand stack's
/// temporary area holds at once and the text-call buffers, which construction reserves; a call that
/// takes texts as values now gives their room back. On the flat store `operands` makes no allocator
/// call in any scan, and `retained_values` and `unions` none after their first scan, whose text
/// branches grew the buffers before; what the first scans still allocate are reference path and
/// index buffers, measured on the builder on 19 October 2026.
///
/// C1-19c4 adds the design's corpus cases the corpus lacked: `results` (functions that take and
/// give texts, arrays, structures and references) and `bindings` (an output into a structure's
/// member, a per-call instance and a local array that hold references, a VAR_IN_OUT bound to an
/// element of a frame's own array, a long member name reached through an index, a structure
/// holding a reference assigned whole), and gives every program with retained variables a retain
/// store, as a board has. On the flat store `results` makes 18 allocator calls and as many frees
/// in every scan, `bindings` as many after its first, and `overlays`, whose retained overlapping
/// structure is captured through its value, 24/12/24, measured on the builder on 19 October 2026.
///
/// C1-19c3 reserves the reference path and index buffers for every reference a scan can hold at
/// once: the walk's reference operands on every call level, the references frame variables hold
/// and the persistent reference leaves, each buffer with room for the longest path. Every corpus
/// run on the flat store, the saved `main` included, makes no allocator call in any scan, the
/// first included, measured on the builder on 19 October 2026.
const CEILINGS: [(&str, usize, Calls); 39] = [
    ("faults/division", 2, Calls::ZERO),
    ("faults/index", 2, Calls::ZERO),
    ("faults/null", 2, Calls::ZERO),
    ("faults/overflow", 2, Calls::ZERO),
    ("faults/work", 2, Calls::ZERO),
    (
        "faults/call_depth",
        2,
        Calls {
            alloc: 11,
            realloc: 0,
            dealloc: 8,
        },
    ),
    ("scalar", 1, Calls::ZERO),
    ("gpio", 1, Calls::ZERO),
    ("saved/gpio", 1, Calls::ZERO),
    (
        "initializers",
        1,
        Calls {
            alloc: 9,
            realloc: 0,
            dealloc: 9,
        },
    ),
    ("sizeof", 1, Calls::ZERO),
    (
        "calls",
        1,
        Calls {
            alloc: 19,
            realloc: 0,
            dealloc: 19,
        },
    ),
    (
        "blocks",
        1,
        Calls {
            alloc: 13,
            realloc: 0,
            dealloc: 13,
        },
    ),
    ("methods", 1, Calls::ZERO),
    // The same runs on the flat store (C1-10 on): those at zero, and each other at the allocator
    // calls of its steady scans when the reference buffers were kept (19 October 2026).
    ("cells/saved/main", 0, Calls::ZERO),
    ("cells/scalar", 0, Calls::ZERO),
    ("cells/gpio", 0, Calls::ZERO),
    ("cells/saved/gpio", 0, Calls::ZERO),
    ("cells/saved/numeric", 0, Calls::ZERO),
    ("cells/sizeof", 0, Calls::ZERO),
    ("cells/retain", 0, Calls::ZERO),
    ("cells/io", 0, Calls::ZERO),
    ("cells/calls", 0, Calls::ZERO),
    ("cells/blocks", 0, Calls::ZERO),
    ("cells/methods", 0, Calls::ZERO),
    ("cells/instances", 0, Calls::ZERO),
    ("cells/arrays", 0, Calls::ZERO),
    ("cells/references", 0, Calls::ZERO),
    ("cells/strings", 0, Calls::ZERO),
    ("cells/initializers", 0, Calls::ZERO),
    ("cells/retained_blocks", 0, Calls::ZERO),
    ("cells/retained_values", 0, Calls::ZERO),
    ("cells/copies", 0, Calls::ZERO),
    ("cells/structures", 0, Calls::ZERO),
    ("cells/overlays", 0, Calls::ZERO),
    ("cells/unions", 0, Calls::ZERO),
    ("cells/operands", 0, Calls::ZERO),
    ("cells/results", 0, Calls::ZERO),
    // A structure holding a reference assigned whole, a frame variable and a per-call instance
    // holding references (C1-19c4 removes them).
    (
        "cells/bindings",
        1,
        Calls {
            alloc: 18,
            realloc: 0,
            dealloc: 18,
        },
    ),
];

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

/// Prepare and construct a fixture outside the counted window, on the flat store (`cells`) or the
/// `Value` store, then count every scan. Before each scan every input byte is set to alternating
/// ASCII, so each bound bit toggles and every STRING window stays valid text.
fn run_fixture(fixture: &Fixture, cells: bool) -> Result<Vec<Scan>, String> {
    let bytes = match fixture.artifact {
        Artifact::Saved(bytes) => bytes.to_vec(),
        Artifact::Corpus => corpus_artifact(fixture.name)?,
    };
    let prepared = PreparedModule::from_bytes(&bytes, run_limits())
        .map_err(|error| format!("preparation refused: {error}"))?;
    let services = Services::new(fixture.retain_store);
    let constructed = if cells {
        prepared.instantiate_on_cells_with_services(0, &services)
    } else {
        prepared.instantiate_with_services(0, &services)
    };
    let mut state = constructed.map_err(|error| format!("construction refused: {error}"))?;
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
    if let Ok(Some(ticks)) = state.read_global_scalar("ticks")
        && ticks != Value::DInt(activations as i32)
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
        let run = name.strip_prefix("cells/").unwrap_or(name);
        assert!(
            FIXTURES.iter().any(|fixture| fixture.name == run)
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
        match run_fixture(fixture, false) {
            Ok(scans) => {
                report(fixture.name, &scans);
                failures.extend(ceiling_violations(fixture.name, &scans));
            }
            Err(error) => failures.push(format!("{}: {error}", fixture.name)),
        }
    }
    assert_eq!(failures, Vec::<String>::new(), "failing runs");
}

/// The flat-store runs that still allocate, listed with their measured ceilings: the corpus cases
/// C1-19c4 added, whose value paths it removes one by one. The list only shrinks.
const NOT_YET_ZERO: [&str; 1] = ["cells/bindings"];

/// The fixtures the flat store refuses until a later slice, with the slice that admits them. None
/// since C1-15 placed the `STRUCT OVERLAP` of `structures` and `overlays` as its backing.
const REFUSED_ON_THE_FLAT_STORE: [&str; 0] = [];

/// Spec 34 MEM-12 and ARCH-05: the same runs on the flat store, which C1-10 to C1-19 build on and
/// make the default. Every scan's counts are printed under `cells/<name>`, so each slice is
/// measured against the store it changes; the ceilings move here as the slices bring them to zero.
/// A fixture the store refuses must be one of [`REFUSED_ON_THE_FLAT_STORE`], and every listed one
/// must still be refused, so the list shrinks as the slices admit them.
#[test]
fn every_fixture_reports_its_scans_on_the_flat_store() {
    let mut failures = Vec::new();
    for fixture in &FIXTURES {
        let listed = REFUSED_ON_THE_FLAT_STORE.contains(&fixture.name);
        match run_fixture(fixture, true) {
            Ok(scans) if !listed => {
                let name = format!("cells/{}", fixture.name);
                report(&name, &scans);
                failures.extend(ceiling_violations(&name, &scans));
                // The whole corpus is in the zero list from its first scan (MEM-09; C1-19c3), but for
                // the runs C1-19c4 still brings there.
                if !NOT_YET_ZERO.contains(&name.as_str())
                    && ceiling_of(&name) != Some((0, Calls::ZERO))
                {
                    failures.push(format!("{name} is not zero from its first scan"));
                }
            }
            Ok(_) => failures.push(format!("cells/{} is admitted now", fixture.name)),
            Err(error) if !listed => failures.push(format!("cells/{}: {error}", fixture.name)),
            Err(error) => eprintln!("scan-allocations,cells/{},refused={error}", fixture.name),
        }
    }
    assert_eq!(failures, Vec::<String>::new(), "failing runs");
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
    assert_eq!(failures, Vec::<String>::new(), "failing runs");
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

/// The retain fixture's retained image captured twice into one capture, counting the allocator calls of
/// each capture: the first reserves the staging buffer, the later one refills it.
fn capture_twice(
    state: &mut RuntimeState<'_>,
) -> (
    trust_runtime_core::retain::capture::RetainCapture,
    Calls,
    Calls,
) {
    let mut capture = trust_runtime_core::retain::capture::RetainCapture::new();
    let first = {
        let _armed = Armed::new();
        state
            .capture_retain(&mut capture)
            .expect("the first capture reads the retained image");
        take_counted()
    };
    let later = {
        let _armed = Armed::new();
        state
            .capture_retain(&mut capture)
            .expect("a later capture reads the retained image");
        take_counted()
    };
    (capture, first, later)
}

/// The STRN v3 payload bytes that the shared codec frames for the retain fixture's four retained values.
fn retain_payload(counter: i32, setpoint: f32, runs: i32) -> Vec<u8> {
    let mut expected = RetainSnapshot::default();
    expected.insert("counter", Value::DInt(counter));
    expected.insert("setpoint", Value::Real(setpoint));
    expected.insert("@program/Plant/runs", Value::DInt(runs));
    expected.insert("@program/Plant/label", Value::String("boiler".into()));
    let mut payload = Vec::new();
    expected
        .encode_snapshot_payload_into(&mut payload)
        .expect("the expected retained payload encodes");
    payload
}

/// Spec 34 §10 and MEM-04 (C5-2, the capture): the corpus `retain` fixture's retained image is
/// captured, as the STRN v3 payload bytes the shared codec writes, into a staging buffer reserved
/// once. The first capture reserves it; every later capture of the same declarations reuses it and
/// makes no allocator call, which is what lets a chip capture its retained values between scans. The
/// staged bytes are the fixture's, not an empty fallback.
#[test]
fn the_retain_fixture_captures_without_allocating_after_the_first_capture() {
    let bytes = corpus_artifact("retain").unwrap_or_else(|error| panic!("{error}"));
    let prepared = PreparedModule::from_bytes(&bytes, run_limits())
        .expect("the chips' admission accepts the retain fixture");
    let services = Services::new(true);
    let mut state = prepared
        .instantiate_with_services(0, &services)
        .expect("the retain fixture constructs");
    for index in 0..4 {
        let scan = measure(&mut state, 10 + 10 * index, false);
        assert_eq!(scan.outcome, None, "scan {index} succeeds");
    }

    let (capture, first, later) = capture_twice(&mut state);
    eprintln!("scan-allocations-capture,retain,first={first},later={later}");
    assert!(
        first.count() >= 1,
        "the first capture reserves the staging buffer; it made {first} allocator calls"
    );
    assert_eq!(
        later,
        Calls::ZERO,
        "a steady-state capture must not allocate (spec 34 MEM-04)"
    );

    // Four scans advance every retained value once per scan: `counter` and `runs` reach 4 and
    // `setpoint` reaches 20.0 + 4 × 0.5 = 22.0. The staged bytes must be exactly the payload the
    // shared codec frames for those values, in registration and declaration order.
    assert_eq!(
        capture.staged_payload(),
        retain_payload(4, 22.0, 4).as_slice(),
        "the capture stages the fixture's retained image as the codec's payload"
    );
}

/// Spec 34 §10, §6.2 and MEM-04 (C1-9d2b-2, R3): the same capture on the cell store. The retain fixture is
/// instantiated on cells, where its retained globals and its program's retained variables live in cells and
/// the encoder writes them from there. The first capture reserves the staging buffer, and every later
/// capture makes no allocator call. This capture follows construction, with no scan in between (a scan on
/// the cell store is compared with the `Value` store's in `store_construction_tests`, C1-9d3a), so the
/// staged bytes are the values of construction, worked out by hand from `retain/src/main.st`:
/// `counter` 0, `setpoint` 20.0, the program's `runs` 0 and its `label` 'boiler'. A cell encoder that
/// built a `Value` for each variable, or that grew its output, would allocate in the later capture; one
/// that read the `Value` half, where the cell store keeps none of them, would stage no entries.
#[test]
fn the_cell_retain_capture_allocates_nothing() {
    let bytes = corpus_artifact("retain").unwrap_or_else(|error| panic!("{error}"));
    let prepared = PreparedModule::from_bytes(&bytes, run_limits())
        .expect("the chips' admission accepts the retain fixture");
    let services = Services::new(true);
    let mut state = prepared
        .instantiate_on_cells_with_services(0, &services)
        .expect("the retain fixture constructs on the cell store");

    let (capture, first, later) = capture_twice(&mut state);
    eprintln!("scan-allocations-capture,retain-cells,first={first},later={later}");
    assert!(
        first.count() >= 1,
        "the first capture reserves the staging buffer; it made {first} allocator calls"
    );
    assert_eq!(
        later,
        Calls::ZERO,
        "a steady-state capture of cells must not allocate (spec 34 MEM-04)"
    );
    assert_eq!(
        capture.staged_payload(),
        retain_payload(0, 20.0, 0).as_slice(),
        "the capture stages the fixture's retained image, written from its cells"
    );
}

/// A diagnostic, run on request (`-- --ignored`): where the allocator calls of each fixture's later
/// scans on the flat store come from, as the innermost frame and the eight innermost engine functions
/// of each call, counted.
/// It asserts nothing; it tells the C1 slices which code still allocates.
#[test]
#[ignore = "diagnostic: run with --ignored --nocapture"]
fn trace_flat_store_allocations() {
    for fixture in &FIXTURES {
        TRACES.with(|traces| traces.borrow_mut().clear());
        TRACING.with(|flag| flag.set(true));
        let outcome = run_fixture(fixture, true);
        TRACING.with(|flag| flag.set(false));
        let traces = TRACES.with(|traces| std::mem::take(&mut *traces.borrow_mut()));
        if let Err(error) = outcome {
            eprintln!("trace,cells/{},refused={error}", fixture.name);
            continue;
        }
        // The first scan fills caches and buffers; count the later scans' calls only by their sites.
        let mut sites = std::collections::BTreeMap::<String, usize>::new();
        for trace in traces {
            *sites.entry(trace).or_default() += 1;
        }
        let mut sites = sites.into_iter().collect::<Vec<_>>();
        sites.sort_by_key(|site| std::cmp::Reverse(site.1));
        for (site, count) in sites.into_iter().take(48) {
            eprintln!("trace,cells/{},{count},{site}", fixture.name);
        }
    }
}

/// A diagnostic, run on request (`-- --ignored`): the allocator calls of a warm restart of each fixture
/// on the flat store, after two scans, as the innermost frame and the eight innermost engine functions
/// of each call, counted. It asserts nothing; it tells C1-14 which code of a restart builds values and
/// which reserves the next generation's store.
#[test]
#[ignore = "diagnostic: run with --ignored --nocapture"]
fn trace_flat_store_restart_allocations() {
    for fixture in &FIXTURES {
        let bytes = match fixture.artifact {
            Artifact::Saved(bytes) => bytes.to_vec(),
            Artifact::Corpus => match corpus_artifact(fixture.name) {
                Ok(bytes) => bytes,
                Err(error) => {
                    eprintln!("trace-restart,cells/{},refused={error}", fixture.name);
                    continue;
                }
            },
        };
        let Ok(prepared) = PreparedModule::from_bytes(&bytes, run_limits()) else {
            eprintln!("trace-restart,cells/{},refused=preparation", fixture.name);
            continue;
        };
        let services = Services::new(fixture.retain_store);
        let Ok(mut state) = prepared.instantiate_on_cells_with_services(0, &services) else {
            eprintln!("trace-restart,cells/{},refused=construction", fixture.name);
            continue;
        };
        for index in 0..2 {
            state.inputs_mut().fill(b'A');
            measure(&mut state, fixture.first_ms + 10 * index, false);
        }
        TRACES.with(|traces| traces.borrow_mut().clear());
        TRACING.with(|flag| flag.set(true));
        let (outcome, calls) = {
            let _armed = Armed::new();
            let outcome = state.restart(RestartMode::Warm);
            (outcome, take_counted())
        };
        TRACING.with(|flag| flag.set(false));
        let traces = TRACES.with(|traces| std::mem::take(&mut *traces.borrow_mut()));
        eprintln!(
            "trace-restart,cells/{},outcome={outcome:?},calls={calls}",
            fixture.name
        );
        let mut sites = std::collections::BTreeMap::<String, usize>::new();
        for trace in traces {
            *sites.entry(trace).or_default() += 1;
        }
        let mut sites = sites.into_iter().collect::<Vec<_>>();
        sites.sort_by_key(|site| std::cmp::Reverse(site.1));
        for (site, count) in sites.into_iter().take(20) {
            eprintln!("trace-restart,cells/{},{count},{site}", fixture.name);
        }
    }
}
