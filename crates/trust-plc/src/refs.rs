//! In-outs and references of Rust POUs (specification 48 RU-29).
//!
//! A function's in-out is the caller's variable itself, [`InOut`], borrowed for the call. A
//! function block's in-out keeps the reference of its last call, and a `REF_TO` variable or input
//! holds one: a [`Ref`], which names a variable in the program's areas and reads and writes it with
//! the checks Structured Text's dereference makes (specification 44 §6.5, §7.3). Both read and
//! write the variable through its [`Place`], at once.

use trust_plc_abi::{module, FaultCode, Plain, RefValue};

use crate::{Cx, Fault, Place};

/// A function's or a method's in-out of type `T`: the caller's variable itself, for the length of
/// the call (IEC §6.6.1.2.2). A write is made at once, as Structured Text's is; a structure's
/// writes every field.
///
/// A unit test gives one a variable of its own, a [`Cell`](core::cell::Cell) of any type:
/// `InOut::new(&Cell::new(2.5))` for an `LREAL`.
pub struct InOut<'a, T> {
    place: &'a dyn Place<T>,
}

impl<'a, T> InOut<'a, T> {
    /// The in-out of the variable that lies in `place`.
    #[must_use]
    pub fn new(place: &'a dyn Place<T>) -> Self {
        Self { place }
    }

    /// The variable's value.
    #[must_use]
    pub fn get(&self) -> T {
        self.place.load()
    }

    /// Writes `value` into the variable; a string is cut to the variable's capacity (specification
    /// 02 §6).
    pub fn set(&self, value: T)
    where
        T: Copy,
    {
        self.place.store(&value);
    }
}

/// A reference to a variable of type `T` (`REF_TO`, specification 44 §7.3), or `NULL`: a function
/// block's in-out (`#[in_out]`), the reference of its last call, or a `REF_TO` variable or input.
/// It is copied and compared as Structured Text's references are.
pub struct Ref<T> {
    value: RefValue,
    access: Access<T>,
}

/// How a reference reaches its variable: the dereference of the type the variable lies as, which
/// the generated code names when it reads the reference (specification 44 §6.5).
struct Access<T> {
    load: fn(&mut trust_plc_abi::Cx, &RefValue) -> Result<T, Fault>,
    store: fn(&mut trust_plc_abi::Cx, &RefValue, &T) -> Result<(), Fault>,
}

impl<T> Clone for Access<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> Copy for Access<T> {}

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
        access: Access {
            load: null_load::<T>,
            store: null_store::<T>,
        },
    };

    /// Whether it is `NULL`.
    #[must_use]
    pub fn is_null(&self) -> bool {
        self.value.kind == RefValue::NULL
    }

    /// The value of the variable it names; a structure's every field.
    ///
    /// # Errors
    ///
    /// The faults of Structured Text's dereference (specification 44 §6.5): `NullReference` for
    /// `NULL`, `ReferenceLifetime` for a reference of another generation or outside its area, and
    /// `TypeMismatch` for a variable that is no `T`.
    pub fn get(&self, cx: &mut Cx) -> Result<T, Fault> {
        (self.access.load)(cx.raw(), &self.value)
    }

    /// Writes `value` into the variable it names, a structure's every field, at once; a string is
    /// cut to the variable's capacity (specification 02 §6).
    ///
    /// # Errors
    ///
    /// The faults of [`get`](Ref::get); nothing is written then.
    pub fn set(&self, cx: &mut Cx, value: T) -> Result<(), Fault>
    where
        T: Copy,
    {
        (self.access.store)(cx.raw(), &self.value, &value)
    }
}

/// The reference the generated code's `value` holds, to a variable that lies as `A`.
#[must_use]
#[inline]
pub fn reference<T, A: Plain + Place<T>>(value: RefValue) -> Ref<T> {
    Ref {
        value,
        access: Access {
            load: load_at::<T, A>,
            store: store_at::<T, A>,
        },
    }
}

/// The generated code's form of `reference`.
#[must_use]
#[inline]
pub fn reference_value<T>(reference: Ref<T>) -> RefValue {
    reference.value
}

/// Reads the variable `value` names, which lies as `A`, with the dereference's checks.
fn load_at<T, A: Plain + Place<T>>(
    raw: &mut trust_plc_abi::Cx,
    value: &RefValue,
) -> Result<T, Fault> {
    module::with_areas(raw, |raw, areas| {
        areas.deref::<A>(raw, value).map(<A as Place<T>>::load)
    })
}

/// Writes `item` into the variable `value` names, which lies as `A`, with the dereference's checks.
fn store_at<T, A: Plain + Place<T>>(
    raw: &mut trust_plc_abi::Cx,
    value: &RefValue,
    item: &T,
) -> Result<(), Fault> {
    module::with_areas(raw, |raw, areas| {
        areas
            .deref::<A>(raw, value)
            .map(|place| <A as Place<T>>::store(place, item))
    })
}

/// `NULL`'s read: the fault Structured Text's dereference of `NULL` records.
fn null_load<T>(raw: &mut trust_plc_abi::Cx, _: &RefValue) -> Result<T, Fault> {
    Err(module::fault(raw, FaultCode::NullReference, 0, 0, 0))
}

/// `NULL`'s write, which writes nothing.
fn null_store<T>(raw: &mut trust_plc_abi::Cx, _: &RefValue, _: &T) -> Result<(), Fault> {
    Err(module::fault(raw, FaultCode::NullReference, 0, 0, 0))
}

#[cfg(test)]
#[path = "refs_tests.rs"]
mod tests;
