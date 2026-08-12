//! DAP-compatible debug helpers for control clients.

#![allow(missing_docs)]

use std::collections::HashMap;

use serde::Serialize;
use smol_str::SmolStr;

use crate::io::{IoAddress, IoSize, IoSnapshot, IoSnapshotEntry, IoSnapshotValue};
use crate::memory::{FrameId, InstanceId, IoArea, VariableStorage};
use crate::value::{ArrayValue, StructValue, Value, ValueRef};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DebugSource {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DebugScope {
    pub name: String,
    pub variables_reference: u32,
    pub expensive: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<DebugSource>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub column: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_line: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_column: Option<u32>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DebugVariable {
    pub name: String,
    pub value: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub r#type: Option<String>,
    pub variables_reference: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub evaluate_name: Option<String>,
}

#[derive(Debug, Clone)]
pub enum VariableHandle {
    Locals(FrameId),
    Globals,
    Retain,
    Instances,
    Instance(InstanceId),
    Struct(StructValue),
    Array(ArrayValue),
    Reference(ValueRef),
    IoRoot,
    IoInputs,
    IoOutputs,
    IoMemory,
}

#[derive(Debug, Default)]
pub struct DebugVariableHandles {
    next_id: u32,
    handles: HashMap<u32, VariableHandle>,
}

impl DebugVariableHandles {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn clear(&mut self) {
        self.handles.clear();
        self.next_id = 1;
    }

    pub fn alloc(&mut self, handle: VariableHandle) -> u32 {
        assert!(
            self.next_id != 0 || self.handles.is_empty(),
            "debug variable handle space exhausted"
        );
        let id = self.next_id.max(1);
        self.next_id = id.checked_add(1).unwrap_or(0);
        self.handles.insert(id, handle);
        id
    }

    pub fn get(&self, id: u32) -> Option<&VariableHandle> {
        self.handles.get(&id)
    }
}

pub fn value_type_name(value: &Value) -> Option<String> {
    let name = match value {
        Value::Bool(_) => "BOOL",
        Value::SInt(_) => "SINT",
        Value::Int(_) => "INT",
        Value::DInt(_) => "DINT",
        Value::LInt(_) => "LINT",
        Value::USInt(_) => "USINT",
        Value::UInt(_) => "UINT",
        Value::UDInt(_) => "UDINT",
        Value::ULInt(_) => "ULINT",
        Value::Real(_) => "REAL",
        Value::LReal(_) => "LREAL",
        Value::Byte(_) => "BYTE",
        Value::Word(_) => "WORD",
        Value::DWord(_) => "DWORD",
        Value::LWord(_) => "LWORD",
        Value::Time(_) => "TIME",
        Value::LTime(_) => "LTIME",
        Value::Date(_) => "DATE",
        Value::LDate(_) => "LDATE",
        Value::Tod(_) => "TOD",
        Value::LTod(_) => "LTOD",
        Value::Dt(_) => "DT",
        Value::Ldt(_) => "LDT",
        Value::String(_) => "STRING",
        Value::WString(_) => "WSTRING",
        Value::Char(_) => "CHAR",
        Value::WChar(_) => "WCHAR",
        Value::Array(_) => "ARRAY",
        Value::Struct(value) => return Some(value.type_name().to_string()),
        Value::Enum(value) => return Some(value.type_name().to_string()),
        Value::Reference(_) => "REF",
        Value::Instance(_) => "INSTANCE",
        Value::Null => "NULL",
    };
    Some(name.to_string())
}

pub fn format_value(value: &Value) -> String {
    crate::value::format_user_value(value)
}

pub fn variables_from_entries(
    handles: &mut DebugVariableHandles,
    entries: Vec<(String, Value)>,
) -> Vec<DebugVariable> {
    entries
        .into_iter()
        .map(|(name, value)| variable_from_value(handles, name.clone(), value, Some(name)))
        .collect()
}

pub fn variables_from_instances(
    handles: &mut DebugVariableHandles,
    instances: Vec<(InstanceId, String)>,
) -> Vec<DebugVariable> {
    instances
        .into_iter()
        .map(|(id, type_name)| DebugVariable {
            name: format!("{type_name}#{}", id.0),
            value: type_name.clone(),
            r#type: Some(type_name),
            variables_reference: handles.alloc(VariableHandle::Instance(id)),
            evaluate_name: None,
        })
        .collect()
}

pub fn variables_from_struct(
    handles: &mut DebugVariableHandles,
    value: StructValue,
) -> Vec<DebugVariable> {
    value
        .fields()
        .iter()
        .map(|(name, value)| {
            variable_from_value(
                handles,
                name.to_string(),
                value.clone(),
                Some(name.to_string()),
            )
        })
        .collect()
}

pub fn variables_from_array(
    handles: &mut DebugVariableHandles,
    value: ArrayValue,
) -> Vec<DebugVariable> {
    value
        .elements()
        .iter()
        .cloned()
        .enumerate()
        .map(|(idx, value)| {
            let name = format!("[{idx}]");
            variable_from_value(handles, name.clone(), value, Some(name))
        })
        .collect()
}

pub fn variables_from_io_entries(entries: &[IoSnapshotEntry]) -> Vec<DebugVariable> {
    entries
        .iter()
        .map(|entry| {
            let name = entry
                .name
                .as_ref()
                .map(SmolStr::to_string)
                .unwrap_or_else(|| format_address(&entry.address));
            DebugVariable {
                name: name.clone(),
                value: format_snapshot_value(&entry.value),
                r#type: None,
                variables_reference: 0,
                evaluate_name: Some(name),
            }
        })
        .collect()
}

pub fn variable_from_value(
    handles: &mut DebugVariableHandles,
    name: String,
    value: Value,
    evaluate_name: Option<String>,
) -> DebugVariable {
    variable_from_value_with_metadata(handles, name, value, evaluate_name, None, None)
}

pub fn variable_from_value_with_metadata(
    handles: &mut DebugVariableHandles,
    name: String,
    value: Value,
    evaluate_name: Option<String>,
    display: Option<String>,
    type_name: Option<String>,
) -> DebugVariable {
    let mut variables_reference = 0;
    match &value {
        Value::Struct(value) => {
            variables_reference = handles.alloc(VariableHandle::Struct((**value).clone()));
        }
        Value::Array(value) => {
            variables_reference = handles.alloc(VariableHandle::Array((**value).clone()));
        }
        Value::Instance(id) => {
            variables_reference = handles.alloc(VariableHandle::Instance(*id));
        }
        Value::Reference(Some(value_ref)) => {
            variables_reference = handles.alloc(VariableHandle::Reference(value_ref.clone()));
        }
        _ => {}
    }

    DebugVariable {
        name,
        value: display.unwrap_or_else(|| format_value(&value)),
        r#type: type_name.or_else(|| value_type_name(&value)),
        variables_reference,
        evaluate_name,
    }
}

pub fn instance_display_metadata(
    value: &Value,
    storage: &VariableStorage,
) -> (Option<String>, Option<String>) {
    let Value::Instance(id) = value else {
        return (None, None);
    };
    let Some(instance) = storage.get_instance(*id) else {
        return (None, None);
    };
    let type_name = instance.type_name.to_string();
    (Some(type_name.clone()), Some(type_name))
}

pub fn io_scope_available(snapshot: Option<&IoSnapshot>) -> bool {
    snapshot
        .map(|state| {
            !(state.inputs.is_empty() && state.outputs.is_empty() && state.memory.is_empty())
        })
        .unwrap_or(false)
}

fn format_snapshot_value(value: &IoSnapshotValue) -> String {
    match value {
        IoSnapshotValue::Value(value) => format!("{value:?}"),
        IoSnapshotValue::Error(err) => format!("error: {err}"),
        IoSnapshotValue::Unresolved => "unresolved".to_string(),
    }
}

fn format_address(address: &IoAddress) -> String {
    let area = match address.area {
        IoArea::Input => "I",
        IoArea::Output => "Q",
        IoArea::Memory => "M",
    };
    let size = match address.size {
        IoSize::Bit => "X",
        IoSize::Byte => "B",
        IoSize::Word => "W",
        IoSize::DWord => "D",
        IoSize::LWord => "L",
        IoSize::Bytes(_) => "B",
    };
    if address.wildcard {
        return format!("%{area}{size}*");
    }
    if address.size == IoSize::Bit {
        format!("%{area}{size}{}.{}", address.byte, address.bit)
    } else {
        format!("%{area}{size}{}", address.byte)
    }
}

#[cfg(test)]
#[path = "dap/contract_tests.rs"]
mod contract_tests;
