use alloc::{string::String, vec::Vec};

use smol_str::SmolStr;

use crate::error::RuntimeError;
use crate::memory::layout::{ByteRange, LayoutRegion};
use crate::value::Value;

use super::{VmTrap, VM_MAX_OPERAND_STACK};

/// An array or a structure that lies, unchanged since a load pushed it, in a flat block
/// (specification 34 MEM-12; C1-10c). A store copies its bytes to a destination of its type, so an
/// assignment of a compound builds no value. Before a call that leaves it on the stack the dispatcher
/// replaces it with a copy of its bytes (C1-19b), and before any other instruction that pops it, or
/// that may write a block or call, with the value it stands for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Place {
    /// The compound's bytes.
    pub range: ByteRange,
    /// Its `TYPE_TABLE` type.
    pub type_id: u32,
}

/// A structure nested in the backing of a structure with a physical layout, which a store's
/// destination is (specification 34 MEM-12; C1-15): the cells and the type of the structure that
/// holds it, and the anchor and the depth that name it there.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlaceNode {
    /// The cells of the structure that holds the node.
    pub root: ByteRange,
    /// The type of the structure that holds the node.
    pub root_type: u32,
    /// A leaf or a member beside the backing below the node.
    pub anchor: u32,
    /// The number of the anchor's path segments that name the node.
    pub depth: u8,
}

/// Where a store copies a place: the destination's bytes and the presence byte of the declaration
/// they are, when it keeps one (MEM-12).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlaceDestination {
    /// The destination's bytes, as long as the place's.
    pub range: ByteRange,
    /// The block offset of the declaration's presence byte.
    pub presence: Option<u32>,
    /// The node of a backing the destination is, when it is one (C1-15).
    pub node: Option<PlaceNode>,
    /// Whether the value store stages the same write as a physical patch, which samples the
    /// deadline before it is published: a node of a backing, or a member beside one (C1-15).
    pub physical: bool,
}

/// A text that a standard text function made, held in the operand stack's text area (specification
/// 34 ARCH-05; C1-12): the UTF-8 of its characters, `len` bytes from byte `start`, and whether it
/// is a `WSTRING`. An instruction that pops it as a value reads it into one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct TextTemp {
    start: u32,
    len: u32,
    wide: bool,
}

impl TextTemp {
    /// Whether the text is a `WSTRING`.
    pub(crate) fn wide(self) -> bool {
        self.wide
    }
}

/// An enumeration as its type and the number of its variant (specification 34 MEM-12; C1-19a), the
/// enumeration cell of the flat store's design. A load of an enumeration leaf of the flat store and
/// a load of an enumeration constant push it, a store writes its number into the destination's leaf
/// and `=` and `<>` compare it, so none of them builds a value; before any other instruction that
/// pops it the dispatcher replaces it with the value it stands for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EnumTemp {
    /// The enumeration's `TYPE_TABLE` type, aliases resolved.
    pub type_id: u32,
    /// The number of the variant.
    pub value: i64,
}

/// The bytes of an array or a structure that a call between its load and its store could change,
/// copied into the operand stack's temporary area before the call (specification 34 MEM-12;
/// C1-19b): `len` bytes from byte `start`, a value of `type_id`. A store writes them to a
/// destination of its type; an instruction that pops it as a value reads it into one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct CopyTemp {
    start: u32,
    len: u32,
    type_id: u32,
}

impl CopyTemp {
    /// The `TYPE_TABLE` type of the value the copy holds.
    #[cfg(feature = "register-ir")]
    pub(crate) fn type_id(self) -> u32 {
        self.type_id
    }

    /// The place the copy was taken from as the checks of a store see it: a value of its type, as
    /// long as the copy, in the persistent block, where every place lies (C1-10c). The copy keeps
    /// no offset there.
    pub(crate) fn place(self) -> Place {
        Place {
            range: ByteRange {
                region: LayoutRegion::Persistent,
                start: 0,
                bytes: self.len,
            },
            type_id: self.type_id,
        }
    }
}

/// One operand: a value, a place on the flat store (C1-10c), a temporary text (C1-12), an
/// enumeration (C1-19a) or a copy (C1-19b).
#[derive(Debug)]
pub(crate) enum Operand {
    /// A value.
    Value(Value),
    /// A compound left where it lies.
    Place(Place),
    /// A text in the operand stack's temporary area.
    Text(TextTemp),
    /// An enumeration as its type and number.
    Enum(EnumTemp),
    /// A compound's bytes in the operand stack's temporary area.
    Copy(CopyTemp),
}

/// An operand holds a place in the room of a value, so the operand storage that construction
/// reserves and charges by values is unchanged (spec 34 MEM-03).
const _: () = assert!(core::mem::size_of::<Operand>() == core::mem::size_of::<Value>());

/// Bounded operand storage used by the shared interpreter and hosted tiers.
#[derive(Debug, Default)]
pub struct OperandStack {
    values: Vec<Operand>,
    floor: usize,
    /// The places among `values`, so that the dispatcher's check costs nothing while there are none.
    places: usize,
    /// The temporary texts among `values` (C1-12).
    texts: usize,
    /// The enumerations among `values` (C1-19a).
    enums: usize,
    /// The copies among `values` (C1-19b).
    copies: usize,
    /// The UTF-8 of the temporary texts and the bytes of the copies, in the order of their operands
    /// (C1-12, C1-19b). It keeps its room, so a text result or a copy allocates nothing once the
    /// engine is warm.
    temp_bytes: Vec<u8>,
}

impl OperandStack {
    /// Payload bytes held by the pre-reserved scratch backing.
    #[cfg(all(feature = "register-ir", any(test, feature = "test-support")))]
    pub fn storage_bytes(&self) -> usize {
        self.values
            .capacity()
            .saturating_mul(core::mem::size_of::<Value>())
    }

    /// Reserve storage for `capacity` operands across the whole activation tree at
    /// construction (spec 34 MEM-03). The per-activation limit and its refusal are unchanged.
    pub(super) fn reserve_storage(&mut self, capacity: usize) -> Result<(), VmTrap> {
        self.values
            .try_reserve_exact(capacity.saturating_sub(self.values.len()))
            .map_err(|_| VmTrap::BudgetExceeded)
    }

    pub(super) fn enter_call(&mut self) -> (usize, usize) {
        let previous = self.floor;
        self.floor = self.values.len();
        (self.floor, previous)
    }
    pub(super) fn leave_call(&mut self, base: usize, previous: usize) {
        self.truncate(base);
        self.floor = previous;
    }

    /// Drain the active frame's top `count` values in stack order, oldest first.
    ///
    /// If the active frame does not contain enough values, consume only its
    /// available values and leave the suspended caller prefix untouched. A place, an enumeration or
    /// a copy among them is refused: the dispatcher replaces each with its value before a call
    /// (C1-10c, C1-19a, C1-19b).
    pub(super) fn drain_top(
        &mut self,
        count: usize,
    ) -> Result<impl Iterator<Item = Value> + '_, VmTrap> {
        let available = self.values.len() - self.floor;
        if available < count {
            self.truncate(self.floor);
            return Err(VmTrap::StackUnderflow);
        }

        let start = self.values.len() - count;
        if self.values[start..].iter().any(|operand| {
            matches!(
                operand,
                Operand::Place(_) | Operand::Enum(_) | Operand::Copy(_)
            )
        }) {
            return Err(VmTrap::Runtime(RuntimeError::InvalidExecutionState));
        }
        // A temporary text is given to the call as its value (C1-12).
        for index in start..self.values.len() {
            if let Operand::Text(temp) = self.values[index] {
                self.values[index] = Operand::Value(text_value(&self.temp_bytes, temp)?);
                self.texts -= 1;
            }
        }
        self.forget_temps();
        Ok(self.values.drain(start..).map(|operand| match operand {
            Operand::Value(value) => value,
            // Refused or read above: the drained operands hold no place, no text, no enumeration
            // and no copy.
            Operand::Place(_) | Operand::Text(_) | Operand::Enum(_) | Operand::Copy(_) => {
                Value::Null
            }
        }))
    }

    /// Remove every value from the operand stack.
    #[inline]
    pub fn clear(&mut self) {
        self.values.clear();
        self.floor = 0;
        self.places = 0;
        self.texts = 0;
        self.enums = 0;
        self.copies = 0;
        self.temp_bytes.clear();
    }

    /// End the temporary area where the room of its topmost live temporary ends, the text or copy
    /// an operand stands for whose room ends last, so that the room of every temporary released
    /// above it comes back (C1-19c2). The area then never holds more than the temporaries that were
    /// alive when its topmost live one was pushed, so construction can reserve it.
    fn forget_temps(&mut self) {
        if self.texts == 0 && self.copies == 0 {
            self.temp_bytes.clear();
            return;
        }
        let end = self
            .values
            .iter()
            .filter_map(|operand| match operand {
                Operand::Text(temp) => Some(temp.start as usize + temp.len as usize),
                Operand::Copy(temp) => Some(temp.start as usize + temp.len as usize),
                _ => None,
            })
            .max()
            .unwrap_or(0);
        self.temp_bytes.truncate(end);
    }

    /// Room for `bytes` bytes of temporary texts and copies, so that no scan grows the area
    /// (spec 34 MEM-09; C1-19c2).
    pub(crate) fn reserve_temporaries(&mut self, bytes: usize) -> Result<(), VmTrap> {
        self.temp_bytes
            .try_reserve_exact(bytes.saturating_sub(self.temp_bytes.len()))
            .map_err(|_| VmTrap::BudgetExceeded)
    }

    /// Shorten the stack to `len` operands, counting the places, texts, enumerations and copies
    /// that go.
    fn truncate(&mut self, len: usize) {
        if self.places != 0 || self.texts != 0 || self.enums != 0 || self.copies != 0 {
            for operand in self.values.get(len..).unwrap_or_default() {
                match operand {
                    Operand::Place(_) => self.places -= 1,
                    Operand::Text(_) => self.texts -= 1,
                    Operand::Enum(_) => self.enums -= 1,
                    Operand::Copy(_) => self.copies -= 1,
                    Operand::Value(_) => {}
                }
            }
            self.forget_temps();
        }
        self.values.truncate(len);
    }

    /// Push one operand value, enforcing the VM stack limit.
    #[inline]
    pub fn push(&mut self, value: Value) -> Result<(), VmTrap> {
        self.push_operand(Operand::Value(value))
    }

    /// Push a place, enforcing the VM stack limit (C1-10c).
    pub(crate) fn push_place(&mut self, place: Place) -> Result<(), VmTrap> {
        self.push_operand(Operand::Place(place))?;
        self.places += 1;
        Ok(())
    }

    /// Push an enumeration as its type and number, enforcing the VM stack limit (C1-19a).
    pub(crate) fn push_enum(&mut self, temp: EnumTemp) -> Result<(), VmTrap> {
        self.push_operand(Operand::Enum(temp))?;
        self.enums += 1;
        Ok(())
    }

    /// Push `text`, a `WSTRING` when `wide`, into the temporary area as one operand, enforcing the
    /// VM stack limit (C1-12). The area is emptied first when no operand stands for a text or a copy
    /// in it.
    pub(crate) fn push_text(&mut self, text: &str, wide: bool) -> Result<(), VmTrap> {
        self.forget_temps();
        let start = self.temp_bytes.len();
        self.temp_bytes
            .try_reserve(text.len())
            .map_err(|_| VmTrap::BudgetExceeded)?;
        self.temp_bytes.extend_from_slice(text.as_bytes());
        self.push_text_at(start, wide)
    }

    /// Push the `WSTRING` whose UTF-16 code units are `units` into the temporary area as one operand,
    /// its characters as UTF-8 (C1-12, C1-19c4).
    ///
    /// # Errors
    ///
    /// [`VmTrap::Runtime`] with [`RuntimeError::InvalidExecutionState`] for units that are no
    /// UTF-16, and [`VmTrap::BudgetExceeded`] when the area cannot grow.
    pub(crate) fn push_wide_text(
        &mut self,
        units: impl Iterator<Item = u16> + Clone,
    ) -> Result<(), VmTrap> {
        self.forget_temps();
        let start = self.temp_bytes.len();
        let mut bytes = 0usize;
        for character in char::decode_utf16(units.clone()) {
            let character =
                character.map_err(|_| VmTrap::Runtime(RuntimeError::InvalidExecutionState))?;
            bytes += character.len_utf8();
        }
        self.temp_bytes
            .try_reserve(bytes)
            .map_err(|_| VmTrap::BudgetExceeded)?;
        for character in char::decode_utf16(units).flatten() {
            self.temp_bytes
                .extend_from_slice(character.encode_utf8(&mut [0; 4]).as_bytes());
        }
        self.push_text_at(start, true)
    }

    /// Push a second operand for the temporary text `temp`, its characters copied (C1-12).
    pub(crate) fn duplicate_text(&mut self, temp: TextTemp) -> Result<(), VmTrap> {
        let start = self.temp_bytes.len();
        let from = temp.start as usize;
        let to = from
            .checked_add(temp.len as usize)
            .filter(|end| *end <= start)
            .ok_or(VmTrap::Runtime(RuntimeError::InvalidExecutionState))?;
        self.temp_bytes
            .try_reserve(to - from)
            .map_err(|_| VmTrap::BudgetExceeded)?;
        self.temp_bytes.extend_from_within(from..to);
        self.push_text_at(start, temp.wide)
    }

    /// Push the operand of the text that starts at byte `start` of the text area and runs to its end.
    fn push_text_at(&mut self, start: usize, wide: bool) -> Result<(), VmTrap> {
        let text = TextTemp {
            start: u32::try_from(start).map_err(|_| VmTrap::Runtime(RuntimeError::Overflow))?,
            len: u32::try_from(self.temp_bytes.len() - start)
                .map_err(|_| VmTrap::Runtime(RuntimeError::Overflow))?,
            wide,
        };
        if let Err(error) = self.push_operand(Operand::Text(text)) {
            self.temp_bytes.truncate(start);
            return Err(error);
        }
        self.texts += 1;
        Ok(())
    }

    /// The characters of the temporary text `temp`. A popped text stays readable until its room is
    /// given back with [`Self::release_text`] or the next text is pushed (C1-12).
    pub(crate) fn text(&self, temp: TextTemp) -> Result<&str, VmTrap> {
        text_of(&self.temp_bytes, temp)
    }

    /// Give back the room of the popped temporary text `temp`: the area ends where the room of the
    /// topmost temporary still alive ends (C1-12, C1-19c2).
    pub(crate) fn release_text(&mut self, _temp: TextTemp) {
        self.forget_temps();
    }

    /// The bytes of the copy `temp`. A popped copy stays readable until its room is given back with
    /// [`Self::release_copy`] or the next text or copy is pushed (C1-19b).
    pub(crate) fn copy_bytes(&self, temp: CopyTemp) -> Result<&[u8], VmTrap> {
        let start = temp.start as usize;
        start
            .checked_add(temp.len as usize)
            .and_then(|end| self.temp_bytes.get(start..end))
            .ok_or(VmTrap::Runtime(RuntimeError::InvalidExecutionState))
    }

    /// Give back the room of the popped copy `temp`, as [`Self::release_text`] gives back a text's
    /// (C1-19b).
    pub(crate) fn release_copy(&mut self, _temp: CopyTemp) {
        self.forget_temps();
    }

    fn push_operand(&mut self, operand: Operand) -> Result<(), VmTrap> {
        if self.values.len() - self.floor >= VM_MAX_OPERAND_STACK {
            return Err(VmTrap::StackOverflow);
        }
        self.values.push(operand);
        Ok(())
    }

    /// Pop one operand value. A place is refused: the dispatcher replaces every place with its
    /// value before an instruction that pops one as a value (C1-10c). A temporary text is read into
    /// its value (C1-12).
    #[inline]
    pub fn pop(&mut self) -> Result<Value, VmTrap> {
        match self.pop_operand()? {
            Operand::Value(value) => Ok(value),
            // The dispatcher replaces an enumeration and a copy with its value first, as a place
            // (C1-19a, C1-19b).
            Operand::Place(_) | Operand::Enum(_) | Operand::Copy(_) => {
                Err(VmTrap::Runtime(RuntimeError::InvalidExecutionState))
            }
            Operand::Text(temp) => {
                let value = text_value(&self.temp_bytes, temp);
                self.release_text(temp);
                value
            }
        }
    }

    /// Pop one operand, a value, a place (C1-10c) or a temporary text (C1-12), whose characters
    /// stay readable until it is released.
    pub(crate) fn pop_operand(&mut self) -> Result<Operand, VmTrap> {
        if self.values.len() == self.floor {
            return Err(VmTrap::StackUnderflow);
        }
        let operand = self.values.pop().ok_or(VmTrap::StackUnderflow)?;
        match operand {
            Operand::Place(_) => self.places -= 1,
            Operand::Text(_) => self.texts -= 1,
            Operand::Enum(_) => self.enums -= 1,
            Operand::Copy(_) => self.copies -= 1,
            Operand::Value(_) => {}
        }
        Ok(operand)
    }

    /// The active frame's top `count` operands, oldest first, or `None` when it holds fewer
    /// (C1-12).
    pub(crate) fn top_operands(&self, count: usize) -> Option<&[Operand]> {
        let active = &self.values[self.floor..];
        active.get(active.len().checked_sub(count)?..)
    }

    /// Pop the active frame's top `count` operands and give back the room of their texts (C1-12).
    pub(crate) fn discard_top(&mut self, count: usize) -> Result<(), VmTrap> {
        let available = self.values.len() - self.floor;
        if available < count {
            return Err(VmTrap::StackUnderflow);
        }
        self.truncate(self.values.len() - count);
        Ok(())
    }

    #[cfg(test)]
    pub(super) fn storage_capacity(&self) -> usize {
        self.values.capacity()
    }

    /// Pop the right and left operands for a binary operation.
    #[inline]
    pub fn pop_pair(&mut self) -> Result<(Value, Value), VmTrap> {
        let right = self.pop()?;
        let left = self.pop()?;
        Ok((left, right))
    }

    /// Borrow the next value without allocating or changing the stack. A place is refused, as
    /// [`Self::pop`] refuses it.
    #[inline]
    pub fn peek(&self) -> Result<&Value, VmTrap> {
        match self.values[self.floor..].last() {
            Some(Operand::Value(value)) => Ok(value),
            Some(Operand::Place(_) | Operand::Text(_) | Operand::Enum(_) | Operand::Copy(_)) => {
                Err(VmTrap::Runtime(RuntimeError::InvalidExecutionState))
            }
            None => Err(VmTrap::StackUnderflow),
        }
    }

    /// The temporary text on top of the active frame's operands, when the top operand is one
    /// (C1-12).
    pub(crate) fn top_text(&self) -> Option<TextTemp> {
        match self.values[self.floor..].last() {
            Some(Operand::Text(temp)) => Some(*temp),
            _ => None,
        }
    }

    /// The enumeration on top of the active frame's operands, when the top operand is one
    /// (C1-19a).
    pub(crate) fn top_enum(&self) -> Option<EnumTemp> {
        match self.values[self.floor..].last() {
            Some(Operand::Enum(temp)) => Some(*temp),
            _ => None,
        }
    }

    /// Whether one of the active frame's top `count` operands is an enumeration (C1-19a).
    pub(crate) fn top_holds_enum(&self, count: usize) -> bool {
        self.enums != 0
            && self.values[self.floor..]
                .iter()
                .rev()
                .take(count)
                .any(|operand| matches!(operand, Operand::Enum(_)))
    }

    /// The enumerations on the stack (C1-19a).
    #[inline]
    pub(crate) fn enums(&self) -> usize {
        self.enums
    }

    /// Replace every enumeration on the stack with the value `value_of` gives for it, bottom first
    /// (C1-19a).
    ///
    /// # Errors
    ///
    /// The first error of `value_of`; the enumerations before it are values then, and the rest stay.
    pub(crate) fn materialize_enums(
        &mut self,
        mut value_of: impl FnMut(EnumTemp) -> Result<Value, RuntimeError>,
    ) -> Result<(), RuntimeError> {
        if self.enums == 0 {
            return Ok(());
        }
        for operand in &mut self.values {
            if let Operand::Enum(temp) = *operand {
                *operand = Operand::Value(value_of(temp)?);
                self.enums -= 1;
            }
        }
        Ok(())
    }

    /// The place on top of the active frame's operands, when the top operand is one (C1-10c).
    pub(crate) fn top_place(&self) -> Option<Place> {
        match self.values[self.floor..].last() {
            Some(Operand::Place(place)) => Some(*place),
            _ => None,
        }
    }

    /// Whether one of the active frame's top `count` operands is a place (C1-10c).
    pub(crate) fn top_holds_place(&self, count: usize) -> bool {
        self.places != 0
            && self.values[self.floor..]
                .iter()
                .rev()
                .take(count)
                .any(|operand| matches!(operand, Operand::Place(_)))
    }

    /// The places on the stack (C1-10c).
    #[inline]
    pub(crate) fn places(&self) -> usize {
        self.places
    }

    /// Replace every place on the stack with the value `value_of` reads for it, bottom first
    /// (C1-10c).
    ///
    /// # Errors
    ///
    /// The first error of `value_of`; the places before it are values then, and the rest stay.
    pub(crate) fn materialize_places(
        &mut self,
        mut value_of: impl FnMut(Place) -> Result<Value, RuntimeError>,
    ) -> Result<(), RuntimeError> {
        if self.places == 0 {
            return Ok(());
        }
        for operand in &mut self.values {
            if let Operand::Place(place) = *operand {
                *operand = Operand::Value(value_of(place)?);
                self.places -= 1;
            }
        }
        Ok(())
    }

    /// Replace each place that a call popping the active frame's top `own` operands leaves on the
    /// stack, and whose bytes `bytes_of` gives, with a copy of those bytes in the temporary area,
    /// bottom first (C1-19b). The call may write the bytes a place stands for; the copy keeps the
    /// bytes its load read. A place among the top `own` operands, and one `bytes_of` gives no bytes
    /// for, stays.
    ///
    /// # Errors
    ///
    /// [`VmTrap::BudgetExceeded`] when the area cannot grow; the places before it are copies then.
    pub(crate) fn detach_places<'b>(
        &mut self,
        own: usize,
        mut bytes_of: impl FnMut(Place) -> Option<&'b [u8]>,
    ) -> Result<(), VmTrap> {
        if self.places == 0 {
            return Ok(());
        }
        self.forget_temps();
        let end = self.values.len() - own.min(self.values.len() - self.floor);
        for operand in &mut self.values[..end] {
            let Operand::Place(place) = *operand else {
                continue;
            };
            let Some(bytes) = bytes_of(place) else {
                continue;
            };
            let start = self.temp_bytes.len();
            let copy = CopyTemp {
                start: u32::try_from(start).map_err(|_| VmTrap::Runtime(RuntimeError::Overflow))?,
                len: u32::try_from(bytes.len())
                    .map_err(|_| VmTrap::Runtime(RuntimeError::Overflow))?,
                type_id: place.type_id,
            };
            self.temp_bytes
                .try_reserve(bytes.len())
                .map_err(|_| VmTrap::BudgetExceeded)?;
            self.temp_bytes.extend_from_slice(bytes);
            *operand = Operand::Copy(copy);
            self.places -= 1;
            self.copies += 1;
        }
        Ok(())
    }

    /// Push a copy of `bytes`, a value of `type_id`, into the temporary area as one operand: a
    /// function's result, which the finished frame's bytes held (C1-19c4).
    pub(crate) fn push_copy(&mut self, bytes: &[u8], type_id: u32) -> Result<(), VmTrap> {
        self.forget_temps();
        let start = self.temp_bytes.len();
        let copy = CopyTemp {
            start: u32::try_from(start).map_err(|_| VmTrap::Runtime(RuntimeError::Overflow))?,
            len: u32::try_from(bytes.len()).map_err(|_| VmTrap::Runtime(RuntimeError::Overflow))?,
            type_id,
        };
        self.temp_bytes
            .try_reserve(bytes.len())
            .map_err(|_| VmTrap::BudgetExceeded)?;
        self.temp_bytes.extend_from_slice(bytes);
        if let Err(error) = self.push_operand(Operand::Copy(copy)) {
            self.temp_bytes.truncate(start);
            return Err(error);
        }
        self.copies += 1;
        Ok(())
    }

    /// The copies on the stack (C1-19b).
    #[inline]
    pub(crate) fn copies(&self) -> usize {
        self.copies
    }

    /// Whether one of the active frame's top `count` operands is a copy (C1-19b).
    pub(crate) fn top_holds_copy(&self, count: usize) -> bool {
        self.copies != 0
            && self.values[self.floor..]
                .iter()
                .rev()
                .take(count)
                .any(|operand| matches!(operand, Operand::Copy(_)))
    }

    /// Replace every copy among the active frame's operands with the value `value_of` reads from its
    /// bytes and its type, bottom first (C1-19b). A suspended caller's copies stay: no instruction of
    /// the callee pops them, and no write changes them.
    ///
    /// # Errors
    ///
    /// The first error of `value_of`; the copies before it are values then, and the rest stay.
    pub(crate) fn materialize_copies(
        &mut self,
        mut value_of: impl FnMut(&[u8], u32) -> Result<Value, RuntimeError>,
    ) -> Result<(), RuntimeError> {
        if self.copies == 0 {
            return Ok(());
        }
        for index in self.floor..self.values.len() {
            if let Operand::Copy(temp) = self.values[index] {
                let bytes = self
                    .copy_bytes(temp)
                    .map_err(|_| RuntimeError::InvalidExecutionState)?;
                let value = value_of(bytes, temp.type_id)?;
                self.values[index] = Operand::Value(value);
                self.copies -= 1;
            }
        }
        self.forget_temps();
        Ok(())
    }

    /// Duplicate the top operand value.
    #[inline]
    #[cfg(any(feature = "hir", test))]
    pub fn duplicate_top(&mut self) -> Result<(), VmTrap> {
        let value = self.peek()?.clone();
        self.push(value)
    }

    /// Swap the top two operands.
    #[inline]
    pub fn swap_top(&mut self) -> Result<(), VmTrap> {
        if self.values.len() - self.floor < 2 {
            return Err(VmTrap::StackUnderflow);
        }
        let len = self.values.len();
        self.values.swap(len - 1, len - 2);
        Ok(())
    }
}

/// The characters of the temporary text `temp` in the text area `bytes`.
fn text_of(bytes: &[u8], temp: TextTemp) -> Result<&str, VmTrap> {
    let start = temp.start as usize;
    start
        .checked_add(temp.len as usize)
        .and_then(|end| bytes.get(start..end))
        .and_then(|text| core::str::from_utf8(text).ok())
        .ok_or(VmTrap::Runtime(RuntimeError::InvalidExecutionState))
}

/// The value of the temporary text `temp`: a `STRING` or a `WSTRING` that holds its characters.
fn text_value(bytes: &[u8], temp: TextTemp) -> Result<Value, VmTrap> {
    let text = text_of(bytes, temp)?;
    Ok(if temp.wide {
        Value::WString(String::from(text))
    } else {
        Value::String(SmolStr::new(text))
    })
}

#[cfg(feature = "register-ir")]
mod registers;

#[cfg(test)]
mod continuation_tests {
    use super::*;
    #[test]
    fn deferred_calls_isolate_operands_and_discard_only_callee_residue() {
        let mut stack = OperandStack::default();
        stack.push(Value::Int(7)).unwrap();
        let outer = stack.enter_call();
        assert!(matches!(stack.pop(), Err(VmTrap::StackUnderflow)));
        assert!(matches!(stack.peek(), Err(VmTrap::StackUnderflow)));
        assert!(matches!(stack.swap_top(), Err(VmTrap::StackUnderflow)));
        stack.push(Value::Int(9)).unwrap();
        assert!(matches!(stack.swap_top(), Err(VmTrap::StackUnderflow)));
        let inner = stack.enter_call();
        stack.push(Value::Int(11)).unwrap();
        stack.push(Value::Int(13)).unwrap();
        stack.leave_call(inner.0, inner.1);
        assert_eq!(stack.pop().unwrap(), Value::Int(9));
        assert!(matches!(stack.pop(), Err(VmTrap::StackUnderflow)));
        stack.leave_call(outer.0, outer.1);
        assert_eq!(stack.pop().unwrap(), Value::Int(7));
    }
    #[test]
    fn operand_limit_is_per_callee_not_the_sum_of_suspended_operands() {
        let mut stack = OperandStack::default();
        for _ in 0..VM_MAX_OPERAND_STACK {
            stack.push(Value::Null).unwrap();
        }
        assert!(matches!(
            stack.push(Value::Null),
            Err(VmTrap::StackOverflow)
        ));
        let scope = stack.enter_call();
        for _ in 0..VM_MAX_OPERAND_STACK {
            stack.push(Value::Null).unwrap();
        }
        assert!(matches!(
            stack.push(Value::Null),
            Err(VmTrap::StackOverflow)
        ));
        stack.leave_call(scope.0, scope.1);
        assert!(matches!(
            stack.push(Value::Null),
            Err(VmTrap::StackOverflow)
        ));
        stack.clear();
        stack.push(Value::Int(1)).unwrap();
        assert_eq!(stack.pop().unwrap(), Value::Int(1));
    }
}

#[cfg(test)]
mod place_tests {
    use super::*;

    fn place(start: u32) -> Place {
        Place {
            range: ByteRange {
                region: LayoutRegion::Persistent,
                start,
                bytes: 8,
            },
            type_id: 3,
        }
    }

    /// C1-10c: an instruction that pops values never reads a place as one: `pop`, `peek` and a
    /// call's `drain_top` refuse it, and `pop_operand` gives it back whole. Catches a stack that
    /// hands a place to an operator or a call as some value.
    #[test]
    fn a_place_is_refused_where_a_value_is_popped() {
        let mut stack = OperandStack::default();
        stack.push(Value::Int(1)).unwrap();
        stack.push_place(place(8)).unwrap();
        assert!(matches!(
            stack.peek(),
            Err(VmTrap::Runtime(RuntimeError::InvalidExecutionState))
        ));
        assert!(matches!(
            stack.drain_top(2).map(|_| ()),
            Err(VmTrap::Runtime(RuntimeError::InvalidExecutionState))
        ));
        assert_eq!(stack.top_place(), Some(place(8)));
        assert!(stack.top_holds_place(1));
        assert!(matches!(stack.pop_operand(), Ok(Operand::Place(found)) if found == place(8)));
        assert_eq!(stack.places(), 0);
        assert!(!stack.top_holds_place(1));
        assert_eq!(stack.pop().unwrap(), Value::Int(1));
        stack.push_place(place(8)).unwrap();
        assert!(matches!(
            stack.pop(),
            Err(VmTrap::Runtime(RuntimeError::InvalidExecutionState))
        ));
    }

    /// C1-10c: every place becomes the value read for it, bottom first, and leaving a call takes the
    /// callee's places out of the count. Catches a count that keeps a dropped place, which would make
    /// the dispatcher materialize before every instruction, and a pass that misses a place below the
    /// top.
    #[test]
    fn places_become_values_and_leave_with_their_frame() {
        let mut stack = OperandStack::default();
        stack.push_place(place(0)).unwrap();
        stack.push(Value::Int(5)).unwrap();
        stack.push_place(place(16)).unwrap();
        assert_eq!(stack.places(), 2);
        let mut read = Vec::new();
        stack
            .materialize_places(|place| {
                read.push(place.range.start);
                Ok(Value::UDInt(place.range.start))
            })
            .unwrap();
        assert_eq!(read, [0, 16]);
        assert_eq!(stack.places(), 0);
        assert_eq!(stack.pop().unwrap(), Value::UDInt(16));
        let scope = stack.enter_call();
        stack.push_place(place(24)).unwrap();
        assert_eq!(stack.places(), 1);
        stack.leave_call(scope.0, scope.1);
        assert_eq!(stack.places(), 0);
        assert_eq!(stack.pop().unwrap(), Value::Int(5));
        assert_eq!(stack.pop().unwrap(), Value::UDInt(0));
    }
}

#[cfg(test)]
mod copy_tests {
    use super::*;

    fn place(start: u32) -> Place {
        Place {
            range: ByteRange {
                region: LayoutRegion::Persistent,
                start,
                bytes: 4,
            },
            type_id: 3,
        }
    }

    /// The bytes of `place` in `block`.
    fn bytes_in(block: &[u8], place: Place) -> Option<&[u8]> {
        let start = place.range.start as usize;
        block.get(start..start + place.range.bytes as usize)
    }

    /// The copy a pop gives.
    fn popped(stack: &mut OperandStack) -> CopyTemp {
        match stack.pop_operand().unwrap() {
            Operand::Copy(temp) => temp,
            other => panic!("a copy, not {other:?}"),
        }
    }

    /// Spec 34 MEM-12 (C1-19b): before a call that pops the top two operands, the place below them
    /// becomes a copy of the bytes it lies in, taken into the temporary area after a text, and the
    /// place among the call's operands stays for the call to read as its value. The copy keeps the
    /// bytes it took when the block changes after, a pop as a value and a call refuse it, and a store
    /// pops it and reads its bytes until it gives them back, after which the area ends where the copy
    /// began. Catches a copy taken of the call's own operand, a copy that reads the block when it is
    /// stored instead of when the call begins, and a copy handed to an operator or a call as some
    /// value.
    #[test]
    fn a_call_copies_the_places_it_leaves_on_the_stack() {
        let mut block = [1_u8, 2, 3, 4, 5, 6, 7, 8];
        let mut stack = OperandStack::default();
        stack.push_text("pump", false).unwrap();
        stack.push_place(place(0)).unwrap();
        stack.push(Value::Int(1)).unwrap();
        stack.push_place(place(4)).unwrap();
        stack
            .detach_places(2, |place| bytes_in(&block, place))
            .unwrap();
        assert_eq!((stack.places(), stack.copies()), (1, 1));
        block[0] = 9;
        assert_eq!(stack.top_place(), Some(place(4)));
        assert!(matches!(stack.pop_operand(), Ok(Operand::Place(found)) if found == place(4)));
        assert_eq!(stack.pop().unwrap(), Value::Int(1));
        assert!(stack.top_holds_copy(1));
        assert!(matches!(
            stack.peek(),
            Err(VmTrap::Runtime(RuntimeError::InvalidExecutionState))
        ));
        assert!(matches!(
            stack.drain_top(1).map(|_| ()),
            Err(VmTrap::Runtime(RuntimeError::InvalidExecutionState))
        ));
        let copy = popped(&mut stack);
        assert_eq!(stack.copies(), 0);
        assert_eq!(stack.copy_bytes(copy).unwrap(), [1, 2, 3, 4]);
        assert_eq!(copy.place().type_id, 3);
        assert_eq!(copy.place().range.bytes, 4);
        stack.release_copy(copy);
        assert_eq!(stack.temp_bytes.len(), 4);
        assert_eq!(stack.pop().unwrap(), Value::String("pump".into()));
        stack.push_place(place(0)).unwrap();
        stack
            .detach_places(0, |place| bytes_in(&block, place))
            .unwrap();
        assert!(matches!(
            stack.pop(),
            Err(VmTrap::Runtime(RuntimeError::InvalidExecutionState))
        ));
    }

    /// Spec 34 MEM-12 (C1-19b): a text pushed while a copy lives goes after it, so the area is not
    /// emptied under the copy; every copy of the active frame becomes the value read from its own
    /// bytes and its type, bottom first, while a suspended caller's copy stays for the caller's
    /// store; and leaving a call takes the callee's copies out of the count. Catches a text push that
    /// empties the area while only copies stand for it, a value read from another copy's bytes, a
    /// callee's instruction that turns its caller's copy into a value, and a count that keeps a
    /// dropped copy, which would make the dispatcher read copies before every instruction.
    #[test]
    fn copies_keep_their_bytes_beside_texts_and_become_values() {
        let block = [1_u8, 2, 3, 4, 5, 6, 7, 8];
        let mut stack = OperandStack::default();
        stack.push_place(place(4)).unwrap();
        stack.push_place(place(0)).unwrap();
        stack.push(Value::Int(1)).unwrap();
        stack
            .detach_places(1, |place| bytes_in(&block, place))
            .unwrap();
        stack.push_text("ab", false).unwrap();
        assert_eq!(stack.temp_bytes.len(), 10);
        assert_eq!(stack.pop().unwrap(), Value::String("ab".into()));
        let mut read = Vec::new();
        stack
            .materialize_copies(|bytes, type_id| {
                read.push((<[u8; 4]>::try_from(bytes).unwrap(), type_id));
                Ok(Value::UDInt(u32::from(bytes[0])))
            })
            .unwrap();
        assert_eq!(read, [([5, 6, 7, 8], 3), ([1, 2, 3, 4], 3)]);
        assert_eq!(stack.copies(), 0);
        assert_eq!(stack.pop().unwrap(), Value::Int(1));
        assert_eq!(stack.pop().unwrap(), Value::UDInt(1));
        assert_eq!(stack.pop().unwrap(), Value::UDInt(5));

        stack.push_place(place(4)).unwrap();
        stack
            .detach_places(0, |place| bytes_in(&block, place))
            .unwrap();
        let scope = stack.enter_call();
        stack.push_place(place(0)).unwrap();
        stack
            .detach_places(0, |place| bytes_in(&block, place))
            .unwrap();
        assert_eq!(stack.copies(), 2);
        let mut read = Vec::new();
        stack
            .materialize_copies(|bytes, _| {
                read.push(bytes[0]);
                Ok(Value::Null)
            })
            .unwrap();
        assert_eq!(read, [1]);
        stack.leave_call(scope.0, scope.1);
        assert_eq!(stack.copies(), 1);
        let copy = popped(&mut stack);
        assert_eq!(stack.copy_bytes(copy).unwrap(), [5, 6, 7, 8]);
        stack.release_copy(copy);
        assert_eq!(stack.copies(), 0);
        stack.push_text("x", false).unwrap();
        assert_eq!(stack.temp_bytes.len(), 1);
    }
}

#[cfg(test)]
mod text_tests {
    use super::*;

    /// The temporary text a pop gives.
    fn popped(stack: &mut OperandStack) -> TextTemp {
        match stack.pop_operand().unwrap() {
            Operand::Text(temp) => temp,
            other => panic!("a temporary text, not {other:?}"),
        }
    }

    /// Spec 34 ARCH-05 (C1-12): the text area is a stack. A popped text stays readable until it is
    /// given back; giving back the last text shrinks the area to where that text began, a text below
    /// another is given back with it only, and once no operand stands for a text the next push starts
    /// at the beginning, so the area keeps its room and grows only to the most texts alive at once.
    /// Catches a pop that frees the characters before the consumer reads them, a release that frees
    /// a text below another, and an area that grows with every text a scan makes.
    #[test]
    fn the_text_area_is_a_stack_that_keeps_its_room() {
        let mut stack = OperandStack::default();
        stack.push_text("pump", false).unwrap();
        stack.push_text("Ventil", true).unwrap();
        let top = popped(&mut stack);
        assert_eq!(stack.text(top).unwrap(), "Ventil");
        assert!(top.wide());
        stack.release_text(top);
        assert_eq!(stack.temp_bytes.len(), 4);
        stack.push_text("ab", false).unwrap();
        stack.swap_top().unwrap();
        let lower = popped(&mut stack);
        assert_eq!(stack.text(lower).unwrap(), "pump");
        stack.release_text(lower);
        assert_eq!(stack.temp_bytes.len(), 6);
        assert_eq!(stack.pop().unwrap(), Value::String("ab".into()));
        stack.push_text("x", false).unwrap();
        assert_eq!(stack.temp_bytes.len(), 1);
        stack.push_text("station", false).unwrap();
        let capacity = stack.temp_bytes.capacity();
        for _ in 0..100 {
            let temp = popped(&mut stack);
            stack.release_text(temp);
            stack.push_text("station", false).unwrap();
        }
        assert_eq!(stack.temp_bytes.len(), 8);
        assert_eq!(stack.temp_bytes.capacity(), capacity);
    }

    /// Spec 34 MEM-09 (C1-19c2): a call that takes texts as values gives their room back while an
    /// older text lives below them, so a loop that passes a text result to a call keeps the area at
    /// the room of the texts alive. A live "pump" takes 4 bytes; each of 100 rounds pushes
    /// "station", 7 bytes, and a call takes it: after each round the area holds 4 bytes. Catches a
    /// call that gives no room back while another temporary lives, whose area grows by 7 bytes a
    /// round, to 704 after 100, which no reservation bounds.
    #[test]
    fn a_call_gives_back_the_room_of_its_texts_above_a_live_text() {
        let mut stack = OperandStack::default();
        stack.push_text("pump", false).unwrap();
        for _ in 0..100 {
            stack.push_text("station", false).unwrap();
            let taken: Vec<Value> = stack.drain_top(1).unwrap().collect();
            assert_eq!(taken, [Value::String("station".into())]);
            assert_eq!(stack.temp_bytes.len(), 4);
        }
        assert_eq!(stack.pop().unwrap(), Value::String("pump".into()));
        assert!(stack.temp_bytes.is_empty());
    }

    /// Spec 34 ARCH-05 (C1-12): a second operand for a text copies its characters, and a call that
    /// binds values is given each text as its value, after which the area is empty. Catches a
    /// duplicate that shares the first text's room, which giving the duplicate back would free, and
    /// a call given a text's room instead of its value.
    #[test]
    fn a_duplicate_has_its_own_characters_and_a_call_takes_texts_as_values() {
        let mut stack = OperandStack::default();
        stack.push_text("pump", false).unwrap();
        let first = stack.top_text().unwrap();
        stack.duplicate_text(first).unwrap();
        let copy = popped(&mut stack);
        assert_eq!(stack.text(copy).unwrap(), "pump");
        stack.release_text(copy);
        assert_eq!(stack.text(first).unwrap(), "pump");
        stack.push(Value::Int(4)).unwrap();
        stack.push_text("Ventil", true).unwrap();
        let drained: Vec<Value> = stack.drain_top(3).unwrap().collect();
        assert_eq!(
            drained,
            [
                Value::String("pump".into()),
                Value::Int(4),
                Value::WString(String::from("Ventil")),
            ]
        );
        assert!(stack.temp_bytes.is_empty());
    }
}
