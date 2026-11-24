//! A trace names each variable as the debugger names it, whatever form the request named it in
//! (Appendix C, C.6). A recording keeps the names as the requests gave them, since a scan cannot
//! look a name up without allocating (specification 34 MEM-09); the trace's file names them, when it
//! is written after the recording.

use alloc::string::String;
use alloc::vec::Vec;

use super::read::{read_change_run, read_cycle};
use super::write::{encode_change, encode_cycle};
use super::{Change, ChangeKind, TraceError};

/// The record of a cycle (`CYCL`, C.6), with each variable's path and each method call's instance
/// named by `name`: the name the debugger gives the path, or `None` to keep the path as it is.
///
/// # Errors
///
/// The [`TraceError`] of a record that does not read.
pub fn named_cycle(
    record: &[u8],
    name: &mut dyn FnMut(&str) -> Option<String>,
) -> Result<Vec<u8>, TraceError> {
    let mut cycle = read_cycle(record)?;
    for change in &mut cycle.changes {
        rename(change, name);
    }
    let mut out = Vec::with_capacity(record.len());
    encode_cycle(&cycle, &mut out);
    Ok(out)
}

/// The `count` changes that `bytes` holds one after another, as a start state holds its forces
/// (`STAT`, C.5), named as [`named_cycle`] names a cycle's.
///
/// # Errors
///
/// The [`TraceError`] of changes that do not read.
pub fn named_changes(
    bytes: &[u8],
    count: usize,
    name: &mut dyn FnMut(&str) -> Option<String>,
) -> Result<Vec<u8>, TraceError> {
    let mut out = Vec::with_capacity(bytes.len());
    for mut change in read_change_run(bytes, count)? {
        rename(&mut change, name);
        encode_change(
            change.source,
            &change.kind.borrowed(&mut Vec::new()),
            &mut out,
        );
    }
    Ok(out)
}

/// Names the variable or the instance that `change` names, when `name` gives it a name.
fn rename(change: &mut Change, name: &mut dyn FnMut(&str) -> Option<String>) {
    let path = match &mut change.kind {
        ChangeKind::VariableWrite { path, .. }
        | ChangeKind::VariableForce { path, .. }
        | ChangeKind::VariableRelease { path } => path,
        ChangeKind::MethodCall { instance, .. } => instance,
        ChangeKind::AddressWrite { .. }
        | ChangeKind::AddressForce { .. }
        | ChangeKind::AddressRelease { .. } => return,
    };
    if let Some(named) = name(path) {
        *path = named;
    }
}
