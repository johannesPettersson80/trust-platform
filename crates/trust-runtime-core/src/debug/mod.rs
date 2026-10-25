//! A statement's place in its source, which the debugger names in its stops and steps
//! (specification 44 §5.9, §13.1).

/// A statement's source span as a device keeps it: the index of its file in the image's statement
/// table and its half-open span of 1-based `(line, column)` positions (specification 44 §5.9). This
/// is the form that the statement identity of specification 13 takes, without the generation the
/// breakpoint set gives it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StatementSpan {
    /// Index into the table's file entries.
    pub file_index: u32,
    /// The first position of the statement: its first line and column.
    pub start: (u32, u32),
    /// The position just after the statement: its end line and column.
    pub end: (u32, u32),
}
