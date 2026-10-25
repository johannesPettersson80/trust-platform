//! What the read operations list that every runtime lists alike: the position of a frame, the one
//! `Main` frame between scans, and the rows of the I/O scope (specification 13, "Stack Trace and
//! Navigation" and "Variables / Evaluate"; specification 11 §6.13.1).

use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec;
use alloc::vec::Vec;

use super::paging::{ChildKind, Page};
use super::projection::{variables_from_io_rows, DebugSource, DebugVariable, IoRow, IoRowValue};
use super::replies;
use crate::live_values::io_snapshot::{IoSnapshot, IoSnapshotEntry, IoSnapshotValue};

/// A position in a source file, as a frame or a scope reply carries it: zero-based.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Located {
    /// The file.
    pub source: DebugSource,
    /// The zero-based line.
    pub line: u32,
    /// The zero-based column.
    pub column: u32,
}

/// The frames of a stack reply when no scan is held: the latest stop's position as the one
/// synthetic `Main` frame, and no frame without a position.
#[must_use]
pub fn main_frame(fallback: Option<&Located>) -> Vec<serde_json::Value> {
    fallback
        .map(|place| {
            vec![replies::stack_frame(
                0,
                "Main",
                &place.source,
                place.line,
                place.column,
            )]
        })
        .unwrap_or_default()
}

/// The entry of one area in the I/O root.
pub(super) fn io_area(name: &str, items: usize, variables_reference: u32) -> DebugVariable {
    DebugVariable {
        name: name.to_string(),
        value: format!("{items} items"),
        r#type: None,
        variables_reference,
        evaluate_name: None,
        indexed_variables: None,
        named_variables: None,
    }
}

/// The rows of one I/O area that `page` lists.
pub(super) fn io_rows(
    io: Option<&IoSnapshot>,
    area: impl FnOnce(&IoSnapshot) -> &[IoSnapshotEntry],
    page: Page,
) -> Result<Vec<DebugVariable>, String> {
    let io = io.ok_or_else(|| String::from("I/O snapshot unavailable"))?;
    Ok(variables_from_io_rows(
        page.select(ChildKind::Named, area(io).iter())
            .map(|entry| IoRow {
                name: entry.name.as_deref(),
                address: &entry.address,
                value: match &entry.value {
                    IoSnapshotValue::Value(value) => IoRowValue::Value(value),
                    IoSnapshotValue::Error(error) => IoRowValue::Error(error),
                    IoSnapshotValue::Unresolved => IoRowValue::Unresolved,
                },
            }),
    ))
}
