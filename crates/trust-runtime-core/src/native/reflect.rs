//! A compiled program's variables reached by path and its function blocks by type, through the
//! image's symbol and type tables alone (specification 44 §5.7, §5.8, §13.3): a computer's services
//! (Control Studio, the HMI, `Comm.Link`, the simulated plants) find `Main.Pumps[2].Running` and the
//! instances of a block and its derived blocks without the sources, as Live Values find a symbol.

use alloc::string::String;
use alloc::vec::Vec;
use core::fmt;

use smol_str::SmolStr;

use super::image::{type_kind as kind, Image, SymbolRecord, TypeRecord, NONE, SYMBOL_PROGRAM};
use super::memory::{symbol_place, Place};
use super::values::Types;
use crate::error::RuntimeError;

/// The deepest nesting a path or a walk follows, past which the image is taken as damaged.
const MAX_DEPTH: usize = 32;

/// A variable, a member or an element of a compiled program: where it lies and its type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Located {
    /// Where its bytes lie.
    pub place: Place,
    /// Its type's index in the type table.
    pub type_index: u32,
}

/// Why a path names nothing in a compiled program.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PathError {
    /// The path is empty or is not names and indices: `a.b[1, 2].c`.
    Syntax(SmolStr),
    /// No global, program instance or unique program variable has the path's first name.
    Unknown(SmolStr),
    /// The first name alone names a variable of several program instances.
    Ambiguous(SmolStr),
    /// A name that the value before it has no member of.
    NoMember(SmolStr),
    /// An index of a value that is no array, of another number of dimensions, or outside a
    /// dimension's bounds.
    Index(SmolStr),
    /// The image's tables do not read.
    Damaged,
}

impl fmt::Display for PathError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Syntax(path) => write!(formatter, "{path} is not a variable path"),
            Self::Unknown(name) => write!(formatter, "{name} names no variable"),
            Self::Ambiguous(name) => {
                write!(formatter, "{name} names a variable of several programs")
            }
            Self::NoMember(path) => write!(formatter, "{path} names no member"),
            Self::Index(path) => write!(formatter, "{path} is not an element of an array"),
            Self::Damaged => formatter.write_str("the program image's tables do not read"),
        }
    }
}

impl From<RuntimeError> for PathError {
    fn from(_: RuntimeError) -> Self {
        Self::Damaged
    }
}

/// One step of a path: a member's name or an element's indices.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Step<'p> {
    Member(&'p str),
    Index(Vec<i64>),
}

/// The steps of `path`: names joined by `.`, each name followed by any number of index lists
/// `[i, j]`; spaces around an index are allowed.
fn steps(path: &str) -> Result<Vec<Step<'_>>, PathError> {
    let syntax = || PathError::Syntax(SmolStr::new(path));
    let mut steps = Vec::new();
    let mut rest = path;
    loop {
        let end = rest.find(['.', '[']).unwrap_or(rest.len());
        let name = &rest[..end];
        if name.is_empty() || !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
            return Err(syntax());
        }
        steps.push(Step::Member(name));
        rest = &rest[end..];
        while let Some(after) = rest.strip_prefix('[') {
            let close = after.find(']').ok_or_else(syntax)?;
            let indices = after[..close]
                .split(',')
                .map(|index| index.trim().parse::<i64>().map_err(|_| syntax()))
                .collect::<Result<Vec<_>, _>>()?;
            steps.push(Step::Index(indices));
            rest = &after[close + 1..];
        }
        if rest.is_empty() {
            return Ok(steps);
        }
        rest = rest.strip_prefix('.').ok_or_else(syntax)?;
    }
}

/// The variable, member or element `path` names, compared without case (IEC 61131-3 §6.1.2): its
/// first name is a global, a program instance or, when exactly one program declares it, a program
/// variable, as [`Image::find_variable`] resolves a name; each further name a member of a
/// structure, union, function block, class or program instance (a derived block's inherited members
/// included), and each index list an element of an array, within its declared bounds. The longest
/// part of the path that the symbol table names is found by one lookup.
///
/// # Errors
///
/// The [`PathError`] that says why the path names nothing.
pub fn resolve_path(image: &Image<'_>, path: &str) -> Result<Located, PathError> {
    let steps = steps(path)?;
    let types = Types::of(image)?;
    // The names before the first index, as long as the symbol table names them.
    let names: Vec<&str> = steps
        .iter()
        .map_while(|step| match step {
            Step::Member(name) => Some(*name),
            Step::Index(_) => None,
        })
        .collect();
    let mut found = None;
    for length in (1..=names.len()).rev() {
        let canonical = names[..length].join(".").to_ascii_uppercase();
        if let Some((_, symbol)) = image.find_symbol(&canonical)
            && let Some(located) = located_symbol(&symbol)
        {
            found = Some((located, length));
            break;
        }
    }
    let (mut located, taken) = match found {
        Some(found) => found,
        None => match image.find_variable(names[0]) {
            Ok(index) => {
                let (place, type_index) = symbol_place(image, index)?;
                (Located { place, type_index }, 1)
            }
            Err(super::variables::NameError::Ambiguous) => {
                return Err(PathError::Ambiguous(SmolStr::new(names[0])));
            }
            Err(_) => return Err(PathError::Unknown(SmolStr::new(names[0]))),
        },
    };
    let mut walked = String::from(names[..taken].join(".").as_str());
    for step in &steps[taken..] {
        located = match step {
            Step::Member(name) => {
                walked.push('.');
                walked.push_str(name);
                member(&types, located, name)?
                    .ok_or_else(|| PathError::NoMember(SmolStr::new(walked.as_str())))?
            }
            Step::Index(indices) => {
                walked.push('[');
                walked.push_str(&indices_text(indices));
                walked.push(']');
                element(&types, located, indices)?
                    .ok_or_else(|| PathError::Index(SmolStr::new(walked.as_str())))?
            }
        };
    }
    Ok(located)
}

/// Where the variable or program instance of `symbol` lies, with its type: a symbol of an area,
/// a program instance's root included, whose members a path then names; `None` for a symbol that
/// holds no storage (a POU's, a frame variable's).
fn located_symbol(symbol: &SymbolRecord) -> Option<Located> {
    ((1..=6).contains(&symbol.area) && symbol.type_index != NONE).then_some(Located {
        place: Place::Area {
            area: symbol.area,
            offset: symbol.offset,
        },
        type_index: symbol.type_index,
    })
}

fn indices_text(indices: &[i64]) -> String {
    let mut text = String::new();
    for (position, index) in indices.iter().enumerate() {
        if position > 0 {
            text.push(',');
        }
        text.push_str(&alloc::format!("{index}"));
    }
    text
}

/// The type `type_index` names, through its aliases.
fn unaliased(types: &Types<'_>, type_index: u32) -> Result<(u32, TypeRecord), RuntimeError> {
    let mut index = type_index;
    for _ in 0..MAX_DEPTH {
        let record = types.get(index)?;
        if record.kind != kind::ALIAS {
            return Ok((index, record));
        }
        let detail = types.detail(&record, 4)?;
        index = u32::from_le_bytes([detail[0], detail[1], detail[2], detail[3]]);
    }
    Err(RuntimeError::InvalidBundle(SmolStr::new_static(
        "the program image's type table nests too deep",
    )))
}

/// The member `name` of the value at `located`; `None` for a value without such a member. Hidden
/// members (`__BASE`, `__CLASS`, an edge's trigger) are not members to a path.
fn member(
    types: &Types<'_>,
    located: Located,
    name: &str,
) -> Result<Option<Located>, RuntimeError> {
    let (_, record) = unaliased(types, located.type_index)?;
    if !matches!(
        record.kind,
        kind::STRUCT | kind::UNION | kind::FUNCTION_BLOCK | kind::CLASS | kind::PROGRAM
    ) {
        return Ok(None);
    }
    for detail in types.members(&record)? {
        let member_name = types.string(detail.name)?;
        if !member_name.starts_with("__") && member_name.eq_ignore_ascii_case(name) {
            return Ok(Some(Located {
                place: located.place.at(detail.offset),
                type_index: detail.type_index,
            }));
        }
    }
    Ok(None)
}

/// The element `indices` of the array at `located`, in row-major order; `None` for a value that is
/// no array, another number of indices than its dimensions, or an index outside its bounds.
fn element(
    types: &Types<'_>,
    located: Located,
    indices: &[i64],
) -> Result<Option<Located>, RuntimeError> {
    let (_, record) = unaliased(types, located.type_index)?;
    if record.kind != kind::ARRAY {
        return Ok(None);
    }
    let (element_type, dimensions) = types.array(&record)?;
    if dimensions.len() != indices.len() {
        return Ok(None);
    }
    let stride = u64::from(types.get(element_type)?.size);
    let mut position: u64 = 0;
    for ((lower, upper), index) in dimensions.zip(indices) {
        if *index < lower || *index > upper {
            return Ok(None);
        }
        let length = u64::try_from(upper - lower + 1).map_err(|_| RuntimeError::Overflow)?;
        let offset = u64::try_from(index - lower).map_err(|_| RuntimeError::Overflow)?;
        position = position
            .checked_mul(length)
            .and_then(|position| position.checked_add(offset))
            .ok_or(RuntimeError::Overflow)?;
    }
    let bytes = position
        .checked_mul(stride)
        .and_then(|bytes| u32::try_from(bytes).ok())
        .ok_or(RuntimeError::Overflow)?;
    Ok(Some(Located {
        place: located.place.at(bytes),
        type_index: element_type,
    }))
}

/// What a function block's or class's type record says beyond its members (specification 44
/// §5.8): its base type, and the interfaces it implements, by their declared names.
fn class_detail<'i>(
    types: &Types<'i>,
    record: &TypeRecord,
) -> Result<(u32, Vec<&'i str>), RuntimeError> {
    let members = usize::try_from(record.count).map_err(|_| RuntimeError::Overflow)?;
    let after = members
        .checked_mul(super::image::MEMBER_BYTES)
        .ok_or(RuntimeError::Overflow)?;
    let head = types.detail(record, after + 8)?;
    let word = |at: usize| u32::from_le_bytes([head[at], head[at + 1], head[at + 2], head[at + 3]]);
    let base = word(after);
    let count = usize::try_from(word(after + 4)).map_err(|_| RuntimeError::Overflow)?;
    let all = types.detail(
        record,
        after + 8 + count.checked_mul(4).ok_or(RuntimeError::Overflow)?,
    )?;
    let mut interfaces = Vec::with_capacity(count);
    for name in all[after + 8..].chunks_exact(4) {
        interfaces.push(types.string(u32::from_le_bytes([name[0], name[1], name[2], name[3]]))?);
    }
    Ok((base, interfaces))
}

/// The type a function block or class of type `type_index` extends; `None` for another type and for
/// a block without a base.
///
/// # Errors
///
/// [`RuntimeError::InvalidBundle`] for a type table that does not read.
pub fn type_base(image: &Image<'_>, type_index: u32) -> Result<Option<u32>, RuntimeError> {
    let types = Types::of(image)?;
    let (_, record) = unaliased(&types, type_index)?;
    if !matches!(record.kind, kind::FUNCTION_BLOCK | kind::CLASS) {
        return Ok(None);
    }
    let (base, _) = class_detail(&types, &record)?;
    Ok((base != NONE).then_some(base))
}

/// The interfaces a function block or class of type `type_index` implements, itself, through a
/// base or through an interface that extends one (IEC 61131-3 §6.6.6.4), by their declared names;
/// none for another type.
///
/// # Errors
///
/// [`RuntimeError::InvalidBundle`] for a type table that does not read.
pub fn type_interfaces(image: &Image<'_>, type_index: u32) -> Result<Vec<SmolStr>, RuntimeError> {
    let types = Types::of(image)?;
    let (_, record) = unaliased(&types, type_index)?;
    if !matches!(record.kind, kind::FUNCTION_BLOCK | kind::CLASS) {
        return Ok(Vec::new());
    }
    let (_, interfaces) = class_detail(&types, &record)?;
    Ok(interfaces.into_iter().map(SmolStr::new).collect())
}

/// Whether type `type_index` is the function block or class `name` or extends it, compared without
/// case.
///
/// # Errors
///
/// [`RuntimeError::InvalidBundle`] for a type table that does not read.
pub fn type_is_a(image: &Image<'_>, type_index: u32, name: &str) -> Result<bool, RuntimeError> {
    let types = Types::of(image)?;
    let mut current = Some(type_index);
    for _ in 0..MAX_DEPTH {
        let Some(index) = current else {
            return Ok(false);
        };
        let (_, record) = unaliased(&types, index)?;
        if !matches!(record.kind, kind::FUNCTION_BLOCK | kind::CLASS) {
            return Ok(false);
        }
        if record.name != 0 && types.string(record.name)?.eq_ignore_ascii_case(name) {
            return Ok(true);
        }
        let (base, _) = class_detail(&types, &record)?;
        current = (base != NONE).then_some(base);
    }
    Ok(false)
}

/// Whether type `type_index` implements the interface `name`, compared without case.
///
/// # Errors
///
/// [`RuntimeError::InvalidBundle`] for a type table that does not read.
pub fn type_implements(
    image: &Image<'_>,
    type_index: u32,
    name: &str,
) -> Result<bool, RuntimeError> {
    Ok(type_interfaces(image, type_index)?
        .iter()
        .any(|interface| interface.eq_ignore_ascii_case(name)))
}

/// What a walk does past an instance it visits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Walk {
    /// Goes on into the instance's members.
    Descend,
    /// Leaves the instance's members unvisited.
    Skip,
    /// Ends the walk.
    Stop,
}

/// Visits every function block and class instance of the program, with its path and place, in
/// declaration order: each program instance's own variables, then the globals, and in each the
/// members of structures, the elements of arrays (`Pumps[1]`, in row-major order) and the members of
/// instances the visitor descends into. A program instance itself is not visited.
///
/// # Errors
///
/// [`RuntimeError::InvalidBundle`] for tables that do not read.
pub fn walk_instances(
    image: &Image<'_>,
    visit: &mut dyn FnMut(&str, Located) -> Walk,
) -> Result<(), RuntimeError> {
    let types = Types::of(image)?;
    let symbols = image
        .table::<SymbolRecord>()
        .map_err(|_| super::values::damaged("symbol table"))?;
    let roots: Vec<(u32, SymbolRecord)> = (0_u32..)
        .zip(symbols.iter())
        .filter(|(_, symbol)| symbol.area != 0 && (1..=6).contains(&symbol.area))
        .filter(|(_, symbol)| {
            let parent = if symbol.parent == NONE {
                None
            } else {
                symbols.get(symbol.parent)
            };
            symbol.flags & SYMBOL_PROGRAM == 0
                && match parent {
                    None => symbol.parent == NONE,
                    Some(parent) => parent.flags & SYMBOL_PROGRAM != 0 && parent.parent == NONE,
                }
        })
        .collect();
    // Program instances' variables first, each program's in the order of their places, then the
    // globals in theirs.
    let mut ordered = roots;
    ordered.sort_by_key(|(_, symbol)| {
        (
            if symbol.parent == NONE {
                u32::MAX
            } else {
                symbol.parent
            },
            symbol.area,
            symbol.offset,
        )
    });
    let mut walker = Walker {
        types: &types,
        visit,
        stopped: false,
    };
    for (index, symbol) in ordered {
        let name = image
            .string(symbol.display)
            .ok_or_else(|| super::values::damaged("string pool"))?;
        let (place, type_index) = symbol_place(image, index)?;
        walker.value(name, Located { place, type_index }, 0)?;
        if walker.stopped {
            break;
        }
    }
    Ok(())
}

struct Walker<'w, 'i> {
    types: &'w Types<'i>,
    visit: &'w mut dyn FnMut(&str, Located) -> Walk,
    stopped: bool,
}

impl Walker<'_, '_> {
    fn value(&mut self, path: &str, located: Located, depth: usize) -> Result<(), RuntimeError> {
        if self.stopped || depth > MAX_DEPTH {
            return Ok(());
        }
        let (_, record) = unaliased(self.types, located.type_index)?;
        match record.kind {
            kind::FUNCTION_BLOCK | kind::CLASS => match (self.visit)(path, located) {
                Walk::Stop => self.stopped = true,
                Walk::Skip => {}
                Walk::Descend => self.members(path, located, &record, depth)?,
            },
            kind::STRUCT | kind::UNION | kind::PROGRAM => {
                self.members(path, located, &record, depth)?;
            }
            kind::ARRAY => {
                let (element, dimensions) = self.types.array(&record)?;
                let (_, element_record) = unaliased(self.types, element)?;
                if !matches!(
                    element_record.kind,
                    kind::FUNCTION_BLOCK
                        | kind::CLASS
                        | kind::STRUCT
                        | kind::UNION
                        | kind::ARRAY
                ) {
                    return Ok(());
                }
                let bounds: Vec<(i64, i64)> = dimensions.collect();
                let stride = self.types.get(element)?.size;
                let mut index: Vec<i64> = bounds.iter().map(|(lower, _)| *lower).collect();
                let mut position: u32 = 0;
                if bounds.iter().any(|(lower, upper)| lower > upper) {
                    return Ok(());
                }
                loop {
                    let element_path = alloc::format!("{path}[{}]", indices_text(&index));
                    let at = position.checked_mul(stride).ok_or(RuntimeError::Overflow)?;
                    self.value(
                        &element_path,
                        Located {
                            place: located.place.at(at),
                            type_index: element,
                        },
                        depth + 1,
                    )?;
                    if self.stopped {
                        return Ok(());
                    }
                    position = position.checked_add(1).ok_or(RuntimeError::Overflow)?;
                    // The next index in row-major order: the last dimension moves fastest.
                    let mut dimension = index.len();
                    loop {
                        if dimension == 0 {
                            return Ok(());
                        }
                        dimension -= 1;
                        if index[dimension] < bounds[dimension].1 {
                            index[dimension] += 1;
                            break;
                        }
                        index[dimension] = bounds[dimension].0;
                    }
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn members(
        &mut self,
        path: &str,
        located: Located,
        record: &TypeRecord,
        depth: usize,
    ) -> Result<(), RuntimeError> {
        for detail in self.types.members(record)? {
            let name = self.types.string(detail.name)?;
            if name.starts_with("__") {
                continue;
            }
            let member_path = alloc::format!("{path}.{name}");
            self.value(
                &member_path,
                Located {
                    place: located.place.at(detail.offset),
                    type_index: detail.type_index,
                },
                depth + 1,
            )?;
            if self.stopped {
                break;
            }
        }
        Ok(())
    }
}
