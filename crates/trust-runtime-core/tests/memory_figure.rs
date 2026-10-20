//! Spec 34 MEM-12 and MEM-14 (checklist C1-21): the memory figure is the heap a program holds,
//! computed at preparation from the program, so it equals what the allocator measures.
//!
//! A counting global allocator measures, on this thread, the bytes a prepared module holds, and,
//! with the `footprint` feature, the bytes each of its parts gives back when the module is freed
//! part by part, and the bytes construction leaves allocated for the program's engine. The figure
//! of every program of the bounded-C corpus and of the saved board artifacts must equal the
//! measurement, part by part. Run with `-- --nocapture` to see each program's figures part by part
//! against its container's bytes, the target of MEM-14.
//!
//! The corpus is generated on the builder, never written by hand:
//!
//! ```sh
//! cargo run --locked -p trust-runtime --example bounded_c_fixtures -- crates/trust-runtime/tests/fixtures/bounded-c
//! ```
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::{Cell, RefCell};
use trust_runtime_core::bytecode::ValidationLimits;
use trust_runtime_core::error::RuntimeError;
use trust_runtime_core::retain::capture::RetainCapture;
use trust_runtime_core::value::{DateTimeValue, Duration};
use trust_runtime_core::vm::{ExecutionServices, PreparationLimits, PreparedModule};

thread_local! {
    static ARMED: Cell<bool> = const { Cell::new(false) };
    static LIVE: Cell<i64> = const { Cell::new(0) };
}

/// Add `change` to the bytes live on this thread while it is armed. Only const-initialized `Cell`s
/// are touched, so counting never allocates, blocks or unwinds.
fn record(change: i64) {
    let _ = ARMED.try_with(|armed| {
        if armed.get() {
            let _ = LIVE.try_with(|live| live.set(live.get() + change));
        }
    });
}

struct Measured;

// SAFETY: every operation forwards the caller's unchanged layout and pointer to `System`; the
// default `realloc` goes through these two, so a reallocation is counted as an allocation and a
// free. The counting touches only const-initialized thread-local `Cell`s.
unsafe impl GlobalAlloc for Measured {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // SAFETY: the caller supplies a valid allocation layout.
        let pointer = unsafe { System.alloc(layout) };
        if !pointer.is_null() {
            record(layout.size() as i64);
        }
        pointer
    }

    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        record(-(layout.size() as i64));
        // SAFETY: `pointer` and `layout` are the pair `System` returned.
        unsafe { System.dealloc(pointer, layout) }
    }
}

#[global_allocator]
static ALLOCATOR: Measured = Measured;

/// The bytes that `run` leaves live on this thread.
fn live_after(run: impl FnOnce()) -> i64 {
    LIVE.with(|live| live.set(0));
    ARMED.with(|armed| armed.set(true));
    run();
    ARMED.with(|armed| armed.set(false));
    LIVE.with(Cell::get)
}

/// The limits each program is prepared under: the chips' admission limits on the chips' stack
/// executor, and the hosted defaults with the register tier, whose lowering needs more.
fn limits() -> PreparationLimits {
    if cfg!(feature = "register-ir") {
        PreparationLimits::default()
    } else {
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
}

const CORPUS: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../trust-runtime/tests/fixtures/bounded-c"
);

/// Every program of the bounded-C corpus.
const CORPUS_PROGRAMS: [&str; 23] = [
    "scalar",
    "gpio",
    "io",
    "calls",
    "blocks",
    "methods",
    "instances",
    "arrays",
    "structures",
    "references",
    "strings",
    "initializers",
    "sizeof",
    "retain",
    "retained_blocks",
    "retained_values",
    "copies",
    "overlays",
    "unions",
    "operands",
    "results",
    "bindings",
    "faults",
];

/// The saved artifacts the boards run, and every corpus program.
fn programs() -> Vec<(String, Vec<u8>)> {
    let saved: [(&str, &[u8]); 3] = [
        (
            "saved/main",
            include_bytes!(
                "../../trust-runtime/tests/fixtures/portability/stbc-2.0/program-v2.stbc"
            ),
        ),
        (
            "saved/numeric",
            include_bytes!(
                "../../trust-runtime/tests/fixtures/portability/stbc-2.0/numeric-v2.stbc"
            ),
        ),
        (
            "saved/gpio",
            include_bytes!("../../trust-runtime/tests/fixtures/portability/f401/gpio.stbc"),
        ),
    ];
    let mut programs = saved
        .iter()
        .map(|(name, bytes)| ((*name).to_owned(), bytes.to_vec()))
        .collect::<Vec<_>>();
    for name in CORPUS_PROGRAMS {
        let path = format!("{CORPUS}/{name}/program.stbc");
        let bytes = std::fs::read(&path).unwrap_or_else(|error| {
            panic!("{path} is missing ({error}); generate the bounded-C corpus on the builder")
        });
        programs.push((name.to_owned(), bytes));
    }
    programs
}

#[test]
fn the_prepared_programs_figure_is_the_heap_it_holds() {
    // Spec 34 MEM-12 and MEM-14: preparation computes the bytes of heap the prepared program holds
    // from its own tables. The allocator measures what preparation leaves allocated; the figure,
    // less the module's own record, which preparation returns by value and a runtime then keeps on
    // the heap, must equal it for every program. A table that the figure skips, counts twice or
    // counts by its length instead of its capacity makes them differ.
    let mut differences = Vec::new();
    for (name, bytes) in programs() {
        let mut prepared = None;
        let measured = live_after(|| {
            prepared = Some(PreparedModule::from_bytes(&bytes, limits()).expect("prepares"));
        });
        let prepared = prepared.expect("prepared");
        let parts = prepared.program_parts();
        let record = parts
            .iter()
            .find(|part| part.name == "module record")
            .expect("the figure names the module record")
            .bytes;
        assert_eq!(
            record,
            std::mem::size_of::<PreparedModule>(),
            "{name}: the record is the module's own bytes"
        );
        assert_eq!(
            prepared.memory_figure().program,
            prepared.program_bytes(),
            "{name}: preparation computed the program's part of the figure"
        );
        let computed = prepared.program_bytes() - record;
        println!(
            "figure: {name}: {} bytes of bytecode; prepared program {} bytes, {:.2} times the \
             container (MEM-14's target is at most 1)",
            bytes.len(),
            prepared.program_bytes(),
            prepared.program_bytes() as f64 / bytes.len() as f64
        );
        for part in parts.iter().filter(|part| part.bytes != 0) {
            println!("figure: {name}:   {}: {} B", part.name, part.bytes);
        }
        if computed as i64 != measured {
            differences.push(format!(
                "{name}: the figure counts {computed} bytes, the allocator measured {measured}"
            ));
        }
        #[cfg(feature = "footprint")]
        differences.extend(part_differences(&name, prepared));
    }
    assert!(differences.is_empty(), "{differences:#?}");
}

/// Each part whose figure differs from the bytes that freeing it gives back.
#[cfg(feature = "footprint")]
fn part_differences(name: &str, prepared: PreparedModule) -> Vec<String> {
    let figure = prepared.program_parts();
    let mut freed = Vec::with_capacity(32);
    let mut last = 0i64;
    live_after(|| {
        prepared.drop_in_parts(|part| {
            let now = LIVE.with(Cell::get);
            if part != "start" {
                freed.push((part, last - now));
            }
            last = now;
        });
    });
    freed
        .into_iter()
        .filter_map(|(part, measured)| {
            let computed = figure
                .iter()
                .find(|computed| computed.name == part)
                .map(|computed| computed.bytes as i64);
            (computed != Some(measured)).then(|| {
                format!(
                    "{name}: {part}: the figure counts {computed:?} bytes, freeing it gave back \
                     {measured}"
                )
            })
        })
        .collect()
}

/// The corpus programs that keep retained values, which a device keeps a retain store for.
const RETAINING: [&str; 5] = [
    "retain",
    "retained_blocks",
    "retained_values",
    "overlays",
    "unions",
];

/// Execution services without a wall clock that keep a retain store when `retain_store` holds, as
/// a device whose record keeps retained values does.
struct Services {
    retain_store: bool,
    retain: RefCell<RetainCapture>,
}

impl Services {
    fn new(retain_store: bool) -> Self {
        let mut retain = RetainCapture::new();
        retain.configure(Some(Duration::ZERO), Duration::ZERO);
        Self {
            retain_store,
            retain: RefCell::new(retain),
        }
    }
}

impl ExecutionServices for Services {
    fn deadline_exceeded(&self) -> bool {
        false
    }
    fn has_wall_clock(&self) -> bool {
        false
    }
    fn has_retain_store(&self) -> bool {
        self.retain_store
    }
    fn retain_capture(&self) -> Option<&RefCell<RetainCapture>> {
        self.retain_store.then_some(&self.retain)
    }
    fn save_retain_payload(&self, _payload: &[u8]) -> Result<(), RuntimeError> {
        Ok(())
    }
    fn current_dt(&self) -> Result<DateTimeValue, RuntimeError> {
        Err(RuntimeError::ProfileUnsupported("no UTC clock".into()))
    }
}

#[test]
fn the_constructed_programs_figure_is_the_heap_it_holds() {
    // Spec 34 MEM-12: preparation computes what construction reserves for the program's engine: the
    // persistent block, the activation stack for the deepest call chain, the operand stack, the
    // engine's own state and, when the runtime keeps retained values, their capture. The allocator
    // measures what construction leaves allocated; the figure less the prepared program must equal
    // it for every program. A reservation the figure leaves out or computes otherwise than
    // construction reserves it, or a list that grows on first use during construction, makes them
    // differ. A scan allocates nothing (ARCH-05; `scan_allocation_zero`), so this is what the
    // running program holds.
    let mut differences = Vec::new();
    for (name, bytes) in programs() {
        let retains = RETAINING.contains(&name.as_str());
        let prepared = PreparedModule::from_bytes(&bytes, limits()).expect("prepares");
        let figure = prepared.memory_figure();
        let services = Services::new(retains);
        let mut state = None;
        let measured = live_after(|| {
            state = Some(
                prepared
                    .instantiate_with_services(0, &services)
                    .expect("constructs"),
            );
        });
        drop(state);
        println!(
            "figure: {name}: {} bytes in all{}: program {}, persistent block {}, activation stack \
             {}, operand stack {}, engine {}, retain capture {}",
            figure.total(retains),
            if retains { " with retained values" } else { "" },
            figure.program,
            figure.persistent_block,
            figure.activation_stack,
            figure.operand_stack,
            figure.engine,
            figure.retain_capture
        );
        let computed = figure.total(retains) - figure.program;
        if computed as i64 != measured {
            differences.push(format!(
                "{name}: the figure counts {computed} bytes for the engine, construction left \
                 {measured}"
            ));
        }
    }
    assert!(differences.is_empty(), "{differences:#?}");
}
