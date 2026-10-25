//! Arrays whose bounds a caller supplies, `ARRAY[*]` (IEC 61131-3 §6.5.3; specification 03 §6;
//! specification 44 §6.2, §7.3): a view of the caller's elements with each dimension's bounds, and
//! a reference to such an array, which a `POINTER TO ARRAY[*]`, a `REF_TO ARRAY[*]` and a function
//! block's open-array in-out hold.

use core::cell::Cell;

use crate::module::fault;
use crate::{Cx, Fault, FaultCode, Plain, RefValue};

/// The elements of an array a caller supplies, in their order, the last index fastest, with the
/// bounds of each of its `R` dimensions.
pub struct Open<'a, T, const R: usize> {
    elements: &'a [T],
    bounds: [(i64, i64); R],
}

impl<T, const R: usize> Clone for Open<'_, T, R> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T, const R: usize> Copy for Open<'_, T, R> {}

impl<'a, T, const R: usize> Open<'a, T, R> {
    /// The array of `elements` with the bounds `bounds`, which count as many elements.
    #[must_use]
    pub const fn new(elements: &'a [T], bounds: [(i64, i64); R]) -> Self {
        Self { elements, bounds }
    }

    /// The element at `index`, each index checked against its dimension's bounds.
    ///
    /// # Errors
    ///
    /// [`FaultCode::IndexOutOfBounds`] with the first index outside its bounds and those bounds.
    pub fn at(&self, cx: &mut Cx, index: [i64; R]) -> Result<&'a T, Fault> {
        let mut position = 0u64;
        for (value, (lower, upper)) in index.into_iter().zip(self.bounds) {
            if value < lower || value > upper {
                return Err(fault(cx, FaultCode::IndexOutOfBounds, value, lower, upper));
            }
            let length = upper.abs_diff(lower).saturating_add(1);
            position = position
                .saturating_mul(length)
                .saturating_add(value.abs_diff(lower));
        }
        usize::try_from(position)
            .ok()
            .and_then(|position| self.elements.get(position))
            .ok_or_else(|| fault(cx, FaultCode::ModuleInternal, 0, 0, 0))
    }

    /// `LOWER_BOUND` of dimension `dimension`, counted from 1 (IEC §6.5.3).
    ///
    /// # Errors
    ///
    /// [`FaultCode::IndexOutOfBounds`] for a dimension the array does not have.
    pub fn lower(&self, cx: &mut Cx, dimension: i64) -> Result<i64, Fault> {
        self.dimension(cx, dimension).map(|(lower, _)| lower)
    }

    /// `UPPER_BOUND` of dimension `dimension`, counted from 1 (IEC §6.5.3).
    ///
    /// # Errors
    ///
    /// [`FaultCode::IndexOutOfBounds`] for a dimension the array does not have.
    pub fn upper(&self, cx: &mut Cx, dimension: i64) -> Result<i64, Fault> {
        self.dimension(cx, dimension).map(|(_, upper)| upper)
    }

    fn dimension(&self, cx: &mut Cx, dimension: i64) -> Result<(i64, i64), Fault> {
        usize::try_from(dimension)
            .ok()
            .and_then(|dimension| dimension.checked_sub(1))
            .and_then(|index| self.bounds.get(index))
            .copied()
            .ok_or_else(|| {
                fault(
                    cx,
                    FaultCode::IndexOutOfBounds,
                    dimension,
                    1,
                    i64::try_from(R).unwrap_or(i64::MAX),
                )
            })
    }

    /// The elements, in their order.
    #[must_use]
    pub const fn elements(&self) -> &'a [T] {
        self.elements
    }

    /// The bounds of each dimension.
    #[must_use]
    pub const fn bounds(&self) -> [(i64, i64); R] {
        self.bounds
    }
}

/// A reference to an array a caller supplies, with the bounds of each of its `R` dimensions: `NULL`
/// until it names an array (specification 44 §7.3).
#[repr(C)]
pub struct OpenRef<const R: usize> {
    reference: Cell<RefValue>,
    bounds: [[Cell<i64>; 2]; R],
}

// SAFETY: the structure is `#[repr(C)]` of a `RefValue` cell and `i64` cells, every bit pattern of
// which is a value; all zeros is `NULL` with no bounds.
#[allow(unsafe_code)]
unsafe impl<const R: usize> Plain for OpenRef<R> {
    #[allow(clippy::declare_interior_mutable_const)]
    const ZEROED: Self = Self {
        reference: <Cell<RefValue> as Plain>::ZEROED,
        bounds: <[[Cell<i64>; 2]; R] as Plain>::ZEROED,
    };
}

impl<const R: usize> OpenRef<R> {
    /// The reference, without its bounds: `NULL`, or the array it names.
    #[must_use]
    pub fn get(&self) -> RefValue {
        self.reference.get()
    }

    /// Names `reference`, an array of the bounds `bounds`.
    pub fn set(&self, reference: RefValue, bounds: [(i64, i64); R]) {
        self.reference.set(reference);
        for (cells, (lower, upper)) in self.bounds.iter().zip(bounds) {
            cells[0].set(lower);
            cells[1].set(upper);
        }
    }

    /// Takes the reference and the bounds of `source`.
    pub fn copy_from(&self, source: &Self) {
        self.set(source.get(), source.bounds());
    }

    /// The bounds of each dimension of the array it names.
    #[must_use]
    pub fn bounds(&self) -> [(i64, i64); R] {
        core::array::from_fn(|index| (self.bounds[index][0].get(), self.bounds[index][1].get()))
    }
}
