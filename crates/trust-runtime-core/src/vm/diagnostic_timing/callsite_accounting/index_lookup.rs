//! Exact paths for the two prepared owner/declaration lookups under investigation.
use super::{
    add_count, callsite_accounting_log_enabled, note_busy, note_overflow, with_recorder, CallSite,
    DemandRange, Recorder,
};
use core::cell::Cell;
use core::panic::Location;

/// Maximum operation/path aggregates for the two observed lookup callers.
pub const INDEX_LOOKUP_RECORD_CAPACITY: usize = 16;
/// Prepared lookup whose unchanged admission has already completed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IndexLookupOperation {
    /// Constant-write declaration permission lookup.
    WritableDeclaration,
    /// Encoded instance owner to construction root lookup.
    InstanceRoot,
}
/// Actual indexed or direct path through the observed prepared lookup.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IndexLookupPath {
    /// Lookup stopped before an indexed path, including invalid slots/owners.
    MissingInput,
    /// Global declaration was obtained directly from its dense layout slot.
    DenseGlobal,
    /// Owner/group resolution produced no declaration range.
    MissingOwner,
    /// Owner declaration was obtained directly from a dense group slot.
    DenseOwnerSlot,
    /// Owner declaration required the sorted sparse-slot index.
    SparseOwnerSlot,
    /// Root owner required the sorted root-owner index.
    RootOwner,
}
/// Exact geometry of completed lookups; no additional work is charged.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IndexLookupRecord {
    /// Source file of the observed engine caller.
    pub file: &'static str,
    /// One-based caller line.
    pub line: u32,
    /// One-based caller column.
    pub column: u32,
    /// One of the two observed operations.
    pub operation: IndexLookupOperation,
    /// Actual selected path.
    pub path: IndexLookupPath,
    /// Whether owner-plan resolution used binary search instead of dense POU access.
    pub owner_binary_search: bool,
    /// Number of completed lookups.
    pub calls: u64,
    /// Number returning an existing declaration/root.
    pub found: u64,
    /// Owner-plan index cardinality, zero when that index was not visited.
    pub owner_table_len: DemandRange,
    /// Actual owner-plan comparator calls, zero for a direct owner.
    pub owner_comparisons: DemandRange,
    /// Declaration group or root-owner table cardinality.
    pub target_table_len: DemandRange,
    /// Actual sparse-slot/root-owner comparator calls, zero for a direct slot.
    pub target_comparisons: DemandRange,
}
impl IndexLookupRecord {
    pub(super) const EMPTY: Self = Self {
        file: "",
        line: 0,
        column: 0,
        operation: IndexLookupOperation::WritableDeclaration,
        path: IndexLookupPath::MissingInput,
        owner_binary_search: false,
        calls: 0,
        found: 0,
        owner_table_len: DemandRange::EMPTY,
        owner_comparisons: DemandRange::EMPTY,
        target_table_len: DemandRange::EMPTY,
        target_comparisons: DemandRange::EMPTY,
    };
}
#[derive(Clone, Copy)]
struct Context {
    site: CallSite,
    operation: IndexLookupOperation,
    owner_query: Option<u32>,
    path: IndexLookupPath,
    owner_binary_search: bool,
    owner_table_len: usize,
    owner_comparisons: usize,
    target_table_len: usize,
    target_comparisons: usize,
}
struct RestoreContext(Option<Context>);
impl Drop for RestoreContext {
    fn drop(&mut self) {
        if CONTEXT.try_with(|slot| slot.set(self.0)).is_err() {
            note_busy();
        }
    }
}
thread_local! { static CONTEXT: Cell<Option<Context>> = const { Cell::new(None) }; }

#[cfg(test)]
#[track_caller]
pub(crate) fn observe_index_lookup<T>(
    operation: IndexLookupOperation,
    action: impl FnOnce() -> Option<T>,
) -> Option<T> {
    with_index_context(operation, action, Option::is_some)
}
#[track_caller]
pub(crate) fn observe_index_lookup_result<T, E>(
    operation: IndexLookupOperation,
    action: impl FnOnce() -> Result<Option<T>, E>,
) -> Result<Option<T>, E> {
    with_index_context(operation, action, |result| {
        result.as_ref().is_ok_and(Option::is_some)
    })
}
#[track_caller]
fn with_index_context<T>(
    operation: IndexLookupOperation,
    action: impl FnOnce() -> T,
    found: impl FnOnce(&T) -> bool,
) -> T {
    if !callsite_accounting_log_enabled() {
        return action();
    }
    let context = Context {
        site: CallSite::caller(Location::caller()),
        operation,
        owner_query: None,
        path: IndexLookupPath::MissingInput,
        owner_binary_search: false,
        owner_table_len: 0,
        owner_comparisons: 0,
        target_table_len: 0,
        target_comparisons: 0,
    };
    let Ok(previous) = CONTEXT.try_with(|slot| slot.replace(Some(context))) else {
        note_busy();
        return action();
    };
    let _restore = RestoreContext(previous);
    let result = action();
    match CONTEXT.try_with(Cell::get) {
        Ok(Some(context)) => {
            with_recorder(|recorder| recorder.record_index_lookup(context, found(&result)))
        }
        _ => note_busy(),
    }
    result
}
pub(crate) fn record_index_owner_query(pou_id: u32) {
    update_context(|context| context.owner_query = Some(pou_id));
}
pub(crate) fn record_index_owner_geometry(
    binary_search: bool,
    table_len: usize,
    comparisons: usize,
) {
    update_context(|context| {
        context.owner_binary_search = binary_search;
        context.owner_table_len = table_len;
        context.owner_comparisons = comparisons;
    });
}
pub(crate) fn record_index_target_geometry(
    path: IndexLookupPath,
    table_len: usize,
    comparisons: usize,
) {
    update_context(|context| {
        context.path = path;
        context.target_table_len = table_len;
        context.target_comparisons = comparisons;
    });
}
fn update_context(update: impl FnOnce(&mut Context)) {
    if !callsite_accounting_log_enabled() {
        return;
    }
    if CONTEXT
        .try_with(|slot| {
            if let Some(mut context) = slot.get() {
                update(&mut context);
                slot.set(Some(context));
            }
        })
        .is_err()
    {
        note_busy();
    }
}
impl Recorder {
    fn record_index_lookup(&mut self, context: Context, found: bool) {
        if let Some(pou_id) = context.owner_query {
            self.record_owner_query(super::attribution::OwnerQueryEvent {
                site: context.site,
                operation: context.operation,
                pou_id,
                binary_search: context.owner_binary_search,
                path: context.path,
                found,
                table_len: context.owner_table_len,
                comparisons: context.owner_comparisons,
            });
        }
        let index = match self.index_lookups[..self.index_lookup_len]
            .iter()
            .position(|r| {
                r.calls != 0
                    && r.file == context.site.file
                    && r.line == context.site.line
                    && r.column == context.site.column
                    && r.operation == context.operation
                    && r.path == context.path
                    && r.owner_binary_search == context.owner_binary_search
            }) {
            Some(index) => index,
            None if self.index_lookup_len < INDEX_LOOKUP_RECORD_CAPACITY => {
                let index = self.index_lookup_len;
                self.index_lookups[index] = IndexLookupRecord {
                    file: context.site.file,
                    line: context.site.line,
                    column: context.site.column,
                    operation: context.operation,
                    path: context.path,
                    owner_binary_search: context.owner_binary_search,
                    ..IndexLookupRecord::EMPTY
                };
                self.index_lookup_len += 1;
                index
            }
            None => {
                note_overflow(&mut self.overflow_count);
                return;
            }
        };
        let record = &mut self.index_lookups[index];
        if !add_count(&mut record.calls, 1)
            || !add_count(&mut record.found, u64::from(found))
            || !record.owner_table_len.add(context.owner_table_len)
            || !record.owner_comparisons.add(context.owner_comparisons)
            || !record.target_table_len.add(context.target_table_len)
            || !record.target_comparisons.add(context.target_comparisons)
        {
            note_overflow(&mut self.overflow_count);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::{finish_callsite_accounting_log, start_callsite_accounting_log, RECORDER};
    use super::*;

    #[test]
    fn index_lookup_context_restores_nested_state_and_capacity_loss_is_invalid() {
        start_callsite_accounting_log();
        observe_index_lookup(IndexLookupOperation::WritableDeclaration, || {
            record_index_target_geometry(IndexLookupPath::DenseGlobal, 100, 0);
            observe_index_lookup(IndexLookupOperation::InstanceRoot, || {
                record_index_target_geometry(IndexLookupPath::RootOwner, 3, 3);
                None::<()>
            });
            Some(())
        });
        assert!(CONTEXT.with(Cell::get).is_none());
        let snapshot = finish_callsite_accounting_log();
        assert!(snapshot.is_valid());
        let rows = &snapshot.index_lookups[..snapshot.index_lookup_len];
        let outer = rows
            .iter()
            .find(|r| r.operation == IndexLookupOperation::WritableDeclaration)
            .unwrap();
        assert_eq!(outer.path, IndexLookupPath::DenseGlobal);
        assert_eq!(outer.found, 1);
        assert_eq!(outer.target_table_len.sum, 100);
        let inner = rows
            .iter()
            .find(|r| r.operation == IndexLookupOperation::InstanceRoot)
            .unwrap();
        assert_eq!(inner.found, 0);
        assert_eq!(inner.target_comparisons.sum, 3);

        start_callsite_accounting_log();
        RECORDER.with(|recorder| {
            let mut recorder = recorder.borrow_mut();
            for line in 1..=INDEX_LOOKUP_RECORD_CAPACITY + 1 {
                recorder.record_index_lookup(
                    Context {
                        site: CallSite {
                            file: "lookup.rs",
                            line: line as u32,
                            column: 1,
                        },
                        operation: IndexLookupOperation::InstanceRoot,
                        owner_query: None,
                        path: IndexLookupPath::RootOwner,
                        owner_binary_search: false,
                        owner_table_len: 0,
                        owner_comparisons: 0,
                        target_table_len: 3,
                        target_comparisons: 3,
                    },
                    true,
                );
            }
        });
        let snapshot = finish_callsite_accounting_log();
        assert_eq!(snapshot.index_lookup_len, INDEX_LOOKUP_RECORD_CAPACITY);
        assert_eq!(snapshot.overflow_count, 1);
        assert!(!snapshot.is_valid());
    }
}
