//! Statement lookup by code offset, built once from a module's `LINE_TABLE`
//! (specification 12 §6.13).
//!
//! The debugger asks, for each executed instruction, whether its offset starts a
//! statement and which statement contains it. [`StatementIndex`] answers both by
//! binary search. It is built once at load, and a query neither scans the table nor
//! allocates. Its answers are those of [`LineResolver`](crate::debug::LineResolver),
//! which stays the linear reference.

use alloc::collections::{BTreeMap, BTreeSet};
use alloc::vec::Vec;

use crate::bytecode::{LineStatement, LineTable};

/// Statement lookup over one module's `LINE_TABLE` (specification 12 §6.13).
///
/// A statement is named by its table index, its position in
/// [`LineTable::statements`](crate::bytecode::LineTable::statements). Build the index
/// once with [`StatementIndex::new`] and keep it: every query is a binary search.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatementIndex {
    /// Each POU that owns at least one statement, keyed by `pou_id`.
    pous: BTreeMap<u32, PouStatements>,
}

/// One POU's statements: where each one starts, and which one contains each offset.
#[derive(Debug, Clone, PartialEq, Eq)]
struct PouStatements {
    /// The distinct statement start offsets, ascending.
    starts: Vec<u32>,
    /// For each entry of `starts`, the table index of the innermost statement that
    /// starts there.
    owners: Vec<u32>,
    /// Boundaries in ascending offset order. From a segment's offset up to the next
    /// segment's offset, the innermost statement whose span contains the offset is
    /// the segment's `statement`.
    segments: Vec<Segment>,
}

/// The innermost statement that contains every offset from `offset` up to the next
/// segment's offset; `statement` is `None` when no span contains them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Segment {
    offset: u32,
    statement: Option<u32>,
}

impl StatementIndex {
    /// Build the index from `table`, the table the decoder has accepted (specification
    /// 12 §6.13). This is the only step that allocates.
    #[must_use]
    pub fn new(table: &LineTable) -> Self {
        // Each POU keeps its entries in table order, so a later entry is the inner one.
        let mut by_pou: BTreeMap<u32, Vec<(u32, &LineStatement)>> = BTreeMap::new();
        for (index, statement) in (0u32..).zip(&table.statements) {
            by_pou
                .entry(statement.pou_id)
                .or_default()
                .push((index, statement));
        }
        let pous = by_pou
            .into_iter()
            .map(|(pou_id, entries)| (pou_id, PouStatements::new(&entries)))
            .collect();
        Self { pous }
    }

    /// The table index of the innermost statement that starts at `code_offset` in
    /// `pou_id`, or `None` when no statement starts there.
    ///
    /// Statements that share a start nest, and the innermost (last) entry owns the
    /// start (specification 12 §6.13).
    #[must_use]
    pub fn statement_at_start(&self, pou_id: u32, code_offset: u32) -> Option<u32> {
        let pou = self.pous.get(&pou_id)?;
        let position = pou.starts.binary_search(&code_offset).ok()?;
        pou.owners.get(position).copied()
    }

    /// The table index of the innermost statement whose code span contains
    /// `code_offset` in `pou_id`, or `None` when no span contains it.
    ///
    /// A span is half-open, `[start_code_offset, end_code_offset)`, so its end offset
    /// lies outside it (specification 12 §6.13).
    #[must_use]
    pub fn statement_containing(&self, pou_id: u32, code_offset: u32) -> Option<u32> {
        let pou = self.pous.get(&pou_id)?;
        // The segments ascend by offset, so the last one that starts at or before
        // `code_offset` holds it. A count of zero means it precedes every boundary.
        let count = pou
            .segments
            .partition_point(|segment| segment.offset <= code_offset);
        let segment = count.checked_sub(1)?;
        pou.segments.get(segment)?.statement
    }

    /// The statement start offsets of `pou_id`, ascending and each listed once: the
    /// offsets a step may stop at, as
    /// [`LineResolver::statement_starts`](crate::debug::LineResolver::statement_starts)
    /// yields them.
    #[must_use]
    pub fn starts(&self, pou_id: u32) -> &[u32] {
        match self.pous.get(&pou_id) {
            Some(pou) => &pou.starts,
            None => &[],
        }
    }
}

impl PouStatements {
    /// Build one POU's lookup tables from its `entries`, each with its table index, in
    /// table order.
    fn new(entries: &[(u32, &LineStatement)]) -> Self {
        // A later entry with the same start is the inner one, so it replaces the
        // earlier entry in the map.
        let mut owner_of_start: BTreeMap<u32, u32> = BTreeMap::new();
        for &(index, statement) in entries {
            owner_of_start.insert(statement.start_code_offset, index);
        }
        let (starts, owners): (Vec<u32>, Vec<u32>) = owner_of_start.into_iter().unzip();

        // An entry covers offsets with `start <= offset < end`, so an empty span covers
        // none. Each other entry joins the active set at its start and leaves it at its
        // end.
        let mut events: Vec<(u32, bool, u32)> = Vec::with_capacity(2 * entries.len());
        for &(index, statement) in entries {
            if statement.start_code_offset < statement.end_code_offset {
                events.push((statement.start_code_offset, true, index));
                events.push((statement.end_code_offset, false, index));
            }
        }
        events.sort_unstable();

        // The active set is ordered by table index, so its last member is the innermost
        // statement containing the offsets of the segment.
        let mut active: BTreeSet<u32> = BTreeSet::new();
        let mut segments = Vec::new();
        for group in events.chunk_by(|a, b| a.0 == b.0) {
            for &(_, enters, index) in group {
                if enters {
                    active.insert(index);
                } else {
                    active.remove(&index);
                }
            }
            segments.push(Segment {
                offset: group[0].0,
                statement: active.last().copied(),
            });
        }
        Self {
            starts,
            owners,
            segments,
        }
    }
}

/// A statement's source span as a chip keeps it: the file index of its `LINE_TABLE` entry
/// and its half-open span of 1-based `(line, column)` positions (specification 12 §6.13).
/// This is the form that the statement identity of specification 13 takes, without the
/// generation the breakpoint set gives it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StatementSpan {
    /// Index into the table's file entries.
    pub file_index: u32,
    /// The first position of the statement: its first line and column.
    pub start: (u32, u32),
    /// The position just after the statement: its end line and column.
    pub end: (u32, u32),
}

/// The statement starts of one prepared module: its [`StatementIndex`] and the source span
/// of every statement, by table index. A module without a `LINE_TABLE` has none.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatementTable {
    index: StatementIndex,
    /// The source span of each statement, by its table index.
    spans: Vec<StatementSpan>,
}

/// Bytes that [`StatementTable::new`] takes per statement, as an upper bound for the 64-bit
/// layout: the span kept for the statement (20), its start and owner (8), its two segments
/// (24), and the build's scratch (the POU grouping 16, the owner of each start 8, the two
/// sweep events 24 and the active set 4).
const BYTES_PER_STATEMENT: usize = 104;

/// Bytes that [`StatementTable::new`] takes per POU that owns a statement, as an upper bound
/// for the 64-bit layout: the POU's key (4) and its per-POU lists (72).
const BYTES_PER_POU: usize = 76;

// The fixed charges must cover what the build takes on every target, so admission does not
// depend on the pointer width. The 64-bit layout is the larger one.
const _: () = assert!(
    core::mem::size_of::<StatementSpan>()
        + core::mem::size_of::<(u32, &LineStatement)>()
        + 2 * core::mem::size_of::<u32>()
        + 2 * core::mem::size_of::<Segment>()
        + core::mem::size_of::<(u32, u32)>()
        + 2 * core::mem::size_of::<(u32, bool, u32)>()
        + core::mem::size_of::<u32>()
        <= BYTES_PER_STATEMENT
);
const _: () =
    assert!(core::mem::size_of::<u32>() + core::mem::size_of::<PouStatements>() <= BYTES_PER_POU);

impl StatementTable {
    /// Build the table from `table`, which the decoder and the validator have accepted
    /// (specification 12 §6.13).
    #[must_use]
    pub fn new(table: &LineTable) -> Self {
        let spans = table
            .statements
            .iter()
            .map(|statement| StatementSpan {
                file_index: statement.file_index,
                start: (statement.start_line, statement.start_column),
                end: (statement.end_line, statement.end_column),
            })
            .collect();
        Self {
            index: StatementIndex::new(table),
            spans,
        }
    }

    /// The `(bytes, work)` that [`StatementTable::new`] takes for `table`, counted without
    /// allocating, so that preparation charges them before the build reserves them. The
    /// work is one unit per statement, as the budget charges records. `None` when the total
    /// overflows.
    #[must_use]
    pub fn preparation_demand(table: &LineTable) -> Option<(usize, usize)> {
        // A validated table lists each POU's statements together, so each run is one POU.
        let pous = table
            .statements
            .chunk_by(|left, right| left.pou_id == right.pou_id)
            .count();
        let statements = table.statements.len();
        let bytes = statements
            .checked_mul(BYTES_PER_STATEMENT)?
            .checked_add(pous.checked_mul(BYTES_PER_POU)?)?;
        Some((bytes, statements))
    }

    /// The source span of the statement that starts at `code_offset` in `pou_id`, or
    /// `None` when no statement starts there (specification 12 §6.13).
    #[must_use]
    pub fn statement_start(&self, pou_id: u32, code_offset: u32) -> Option<StatementSpan> {
        let statement = self.index.statement_at_start(pou_id, code_offset)?;
        self.spans.get(statement as usize).copied()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A table whose statements have the given `(pou_id, start, end)` code spans, in
    /// table order. The index reads no source position, so the entries carry none.
    fn table(spans: &[(u32, u32, u32)]) -> LineTable {
        LineTable {
            statements: spans
                .iter()
                .map(
                    |&(pou_id, start_code_offset, end_code_offset)| LineStatement {
                        pou_id,
                        start_code_offset,
                        end_code_offset,
                        ..LineStatement::default()
                    },
                )
                .collect(),
            ..LineTable::default()
        }
    }

    #[test]
    fn a_shared_start_is_owned_by_the_innermost_entry() {
        // Specification 12 §6.13: a label (entry 0) and the statement it labels (entry 1)
        // share the code span [0, 4). The later entry is the inner one, so it owns the
        // start. An index that keeps the first entry for a shared start answers 0 here.
        let index = StatementIndex::new(&table(&[(1, 0, 4), (1, 0, 4)]));
        assert_eq!(index.statement_at_start(1, 0), Some(1));
        assert_eq!(index.statement_containing(1, 0), Some(1));
    }

    #[test]
    fn the_end_offset_of_a_statement_is_outside_it() {
        // Specification 12 §6.13: a span is half-open. Entry 0 spans [2, 5): offsets 2
        // to 4 are inside it, and 5, its end offset, is not. An index with an inclusive
        // end answers 0 at offset 5.
        let index = StatementIndex::new(&table(&[(1, 2, 5)]));
        assert_eq!(index.statement_containing(1, 1), None);
        assert_eq!(index.statement_containing(1, 2), Some(0));
        assert_eq!(index.statement_containing(1, 4), Some(0));
        assert_eq!(index.statement_containing(1, 5), None);
    }

    #[test]
    fn an_offset_between_statements_has_no_statement() {
        // Specification 12 §6.13: an offset that no span covers has no statement. Entry 0
        // spans [0, 2) and entry 1 spans [4, 6), so offset 3 lies between them. An index
        // that answers the nearest preceding entry, the neighbour, gives 0 here.
        let index = StatementIndex::new(&table(&[(1, 0, 2), (1, 4, 6)]));
        assert_eq!(index.statement_containing(1, 3), None);
        assert_eq!(index.statement_at_start(1, 3), None);
    }

    #[test]
    fn a_nested_body_offset_belongs_to_the_body_statement() {
        // Specification 12 §6.13: a FOR (entry 0) spans [1, 9), and its body statement
        // (entry 1) spans [2, 4). Offsets 2 and 3 lie in both, and the innermost, the body,
        // owns them. Offset 5 lies in the FOR alone. An index that answers the enclosing
        // entry gives 0 at offset 2.
        let index = StatementIndex::new(&table(&[(1, 1, 9), (1, 2, 4)]));
        assert_eq!(index.statement_containing(1, 2), Some(1));
        assert_eq!(index.statement_containing(1, 3), Some(1));
        assert_eq!(index.statement_containing(1, 5), Some(0));
    }

    #[test]
    fn an_index_of_an_empty_table_answers_nothing() {
        // Specification 12 §6.13: a table with no statements has no statement start and
        // contains no offset. An index that answers 0, or panics, on an empty table
        // fails here.
        let index = StatementIndex::new(&LineTable::default());
        assert_eq!(index.starts(1), &[] as &[u32]);
        assert_eq!(index.statement_at_start(1, 0), None);
        assert_eq!(index.statement_containing(1, 0), None);
    }

    #[test]
    fn starts_lists_each_start_once_in_ascending_order() {
        // The statement starts are the step stops of `LineResolver::statement_starts`
        // (specification 12 §6.13): ascending, and a shared start yields one offset.
        // Entries 0 and 1 share start 0, entry 2 starts at 2 and entry 3 at 5. An index
        // that lists every entry's start would give 0, 0, 2, 5.
        let index = StatementIndex::new(&table(&[(1, 0, 4), (1, 0, 4), (1, 2, 3), (1, 5, 9)]));
        assert_eq!(index.starts(1), &[0, 2, 5]);
        assert_eq!(index.statement_at_start(1, 2), Some(2));
    }

    #[test]
    fn a_query_names_only_the_statements_of_its_own_pou() {
        // Specification 12 §6.13: each entry belongs to the POU named by its `pou_id`.
        // POU 1 has entry 0 spanning [0, 4), and POU 2 has entry 1 spanning [0, 9). Offset
        // 6 lies in POU 2's span only, so POU 1 has no statement there. An index that
        // ignores `pou_id` answers 1 for POU 1.
        let index = StatementIndex::new(&table(&[(1, 0, 4), (2, 0, 9)]));
        assert_eq!(index.statement_containing(1, 6), None);
        assert_eq!(index.statement_containing(2, 6), Some(1));
        assert_eq!(index.starts(3), &[] as &[u32]);
    }

    #[test]
    fn statement_start_gives_the_span_of_the_innermost_statement_that_starts_there() {
        // Specification 12 §6.13: a label (entry 0) and the statement it labels (entry 1)
        // share the code span [0, 4), and the innermost (last) entry owns the start. Their
        // source spans are 10:1-10:15 and 10:6-10:15, so the start at offset 0 answers the
        // labelled statement's span. An index that keeps the first entry for a shared start
        // answers 10:1 here, and offset 1 starts no statement.
        let statement = |start_column| LineStatement {
            pou_id: 1,
            start_code_offset: 0,
            end_code_offset: 4,
            file_index: 0,
            start_line: 10,
            start_column,
            end_line: 10,
            end_column: 15,
        };
        let line_table = LineTable {
            statements: Vec::from([statement(1), statement(6)]),
            ..LineTable::default()
        };
        let statements = StatementTable::new(&line_table);
        assert_eq!(
            statements.statement_start(1, 0),
            Some(StatementSpan {
                file_index: 0,
                start: (10, 6),
                end: (10, 15),
            })
        );
        assert_eq!(statements.statement_start(1, 1), None);
    }

    #[test]
    fn preparation_demand_charges_the_fixed_bytes_per_statement_and_per_pou() {
        // Three statements in two POUs: 3 x 104 + 2 x 76 = 464 bytes, and one unit of work per
        // statement. An index that charges per statement alone gives 312, and one that charges
        // per POU alone gives 152.
        let line_table = table(&[(1, 0, 1), (1, 1, 2), (2, 0, 1)]);
        assert_eq!(
            StatementTable::preparation_demand(&line_table),
            Some((464, 3))
        );
    }
}
