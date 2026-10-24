//! A compiled program's memory as the tools read and write it (specification 44 §13.2, §13.3): the
//! areas between scans, and while a debugger holds a scan, the areas and the frames of the scan's
//! call stack. Live Values, the debugger and a stepped scan reach a variable through the symbol
//! table, the retain map or a frame, and read and write it through the type table.

use alloc::boxed::Box;
use alloc::format;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;

use smol_str::SmolStr;

use super::image::{Image, RetainRecord, SymbolRecord, NONE, SYMBOL_PROGRAM};
use super::values::{damaged, read_value, Types, Writer};
use crate::error::RuntimeError;
use crate::io_address::IoAddress;
use crate::value::Value;

/// Where a variable lies.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Place {
    /// At `offset` of area `area` (specification 44 §7.1).
    Area {
        /// The area's id, 1 to 7.
        area: u8,
        /// The offset in the area.
        offset: u32,
    },
    /// At `offset` of the variables of frame `frame` of a held scan's call stack, innermost first.
    Frame {
        /// The frame's position in the call stack, 0 for the innermost.
        frame: usize,
        /// The offset in the frame.
        offset: u32,
    },
}

impl Place {
    /// The place `bytes` further on.
    #[must_use]
    pub fn at(self, bytes: u32) -> Self {
        match self {
            Self::Area { area, offset } => Self::Area {
                area,
                offset: offset.saturating_add(bytes),
            },
            Self::Frame { frame, offset } => Self::Frame {
                frame,
                offset: offset.saturating_add(bytes),
            },
        }
    }
}

/// One frame of a held scan's call stack (specification 44 §13.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StopFrame {
    /// The POU's id (§5.7).
    pub pou: u32,
    /// The statement the frame stands at: the held one for the innermost frame, the call for each
    /// caller.
    pub stmt: u32,
    /// The instance the POU runs on, in its area; `None` for a function.
    pub instance: Option<Place>,
    /// Whether the frame has frame variables.
    pub locals: bool,
}

/// A compiled program's memory: its areas, and while a debugger holds its scan the frames of the
/// scan's call stack.
pub trait ProgramMemory {
    /// The program's image.
    fn image(&self) -> Image<'_>;

    /// Copies the bytes at `offset` of area `id` into `out`; `false` outside the area.
    fn read_area(&self, id: u8, offset: usize, out: &mut [u8]) -> bool;

    /// Writes `data` at `offset` of area `id`; `false` outside the area.
    fn write_area(&mut self, id: u8, offset: usize, data: &[u8]) -> bool;

    /// The frames of the held scan's call stack, innermost first; none between scans.
    fn frames(&self) -> Vec<StopFrame> {
        Vec::new()
    }

    /// Copies the bytes at `offset` of the variables of frame `frame` into `out`; `false` outside
    /// the frame, and between scans.
    fn read_frame(&self, _frame: usize, _offset: usize, _out: &mut [u8]) -> bool {
        false
    }

    /// Writes `data` at `offset` of the variables of frame `frame`; `false` outside the frame, and
    /// between scans.
    fn write_frame(&mut self, _frame: usize, _offset: usize, _data: &[u8]) -> bool {
        false
    }

    /// Writes `value` at the process-image `address`.
    ///
    /// # Errors
    ///
    /// The [`RuntimeError`] of an address that is no flat address of its image, or of a value of
    /// another size than the address's.
    fn write_image(&mut self, address: &IoAddress, value: Value) -> Result<(), RuntimeError>;
}

fn read_bytes<M: ProgramMemory + ?Sized>(memory: &M, place: Place, out: &mut [u8]) -> bool {
    match place {
        Place::Area { area, offset } => memory.read_area(area, offset as usize, out),
        Place::Frame { frame, offset } => memory.read_frame(frame, offset as usize, out),
    }
}

fn write_bytes<M: ProgramMemory + ?Sized>(memory: &mut M, place: Place, data: &[u8]) -> bool {
    match place {
        Place::Area { area, offset } => memory.write_area(area, offset as usize, data),
        Place::Frame { frame, offset } => memory.write_frame(frame, offset as usize, data),
    }
}

/// The value of type `type_index` at `place`, read as the interpreter holds it (specification 44
/// §5.8).
///
/// # Errors
///
/// [`RuntimeError::TypeMismatch`] for a type that holds no value (a function block, a program) or
/// bytes that are no value of the type, and [`RuntimeError::InvalidBundle`] for a place outside
/// its area or frame or a type the type table does not hold.
pub fn read_place<M: ProgramMemory + ?Sized>(
    memory: &M,
    place: Place,
    type_index: u32,
) -> Result<Value, RuntimeError> {
    let image = memory.image();
    let types = Types::of(&image)?;
    let size = types.get(type_index)?.size as usize;
    let mut bytes = vec![0u8; size];
    if !read_bytes(memory, place, &mut bytes) {
        return Err(damaged("symbol table"));
    }
    read_value(&types, bytes.as_slice(), 0, type_index)
}

/// Writes `value` into the variable of type `type_index` at `place` with the conversions of the
/// type table (specification 11 §6.7): a number widened within its family, a text cut to its
/// capacity, an enumeration of the same type and value, a structure member by member. The variable
/// keeps its value when the write is refused.
///
/// # Errors
///
/// [`RuntimeError::TypeMismatch`] for a value the type refuses, [`RuntimeError::SubrangeViolation`]
/// for a number outside its subrange, and [`RuntimeError::InvalidBundle`] for a place outside its
/// area or frame.
pub fn write_place<M: ProgramMemory + ?Sized>(
    memory: &mut M,
    place: Place,
    type_index: u32,
    value: &Value,
) -> Result<(), RuntimeError> {
    let staged = {
        let image = memory.image();
        let types = Types::of(&image)?;
        let size = types.get(type_index)?.size as usize;
        let mut current = vec![0u8; size];
        if !read_bytes(memory, place, &mut current) {
            return Err(damaged("symbol table"));
        }
        let mut staged = current.clone();
        Writer {
            types: &types,
            staged: &mut staged,
            starting: &current,
        }
        .write(type_index, 0, value, 0)?;
        staged
    };
    if write_bytes(memory, place, &staged) {
        Ok(())
    } else {
        Err(damaged("symbol table"))
    }
}

/// The place and the type of the variable of symbol `index`: a variable of an area. A program, a
/// POU and a frame variable hold no value of their own in an area.
///
/// # Errors
///
/// [`RuntimeError::TypeMismatch`] for a symbol that names no variable of an area, and
/// [`RuntimeError::InvalidBundle`] for an index outside the symbol table.
pub fn symbol_place(image: &Image<'_>, index: u32) -> Result<(Place, u32), RuntimeError> {
    let symbol = image
        .table::<SymbolRecord>()
        .ok()
        .and_then(|symbols| symbols.get(index))
        .ok_or_else(|| damaged("symbol table"))?;
    if symbol.flags & SYMBOL_PROGRAM != 0
        || !(1..=6).contains(&symbol.area)
        || symbol.type_index == NONE
    {
        return Err(RuntimeError::TypeMismatch);
    }
    Ok((
        Place::Area {
            area: symbol.area,
            offset: symbol.offset,
        },
        symbol.type_index,
    ))
}

/// The place and the type of the retained variable whose retain-map key is `label`, compared
/// without regard to case (specification 44 §5.10).
///
/// # Errors
///
/// [`RuntimeError::UndefinedVariable`] for a key the retain map does not hold.
pub fn retained_place(image: &Image<'_>, label: &str) -> Result<(Place, u32), RuntimeError> {
    let records = image
        .table::<RetainRecord>()
        .map_err(|_| damaged("retain map"))?;
    let record = records
        .iter()
        .find(|record| {
            image
                .string(record.key)
                .is_some_and(|key| key.eq_ignore_ascii_case(label))
        })
        .ok_or_else(|| RuntimeError::UndefinedVariable(SmolStr::new(label)))?;
    let area = u8::try_from(record.area)
        .ok()
        .filter(|area| (1..=6).contains(area))
        .ok_or_else(|| damaged("retain map"))?;
    Ok((
        Place::Area {
            area,
            offset: record.offset,
        },
        record.type_index,
    ))
}

/// The place and the type of member `member` of the instance whose symbol is `instance`: the
/// member's symbol, `<instance>.<MEMBER>`.
///
/// # Errors
///
/// [`RuntimeError::UndefinedVariable`] for an instance symbol the image does not hold, and
/// [`RuntimeError::UndefinedField`] for a member it does not declare.
pub fn member_place(
    image: &Image<'_>,
    instance: u32,
    member: &str,
) -> Result<(Place, u32), RuntimeError> {
    let path = image
        .table::<SymbolRecord>()
        .ok()
        .and_then(|symbols| symbols.get(instance))
        .and_then(|symbol| image.string(symbol.canonical))
        .map(|owner| format!("{owner}.{}", member.to_ascii_uppercase()))
        .ok_or_else(|| RuntimeError::UndefinedVariable(format!("{instance}").into()))?;
    let (index, _) = image
        .find_symbol(&path)
        .ok_or_else(|| RuntimeError::UndefinedField(member.into()))?;
    symbol_place(image, index)
}

/// The index and the record of the POU symbol of POU `pou` (specification 44 §5.7): a debug image
/// has one for each POU, named `@` and the POU's canonical name.
#[must_use]
pub fn pou_symbol(image: &Image<'_>, pou: u32) -> Option<(u32, SymbolRecord)> {
    let symbols = image.table::<SymbolRecord>().ok()?;
    (0u32..).zip(symbols.iter()).find(|(_, symbol)| {
        symbol.area == 0
            && symbol.pou == pou
            && symbol.flags & SYMBOL_PROGRAM == 0
            && image
                .string(symbol.canonical)
                .is_some_and(|name| name.starts_with('@'))
    })
}

/// One member of an instance's type, as the type table lists it (specification 44 §5.8).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Member {
    /// The member's declared name.
    pub name: SmolStr,
    /// The member's type.
    pub type_index: u32,
    /// Its offset in the instance.
    pub offset: u32,
}

/// What a variable of a type is to a reader that browses it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Layout {
    /// A value the type table decodes: an elementary value, a text, an enumeration, a structure
    /// or an array of those.
    Value,
    /// A function block, class or program instance, browsed by its members; a library block's own
    /// state, which no program declares, is not among them (§5.7).
    Instance(Vec<Member>),
    /// An array of instances: its element type, its dimensions and the bytes between two elements.
    Instances {
        /// The element type.
        element: u32,
        /// Each dimension's bounds.
        dimensions: Vec<(i64, i64)>,
        /// The bytes between two elements.
        stride: u32,
    },
}

/// The layout of type `type_index`.
///
/// # Errors
///
/// [`RuntimeError::InvalidBundle`] for a type the type table does not hold.
pub fn layout(image: &Image<'_>, type_index: u32) -> Result<Layout, RuntimeError> {
    use super::image::type_kind as kind;
    let types = Types::of(image)?;
    let ty = types.get(type_index)?;
    match ty.kind {
        kind::FUNCTION_BLOCK | kind::CLASS | kind::PROGRAM => {
            let mut members = Vec::new();
            for member in types.members(&ty)? {
                let name = types.string(member.name)?;
                if name.starts_with("__") {
                    continue;
                }
                members.push(Member {
                    name: SmolStr::new(name),
                    type_index: member.type_index,
                    offset: member.offset,
                });
            }
            Ok(Layout::Instance(members))
        }
        kind::ARRAY => {
            let (element, dimensions) = types.array(&ty)?;
            let element_kind = types.get(element)?.kind;
            if matches!(element_kind, kind::FUNCTION_BLOCK | kind::CLASS) {
                let (_, stride) = types.layout(element, dimensions.clone())?;
                Ok(Layout::Instances {
                    element,
                    dimensions: dimensions.collect(),
                    stride: u32::try_from(stride).map_err(|_| RuntimeError::Overflow)?,
                })
            } else {
                Ok(Layout::Value)
            }
        }
        _ => Ok(Layout::Value),
    }
}

/// The IEC name of type `type_index`: its declared name, and for an anonymous array its
/// declaration, `ARRAY[1..3, 0..1] OF INT` (IEC 61131-3 Table 11); empty for another anonymous
/// type.
#[must_use]
pub fn type_name(image: &Image<'_>, type_index: u32) -> SmolStr {
    Types::of(image)
        .map(|types| named(&types, type_index, 0))
        .unwrap_or_default()
}

/// The name of type `type_index` at the array nesting `depth`, which a damaged table cannot make
/// endless.
fn named(types: &Types<'_>, type_index: u32, depth: u8) -> SmolStr {
    use super::image::type_kind as kind;
    let Ok(ty) = types.get(type_index) else {
        return SmolStr::default();
    };
    let name = types.string(ty.name).unwrap_or("");
    if !name.is_empty() || ty.kind != kind::ARRAY || depth >= 8 {
        return SmolStr::new(name);
    }
    let Ok((element, dimensions)) = types.array(&ty) else {
        return SmolStr::default();
    };
    let bounds = dimensions
        .map(|(lower, upper)| format!("{lower}..{upper}"))
        .collect::<Vec<String>>()
        .join(", ");
    SmolStr::new(format!(
        "ARRAY[{bounds}] OF {}",
        named(types, element, depth + 1)
    ))
}

/// What a type of the type table is, as a tool compares it with its own description of a variable,
/// a protocol client's point (specification 44 §5.8, §13.3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypeShape {
    /// An elementary type, by its `type_kind` (`type_kind::INT`, ...).
    Elementary(u32),
    /// `STRING[n]`, or `WSTRING[n]` when `wide`.
    Text {
        /// Whether the text is a `WSTRING`.
        wide: bool,
        /// The capacity `n` in characters.
        capacity: u32,
    },
    /// An enumeration, by its declared name. The type table holds the bytes of its integer base,
    /// not the base's name.
    Enumeration {
        /// The declared name.
        name: SmolStr,
        /// The bytes of a value.
        size: u32,
    },
    /// A subrange of an elementary integer type, by that type's `type_kind`.
    Subrange(u32),
    /// An array of `element`, with each dimension's bounds.
    Array {
        /// The shape of an element.
        element: Box<TypeShape>,
        /// Each dimension's lower and upper bound.
        dimensions: Vec<(i64, i64)>,
    },
    /// Any other type: a structure, a union, an instance, a reference or an interface.
    Other,
}

/// The shape of type `type_index`; an alias has its base type's.
///
/// # Errors
///
/// [`RuntimeError::InvalidBundle`] for a type the type table does not hold or whose detail does not
/// read.
pub fn type_shape(image: &Image<'_>, type_index: u32) -> Result<TypeShape, RuntimeError> {
    shape_of(&Types::of(image)?, type_index, 0)
}

/// The shape of type `type_index` at the nesting `depth`, which a damaged table cannot make endless.
fn shape_of(types: &Types<'_>, type_index: u32, depth: u8) -> Result<TypeShape, RuntimeError> {
    use super::image::type_kind as kind;
    if depth >= 8 {
        return Err(damaged("type table"));
    }
    let ty = types.get(type_index)?;
    Ok(match ty.kind {
        kind::STRING | kind::WSTRING => TypeShape::Text {
            wide: ty.kind == kind::WSTRING,
            capacity: ty.count,
        },
        kind::ENUM => TypeShape::Enumeration {
            name: SmolStr::new(types.string(ty.name)?),
            size: ty.size,
        },
        kind::SUBRANGE => TypeShape::Subrange(types.subrange(&ty)?.0),
        kind::ARRAY => {
            let (element, dimensions) = types.array(&ty)?;
            TypeShape::Array {
                element: Box::new(shape_of(types, element, depth + 1)?),
                dimensions: dimensions.collect(),
            }
        }
        kind::ALIAS => {
            let base = types
                .detail(&ty, 4)?
                .first_chunk::<4>()
                .map(|word| u32::from_le_bytes(*word))
                .ok_or_else(|| damaged("type table"))?;
            shape_of(types, base, depth + 1)?
        }
        elementary if elementary < kind::ENUM => TypeShape::Elementary(elementary),
        _ => TypeShape::Other,
    })
}

/// The names of the values of the enumeration `type_index`, in their declared order; empty for a
/// type that is no enumeration.
#[must_use]
pub fn enum_values(image: &Image<'_>, type_index: u32) -> Vec<SmolStr> {
    use super::image::type_kind as kind;
    let Ok(types) = Types::of(image) else {
        return Vec::new();
    };
    let Ok(ty) = types.get(type_index) else {
        return Vec::new();
    };
    if ty.kind != kind::ENUM {
        return Vec::new();
    }
    types
        .variants(&ty)
        .map(|values| {
            values
                .filter_map(|(name, _)| types.string(name).ok().map(SmolStr::new))
                .collect()
        })
        .unwrap_or_default()
}
