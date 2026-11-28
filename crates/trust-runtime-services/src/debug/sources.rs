//! The statement table of a program image and the labels of its files, as the debug requests of a
//! runtime without source text read them (specification 44 §5.9, §13.1; specification 13, "Source
//! Mapping").
//!
//! A chip holds no ST text, so a breakpoint is placed, a stop is named and a file is listed from
//! the table alone: the table carries each statement's file, start and end line and column, and the
//! file records carry the labels. Every runtime resolves through this one table, so each gives the
//! same answer for the same image.

use alloc::vec::Vec;

use smol_str::SmolStr;
use trust_runtime_core::native::image::StmtRecord;

/// One statement of the table: its POU, its id and its 1-based source span.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LineStatement {
    /// The POU that owns the statement.
    pub pou_id: u32,
    /// The statement's id, its index in the image's statement table.
    pub id: u32,
    /// The index of its file.
    pub file_index: u32,
    /// 1-based line of the statement's first byte.
    pub start_line: u32,
    /// 1-based byte column within that line.
    pub start_column: u32,
    /// 1-based line of the statement's source span end, trailing trivia included.
    pub end_line: u32,
    /// 1-based byte column of that end within its line.
    pub end_column: u32,
}

/// One resolved statement: its POU, its id, and its 1-based source start and end.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResolvedStatement {
    /// The POU that owns the statement.
    pub pou_id: u32,
    /// The statement's id.
    pub code_offset: u32,
    /// 1-based line of the statement's first byte.
    pub line: u32,
    /// 1-based byte column within that line.
    pub column: u32,
    /// 1-based line of the statement's source span end, trailing trivia included.
    pub end_line: u32,
    /// 1-based byte column of that end within its line.
    pub end_column: u32,
}

/// Where a statement lies, as a client that holds the sources shows it: its file's label and the
/// lines of its own text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatementAt {
    /// The label of the statement's file: its source path relative to the project root with `/`
    /// separators, or the fallback label `file_<file_id>` (specification 44 §5.9).
    pub file: SmolStr,
    /// 1-based line of the statement's first byte.
    pub line: u32,
    /// 1-based line where the statement's own text ends: its last line, or, for a statement that
    /// holds others, such as an `IF`, the line before the first statement it holds, and never
    /// before `line` (specification 22, "Test reporting and CI output").
    pub last_line: u32,
}

/// The last line of `statement`'s own text, `next` being the statement after it in the table
/// (specification 44 §5.9: ids in source order, so a statement that holds others is followed by
/// the first one it holds): `statement`'s last line, or, when `next` begins inside it, the line
/// before `next` begins, and never before `statement`'s first line.
fn own_last_line(statement: &StmtRecord, next: Option<StmtRecord>) -> u32 {
    let start = (statement.start_line, statement.start_column);
    // The end column is the column after the statement's last character.
    let end = (statement.end_line, statement.end_column);
    let held = next.filter(|next| {
        let begins = (next.start_line, next.start_column);
        next.file == statement.file && start <= begins && begins < end
    });
    match held {
        Some(next) => next.start_line.saturating_sub(1),
        None => statement.end_line,
    }
    .max(statement.start_line)
}

/// Where statement `id` of `image` lies, from the image's statement table (specification 44 §5.9);
/// `None` for an id the table does not hold. It reads the one statement, so a fault's statement is
/// found without building the whole table.
#[must_use]
pub fn statement_at(
    image: &trust_runtime_core::native::image::Image<'_>,
    id: u32,
) -> Option<StatementAt> {
    use trust_runtime_core::native::image::FileRecord;
    let statements = image.table::<StmtRecord>().ok()?;
    let statement = statements.get(id)?;
    let file = image
        .table::<FileRecord>()
        .ok()?
        .get(u32::from(statement.file))?;
    let next = id.checked_add(1).and_then(|next| statements.get(next));
    Some(StatementAt {
        file: SmolStr::new(image.string(file.label)?),
        line: statement.start_line,
        last_line: own_last_line(&statement, next),
    })
}

/// Where the fault that ended `program`'s last call was raised (specification 44 §8.6): the
/// statement that raised it. `None` when no fault ended the call, as for a deadline the runtime
/// measured, and for a release image, whose statements record no number (§13.2).
#[must_use]
pub fn fault_at<C: trust_runtime_core::native::program::ModuleCode>(
    program: &trust_runtime_core::native::program::NativeProgram<'_, C>,
) -> Option<StatementAt> {
    let fault = program.last_fault();
    let image = program.image();
    if fault.code == 0 || image.header().is_release() {
        return None;
    }
    statement_at(&image, fault.stmt)
}

/// A program's statements with the labels of their files.
#[derive(Debug, Clone, Default)]
pub struct LineSources {
    /// `labels[file_index]` names a file.
    labels: Vec<SmolStr>,
    /// The statements in source order: a statement comes before the statements nested in it.
    statements: Vec<LineStatement>,
}

impl LineSources {
    /// The sources whose files `labels` names, by file index, and whose statements are
    /// `statements`, in source order.
    #[must_use]
    pub fn new(labels: Vec<SmolStr>, statements: Vec<LineStatement>) -> Self {
        Self { labels, statements }
    }

    /// The sources of a compiled program's image (specification 44 §5.9, §13.1): its statement
    /// table, each statement's id standing for its code offset, and the labels of its files. The
    /// ids are in source order, so a statement comes before the statements nested in it.
    #[must_use]
    pub fn from_image(image: &trust_runtime_core::native::image::Image<'_>) -> Self {
        use trust_runtime_core::native::image::FileRecord;
        // Each list holds exactly its entries: the memory figure counts them (specification 44 §10.4).
        let mut labels = Vec::new();
        if let Ok(records) = image.table::<FileRecord>() {
            labels.reserve_exact(usize::try_from(records.len()).unwrap_or(0));
            for file in records.iter() {
                labels.push(SmolStr::new(image.string(file.label).unwrap_or("")));
            }
        }
        let mut statements = Vec::new();
        if let Ok(records) = image.table::<StmtRecord>() {
            statements.reserve_exact(usize::try_from(records.len()).unwrap_or(0));
            for (id, statement) in (0u32..).zip(records.iter()) {
                statements.push(LineStatement {
                    pou_id: u32::from(statement.pou),
                    id,
                    file_index: u32::from(statement.file),
                    start_line: statement.start_line,
                    start_column: u32::from(statement.start_column),
                    end_line: statement.end_line,
                    end_column: u32::from(statement.end_column),
                });
            }
        }
        Self { labels, statements }
    }

    /// The label of file `file_index`: its source path relative to the project root with `/`
    /// separators, or the fallback label `file_<file_id>` (specification 44 §5.9).
    #[must_use]
    pub fn label(&self, file_index: u32) -> Option<&str> {
        self.labels
            .get(usize::try_from(file_index).ok()?)
            .map(SmolStr::as_str)
    }

    /// The file a requested path names: the file whose label is the path itself, or else the
    /// label that is the longest tail of the path after a `/`. A client names a file by the path
    /// it holds, and a label is relative to the project root, so the path of a file below the
    /// project ends with its label (specification 13, "`SetBreakpoints`"). A `\` separator counts
    /// as `/`.
    #[must_use]
    pub fn file_index_of(&self, path: &str) -> Option<u32> {
        let path = path.replace('\\', "/");
        let mut best: Option<(u32, usize)> = None;
        for (index, label) in (0_u32..).zip(&self.labels) {
            let names_the_file = !label.is_empty()
                && (path == label.as_str()
                    || path
                        .strip_suffix(label.as_str())
                        .is_some_and(|head| head.ends_with('/')));
            if names_the_file && best.is_none_or(|(_, longest)| label.len() > longest) {
                best = Some((index, label.len()));
            }
        }
        best.map(|(index, _)| index)
    }

    /// The statement a 1-based `(line, column)` request on file `file_index` resolves to
    /// (specification 13, "Breakpoint evaluation, source resolution, and hook contract").
    ///
    /// On `line` it selects the statement with the greatest start column not after `column`, else
    /// the smallest start column after it. When no statement starts on `line` it selects the
    /// innermost statement whose source span contains the position. Otherwise it selects the
    /// earliest statement at or after the position. A file with no candidate returns `None`.
    #[must_use]
    pub fn resolve_breakpoint(
        &self,
        file_index: u32,
        line: u32,
        column: u32,
    ) -> Option<ResolvedStatement> {
        self.label(file_index)?;
        let mut before: Option<&LineStatement> = None;
        let mut after: Option<&LineStatement> = None;
        for statement in self
            .statements
            .iter()
            .filter(|statement| statement.file_index == file_index && statement.start_line == line)
        {
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
        // A nested pair is ordered outermost first, so the last containing entry is the innermost
        // one that owns the position.
        if let Some(statement) = self.statements.iter().rfind(|statement| {
            statement.file_index == file_index && contains(statement, line, column)
        }) {
            return Some(resolved(statement));
        }
        self.statements
            .iter()
            .filter(|statement| {
                statement.file_index == file_index
                    && (statement.start_line, statement.start_column) >= (line, column)
            })
            .min_by_key(|statement| (statement.start_line, statement.start_column))
            .map(resolved)
    }

    /// The 1-based `(line, column)` at which each statement of file `file_index` starts, in
    /// table order: the valid breakpoint positions of the file (specification 13, "Breakpoint
    /// Locations"). A statement nested in another is listed too.
    pub fn statement_starts(&self, file_index: u32) -> impl Iterator<Item = (u32, u32)> + '_ {
        self.statements
            .iter()
            .filter(move |statement| statement.file_index == file_index)
            .map(|statement| (statement.start_line, statement.start_column))
    }
}

/// A statement as the resolver answers it.
fn resolved(statement: &LineStatement) -> ResolvedStatement {
    ResolvedStatement {
        pou_id: statement.pou_id,
        code_offset: statement.id,
        line: statement.start_line,
        column: statement.start_column,
        end_line: statement.end_line,
        end_column: statement.end_column,
    }
}

/// Whether a 1-based position lies in a statement's source span. The end is inclusive
/// (specification 13, "Breakpoint evaluation, source resolution, and hook contract").
fn contains(statement: &LineStatement, line: u32, column: u32) -> bool {
    (statement.start_line, statement.start_column) <= (line, column)
        && (line, column) <= (statement.end_line, statement.end_column)
}

#[cfg(test)]
mod tests {
    use alloc::vec;

    use super::*;

    /// Three files labelled `a.st`, `src/a.st` and `lib/b.st`; `a.st` has two statements, `3:1-4:1`
    /// and `5:5-6:1`, `src/a.st` one, `1:1-2:1`, and `lib/b.st` none.
    fn sources() -> LineSources {
        let statement =
            |file_index, start_line, start_column, end_line, end_column| LineStatement {
                pou_id: 1,
                id: 0,
                file_index,
                start_line,
                start_column,
                end_line,
                end_column,
            };
        LineSources::new(
            vec!["a.st".into(), "src/a.st".into(), "lib/b.st".into()],
            vec![
                statement(0, 3, 1, 4, 1),
                statement(0, 5, 5, 6, 1),
                statement(1, 1, 1, 2, 1),
            ],
        )
    }

    #[test]
    fn a_file_is_named_by_its_label_or_by_a_path_that_ends_with_it() {
        // Specification 11 §6.13.3: the file is the one whose label is the path, or else the longest
        // label that is a tail of the path after a `/`; a `\` counts as `/`. `/p/src/a.st` ends with
        // both `a.st` and `src/a.st`, and the longer one names the file; `/p/other/a.st` ends with
        // `a.st` only; `/p/xsrc/a.st` ends with `src/a.st` but not after a `/`, so it is `a.st`.
        // Wrong behaviour caught: the shortest tail winning, a tail that begins inside a name, a path
        // with `\` that matches nothing, and an empty path that matches.
        let sources = sources();

        assert_eq!(sources.file_index_of("a.st"), Some(0));
        assert_eq!(sources.file_index_of("src/a.st"), Some(1));
        assert_eq!(sources.file_index_of("/p/src/a.st"), Some(1));
        assert_eq!(sources.file_index_of("/p/other/a.st"), Some(0));
        assert_eq!(sources.file_index_of("/p/xsrc/a.st"), Some(0));
        assert_eq!(sources.file_index_of("C:\\proj\\lib\\b.st"), Some(2));
        assert_eq!(sources.file_index_of("/p/c.st"), None);
        assert_eq!(sources.file_index_of("lib/b.st.bak"), None);
        assert_eq!(sources.file_index_of(""), None);
    }

    #[test]
    fn a_label_names_its_file_and_no_other() {
        // Specification 44 §5.9: a file record's label names it. Wrong behaviour caught: a label
        // for a file beyond the table.
        let sources = sources();

        assert_eq!(sources.label(0), Some("a.st"));
        assert_eq!(sources.label(1), Some("src/a.st"));
        assert_eq!(sources.label(2), Some("lib/b.st"));
        assert_eq!(sources.label(3), None);
    }

    #[test]
    fn statement_starts_list_only_the_statements_of_the_file_in_table_order() {
        // Specification 13, "Breakpoint Locations": the starts of the statements of the requested
        // file. `a.st` starts at 3:1 and 5:5, `src/a.st` at 1:1, `lib/b.st` nowhere. Wrong behaviour
        // caught: starts of another file, and a different order.
        let sources = sources();

        assert_eq!(
            sources.statement_starts(0).collect::<Vec<_>>(),
            [(3, 1), (5, 5)]
        );
        assert_eq!(sources.statement_starts(1).collect::<Vec<_>>(), [(1, 1)]);
        assert_eq!(
            sources.statement_starts(2).collect::<Vec<_>>(),
            Vec::<(u32, u32)>::new()
        );
    }

    #[test]
    fn a_breakpoint_resolves_among_the_statements_of_its_own_file() {
        // Specification 13, "Breakpoint evaluation, source resolution, and hook contract": the
        // resolver considers the statements of the requested file only. Line 1 of `src/a.st` is the
        // statement 1:1-2:1; asking `src/a.st` for line 3 finds nothing (the statements at 3:1 and 5:5
        // belong to `a.st`). Wrong behaviour caught: a resolver that ignores the file.
        let sources = sources();

        let found = sources
            .resolve_breakpoint(1, 1, 1)
            .expect("the statement at 1:1");
        assert_eq!(
            (found.line, found.column, found.end_line, found.end_column),
            (1, 1, 2, 1)
        );
        assert_eq!(sources.resolve_breakpoint(1, 3, 1), None);
        assert_eq!(sources.resolve_breakpoint(9, 1, 1), None, "no such file");
    }

    #[test]
    fn a_position_between_statements_resolves_to_the_innermost_or_the_next() {
        // Specification 13, "Breakpoint evaluation, source resolution, and hook contract": with no
        // statement starting on the line, the innermost statement whose span holds the position is
        // chosen, else the first one after it. Line 4 of `a.st` lies inside 3:1-4:1 (the end is
        // inclusive); line 4 column 9 lies after it, so the next statement, 5:5, is chosen. Wrong
        // behaviour caught: an exclusive end, and no statement found after a gap.
        let sources = sources();

        let inside = sources.resolve_breakpoint(0, 4, 1).expect("inside 3:1-4:1");
        assert_eq!((inside.line, inside.column), (3, 1));
        let next = sources
            .resolve_breakpoint(0, 4, 9)
            .expect("the next statement");
        assert_eq!((next.line, next.column), (5, 5));
    }

    #[test]
    fn a_statement_s_own_text_ends_before_the_first_statement_it_holds() {
        // Specification 22, "Test reporting and CI output": a report shows the statement's own
        // text. An assertion over lines 6 to 9 that holds nothing ends on line 9, whether the table
        // ends after it or goes on with a statement after it; an `IF` from 17:1 to 21:8 whose first
        // held statement begins at 20:5 ends on line 19; an `IF` from 3:1 to 5:8 whose held
        // statement begins on line 3 ends on line 3; the `IF` of line 17 followed by a statement
        // of another file at 18:1 holds nothing and ends on line 21; a statement from 1:1 to 3:10
        // followed by one at 3:10, the column after its last character, holds nothing and ends on
        // line 3. Wrong behaviour caught: an `IF` whose whole body is reported, a statement of
        // another file taken as held, and the end column read as the last character's.
        let record = |file, start_line, start_column, end_line, end_column| StmtRecord {
            file,
            pou: 0,
            start_line,
            start_column,
            end_line,
            end_column,
        };
        let assertion = record(0, 6, 1, 9, 3);
        assert_eq!(own_last_line(&assertion, None), 9);
        assert_eq!(own_last_line(&assertion, Some(record(0, 10, 1, 10, 20))), 9);
        let branch = record(0, 17, 1, 21, 8);
        assert_eq!(own_last_line(&branch, Some(record(0, 20, 5, 20, 19))), 19);
        assert_eq!(own_last_line(&branch, Some(record(1, 18, 1, 18, 9))), 21);
        assert_eq!(
            own_last_line(&record(0, 3, 1, 5, 8), Some(record(0, 3, 11, 3, 18))),
            3
        );
        assert_eq!(
            own_last_line(&record(0, 1, 1, 3, 10), Some(record(0, 3, 10, 3, 20))),
            3
        );
    }
}
