//! The types that may lie in an area.

use core::cell::Cell;

use crate::{RefValue, Str, WStr};

/// A type that may lie in an area and be viewed there through a shared reference.
///
/// # Safety
///
/// Every bit pattern of the type's size is a valid value of it, it holds no reference or pointer,
/// and it changes only through cells: a kernel copies an area's bytes in and out (the starting
/// memory, retain, online change), and a program writes it through shared references. Generated
/// code declares its types with [`plain_struct!`](crate::plain_struct), which implements this trait
/// only for a structure whose every field is `Plain`.
#[allow(unsafe_code)]
pub unsafe trait Plain: Sized {
    /// The value of all-zero bytes, which a function's frame starts from.
    #[allow(clippy::declare_interior_mutable_const)]
    const ZEROED: Self;
}

macro_rules! plain_cells {
    ($($t:ty = $zero:expr),*) => {
        $(
            // SAFETY: a cell of an integer, a float or a reference record has the layout of its
            // value, every bit pattern is a value, and it holds no pointer.
            #[allow(unsafe_code)]
            unsafe impl Plain for Cell<$t> {
                #[allow(clippy::declare_interior_mutable_const)]
                const ZEROED: Self = Cell::new($zero);
            }
        )*
    };
}

plain_cells!(
    u8 = 0,
    i8 = 0,
    u16 = 0,
    i16 = 0,
    u32 = 0,
    i32 = 0,
    u64 = 0,
    i64 = 0,
    f32 = 0.0,
    f64 = 0.0,
    RefValue = RefValue::NULL_REF
);

// SAFETY: an array of plain elements is its elements one after the other.
#[allow(unsafe_code)]
unsafe impl<T: Plain, const N: usize> Plain for [T; N] {
    #[allow(clippy::declare_interior_mutable_const)]
    const ZEROED: Self = [const { T::ZEROED }; N];
}

// SAFETY: a string is a cell of its length and cells of its bytes or code units.
#[allow(unsafe_code)]
unsafe impl<const N: usize> Plain for Str<N> {
    #[allow(clippy::declare_interior_mutable_const)]
    const ZEROED: Self = Str::new();
}

// SAFETY: as for `Str`.
#[allow(unsafe_code)]
unsafe impl<const N: usize> Plain for WStr<N> {
    #[allow(clippy::declare_interior_mutable_const)]
    const ZEROED: Self = WStr::new();
}

/// Declares a `#[repr(C)]` structure of plain fields and implements [`Plain`] for it, as generated
/// code and the program library declare every structure, function block and area (specification
/// 44 §6.3). A field that is not `Plain` fails the build.
///
/// ```
/// use core::cell::Cell;
/// use trust_plc_abi::Plain;
/// trust_plc_abi::plain_struct! {
///     pub struct Recipe {
///         pub fill_level: Cell<f32>,
///         pub name: trust_plc_abi::Str<16>,
///     }
/// }
/// // 4 bytes of the level, 2 of the name's length and 16 of its bytes, padded to 4.
/// assert_eq!(core::mem::size_of::<Recipe>(), 24);
/// let recipe = Recipe::ZEROED;
/// assert_eq!(recipe.fill_level.get(), 0.0);
/// ```
#[macro_export]
macro_rules! plain_struct {
    ($(#[$attr:meta])* $vis:vis struct $name:ident { $($(#[$fattr:meta])* $fvis:vis $field:ident : $ty:ty),* $(,)? }) => {
        $(#[$attr])*
        #[repr(C)]
        $vis struct $name { $($(#[$fattr])* $fvis $field: $ty),* }

        // SAFETY: the structure is `#[repr(C)]` and its zero value's fields, each the `ZEROED` of
        // its type, prove every field `Plain`, so every bit pattern of its fields and padding is a
        // value of it.
        #[allow(unsafe_code)]
        unsafe impl $crate::Plain for $name {
            #[allow(clippy::declare_interior_mutable_const)]
            const ZEROED: Self = Self { $($field: <$ty as $crate::Plain>::ZEROED),* };
        }
    };
}
