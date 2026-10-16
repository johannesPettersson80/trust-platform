//! Bounded numeric identities for the owner and array-copy investigation only.
use super::{
    add_count, callsite_accounting_log_enabled, note_busy, note_overflow, CallSite,
    CopyAdmissionOutcome, CopyAdmissionRecord, DemandRange, Recorder, ValueVariant,
};
use alloc::rc::Rc;
use core::{cell::Cell, marker::PhantomData, panic::Location};

/// Maximum distinct identities retained per attribution family.
pub const ATTRIBUTION_RECORD_CAPACITY: usize = 256;
/// Binding consumer of an observed array copy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CopyConsumerKind {
    /// No binding consumer scope is active.
    Unattributed,
    /// User function or resolved method binding.
    UserCall,
    /// User function-block binding.
    FunctionBlock,
    /// Native builtin function-block binding.
    BuiltinFunctionBlock,
    /// Standard function, conversion or temporal split binding.
    StandardLibrary,
}
/// Role of the formal whose value is materialized.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CopyParameterRole {
    /// No formal scope is active.
    Unattributed,
    /// Supplied input argument.
    Input,
    /// Supplied input/output argument.
    InOut,
    /// Output formal initialization.
    Output,
    /// Omitted input retaining a prior field or using a default.
    OmittedInput,
    /// Resolved standard function operand.
    StandardOperand,
}
/// Actual materialization path, independent of its binding consumer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CopyReadKind {
    /// Copy outside a tagged binding-target materialization.
    Unattributed,
    /// Direct caller-local target.
    CallerLocal,
    /// Direct storage target.
    DirectStorage,
    /// General reference target.
    Reference,
    /// Retained function-block input field.
    RetainedField,
}
/// Numeric binding identity, resolved offline against the same saved artifact.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CopyConsumer {
    /// Raw user POU ID, if the consumer has one.
    pub pou_id: Option<u32>,
    /// Encoded native symbol identity, inherited from native call preparation.
    pub symbol_idx: Option<u32>,
    /// Resolved formal ordinal, not incoming named-argument order.
    pub formal: Option<usize>,
    /// Binding family.
    pub kind: CopyConsumerKind,
    /// Formal direction or omitted-input role.
    pub role: CopyParameterRole,
    /// Actual read path.
    pub read: CopyReadKind,
    /// File of the resolved formal's consumer scope.
    pub file: &'static str,
    /// Source line of that scope.
    pub line: u32,
    /// Source column of that scope.
    pub column: u32,
}
impl CopyConsumer {
    const EMPTY: Self = Self {
        pou_id: None,
        symbol_idx: None,
        formal: None,
        kind: CopyConsumerKind::Unattributed,
        role: CopyParameterRole::Unattributed,
        read: CopyReadKind::Unattributed,
        file: "",
        line: 0,
        column: 0,
    };
}
/// Array events retaining the unchanged coarse copy admission fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ArrayConsumerRecord {
    /// Numeric consuming identity; missing scopes remain explicit.
    pub consumer: CopyConsumer,
    /// Same source, shape, bytes and outcome counters as coarse copy accounting.
    pub copy: CopyAdmissionRecord,
}
/// Raw POU query within an observed declaration-permission lookup.
/// Root-owner IDs have another identity domain; unobserved lookups are not covered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OwnerQueryRecord {
    /// Source file of the observed query.
    pub file: &'static str,
    /// Query source line.
    pub line: u32,
    /// Query source column.
    pub column: u32,
    /// Observed lookup operation, independent of its source location.
    pub operation: super::IndexLookupOperation,
    /// Raw POU passed to the owning declaration plan (not a vector ordinal).
    pub pou_id: u32,
    /// True only when an owner binary search was performed.
    pub binary_search: bool,
    /// Final declaration path.
    pub path: super::IndexLookupPath,
    /// Query count.
    pub calls: u64,
    /// Queries returning a declaration.
    pub found: u64,
    /// Owner-plan table cardinality, zero when the lookup was not reached.
    pub table_len: DemandRange,
    /// Actual comparator calls, zero on the direct path.
    pub comparisons: DemandRange,
}

pub(super) struct OwnerQueryEvent {
    pub(super) site: CallSite,
    pub(super) operation: super::IndexLookupOperation,
    pub(super) pou_id: u32,
    pub(super) binary_search: bool,
    pub(super) path: super::IndexLookupPath,
    pub(super) found: bool,
    pub(super) table_len: usize,
    pub(super) comparisons: usize,
}

thread_local! { static CONSUMER: Cell<Option<CopyConsumer>> = const { Cell::new(None) }; }
/// Restores the prior diagnostic identity on nested returns and errors.
pub(crate) struct Scope {
    previous: Option<CopyConsumer>,
    active: bool,
    _not_send_or_sync: PhantomData<Rc<()>>,
}
impl Drop for Scope {
    fn drop(&mut self) {
        if self.active && CONSUMER.try_with(|slot| slot.set(self.previous)).is_err() {
            note_busy();
        }
    }
}
fn change(update: impl FnOnce(&mut CopyConsumer)) -> Scope {
    if !callsite_accounting_log_enabled() {
        return Scope {
            previous: None,
            active: false,
            _not_send_or_sync: PhantomData,
        };
    }
    match CONSUMER.try_with(|slot| {
        let previous = slot.get();
        let mut next = previous.unwrap_or(CopyConsumer::EMPTY);
        update(&mut next);
        slot.set(Some(next));
        previous
    }) {
        Ok(previous) => Scope {
            previous,
            active: true,
            _not_send_or_sync: PhantomData,
        },
        Err(_) => {
            note_busy();
            Scope {
                previous: None,
                active: false,
                _not_send_or_sync: PhantomData,
            }
        }
    }
}
pub(crate) fn native(symbol_idx: u32) -> Scope {
    change(|c| {
        *c = CopyConsumer {
            symbol_idx: Some(symbol_idx),
            ..CopyConsumer::EMPTY
        };
    })
}
pub(crate) fn consumer(kind: CopyConsumerKind, pou_id: Option<u32>) -> Scope {
    change(|c| {
        c.kind = kind;
        c.pou_id = pou_id;
        c.formal = None;
        c.role = CopyParameterRole::Unattributed;
        c.read = CopyReadKind::Unattributed;
    })
}
#[track_caller]
pub(crate) fn parameter(formal: usize, direction: u8, omitted: bool) -> Scope {
    let site = Location::caller();
    change(|c| {
        c.formal = Some(formal);
        c.role = match direction {
            0 if omitted => CopyParameterRole::OmittedInput,
            0 => CopyParameterRole::Input,
            1 => CopyParameterRole::Output,
            2 => CopyParameterRole::InOut,
            _ => CopyParameterRole::StandardOperand,
        };
        c.file = site.file();
        c.line = site.line();
        c.column = site.column();
    })
}
#[track_caller]
pub(crate) fn variadic_parameter(fixed: usize, offset: usize) -> Scope {
    match fixed.checked_add(offset) {
        Some(formal) => parameter(formal, 3, false),
        None => {
            super::with_recorder(|r| note_overflow(&mut r.overflow_count));
            Scope {
                previous: None,
                active: false,
                _not_send_or_sync: PhantomData,
            }
        }
    }
}
pub(super) fn scope_open() -> bool {
    match CONSUMER.try_with(Cell::get) {
        Ok(c) => c.is_some(),
        Err(_) => {
            note_busy();
            true
        }
    }
}
pub(crate) fn read(kind: CopyReadKind) -> Scope {
    change(|c| c.read = kind)
}

impl Recorder {
    pub(super) fn reset_attribution(&mut self) {
        self.owner_queries.clear();
        self.array_consumers.clear();
        if (self.owner_queries.capacity() < ATTRIBUTION_RECORD_CAPACITY
            && self
                .owner_queries
                .try_reserve_exact(ATTRIBUTION_RECORD_CAPACITY)
                .is_err())
            || (self.array_consumers.capacity() < ATTRIBUTION_RECORD_CAPACITY
                && self
                    .array_consumers
                    .try_reserve_exact(ATTRIBUTION_RECORD_CAPACITY)
                    .is_err())
        {
            note_overflow(&mut self.overflow_count);
        }
    }
    pub(super) fn record_owner_query(&mut self, event: OwnerQueryEvent) {
        let OwnerQueryEvent {
            site,
            operation,
            pou_id,
            binary_search,
            path,
            found,
            table_len,
            comparisons,
        } = event;
        let at = self.owner_queries.iter().position(|r| {
            r.file == site.file
                && r.line == site.line
                && r.column == site.column
                && r.operation == operation
                && r.pou_id == pou_id
                && r.binary_search == binary_search
                && r.path == path
        });
        let at = match at {
            Some(at) => at,
            None if self.owner_queries.len() < ATTRIBUTION_RECORD_CAPACITY
                && self.owner_queries.len() < self.owner_queries.capacity() =>
            {
                let at = self.owner_queries.len();
                self.owner_queries.push(OwnerQueryRecord {
                    file: site.file,
                    line: site.line,
                    column: site.column,
                    operation,
                    pou_id,
                    binary_search,
                    path,
                    calls: 0,
                    found: 0,
                    table_len: DemandRange::EMPTY,
                    comparisons: DemandRange::EMPTY,
                });
                at
            }
            None => {
                note_overflow(&mut self.overflow_count);
                return;
            }
        };
        let r = &mut self.owner_queries[at];
        if !add_count(&mut r.calls, 1)
            || !add_count(&mut r.found, u64::from(found))
            || !r.table_len.add(table_len)
            || !r.comparisons.add(comparisons)
        {
            note_overflow(&mut self.overflow_count);
        }
    }
    pub(super) fn record_array_consumer(
        &mut self,
        site: CallSite,
        shape_len: usize,
        estimated: usize,
        admitted: usize,
        outcome: CopyAdmissionOutcome,
    ) {
        let consumer = match CONSUMER.try_with(Cell::get) {
            Ok(c) => c.unwrap_or(CopyConsumer::EMPTY),
            Err(_) => {
                note_busy();
                return;
            }
        };
        let at = self
            .array_consumers
            .iter()
            .position(|r| r.consumer == consumer && r.copy.matches(site, ValueVariant::Array));
        let at = match at {
            Some(at) => at,
            None if self.array_consumers.len() < ATTRIBUTION_RECORD_CAPACITY
                && self.array_consumers.len() < self.array_consumers.capacity() =>
            {
                let at = self.array_consumers.len();
                self.array_consumers.push(ArrayConsumerRecord {
                    consumer,
                    copy: CopyAdmissionRecord {
                        file: site.file,
                        line: site.line,
                        column: site.column,
                        value_variant: ValueVariant::Array,
                        ..CopyAdmissionRecord::EMPTY
                    },
                });
                at
            }
            None => {
                note_overflow(&mut self.overflow_count);
                return;
            }
        };
        super::add_copy_event(
            &mut self.array_consumers[at].copy,
            shape_len,
            estimated,
            admitted,
            outcome,
            &mut self.overflow_count,
        );
    }
}

#[cfg(test)]
mod tests;
