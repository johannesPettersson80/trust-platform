//! Breakpoint and stop-location parity between trust-runtime's source-based
//! resolver and the portable `LINE_TABLE` resolver.
//!
//! Specification 13, "One resolver on every runtime" and "Breakpoint evaluation,
//! source resolution, and hook contract" (owner decision Q4, 18 October 2026), and
//! specification 12 §6.13 and §10: moving to the line table changes no answer, so
//! for the bounded-C corpus and the emission test's programs this test builds the
//! module with its line table and today's source-based statement list from the same
//! compile and compares the two resolvers over every requested `(line, column)` and
//! every instruction offset. A difference is collected and reported per program,
//! line and column; it is a defect in the line table, not a new behaviour
//! (specification 13, "Source Mapping").

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use trust_runtime::bytecode::{
    BytecodeModule, DebugEntry, LineTable, PouIndex, SectionData, SectionId, StringTable,
};
use trust_runtime::debug::{location_to_line_col, resolve_breakpoint_location, SourceLocation};
use trust_runtime::harness::CompileSession;
use trust_runtime_core::debug::LineResolver;

/// One source file with an IF, a FOR and a function call (mirrors
/// `line_table_emission.rs`'s `SOURCE`, so the parity covers the emission program).
const SOURCE: &str = concat!(
    "FUNCTION Add : INT\n",
    "VAR_INPUT\n",
    "x : INT;\n",
    "END_VAR\n",
    "Add := x + INT#1;\n",
    "END_FUNCTION\n",
    "PROGRAM Main\n",
    "VAR\n",
    "f : BOOL;\n",
    "i : INT;\n",
    "END_VAR\n",
    "f := TRUE;\n",
    "IF f THEN\n",
    "i := 1;\n",
    "END_IF;\n",
    "FOR i := 0 TO 1 DO\n",
    "f := FALSE;\n",
    "END_FOR;\n",
    "i := Add(x := INT#1);\n",
    "END_PROGRAM\n",
);

/// One source file with a REPEAT and a labelled statement, where nested statements
/// share a start (mirrors `line_table_emission.rs`'s `NESTED_SOURCE`).
const NESTED_SOURCE: &str = concat!(
    "PROGRAM Main\n",
    "VAR\n",
    "i : INT;\n",
    "f : BOOL;\n",
    "END_VAR\n",
    "REPEAT\n",
    "i := i + INT#1;\n",
    "UNTIL i > INT#3\n",
    "END_REPEAT;\n",
    "Top: f := TRUE;\n",
    "END_PROGRAM\n",
);

/// One program's comparison totals and every difference found.
struct Report {
    name: String,
    columns: usize,
    offsets: usize,
    differences: usize,
    shown: Vec<String>,
}

/// A difference keeps every program's total, but the message shows only the first
/// few cases of that program so a corpus-wide failure stays readable.
const DIFFERENCES_SHOWN: usize = 2;

impl Report {
    fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            columns: 0,
            offsets: 0,
            differences: 0,
            shown: Vec::new(),
        }
    }

    fn difference(&mut self, message: String) {
        self.differences += 1;
        if self.shown.len() < DIFFERENCES_SHOWN {
            self.shown.push(format!("{}: {message}", self.name));
        }
    }

    fn differ(&self) -> bool {
        self.differences > 0
    }
}

/// The emission test's two programs and every `bounded-c/*/src` program.
fn programs() -> Vec<(String, String)> {
    let mut programs = vec![
        ("emission SOURCE".to_string(), SOURCE.to_string()),
        (
            "emission NESTED_SOURCE".to_string(),
            NESTED_SOURCE.to_string(),
        ),
    ];
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/bounded-c");
    let mut fixtures = Vec::new();
    for entry in std::fs::read_dir(&root).expect("bounded-c corpus directory") {
        let source_dir = entry.expect("corpus entry").path().join("src");
        if !source_dir.is_dir() {
            continue;
        }
        for file in std::fs::read_dir(&source_dir).expect("corpus source directory") {
            let path = file.expect("corpus source entry").path();
            if path.extension().is_some_and(|extension| extension == "st") {
                fixtures.push(path);
            }
        }
    }
    fixtures.sort();
    for path in fixtures {
        let name = path.to_string_lossy().into_owned();
        let text = std::fs::read_to_string(&path).expect("corpus program");
        programs.push((name, text));
    }
    programs
}

/// Every 1-based line's first byte offset, plus one entry after the last line
/// (specification 13: line starts contain offset zero and add the byte after every
/// LF).
fn line_starts(text: &str) -> Vec<u32> {
    let mut starts = vec![0u32];
    for (index, byte) in text.bytes().enumerate() {
        if byte == b'\n' {
            starts.push(u32::try_from(index + 1).expect("source fits u32"));
        }
    }
    starts
}

/// The byte offset of a 1-based `(line, column)`, clamped to the end of the file.
fn byte_of(starts: &[u32], text_len: u32, line: u32, column: u32) -> u32 {
    let index = line.saturating_sub(1) as usize;
    let start = starts.get(index).copied().unwrap_or(text_len);
    start.saturating_add(column.saturating_sub(1)).min(text_len)
}

/// The byte length of a 1-based line, excluding its LF (specification 13).
fn line_length(starts: &[u32], text_len: u32, line: u32) -> u32 {
    let index = line.saturating_sub(1) as usize;
    let start = starts.get(index).copied().unwrap_or(text_len);
    let end = if index + 1 < starts.len() {
        starts[index + 1].saturating_sub(1)
    } else {
        text_len
    };
    end.saturating_sub(start)
}

/// One program's shared data for the two comparisons.
struct Context<'a> {
    text: &'a str,
    file_id: u32,
    file_label: &'a str,
    /// The source-based statement list of the file: the statement index that the source text
    /// provides (specification 13, "Breakpoint evaluation, source resolution, and hook contract").
    statements: &'a [SourceLocation],
    table: &'a LineTable,
    strings: &'a StringTable,
    starts: &'a [u32],
    text_len: u32,
    identity: &'a BTreeMap<u32, (u32, u32)>,
    by_offset: &'a BTreeMap<(u32, u32), &'a DebugEntry>,
    entries: &'a [DebugEntry],
    resolver: LineResolver<'a>,
}

impl Context<'_> {
    /// Today's source-based answer for a 0-based `(line, column)`: the statement list resolved
    /// through the source text, exactly as the statement index answered before a LINE_TABLE did
    /// (specification 13, "Breakpoint evaluation, source resolution, and hook contract").
    fn source_location(&self, line: u32, column: u32) -> Option<SourceLocation> {
        resolve_breakpoint_location(self.text, self.file_id, self.statements, line, column)
    }

    /// Today's stop location for a DEBUG_MAP statement entry: its source position
    /// through the source-based resolver, then its start byte projected back to a
    /// 1-based `(line, column)`, labelled by DEBUG_STRING_TABLE.
    fn today(&self, entry: &DebugEntry) -> Option<(String, u32, u32)> {
        let location =
            self.source_location(entry.line.saturating_sub(1), entry.column.saturating_sub(1))?;
        let (line, column) = location_to_line_col(self.text, &location);
        Some((
            self.strings.entries[entry.file_idx as usize]
                .as_str()
                .to_string(),
            line + 1,
            column + 1,
        ))
    }

    /// R1 against today's resolver, over every line and the requested columns
    /// (specification 13, "Breakpoint evaluation, source resolution, and hook
    /// contract").
    fn compare_breakpoints(&self, report: &mut Report) {
        let last_line = u32::try_from(self.starts.len()).expect("line count fits u32");
        for line in 1..=last_line + 1 {
            let mut columns: BTreeSet<u32> = BTreeSet::new();
            columns.insert(1);
            for statement in &self.table.statements {
                if statement.start_line == line {
                    columns.insert(statement.start_column);
                }
            }
            columns.insert(line_length(self.starts, self.text_len, line) + 5);
            for column in columns {
                report.columns += 1;
                self.compare_breakpoint(line, column, report);
            }
        }
    }

    /// One R1 case: today's resolver and the line table must name one statement.
    fn compare_breakpoint(&self, line: u32, column: u32, report: &mut Report) {
        let today = self.source_location(line.saturating_sub(1), column.saturating_sub(1));
        let resolved = self
            .resolver
            .resolve_breakpoint(self.file_label, line, column);
        match (resolved, today) {
            (None, None) => {}
            (Some(resolved), Some(location)) => {
                let (line0, column0) = location_to_line_col(self.text, &location);
                if (resolved.line, resolved.column) != (line0 + 1, column0 + 1) {
                    report.difference(format!(
                        "{line}:{column} position: line table {}:{}, today {}:{}",
                        resolved.line,
                        resolved.column,
                        line0 + 1,
                        column0 + 1
                    ));
                    return;
                }
                match self.identity.get(&location.start) {
                    Some(expected) if (resolved.pou_id, resolved.code_offset) == *expected => {}
                    Some(expected) => report.difference(format!(
                        "{line}:{column} statement: line table POU {} offset {}, today POU {} offset {}",
                        resolved.pou_id, resolved.code_offset, expected.0, expected.1
                    )),
                    None => report.difference(format!(
                        "{line}:{column} today resolves source byte {} which is not a statement start",
                        location.start
                    )),
                }
            }
            (Some(resolved), None) => report.difference(format!(
                "{line}:{column}: line table names POU {} offset {}, today resolves nothing",
                resolved.pou_id, resolved.code_offset
            )),
            (None, Some(_)) => report.difference(format!(
                "{line}:{column}: today resolves a statement, line table none"
            )),
        }
    }

    /// R2 against today's stop location, over every statement start and every
    /// instruction offset (specification 12 §6.13, §10).
    fn compare_offsets(&self, pou_index: &PouIndex, report: &mut Report) {
        for pou in &pou_index.entries {
            let end = pou
                .code_offset
                .checked_add(pou.code_length)
                .unwrap_or_else(|| panic!("{}: POU {} body range", report.name, pou.id));
            for offset in pou.code_offset..end {
                report.offsets += 1;
                self.compare_offset(pou.id, offset, report);
            }
        }
    }

    /// One R2 case: today's stop location at the offset and the line table's answer.
    fn compare_offset(&self, pou_id: u32, offset: u32, report: &mut Report) {
        let resolved = self.resolver.location_of(pou_id, offset);
        let today = self
            .by_offset
            .get(&(pou_id, offset))
            .copied()
            .and_then(|entry| self.today(entry));
        match (resolved, today) {
            (None, None) => {}
            (Some(resolved), Some(today)) => {
                if (resolved.0, resolved.1, resolved.2) != (today.0.as_str(), today.1, today.2) {
                    report.difference(format!(
                        "POU {pou_id} offset {offset} stop location: line table {} {}:{}, today {} {}:{}",
                        resolved.0, resolved.1, resolved.2, today.0, today.1, today.2
                    ));
                }
            }
            (Some(resolved), None) => {
                // The offset is inside a statement but is not a statement start, so
                // today stops nowhere there. The named statement must still be one
                // today records, with the same recorded position.
                let entry = self
                    .entries
                    .iter()
                    .filter(|entry| entry.kind == 0)
                    .find(|entry| {
                        entry.pou_id == pou_id
                            && entry.line == resolved.1
                            && entry.column == resolved.2
                    });
                match entry.and_then(|entry| self.today(entry)) {
                    Some(recorded)
                        if (resolved.0, resolved.1, resolved.2)
                            == (recorded.0.as_str(), recorded.1, recorded.2) => {}
                    Some(recorded) => report.difference(format!(
                        "POU {pou_id} offset {offset}: line table names {} {}:{}, today records {} {}:{}",
                        resolved.0, resolved.1, resolved.2, recorded.0, recorded.1, recorded.2
                    )),
                    None => report.difference(format!(
                        "POU {pou_id} offset {offset}: line table names {} {}:{} with no today statement",
                        resolved.0, resolved.1, resolved.2
                    )),
                }
            }
            (None, Some(today)) => report.difference(format!(
                "POU {pou_id} offset {offset}: today stops at {} {}:{}, line table none",
                today.0, today.1, today.2
            )),
        }
    }
}

/// Build one program's context and compare both resolvers.
fn check_program(name: &str, text: &str) -> Report {
    let session = CompileSession::from_source(text);
    let module: BytecodeModule = session
        .build_bytecode_module()
        .unwrap_or_else(|error| panic!("{name}: line table module: {error}"));
    let runtime = session
        .build_runtime()
        .unwrap_or_else(|error| panic!("{name}: source-based runtime: {error}"));
    let table = module
        .line_table()
        .unwrap_or_else(|| panic!("{name}: LINE_TABLE"));
    let Some(SectionData::DebugStringTable(strings)) = module.section(SectionId::DebugStringTable)
    else {
        panic!("{name}: DEBUG_STRING_TABLE")
    };
    let Some(SectionData::DebugMap(debug_map)) = module.section(SectionId::DebugMap) else {
        panic!("{name}: DEBUG_MAP")
    };
    let Some(SectionData::PouIndex(pou_index)) = module.section(SectionId::PouIndex) else {
        panic!("{name}: POU_INDEX")
    };
    assert_eq!(table.files.len(), 1, "{name}: expected one source file");
    let file_label = strings.entries[table.files[0].path_idx as usize].as_str();
    let file_id: u32 = file_label
        .strip_prefix("file_")
        .and_then(|suffix| suffix.parse().ok())
        .unwrap_or_else(|| panic!("{name}: virtual file label {file_label}"));
    // Today's statement list is the source-based one. The runtime's own statement listing answers
    // from the line table once one is bound, so the reference is read from the metadata's
    // statement index, which the source text provided.
    let metadata = runtime.metadata_snapshot();
    let statements = metadata
        .source_statement_locations(file_id)
        .unwrap_or_else(|| panic!("{name}: today's statement list for file {file_id}"));

    let starts = line_starts(text);
    let text_len = u32::try_from(text.len()).expect("source fits u32");

    // Every statement start byte identifies one statement (specification 12 §6.13:
    // the source span starts at the statement's first non-trivia token).
    let mut identity: BTreeMap<u32, (u32, u32)> = BTreeMap::new();
    for statement in &table.statements {
        let byte = byte_of(
            &starts,
            text_len,
            statement.start_line,
            statement.start_column,
        );
        if let Some(previous) =
            identity.insert(byte, (statement.pou_id, statement.start_code_offset))
        {
            panic!(
                "{name}: two statements start at source byte {byte}: {previous:?} and ({}, {})",
                statement.pou_id, statement.start_code_offset
            );
        }
    }

    // Today's DEBUG_MAP keyed by code offset; the last duplicate (the innermost
    // entry, emitted last) owns the offset, exactly as the runtime's map does.
    let mut by_offset: BTreeMap<(u32, u32), &DebugEntry> = BTreeMap::new();
    for entry in debug_map.entries.iter().filter(|entry| entry.kind == 0) {
        by_offset.insert((entry.pou_id, entry.code_offset), entry);
    }

    let mut report = Report::new(name);
    let context = Context {
        text,
        file_id,
        file_label,
        statements,
        table,
        strings,
        starts: &starts,
        text_len,
        identity: &identity,
        by_offset: &by_offset,
        entries: &debug_map.entries,
        resolver: LineResolver::new(table, &strings.entries),
    };
    context.compare_breakpoints(&mut report);
    context.compare_offsets(pou_index, &mut report);
    report
}

#[test]
fn line_table_resolution_matches_the_source_based_resolver() {
    let mut columns = 0usize;
    let mut offsets = 0usize;
    let mut programs_checked = 0usize;
    let mut differences = 0usize;
    let mut differing = 0usize;
    let mut shown: Vec<String> = Vec::new();
    for (name, text) in programs() {
        let report = check_program(&name, &text);
        columns += report.columns;
        offsets += report.offsets;
        programs_checked += 1;
        if report.differ() {
            differing += 1;
            differences += report.differences;
            shown.extend(report.shown);
        }
    }
    println!(
        "line-table parity: {programs_checked} programs, {columns} (line, column) cases, {offsets} code offsets"
    );
    assert_eq!(
        differences,
        0,
        "{differing} of {programs_checked} programs differ in {differences} of {columns} (line, column) cases ({offsets} code offsets agree): {}",
        shown.join(" | ")
    );
}
