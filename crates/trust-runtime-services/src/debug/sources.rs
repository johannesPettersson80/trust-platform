//! The module's `LINE_TABLE` and the labels of its files, as the debug requests of a runtime
//! without source text read them (specification 12 §6.13; specification 13, "Source Mapping").
//!
//! A chip holds no ST text, so a breakpoint is placed, a stop is named and a file is listed from
//! the table alone: the table carries each statement's file, start and end line and column, and
//! `DEBUG_STRING_TABLE` carries the file labels. The resolution is the core's
//! [`LineResolver`], the one implementation that
//! trust-runtime resolves through as well, so both runtimes give the same answer for the same
//! artifact.

use alloc::vec::Vec;

use smol_str::SmolStr;
use trust_runtime_core::bytecode::{BytecodeModule, LineTable, SectionData};
use trust_runtime_core::debug::{LineResolver, ResolvedStatement};
use trust_runtime_core::error::RuntimeError;
use trust_runtime_core::vm::PreparationLimits;

/// A module's `LINE_TABLE` with the labels of its files.
#[derive(Debug, Clone)]
pub struct LineSources {
    table: LineTable,
    /// `DEBUG_STRING_TABLE`: `labels[path_idx]` names a file entry.
    labels: Vec<SmolStr>,
}

impl LineSources {
    /// The sources of `table`, whose file entries index `labels` (the module's
    /// `DEBUG_STRING_TABLE`) with their `path_idx`.
    #[must_use]
    pub fn new(table: LineTable, labels: Vec<SmolStr>) -> Self {
        Self { table, labels }
    }

    /// Reads the `LINE_TABLE` and the file labels of the container `program`, or `None` when the
    /// container has no `LINE_TABLE` (specification 12 §6.13: the section is optional).
    ///
    /// The container is decoded within the preparation allowance of `limits`, which the decoder
    /// charges before it reserves, so a program too large to read is refused instead of failing
    /// on the heap. A device reads its sources before it prepares the program: preparation drops
    /// the table, and the heap is as free then as it is for preparation (specification 34 §5.2).
    ///
    /// # Errors
    ///
    /// The [`RuntimeError`] of a container that does not decode within `limits`, and
    /// [`RuntimeError::InvalidExecutionState`] for a table whose files have no labels.
    pub fn read(program: &[u8], limits: &PreparationLimits) -> Result<Option<Self>, RuntimeError> {
        let (module, _) = BytecodeModule::decode_with_limits(
            program,
            limits.max_preparation_bytes,
            limits.max_preparation_work,
        )
        .map_err(RuntimeError::from)?;
        let mut table = None;
        let mut labels = None;
        for section in module.sections {
            match section.data {
                SectionData::LineTable(found) => table = Some(found),
                SectionData::DebugStringTable(found) => labels = Some(found.entries),
                _ => {}
            }
        }
        let Some(table) = table else {
            return Ok(None);
        };
        let labels = match labels {
            Some(labels) => labels,
            None if table.files.is_empty() => Vec::new(),
            None => return Err(RuntimeError::InvalidExecutionState),
        };
        Ok(Some(Self { table, labels }))
    }

    /// The label of file `file_index`: its source path relative to the project root with `/`
    /// separators, or the fallback label `file_<file_id>` (specification 12 §6.13 and §7.5.1.4).
    #[must_use]
    pub fn label(&self, file_index: u32) -> Option<&str> {
        let file = self.table.files.get(usize::try_from(file_index).ok()?)?;
        self.labels
            .get(usize::try_from(file.path_idx).ok()?)
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
        for (index, file) in (0_u32..).zip(&self.table.files) {
            let Some(label) = usize::try_from(file.path_idx)
                .ok()
                .and_then(|path_idx| self.labels.get(path_idx))
                .map(SmolStr::as_str)
            else {
                continue;
            };
            let names_the_file = !label.is_empty()
                && (path == label
                    || path
                        .strip_suffix(label)
                        .is_some_and(|head| head.ends_with('/')));
            if names_the_file && best.is_none_or(|(_, longest)| label.len() > longest) {
                best = Some((index, label.len()));
            }
        }
        best.map(|(index, _)| index)
    }

    /// The statement a 1-based `(line, column)` request on file `file_index` resolves to
    /// (specification 13, "Breakpoint evaluation, source resolution, and hook contract").
    #[must_use]
    pub fn resolve_breakpoint(
        &self,
        file_index: u32,
        line: u32,
        column: u32,
    ) -> Option<ResolvedStatement> {
        let label = self.label(file_index)?;
        LineResolver::new(&self.table, &self.labels).resolve_breakpoint(label, line, column)
    }

    /// The 1-based `(line, column)` at which each statement of file `file_index` starts, in
    /// table order: the valid breakpoint positions of the file (specification 13, "Breakpoint
    /// Locations"). A statement nested in another is listed too.
    pub fn statement_starts(&self, file_index: u32) -> impl Iterator<Item = (u32, u32)> + '_ {
        self.table
            .statements
            .iter()
            .filter(move |statement| statement.file_index == file_index)
            .map(|statement| (statement.start_line, statement.start_column))
    }
}

#[cfg(test)]
mod tests {
    use alloc::vec;

    use trust_runtime_core::bytecode::{LineFile, LineStatement};

    use super::*;

    /// Three files labelled `a.st`, `src/a.st` and `lib/b.st`; `a.st` has two statements, `3:1-4:1`
    /// and `5:5-6:1`, `src/a.st` one, `1:1-2:1`, and `lib/b.st` none.
    fn sources() -> LineSources {
        let statement =
            |file_index, start_line, start_column, end_line, end_column| LineStatement {
                pou_id: 1,
                start_code_offset: 0,
                end_code_offset: 1,
                file_index,
                start_line,
                start_column,
                end_line,
                end_column,
            };
        LineSources::new(
            LineTable {
                files: vec![
                    LineFile { path_idx: 0 },
                    LineFile { path_idx: 1 },
                    LineFile { path_idx: 2 },
                ],
                statements: vec![
                    statement(0, 3, 1, 4, 1),
                    statement(0, 5, 5, 6, 1),
                    statement(1, 1, 1, 2, 1),
                ],
            },
            vec!["a.st".into(), "src/a.st".into(), "lib/b.st".into()],
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
    fn a_label_names_its_file_entry_and_no_other() {
        // Specification 12 §6.13: a file entry's `path_idx` names its label in DEBUG_STRING_TABLE.
        // Wrong behaviour caught: a label looked up by the entry's position instead of its `path_idx`
        // (the same here, so the entries are checked by value), and a label for an entry beyond the
        // table.
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
}
