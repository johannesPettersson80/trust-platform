use super::*;
use crate::bytecode::BytecodeError;

/// Wire identifier of the LINE_TABLE section (spec 12 §6.13).
///
/// LINE_TABLE belongs to the source-free container versions 2.0 and 3.0, where
/// §11.5 keeps the id `0x0011` free; a 1.x producer never writes it. A reader
/// that predates it keeps the payload as an unknown optional extension of §4.2.
pub const LINE_TABLE_SECTION_ID: u16 = 0x0011;

/// Statement code spans and source spans per file (spec 12 §6.13).
///
/// This is what source-level debugging needs when the ST text is absent, so every
/// runtime resolves breakpoints and steps from the artifact alone.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct LineTable {
    /// One entry per source file carrying a statement, ascending by label bytes.
    pub files: Vec<LineFile>,
    /// Statement spans, ordered by `pou_id`, then `start_code_offset`, then
    /// descending `end_code_offset`.
    pub statements: Vec<LineStatement>,
}

impl LineTable {
    /// Check the statement rules of §6.13 that the decoder and the validator share.
    ///
    /// A source-built container reaches validation without the decoder, so the
    /// decoder and the validator share this one rule. Every statement's code span and
    /// source span run forwards and are non-empty, and its positions are 1-based.
    /// Entries ascend by `pou_id`, then `start_code_offset`, then descending
    /// `end_code_offset`, and two entries that share a start must nest (the later
    /// span inside the earlier one). The code spans of one POU never cross: they
    /// nest, or one ends where the next starts.
    ///
    /// # Errors
    ///
    /// Returns the §6.13 section fault that names the first rule an entry breaks.
    pub fn check_statement_order(&self) -> Result<(), BytecodeError> {
        let mut previous: Option<&LineStatement> = None;
        for statement in &self.statements {
            check_statement_spans(statement)?;
            if let Some(previous) = previous {
                if (statement.pou_id, statement.start_code_offset)
                    < (previous.pou_id, previous.start_code_offset)
                {
                    return Err(BytecodeError::section_static(
                        "LINE_TABLE statements are out of order",
                    ));
                }
                if statement.pou_id == previous.pou_id
                    && statement.start_code_offset == previous.start_code_offset
                    && statement.end_code_offset > previous.end_code_offset
                {
                    return Err(BytecodeError::section_static(
                        "LINE_TABLE statements that share a start do not nest",
                    ));
                }
            }
            previous = Some(statement);
        }
        let code_spans = self.statements.iter().map(|statement| {
            (
                statement.pou_id,
                statement.start_code_offset,
                statement.end_code_offset,
            )
        });
        if spans_cross(code_spans) {
            return Err(BytecodeError::section_static(
                "LINE_TABLE statement code spans cross",
            ));
        }
        Ok(())
    }
}

/// The span rules of one statement (spec 12 §6.13): its code span and its source span
/// each run forwards and are non-empty, and every position is 1-based.
fn check_statement_spans(statement: &LineStatement) -> Result<(), BytecodeError> {
    if statement.start_code_offset >= statement.end_code_offset {
        return Err(BytecodeError::section_static(
            if statement.start_code_offset == statement.end_code_offset {
                "LINE_TABLE statement code span is empty"
            } else {
                "LINE_TABLE statement code span is reversed"
            },
        ));
    }
    if statement.start_line == 0
        || statement.end_line == 0
        || statement.start_column == 0
        || statement.end_column == 0
    {
        return Err(BytecodeError::section_static(
            "LINE_TABLE statement position is not 1-based",
        ));
    }
    let start = (statement.start_line, statement.start_column);
    let end = (statement.end_line, statement.end_column);
    if start >= end {
        return Err(BytecodeError::section_static(if start == end {
            "LINE_TABLE statement source span is empty"
        } else {
            "LINE_TABLE statement source span is reversed"
        }));
    }
    Ok(())
}

/// Whether two spans of one group cross: they overlap, and neither contains the other
/// (spec 12 §6.13). The spans ascend by start, and at an equal start descend by end. A
/// span that ends where the next one starts only touches it.
fn spans_cross<G: PartialEq + Copy, P: Ord + Copy>(spans: impl Iterator<Item = (G, P, P)>) -> bool {
    // The ends of the spans that still hold the current start, innermost last. A nested
    // span ends no later than the span around it, so the innermost end is the smallest.
    let mut open: Vec<P> = Vec::new();
    let mut group: Option<G> = None;
    for (this_group, start, end) in spans {
        if group != Some(this_group) {
            open.clear();
            group = Some(this_group);
        }
        while open.last().is_some_and(|&innermost| innermost <= start) {
            open.pop();
        }
        if open.last().is_some_and(|&innermost| end > innermost) {
            return true;
        }
        open.push(end);
    }
    false
}

/// One source file's label (spec 12 §6.13).
///
/// A file entry carries the label only: the statement entries carry every position
/// a runtime resolves, so nothing is stored per source line.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct LineFile {
    /// File label index in DEBUG_STRING_TABLE.
    pub path_idx: u32,
}

/// One executable statement's code span and source span (spec 12 §6.13).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct LineStatement {
    /// POU_INDEX identity of the owning POU.
    pub pou_id: u32,
    /// Offset within POU_BODIES of the statement's first instruction.
    pub start_code_offset: u32,
    /// Offset within POU_BODIES, one past the statement's last instruction.
    pub end_code_offset: u32,
    /// Index into [`LineTable::files`](self::LineTable::files).
    pub file_index: u32,
    /// 1-based line of the statement's first byte.
    pub start_line: u32,
    /// 1-based byte column within that line.
    pub start_column: u32,
    /// 1-based line of the byte one past the statement's last byte.
    pub end_line: u32,
    /// 1-based byte column within that line.
    pub end_column: u32,
}
