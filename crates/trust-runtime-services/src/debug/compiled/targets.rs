//! What a debugger's write names in a compiled program (`debug.set`, specification 44 §13.2;
//! specification 13, "setVariable and setExpression"): a variable a listing showed, by the handle of
//! the listing and the name it showed, or a variable path, its first name looked up among the
//! variables of the held frame before the program's names. Either is found as the debug reads find
//! it, through the image's tables, and checked as a write: a constant, a member or an element of
//! one, and an instance, which holds no value of its own, are refused with a plain reason. A
//! variable that lies in an area has a path, by which Live Values queue and force it; a variable
//! of a frame on the held call stack has none.

use alloc::format;
use alloc::string::{String, ToString};

use trust_runtime_core::io_address::IoAddress;
use trust_runtime_core::native::image::{RetainRecord, SYMBOL_CONSTANT};
use trust_runtime_core::native::memory::{layout, pou_symbol, read_place, Layout, Place};
use trust_runtime_core::native::reflect::{
    array_shape, member_of, path_of, resolve_for_write, Located as VariableAt,
};
use trust_runtime_core::value::Value;

use super::super::projection::VariableHandle;
use super::{area_place, frame_index, records, Reader, FRAME_GONE};
use crate::live_values::io_snapshot::{IoSnapshot, IoSnapshotEntry};

/// The refusal of a handle whose children are copies of a value, which a debugger's write cannot
/// reach: an evaluated expression's structure or array.
const COPY: &str = "the value is a copy that an expression evaluated, not a variable: write the \
                    variable by its path";

/// What a debugger's write reaches.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::debug) enum WriteTarget {
    /// The variable, member or element at `at`, with its path when it lies in an area.
    Variable {
        /// Where it lies, and its type.
        at: VariableAt,
        /// Its path, which Live Values queue and force it by; none for a frame's variable.
        path: Option<String>,
    },
    /// A row of the I/O scope, written at its process-image address with the I/O requests.
    Io {
        /// The row's address.
        address: IoAddress,
    },
}

/// The variable named `name` that a listing of `handle` shows, over `memory` and the published
/// I/O snapshot `io`.
///
/// # Errors
///
/// The plain reason of a write the target cannot take.
pub(in crate::debug) fn listed(
    memory: &dyn trust_runtime_core::native::memory::ProgramMemory,
    io: Option<&IoSnapshot>,
    handle: &VariableHandle,
    name: &str,
) -> Result<WriteTarget, String> {
    let reader = Reader::new(memory);
    let at = match handle {
        VariableHandle::CompiledLocals(frame) => reader.local_named(*frame, name)?,
        VariableHandle::CompiledGlobals => reader.global_named(name),
        VariableHandle::CompiledRetain => reader.retained_named(name),
        VariableHandle::CompiledInstance { place, type_index } => {
            reader.member_named(*place, *type_index, name)?
        }
        VariableHandle::CompiledValue { place, type_index } => {
            reader.part_named(*place, *type_index, name)?
        }
        VariableHandle::CompiledInstances | VariableHandle::CompiledInstanceArray { .. } => {
            return Err(format!("{name} is an instance, which holds no value"));
        }
        VariableHandle::IoInputs => return io_row(io, |io| io.inputs.as_slice(), name),
        VariableHandle::IoOutputs => return io_row(io, |io| io.outputs.as_slice(), name),
        VariableHandle::IoMemory => return io_row(io, |io| io.memory.as_slice(), name),
        VariableHandle::IoRoot => {
            return Err(format!(
                "{name} is an area of the process image, which holds no value"
            ));
        }
        VariableHandle::Struct(_)
        | VariableHandle::Array(_)
        | VariableHandle::Reference(_)
        | VariableHandle::Instance(_)
        | VariableHandle::Locals(_)
        | VariableHandle::Globals
        | VariableHandle::Retain
        | VariableHandle::Instances => return Err(String::from(COPY)),
    };
    let at = at.ok_or_else(|| format!("the listing shows no variable {name}"))?;
    checked(&reader, at, name)
}

/// The variable `path` names over `memory`, its first name looked up among the variables of the
/// held frame `frame` first (an index from the innermost), as a statement of that frame names it.
///
/// # Errors
///
/// The plain reason of a path that names nothing a write reaches.
pub(in crate::debug) fn named(
    memory: &dyn trust_runtime_core::native::memory::ProgramMemory,
    path: &str,
    frame: Option<usize>,
) -> Result<WriteTarget, String> {
    let reader = Reader::new(memory);
    let first = |name: &str| frame.and_then(|index| reader.in_frame(index, name));
    let at =
        resolve_for_write(&reader.image, path.trim(), &first).map_err(|error| error.to_string())?;
    checked(&reader, at, path.trim())
}

/// The index, innermost first, of the held frame `frame_id` of `memory`.
///
/// # Errors
///
/// The refusal of a frame the held call stack does not have.
pub(in crate::debug) fn held_frame(
    memory: &dyn trust_runtime_core::native::memory::ProgramMemory,
    frame_id: u32,
) -> Result<usize, String> {
    frame_index(memory.frames().len(), frame_id).ok_or_else(|| String::from("unknown frame id"))
}

/// `at` as a write's target named `name`, checked as a write: an instance, which holds no value, is
/// refused; a variable of an area is found again by its path, which refuses a constant and a part
/// of one; a variable of a frame is refused when a constant of the frame holds it.
fn checked(reader: &Reader<'_>, at: VariableAt, name: &str) -> Result<WriteTarget, String> {
    if matches!(
        layout(&reader.image, at.type_index),
        Ok(Layout::Instance(_) | Layout::Instances { .. })
    ) {
        return Err(format!("{name} is an instance, which holds no value"));
    }
    match at.place {
        Place::Area { .. } => {
            // The path the variable is found by again refuses a constant that holds it; a path
            // that would find another place is not the variable's.
            let mut path = path_of(&reader.image, &at).map_err(|error| error.to_string())?;
            if let Some(text) = &path {
                let found = resolve_for_write(&reader.image, text, &|_| None)
                    .map_err(|error| error.to_string())?;
                if found != at {
                    path = None;
                }
            }
            Ok(WriteTarget::Variable { at, path })
        }
        Place::Frame { frame, offset } => {
            if reader.frame_constant(frame, offset) {
                return Err(format!("{name} is a constant"));
            }
            Ok(WriteTarget::Variable { at, path: None })
        }
    }
}

/// The row of an I/O area that `area` takes from the published snapshot, named `name` by its bound
/// name or else by its address, as the I/O scope lists it.
fn io_row(
    io: Option<&IoSnapshot>,
    area: impl FnOnce(&IoSnapshot) -> &[IoSnapshotEntry],
    name: &str,
) -> Result<WriteTarget, String> {
    let io = io.ok_or_else(|| String::from("I/O snapshot unavailable"))?;
    area(io)
        .iter()
        .find(|entry| {
            entry
                .name
                .as_deref()
                .map_or_else(|| entry.address.to_string() == name, |bound| bound == name)
        })
        .map(|entry| WriteTarget::Io {
            address: entry.address.clone(),
        })
        .ok_or_else(|| format!("the I/O scope shows no row {name}"))
}

impl Reader<'_> {
    /// The variable `name` among the locals of the held frame `index`: the variables of the
    /// instance it runs on, then its frame variables, as [`Reader::locals`] lists them.
    fn local_named(&self, index: usize, name: &str) -> Result<Option<VariableAt>, String> {
        let frames = self.memory.frames();
        let frame = frames.get(index).ok_or_else(|| String::from(FRAME_GONE))?;
        let (pou, symbol) = pou_symbol(&self.image, frame.pou)
            .ok_or_else(|| String::from("the frame's POU has no symbol"))?;
        if let Some(place) = frame.instance {
            let members = self.members(place, symbol.type_index)?;
            if let Some(child) = self
                .children(&members, place)
                .find(|child| child.name == name)
            {
                return Ok(Some(VariableAt {
                    place: child.place,
                    type_index: child.type_index,
                }));
            }
        }
        Ok(self
            .symbols()
            .filter(|(_, variable)| frame.locals && variable.parent == pou)
            .map(|(_, variable)| {
                let place = Place::Frame {
                    frame: index,
                    offset: variable.offset,
                };
                self.child(&variable, place)
            })
            .find(|child| child.name == name)
            .map(|child| VariableAt {
                place: child.place,
                type_index: child.type_index,
            }))
    }

    /// The global `name`, by the display name the `Globals` scope lists it by.
    fn global_named(&self, name: &str) -> Option<VariableAt> {
        self.symbols()
            .find(|(_, symbol)| self.is_global(symbol) && self.display(symbol) == name)
            .map(|(_, symbol)| VariableAt {
                place: area_place(&symbol),
                type_index: symbol.type_index,
            })
    }

    /// The retained variable under the checkpoint key `name`, as the `Retain` scope lists it.
    fn retained_named(&self, name: &str) -> Option<VariableAt> {
        records::<RetainRecord>(&self.image)
            .find(|(_, record)| self.image.string(record.key) == Some(name))
            .and_then(|(_, record)| {
                Some(VariableAt {
                    place: Place::Area {
                        area: u8::try_from(record.area).ok()?,
                        offset: record.offset,
                    },
                    type_index: record.type_index,
                })
            })
    }

    /// The member `name` of the instance of type `type_index` at `place`.
    fn member_named(
        &self,
        place: Place,
        type_index: u32,
        name: &str,
    ) -> Result<Option<VariableAt>, String> {
        let members = self.members(place, type_index)?;
        Ok(self
            .children(&members, place)
            .find(|child| child.name == name)
            .map(|child| VariableAt {
                place: child.place,
                type_index: child.type_index,
            }))
    }

    /// The member `name` of the structure, or the element `[position]` of the array of values, of
    /// type `type_index` at `place`, named as [`Reader::value_children`] lists them.
    fn part_named(
        &self,
        place: Place,
        type_index: u32,
        name: &str,
    ) -> Result<Option<VariableAt>, String> {
        let at = VariableAt { place, type_index };
        if let Some(position) = name
            .strip_prefix('[')
            .and_then(|rest| rest.strip_suffix(']'))
        {
            let Ok(position) = position.parse::<u32>() else {
                return Ok(None);
            };
            let Some(shape) = array_shape(&self.image, type_index).map_err(|e| e.to_string())?
            else {
                return Ok(None);
            };
            if !usize::try_from(position).is_ok_and(|position| position < shape.count()) {
                return Ok(None);
            }
            return Ok(Some(VariableAt {
                place: place.at(position.saturating_mul(shape.stride)),
                type_index: shape.element,
            }));
        }
        // A structure lists its members by their declared names.
        if !matches!(
            read_place(self.memory, place, type_index),
            Ok(Value::Struct(_))
        ) {
            return Ok(None);
        }
        member_of(&self.image, &at, name).map_err(|error| error.to_string())
    }

    /// The variable `name`, compared without case, of the held frame `index`: one of its frame
    /// variables, with whether it is a constant, or else a member of the instance it runs on.
    fn in_frame(&self, index: usize, name: &str) -> Option<(VariableAt, bool)> {
        let frames = self.memory.frames();
        let frame = frames.get(index)?;
        let (pou, symbol) = pou_symbol(&self.image, frame.pou)?;
        if frame.locals
            && let Some((_, variable)) = self.symbols().find(|(_, variable)| {
                variable.parent == pou && self.own_name(variable).eq_ignore_ascii_case(name)
            })
        {
            let at = VariableAt {
                place: Place::Frame {
                    frame: index,
                    offset: variable.offset,
                },
                type_index: variable.type_index,
            };
            return Some((at, variable.flags & SYMBOL_CONSTANT != 0));
        }
        let place = frame.instance?;
        let members = self.members(place, symbol.type_index).ok()?;
        let child = self
            .children(&members, place)
            .find(|child| child.name.eq_ignore_ascii_case(name))?;
        Some((
            VariableAt {
                place: child.place,
                type_index: child.type_index,
            },
            false,
        ))
    }

    /// The name a variable is declared with: its display name after the last `.`.
    fn own_name(&self, symbol: &trust_runtime_core::native::image::SymbolRecord) -> &str {
        let display = self.display(symbol);
        display.rsplit_once('.').map_or(display, |(_, own)| own)
    }

    /// Whether a constant among the frame variables of the held frame `index` holds the bytes at
    /// `offset`.
    fn frame_constant(&self, index: usize, offset: u32) -> bool {
        let frames = self.memory.frames();
        let Some((pou, _)) = frames
            .get(index)
            .and_then(|frame| pou_symbol(&self.image, frame.pou))
        else {
            return false;
        };
        self.symbols().any(|(_, variable)| {
            variable.parent == pou
                && variable.flags & SYMBOL_CONSTANT != 0
                && offset >= variable.offset
                && offset - variable.offset < variable.size.max(1)
        })
    }
}
