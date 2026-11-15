//! In-outs and references of Rust POUs (specification 48 RU-29).
//!
//! A function's in-out is the caller's variable itself, [`InOut`], borrowed for the call. A
//! function block's in-out keeps the reference of its last call, and a `REF_TO` variable or input
//! holds one: a [`Ref`], which names a variable in the program's areas and reads and writes it with
//! the checks Structured Text's dereference makes (specification 44 §6.5, §7.3).

use core::marker::PhantomData;

use trust_plc_abi::{module, Plain, RefValue};

use crate::{Cx, Fault, Placed};

/// A function's or a method's in-out of type `T`: the caller's variable itself, for the length of
/// the call (IEC §6.6.1.2.2). A write is made at once, as Structured Text's is.
///
/// A unit test gives one a variable's place of its own: `InOut::new(&Cell::new(2.5))` for an
/// `LREAL`.
pub struct InOut<'a, T: Placed> {
    place: &'a T::Area,
}

impl<'a, T: Placed + Copy> InOut<'a, T> {
    /// The in-out of the variable that lies in `place`.
    #[must_use]
    pub fn new(place: &'a T::Area) -> Self {
        Self { place }
    }

    /// The variable's value.
    #[must_use]
    pub fn get(&self) -> T {
        T::load(self.place)
    }

    /// Writes `value` into the variable; a string is cut to the variable's capacity (specification
    /// 02 §6).
    pub fn set(&self, value: T) {
        value.store(self.place);
    }
}

/// A reference to a variable of type `T` (`REF_TO`, specification 44 §7.3), or `NULL`: a function
/// block's in-out (`#[in_out]`), the reference of its last call, or a `REF_TO` variable or input.
/// It is copied and compared as Structured Text's references are.
pub struct Ref<T> {
    value: RefValue,
    target: PhantomData<fn() -> T>,
}

impl<T> Clone for Ref<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> Copy for Ref<T> {}

impl<T> PartialEq for Ref<T> {
    fn eq(&self, other: &Self) -> bool {
        self.value == other.value
    }
}

impl<T> Eq for Ref<T> {}

impl<T> core::fmt::Debug for Ref<T> {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.debug_tuple("Ref").field(&self.value).finish()
    }
}

impl<T> Ref<T> {
    /// `NULL`, the reference to nothing, which a reference starts at.
    pub const NULL: Self = Self {
        value: RefValue::NULL_REF,
        target: PhantomData,
    };

    /// Whether it is `NULL`.
    #[must_use]
    pub fn is_null(&self) -> bool {
        self.value.kind == RefValue::NULL
    }
}

impl<T: Placed + Copy> Ref<T>
where
    T::Area: Plain,
{
    /// The value of the variable it names.
    ///
    /// # Errors
    ///
    /// The faults of Structured Text's dereference (specification 44 §6.5): `NullReference` for
    /// `NULL`, `ReferenceLifetime` for a reference of another generation or outside its area, and
    /// `TypeMismatch` for a variable that is no `T`.
    pub fn get(&self, cx: &mut Cx) -> Result<T, Fault> {
        module::with_areas(cx.raw(), |raw, areas| {
            areas.deref::<T::Area>(raw, &self.value).map(T::load)
        })
    }

    /// Writes `value` into the variable it names; a string is cut to the variable's capacity
    /// (specification 02 §6).
    ///
    /// # Errors
    ///
    /// The faults of [`get`](Ref::get); nothing is written then.
    pub fn set(&self, cx: &mut Cx, value: T) -> Result<(), Fault> {
        module::with_areas(cx.raw(), |raw, areas| {
            areas
                .deref::<T::Area>(raw, &self.value)
                .map(|place| value.store(place))
        })
    }
}

/// The reference the generated code's `value` holds.
#[must_use]
#[inline]
pub fn reference<T>(value: RefValue) -> Ref<T> {
    Ref {
        value,
        target: PhantomData,
    }
}

/// The generated code's form of `reference`.
#[must_use]
#[inline]
pub fn reference_value<T>(reference: Ref<T>) -> RefValue {
    reference.value
}

#[cfg(test)]
#[path = "refs_tests.rs"]
mod tests;
