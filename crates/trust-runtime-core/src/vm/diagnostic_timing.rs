//! Bounded, opt-in timing aggregates for hosted diagnostics.
//!
//! The collector records coarse phase spans only. It does not format, print, or
//! allocate while recording. Inclusive phase totals overlap when phases nest;
//! self time subtracts the time spent in directly nested spans.

#![cfg(all(feature = "std", feature = "test-support"))]

use core::cell::{Cell, RefCell};
use core::marker::PhantomData;
use std::rc::Rc;
use std::time::Instant;

const STACK_CAPACITY: usize = 64;

mod callsite_accounting;
mod frame_query;
mod initializer_branch;
pub(crate) use callsite_accounting::attribution;
#[cfg(test)]
pub(crate) use callsite_accounting::observe_index_lookup;
pub use callsite_accounting::{
    callsite_accounting_log_enabled, finish_callsite_accounting_log, start_callsite_accounting_log,
    ArrayConsumerRecord, CallsiteAccountingSnapshot, CopyAdmissionOutcome, CopyAdmissionRecord,
    CopyConsumer, CopyConsumerKind, CopyParameterRole, CopyReadKind, DemandRange,
    IndexLookupOperation, IndexLookupPath, IndexLookupRecord, LifecycleDemandRecord,
    LifecycleDemandUnit, OwnerQueryRecord, ValueVariant, WorkChargeOutcome, WorkChargeRecord,
    ATTRIBUTION_RECORD_CAPACITY, CALLSITE_RECORD_CAPACITY, INDEX_LOOKUP_RECORD_CAPACITY,
    LIFECYCLE_RECORD_CAPACITY,
};
pub(crate) use callsite_accounting::{
    copy_outcome, observe_index_lookup_result, record_copy_admission, record_index_owner_geometry,
    record_index_owner_query, record_index_target_geometry, record_work_charge, work_outcome,
};
pub use frame_query::{
    begin_frame_query, finish_frame_query_log, start_frame_query_log, FrameQuery, FrameQueryRecord,
    FrameQuerySnapshot, FRAME_QUERY_RECORD_CAPACITY,
};
pub use initializer_branch::{
    finish_initializer_branch_log, initializer_branch_log_enabled, record_initializer_branch,
    start_initializer_branch_log, InitializerBranch, InitializerBranchMetadata,
    InitializerBranchRecord, InitializerBranchSnapshot, INITIALIZER_BRANCH_RECORD_CAPACITY,
};

/// A coarse hosted runtime phase measured by the diagnostic collector.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(usize)]
pub enum Phase {
    /// Hosted runtime cycle orchestration.
    HostedCycle = 0,
    /// Prepare one native call and resolve its target.
    CallPrepare = 1,
    /// Bind named, positional, and output arguments.
    ArgumentBind = 2,
    /// Execute one callee from suspension through caller resumption.
    PouExecute = 3,
    /// Initialize one activation frame and its declarations.
    FrameInitialize = 4,
    /// Retire one activation frame.
    FrameRetire = 5,
    /// Copy call outputs back and recycle call storage.
    CallComplete = 6,
    /// Execute one built-in function or function block.
    BuiltinExecute = 7,
    /// Read a VM reference including access policy and materialization.
    ReferenceRead = 8,
    /// Write a VM reference including policy and prepared storage commit.
    ReferenceWrite = 9,
    /// Normalize a value for a declared assignment target.
    AssignmentNormalize = 10,
    /// Commit a prepared write into variable storage.
    StorageWrite = 11,
    /// Charge or materialize an owned copy of a runtime value.
    CopyAccounting = 12,
    /// Charge logical work for an engine operation.
    WorkCharge = 13,
    /// Sample the physical execution deadline.
    DeadlineCheck = 14,
    /// Run one declaration initialization action.
    DeclarationAction = 15,
    /// Evaluate an initializer body or recipe.
    InitializerEvaluate = 16,
    /// Commit an initialized declaration into its destination.
    InitializerCommit = 17,
    /// Take, reset, or recycle register execution buffers.
    RegisterBufferLifecycle = 18,
    /// Execute one prepared register program.
    RegisterExecute = 19,
    /// Execute one POU through the stack interpreter.
    StackExecute = 20,
}

impl Phase {
    /// Number of recorded phases.
    pub const COUNT: usize = 21;

    /// Phases in stable snapshot order.
    pub const ALL: [Self; Self::COUNT] = [
        Self::HostedCycle,
        Self::CallPrepare,
        Self::ArgumentBind,
        Self::PouExecute,
        Self::FrameInitialize,
        Self::FrameRetire,
        Self::CallComplete,
        Self::BuiltinExecute,
        Self::ReferenceRead,
        Self::ReferenceWrite,
        Self::AssignmentNormalize,
        Self::StorageWrite,
        Self::CopyAccounting,
        Self::WorkCharge,
        Self::DeadlineCheck,
        Self::DeclarationAction,
        Self::InitializerEvaluate,
        Self::InitializerCommit,
        Self::RegisterBufferLifecycle,
        Self::RegisterExecute,
        Self::StackExecute,
    ];

    /// Mask selecting every phase.
    pub const MASK_ALL: u64 = (1u64 << Self::COUNT) - 1;

    /// Mask selecting phases shared by the paired consumers, including the
    /// driver-owned hosted-cycle wrapper.
    pub const MASK_MATCHED: u64 = Self::HostedCycle.mask_bit()
        | Self::CallPrepare.mask_bit()
        | Self::ArgumentBind.mask_bit()
        | Self::PouExecute.mask_bit()
        | Self::FrameInitialize.mask_bit()
        | Self::FrameRetire.mask_bit()
        | Self::CallComplete.mask_bit()
        | Self::BuiltinExecute.mask_bit()
        | Self::ReferenceRead.mask_bit()
        | Self::ReferenceWrite.mask_bit()
        | Self::AssignmentNormalize.mask_bit()
        | Self::StorageWrite.mask_bit()
        | Self::CopyAccounting.mask_bit()
        | Self::WorkCharge.mask_bit()
        | Self::DeadlineCheck.mask_bit()
        | Self::RegisterBufferLifecycle.mask_bit()
        | Self::RegisterExecute.mask_bit()
        | Self::StackExecute.mask_bit();

    /// Stable label for post-run diagnostic export.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::HostedCycle => "hosted_cycle",
            Self::CallPrepare => "call_prepare",
            Self::ArgumentBind => "argument_bind",
            Self::PouExecute => "pou_execute",
            Self::FrameInitialize => "frame_initialize",
            Self::FrameRetire => "frame_retire",
            Self::CallComplete => "call_complete",
            Self::BuiltinExecute => "builtin_execute",
            Self::ReferenceRead => "reference_read",
            Self::ReferenceWrite => "reference_write",
            Self::AssignmentNormalize => "assignment_normalize",
            Self::StorageWrite => "storage_write",
            Self::CopyAccounting => "copy_accounting",
            Self::WorkCharge => "work_charge",
            Self::DeadlineCheck => "deadline_check",
            Self::DeclarationAction => "declaration_action",
            Self::InitializerEvaluate => "initializer_evaluate",
            Self::InitializerCommit => "initializer_commit",
            Self::RegisterBufferLifecycle => "register_buffer_lifecycle",
            Self::RegisterExecute => "register_execute",
            Self::StackExecute => "stack_execute",
        }
    }

    /// Compatibility name for the former coarse execution phase.
    #[allow(non_upper_case_globals)]
    pub const Execution: Self = Self::PouExecute;
    /// Compatibility name for the former call-preparation phase.
    #[allow(non_upper_case_globals)]
    pub const CallPreparation: Self = Self::CallPrepare;
    /// Compatibility name for the former frame-initialization phase.
    #[allow(non_upper_case_globals)]
    pub const FrameInitialization: Self = Self::FrameInitialize;
    /// Compatibility name for the former initializer phase.
    #[allow(non_upper_case_globals)]
    pub const Initializer: Self = Self::InitializerEvaluate;
    /// Compatibility name for the former cleanup phase.
    #[allow(non_upper_case_globals)]
    pub const Cleanup: Self = Self::FrameRetire;

    const fn index(self) -> usize {
        self as usize
    }

    /// Bit selecting this phase in [`set_phase_mask`].
    #[must_use]
    pub const fn mask_bit(self) -> u64 {
        1u64 << self.index()
    }
}

/// Fixed-size aggregate for one phase.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct PhaseMeasurement {
    /// Completed span count.
    pub calls: u64,
    /// Total time including nested phases. Nested totals overlap their parents.
    pub inclusive_nanos: u64,
    /// Time excluding directly nested spans.
    pub self_nanos: u64,
    /// Longest inclusive span.
    pub max_nanos: u64,
}

/// Fixed-size timing result suitable for export after the measured operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Snapshot {
    /// Measurements in [`Phase::ALL`] order.
    pub measurements: [PhaseMeasurement; Phase::COUNT],
    /// Spans dropped because recording was busy or the fixed stack was full.
    pub dropped_spans: u64,
    /// Spans closed out of order or left open when [`finish`] was called.
    pub unbalanced_spans: u64,
    /// Attempts rejected because the fixed nesting stack was full.
    pub depth_overflows: u64,
    /// Attempts rejected while the thread-local recorder was already borrowed.
    pub recorder_busy: u64,
    /// Number of spans still active when [`finish`] captured this result.
    pub active_spans: u64,
}

impl Snapshot {
    /// Return one phase's aggregate.
    #[must_use]
    pub const fn measurement(&self, phase: Phase) -> PhaseMeasurement {
        self.measurements[phase.index()]
    }

    /// Whether no timing samples or nesting state were lost.
    #[must_use]
    pub const fn is_valid(&self) -> bool {
        self.dropped_spans == 0
            && self.unbalanced_spans == 0
            && self.depth_overflows == 0
            && self.recorder_busy == 0
            && self.active_spans == 0
    }
}

/// RAII span returned by [`begin`]. It is deliberately neither `Send` nor `Sync`.
pub struct Span {
    token: u64,
    generation: u64,
    active: bool,
    _not_send_or_sync: PhantomData<Rc<()>>,
}

impl Drop for Span {
    fn drop(&mut self) {
        if !self.active {
            return;
        }
        let completed = RECORDER.try_with(|recorder| {
            let Ok(mut recorder) = recorder.try_borrow_mut() else {
                note_recorder_busy();
                return;
            };
            recorder.end(self.token, self.generation, Instant::now());
        });
        if completed.is_err() {
            note_recorder_busy();
        }
    }
}

/// Start timing one selected phase when collection is enabled.
///
/// An inert span is returned when collection is disabled or the collector
/// cannot borrow its thread-local state. The latter invalidates the next
/// snapshot rather than panicking in an instrumented runtime path.
#[must_use]
pub fn begin(phase: Phase) -> Span {
    let mut span = Span {
        token: 0,
        generation: 0,
        active: false,
        _not_send_or_sync: PhantomData,
    };
    let config = match CONFIG.try_with(Cell::get) {
        Ok(config) => config,
        Err(_) => {
            note_recorder_busy();
            return span;
        }
    };
    if !config.enabled || config.phase_mask & phase.mask_bit() == 0 {
        return span;
    }
    let result = RECORDER.try_with(|recorder| {
        let Ok(mut recorder) = recorder.try_borrow_mut() else {
            note_recorder_busy();
            return;
        };
        if let Some((token, generation)) = recorder.begin(phase) {
            span.token = token;
            span.generation = generation;
            span.active = true;
        }
    });
    if result.is_err() {
        note_recorder_busy();
    }
    span
}

/// Enable or disable collection on the current thread.
///
/// Disabling with spans still open records them as unbalanced and invalidates
/// their later drops. Collection is disabled by default.
pub fn set_enabled(enabled: bool) {
    let mut recorder_updated = false;
    let result = RECORDER.try_with(|recorder| {
        let Ok(mut recorder) = recorder.try_borrow_mut() else {
            return;
        };
        if !enabled {
            recorder.disable_with_open_spans();
        }
        recorder_updated = true;
    });
    if result.is_err() || !recorder_updated {
        note_recorder_busy();
        return;
    }
    let _ = CONFIG.try_with(|config| {
        let mut value = config.get();
        value.enabled = enabled;
        config.set(value);
    });
}

/// Select which phases [`begin`] records. Unselected phases return before the
/// recorder is borrowed or `Instant::now()` is sampled.
pub fn set_phase_mask(phase_mask: u64) {
    let _ = CONFIG.try_with(|config| {
        let mut value = config.get();
        value.phase_mask = phase_mask & Phase::MASK_ALL;
        config.set(value);
    });
}

/// Clear measurements and counters while preserving the enabled state and phase mask.
///
/// Any spans from before the reset become stale and cannot modify the new
/// recording generation when they are dropped.
pub fn reset() {
    let mut recorder_reset = false;
    let result = RECORDER.try_with(|recorder| {
        let Ok(mut recorder) = recorder.try_borrow_mut() else {
            return;
        };
        recorder.reset();
        recorder_reset = true;
    });
    if result.is_err() || !recorder_reset {
        note_recorder_busy();
        return;
    }
    let _ = FAILURES.try_with(|failures| failures.set(FailureCounters::default()));
}

/// Stop collection and capture the current fixed-size aggregate.
///
/// Open spans are counted as active and unbalanced. Their later drops are
/// ignored because finishing advances the recording generation.
#[must_use]
pub fn finish() -> Snapshot {
    let mut snapshot = Snapshot::empty();
    let result = RECORDER.try_with(|recorder| {
        let Ok(mut recorder) = recorder.try_borrow_mut() else {
            snapshot.recorder_busy = 1;
            snapshot.dropped_spans = 1;
            return;
        };
        snapshot = recorder.finish();
    });
    if result.is_err() {
        snapshot.recorder_busy = snapshot.recorder_busy.saturating_add(1);
        snapshot.dropped_spans = snapshot.dropped_spans.saturating_add(1);
    }
    if let Ok(failures) = FAILURES.try_with(Cell::get) {
        snapshot.dropped_spans = snapshot
            .dropped_spans
            .saturating_add(failures.dropped_spans);
        snapshot.recorder_busy = snapshot
            .recorder_busy
            .saturating_add(failures.recorder_busy);
    }
    let _ = CONFIG.try_with(|config| {
        let mut value = config.get();
        value.enabled = false;
        config.set(value);
    });
    snapshot
}

impl Snapshot {
    const fn empty() -> Self {
        Self {
            measurements: [PhaseMeasurement {
                calls: 0,
                inclusive_nanos: 0,
                self_nanos: 0,
                max_nanos: 0,
            }; Phase::COUNT],
            dropped_spans: 0,
            unbalanced_spans: 0,
            depth_overflows: 0,
            recorder_busy: 0,
            active_spans: 0,
        }
    }
}

struct Frame {
    token: u64,
    phase: Phase,
    started: Instant,
    child_nanos: u64,
}

#[derive(Clone, Copy, Default)]
struct FailureCounters {
    dropped_spans: u64,
    recorder_busy: u64,
}

#[derive(Clone, Copy)]
struct RecordingConfig {
    enabled: bool,
    phase_mask: u64,
}

struct Recorder {
    generation: u64,
    next_token: u64,
    depth: usize,
    stack: [Option<Frame>; STACK_CAPACITY],
    measurements: [PhaseMeasurement; Phase::COUNT],
    dropped_spans: u64,
    unbalanced_spans: u64,
    depth_overflows: u64,
}

impl Recorder {
    fn new() -> Self {
        Self {
            generation: 0,
            next_token: 0,
            depth: 0,
            stack: core::array::from_fn(|_| None),
            measurements: [PhaseMeasurement::default(); Phase::COUNT],
            dropped_spans: 0,
            unbalanced_spans: 0,
            depth_overflows: 0,
        }
    }

    fn begin(&mut self, phase: Phase) -> Option<(u64, u64)> {
        if self.depth == STACK_CAPACITY {
            self.dropped_spans = self.dropped_spans.saturating_add(1);
            self.depth_overflows = self.depth_overflows.saturating_add(1);
            return None;
        }
        let started = Instant::now();
        self.next_token = self.next_token.wrapping_add(1);
        if self.next_token == 0 {
            self.next_token = 1;
        }
        let token = self.next_token;
        self.stack[self.depth] = Some(Frame {
            token,
            phase,
            started,
            child_nanos: 0,
        });
        self.depth += 1;
        Some((token, self.generation))
    }

    fn end(&mut self, token: u64, generation: u64, ended: Instant) {
        if generation != self.generation {
            return;
        }
        let Some(top) = self.depth.checked_sub(1) else {
            self.note_unbalanced();
            return;
        };
        if self.stack[top].as_ref().map(|frame| frame.token) != Some(token) {
            self.note_unbalanced();
            self.clear_stack();
            return;
        }
        let Some(frame) = self.stack[top].take() else {
            self.note_unbalanced();
            self.clear_stack();
            return;
        };
        self.depth = top;
        let elapsed = nanos(ended.saturating_duration_since(frame.started));
        let own = elapsed.saturating_sub(frame.child_nanos);
        let measurement = &mut self.measurements[frame.phase.index()];
        measurement.calls = measurement.calls.saturating_add(1);
        measurement.inclusive_nanos = measurement.inclusive_nanos.saturating_add(elapsed);
        measurement.self_nanos = measurement.self_nanos.saturating_add(own);
        measurement.max_nanos = measurement.max_nanos.max(elapsed);
        if let Some(parent) = self.depth.checked_sub(1)
            && let Some(parent) = self.stack[parent].as_mut()
        {
            parent.child_nanos = parent.child_nanos.saturating_add(elapsed);
        }
    }

    fn disable_with_open_spans(&mut self) {
        if self.depth != 0 {
            self.unbalanced_spans = self
                .unbalanced_spans
                .saturating_add(saturating_u64(self.depth));
            self.clear_stack();
        }
    }

    fn reset(&mut self) {
        self.generation = self.generation.wrapping_add(1);
        self.depth = 0;
        self.stack.fill_with(|| None);
        self.measurements.fill(PhaseMeasurement::default());
        self.dropped_spans = 0;
        self.unbalanced_spans = 0;
        self.depth_overflows = 0;
    }

    fn finish(&mut self) -> Snapshot {
        let active = saturating_u64(self.depth);
        self.unbalanced_spans = self.unbalanced_spans.saturating_add(active);
        self.clear_stack();
        Snapshot {
            measurements: self.measurements,
            dropped_spans: self.dropped_spans,
            unbalanced_spans: self.unbalanced_spans,
            depth_overflows: self.depth_overflows,
            recorder_busy: 0,
            active_spans: active,
        }
    }

    fn clear_stack(&mut self) {
        self.generation = self.generation.wrapping_add(1);
        self.depth = 0;
        self.stack.fill_with(|| None);
    }

    fn note_unbalanced(&mut self) {
        self.unbalanced_spans = self.unbalanced_spans.saturating_add(1);
        self.dropped_spans = self.dropped_spans.saturating_add(1);
    }
}

fn nanos(duration: std::time::Duration) -> u64 {
    duration.as_nanos().min(u128::from(u64::MAX)) as u64
}

fn saturating_u64(value: usize) -> u64 {
    u64::try_from(value).unwrap_or(u64::MAX)
}

fn note_recorder_busy() {
    let _ = FAILURES.try_with(|failures| {
        let mut counters = failures.get();
        counters.recorder_busy = counters.recorder_busy.saturating_add(1);
        counters.dropped_spans = counters.dropped_spans.saturating_add(1);
        failures.set(counters);
    });
}

thread_local! {
    static RECORDER: RefCell<Recorder> = RefCell::new(Recorder::new());
    static CONFIG: Cell<RecordingConfig> = const { Cell::new(RecordingConfig {
        enabled: false,
        phase_mask: Phase::MASK_ALL,
    }) };
    static FAILURES: Cell<FailureCounters> = const { Cell::new(FailureCounters {
        dropped_spans: 0,
        recorder_busy: 0,
    }) };
}

#[cfg(test)]
mod tests {
    use super::{begin, finish, reset, set_enabled, set_phase_mask, Phase};
    use std::vec::Vec;

    #[test]
    fn nested_spans_record_counts_and_inclusive_and_self_time() {
        reset();
        set_enabled(true);
        let hosted = begin(Phase::HostedCycle);
        let execution = begin(Phase::PouExecute);
        let preparation = begin(Phase::CallPrepare);
        drop(preparation);
        drop(execution);
        drop(hosted);
        let snapshot = finish();

        let hosted = snapshot.measurement(Phase::HostedCycle);
        let execution = snapshot.measurement(Phase::PouExecute);
        let preparation = snapshot.measurement(Phase::CallPrepare);
        assert!(snapshot.is_valid());
        assert_eq!(
            (hosted.calls, execution.calls, preparation.calls),
            (1, 1, 1)
        );
        assert!(hosted.inclusive_nanos >= execution.inclusive_nanos);
        assert!(execution.inclusive_nanos >= preparation.inclusive_nanos);
        assert_eq!(
            hosted.self_nanos + execution.inclusive_nanos,
            hosted.inclusive_nanos
        );
        assert_eq!(
            execution.self_nanos + preparation.inclusive_nanos,
            execution.inclusive_nanos
        );
        assert_eq!(hosted.max_nanos, hosted.inclusive_nanos);
        assert_eq!(execution.max_nanos, execution.inclusive_nanos);
        assert_eq!(preparation.max_nanos, preparation.inclusive_nanos);
        assert_eq!(Phase::FrameRetire.name(), "frame_retire");
    }

    #[test]
    fn disabled_recording_is_empty_and_stack_overflow_invalidates_snapshot() {
        reset();
        let span = begin(Phase::PouExecute);
        drop(span);
        assert_eq!(finish().measurement(Phase::PouExecute).calls, 0);

        reset();
        set_enabled(true);
        let mut spans = Vec::with_capacity(65);
        for _ in 0..65 {
            spans.push(begin(Phase::PouExecute));
        }
        while let Some(span) = spans.pop() {
            drop(span);
        }
        let snapshot = finish();
        assert_eq!(snapshot.depth_overflows, 1);
        assert_eq!(snapshot.dropped_spans, 1);
        assert!(!snapshot.is_valid());
    }

    #[test]
    fn mask_only_records_selected_nested_phases_and_reset_preserves_selection() {
        let mask = Phase::PouExecute.mask_bit() | Phase::CallPrepare.mask_bit();
        set_phase_mask(mask);
        reset();
        set_enabled(true);
        let execution = begin(Phase::PouExecute);
        let call = begin(Phase::CallPrepare);
        let excluded = begin(Phase::ReferenceRead);
        drop(excluded);
        drop(call);
        drop(execution);
        let snapshot = finish();

        let execution = snapshot.measurement(Phase::PouExecute);
        let call = snapshot.measurement(Phase::CallPrepare);
        assert!(snapshot.is_valid());
        assert_eq!((execution.calls, call.calls), (1, 1));
        assert_eq!(snapshot.measurement(Phase::ReferenceRead).calls, 0);
        assert_eq!(
            execution.self_nanos + call.inclusive_nanos,
            execution.inclusive_nanos
        );
        set_phase_mask(Phase::MASK_ALL);
    }

    #[test]
    fn finish_marks_open_spans_and_reset_makes_late_drops_stale() {
        reset();
        set_enabled(true);
        let span = begin(Phase::PouExecute);
        let snapshot = finish();
        assert_eq!(snapshot.active_spans, 1);
        assert_eq!(snapshot.unbalanced_spans, 1);
        assert!(!snapshot.is_valid());
        assert!(
            !finish().is_valid(),
            "stopping twice cannot erase a lost span"
        );

        reset();
        set_enabled(true);
        drop(span);
        let clean = finish();
        assert!(clean.is_valid());
    }
}
