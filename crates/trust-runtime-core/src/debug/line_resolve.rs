//! Breakpoint, step and stop-location resolution over STBC's `LINE_TABLE`.
//!
//! The line table (specification 12 §6.13) is the single input of breakpoint and
//! step resolution on every runtime, trust-runtime and a chip alike: it carries each
//! executable statement's code span and source span, so a runtime reaches the same
//! answer without the ST sources (specification 12 §10; specification 13, "Source
//! Mapping"). [`LineResolver`] reproduces
//! what trust-runtime's source-based resolver answers today, from the table alone,
//! and allocates nothing.

use smol_str::SmolStr;

use crate::bytecode::{LineStatement, LineTable};

/// One resolved statement: its POU, code offset, and 1-based source start and end.
///
/// The source coordinates are the table's own 1-based ones (specification 12
/// §6.13); a caller that wants a 0-based pair converts at its own boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResolvedStatement {
    /// POU_INDEX identity of the owning POU.
    pub pou_id: u32,
    /// Offset within POU_BODIES of the statement's first instruction.
    pub code_offset: u32,
    /// 1-based line of the statement's first byte.
    pub line: u32,
    /// 1-based byte column within that line.
    pub column: u32,
    /// 1-based line of the statement's source span end, trailing trivia included
    /// (specification 12 §6.13).
    pub end_line: u32,
    /// 1-based byte column of that end within its line.
    pub end_column: u32,
}

/// A portable resolver over one module's `LINE_TABLE` (specification 12 §6.13).
///
/// `labels` are the module's DEBUG_STRING_TABLE entries, indexed by each file
/// entry's `path_idx`; a file's answer label is `labels[path_idx]`. The resolver
/// borrows both and allocates nothing, so it runs on a chip.
#[derive(Debug, Clone, Copy)]
pub struct LineResolver<'a> {
    table: &'a LineTable,
    labels: &'a [SmolStr],
}

impl<'a> LineResolver<'a> {
    /// Build a resolver over `table` that names files through `labels`.
    ///
    /// `labels` is the module's DEBUG_STRING_TABLE entry list, which a file entry
    /// indexes with `path_idx` (specification 12 §6.13).
    #[must_use]
    pub fn new(table: &'a LineTable, labels: &'a [SmolStr]) -> Self {
        Self { table, labels }
    }

    /// Resolve a 1-based `(line, column)` breakpoint to the statement today's
    /// source-based resolver selects (specification 13, "Breakpoint evaluation,
    /// source resolution, and hook contract").
    ///
    /// On `line` it selects the statement with the greatest start column not after
    /// `column`, else the smallest start column after it. When no statement starts
    /// on `line` it selects the innermost statement whose source span contains the
    /// position. Otherwise it selects the earliest statement at or after the
    /// position. A label with no file entry or no candidate returns `None`.
    #[must_use]
    pub fn resolve_breakpoint(
        &self,
        file_label: &str,
        line: u32,
        column: u32,
    ) -> Option<ResolvedStatement> {
        let file_index = self.file_index(file_label)?;
        let on_line = |statement: &&LineStatement| {
            statement.file_index == file_index && statement.start_line == line
        };

        let mut before: Option<&LineStatement> = None;
        let mut after: Option<&LineStatement> = None;
        for statement in self.table.statements.iter().filter(on_line) {
            if statement.start_column <= column {
                if before.is_none_or(|best| statement.start_column > best.start_column) {
                    before = Some(statement);
                }
            } else if after.is_none_or(|best| statement.start_column < best.start_column) {
                after = Some(statement);
            }
        }
        if let Some(statement) = before.or(after) {
            return Some(resolved(statement));
        }

        // §6.13 orders a nested pair outermost first, so the last containing entry
        // is the innermost one that owns the position.
        if let Some(statement) = self.table.statements.iter().rfind(|statement| {
            statement.file_index == file_index && contains(statement, line, column)
        }) {
            return Some(resolved(statement));
        }

        self.table
            .statements
            .iter()
            .filter(|statement| {
                statement.file_index == file_index
                    && (statement.start_line, statement.start_column) >= (line, column)
            })
            .min_by_key(|statement| (statement.start_line, statement.start_column))
            .map(resolved)
    }

    /// The stop location of `code_offset` in `pou_id` (specification 12 §6.13).
    ///
    /// A code offset belongs to the innermost statement whose code span contains
    /// it; the answer is that statement's file label and its 1-based source start.
    /// An offset outside every statement of the POU returns `None`.
    #[must_use]
    pub fn location_of(&self, pou_id: u32, code_offset: u32) -> Option<(&'a str, u32, u32)> {
        let statement = self.table.statements.iter().rfind(|statement| {
            statement.pou_id == pou_id
                && statement.start_code_offset <= code_offset
                && code_offset < statement.end_code_offset
        })?;
        Some((
            self.label(statement.file_index)?,
            statement.start_line,
            statement.start_column,
        ))
    }

    /// The code offsets at which a step may stop in `pou_id`, ascending
    /// (specification 12 §6.13).
    ///
    /// A statement that shares its start with a nested statement (a label and its
    /// statement) yields one offset, because the innermost entry owns it.
    pub fn statement_starts(&self, pou_id: u32) -> impl Iterator<Item = u32> + 'a {
        let mut previous = None;
        self.table
            .statements
            .iter()
            .filter(move |statement| statement.pou_id == pou_id)
            .map(|statement| statement.start_code_offset)
            .filter(move |offset| {
                let fresh = previous != Some(*offset);
                previous = Some(*offset);
                fresh
            })
    }

    /// The file entry index whose label is `file_label`, if the table has one.
    fn file_index(&self, file_label: &str) -> Option<u32> {
        let index = self.table.files.iter().position(|file| {
            self.labels
                .get(file.path_idx as usize)
                .is_some_and(|label| label.as_str() == file_label)
        })?;
        u32::try_from(index).ok()
    }

    /// The label of one file entry, if `labels` holds its `path_idx`.
    fn label(&self, file_index: u32) -> Option<&'a str> {
        let file = self.table.files.get(file_index as usize)?;
        self.labels.get(file.path_idx as usize).map(SmolStr::as_str)
    }
}

/// Project a statement onto the resolver's answer (specification 12 §6.13).
fn resolved(statement: &LineStatement) -> ResolvedStatement {
    ResolvedStatement {
        pou_id: statement.pou_id,
        code_offset: statement.start_code_offset,
        line: statement.start_line,
        column: statement.start_column,
        end_line: statement.end_line,
        end_column: statement.end_column,
    }
}

/// Whether a 1-based position lies in a statement's source span.
///
/// The end is inclusive, as the runtime resolves it for compatibility
/// (specification 13, "Breakpoint evaluation, source resolution, and hook
/// contract").
fn contains(statement: &LineStatement, line: u32, column: u32) -> bool {
    (statement.start_line, statement.start_column) <= (line, column)
        && (line, column) <= (statement.end_line, statement.end_column)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bytecode::{LineFile, LineStatement};
    use alloc::{vec, vec::Vec};

    /// A file label list as DEBUG_STRING_TABLE would hold it.
    fn labels() -> Vec<SmolStr> {
        vec![SmolStr::new("main.st")]
    }

    /// One statement with its 1-based source span and half-open code span.
    fn statement(
        start_code_offset: u32,
        end_code_offset: u32,
        start_line: u32,
        start_column: u32,
        end_line: u32,
        end_column: u32,
    ) -> LineStatement {
        LineStatement {
            pou_id: 1,
            start_code_offset,
            end_code_offset,
            file_index: 0,
            start_line,
            start_column,
            end_line,
            end_column,
        }
    }

    /// The statements of the emission test's `SOURCE` program, read off that source:
    /// an assignment (12:1), an IF over lines 13-15, its body (14:1), a FOR over
    /// 16-18, its body (17:1) and a call (19:1) (specification 12 §6.13).
    fn program_table() -> LineTable {
        LineTable {
            files: vec![LineFile { path_idx: 0 }],
            statements: vec![
                statement(0, 1, 12, 1, 12, 11),
                statement(1, 5, 13, 1, 15, 7),
                statement(2, 3, 14, 1, 14, 8),
                statement(5, 9, 16, 1, 18, 8),
                statement(6, 7, 17, 1, 17, 12),
                statement(9, 10, 19, 1, 19, 22),
            ],
        }
    }

    #[test]
    fn resolve_breakpoint_takes_the_on_line_statement_before_the_column() {
        // Specification 13: on the requested line the resolver selects the statement
        // with the greatest start column not after the request, else the smallest
        // start column after it. Line 14 holds one statement at column 1, so any
        // column on that line resolves to `i := 1;` (code offset 2).
        let labels = labels();
        let table = program_table();
        let resolver = LineResolver::new(&table, &labels);
        let expected = ResolvedStatement {
            pou_id: 1,
            code_offset: 2,
            line: 14,
            column: 1,
            end_line: 14,
            end_column: 8,
        };
        assert_eq!(
            resolver.resolve_breakpoint("main.st", 14, 1),
            Some(expected)
        );
        assert_eq!(
            resolver.resolve_breakpoint("main.st", 14, 8),
            Some(expected)
        );
    }

    #[test]
    fn resolve_breakpoint_takes_the_inside_statement_when_one_contains_the_position() {
        // Specification 13: when no statement starts on the line, the narrowest
        // statement containing the clamped position is selected. Line 15 is inside the
        // IF but starts no statement, so a column at or before the IF's end resolves to
        // the IF (code offset 1), not to the IF's earlier body statement.
        let labels = labels();
        let table = program_table();
        let resolver = LineResolver::new(&table, &labels);
        assert_eq!(
            resolver.resolve_breakpoint("main.st", 15, 1),
            Some(ResolvedStatement {
                pou_id: 1,
                code_offset: 1,
                line: 13,
                column: 1,
                end_line: 15,
                end_column: 7,
            })
        );
    }

    #[test]
    fn resolve_breakpoint_takes_the_next_statement_after_a_position_in_no_statement() {
        // Specification 13: when nothing starts on the line and nothing contains the
        // position, the globally earliest later statement is selected. A column past
        // the IF's end on line 15 reaches no statement, so the FOR at 16:1 (code
        // offset 5) is the answer.
        let labels = labels();
        let table = program_table();
        let resolver = LineResolver::new(&table, &labels);
        assert_eq!(
            resolver.resolve_breakpoint("main.st", 15, 50),
            Some(ResolvedStatement {
                pou_id: 1,
                code_offset: 5,
                line: 16,
                column: 1,
                end_line: 18,
                end_column: 8,
            })
        );
    }

    #[test]
    fn resolve_breakpoint_returns_none_without_a_candidate_or_a_file() {
        // Specification 13: an empty candidate set returns no location. There is no
        // statement on or after line 20, and a label with no file entry resolves to
        // nothing.
        let labels = labels();
        let table = program_table();
        let resolver = LineResolver::new(&table, &labels);
        assert_eq!(resolver.resolve_breakpoint("main.st", 20, 1), None);
        assert_eq!(resolver.resolve_breakpoint("main.st", 21, 1), None);
        assert_eq!(resolver.resolve_breakpoint("other.st", 14, 1), None);
    }

    #[test]
    fn resolve_breakpoint_reports_the_nested_statement_a_shared_start_owns() {
        // Specification 12 §6.13: a label owns exactly its statement's span, so the
        // label (10:1) and the labelled statement (10:6) share a code span and the
        // innermost entry owns a request on their shared line. The on-line rule picks
        // the greatest start column not after the request: 10:3 reaches the label,
        // 10:6 reaches the labelled statement.
        let labels = labels();
        let table = LineTable {
            files: vec![LineFile { path_idx: 0 }],
            statements: vec![
                statement(0, 4, 10, 1, 10, 15),
                statement(0, 4, 10, 6, 10, 15),
            ],
        };
        let resolver = LineResolver::new(&table, &labels);
        assert_eq!(
            resolver.resolve_breakpoint("main.st", 10, 3),
            Some(ResolvedStatement {
                pou_id: 1,
                code_offset: 0,
                line: 10,
                column: 1,
                end_line: 10,
                end_column: 15,
            })
        );
        assert_eq!(
            resolver.resolve_breakpoint("main.st", 10, 6),
            Some(ResolvedStatement {
                pou_id: 1,
                code_offset: 0,
                line: 10,
                column: 6,
                end_line: 10,
                end_column: 15,
            })
        );
    }

    #[test]
    fn location_of_takes_the_innermost_entry_that_contains_the_offset() {
        // Specification 12 §6.13: a code offset belongs to the innermost entry whose
        // code span contains it, with a half-open `[start, end)`. Offset 2 is inside
        // both the IF (1..5) and its body (2..3), so the body at line 14 owns it;
        // offset 1 is only in the IF; offset 4 is in the IF but not the body; offset 10
        // is past every statement.
        let labels = labels();
        let table = program_table();
        let resolver = LineResolver::new(&table, &labels);
        assert_eq!(resolver.location_of(1, 2), Some(("main.st", 14, 1)));
        assert_eq!(resolver.location_of(1, 1), Some(("main.st", 13, 1)));
        assert_eq!(resolver.location_of(1, 4), Some(("main.st", 13, 1)));
        assert_eq!(resolver.location_of(1, 10), None);
        assert_eq!(resolver.location_of(2, 2), None);
    }

    #[test]
    fn location_of_reports_the_nested_statement_a_shared_code_span_owns() {
        // Specification 12 §6.13: entries that share a code span keep the order the
        // compiler emits them, outermost first, and the innermost (last) owns the
        // offset. The label and its statement share `[0, 4)`, so the labelled
        // statement at 10:6 is the answer.
        let labels = labels();
        let table = LineTable {
            files: vec![LineFile { path_idx: 0 }],
            statements: vec![
                statement(0, 4, 10, 1, 10, 15),
                statement(0, 4, 10, 6, 10, 15),
            ],
        };
        let resolver = LineResolver::new(&table, &labels);
        assert_eq!(resolver.location_of(1, 0), Some(("main.st", 10, 6)));
        assert_eq!(resolver.location_of(1, 4), None);
    }

    #[test]
    fn statement_starts_yields_each_stop_offset_once_in_order() {
        // Specification 12 §6.13: a step stops at a statement's first instruction, and
        // two entries that share a start are one stop offset. The program's distinct
        // starts are 0, 1, 2, 5, 6 and 9, ascending.
        let labels = labels();
        let table = program_table();
        let resolver = LineResolver::new(&table, &labels);
        assert_eq!(
            resolver.statement_starts(1).collect::<Vec<_>>(),
            vec![0, 1, 2, 5, 6, 9]
        );
        assert_eq!(
            resolver.statement_starts(2).collect::<Vec<_>>(),
            Vec::<u32>::new()
        );
    }
}
