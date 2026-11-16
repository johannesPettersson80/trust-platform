//! The arrays a Rust POU's variables have (specification 48 RU-25): `ARRAY[a..b] OF T`, written
//! `Array<T, A, N>`, the `N` elements of a type of RU-25's table indexed from `A` as Structured
//! Text indexes them (`ARRAY[A..A + N - 1]`). The length is a parameter of its own because Rust's
//! constant generics cannot compute one from two bounds. The generated code reads an array from
//! its place, element by element, and writes it back (specification 44 §6.2). None of its
//! functions can panic (specification 48 RU-14): an index outside the bounds gives `None`.

use core::fmt;

use crate::types::Placed;

/// `ARRAY[A..A + N - 1] OF T`: `N` elements of `T`, indexed from `A`.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Array<T, const A: i64, const N: usize> {
    items: [T; N],
}

impl<T: Copy, const A: i64, const N: usize> Array<T, A, N> {
    /// The index of the first element, `A`.
    pub const LOWER: i64 = A;

    /// The array of `N` elements, each `item`: the attributes give an array its starting values
    /// with it, the type's defaults when it has no initial value (specification 48 RU-26).
    #[must_use]
    pub const fn filled(item: T) -> Self {
        Self { items: [item; N] }
    }

    /// The array of the elements `items`, the first at index `A`.
    #[must_use]
    pub const fn from_items(items: [T; N]) -> Self {
        Self { items }
    }

    /// The element at `index`, counted from `A`; `None` outside the bounds.
    #[must_use]
    pub fn get(&self, index: i64) -> Option<&T> {
        self.items.get(Self::offset(index)?)
    }

    /// The element at `index`, counted from `A`, to change; `None` outside the bounds.
    #[must_use]
    pub fn get_mut(&mut self, index: i64) -> Option<&mut T> {
        self.items.get_mut(Self::offset(index)?)
    }

    /// The elements in their order, the first at index `A`.
    #[must_use]
    pub fn as_slice(&self) -> &[T] {
        &self.items
    }

    /// The elements in their order, to change.
    #[must_use]
    pub fn as_mut_slice(&mut self) -> &mut [T] {
        &mut self.items
    }

    /// The elements in their order.
    pub fn iter(&self) -> core::slice::Iter<'_, T> {
        self.items.iter()
    }

    /// The number of elements, `N`.
    #[must_use]
    pub const fn len(&self) -> usize {
        N
    }

    /// Whether it has no elements; an array always has one at least (IEC §6.4.4.3).
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        N == 0
    }

    /// The position in the elements of `index`.
    fn offset(index: i64) -> Option<usize> {
        usize::try_from(index.checked_sub(A)?).ok()
    }
}

impl<'a, T, const A: i64, const N: usize> IntoIterator for &'a Array<T, A, N> {
    type Item = &'a T;
    type IntoIter = core::slice::Iter<'a, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.items.iter()
    }
}

/// An array's default holds its elements' defaults (IEC §6.4.2), which a block's `new()` gives it.
impl<T: crate::Declared, const A: i64, const N: usize> crate::Declared for Array<T, A, N> {
    const DEFAULT: Self = Self::from_items([T::DEFAULT; N]);
}

impl<T: Default + Copy, const A: i64, const N: usize> Default for Array<T, A, N> {
    fn default() -> Self {
        Self::from_items([T::default(); N])
    }
}

impl<T: fmt::Debug, const A: i64, const N: usize> fmt::Debug for Array<T, A, N> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "Array<{A}..>")?;
        formatter.debug_list().entries(self.items.iter()).finish()
    }
}

impl<T: Placed, const A: i64, const N: usize> Placed for Array<T, A, N> {
    type Area = [T::Area; N];

    fn load(area: &Self::Area) -> Self {
        Self {
            items: area.each_ref().map(T::load),
        }
    }

    fn store(&self, area: &Self::Area) {
        for (item, place) in self.items.iter().zip(area.iter()) {
            item.store(place);
        }
    }
}
