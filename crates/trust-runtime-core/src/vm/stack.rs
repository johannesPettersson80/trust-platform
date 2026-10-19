use alloc::{string::String, vec::Vec};

use smol_str::SmolStr;

use crate::error::RuntimeError;
use crate::memory::layout::ByteRange;
use crate::value::Value;

use super::{VmTrap, VM_MAX_OPERAND_STACK};

/// An array or a structure that lies, unchanged since a load pushed it, in a flat block
/// (specification 34 MEM-12; C1-10c). A store copies its bytes to a destination of its type, so an
/// assignment of a compound builds no value. Before any other instruction that pops it, or that may
/// write a block or call, the dispatcher replaces it with the value it stands for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Place {
    /// The compound's bytes.
    pub range: ByteRange,
    /// Its `TYPE_TABLE` type.
    pub type_id: u32,
}

/// Where a store copies a place: the destination's bytes and the presence byte of the declaration
/// they are, when it keeps one (MEM-12).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlaceDestination {
    /// The destination's bytes, as long as the place's.
    pub range: ByteRange,
    /// The block offset of the declaration's presence byte.
    pub presence: Option<u32>,
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

/// One operand: a value, a place on the flat store (C1-10c) or a temporary text (C1-12).
#[derive(Debug)]
pub(crate) enum Operand {
    /// A value.
    Value(Value),
    /// A compound left where it lies.
    Place(Place),
    /// A text in the operand stack's text area.
    Text(TextTemp),
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
    /// The UTF-8 of the temporary texts, in the order of their operands (C1-12). It keeps its room,
    /// so a text result allocates nothing once the engine is warm.
    text_bytes: Vec<u8>,
}

impl OperandStack {
    /// Payload bytes held by the pre-reserved scratch backing.
    #[cfg(all(feature = "register-ir", any(test, feature = "test-support")))]
    pub fn storage_bytes(&self) -> usize {
        self.values
            .capacity()
            .saturating_mul(core::mem::size_of::<Value>())
    }
    /// Reserve native operand scratch during controller construction.
    #[cfg(feature = "register-ir")]
    pub fn try_reserve_values(&mut self, capacity: usize) -> Result<(), VmTrap> {
        if capacity > VM_MAX_OPERAND_STACK {
            return Err(VmTrap::StackOverflow);
        }
        self.values
            .try_reserve_exact(capacity.saturating_sub(self.values.len()))
            .map_err(|_| VmTrap::Runtime(crate::error::RuntimeError::PreparationLimit))
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
    /// available values and leave the suspended caller prefix untouched. A place among them is
    /// refused: the dispatcher replaces every place with its value before a call (C1-10c).
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
        if self.values[start..]
            .iter()
            .any(|operand| matches!(operand, Operand::Place(_)))
        {
            return Err(VmTrap::Runtime(RuntimeError::InvalidExecutionState));
        }
        // A temporary text is given to the call as its value (C1-12).
        for index in start..self.values.len() {
            if let Operand::Text(temp) = self.values[index] {
                self.values[index] = Operand::Value(text_value(&self.text_bytes, temp)?);
                self.texts -= 1;
            }
        }
        if self.texts == 0 {
            self.text_bytes.clear();
        }
        Ok(self.values.drain(start..).map(|operand| match operand {
            Operand::Value(value) => value,
            // Refused or read above: the drained operands hold no place and no text.
            Operand::Place(_) | Operand::Text(_) => Value::Null,
        }))
    }

    /// Remove every value from the operand stack.
    #[inline]
    pub fn clear(&mut self) {
        self.values.clear();
        self.floor = 0;
        self.places = 0;
        self.texts = 0;
        self.text_bytes.clear();
    }

    /// Shorten the stack to `len` operands, counting the places and the texts that go.
    fn truncate(&mut self, len: usize) {
        if self.places != 0 || self.texts != 0 {
            for operand in self.values.get(len..).unwrap_or_default() {
                match operand {
                    Operand::Place(_) => self.places -= 1,
                    Operand::Text(_) => self.texts -= 1,
                    Operand::Value(_) => {}
                }
            }
            if self.texts == 0 {
                self.text_bytes.clear();
            }
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

    /// Push `text`, a `WSTRING` when `wide`, into the text area as one operand, enforcing the VM
    /// stack limit (C1-12). The area is emptied first when it holds no text an operand stands for.
    pub(crate) fn push_text(&mut self, text: &str, wide: bool) -> Result<(), VmTrap> {
        if self.texts == 0 {
            self.text_bytes.clear();
        }
        let start = self.text_bytes.len();
        self.text_bytes
            .try_reserve(text.len())
            .map_err(|_| VmTrap::BudgetExceeded)?;
        self.text_bytes.extend_from_slice(text.as_bytes());
        self.push_text_at(start, wide)
    }

    /// Push a second operand for the temporary text `temp`, its characters copied (C1-12).
    pub(crate) fn duplicate_text(&mut self, temp: TextTemp) -> Result<(), VmTrap> {
        let start = self.text_bytes.len();
        let from = temp.start as usize;
        let to = from
            .checked_add(temp.len as usize)
            .filter(|end| *end <= start)
            .ok_or(VmTrap::Runtime(RuntimeError::InvalidExecutionState))?;
        self.text_bytes
            .try_reserve(to - from)
            .map_err(|_| VmTrap::BudgetExceeded)?;
        self.text_bytes.extend_from_within(from..to);
        self.push_text_at(start, temp.wide)
    }

    /// Push the operand of the text that starts at byte `start` of the text area and runs to its end.
    fn push_text_at(&mut self, start: usize, wide: bool) -> Result<(), VmTrap> {
        let text = TextTemp {
            start: u32::try_from(start).map_err(|_| VmTrap::Runtime(RuntimeError::Overflow))?,
            len: u32::try_from(self.text_bytes.len() - start)
                .map_err(|_| VmTrap::Runtime(RuntimeError::Overflow))?,
            wide,
        };
        if let Err(error) = self.push_operand(Operand::Text(text)) {
            self.text_bytes.truncate(start);
            return Err(error);
        }
        self.texts += 1;
        Ok(())
    }

    /// The characters of the temporary text `temp`. A popped text stays readable until its room is
    /// given back with [`Self::release_text`] or the next text is pushed (C1-12).
    pub(crate) fn text(&self, temp: TextTemp) -> Result<&str, VmTrap> {
        text_of(&self.text_bytes, temp)
    }

    /// Give back the room of the popped temporary text `temp`: the area ends where it began when no
    /// text follows it (C1-12).
    pub(crate) fn release_text(&mut self, temp: TextTemp) {
        if temp.start as usize + temp.len as usize == self.text_bytes.len() {
            self.text_bytes.truncate(temp.start as usize);
        }
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
            Operand::Place(_) => Err(VmTrap::Runtime(RuntimeError::InvalidExecutionState)),
            Operand::Text(temp) => {
                let value = text_value(&self.text_bytes, temp);
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
            Some(Operand::Place(_) | Operand::Text(_)) => {
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
    use crate::memory::layout::LayoutRegion;

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
        assert_eq!(stack.text_bytes.len(), 4);
        stack.push_text("ab", false).unwrap();
        stack.swap_top().unwrap();
        let lower = popped(&mut stack);
        assert_eq!(stack.text(lower).unwrap(), "pump");
        stack.release_text(lower);
        assert_eq!(stack.text_bytes.len(), 6);
        assert_eq!(stack.pop().unwrap(), Value::String("ab".into()));
        stack.push_text("x", false).unwrap();
        assert_eq!(stack.text_bytes.len(), 1);
        stack.push_text("station", false).unwrap();
        let capacity = stack.text_bytes.capacity();
        for _ in 0..100 {
            let temp = popped(&mut stack);
            stack.release_text(temp);
            stack.push_text("station", false).unwrap();
        }
        assert_eq!(stack.text_bytes.len(), 8);
        assert_eq!(stack.text_bytes.capacity(), capacity);
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
        assert!(stack.text_bytes.is_empty());
    }
}
