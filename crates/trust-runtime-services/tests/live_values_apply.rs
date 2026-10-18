//! The shared Live Values apply: the phases, the order and the refusal handling a chip and
//! trust-runtime run (specification 11, "Live Values"; specification 34 §4.3.1 and §8).
//!
//! Each test drives a prepared core program and calls the shared apply from the cycle hook where
//! the runtime calls it: queued writes at `CycleStart`, queued I/O writes and forces at
//! `BeforeInputSample`, and forces again at `AfterOutputSample`.
#![allow(
    unsafe_code,
    reason = "the counting allocator is the test-only meter of `scan_allocation_zero`, which must implement the unsafe GlobalAlloc trait"
)]
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

use trust_runtime_core::error::RuntimeError;
use trust_runtime_core::io_address::IoAddress;
use trust_runtime_core::value::{Duration, Value};
use trust_runtime_core::vm::{
    CycleAccess, CycleHooks, CyclePhase, PreparationLimits, PreparedModule,
};
use trust_runtime_services::live_values::apply::{self, WriteSink};
use trust_runtime_services::live_values::queues::{
    ForcedVarTarget, LiveValueQueues, PendingVarTarget, PendingVarWrite,
};
use trust_runtime_services::live_values::retained_names::{self, RetainedNames};

/// A prepared board artifact with a global, an I/O binding and a 10 ms task (the GPIO fixture the
/// core's own tests run).
const GPIO: &[u8] = include_bytes!("../../trust-runtime/tests/fixtures/portability/f401/gpio.stbc");
/// The bounded-C `retain` program: retained globals and retained program members.
const RETAIN: &[u8] =
    include_bytes!("../../trust-runtime/tests/fixtures/bounded-c/retain/program.stbc");

fn prepare(bytes: &[u8]) -> PreparedModule {
    PreparedModule::from_bytes(bytes, PreparationLimits::default())
        .expect("the admitted artifact prepares")
}

/// A sink that throws its reports away.
struct NoReport;

impl WriteSink for NoReport {
    fn refused(&mut self, _target: &str, _error: &RuntimeError, _time: Duration) {}
}

/// A sink that keeps the target text of every refusal it saw.
struct RecordingSink<'a> {
    refused: &'a mut Vec<String>,
}

impl WriteSink for RecordingSink<'_> {
    fn refused(&mut self, target: &str, _error: &RuntimeError, _time: Duration) {
        self.refused.push(target.to_owned());
    }
}

/// The state of the GPIO program's `%QX0.0` output bit.
fn led(access: &CycleAccess<'_, '_>) -> bool {
    access.outputs().first().is_some_and(|byte| byte & 1 == 1)
}

/// Spec 11, "Live Values": queued variable and reference writes are applied at the cycle start, in
/// the order they arrived, before the program runs.
///
/// The two queued targets name the same retained global through different paths (a retained
/// label and a plain global). Applying them out of arrival order, or deduplicating them, leaves
/// the earlier value in storage where the specification leaves the later one.
#[test]
fn queued_writes_apply_at_cycle_start_in_arrival_order() {
    struct StartWrites {
        queues: LiveValueQueues<u32>,
        names: RetainedNames,
        refused: Vec<String>,
    }
    impl CycleHooks for StartWrites {
        fn on_phase(
            &mut self,
            phase: CyclePhase,
            access: &mut CycleAccess<'_, '_>,
        ) -> Result<(), RuntimeError> {
            if phase == CyclePhase::CycleStart {
                let StartWrites {
                    queues,
                    names,
                    refused,
                } = self;
                let mut sink = RecordingSink { refused };
                apply::cycle_start(queues, names, access, &mut sink, |_, _, _| Ok(()))?;
            }
            Ok(())
        }
    }

    let prepared = prepare(RETAIN);
    let names = retained_names::bind_retained_declarations(&prepared).expect("retained labels");
    let mut queues = LiveValueQueues::<u32>::new();
    queues.pending_var_writes.push(PendingVarWrite {
        target: PendingVarTarget::Retain("counter".into()),
        value: Value::DInt(9),
    });
    queues.pending_var_writes.push(PendingVarWrite {
        target: PendingVarTarget::Global("counter".into()),
        value: Value::DInt(5),
    });
    let mut state = prepared
        .instantiate(0)
        .expect("the retain program constructs");
    let mut hooks = StartWrites {
        queues,
        names,
        refused: Vec::new(),
    };
    state
        .execute_cycle_with_hooks(Duration::from_millis(10), &mut hooks)
        .expect("the scan succeeds");
    // The Global write arrived last, so it is applied last; the program then adds 1. A
    // variant-ordered apply would leave 9 and the program would add 1 to that, giving 10.
    assert_eq!(state.storage().get_global("counter"), Some(&Value::DInt(6)));
    assert_eq!(hooks.refused, Vec::<String>::new());
}

/// Spec 11, "Live Values": the queued one-shot I/O writes are applied before the active forces at
/// the input sample, and the forces are applied again after the outputs are sampled.
///
/// The I/O write and the force name the same `%QX0.0` output, so applying them in the wrong order
/// at the input sample leaves the write's value. The force must also be re-applied after the
/// program's own mapped output is sampled, or the program's value would win.
#[test]
fn io_writes_then_forces_apply_before_the_input_sample_and_forces_again_after_outputs() {
    struct PhaseOrder {
        queues: LiveValueQueues<u32>,
        names: RetainedNames,
        before_input: Option<bool>,
        sampled_output: Option<bool>,
        after_force: Option<bool>,
    }
    impl CycleHooks for PhaseOrder {
        fn on_phase(
            &mut self,
            phase: CyclePhase,
            access: &mut CycleAccess<'_, '_>,
        ) -> Result<(), RuntimeError> {
            match phase {
                CyclePhase::BeforeInputSample => {
                    let PhaseOrder {
                        queues,
                        names,
                        before_input,
                        ..
                    } = self;
                    apply::io_writes_and_forces(queues, names, access, &mut NoReport)?;
                    *before_input = Some(led(access));
                }
                CyclePhase::AfterOutputSample => {
                    let PhaseOrder {
                        queues,
                        names,
                        sampled_output,
                        after_force,
                        ..
                    } = self;
                    // The core samples the program's mapped outputs before this hook.
                    *sampled_output = Some(led(access));
                    apply::forces(queues, names, access, &mut NoReport)?;
                    *after_force = Some(led(access));
                }
                _ => {}
            }
            Ok(())
        }
    }

    let prepared = prepare(GPIO);
    let names = retained_names::bind_retained_declarations(&prepared).expect("retained labels");
    let led_address = IoAddress::parse("%QX0.0").expect("a valid output address");
    let mut queues = LiveValueQueues::<u32>::new();
    queues
        .try_enqueue_io_write(led_address.clone(), Value::Bool(false))
        .expect("the I/O queue has room");
    queues
        .try_upsert_forced_io(led_address, Value::Bool(true))
        .expect("the force queue has room");
    let mut state = prepared
        .instantiate(0)
        .expect("the GPIO program constructs");
    let mut hooks = PhaseOrder {
        queues,
        names,
        before_input: None,
        sampled_output: None,
        after_force: None,
    };
    state
        .execute_cycle_with_hooks(Duration::from_millis(10), &mut hooks)
        .expect("the scan succeeds");
    // The I/O write (false) then the force (true) at the input sample.
    assert_eq!(hooks.before_input, Some(true));
    // The program drives `led := button`, which is FALSE while no input is set.
    assert_eq!(hooks.sampled_output, Some(false));
    // The force is applied again after the outputs.
    assert_eq!(hooks.after_force, Some(true));
}

/// Spec 11, "rejected request removal": a force a scan boundary refuses is withdrawn from the
/// queue and reported once, so its value is not applied again.
///
/// A BOOL forced onto the INT global `probe_depth` is refused with a type mismatch. Without the
/// withdrawal the second scan would apply and report it again; without the report the first
/// refusal would be silent.
#[test]
fn a_refused_force_is_withdrawn_and_reported_once() {
    struct ForcedRefusal {
        queues: LiveValueQueues<u32>,
        names: RetainedNames,
        refused: Vec<String>,
    }
    impl CycleHooks for ForcedRefusal {
        fn on_phase(
            &mut self,
            phase: CyclePhase,
            access: &mut CycleAccess<'_, '_>,
        ) -> Result<(), RuntimeError> {
            let ForcedRefusal {
                queues,
                names,
                refused,
            } = self;
            if phase == CyclePhase::BeforeInputSample {
                let mut sink = RecordingSink { refused };
                apply::io_writes_and_forces(queues, names, access, &mut sink)?;
            } else if phase == CyclePhase::AfterOutputSample {
                let mut sink = RecordingSink { refused };
                apply::forces(queues, names, access, &mut sink)?;
            }
            Ok(())
        }
    }

    let prepared = prepare(GPIO);
    let names = retained_names::bind_retained_declarations(&prepared).expect("retained labels");
    let mut queues = LiveValueQueues::<u32>::new();
    queues
        .try_upsert_forced_var(
            ForcedVarTarget::Global("probe_depth".into()),
            Value::Bool(true),
        )
        .expect("the force queue has room");
    let mut state = prepared
        .instantiate(0)
        .expect("the GPIO program constructs");
    let mut hooks = ForcedRefusal {
        queues,
        names,
        refused: Vec::new(),
    };
    for at_ms in [10, 20] {
        state
            .execute_cycle_with_hooks(Duration::from_millis(at_ms), &mut hooks)
            .expect("the scan succeeds");
    }
    assert_eq!(hooks.refused, vec!["probe_depth".to_owned()]);
    assert_eq!(
        hooks
            .queues
            .forced_vars
            .iter()
            .map(|forced| forced.target.clone())
            .collect::<Vec<_>>(),
        Vec::<ForcedVarTarget>::new()
    );
}

/// Spec 11, "Live Values" and spec 34 §4.3.1: retained names resolve to their live storage exactly
/// as the runtime resolves them, from the prepared program alone.
///
/// The label list shows the declared retained globals then the program's retained members, in
/// declaration order; a resolution that missed the program members, matched a non-retained name,
/// or ignored the case-insensitive lookup would fail an assertion below.
#[test]
fn retained_names_resolve_as_the_runtime_resolves_them() {
    let prepared = prepare(RETAIN);
    let state = prepared
        .instantiate(0)
        .expect("the retain program constructs");
    let names = retained_names::bind_retained_declarations(&prepared).expect("retained labels");
    let storage = state.storage();

    let listed = retained_names::retained_references(&names, storage);
    assert_eq!(
        listed
            .iter()
            .map(|(name, _)| name.as_str())
            .collect::<Vec<_>>(),
        [
            "counter",
            "setpoint",
            "@program/Plant/runs",
            "@program/Plant/label"
        ]
    );
    assert!(retained_names::retained_reference(&names, storage, "COUNTER").is_some());
    assert!(retained_names::retained_reference(&names, storage, "ticks").is_none());
    assert!(retained_names::retained_reference(&names, storage, "@program/Plant/runs").is_some());
    // The exact-name lookup keeps the control protocol's case-sensitive behaviour.
    assert!(retained_names::retained_reference_exact(&names, storage, "counter").is_some());
    assert!(retained_names::retained_reference_exact(&names, storage, "COUNTER").is_none());
    // `counter` resolves to the same live global the runtime's storage names directly.
    assert_eq!(
        retained_names::retained_reference(&names, storage, "counter"),
        storage.ref_for_global("counter")
    );
}

/// Allocator calls counted while an apply runs: allocation, reallocation and deallocation.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
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
}

thread_local! {
    static ARMED: Cell<bool> = const { Cell::new(false) };
    static COUNTED: Cell<Calls> = const { Cell::new(Calls::ZERO) };
}

/// Count one call on this thread while it is armed. Only const-initialized `Cell`s are touched,
/// so counting never allocates, blocks or unwinds.
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

/// Spec 34 §8 and MEM-04 (bounded-c: an in-scan hook allocates nothing): applying the queued
/// writes and the active forces allocates nothing while no request is refused.
///
/// The queues hold scalar values and apply in place: the write queue is drained without a new
/// vector, the force stays in its queue, and no target text is built. A snapshot clone, a
/// `Vec`-returning drain, or an eagerly built label would show up as one of the counts below.
#[test]
fn applying_writes_and_forces_allocates_nothing_when_nothing_is_refused() {
    struct AllocationProbe {
        queues: LiveValueQueues<u32>,
        names: RetainedNames,
        cycle_start: Calls,
        before_input: Calls,
        after_output: Calls,
    }
    impl CycleHooks for AllocationProbe {
        fn on_phase(
            &mut self,
            phase: CyclePhase,
            access: &mut CycleAccess<'_, '_>,
        ) -> Result<(), RuntimeError> {
            match phase {
                CyclePhase::CycleStart => {
                    let _armed = Armed::new();
                    let result = {
                        let AllocationProbe { queues, names, .. } = self;
                        apply::cycle_start(queues, names, access, &mut NoReport, |_, _, _| {
                            Ok::<(), RuntimeError>(())
                        })
                    };
                    self.cycle_start = take_counted();
                    result?;
                }
                CyclePhase::BeforeInputSample => {
                    let _armed = Armed::new();
                    let result = {
                        let AllocationProbe { queues, names, .. } = self;
                        apply::io_writes_and_forces(queues, names, access, &mut NoReport)
                    };
                    self.before_input = take_counted();
                    result?;
                }
                CyclePhase::AfterOutputSample => {
                    let _armed = Armed::new();
                    let result = {
                        let AllocationProbe { queues, names, .. } = self;
                        apply::forces(queues, names, access, &mut NoReport)
                    };
                    self.after_output = take_counted();
                    result?;
                }
                _ => {}
            }
            Ok(())
        }
    }

    let prepared = prepare(GPIO);
    let names = retained_names::bind_retained_declarations(&prepared).expect("retained labels");
    let mut queues = LiveValueQueues::<u32>::new();
    queues
        .try_upsert_var_write(
            PendingVarTarget::Global("probe_depth".into()),
            Value::Int(2),
        )
        .expect("the write queue has room");
    queues
        .try_upsert_forced_var(ForcedVarTarget::Global("probe_depth".into()), Value::Int(3))
        .expect("the force queue has room");
    let mut state = prepared
        .instantiate(0)
        .expect("the GPIO program constructs");
    let mut hooks = AllocationProbe {
        queues,
        names,
        cycle_start: Calls::ZERO,
        before_input: Calls::ZERO,
        after_output: Calls::ZERO,
    };
    state
        .execute_cycle_with_hooks(Duration::from_millis(10), &mut hooks)
        .expect("the scan succeeds");
    assert_eq!(
        hooks.cycle_start,
        Calls::ZERO,
        "the cycle-start apply allocated"
    );
    assert_eq!(
        hooks.before_input,
        Calls::ZERO,
        "the input-sample apply allocated"
    );
    assert_eq!(
        hooks.after_output,
        Calls::ZERO,
        "the output-sample apply allocated"
    );
}
