//! What the read operations list: the frames of the stack, the scopes of a frame and the variables
//! of a scope (specification 13, "Stack Trace and Navigation", "Variables / Evaluate" and
//! "Variable Visibility"; specification 11 §6.13.1).
//!
//! A read works on a [`ReadState`]: the storage as the published debug state holds it and the call
//! stack with its variables named. The runtime that serves the request builds the state from what
//! it owns, without copying the storage, and these functions only list it.

use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec;
use alloc::vec::Vec;

use smol_str::SmolStr;
use trust_runtime_core::collections::OrderedMap;
use trust_runtime_core::memory::{FrameId, InstanceId, VariableStorage};
use trust_runtime_core::value::Value;

use super::projection::{
    array_elements, instance_display_metadata, struct_fields, variable_for,
    variables_from_instances, variables_from_io_rows, DebugScope, DebugSource, DebugVariable,
    DebugVariableHandles, IoRow, IoRowValue, VariableHandle,
};
use super::replies;
use crate::live_values::io_snapshot::{IoSnapshot, IoSnapshotEntry, IoSnapshotValue};

/// One frame of the call stack as a read sees it.
#[derive(Debug, Clone)]
pub struct NamedFrame<'a> {
    /// The frame's identity.
    pub id: FrameId,
    /// The name of the POU the frame runs.
    pub owner: &'a str,
    /// The instance that receives the invocation, when it has one.
    pub instance: Option<InstanceId>,
    /// The frame's locals in declaration order. The compiler's scratch slots are not listed
    /// (specification 13, "Variables / Evaluate").
    pub variables: Vec<(&'a str, &'a Value)>,
}

/// The state a read answers from: the storage and the call stack, outermost frame first.
#[derive(Debug)]
pub struct ReadState<'a> {
    /// The storage of the published debug state.
    pub storage: &'a VariableStorage,
    /// The call stack. It is empty when no scan is held, because the published state of a scan
    /// that ended has no call stack (specification 11 §6.13.1).
    pub frames: Vec<NamedFrame<'a>>,
}

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

/// The frames of a stack reply, innermost first. `place_of` gives the position a frame was last
/// seen at, and `fallback` the position of the latest stop. With no frame, the stop's position is
/// the one synthetic `Main` frame, and without a position the stack is empty.
#[must_use]
pub fn stack_frames(
    frames: &[NamedFrame<'_>],
    place_of: impl Fn(FrameId) -> Option<Located>,
    fallback: Option<&Located>,
) -> Vec<serde_json::Value> {
    let frame_json = |id: u32, name: &str, place: &Located| {
        replies::stack_frame(id, name, &place.source, place.line, place.column)
    };
    if frames.is_empty() {
        return fallback
            .map(|place| vec![frame_json(0, "Main", place)])
            .unwrap_or_default();
    }
    frames
        .iter()
        .rev()
        .filter_map(|frame| {
            let place = place_of(frame.id).or_else(|| fallback.cloned())?;
            Some(frame_json(frame.id.0, frame.owner, &place))
        })
        .collect()
}

/// The scopes of frame `frame`: its locals, then the globals, the retained variables, the I/O and
/// the instances, each only when it has something to show. Handles restart at `1`, so the
/// handles of an earlier stop are invalid from here (specification 13, "Runtime debug state,
/// mutation, and projection invariants").
#[must_use]
pub fn scopes(
    handles: &mut DebugVariableHandles,
    storage: &VariableStorage,
    frame: FrameId,
    place: Option<&Located>,
    has_io: bool,
) -> Vec<DebugScope> {
    handles.clear();
    let mut scopes = vec![DebugScope {
        name: "Locals".to_string(),
        variables_reference: handles.alloc(VariableHandle::Locals(frame)),
        expensive: false,
        source: place.map(|place| place.source.clone()),
        line: place.map(|place| place.line),
        column: place.map(|place| place.column),
        end_line: None,
        end_column: None,
    }];
    let mut push = |name: &str, handle: VariableHandle| {
        scopes.push(DebugScope {
            name: name.to_string(),
            variables_reference: handles.alloc(handle),
            expensive: false,
            source: None,
            line: None,
            column: None,
            end_line: None,
            end_column: None,
        });
    };
    if !storage.globals().is_empty() {
        push("Globals", VariableHandle::Globals);
    }
    if !storage.retain().is_empty() {
        push("Retain", VariableHandle::Retain);
    }
    if has_io {
        push("I/O", VariableHandle::IoRoot);
    }
    if !storage.instances().is_empty() {
        push("Instances", VariableHandle::Instances);
    }
    scopes
}

/// The variables a handle lists.
///
/// # Errors
///
/// The error text of a handle whose frame or instance is no longer there, of a reference whose
/// target is gone, and of an I/O handle while no I/O snapshot is published.
pub fn variables(
    state: &ReadState<'_>,
    handle: &VariableHandle,
    io: Option<&IoSnapshot>,
    handles: &mut DebugVariableHandles,
) -> Result<Vec<DebugVariable>, String> {
    let storage = state.storage;
    Ok(match handle {
        VariableHandle::Locals(frame_id) => locals(state, *frame_id, handles)?,
        VariableHandle::Globals => named_values(handles, storage.globals(), storage),
        VariableHandle::Retain => named_values(handles, storage.retain(), storage),
        VariableHandle::Instances => {
            let mut instances = Vec::with_capacity(storage.instances().len());
            for (id, data) in storage.instances() {
                instances.push((*id, data.type_name.to_string()));
            }
            variables_from_instances(handles, instances)
        }
        VariableHandle::Instance(instance_id) => instance(storage, *instance_id, handles)?,
        VariableHandle::Struct(value) => struct_fields(handles, value),
        VariableHandle::Array(value) => array_elements(handles, value),
        VariableHandle::Reference(value_ref) => {
            let Some(value) = storage.read_by_ref_ref(value_ref) else {
                return Err(String::from("reference target unavailable"));
            };
            let (display, type_name) = instance_display_metadata(value, storage);
            vec![variable_for(handles, "*", value, None, display, type_name)]
        }
        VariableHandle::IoRoot => {
            let io = io.ok_or_else(|| String::from("I/O snapshot unavailable"))?;
            let inputs_ref = handles.alloc(VariableHandle::IoInputs);
            let outputs_ref = handles.alloc(VariableHandle::IoOutputs);
            let memory_ref = handles.alloc(VariableHandle::IoMemory);
            vec![
                io_area("Inputs", io.inputs.len(), inputs_ref),
                io_area("Outputs", io.outputs.len(), outputs_ref),
                io_area("Memory", io.memory.len(), memory_ref),
            ]
        }
        VariableHandle::IoInputs => io_rows(io, |io| io.inputs.as_slice())?,
        VariableHandle::IoOutputs => io_rows(io, |io| io.outputs.as_slice())?,
        VariableHandle::IoMemory => io_rows(io, |io| io.memory.as_slice())?,
    })
}

/// The variables of a table of named values, in the table's order.
fn named_values(
    handles: &mut DebugVariableHandles,
    values: &OrderedMap<SmolStr, Value>,
    storage: &VariableStorage,
) -> Vec<DebugVariable> {
    let mut variables = Vec::with_capacity(values.len());
    for (name, value) in values {
        variables.push(entry(handles, name, value, storage));
    }
    variables
}

/// The locals of a frame: the variables of the instance that receives the invocation, then the
/// frame's own.
fn locals(
    state: &ReadState<'_>,
    frame_id: FrameId,
    handles: &mut DebugVariableHandles,
) -> Result<Vec<DebugVariable>, String> {
    let storage = state.storage;
    let Some(frame) = state.frames.iter().find(|frame| frame.id == frame_id) else {
        return Err(replies::unknown_frame(frame_id.0));
    };
    let mut variables = Vec::new();
    if let Some(instance_id) = frame.instance {
        let Some(instance) = storage.get_instance(instance_id) else {
            return Err(format!("unknown instance id {}", instance_id.0));
        };
        for (name, value) in &instance.variables {
            variables.push(entry(handles, name, value, storage));
        }
    }
    for (name, value) in &frame.variables {
        variables.push(entry(handles, name, value, storage));
    }
    Ok(variables)
}

/// The variables of one instance, then its parent as an instance entry.
fn instance(
    storage: &VariableStorage,
    instance_id: InstanceId,
    handles: &mut DebugVariableHandles,
) -> Result<Vec<DebugVariable>, String> {
    let Some(instance) = storage.get_instance(instance_id) else {
        return Err(format!("unknown instance id {}", instance_id.0));
    };
    let mut variables = Vec::new();
    for (name, value) in &instance.variables {
        variables.push(entry(handles, name, value, storage));
    }
    if let Some(parent) = instance.parent {
        variables.push(entry(handles, "parent", &Value::Instance(parent), storage));
    }
    Ok(variables)
}

/// One variable of a scope: the value with the display text and type name of an instance.
fn entry(
    handles: &mut DebugVariableHandles,
    name: &str,
    value: &Value,
    storage: &VariableStorage,
) -> DebugVariable {
    let (display, type_name) = instance_display_metadata(value, storage);
    variable_for(handles, name, value, Some(name), display, type_name)
}

/// The entry of one area in the I/O root.
fn io_area(name: &str, items: usize, variables_reference: u32) -> DebugVariable {
    DebugVariable {
        name: name.to_string(),
        value: format!("{items} items"),
        r#type: None,
        variables_reference,
        evaluate_name: None,
    }
}

/// The rows of one I/O area.
fn io_rows(
    io: Option<&IoSnapshot>,
    area: impl FnOnce(&IoSnapshot) -> &[IoSnapshotEntry],
) -> Result<Vec<DebugVariable>, String> {
    let io = io.ok_or_else(|| String::from("I/O snapshot unavailable"))?;
    Ok(variables_from_io_rows(area(io).iter().map(|entry| IoRow {
        name: entry.name.as_deref(),
        address: &entry.address,
        value: match &entry.value {
            IoSnapshotValue::Value(value) => IoRowValue::Value(value),
            IoSnapshotValue::Error(error) => IoRowValue::Error(error),
            IoSnapshotValue::Unresolved => IoRowValue::Unresolved,
        },
    })))
}
