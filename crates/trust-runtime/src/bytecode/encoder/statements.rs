//! Statement spans collected while emitting POU bodies.
//!
//! Spec 12 §6.10 records where a statement starts; §6.13 additionally needs the
//! end of its code span and of its source span. The source span end is the
//! statement syntax node's own range end, known when the statement begins; the
//! code span end is only known once the whole statement has been emitted. The
//! encoder therefore records one [`PendingStatement`](self::PendingStatement) per
//! DEBUG_MAP statement entry and fills the code end in after the statement's last
//! instruction.

use std::cmp::Reverse;

use smol_str::SmolStr;

use crate::bytecode::{DebugEntry, LineFile, LineStatement, LineTable};

use super::util::to_u32;
use super::{BytecodeEncoder, BytecodeError};

/// One emitted statement, in the POU body's own offset frame.
#[derive(Clone, Debug)]
pub(super) struct PendingStatement {
    /// The DEBUG_MAP record of the statement start (spec 12 §6.10).
    pub(super) entry: DebugEntry,
    /// Offset one past the statement's last instruction (spec 12 §6.13).
    pub(super) end_code_offset: u32,
    /// 1-based line of the statement syntax node's range end, trailing trivia
    /// included (spec 12 §6.13).
    pub(super) end_line: u32,
    /// 1-based byte column within that line.
    pub(super) end_column: u32,
}

/// The statements one POU body emitted, in emission order.
#[derive(Default)]
pub(super) struct EmittedStatements {
    pub(super) statements: Vec<PendingStatement>,
}

impl EmittedStatements {
    pub(super) fn len(&self) -> usize {
        self.statements.len()
    }

    pub(super) fn is_empty(&self) -> bool {
        self.statements.is_empty()
    }

    pub(super) fn push(&mut self, statement: PendingStatement) {
        self.statements.push(statement);
    }

    /// Drop the statements from `len` on; a statement that failed to lower is not
    /// part of the module.
    pub(super) fn truncate(&mut self, len: usize) {
        self.statements.truncate(len);
    }

    fn entry_mut(&mut self, index: usize) -> Option<&mut PendingStatement> {
        self.statements.get_mut(index)
    }
}

/// Rebase every statement's code offsets once its POU body's position is known.
pub(super) fn rebase_statement_offsets(
    statements: &mut [PendingStatement],
    code_offset: u32,
) -> Result<(), BytecodeError> {
    for statement in statements {
        statement.entry.code_offset = statement
            .entry
            .code_offset
            .checked_add(code_offset)
            .ok_or_else(|| BytecodeError::InvalidSection("debug code offset overflow".into()))?;
        statement.end_code_offset = statement
            .end_code_offset
            .checked_add(code_offset)
            .ok_or_else(|| BytecodeError::InvalidSection("debug code offset overflow".into()))?;
    }
    Ok(())
}

impl<'a> BytecodeEncoder<'a> {
    /// Record the statement's DEBUG_MAP position and source span (spec 12 §6.10,
    /// §6.13), returning its index for the code span end.
    ///
    /// A statement without a source location records nothing.
    pub(super) fn begin_statement(
        &mut self,
        pou_id: u32,
        code_offset: u32,
        stmt: &crate::program_model::Stmt,
        statements: &mut EmittedStatements,
    ) -> Result<Option<usize>, BytecodeError> {
        let (Some(location), Some(sources)) = (stmt.location(), self.sources) else {
            return Ok(None);
        };
        let source = sources
            .get(location.file_id as usize)
            .ok_or_else(|| BytecodeError::InvalidSection("debug source missing".into()))?;
        let (line, column) = crate::debug::location_to_line_col(source, location);
        // §6.13: the recorded end is the statement syntax node's range end with its
        // trailing trivia included, the next token's start or the end of the file,
        // which is the span breakpoint resolution compares against (spec 13,
        // "Source Mapping"). The lowering already holds that end in `location.end`.
        let (end_line, end_column) = crate::debug::offset_to_line_col(source, location.end);
        let file_idx = self.file_path_index(location.file_id)?;
        let index = statements.len();
        statements.push(PendingStatement {
            entry: DebugEntry {
                pou_id,
                code_offset,
                file_idx,
                line: line.saturating_add(1),
                column: column.saturating_add(1),
                kind: 0,
            },
            end_code_offset: code_offset,
            end_line: end_line.saturating_add(1),
            end_column: end_column.saturating_add(1),
        });
        Ok(Some(index))
    }

    /// Fill the statement's code span end once its last instruction is emitted.
    pub(super) fn end_statement(
        statements: &mut EmittedStatements,
        index: Option<usize>,
        code_len: usize,
    ) -> Result<(), BytecodeError> {
        let Some(index) = index else {
            return Ok(());
        };
        let end_code_offset = to_u32(code_len, "debug code span end")?;
        let statement = statements.entry_mut(index).ok_or_else(|| {
            BytecodeError::InvalidSection("debug statement record missing".into())
        })?;
        statement.end_code_offset = end_code_offset;
        Ok(())
    }

    /// Project every emitted statement onto the LINE_TABLE section (spec 12 §6.13).
    ///
    /// Files ascend by the byte order of their label and every statement carries the
    /// index of its file. Statements ascend by `pou_id`, then by `start_code_offset`,
    /// then by descending `end_code_offset`, so a statement that shares a start with
    /// its first body statement comes first and the code offset resolves to the
    /// innermost (last) entry that contains it. Two source files sharing one label
    /// share one entry, because a label appears in one entry only, exactly as the two
    /// share one DEBUG_MAP file identity.
    pub(super) fn build_line_table(
        &self,
        statements: &EmittedStatements,
    ) -> Result<LineTable, BytecodeError> {
        let mut files: Vec<(SmolStr, LineFile)> = Vec::new();
        for statement in &statements.statements {
            let label = self
                .debug_strings
                .entries
                .get(statement.entry.file_idx as usize)
                .ok_or_else(|| BytecodeError::InvalidSection("debug path index missing".into()))?;
            if files.iter().any(|(existing, _)| existing == label) {
                continue;
            }
            files.push((
                label.clone(),
                LineFile {
                    path_idx: statement.entry.file_idx,
                },
            ));
        }
        files.sort_by(|left, right| left.0.as_bytes().cmp(right.0.as_bytes()));

        let mut line_statements = Vec::with_capacity(statements.statements.len());
        for statement in &statements.statements {
            let label = self
                .debug_strings
                .entries
                .get(statement.entry.file_idx as usize)
                .ok_or_else(|| BytecodeError::InvalidSection("debug path index missing".into()))?;
            let file_index = files
                .iter()
                .position(|(existing, _)| existing == label)
                .ok_or_else(|| BytecodeError::InvalidSection("LINE_TABLE file missing".into()))?;
            line_statements.push(LineStatement {
                pou_id: statement.entry.pou_id,
                start_code_offset: statement.entry.code_offset,
                end_code_offset: statement.end_code_offset,
                file_index: u32::try_from(file_index)
                    .map_err(|_| BytecodeError::InvalidSection("LINE_TABLE file index".into()))?,
                start_line: statement.entry.line,
                start_column: statement.entry.column,
                end_line: statement.end_line,
                end_column: statement.end_column,
            });
        }
        line_statements.sort_by_key(|statement| {
            (
                statement.pou_id,
                statement.start_code_offset,
                Reverse(statement.end_code_offset),
            )
        });
        Ok(LineTable {
            files: files.into_iter().map(|(_, file)| file).collect(),
            statements: line_statements,
        })
    }
}
