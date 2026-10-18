//! DAP-compatible debug helpers for control clients.
//!
//! The projection of values onto scopes and variables is shared with the chips
//! (`trust_runtime_services::debug::projection`), so a variable reads the same on every runtime
//! (specification 34, ARCH-11). This module re-exports it and keeps the I/O scope's projection of
//! the hosted snapshot.

#![allow(missing_docs)]

pub use trust_runtime_services::debug::projection::{
    format_value, instance_display_metadata, value_type_name, variable_from_value,
    variable_from_value_with_metadata, variables_from_array, variables_from_entries,
    variables_from_instances, variables_from_struct, DebugScope, DebugSource, DebugVariable,
    DebugVariableHandles, VariableHandle,
};
use trust_runtime_services::debug::projection::{variables_from_io_rows, IoRow, IoRowValue};

#[cfg(test)]
use crate::io::IoAddress;
use crate::io::{IoSnapshot, IoSnapshotEntry, IoSnapshotValue};
#[cfg(test)]
use crate::memory::InstanceId;

pub fn variables_from_io_entries(entries: &[IoSnapshotEntry]) -> Vec<DebugVariable> {
    variables_from_io_rows(entries.iter().map(|entry| IoRow {
        name: entry.name.as_deref(),
        address: &entry.address,
        value: match &entry.value {
            IoSnapshotValue::Value(value) => IoRowValue::Value(value),
            IoSnapshotValue::Error(error) => IoRowValue::Error(error),
            IoSnapshotValue::Unresolved => IoRowValue::Unresolved,
        },
    }))
}

pub fn io_scope_available(snapshot: Option<&IoSnapshot>) -> bool {
    snapshot
        .map(|state| {
            !(state.inputs.is_empty() && state.outputs.is_empty() && state.memory.is_empty())
        })
        .unwrap_or(false)
}

#[cfg(test)]
#[path = "dap/contract_tests.rs"]
mod contract_tests;
