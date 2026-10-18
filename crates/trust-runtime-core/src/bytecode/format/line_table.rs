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
    /// Check the statement ordering and nesting rule of §6.13.
    ///
    /// A source-built container reaches validation without the decoder, so the
    /// decoder and the validator share this one rule. Entries ascend by `pou_id`,
    /// then `start_code_offset`, then descending `end_code_offset`, and two entries
    /// that share a start must nest (the later span inside the earlier one).
    ///
    /// # Errors
    ///
    /// Returns the §6.13 section fault when an entry breaks the order or a shared
    /// start does not nest.
    pub fn check_statement_order(&self) -> Result<(), BytecodeError> {
        let mut previous: Option<&LineStatement> = None;
        for statement in &self.statements {
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
        Ok(())
    }
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
