//! The debugger's scope and variable projection (specification 13, "Variables / Evaluate" and
//! "Runtime debug state, mutation, and projection invariants"): the DAP shapes of a source, a
//! scope and a variable, the handles that make a compound value expandable, and the projection of
//! values onto them.
//!
//! trust-runtime and a chip project the same values through these functions, so a variable reads
//! the same on both (specification 34, ARCH-11).

use alloc::collections::{BTreeMap, VecDeque};
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::ops::Range;

use serde::Serialize;
use trust_runtime_core::io_address::{IoAddress, IoSize};
use trust_runtime_core::memory::{FrameId, InstanceId, IoArea, VariableStorage};
use trust_runtime_core::value::{format_user_value, ArrayValue, StructValue, Value, ValueRef};

use super::paging::{ChildKind, Page};

/// The source a scope or a stack frame names (DAP `Source`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DebugSource {
    /// The file's name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// The file's path, or its label on a runtime without source text.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
}

/// One scope of a stack frame (DAP `Scope`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DebugScope {
    /// The scope's name, for example `Locals`.
    pub name: String,
    /// The handle that lists the scope's variables.
    pub variables_reference: u32,
    /// Whether listing the scope is expensive.
    pub expensive: bool,
    /// Where the scope's frame stands, when it has a position.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<DebugSource>,
    /// The frame's line.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line: Option<u32>,
    /// The frame's column.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub column: Option<u32>,
    /// The end line of the scope's extent.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_line: Option<u32>,
    /// The end column of the scope's extent.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_column: Option<u32>,
}

/// One variable of a scope (DAP `Variable`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DebugVariable {
    /// The variable's name.
    pub name: String,
    /// The variable's value in the stable debug representation.
    pub value: String,
    /// The variable's public type name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub r#type: Option<String>,
    /// The handle that lists the variable's members, or `0` for a value that has none.
    pub variables_reference: u32,
    /// The name an evaluate request reads the variable by.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub evaluate_name: Option<String>,
    /// The number of indexed children, the elements of an array, when the value is an array. It is
    /// the size of the whole array, not of a page (specification 11 §6.13.3, "Variable pages").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub indexed_variables: Option<u32>,
    /// The number of named children, the members of a structure, when the value is a structure.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub named_variables: Option<u32>,
}

/// What a variable handle lists.
#[derive(Debug, Clone)]
pub enum VariableHandle {
    /// The locals of one frame.
    Locals(FrameId),
    /// The resource's globals.
    Globals,
    /// The retained variables.
    Retain,
    /// The live function-block and class instances.
    Instances,
    /// The variables of one instance.
    Instance(InstanceId),
    /// The fields of a structure value.
    Struct(StructValue),
    /// The elements of an array value.
    Array(ArrayValue),
    /// The value a reference points to.
    Reference(ValueRef),
    /// The I/O scope's three areas.
    IoRoot,
    /// The input area.
    IoInputs,
    /// The output area.
    IoOutputs,
    /// The memory area.
    IoMemory,
    /// The locals of frame `frame` of a compiled program's held call stack, innermost first: the
    /// variables of the instance it runs on, then its frame variables (specification 44 §13.2).
    CompiledLocals(usize),
    /// A compiled program's globals.
    CompiledGlobals,
    /// A compiled program's retained variables, under their retain-map keys.
    CompiledRetain,
    /// A compiled program's instances: its program instances and the instances its symbol table
    /// names, each by its symbol's index.
    CompiledInstances,
    /// The members of the compiled program's instance of type `type_index` at `place`.
    CompiledInstance {
        /// Where the instance lies.
        place: trust_runtime_core::native::memory::Place,
        /// The instance's type.
        type_index: u32,
    },
    /// The elements of the compiled program's array of instances of type `type_index` at `place`.
    CompiledInstanceArray {
        /// Where the array lies.
        place: trust_runtime_core::native::memory::Place,
        /// The array's type.
        type_index: u32,
    },
}

/// The most live variable handles a runtime without source text (a chip) holds at once
/// (specification 11 §6.13.3). Each compound value a listing shows takes one handle, and a struct
/// or array handle keeps a copy of its value, so the bound limits what a debugging session takes
/// from the heap.
pub const CHIP_HANDLE_BOUND: usize = 256;

/// The variable handles of the current stop: positive identifiers that never alias a live handle,
/// and restart at `1` after a clear (specification 13, "Runtime debug state, mutation, and
/// projection invariants"). A table with a bound holds at most that many live handles, a handle
/// taken out for a listing counted; an allocation past the bound is refused, and the listing that
/// made it is rolled back. A page of an array that finds the table short of room first releases
/// the oldest earlier pages of arrays (specification 11 §6.13.3).
#[derive(Debug)]
pub struct DebugVariableHandles {
    /// The next identifier to hand out. Public only so that the boundary tests of the counter can
    /// move it; no other code writes it.
    #[doc(hidden)]
    pub next_id: u32,
    handles: BTreeMap<u32, VariableHandle>,
    /// The most live handles the table holds.
    bound: usize,
    /// The handles that a listing has taken out and not yet restored.
    taken: usize,
    /// Whether an allocation was refused since the last [`take_overflow`](Self::take_overflow).
    overflowed: bool,
    /// The identifiers that each page of an array took, oldest page first. Only a table with a
    /// bound keeps them, because only such a table releases a page.
    pages: VecDeque<Range<u32>>,
}

impl Default for DebugVariableHandles {
    fn default() -> Self {
        Self::new()
    }
}

impl DebugVariableHandles {
    /// An empty table whose first handle is `1`, without a bound: a computer keeps its handles as
    /// they have always been kept.
    #[must_use]
    pub fn new() -> Self {
        Self::with_bound(usize::MAX)
    }

    /// An empty table whose first handle is `1`, which holds at most `bound` live handles.
    #[must_use]
    pub fn with_bound(bound: usize) -> Self {
        Self {
            next_id: 0,
            handles: BTreeMap::new(),
            bound,
            taken: 0,
            overflowed: false,
            pages: VecDeque::new(),
        }
    }

    /// Invalidates every handle and restarts the numbering at `1`.
    pub fn clear(&mut self) {
        self.handles.clear();
        self.pages.clear();
        self.next_id = 1;
        self.overflowed = false;
    }

    /// Hands out the next identifier for `handle`. At the bound nothing is stored: the refusal is
    /// recorded for [`take_overflow`](Self::take_overflow), and `0` is returned, which the listing
    /// that made the refused allocation never shows.
    ///
    /// # Panics
    ///
    /// When the identifier space is used up while handles are still live, instead of handing out
    /// an identifier that aliases one of them (about four billion allocations without a clear).
    pub fn alloc(&mut self, handle: VariableHandle) -> u32 {
        assert!(
            self.next_id != 0 || self.handles.is_empty(),
            "debug variable handle space exhausted"
        );
        if self.handles.len().saturating_add(self.taken) >= self.bound {
            self.overflowed = true;
            return 0;
        }
        let id = self.next_id.max(1);
        self.next_id = id.checked_add(1).unwrap_or(0);
        self.handles.insert(id, handle);
        id
    }

    /// What handle `id` lists, when it is live.
    #[must_use]
    pub fn get(&self, id: u32) -> Option<&VariableHandle> {
        self.handles.get(&id)
    }

    /// Takes handle `id` out of the table, so that listing it can hand out the handles of its
    /// members without copying it; [`restore`](Self::restore) puts it back under the same
    /// identifier. A handle taken out still counts against the bound.
    pub fn take(&mut self, id: u32) -> Option<VariableHandle> {
        let handle = self.handles.remove(&id)?;
        self.taken += 1;
        Some(handle)
    }

    /// Puts back a handle that [`take`](Self::take) took out, under its own identifier.
    pub fn restore(&mut self, id: u32, handle: VariableHandle) {
        self.handles.insert(id, handle);
        self.taken = self.taken.saturating_sub(1);
    }

    /// The point a listing starts from, for [`rollback`](Self::rollback).
    #[must_use]
    pub fn mark(&self) -> u32 {
        self.next_id
    }

    /// Drops every handle allocated since `mark` and numbers the next handle from `mark` again, so
    /// that a refused listing leaves the table as it was (specification 11 §6.13.3).
    pub fn rollback(&mut self, mark: u32) {
        let _ = self.handles.split_off(&mark);
        self.pages.retain(|page| page.start < mark);
        self.next_id = mark;
    }

    /// Leaves room for `need` more handles by releasing the oldest pages of arrays that
    /// [`record_page`](Self::record_page) recorded, a whole page at a time and only as many pages as
    /// the room needs (specification 11 §6.13.3, "Handle bound"). It releases nothing when the
    /// table has the room already, and nothing when all the pages together would not make it: the
    /// listing is then refused whole and the table stays as it was.
    pub fn make_room(&mut self, need: usize) {
        let used = self.handles.len().saturating_add(self.taken);
        let shortfall = need.saturating_sub(self.bound.saturating_sub(used));
        if shortfall == 0 {
            return;
        }
        let releasable = self
            .pages
            .iter()
            .map(|page| self.handles.range(page.clone()).count())
            .sum::<usize>();
        if releasable < shortfall {
            return;
        }
        let mut released = 0;
        while released < shortfall {
            let Some(page) = self.pages.pop_front() else {
                break;
            };
            let ids = self
                .handles
                .range(page)
                .map(|(id, _)| *id)
                .collect::<Vec<_>>();
            released += ids.len();
            for id in ids {
                self.handles.remove(&id);
            }
        }
    }

    /// Records the handles allocated since `mark` as one page of an array, the unit that
    /// [`make_room`](Self::make_room) releases. A table without a bound never releases a page, so
    /// it keeps no record (specification 11 §6.13.3: a computer keeps its handles).
    pub fn record_page(&mut self, mark: u32) {
        if self.bound == usize::MAX || self.next_id <= mark {
            return;
        }
        self.pages.push_back(mark..self.next_id);
    }

    /// Whether an allocation was refused at the bound since the last call, which clears the record.
    pub fn take_overflow(&mut self) -> bool {
        core::mem::take(&mut self.overflowed)
    }

    /// How many handles are live.
    #[must_use]
    pub fn len(&self) -> usize {
        self.handles.len()
    }

    /// Whether no handle is live.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.handles.is_empty()
    }
}

/// The canonical IEC name of a value's type, or its public type name for a structure or an
/// enumeration.
#[must_use]
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

/// A value in the stable debug representation.
#[must_use]
pub fn format_value(value: &Value) -> String {
    format_user_value(value)
}

/// The variables of named values, each readable by its own name.
#[must_use]
pub fn variables_from_entries(
    handles: &mut DebugVariableHandles,
    entries: Vec<(String, Value)>,
) -> Vec<DebugVariable> {
    entries
        .into_iter()
        .map(|(name, value)| variable_from_value(handles, name.clone(), value, Some(name)))
        .collect()
}

/// The instance list: one expandable entry for each `(identity, type name)`, named
/// `<type>#<identity>`.
#[must_use]
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
            indexed_variables: None,
            named_variables: None,
        })
        .collect()
}

/// The fields of a structure value, in declaration order.
#[must_use]
#[allow(
    clippy::needless_pass_by_value,
    reason = "trust-runtime's handlers and contract tests hand the value over"
)]
pub fn variables_from_struct(
    handles: &mut DebugVariableHandles,
    value: StructValue,
) -> Vec<DebugVariable> {
    struct_fields(handles, &value, Page::everything())
}

/// The fields of a structure value that `page` lists, in declaration order, read from the value
/// where it is (specification 11 §6.13.3, "Variable pages").
#[must_use]
pub fn struct_fields(
    handles: &mut DebugVariableHandles,
    value: &StructValue,
    page: Page,
) -> Vec<DebugVariable> {
    page.select(ChildKind::Named, value.fields().iter())
        .map(|(name, field)| variable_for(handles, name, field, Some(name), None, None))
        .collect()
}

/// The elements of an array value, named by their zero-based index.
#[must_use]
#[allow(
    clippy::needless_pass_by_value,
    reason = "trust-runtime's handlers and contract tests hand the value over"
)]
pub fn variables_from_array(
    handles: &mut DebugVariableHandles,
    value: ArrayValue,
) -> Vec<DebugVariable> {
    array_elements(handles, &value, Page::everything())
}

/// The elements of an array value that `page` lists, named by their zero-based index in the whole
/// array, read from the value where it is. A page that finds the table short of the room for its
/// handles first releases the oldest earlier pages, and is recorded as a page itself (specification
/// 11 §6.13.3, "Handle bound").
#[must_use]
pub fn array_elements(
    handles: &mut DebugVariableHandles,
    value: &ArrayValue,
    page: Page,
) -> Vec<DebugVariable> {
    let listed = || page.select(ChildKind::Indexed, value.elements().iter().enumerate());
    handles.make_room(
        listed()
            .filter(|(_, element)| takes_handle(element))
            .count(),
    );
    let mark = handles.mark();
    let variables = listed()
        .map(|(index, element)| {
            let name = format!("[{index}]");
            variable_for(handles, &name, element, Some(&name), None, None)
        })
        .collect();
    handles.record_page(mark);
    variables
}

/// The sampled value of one row of the I/O scope.
#[derive(Debug, Clone, Copy)]
pub enum IoRowValue<'a> {
    /// The value read from the process image.
    Value(&'a Value),
    /// The image value could not be converted to the declared type.
    Error(&'a str),
    /// The address is a wildcard that configuration has not resolved.
    Unresolved,
}

/// One row of the I/O scope: a bound name, a direct address and the sampled value.
#[derive(Debug, Clone, Copy)]
pub struct IoRow<'a> {
    /// The bound name, when the binding has one.
    pub name: Option<&'a str>,
    /// The direct address.
    pub address: &'a IoAddress,
    /// The sampled value.
    pub value: IoRowValue<'a>,
}

/// The variables of the I/O scope: each row is named by its bound name, or else by its direct
/// address, and carries its value as the runtime's debug representation, `error: <detail>` or
/// `unresolved`.
#[must_use]
pub fn variables_from_io_rows<'a>(rows: impl IntoIterator<Item = IoRow<'a>>) -> Vec<DebugVariable> {
    rows.into_iter()
        .map(|row| {
            let name = row
                .name
                .map_or_else(|| format_io_address(row.address), str::to_string);
            DebugVariable {
                name: name.clone(),
                value: match row.value {
                    IoRowValue::Value(value) => format!("{value:?}"),
                    IoRowValue::Error(error) => format!("error: {error}"),
                    IoRowValue::Unresolved => "unresolved".to_string(),
                },
                r#type: None,
                variables_reference: 0,
                evaluate_name: Some(name),
                indexed_variables: None,
                named_variables: None,
            }
        })
        .collect()
}

/// One variable for `value`, expandable when it is a structure, an array, an instance or a
/// non-null reference.
#[must_use]
pub fn variable_from_value(
    handles: &mut DebugVariableHandles,
    name: String,
    value: Value,
    evaluate_name: Option<String>,
) -> DebugVariable {
    variable_from_value_with_metadata(handles, name, value, evaluate_name, None, None)
}

/// One variable for `value`, with a display text and a type name that override the derived ones
/// without changing whether the variable is expandable.
#[must_use]
#[allow(
    clippy::needless_pass_by_value,
    reason = "trust-runtime's handlers and contract tests hand the value over"
)]
pub fn variable_from_value_with_metadata(
    handles: &mut DebugVariableHandles,
    name: String,
    value: Value,
    evaluate_name: Option<String>,
    display: Option<String>,
    type_name: Option<String>,
) -> DebugVariable {
    variable_for(
        handles,
        &name,
        &value,
        evaluate_name.as_deref(),
        display,
        type_name,
    )
}

/// Whether [`variable_for`] gives the variable of `value` a handle: a structure, an array, an
/// instance and a non-null reference are expandable (specification 13, "Runtime debug state,
/// mutation, and projection invariants"). A page counts these to know the room its handles need.
fn takes_handle(value: &Value) -> bool {
    matches!(
        value,
        Value::Struct(_) | Value::Array(_) | Value::Instance(_) | Value::Reference(Some(_))
    )
}

/// The count of a container's children as an entry reports it. No container of a device or of a
/// computer holds more than `u32::MAX` children.
fn child_count(len: usize) -> u32 {
    u32::try_from(len).unwrap_or(u32::MAX)
}

/// One variable for the value `value` where it is, with a display text and a type name that
/// override the derived ones. Only a compound value is copied, into the handle that expands it, so
/// listing scalars copies nothing. A structure reports its member count and an array its element
/// count (specification 11 §6.13.3, "Variable pages").
#[must_use]
pub fn variable_for(
    handles: &mut DebugVariableHandles,
    name: &str,
    value: &Value,
    evaluate_name: Option<&str>,
    display: Option<String>,
    type_name: Option<String>,
) -> DebugVariable {
    let mut variables_reference = 0;
    let mut indexed_variables = None;
    let mut named_variables = None;
    match value {
        Value::Struct(inner) => {
            variables_reference = handles.alloc(VariableHandle::Struct((**inner).clone()));
            named_variables = Some(child_count(inner.fields().len()));
        }
        Value::Array(inner) => {
            variables_reference = handles.alloc(VariableHandle::Array((**inner).clone()));
            indexed_variables = Some(child_count(inner.elements().len()));
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
        name: name.to_string(),
        value: display.unwrap_or_else(|| format_value(value)),
        r#type: type_name.or_else(|| value_type_name(value)),
        variables_reference,
        evaluate_name: evaluate_name.map(str::to_string),
        indexed_variables,
        named_variables,
    }
}

/// The display text and type name of an instance value: the instance's public type name. A value
/// that is not a live instance has neither.
#[must_use]
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

/// The direct address as the debugger names an unnamed I/O row.
#[must_use]
pub fn format_io_address(address: &IoAddress) -> String {
    let area = match address.area {
        IoArea::Input => "I",
        IoArea::Output => "Q",
        IoArea::Memory => "M",
    };
    let size = match address.size {
        IoSize::Bit => "X",
        IoSize::Byte | IoSize::Bytes(_) => "B",
        IoSize::Word => "W",
        IoSize::DWord => "D",
        IoSize::LWord => "L",
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

/// The name of a file for display: the last `/` component of its label.
#[must_use]
pub fn file_name_of(label: &str) -> &str {
    label.rsplit('/').next().unwrap_or(label)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_file_is_named_by_the_last_component_of_its_label() {
        // Specification 11 §6.13.3: `name` is the last `/` component of the label. Wrong behaviour
        // caught: a name that keeps the directories, and one that is empty for a label without any.
        assert_eq!(file_name_of("portability/f401/gpio.st"), "gpio.st");
        assert_eq!(file_name_of("main.st"), "main.st");
    }

    #[test]
    fn an_io_row_is_named_by_its_binding_or_else_by_its_address() {
        // Specification 13, "Runtime debug state, mutation, and projection invariants": an I/O entry
        // uses its declared symbol name when present and otherwise a canonical uppercase direct
        // address; its value is the runtime debug value, `error: <detail>` or `unresolved`.
        // Wrong behaviour caught: a row named by its address although it has a name, a value without
        // the `error: ` prefix, and an unnamed row without a name.
        let named = IoAddress::parse("%QX0.0").expect("a direct address");
        let unnamed = IoAddress::parse("%IW4").expect("a direct address");
        let flag = Value::Bool(true);
        let rows = [
            IoRow {
                name: Some("motor"),
                address: &named,
                value: IoRowValue::Value(&flag),
            },
            IoRow {
                name: None,
                address: &unnamed,
                value: IoRowValue::Error("offline"),
            },
            IoRow {
                name: None,
                address: &named,
                value: IoRowValue::Unresolved,
            },
        ];

        let variables = variables_from_io_rows(rows);

        let listed = variables
            .iter()
            .map(|variable| (variable.name.as_str(), variable.value.as_str()))
            .collect::<Vec<_>>();
        assert_eq!(
            listed,
            [
                ("motor", "Bool(true)"),
                ("%IW4", "error: offline"),
                ("%QX0.0", "unresolved")
            ]
        );
    }

    #[test]
    fn handles_are_counted_and_a_clear_empties_them() {
        // Specification 13, "Runtime debug state, mutation, and projection invariants": a clear
        // invalidates every old handle. Wrong behaviour caught: a table that still counts handles
        // after the clear.
        let mut handles = DebugVariableHandles::new();
        assert!(handles.is_empty());
        let _ = handles.alloc(VariableHandle::Globals);
        let _ = handles.alloc(VariableHandle::Retain);
        assert_eq!(handles.len(), 2);
        handles.clear();
        assert!(handles.is_empty());
    }

    #[test]
    fn the_handle_table_stops_at_its_bound() {
        // Specification 11 §6.13.3: a runtime without source text holds at most 256 live handles, a
        // handle taken out for a listing counted, and the allocation past the bound is refused. Wrong
        // behaviour caught: an unbounded table (the 257th handle is numbered), a bound one handle
        // short (the 256th is refused) or long (the 257th is numbered), and a handle taken out for a
        // listing that does not count, so the table would grow past the bound by one per listing.
        assert_eq!(CHIP_HANDLE_BOUND, 256, "specification 11 §6.13.3");
        let mut handles = DebugVariableHandles::with_bound(256);
        for expected in 1..=256 {
            assert_eq!(handles.alloc(VariableHandle::Globals), expected);
        }
        assert_eq!(handles.alloc(VariableHandle::Globals), 0);
        assert!(handles.take_overflow(), "the refusal is recorded");
        assert!(
            !handles.take_overflow(),
            "the record is cleared by the read"
        );
        assert_eq!(handles.len(), 256);

        assert!(handles.take(1).is_some());
        assert_eq!(
            handles.alloc(VariableHandle::Globals),
            0,
            "a handle out for a listing still counts against the bound"
        );
        assert!(handles.take_overflow());
        handles.restore(1, VariableHandle::Globals);
        assert_eq!(handles.len(), 256);
    }

    #[test]
    fn a_refused_listing_leaves_the_table_as_it_was() {
        // Specification 11 §6.13.3: a listing the bound refuses leaves the table as it was, and the
        // next handle takes the number that the refused listing started from. Wrong behaviour caught:
        // a refused listing that keeps the handles it had allocated (the table stays full), and one
        // that numbers the next handle on from them.
        let mut handles = DebugVariableHandles::with_bound(3);
        assert_eq!(handles.alloc(VariableHandle::Globals), 1);
        let mark = handles.mark();
        assert_eq!(handles.alloc(VariableHandle::Globals), 2);
        assert_eq!(handles.alloc(VariableHandle::Globals), 3);
        assert_eq!(handles.alloc(VariableHandle::Globals), 0);
        assert!(handles.take_overflow());

        handles.rollback(mark);

        assert_eq!(handles.len(), 1);
        assert_eq!(handles.alloc(VariableHandle::Globals), 2);
    }

    /// Allocates `count` handles and records them as one page, as listing a page of an array does.
    fn page_of(handles: &mut DebugVariableHandles, count: usize) {
        let mark = handles.mark();
        for _ in 0..count {
            let _ = handles.alloc(VariableHandle::Globals);
        }
        handles.record_page(mark);
    }

    /// A table of ten with the two scope handles 1 and 2, then pages of the given sizes.
    fn table_of_ten_with_pages(pages: &[usize]) -> DebugVariableHandles {
        let mut handles = DebugVariableHandles::with_bound(10);
        let _ = handles.alloc(VariableHandle::Locals(FrameId(1)));
        let _ = handles.alloc(VariableHandle::Globals);
        for count in pages {
            page_of(&mut handles, *count);
        }
        handles
    }

    #[test]
    fn a_bounded_table_releases_the_oldest_page_first_and_only_as_far_as_the_room_needs() {
        // Specification 11 §6.13.3, "Handle bound": a page that finds the table short of room first
        // releases the oldest earlier pages of arrays, a whole page at a time and only as many as the
        // room needs. Worked: a table of 10 holds the scope handles 1 and 2 and two pages of 4
        // (3 to 6 and 7 to 10), so it is full. Room for 3 more needs 3 released: the oldest page goes
        // whole, which makes 4, and the newer page stays. Room for 5 more then finds 4 free and needs
        // 1 released: the remaining page goes whole.
        // Wrong behaviour caught: the newest page released first, a page released in part (3 to 5
        // gone and 6 kept), every page released when one is enough, a scope handle released, and
        // nothing released.
        let mut handles = table_of_ten_with_pages(&[4, 4]);
        assert_eq!(handles.len(), 10);

        handles.make_room(3);

        assert_eq!(handles.len(), 6);
        assert!(
            (3..=6).all(|id| handles.get(id).is_none()),
            "the oldest page"
        );
        assert!(
            (7..=10).all(|id| handles.get(id).is_some()),
            "the newer page"
        );
        assert!(handles.get(1).is_some() && handles.get(2).is_some());

        handles.make_room(5);

        assert_eq!(handles.len(), 2);
        assert!((7..=10).all(|id| handles.get(id).is_none()));
        assert!(handles.get(1).is_some() && handles.get(2).is_some());
    }

    #[test]
    fn a_table_that_cannot_make_the_room_releases_nothing() {
        // Specification 11 §6.13.3, "Handle bound": when all the earlier pages together would not make
        // the room, none is released, because the listing is then refused whole and the table stays
        // as it was; and none is released when the room is there already. Worked: a table of 10 holds
        // the scope handles and one page of 4, so 6 are live and 4 are free. Room for 4 is there;
        // room for 9 needs 5 released, and the page holds only 4.
        // Wrong behaviour caught: a page released although the listing is refused anyway, and a page
        // released when the room is there.
        let mut handles = table_of_ten_with_pages(&[4]);
        assert_eq!(handles.len(), 6);

        handles.make_room(4);
        assert_eq!(handles.len(), 6, "the room is there already");

        handles.make_room(9);
        assert_eq!(handles.len(), 6, "the page would not make the room");
        assert!((3..=6).all(|id| handles.get(id).is_some()));
    }

    #[test]
    fn a_table_without_a_bound_never_releases_a_page() {
        // Specification 11 §6.13.3, "Handle bound": a computer keeps its handles unbounded, as it
        // always has, and releases none. Worked: five handles live, the last four of them taken as a
        // page; a request for so much room that it would need those four released releases none,
        // because an unbounded table keeps no record of pages.
        // Wrong behaviour caught: a computer whose pages are released, so that a client's handles
        // would go stale where they never did.
        let mut handles = DebugVariableHandles::new();
        let _ = handles.alloc(VariableHandle::Locals(FrameId(1)));
        page_of(&mut handles, 4);

        handles.make_room(usize::MAX - 1);

        assert_eq!(handles.len(), 5);
        assert!((2..=5).all(|id| handles.get(id).is_some()));
    }

    #[test]
    fn only_an_expandable_value_takes_a_handle() {
        // Specification 13, "Runtime debug state, mutation, and projection invariants": scalar and
        // null values have `variablesReference = 0`; structs, arrays, instances and non-null
        // references allocate expandable handles. A page counts the values that take a handle to know
        // the room its handles need, so the count and the variable must agree.
        // Wrong behaviour caught: a count that includes a null reference or a scalar, or leaves out
        // an instance, so that a page would release too much or too little.
        use alloc::boxed::Box;
        use alloc::sync::Arc;
        use alloc::vec;
        use trust_runtime_core::collections::OrderedMap;
        use trust_runtime_core::memory::{Generation, MemoryLocation};

        let reference = ValueRef {
            location: MemoryLocation::Global,
            offset: 3,
            generation: Generation::FIRST,
            path: Vec::new(),
        };
        let structure = StructValue::from_untyped_parts("Motor".into(), OrderedMap::default());
        let array =
            ArrayValue::from_untyped_parts(vec![Value::DInt(1)], vec![(0, 0)]).expect("an array");
        let cases = [
            (Value::Bool(true), false),
            (Value::DInt(5), false),
            (Value::Null, false),
            (Value::Reference(None), false),
            (Value::Struct(Arc::new(structure)), true),
            (Value::Array(Box::new(array)), true),
            (Value::Instance(InstanceId(7)), true),
            (Value::Reference(Some(reference)), true),
        ];

        for (value, expands) in cases {
            let mut handles = DebugVariableHandles::new();
            let variable = variable_for(&mut handles, "v", &value, None, None, None);
            assert_eq!(takes_handle(&value), expands, "{value:?}");
            assert_eq!(variable.variables_reference != 0, expands, "{value:?}");
        }
    }
}
