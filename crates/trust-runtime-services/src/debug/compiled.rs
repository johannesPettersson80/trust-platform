//! The debug reads of a compiled program (specification 44 §13.2, "The debugger of compiled
//! programs"): `debug.stack`, `debug.scopes` and `debug.variables` answered from the program's
//! memory through its image's tables, with the reply shapes, the error texts and the handle
//! discipline of the interpreter's reads (`requests`, `reads`).
//!
//! While the debugger holds a scan, the memory lists the scan's call frames innermost first; of `n`
//! frames, the one at index `i` is the stack's frame `n - i`, so the outermost is frame 1 and the
//! innermost frame `n`. A frame stands at its statement of the statement table (§5.9), the held one
//! for the innermost and the call for each caller, and is named by its POU symbol (§5.7). Between
//! scans no frame is held: the stack is the interpreter's one `Main` frame at the latest stop, and
//! no frame has scopes.
//!
//! A frame's `Locals` are the variables of the instance it runs on, then its frame variables.
//! `Globals`, `Retain` and `Instances` are read through the symbol table and the retain map (§5.7,
//! §5.10), and each variable through the type table (§5.8): a value is read from its place and
//! projected as the interpreter projects a value, so that a structure and an array take the
//! interpreter's handles; an instance is browsed by its members and an array of instances by its
//! elements, through handles of their own. An instance's identity is the index of its symbol, which
//! Live Values' `instance:<id>` takes.

use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec;
use alloc::vec::Vec;

use trust_runtime_core::debug::StatementSpan;
use trust_runtime_core::error::RuntimeError;
use trust_runtime_core::native::image::{
    Image, Record, RetainRecord, StmtRecord, SymbolRecord, Table, NONE, SYMBOL_INSTANCE,
    SYMBOL_PROGRAM,
};
use trust_runtime_core::native::memory::{
    layout, pou_symbol, read_place, type_name, Layout, Member, Place, ProgramMemory,
};

use super::paging::{ChildKind, Page};
use super::projection::{
    array_elements, struct_fields, variable_for, DebugScope, DebugVariable, DebugVariableHandles,
    VariableHandle,
};
use super::reads::{self, Located};
use super::replies;
use super::requests::{failed, located, parse, DebugScopesParams, DebugVariablesParams, Debugger};
use super::sources::LineSources;
use crate::live_values::io_requests::IoSnapshotRead;
use crate::live_values::io_snapshot::IoSnapshot;
use crate::protocol::ControlResponse;

/// The refusal of a read while the I/O snapshot cannot be read, in the interpreter's words.
const IO_UNAVAILABLE: &str = "I/O snapshot unavailable";

/// The refusal of a handle of the interpreter's kinds, which names no variable of a compiled
/// program, in the words the interpreter refuses a compiled program's handles with.
const NOT_THIS_PROGRAM: &str = "the handle names no variable of this program";

/// The refusal of the locals of a frame that the held call stack no longer has, because the scan
/// went on or the hold ended since the scopes were listed (specification 11 §6.9.5: a stale frame
/// fails closed).
const FRAME_GONE: &str = "the frame is no longer on the held call stack";

/// `debug.stack` of a compiled program (specification 44 §13.2; specification 13, "Stack Trace
/// and Navigation"): the held scan's frames innermost first, each named by its POU symbol and
/// placed at its statement, or at the latest stop when its statement has no position, and left out
/// without either, as the interpreter's frames are. Between scans it is the interpreter's one
/// `Main` frame at the latest stop. A frame whose POU has no POU symbol, which a debug image never
/// lacks, fails the request as the interpreter's inconsistent metadata fails it.
pub(super) fn stack(
    id: u64,
    memory: &dyn ProgramMemory,
    debugger: &dyn Debugger,
    sources: &LineSources,
) -> ControlResponse {
    let fallback = debugger
        .last_stop()
        .and_then(|stop| located(sources, stop.statement));
    let frames = memory.frames();
    if frames.is_empty() {
        return replies::stack(id, &reads::stack_frames(&[], |_| None, fallback.as_ref()));
    }
    let reader = Reader::new(memory);
    let mut listed = Vec::with_capacity(frames.len());
    for (index, frame) in frames.iter().enumerate() {
        let Some(name) = reader.pou_name(frame.pou) else {
            return failed(id, inconsistent());
        };
        let place = reader
            .statement(frame.stmt)
            .and_then(|statement| located(sources, statement))
            .or_else(|| fallback.clone());
        if let Some(place) = place {
            listed.push(replies::stack_frame(
                frame_id(frames.len(), index),
                name,
                &place.source,
                place.line,
                place.column,
            ));
        }
    }
    replies::stack(id, &listed)
}

/// `debug.scopes` of a compiled program (specification 44 §13.2; specification 13, "Variable
/// Visibility"): the frame's `Locals` at its statement, or at the latest stop, then `Globals` when
/// the program has a global, `Retain` when the retain map has a record, `I/O` when a snapshot with
/// entries is published and `Instances` when an instance lies in an area. The handles restart at 1,
/// and scopes that the handle bound refuses leave the table empty, as the interpreter's do.
pub(super) fn scopes(
    id: u64,
    params: Option<&serde_json::Value>,
    memory: &dyn ProgramMemory,
    debugger: &dyn Debugger,
    sources: &LineSources,
) -> ControlResponse {
    let params: DebugScopesParams = match parse(params) {
        Ok(params) => params,
        Err(error) => return failed(id, error),
    };
    let reader = Reader::new(memory);
    let frames = memory.frames();
    let Some(index) = frame_index(frames.len(), params.frame_id) else {
        return failed(id, replies::unknown_frame(params.frame_id));
    };
    let place = frames
        .get(index)
        .and_then(|frame| reader.statement(frame.stmt))
        .or_else(|| debugger.last_stop().map(|stop| stop.statement))
        .and_then(|statement| located(sources, statement));
    let has_io = match debugger.io_snapshot() {
        IoSnapshotRead::Published(io) => {
            !(io.inputs.is_empty() && io.outputs.is_empty() && io.memory.is_empty())
        }
        IoSnapshotRead::Missing => false,
        IoSnapshotRead::Unavailable => return failed(id, IO_UNAVAILABLE),
    };
    let others: Vec<(&str, VariableHandle)> = [
        (
            "Globals",
            VariableHandle::CompiledGlobals,
            reader
                .symbols()
                .any(|(_, symbol)| reader.is_global(&symbol)),
        ),
        (
            "Retain",
            VariableHandle::CompiledRetain,
            records::<RetainRecord>(&reader.image).next().is_some(),
        ),
        ("I/O", VariableHandle::IoRoot, has_io),
        (
            "Instances",
            VariableHandle::CompiledInstances,
            reader.symbols().any(|(_, symbol)| is_instance(&symbol)),
        ),
    ]
    .into_iter()
    .filter(|(_, _, shown)| *shown)
    .map(|(name, handle, _)| (name, handle))
    .collect();
    let mut reply = None;
    debugger.with_handles(&mut |handles| {
        let scopes = frame_scopes(handles, index, place.as_ref(), &others);
        reply = Some(if handles.take_overflow() {
            handles.clear();
            replies::handles_full(id)
        } else {
            replies::scopes(id, &scopes)
        });
    });
    reply.unwrap_or_else(|| failed(id, "debug variables unavailable"))
}

/// `debug.variables` of a compiled program (specification 44 §13.2): the variables a handle lists,
/// cut to the page the request asks for (specification 11 §6.13.3, "Variable pages"), with the
/// interpreter's handle discipline: the handle is out of the table while its members are listed and
/// back under its own identifier afterwards, and a listing that the bound refuses is rolled back,
/// so the table is as it was.
pub(super) fn variables(
    id: u64,
    params: Option<&serde_json::Value>,
    memory: &dyn ProgramMemory,
    debugger: &dyn Debugger,
) -> ControlResponse {
    let params: DebugVariablesParams = match parse(params) {
        Ok(params) => params,
        Err(error) => return failed(id, error),
    };
    let page = params.page();
    let io: Option<IoSnapshot> = match debugger.io_snapshot() {
        IoSnapshotRead::Published(io) => Some(io),
        IoSnapshotRead::Missing => None,
        IoSnapshotRead::Unavailable => return failed(id, IO_UNAVAILABLE),
    };
    let reader = Reader::new(memory);
    let mut reply = None;
    debugger.with_handles(&mut |handles| {
        reply = Some(match handles.take(params.variables_reference) {
            Some(handle) => {
                let mark = handles.mark();
                let members = reader.list(&handle, io.as_ref(), handles, page);
                handles.restore(params.variables_reference, handle);
                if handles.take_overflow() {
                    handles.rollback(mark);
                    replies::handles_full(id)
                } else {
                    match members {
                        Ok(variables) => replies::variables(id, &variables),
                        Err(error) => failed(id, error),
                    }
                }
            }
            None => failed(
                id,
                format!("unknown variables reference {}", params.variables_reference),
            ),
        });
    });
    reply.unwrap_or_else(|| failed(id, "debug variables unavailable"))
}

/// The scopes of the frame at `frame`, innermost first: its locals at `place`, then `others` in
/// order. The handles restart at 1, so the handles of an earlier stop are invalid from here, as the
/// interpreter's scopes make them (`reads::scopes`).
fn frame_scopes(
    handles: &mut DebugVariableHandles,
    frame: usize,
    place: Option<&Located>,
    others: &[(&str, VariableHandle)],
) -> Vec<DebugScope> {
    handles.clear();
    let mut scopes = vec![DebugScope {
        name: String::from("Locals"),
        variables_reference: handles.alloc(VariableHandle::CompiledLocals(frame)),
        expensive: false,
        source: place.map(|place| place.source.clone()),
        line: place.map(|place| place.line),
        column: place.map(|place| place.column),
        end_line: None,
        end_column: None,
    }];
    scopes.extend(others.iter().map(|(name, handle)| DebugScope {
        name: String::from(*name),
        variables_reference: handles.alloc(handle.clone()),
        expensive: false,
        source: None,
        line: None,
        column: None,
        end_line: None,
        end_column: None,
    }));
    scopes
}

/// A compiled program's memory with its image, through which the reads name, place and read its
/// variables.
struct Reader<'m> {
    /// The memory the values are read from: the areas, and the frames of a held scan.
    memory: &'m dyn ProgramMemory,
    /// The program's image, whose tables name and lay out the variables.
    image: Image<'m>,
}

/// One variable a listing shows: the name it is listed by, its type and where it lies.
#[derive(Clone, Copy)]
struct Child<'n> {
    name: &'n str,
    type_index: u32,
    place: Place,
}

/// What the members of an instance are listed from.
enum Members {
    /// The child symbols of the program root of this index: the program's variables, among them
    /// its retained ones, which lie in the retain area and not in the instance (specification 44
    /// §5.7).
    Root(u32),
    /// The members of the instance's type, each at its offset in the instance (§5.8).
    Type(Vec<Member>),
}

impl<'m> Reader<'m> {
    fn new(memory: &'m dyn ProgramMemory) -> Self {
        Self {
            memory,
            image: memory.image(),
        }
    }

    /// The symbols with their indexes, in the table's order (specification 44 §5.7).
    fn symbols(&self) -> impl Iterator<Item = (u32, SymbolRecord)> {
        records::<SymbolRecord>(&self.image)
    }

    /// The span of statement `stmt` by the statement table (specification 44 §5.9).
    fn statement(&self, stmt: u32) -> Option<StatementSpan> {
        let record = self.image.table::<StmtRecord>().ok()?.get(stmt)?;
        Some(StatementSpan {
            file_index: u32::from(record.file),
            start: (record.start_line, u32::from(record.start_column)),
            end: (record.end_line, u32::from(record.end_column)),
        })
    }

    /// The display name of the POU symbol of POU `pou` (specification 44 §5.7).
    fn pou_name(&self, pou: u32) -> Option<&'m str> {
        pou_symbol(&self.image, pou).and_then(|(_, symbol)| self.image.string(symbol.display))
    }

    /// The display name of `symbol`.
    fn display(&self, symbol: &SymbolRecord) -> &'m str {
        self.image.string(symbol.display).unwrap_or_default()
    }

    /// The variable of `symbol` at `place`, by its own name: its display name after the last `.`,
    /// the name it is declared with (`Plant.Speed` is `Speed`, `SumTo.N` is `N`).
    fn child(&self, symbol: &SymbolRecord, place: Place) -> Child<'m> {
        let display = self.display(symbol);
        Child {
            name: display.rsplit_once('.').map_or(display, |(_, own)| own),
            type_index: symbol.type_index,
            place,
        }
    }

    /// Whether `symbol` is a global variable (specification 44 §5.7): without a parent, in an area
    /// of variables, and neither a program root nor a POU's symbol, whose name begins with `@`.
    fn is_global(&self, symbol: &SymbolRecord) -> bool {
        symbol.parent == NONE
            && in_variable_area(symbol)
            && symbol.flags & SYMBOL_PROGRAM == 0
            && !self
                .image
                .string(symbol.canonical)
                .is_some_and(|name| name.starts_with('@'))
    }

    /// What the members of the instance of type `type_index` at `place` are listed from: the
    /// program root of that type that lies there, or else the type's members (specification 44
    /// §5.7, §5.8). The type tells a program from the function block instance that is its first
    /// variable and so lies where the program does.
    fn members(&self, place: Place, type_index: u32) -> Result<Members, String> {
        if let Place::Area { area, offset } = place
            && let Some((root, _)) = self.symbols().find(|(_, symbol)| {
                is_program_root(symbol)
                    && symbol.area == area
                    && symbol.offset == offset
                    && symbol.type_index == type_index
            })
        {
            return Ok(Members::Root(root));
        }
        match layout(&self.image, type_index) {
            Ok(Layout::Instance(members)) => Ok(Members::Type(members)),
            Ok(Layout::Value | Layout::Instances { .. }) => Ok(Members::Type(Vec::new())),
            Err(error) => Err(error.to_string()),
        }
    }

    /// The variables that `members` names for the instance at `place`, in order: a program root's
    /// child symbols in the symbol table's order, or the type's members in their declared order.
    fn children<'a>(
        &'a self,
        members: &'a Members,
        place: Place,
    ) -> impl Iterator<Item = Child<'a>> {
        let root = match members {
            Members::Root(root) => Some(*root),
            Members::Type(_) => None,
        };
        let of_type: &[Member] = match members {
            Members::Root(_) => &[],
            Members::Type(of_type) => of_type,
        };
        let of_root = root.into_iter().flat_map(move |parent| {
            self.symbols()
                .filter(move |(_, symbol)| symbol.parent == parent)
                .map(move |(_, symbol)| self.child(&symbol, area_place(&symbol)))
        });
        of_root.chain(of_type.iter().map(move |member| Child {
            name: member.name.as_str(),
            type_index: member.type_index,
            place: place.at(member.offset),
        }))
    }

    /// The variables that `handle` lists, cut to `page` (specification 44 §13.2; specification 11
    /// §6.13.3). A structure, an array and the I/O scope list as the interpreter lists them.
    fn list(
        &self,
        handle: &VariableHandle,
        io: Option<&IoSnapshot>,
        handles: &mut DebugVariableHandles,
        page: Page,
    ) -> Result<Vec<DebugVariable>, String> {
        Ok(match handle {
            VariableHandle::CompiledLocals(frame) => self.locals(*frame, handles, page)?,
            VariableHandle::CompiledGlobals => self.globals(handles, page),
            VariableHandle::CompiledRetain => self.retained(handles, page),
            VariableHandle::CompiledInstances => self.instances(handles, page),
            VariableHandle::CompiledInstance { place, type_index } => {
                self.instance(*place, *type_index, handles, page)?
            }
            VariableHandle::CompiledInstanceArray { place, type_index } => {
                self.instance_array(*place, *type_index, handles, page)?
            }
            VariableHandle::Struct(value) => struct_fields(handles, value, page),
            VariableHandle::Array(value) => array_elements(handles, value, page),
            VariableHandle::IoRoot => io_root(io, handles, page)?,
            VariableHandle::IoInputs => reads::io_rows(io, |io| io.inputs.as_slice(), page)?,
            VariableHandle::IoOutputs => reads::io_rows(io, |io| io.outputs.as_slice(), page)?,
            VariableHandle::IoMemory => reads::io_rows(io, |io| io.memory.as_slice(), page)?,
            VariableHandle::Locals(_)
            | VariableHandle::Globals
            | VariableHandle::Retain
            | VariableHandle::Instances
            | VariableHandle::Instance(_)
            | VariableHandle::Reference(_) => return Err(String::from(NOT_THIS_PROGRAM)),
        })
    }

    /// The locals of frame `index` of the held call stack, innermost first, that `page` lists: the
    /// variables of the instance the frame runs on, then its frame variables, the child symbols of
    /// its POU symbol, in its frame (specification 44 §5.7, §13.2).
    fn locals(
        &self,
        index: usize,
        handles: &mut DebugVariableHandles,
        page: Page,
    ) -> Result<Vec<DebugVariable>, String> {
        let frames = self.memory.frames();
        let frame = frames.get(index).ok_or_else(|| String::from(FRAME_GONE))?;
        let (pou, symbol) = pou_symbol(&self.image, frame.pou).ok_or_else(inconsistent)?;
        let instance = match frame.instance {
            Some(place) => Some((self.members(place, symbol.type_index)?, place)),
            None => None,
        };
        let of_instance = instance
            .iter()
            .flat_map(|(members, place)| self.children(members, *place));
        let own = self
            .symbols()
            .filter(|(_, variable)| frame.locals && variable.parent == pou)
            .map(|(_, variable)| {
                let place = Place::Frame {
                    frame: index,
                    offset: variable.offset,
                };
                self.child(&variable, place)
            });
        Ok(page
            .select(ChildKind::Named, of_instance.chain(own))
            .map(|child| self.entry(handles, child))
            .collect())
    }

    /// The globals that `page` lists: the global symbols in the table's order, each by its display
    /// name at its place (specification 44 §5.7).
    fn globals(&self, handles: &mut DebugVariableHandles, page: Page) -> Vec<DebugVariable> {
        let globals = self
            .symbols()
            .filter(|(_, symbol)| self.is_global(symbol))
            .map(|(_, symbol)| Child {
                name: self.display(&symbol),
                type_index: symbol.type_index,
                place: area_place(&symbol),
            });
        page.select(ChildKind::Named, globals)
            .map(|child| self.entry(handles, child))
            .collect()
    }

    /// The retained variables that `page` lists: the retain map's records in its order, each under
    /// its checkpoint key at its area and offset with its type (specification 44 §5.10). An area
    /// number that is no area's names no place, and its variable does not read.
    fn retained(&self, handles: &mut DebugVariableHandles, page: Page) -> Vec<DebugVariable> {
        let retained = records::<RetainRecord>(&self.image).map(|(_, record)| Child {
            name: self.image.string(record.key).unwrap_or_default(),
            type_index: record.type_index,
            place: Place::Area {
                area: u8::try_from(record.area).unwrap_or(u8::MAX),
                offset: record.offset,
            },
        });
        page.select(ChildKind::Named, retained)
            .map(|child| self.entry(handles, child))
            .collect()
    }

    /// The instances that `page` lists (specification 44 §13.2): the program roots and the function
    /// block and class instances that lie in an area, ascending by symbol index, each named
    /// `<type>#<index>` with its type as its value and its type, as the interpreter lists its
    /// instances. The index is the instance's identity, which Live Values' `instance:<id>` takes.
    fn instances(&self, handles: &mut DebugVariableHandles, page: Page) -> Vec<DebugVariable> {
        let instances = self.symbols().filter(|(_, symbol)| is_instance(symbol));
        page.select(ChildKind::Named, instances)
            .map(|(index, symbol)| {
                let named = type_name(&self.image, symbol.type_index);
                let declared = if named.is_empty() {
                    String::from(self.display(&symbol))
                } else {
                    named.to_string()
                };
                DebugVariable {
                    name: format!("{declared}#{index}"),
                    value: declared.clone(),
                    r#type: Some(declared),
                    variables_reference: handles.alloc(VariableHandle::CompiledInstance {
                        place: area_place(&symbol),
                        type_index: symbol.type_index,
                    }),
                    evaluate_name: None,
                    indexed_variables: None,
                    named_variables: None,
                }
            })
            .collect()
    }

    /// The members of the instance of type `type_index` at `place` that `page` lists
    /// (specification 44 §13.2).
    fn instance(
        &self,
        place: Place,
        type_index: u32,
        handles: &mut DebugVariableHandles,
        page: Page,
    ) -> Result<Vec<DebugVariable>, String> {
        let members = self.members(place, type_index)?;
        Ok(page
            .select(ChildKind::Named, self.children(&members, place))
            .map(|child| self.entry(handles, child))
            .collect())
    }

    /// The elements of the array of instances of type `type_index` at `place` that `page` lists, in
    /// row-major order, each an instance named by its zero-based index in the whole array, as an
    /// array's elements are named (`projection::array_elements`); an element of an array of
    /// instances is reached through its array (specification 44 §13.2). As a page of an array's
    /// elements, the page first makes room for its handles and is recorded as a page, which a later
    /// page may release (specification 11 §6.13.3, "Handle bound").
    fn instance_array(
        &self,
        place: Place,
        type_index: u32,
        handles: &mut DebugVariableHandles,
        page: Page,
    ) -> Result<Vec<DebugVariable>, String> {
        let (element, count, stride) = match layout(&self.image, type_index) {
            Ok(Layout::Instances {
                element,
                dimensions,
                stride,
            }) => (element, element_count(&dimensions), stride),
            Ok(Layout::Value | Layout::Instance(_)) => return Ok(Vec::new()),
            Err(error) => return Err(error.to_string()),
        };
        let listed = || page.select(ChildKind::Indexed, 0..count);
        // Every element is an instance, which takes a handle.
        handles.make_room(listed().count());
        let mark = handles.mark();
        let variables = listed()
            .map(|position| {
                let name = format!("[{position}]");
                let offset = u32::try_from(position)
                    .map_or(u32::MAX, |position| position.saturating_mul(stride));
                let child = Child {
                    name: name.as_str(),
                    type_index: element,
                    place: place.at(offset),
                };
                self.entry(handles, child)
            })
            .collect();
        handles.record_page(mark);
        Ok(variables)
    }

    /// The entry of `child` by its type's layout (specification 44 §5.8): a value is read from its
    /// place and projected as the interpreter projects a value, so that a structure and an array
    /// take the interpreter's handles; an instance takes a handle that lists its members, and an
    /// array of instances one that lists its elements, shown as the interpreter shows an array; a
    /// value that does not read shows the error's text, without a type or a handle.
    fn entry(&self, handles: &mut DebugVariableHandles, child: Child<'_>) -> DebugVariable {
        let Child {
            name,
            type_index,
            place,
        } = child;
        let shape = match layout(&self.image, type_index) {
            Ok(shape) => shape,
            Err(error) => return unreadable(name, &error),
        };
        match shape {
            Layout::Value => match read_place(self.memory, place, type_index) {
                Ok(value) => variable_for(handles, name, &value, Some(name), None, None),
                Err(error) => unreadable(name, &error),
            },
            Layout::Instance(members) => {
                let declared = type_name(&self.image, type_index).to_string();
                DebugVariable {
                    name: name.to_string(),
                    value: declared.clone(),
                    r#type: Some(declared),
                    variables_reference: handles
                        .alloc(VariableHandle::CompiledInstance { place, type_index }),
                    evaluate_name: Some(name.to_string()),
                    indexed_variables: None,
                    named_variables: Some(child_count(members.len())),
                }
            }
            Layout::Instances { dimensions, .. } => {
                let elements = element_count(&dimensions);
                DebugVariable {
                    name: name.to_string(),
                    value: format!("[{elements}]"),
                    r#type: Some(String::from("ARRAY")),
                    variables_reference: handles
                        .alloc(VariableHandle::CompiledInstanceArray { place, type_index }),
                    evaluate_name: Some(name.to_string()),
                    indexed_variables: Some(child_count(elements)),
                    named_variables: None,
                }
            }
        }
    }
}

/// The I/O scope's three areas that `page` lists, as the interpreter lists them (`reads`).
fn io_root(
    io: Option<&IoSnapshot>,
    handles: &mut DebugVariableHandles,
    page: Page,
) -> Result<Vec<DebugVariable>, String> {
    let io = io.ok_or_else(|| String::from(IO_UNAVAILABLE))?;
    let areas = [
        ("Inputs", io.inputs.len(), VariableHandle::IoInputs),
        ("Outputs", io.outputs.len(), VariableHandle::IoOutputs),
        ("Memory", io.memory.len(), VariableHandle::IoMemory),
    ];
    Ok(page
        .select(ChildKind::Named, areas.into_iter())
        .map(|(name, items, area)| reads::io_area(name, items, handles.alloc(area)))
        .collect())
}

/// The records of the table of `R` with their indexes, in the table's order; none when the table
/// does not read.
fn records<'i, R: Record + 'i>(image: &Image<'i>) -> impl Iterator<Item = (u32, R)> {
    let table = image.table::<R>().ok();
    let count = table.as_ref().map_or(0, Table::len);
    (0..count).filter_map(move |index| Some((index, table?.get(index)?)))
}

/// Whether `symbol` lies in an area of variables, 1 to 6 (specification 44 §7.1), and not in a
/// frame (7) or nowhere (0).
fn in_variable_area(symbol: &SymbolRecord) -> bool {
    (1..=6).contains(&symbol.area)
}

/// Whether `symbol` is a program root: a program instance (specification 44 §5.7).
fn is_program_root(symbol: &SymbolRecord) -> bool {
    symbol.flags & SYMBOL_PROGRAM != 0 && symbol.parent == NONE
}

/// Whether `symbol` is one of the instances the `Instances` scope lists: a program root, or a
/// function block or class instance, that lies in an area. An instance among the frame variables of
/// a function or a method lies in its frame, where only the frame's locals reach it.
fn is_instance(symbol: &SymbolRecord) -> bool {
    (is_program_root(symbol) || symbol.flags & SYMBOL_INSTANCE != 0) && in_variable_area(symbol)
}

/// Where the variable of `symbol` lies in its area.
fn area_place(symbol: &SymbolRecord) -> Place {
    Place::Area {
        area: symbol.area,
        offset: symbol.offset,
    }
}

/// The index, innermost first, of the frame that `frame_id` names among `count` held frames: the
/// outermost is frame 1 and the innermost frame `count`.
fn frame_index(count: usize, frame_id: u32) -> Option<usize> {
    let id = usize::try_from(frame_id)
        .ok()
        .filter(|id| (1..=count).contains(id))?;
    Some(count - id)
}

/// The identifier of the frame at `index`, innermost first, of `count` held frames.
fn frame_id(count: usize, index: usize) -> u32 {
    u32::try_from(count.saturating_sub(index)).unwrap_or(u32::MAX)
}

/// The number of elements of an array of `dimensions`; the layout that gave them found that the
/// count fits.
fn element_count(dimensions: &[(i64, i64)]) -> usize {
    dimensions
        .iter()
        .try_fold(1usize, |count, (lower, upper)| {
            let length = upper.checked_sub(*lower)?.checked_add(1)?;
            count.checked_mul(usize::try_from(length).ok()?)
        })
        .unwrap_or(0)
}

/// The count of a container's children as an entry reports it.
fn child_count(len: usize) -> u32 {
    u32::try_from(len).unwrap_or(u32::MAX)
}

/// The entry of a variable whose value does not read: the error's text as its value, without a
/// type or a handle.
fn unreadable(name: &str, error: &RuntimeError) -> DebugVariable {
    DebugVariable {
        name: name.to_string(),
        value: error.to_string(),
        r#type: None,
        variables_reference: 0,
        evaluate_name: Some(name.to_string()),
        indexed_variables: None,
        named_variables: None,
    }
}

/// The error text of metadata that does not name a frame's POU, the interpreter's for a frame it
/// cannot name.
fn inconsistent() -> String {
    RuntimeError::InvalidExecutionState.to_string()
}

mod names;

pub(super) use names::evaluate;
pub(crate) use names::FrameNames;

#[cfg(test)]
mod tests;
