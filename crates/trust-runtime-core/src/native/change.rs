//! The values an online change carries from the running compiled program into its replacement
//! (specification 44 §12.2; specification 11 §6.7.1). The device works the copy plan out from the
//! two images it holds, whose symbol and type tables name every variable with its type.
//!
//! The process images (`%I`, `%Q`, `%M`) are carried by address: the bytes both programs have. The
//! variables are carried by name, names compared without regard to case: each global of the new
//! program from the running program's global of the same name, and each variable of a program
//! instance from the running program's variable of the same name, the instances matched by their
//! names, from where it lay to where it lies, so that a `RETAIN` added or removed moves it between
//! the retain area and the globals or instance area. Each value is carried by its type, read from
//! each image's type table:
//!
//! - a value whose type has the same shape in both programs and holds nothing that a change names
//!   anew is copied as bytes, without building a value;
//! - an elementary value of a changed type is kept when the new type holds it, a text when the new
//!   capacity holds it uncut (`fit`);
//! - a structure keeps its members by name, but one whose members overlap, and a union, only into
//!   the same layout; an array keeps the elements at the indices both arrays have; a function block
//!   or class instance keeps its members by name, its base's among them, while its class id is
//!   always the new program's, a library block's own state goes only into an instance of the same
//!   block, and neither an in-out member nor a constant is carried;
//! - a reference, a pointer or an interface value names the same variable or instance in the new
//!   program, or it is `NULL` (`references`).
//!
//! Anything else keeps the new program's starting value. Each variable that does not go on with
//! the running value as it was is recorded as an event, at most `MAX_CARRIED_VALUE_EVENTS` of them
//! and then their count, with the variable's path as the interpreter names it (`P1.Speed`,
//! `Plant.motor.speed`, `Plant.table[3]`). The plan is worked out and applied in batches of whole
//! variables (`plan`).

use alloc::string::String;
use alloc::vec::Vec;
use core::fmt::Write as _;

use smol_str::SmolStr;
use trust_plc_abi::{
    AREA_GLOBALS, AREA_INPUTS, AREA_INSTANCES, AREA_MARKERS, AREA_OUTPUTS, AREA_RETAIN,
};

use super::image::{
    type_kind as kind, Image, SymbolRecord, Table, NONE, SYMBOL_CONSTANT, SYMBOL_IN_OUT,
    SYMBOL_PROGRAM,
};
use super::memory::ProgramMemory;
use super::values::{damaged, Types};
use crate::bytecode::BYTECODE_MAX_TYPE_NESTING;
use crate::error::RuntimeError;
use crate::vm::{CarriedValueEvent, MAX_CARRIED_VALUE_EVENTS};

mod fit;
mod plan;
mod references;
mod replace;
mod shape;

use plan::{Plan, Spot};
use shape::{
    each_member, find_member, is_instance, is_structure, overlapping, plain, resolve, same, Array,
    Member, Ty, CLASS, NOT_CARRIED,
};

/// Carries the running program's values from `old` into the program in `new`, whose areas hold its
/// starting memory and the work of its cold-init entry, by the rules of specification 44 §12.2
/// (specification 11 §6.7.1). Returns the events of the variables that did not go on with the
/// running value as it was: at most `MAX_CARRIED_VALUE_EVENTS`, then one `More` with their count.
///
/// # Errors
///
/// `RuntimeError::InvalidBundle` (through `values::damaged`) for a table of either image that does
/// not read as the build wrote it, or an area access outside an area. `old` is only read; `new` may
/// hold part of the values then, and the caller drops it.
pub fn carry(
    old: &dyn ProgramMemory,
    new: &mut dyn ProgramMemory,
) -> Result<Vec<CarriedValueEvent>, RuntimeError> {
    let running = Tables::of(old.image())?;
    process_images(&running, old, new)?;
    let mut carry = Carry::new()?;
    let count = Tables::of(new.image())?.symbols.len();
    let mut next = 0;
    while next < count {
        // The new program's tables lie in its memory, which lends them until the batch is worked
        // out; the batch is applied once they are given back.
        {
            let changed = Tables::of(new.image())?;
            let mut walk = Walk {
                running: &running,
                changed: &changed,
                memory: old,
                carry: &mut carry,
                path: Vec::new(),
            };
            while next < count && !walk.carry.plan.full() {
                walk.symbol(next)?;
                next += 1;
            }
        }
        carry.plan.apply(old, new)?;
    }
    Ok(carry.events.finish())
}

/// Carries the process images by address: the bytes both programs have, from the start of each
/// (specification 44 §12.2). A longer image of the new program keeps its starting bytes beyond.
fn process_images(
    running: &Tables<'_>,
    old: &dyn ProgramMemory,
    new: &mut dyn ProgramMemory,
) -> Result<(), RuntimeError> {
    for area in [AREA_INPUTS, AREA_OUTPUTS, AREA_MARKERS] {
        let held = running.image.area_length(area).unwrap_or(0);
        let length = held.min(new.image().area_length(area).unwrap_or(0));
        let length = u32::try_from(length).map_err(|_| damaged("area table"))?;
        let start = Spot { area, offset: 0 };
        plan::copy(old, start, new, start, length, "area table")?;
    }
    Ok(())
}

/// One program's image with the tables a change reads.
struct Tables<'i> {
    image: Image<'i>,
    types: Types<'i>,
    symbols: Table<'i, SymbolRecord>,
}

impl<'i> Tables<'i> {
    fn of(image: Image<'i>) -> Result<Self, RuntimeError> {
        Ok(Self {
            types: Types::of(&image)?,
            symbols: image
                .table::<SymbolRecord>()
                .map_err(|_| damaged("symbol table"))?,
            image,
        })
    }

    /// The symbol at `index`.
    fn symbol(&self, index: u32) -> Result<SymbolRecord, RuntimeError> {
        self.symbols
            .get(index)
            .ok_or_else(|| damaged("symbol table"))
    }

    /// The string at `offset` of the string pool.
    fn string(&self, offset: u32) -> Result<&'i str, RuntimeError> {
        self.image
            .string(offset)
            .ok_or_else(|| damaged("string pool"))
    }

    /// What `symbol` is to a change (specification 44 §5.7).
    fn role(&self, symbol: &SymbolRecord) -> Result<Role, RuntimeError> {
        if symbol.flags & SYMBOL_PROGRAM != 0 {
            return Ok(if symbol.parent == NONE {
                Role::Program
            } else {
                Role::Other
            });
        }
        if !matches!(symbol.area, AREA_GLOBALS | AREA_RETAIN | AREA_INSTANCES) {
            return Ok(Role::Other);
        }
        if symbol.parent == NONE {
            return Ok(if symbol.area == AREA_INSTANCES {
                Role::Other
            } else {
                Role::Global
            });
        }
        let parent = self.symbol(symbol.parent)?;
        Ok(
            if parent.flags & SYMBOL_PROGRAM != 0 && parent.parent == NONE {
                Role::Variable {
                    program: symbol.parent,
                }
            } else {
                Role::Other
            },
        )
    }

    /// The program instance whose canonical name is `canonical`.
    fn program(&self, canonical: &str) -> Option<SymbolRecord> {
        self.image
            .find_symbol(canonical)
            .map(|(_, symbol)| symbol)
            .filter(|symbol| symbol.flags & SYMBOL_PROGRAM != 0 && symbol.parent == NONE)
    }

    /// The variable whose canonical name is `canonical` that a change carries: a global when
    /// `global` is set, else a variable of a program instance.
    fn variable(
        &self,
        canonical: &str,
        global: bool,
    ) -> Result<Option<SymbolRecord>, RuntimeError> {
        let Some((_, symbol)) = self.image.find_symbol(canonical) else {
            return Ok(None);
        };
        let matches = match self.role(&symbol)? {
            Role::Global => global,
            Role::Variable { .. } => !global,
            Role::Program | Role::Other => false,
        };
        Ok((matches && carried(&symbol)).then_some(symbol))
    }
}

/// Whether a change carries the variable of `symbol`: it is neither an in-out, which every call
/// binds again, nor a constant, whose value is the new program's.
fn carried(symbol: &SymbolRecord) -> bool {
    symbol.flags & (SYMBOL_IN_OUT | SYMBOL_CONSTANT) == 0
}

/// What a symbol is to a change.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Role {
    /// A global: a variable without a parent in the globals or retain area.
    Global,
    /// The root of a program instance.
    Program,
    /// A variable of the program instance whose root is the symbol at `program`.
    Variable { program: u32 },
    /// A member of a variable, which its variable's walk reaches, a POU or a frame variable.
    Other,
}

/// A value of one of the programs: where it lies and its type.
#[derive(Clone, Copy)]
struct Node {
    area: u8,
    offset: u32,
    ty: u32,
}

impl Node {
    /// The variable of `symbol`.
    fn of(symbol: &SymbolRecord) -> Self {
        Self {
            area: symbol.area,
            offset: symbol.offset,
            ty: symbol.type_index,
        }
    }

    /// Where the value lies.
    fn spot(self) -> Spot {
        Spot {
            area: self.area,
            offset: self.offset,
        }
    }

    /// The member `member` of this value.
    fn member(self, member: &Member<'_>) -> Self {
        Self {
            area: self.area,
            offset: member.offset,
            ty: member.ty.index,
        }
    }

    /// The element at flat `position` of this array, whose elements are of type `element`.
    fn element(self, element: &Ty, position: u64) -> Result<Self, RuntimeError> {
        let offset = position
            .checked_mul(u64::from(element.size()))
            .and_then(|bytes| bytes.checked_add(u64::from(self.offset)))
            .and_then(|offset| u32::try_from(offset).ok())
            .ok_or_else(|| damaged("type table"))?;
        Ok(Self {
            area: self.area,
            offset,
            ty: element.index,
        })
    }
}

/// One step of a variable's path.
#[derive(Clone, Copy)]
enum Segment<'n> {
    Name(&'n str),
    Index(i64),
}

/// What happened to one variable's value, for its event.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    New,
    Converted,
    NotKept,
    Cleared,
}

/// The events of one change: at most `MAX_CARRIED_VALUE_EVENTS`, then their count. While the
/// elements of an array whose element type changed are carried, their events are muted and the
/// array records one, so that one variable takes one event, whatever its length.
struct Events {
    list: Vec<CarriedValueEvent>,
    more: usize,
    /// How many arrays the elements being carried belong to whose element type changed.
    muted: usize,
    /// Whether a muted element did not keep its value or lost its reference.
    lost: bool,
}

impl Events {
    fn new() -> Result<Self, RuntimeError> {
        let mut list = Vec::new();
        // Bookkeeping of the change, reserved once.
        list.try_reserve_exact(MAX_CARRIED_VALUE_EVENTS + 1)
            .map_err(|_| RuntimeError::Overflow)?;
        Ok(Self {
            list,
            more: 0,
            muted: 0,
            lost: false,
        })
    }

    fn finish(mut self) -> Vec<CarriedValueEvent> {
        if self.more > 0 {
            self.list.push(CarriedValueEvent::More { count: self.more });
        }
        self.list
    }
}

/// What a change keeps across its batches: the events, the plan of the batch and, from the first
/// reference on, the running program's top-level variables by place.
struct Carry {
    events: Events,
    plan: Plan,
    roots: Option<Vec<references::Root>>,
}

impl Carry {
    fn new() -> Result<Self, RuntimeError> {
        Ok(Self {
            events: Events::new()?,
            plan: Plan::new()?,
            roots: None,
        })
    }
}

/// One batch of a change: both programs' tables, the running program's memory, what the change
/// keeps and the path of the variable being carried.
struct Walk<'w, 'o, 'n> {
    running: &'w Tables<'o>,
    changed: &'w Tables<'n>,
    memory: &'w dyn ProgramMemory,
    carry: &'w mut Carry,
    path: Vec<Segment<'n>>,
}

impl Walk<'_, '_, '_> {
    /// The new program's symbol `index`, when it is a variable a change carries: a global, or a
    /// variable of a program instance whose instance the running program has. A program instance
    /// the running program does not have records one event, for its variables too.
    fn symbol(&mut self, index: u32) -> Result<(), RuntimeError> {
        let (running, changed) = (self.running, self.changed);
        let symbol = changed.symbol(index)?;
        match changed.role(&symbol)? {
            Role::Program => {
                if running.program(changed.string(symbol.canonical)?).is_none() {
                    let name = changed.string(symbol.display)?;
                    self.record(Kind::New, Some(Segment::Name(name)));
                }
                Ok(())
            }
            Role::Global if carried(&symbol) => {
                let counterpart = running.variable(changed.string(symbol.canonical)?, true)?;
                self.variable(&symbol, counterpart)
            }
            Role::Variable { program } if carried(&symbol) => {
                let root = changed.symbol(program)?;
                if running.program(changed.string(root.canonical)?).is_none() {
                    return Ok(());
                }
                let counterpart = running.variable(changed.string(symbol.canonical)?, false)?;
                self.variable(&symbol, counterpart)
            }
            Role::Global | Role::Variable { .. } | Role::Other => Ok(()),
        }
    }

    /// The new program's variable of `symbol` from `counterpart`, the running program's variable of
    /// the same name, or new.
    fn variable(
        &mut self,
        symbol: &SymbolRecord,
        counterpart: Option<SymbolRecord>,
    ) -> Result<(), RuntimeError> {
        let name = self.changed.string(symbol.display)?;
        let Some(counterpart) = counterpart else {
            self.record(Kind::New, Some(Segment::Name(name)));
            return Ok(());
        };
        self.path.push(Segment::Name(name));
        self.value(Node::of(&counterpart), Node::of(symbol), 0)?;
        self.path.pop();
        Ok(())
    }

    /// The running program's value at `old` into the new program's place `new`, by their types.
    fn value(&mut self, old: Node, new: Node, depth: usize) -> Result<(), RuntimeError> {
        if depth > BYTECODE_MAX_TYPE_NESTING {
            return Err(damaged("type table"));
        }
        let (running, changed) = (self.running, self.changed);
        let old_ty = resolve(&running.types, old.ty)?;
        let new_ty = resolve(&changed.types, new.ty)?;
        let unchanged = same(&running.types, old_ty.index, &changed.types, new_ty.index)?;
        if unchanged && plain(&changed.types, new_ty.index)? {
            return self.carry.plan.copy(old.spot(), new.spot(), new_ty.size());
        }
        match (old_ty.kind(), new_ty.kind()) {
            (kind::REFERENCE, kind::REFERENCE)
            | (kind::POINTER, kind::POINTER)
            | (kind::INTERFACE, kind::INTERFACE) => self.reference(old, &old_ty, new, &new_ty),
            (from, to) if is_instance(from) && is_instance(to) => {
                self.members(old, &old_ty, new, &new_ty, depth)
            }
            (from, to) if is_structure(from) && is_structure(to) => {
                // A layout with shared bytes is kept only as it is (specification 44 §12.2).
                if !unchanged
                    && (overlapping(&running.types, &old_ty)?
                        || overlapping(&changed.types, &new_ty)?)
                {
                    self.record(Kind::NotKept, None);
                    return Ok(());
                }
                self.members(old, &old_ty, new, &new_ty, depth)
            }
            (kind::ARRAY, kind::ARRAY) => self.elements(old, &old_ty, new, &new_ty, depth),
            _ => self.elementary(old, &old_ty, new, &new_ty),
        }
    }

    /// The members of a structure or an instance, by name. A member the running program's value
    /// does not have keeps its starting value and records `New`. An instance's class id is always
    /// the new program's, and an in-out member and a constant are not carried. A library block's
    /// own state, its other `__` members, goes only into an instance of the same block and records
    /// nothing, as no program declares it.
    fn members(
        &mut self,
        old: Node,
        old_ty: &Ty,
        new: Node,
        new_ty: &Ty,
        depth: usize,
    ) -> Result<(), RuntimeError> {
        let (running, changed) = (self.running, self.changed);
        each_member(&changed.types, new_ty, new.offset, &mut |member| {
            if member.name.eq_ignore_ascii_case(CLASS) || member.flags & NOT_CARRIED != 0 {
                return Ok(());
            }
            let counterpart = find_member(&running.types, old_ty, old.offset, &mut |candidate| {
                Ok(candidate.flags & NOT_CARRIED == 0
                    && candidate.name.eq_ignore_ascii_case(member.name))
            })?;
            if member.name.starts_with("__") {
                if let Some(from) = counterpart
                    && from.owner.eq_ignore_ascii_case(member.owner)
                    && same(
                        &running.types,
                        from.ty.index,
                        &changed.types,
                        member.ty.index,
                    )?
                {
                    self.value(old.member(&from), new.member(&member), depth + 1)?;
                }
                return Ok(());
            }
            match counterpart {
                Some(from) => {
                    self.path.push(Segment::Name(member.name));
                    self.value(old.member(&from), new.member(&member), depth + 1)?;
                    self.path.pop();
                }
                None => self.record(Kind::New, Some(Segment::Name(member.name))),
            }
            Ok(())
        })
    }

    /// The elements of an array at the indices both arrays have; the others keep their starting
    /// values. Arrays of one dimension whose elements are carried as bytes are copied as one run.
    /// When the element type changed, the elements record nothing and the array records one event.
    fn elements(
        &mut self,
        old: Node,
        old_ty: &Ty,
        new: Node,
        new_ty: &Ty,
        depth: usize,
    ) -> Result<(), RuntimeError> {
        let (running, changed) = (self.running, self.changed);
        let from = Array::of(&running.types, old_ty)?;
        let to = Array::of(&changed.types, new_ty)?;
        if from.dims.len() != to.dims.len() {
            self.record(Kind::NotKept, None);
            return Ok(());
        }
        let unchanged = same(
            &running.types,
            from.element.index,
            &changed.types,
            to.element.index,
        )?;
        if to.dims.len() == 1 && unchanged && plain(&changed.types, to.element.index)? {
            return self.run(old, &from, new, &to);
        }
        let mut indices = Vec::new();
        reserve(&mut indices, to.dims.len())?;
        indices.extend(to.dims.iter().map(|&(lower, _)| lower));
        let mut dropped = false;
        let outer = core::mem::take(&mut self.carry.events.lost);
        if !unchanged {
            self.carry.events.muted += 1;
        }
        loop {
            let target = to.position(&indices).ok_or_else(|| damaged("type table"))?;
            match from.position(&indices) {
                Some(position) => {
                    let single = match *indices.as_slice() {
                        [index] => Some(index),
                        _ => None,
                    };
                    if let Some(index) = single {
                        self.path.push(Segment::Index(index));
                    }
                    self.value(
                        old.element(&from.element, position)?,
                        new.element(&to.element, target)?,
                        depth + 1,
                    )?;
                    if single.is_some() {
                        self.path.pop();
                    }
                }
                None => dropped = true,
            }
            if !to.step(&mut indices) {
                break;
            }
        }
        if !unchanged {
            self.carry.events.muted -= 1;
        }
        let lost = core::mem::replace(&mut self.carry.events.lost, outer);
        if unchanged {
            // A loss among elements of an unchanged type is one under an outer array's muting,
            // which that array records.
            self.carry.events.lost |= lost;
            if dropped || from.count != to.count {
                self.record(Kind::Converted, None);
            }
        } else {
            self.record(if lost { Kind::NotKept } else { Kind::Converted }, None);
        }
        Ok(())
    }

    /// The elements both arrays of one dimension and of the same element type have, copied as one
    /// run; the array records `Converted` when its bounds changed, and `NotKept` when the two have
    /// no index in common.
    fn run(&mut self, old: Node, from: &Array, new: Node, to: &Array) -> Result<(), RuntimeError> {
        let (Some(&(old_lower, old_upper)), Some(&(new_lower, new_upper))) =
            (from.dims.first(), to.dims.first())
        else {
            return Err(damaged("type table"));
        };
        let (lower, upper) = (old_lower.max(new_lower), old_upper.min(new_upper));
        if lower > upper {
            self.record(Kind::NotKept, None);
            return Ok(());
        }
        let skip = |first: i64| {
            lower
                .checked_sub(first)
                .and_then(|skip| u64::try_from(skip).ok())
                .ok_or_else(|| damaged("type table"))
        };
        let source = old.element(&from.element, skip(old_lower)?)?;
        let target = new.element(&to.element, skip(new_lower)?)?;
        let length = upper
            .checked_sub(lower)
            .and_then(|span| span.checked_add(1))
            .and_then(|count| u64::try_from(count).ok())
            .and_then(|count| count.checked_mul(u64::from(to.element.size())))
            .and_then(|bytes| u32::try_from(bytes).ok())
            .ok_or_else(|| damaged("type table"))?;
        self.carry.plan.copy(source.spot(), target.spot(), length)?;
        if (lower, upper) != (old_lower, old_upper) || (lower, upper) != (new_lower, new_upper) {
            self.record(Kind::Converted, None);
        }
        Ok(())
    }

    /// An elementary value of a changed type: converted when the new type holds it, which records
    /// `Converted`, else the new program's starting value, which records `NotKept`.
    fn elementary(
        &mut self,
        old: Node,
        old_ty: &Ty,
        new: Node,
        new_ty: &Ty,
    ) -> Result<(), RuntimeError> {
        let (running, changed) = (self.running, self.changed);
        let (Some(from), Some(to)) = (
            fit::elementary(&running.types, old_ty)?,
            fit::elementary(&changed.types, new_ty)?,
        ) else {
            self.record(Kind::NotKept, None);
            return Ok(());
        };
        let held = fit::read(self.memory, old.spot(), &from, &running.types)?;
        let room = new_ty.size() as usize;
        let kept = self.carry.plan.write_with(new.spot(), room, |out| {
            fit::fit(&held, &to, &changed.types, out)
        })?;
        self.record(if kept { Kind::Converted } else { Kind::NotKept }, None);
        Ok(())
    }

    /// Records what happened to the variable being carried, or to its member `last`; while the
    /// elements of an array whose element type changed are carried, only whether one was lost.
    fn record(&mut self, kind: Kind, last: Option<Segment<'_>>) {
        let events = &mut self.carry.events;
        if events.muted > 0 {
            events.lost |= matches!(kind, Kind::NotKept | Kind::Cleared);
            return;
        }
        if events.list.len() >= MAX_CARRIED_VALUE_EVENTS {
            events.more = events.more.saturating_add(1);
            return;
        }
        let name = path_name(&self.path, last);
        events.list.push(match kind {
            Kind::New => CarriedValueEvent::New { name },
            Kind::Converted => CarriedValueEvent::Converted { name },
            Kind::NotKept => CarriedValueEvent::NotKept { name },
            Kind::Cleared => CarriedValueEvent::Cleared { name },
        });
    }
}

/// The path `path`, with `last` after it when given, as the interpreter names a variable:
/// `Plant.motor.speed`, `Plant.table[3]`.
fn path_name(path: &[Segment<'_>], last: Option<Segment<'_>>) -> SmolStr {
    let mut text = String::new();
    for segment in path.iter().copied().chain(last) {
        match segment {
            Segment::Name(name) => {
                if !text.is_empty() {
                    text.push('.');
                }
                text.push_str(name);
            }
            Segment::Index(index) => {
                let _ = write!(text, "[{index}]");
            }
        }
    }
    SmolStr::new(text)
}

/// Makes room for `more` items in `list`: the bookkeeping of a change grows with the program, and a
/// device's heap may not hold it, which fails the change with `RuntimeError::Overflow`.
fn reserve<T>(list: &mut Vec<T>, more: usize) -> Result<(), RuntimeError> {
    list.try_reserve(more).map_err(|_| RuntimeError::Overflow)
}

#[cfg(test)]
mod tests;
