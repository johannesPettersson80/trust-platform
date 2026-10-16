//! Fixed, count-only caller records for hosted work and value-copy admission.

pub(crate) mod attribution;
use alloc::vec::Vec;
pub use attribution::{
    ArrayConsumerRecord, CopyConsumer, CopyConsumerKind, CopyParameterRole, CopyReadKind,
    OwnerQueryRecord, ATTRIBUTION_RECORD_CAPACITY,
};
mod index_lookup;
#[cfg(test)]
pub(crate) use index_lookup::observe_index_lookup;
pub(crate) use index_lookup::{
    observe_index_lookup_result, record_index_owner_geometry, record_index_owner_query,
    record_index_target_geometry,
};
pub use index_lookup::{
    IndexLookupOperation, IndexLookupPath, IndexLookupRecord, INDEX_LOOKUP_RECORD_CAPACITY,
};
mod lifecycle;
pub use lifecycle::{
    DemandRange, LifecycleDemandRecord, LifecycleDemandUnit, LIFECYCLE_RECORD_CAPACITY,
};

use core::cell::{Cell, RefCell};
use core::panic::Location;

use crate::error::{ExecutionLimitKind, RuntimeError};
use crate::value::Value;

/// Maximum distinct caller records kept for each event family.
pub const CALLSITE_RECORD_CAPACITY: usize = 128;

/// Stable work-charge result category for post-run export.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkChargeOutcome {
    /// The requested work was consumed and the operation completed.
    Charged,
    /// The work allowance rejected the charge without consuming it.
    WorkRefused,
    /// Work was consumed, but the following deadline sample expired.
    DeadlineRefused,
    /// The work operation returned a different error.
    OtherRefusal,
}

impl WorkChargeOutcome {
    /// Stable label for external diagnostic export.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Charged => "charged",
            Self::WorkRefused => "work_refused",
            Self::DeadlineRefused => "deadline_refused",
            Self::OtherRefusal => "other_refusal",
        }
    }
}

/// Stable checked-copy result category for post-run export.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CopyAdmissionOutcome {
    /// Work and backing allocation were admitted.
    Admitted,
    /// Work allowance rejected the copy before materialization.
    WorkRefused,
    /// Work was consumed, but the deadline sample expired.
    DeadlineRefused,
    /// Structural nesting exceeded the copy-accounting limit.
    DepthRefused,
    /// Backing allocation was rejected before materialization.
    AllocationRefused,
    /// Checked size arithmetic overflowed.
    Overflow,
    /// Copy admission returned a different error.
    OtherRefusal,
}

impl CopyAdmissionOutcome {
    /// Stable label for external diagnostic export.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Admitted => "admitted",
            Self::WorkRefused => "work_refused",
            Self::DeadlineRefused => "deadline_refused",
            Self::DepthRefused => "depth_refused",
            Self::AllocationRefused => "allocation_refused",
            Self::Overflow => "overflow",
            Self::OtherRefusal => "other_refusal",
        }
    }
}

/// Runtime value variant observed at a checked-copy boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ValueVariant {
    /// IEC BOOL value.
    Bool,
    /// IEC SINT value.
    SInt,
    /// IEC INT value.
    Int,
    /// IEC DINT value.
    DInt,
    /// IEC LINT value.
    LInt,
    /// IEC USINT value.
    USInt,
    /// IEC UINT value.
    UInt,
    /// IEC UDINT value.
    UDInt,
    /// IEC ULINT value.
    ULInt,
    /// IEC REAL value.
    Real,
    /// IEC LREAL value.
    LReal,
    /// IEC BYTE value.
    Byte,
    /// IEC WORD value.
    Word,
    /// IEC DWORD value.
    DWord,
    /// IEC LWORD value.
    LWord,
    /// IEC TIME value.
    Time,
    /// IEC LTIME value.
    LTime,
    /// IEC DATE value.
    Date,
    /// IEC LDATE value.
    LDate,
    /// IEC TOD value.
    Tod,
    /// IEC LTOD value.
    LTod,
    /// IEC DT value.
    Dt,
    /// IEC LDT value.
    Ldt,
    /// IEC STRING value.
    String,
    /// IEC WSTRING value.
    WString,
    /// IEC CHAR value.
    Char,
    /// IEC WCHAR value.
    WChar,
    /// Array value.
    Array,
    /// Structure value.
    Struct,
    /// Enumeration value.
    Enum,
    /// Absent reference value.
    ReferenceNone,
    /// Present reference value.
    ReferenceSome,
    /// Function-block or class instance value.
    Instance,
    /// Null sentinel value.
    Null,
}

impl ValueVariant {
    /// Stable label for external diagnostic export.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Bool => "bool",
            Self::SInt => "sint",
            Self::Int => "int",
            Self::DInt => "dint",
            Self::LInt => "lint",
            Self::USInt => "usint",
            Self::UInt => "uint",
            Self::UDInt => "udint",
            Self::ULInt => "ulint",
            Self::Real => "real",
            Self::LReal => "lreal",
            Self::Byte => "byte",
            Self::Word => "word",
            Self::DWord => "dword",
            Self::LWord => "lword",
            Self::Time => "time",
            Self::LTime => "ltime",
            Self::Date => "date",
            Self::LDate => "ldate",
            Self::Tod => "tod",
            Self::LTod => "ltod",
            Self::Dt => "dt",
            Self::Ldt => "ldt",
            Self::String => "string",
            Self::WString => "wstring",
            Self::Char => "char",
            Self::WChar => "wchar",
            Self::Array => "array",
            Self::Struct => "struct",
            Self::Enum => "enum",
            Self::ReferenceNone => "reference_none",
            Self::ReferenceSome => "reference_some",
            Self::Instance => "instance",
            Self::Null => "null",
        }
    }
}

/// One source location whose logical work charges were aggregated.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WorkChargeRecord {
    /// Source file containing the charge caller.
    pub file: &'static str,
    /// One-based source line of the charge caller.
    pub line: u32,
    /// One-based source column of the charge caller.
    pub column: u32,
    /// Number of charge calls at this location.
    pub calls: u64,
    /// Sum of requested work units.
    pub requested_units: u64,
    /// Sum of consumed units; a deadline refusal still consumes its charge.
    pub consumed_units: u64,
    /// Largest single charge request.
    pub max_units: usize,
    /// Calls that charged successfully.
    pub charged: u64,
    /// Calls refused by the work allowance.
    pub work_refused: u64,
    /// Calls whose post-charge deadline check expired.
    pub deadline_refused: u64,
    /// Calls refused for another reason.
    pub other_refusal: u64,
}

impl WorkChargeRecord {
    const EMPTY: Self = Self {
        file: "",
        line: 0,
        column: 0,
        calls: 0,
        requested_units: 0,
        consumed_units: 0,
        max_units: 0,
        charged: 0,
        work_refused: 0,
        deadline_refused: 0,
        other_refusal: 0,
    };

    fn matches(self, site: CallSite) -> bool {
        self.calls != 0
            && self.file == site.file
            && self.line == site.line
            && self.column == site.column
    }
}

/// One source location and value shape whose copy-admission events were aggregated.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CopyAdmissionRecord {
    /// Source file containing the checked-copy caller.
    pub file: &'static str,
    /// One-based source line of the checked-copy caller.
    pub line: u32,
    /// One-based source column of the checked-copy caller.
    pub column: u32,
    /// Runtime value variant seen by the copy hook.
    pub value_variant: ValueVariant,
    /// Array elements, structure fields, UTF-8 bytes, or reference path segments.
    pub shape_len_sum: u64,
    /// Largest single shape length.
    pub shape_len_max: usize,
    /// Number of checked-copy calls at this location and variant.
    pub calls: u64,
    /// Sum of estimated clone bytes before allocation admission.
    pub estimated_bytes: u64,
    /// Sum of clone bytes admitted by the allocation limit.
    pub admitted_bytes: u64,
    /// Calls admitted for materialization.
    pub admitted: u64,
    /// Calls refused by the work allowance.
    pub work_refused: u64,
    /// Calls whose post-charge deadline check expired.
    pub deadline_refused: u64,
    /// Calls refused by the structural nesting bound.
    pub depth_refused: u64,
    /// Calls refused by the allocation-byte limit.
    pub allocation_refused: u64,
    /// Calls refused by checked size overflow.
    pub overflow: u64,
    /// Calls refused for another reason.
    pub other_refusal: u64,
}

impl CopyAdmissionRecord {
    const EMPTY: Self = Self {
        file: "",
        line: 0,
        column: 0,
        value_variant: ValueVariant::Null,
        shape_len_sum: 0,
        shape_len_max: 0,
        calls: 0,
        estimated_bytes: 0,
        admitted_bytes: 0,
        admitted: 0,
        work_refused: 0,
        deadline_refused: 0,
        depth_refused: 0,
        allocation_refused: 0,
        overflow: 0,
        other_refusal: 0,
    };

    fn matches(self, site: CallSite, variant: ValueVariant) -> bool {
        self.calls != 0
            && self.file == site.file
            && self.line == site.line
            && self.column == site.column
            && self.value_variant == variant
    }
}

/// Bounded caller-attributed snapshot; added identity vectors move at export.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallsiteAccountingSnapshot {
    /// Populated work records occupy `work[..work_len]`.
    pub work: [WorkChargeRecord; CALLSITE_RECORD_CAPACITY],
    /// Number of populated work records.
    pub work_len: usize,
    /// Populated copy records occupy `copies[..copy_len]`.
    pub copies: [CopyAdmissionRecord; CALLSITE_RECORD_CAPACITY],
    /// Number of populated copy records.
    pub copy_len: usize,
    /// Populated lifecycle directory/page demand records occupy `lifecycle[..lifecycle_len]`.
    pub lifecycle: [LifecycleDemandRecord; LIFECYCLE_RECORD_CAPACITY],
    /// Number of populated lifecycle records.
    pub lifecycle_len: usize,
    /// Populated exact index path records occupy `index_lookups[..index_lookup_len]`.
    pub index_lookups: [IndexLookupRecord; INDEX_LOOKUP_RECORD_CAPACITY],
    /// Number of populated index path records.
    pub index_lookup_len: usize,
    /// Bounded raw owner query aggregates, allocated before the observed operation.
    pub owner_queries: Vec<OwnerQueryRecord>,
    /// Bounded array-consumer aggregates; missing scopes are explicit.
    pub array_consumers: Vec<ArrayConsumerRecord>,
    /// Events lost due to table or counter saturation.
    pub overflow_count: u64,
    /// Events lost because the thread-local recorder was busy or unavailable.
    pub recorder_busy: u64,
}

impl CallsiteAccountingSnapshot {
    const fn empty() -> Self {
        Self {
            work: [WorkChargeRecord::EMPTY; CALLSITE_RECORD_CAPACITY],
            work_len: 0,
            copies: [CopyAdmissionRecord::EMPTY; CALLSITE_RECORD_CAPACITY],
            copy_len: 0,
            lifecycle: [LifecycleDemandRecord::EMPTY; LIFECYCLE_RECORD_CAPACITY],
            lifecycle_len: 0,
            index_lookups: [IndexLookupRecord::EMPTY; INDEX_LOOKUP_RECORD_CAPACITY],
            index_lookup_len: 0,
            owner_queries: Vec::new(),
            array_consumers: Vec::new(),
            overflow_count: 0,
            recorder_busy: 0,
        }
    }

    /// Whether every caller event fit in the bounded recorder.
    #[must_use]
    pub const fn is_valid(&self) -> bool {
        self.overflow_count == 0 && self.recorder_busy == 0
    }
}

#[derive(Clone, Copy)]
struct CallSite {
    file: &'static str,
    line: u32,
    column: u32,
}

impl CallSite {
    fn caller(location: &'static Location<'static>) -> Self {
        Self {
            file: location.file(),
            line: location.line(),
            column: location.column(),
        }
    }
}

/// Clear caller aggregates and enable count-only recording on this thread.
pub fn start_callsite_accounting_log() {
    let mut reset = false;
    let result = RECORDER.try_with(|recorder| {
        let Ok(mut recorder) = recorder.try_borrow_mut() else {
            return;
        };
        recorder.reset();
        reset = true;
    });
    if result.is_err() || !reset {
        note_busy();
        let _ = ENABLED.try_with(|enabled| enabled.set(false));
        return;
    }
    let _ = FAILURES.try_with(|failures| failures.set(Failures::default()));
    let _ = ENABLED.try_with(|enabled| enabled.set(true));
}

/// Stop recording and drain all event families for post-run export.
/// After a successful drain, another finish without a new start has no records.
/// A busy recorder produces an invalid snapshot and leaves its records for retry.
#[must_use]
pub fn finish_callsite_accounting_log() -> CallsiteAccountingSnapshot {
    let _ = ENABLED.try_with(|enabled| enabled.set(false));
    let mut snapshot = CallsiteAccountingSnapshot::empty();
    let result = RECORDER.try_with(|recorder| {
        let Ok(mut recorder) = recorder.try_borrow_mut() else {
            snapshot.recorder_busy = 1;
            return;
        };
        snapshot = recorder.snapshot();
    });
    if result.is_err() {
        snapshot.recorder_busy = snapshot.recorder_busy.saturating_add(1);
    }
    if attribution::scope_open() {
        snapshot.recorder_busy = snapshot.recorder_busy.saturating_add(1);
    }
    if let Ok(failures) = FAILURES.try_with(|failures| failures.replace(Failures::default())) {
        snapshot.overflow_count = snapshot.overflow_count.saturating_add(failures.overflow);
        snapshot.recorder_busy = snapshot.recorder_busy.saturating_add(failures.busy);
    }
    snapshot
}

/// Whether caller accounting is active on this thread.
#[must_use]
pub fn callsite_accounting_log_enabled() -> bool {
    match ENABLED.try_with(Cell::get) {
        Ok(enabled) => enabled,
        Err(_) => {
            note_busy();
            false
        }
    }
}

/// Record a completed work charge without formatting or allocating.
pub fn record_work_charge(
    location: &'static Location<'static>,
    requested: usize,
    consumed: usize,
    outcome: WorkChargeOutcome,
) {
    if !callsite_accounting_log_enabled() {
        return;
    }
    let site = CallSite::caller(location);
    with_recorder(|recorder| recorder.record_work(site, requested, consumed, outcome));
}

/// Record one checked value-copy admission without walking its payload.
pub fn record_copy_admission(
    location: &'static Location<'static>,
    value: &Value,
    estimated_bytes: usize,
    admitted_bytes: usize,
    outcome: CopyAdmissionOutcome,
) {
    if !callsite_accounting_log_enabled() {
        return;
    }
    let site = CallSite::caller(location);
    let (variant, shape_len) = value_shape(value);
    with_recorder(|recorder| {
        if variant == ValueVariant::Array {
            recorder.record_array_consumer(
                site,
                shape_len,
                estimated_bytes,
                admitted_bytes,
                outcome,
            );
        }
        recorder.record_copy(
            site,
            variant,
            shape_len,
            estimated_bytes,
            admitted_bytes,
            outcome,
        )
    });
}

fn value_shape(value: &Value) -> (ValueVariant, usize) {
    match value {
        Value::Bool(_) => (ValueVariant::Bool, 0),
        Value::SInt(_) => (ValueVariant::SInt, 0),
        Value::Int(_) => (ValueVariant::Int, 0),
        Value::DInt(_) => (ValueVariant::DInt, 0),
        Value::LInt(_) => (ValueVariant::LInt, 0),
        Value::USInt(_) => (ValueVariant::USInt, 0),
        Value::UInt(_) => (ValueVariant::UInt, 0),
        Value::UDInt(_) => (ValueVariant::UDInt, 0),
        Value::ULInt(_) => (ValueVariant::ULInt, 0),
        Value::Real(_) => (ValueVariant::Real, 0),
        Value::LReal(_) => (ValueVariant::LReal, 0),
        Value::Byte(_) => (ValueVariant::Byte, 0),
        Value::Word(_) => (ValueVariant::Word, 0),
        Value::DWord(_) => (ValueVariant::DWord, 0),
        Value::LWord(_) => (ValueVariant::LWord, 0),
        Value::Time(_) => (ValueVariant::Time, 0),
        Value::LTime(_) => (ValueVariant::LTime, 0),
        Value::Date(_) => (ValueVariant::Date, 0),
        Value::LDate(_) => (ValueVariant::LDate, 0),
        Value::Tod(_) => (ValueVariant::Tod, 0),
        Value::LTod(_) => (ValueVariant::LTod, 0),
        Value::Dt(_) => (ValueVariant::Dt, 0),
        Value::Ldt(_) => (ValueVariant::Ldt, 0),
        Value::String(text) => (ValueVariant::String, text.len()),
        Value::WString(text) => (ValueVariant::WString, text.len()),
        Value::Char(_) => (ValueVariant::Char, 0),
        Value::WChar(_) => (ValueVariant::WChar, 0),
        Value::Array(array) => (ValueVariant::Array, array.elements().len()),
        Value::Struct(structure) => (ValueVariant::Struct, structure.fields().len()),
        Value::Enum(_) => (ValueVariant::Enum, 0),
        Value::Reference(None) => (ValueVariant::ReferenceNone, 0),
        Value::Reference(Some(reference)) => (ValueVariant::ReferenceSome, reference.path.len()),
        Value::Instance(_) => (ValueVariant::Instance, 0),
        Value::Null => (ValueVariant::Null, 0),
    }
}

fn with_recorder(update: impl FnOnce(&mut Recorder)) {
    let result = RECORDER.try_with(|recorder| {
        let Ok(mut recorder) = recorder.try_borrow_mut() else {
            note_busy();
            return;
        };
        update(&mut recorder);
    });
    if result.is_err() {
        note_busy();
    }
}

fn note_busy() {
    let _ = FAILURES.try_with(|failures| {
        let mut value = failures.get();
        value.busy = value.busy.saturating_add(1);
        failures.set(value);
    });
}

fn note_overflow(overflow: &mut u64) {
    *overflow = overflow.saturating_add(1);
}

#[derive(Clone, Copy, Default)]
struct Failures {
    overflow: u64,
    busy: u64,
}

struct Recorder {
    work: [WorkChargeRecord; CALLSITE_RECORD_CAPACITY],
    work_len: usize,
    copies: [CopyAdmissionRecord; CALLSITE_RECORD_CAPACITY],
    copy_len: usize,
    lifecycle: [LifecycleDemandRecord; LIFECYCLE_RECORD_CAPACITY],
    lifecycle_len: usize,
    index_lookups: [IndexLookupRecord; INDEX_LOOKUP_RECORD_CAPACITY],
    index_lookup_len: usize,
    owner_queries: Vec<OwnerQueryRecord>,
    array_consumers: Vec<ArrayConsumerRecord>,
    overflow_count: u64,
}

impl Recorder {
    const fn new() -> Self {
        Self {
            work: [WorkChargeRecord::EMPTY; CALLSITE_RECORD_CAPACITY],
            work_len: 0,
            copies: [CopyAdmissionRecord::EMPTY; CALLSITE_RECORD_CAPACITY],
            copy_len: 0,
            lifecycle: [LifecycleDemandRecord::EMPTY; LIFECYCLE_RECORD_CAPACITY],
            lifecycle_len: 0,
            index_lookups: [IndexLookupRecord::EMPTY; INDEX_LOOKUP_RECORD_CAPACITY],
            index_lookup_len: 0,
            owner_queries: Vec::new(),
            array_consumers: Vec::new(),
            overflow_count: 0,
        }
    }

    fn reset(&mut self) {
        self.work.fill(WorkChargeRecord::EMPTY);
        self.work_len = 0;
        self.copies.fill(CopyAdmissionRecord::EMPTY);
        self.copy_len = 0;
        self.lifecycle.fill(LifecycleDemandRecord::EMPTY);
        self.lifecycle_len = 0;
        self.index_lookups.fill(IndexLookupRecord::EMPTY);
        self.index_lookup_len = 0;
        self.overflow_count = 0;
        self.reset_attribution();
    }

    fn record_work(
        &mut self,
        site: CallSite,
        requested: usize,
        consumed: usize,
        outcome: WorkChargeOutcome,
    ) {
        let index = match self.work[..self.work_len]
            .iter()
            .position(|record| record.matches(site))
        {
            Some(index) => index,
            None if self.work_len < CALLSITE_RECORD_CAPACITY => {
                let index = self.work_len;
                self.work[index] = WorkChargeRecord {
                    file: site.file,
                    line: site.line,
                    column: site.column,
                    ..WorkChargeRecord::EMPTY
                };
                self.work_len += 1;
                index
            }
            None => {
                note_overflow(&mut self.overflow_count);
                return;
            }
        };
        let record = &mut self.work[index];
        if !add_count(&mut record.calls, 1)
            || !add_count(&mut record.requested_units, requested as u64)
            || !add_count(&mut record.consumed_units, consumed as u64)
        {
            note_overflow(&mut self.overflow_count);
            return;
        }
        record.max_units = record.max_units.max(requested);
        let counter = match outcome {
            WorkChargeOutcome::Charged => &mut record.charged,
            WorkChargeOutcome::WorkRefused => &mut record.work_refused,
            WorkChargeOutcome::DeadlineRefused => &mut record.deadline_refused,
            WorkChargeOutcome::OtherRefusal => &mut record.other_refusal,
        };
        if !add_count(counter, 1) {
            note_overflow(&mut self.overflow_count);
        }
    }

    fn record_copy(
        &mut self,
        site: CallSite,
        variant: ValueVariant,
        shape_len: usize,
        estimated_bytes: usize,
        admitted_bytes: usize,
        outcome: CopyAdmissionOutcome,
    ) {
        let index = match self.copies[..self.copy_len]
            .iter()
            .position(|record| record.matches(site, variant))
        {
            Some(index) => index,
            None if self.copy_len < CALLSITE_RECORD_CAPACITY => {
                let index = self.copy_len;
                self.copies[index] = CopyAdmissionRecord {
                    file: site.file,
                    line: site.line,
                    column: site.column,
                    value_variant: variant,
                    ..CopyAdmissionRecord::EMPTY
                };
                self.copy_len += 1;
                index
            }
            None => {
                note_overflow(&mut self.overflow_count);
                return;
            }
        };
        add_copy_event(
            &mut self.copies[index],
            shape_len,
            estimated_bytes,
            admitted_bytes,
            outcome,
            &mut self.overflow_count,
        );
    }

    fn snapshot(&mut self) -> CallsiteAccountingSnapshot {
        let snapshot = CallsiteAccountingSnapshot {
            work: self.work,
            work_len: self.work_len,
            copies: self.copies,
            copy_len: self.copy_len,
            lifecycle: self.lifecycle,
            lifecycle_len: self.lifecycle_len,
            index_lookups: self.index_lookups,
            index_lookup_len: self.index_lookup_len,
            owner_queries: core::mem::take(&mut self.owner_queries),
            array_consumers: core::mem::take(&mut self.array_consumers),
            overflow_count: self.overflow_count,
            recorder_busy: 0,
        };
        // All families drain together; fixed-array tails are unpopulated once
        // their lengths are zero, just as the moved attribution vectors are.
        self.work_len = 0;
        self.copy_len = 0;
        self.lifecycle_len = 0;
        self.index_lookup_len = 0;
        self.overflow_count = 0;
        snapshot
    }
}

fn add_copy_event(
    record: &mut CopyAdmissionRecord,
    shape_len: usize,
    estimated_bytes: usize,
    admitted_bytes: usize,
    outcome: CopyAdmissionOutcome,
    overflow_count: &mut u64,
) {
    if !add_count(&mut record.calls, 1)
        || !add_count(&mut record.shape_len_sum, shape_len as u64)
        || !add_count(&mut record.estimated_bytes, estimated_bytes as u64)
        || !add_count(&mut record.admitted_bytes, admitted_bytes as u64)
    {
        note_overflow(overflow_count);
        return;
    }
    record.shape_len_max = record.shape_len_max.max(shape_len);
    let counter = match outcome {
        CopyAdmissionOutcome::Admitted => &mut record.admitted,
        CopyAdmissionOutcome::WorkRefused => &mut record.work_refused,
        CopyAdmissionOutcome::DeadlineRefused => &mut record.deadline_refused,
        CopyAdmissionOutcome::DepthRefused => &mut record.depth_refused,
        CopyAdmissionOutcome::AllocationRefused => &mut record.allocation_refused,
        CopyAdmissionOutcome::Overflow => &mut record.overflow,
        CopyAdmissionOutcome::OtherRefusal => &mut record.other_refusal,
    };
    if !add_count(counter, 1) {
        note_overflow(overflow_count);
    }
}

fn add_count(value: &mut u64, amount: u64) -> bool {
    if let Some(total) = value.checked_add(amount) {
        *value = total;
        true
    } else {
        false
    }
}

pub(crate) fn work_outcome(error: &RuntimeError) -> WorkChargeOutcome {
    match error {
        RuntimeError::ExecutionTimeout => WorkChargeOutcome::DeadlineRefused,
        RuntimeError::ExecutionLimit {
            kind: ExecutionLimitKind::Work,
            ..
        } => WorkChargeOutcome::WorkRefused,
        _ => WorkChargeOutcome::OtherRefusal,
    }
}

pub(crate) fn copy_outcome(error: &RuntimeError) -> CopyAdmissionOutcome {
    match error {
        RuntimeError::ExecutionTimeout => CopyAdmissionOutcome::DeadlineRefused,
        RuntimeError::ExecutionLimit {
            kind: ExecutionLimitKind::Work,
            ..
        } => CopyAdmissionOutcome::WorkRefused,
        RuntimeError::ExecutionLimit {
            kind: ExecutionLimitKind::AllocationBytes,
            ..
        } => CopyAdmissionOutcome::AllocationRefused,
        RuntimeError::TypeMismatch => CopyAdmissionOutcome::DepthRefused,
        RuntimeError::Overflow => CopyAdmissionOutcome::Overflow,
        _ => CopyAdmissionOutcome::OtherRefusal,
    }
}

thread_local! {
    static RECORDER: RefCell<Recorder> = const { RefCell::new(Recorder::new()) };
    static ENABLED: Cell<bool> = const { Cell::new(false) };
    static FAILURES: Cell<Failures> = const { Cell::new(Failures { overflow: 0, busy: 0 }) };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[track_caller]
    fn record_work_at_caller(units: usize) {
        record_work_charge(Location::caller(), units, units, WorkChargeOutcome::Charged);
    }

    #[track_caller]
    fn forwarded_work_caller(units: usize) {
        record_work_at_caller(units);
    }

    #[test]
    fn start_resets_and_aggregates_work_and_copy_callers() {
        start_callsite_accounting_log();
        let first_line = line!() + 2;
        for units in [2, 3] {
            forwarded_work_caller(units);
        }
        let snapshot = finish_callsite_accounting_log();
        assert!(snapshot.is_valid());
        assert_eq!(snapshot.work_len, 1);
        let row = snapshot.work[0];
        assert_eq!(row.file, file!());
        assert_eq!(row.line, first_line);
        assert_eq!(row.calls, 2);
        assert_eq!(row.requested_units, 5);
        assert_eq!(row.consumed_units, 5);
        assert_eq!(row.max_units, 3);
        assert_eq!(row.charged, 2);

        start_callsite_accounting_log();
        assert!(callsite_accounting_log_enabled());
        let copy_line = line!() + 1;
        let callsite = Location::caller();
        record_copy_admission(
            callsite,
            &Value::Array(alloc::boxed::Box::new(
                crate::value::ArrayValue::from_canonical_parts(
                    alloc::vec![Value::Int(0), Value::Int(1)],
                    alloc::vec![(0, 1)],
                ),
            )),
            48,
            48,
            CopyAdmissionOutcome::Admitted,
        );
        let copy = finish_callsite_accounting_log();
        assert_eq!(copy.work_len, 0);
        assert_eq!(copy.copy_len, 1);
        assert_eq!(copy.copies[0].line, copy_line);
        assert_eq!(copy.copies[0].value_variant, ValueVariant::Array);
        assert_eq!(copy.copies[0].shape_len_sum, 2);
        assert_eq!(copy.copies[0].shape_len_max, 2);
        assert_eq!(copy.copies[0].estimated_bytes, 48);
        assert_eq!(copy.copies[0].admitted_bytes, 48);
        assert_eq!(copy.copies[0].admitted, 1);
    }

    #[test]
    fn distinct_sites_over_capacity_invalidate_snapshot() {
        start_callsite_accounting_log();
        RECORDER.with(|recorder| {
            let mut recorder = recorder.borrow_mut();
            for line in 1..=CALLSITE_RECORD_CAPACITY + 1 {
                recorder.record_work(
                    CallSite {
                        file: "synthetic.rs",
                        line: line as u32,
                        column: 1,
                    },
                    1,
                    1,
                    WorkChargeOutcome::Charged,
                );
            }
        });
        let snapshot = finish_callsite_accounting_log();
        assert_eq!(snapshot.work_len, CALLSITE_RECORD_CAPACITY);
        assert_eq!(snapshot.overflow_count, 1);
        assert!(!snapshot.is_valid());
    }
}
