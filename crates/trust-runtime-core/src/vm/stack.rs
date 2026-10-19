use alloc::vec::Vec;

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

/// One operand: a value, or a place on the flat store (C1-10c).
#[derive(Debug)]
pub(crate) enum Operand {
    /// A value.
    Value(Value),
    /// A compound left where it lies.
    Place(Place),
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
        Ok(self.values.drain(start..).map(|operand| match operand {
            Operand::Value(value) => value,
            // Refused above: the drained operands hold no place.
            Operand::Place(_) => Value::Null,
        }))
    }

    /// Remove every value from the operand stack.
    #[inline]
    pub fn clear(&mut self) {
        self.values.clear();
        self.floor = 0;
        self.places = 0;
    }

    /// Shorten the stack to `len` operands, counting the places that go.
    fn truncate(&mut self, len: usize) {
        if self.places != 0 {
            let gone = self
                .values
                .get(len..)
                .unwrap_or_default()
                .iter()
                .filter(|operand| matches!(operand, Operand::Place(_)))
                .count();
            self.places -= gone;
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

    fn push_operand(&mut self, operand: Operand) -> Result<(), VmTrap> {
        if self.values.len() - self.floor >= VM_MAX_OPERAND_STACK {
            return Err(VmTrap::StackOverflow);
        }
        self.values.push(operand);
        Ok(())
    }

    /// Pop one operand value. A place is refused: the dispatcher replaces every place with its
    /// value before an instruction that pops one as a value (C1-10c).
    #[inline]
    pub fn pop(&mut self) -> Result<Value, VmTrap> {
        match self.pop_operand()? {
            Operand::Value(value) => Ok(value),
            Operand::Place(_) => Err(VmTrap::Runtime(RuntimeError::InvalidExecutionState)),
        }
    }

    /// Pop one operand, a value or a place (C1-10c).
    pub(crate) fn pop_operand(&mut self) -> Result<Operand, VmTrap> {
        if self.values.len() == self.floor {
            return Err(VmTrap::StackUnderflow);
        }
        let operand = self.values.pop().ok_or(VmTrap::StackUnderflow)?;
        if matches!(operand, Operand::Place(_)) {
            self.places -= 1;
        }
        Ok(operand)
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
            Some(Operand::Place(_)) => Err(VmTrap::Runtime(RuntimeError::InvalidExecutionState)),
            None => Err(VmTrap::StackUnderflow),
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
