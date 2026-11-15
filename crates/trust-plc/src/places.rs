//! Where a Rust POU's in-outs and references read and write (specification 48 RU-29): the
//! variable a function's in-out is, and the one a reference names, as it lies in its place.
//!
//! A [`Place`] of `T` is where a value of `T` lies. The SDK gives the places of RU-25's types as
//! the generated code lays them out (specification 44 §6.2) and the caller's string a function's
//! string in-out is (DEV-035), `#[trust_enum]` gives an enumeration's and the generated
//! code a structure's, through its fields' places; a unit test gives an in-out a [`Cell`] of its
//! own, of any type.

use core::cell::Cell;

use crate::{
    Array, Byte, Char, Date, Dt, Dword, LDate, LTime, LTod, Ldt, Lword, Placed, Str, Time, Tod,
    WChar, WStr, Word,
};

/// Where a value of type `T` lies: an in-out's variable or the variable a reference names
/// (specification 48 RU-29). A read reads all of it and a write writes all of it at once, as
/// Structured Text's do, a structure every field.
pub trait Place<T> {
    /// The value the place holds.
    fn load(&self) -> T;
    /// Writes `value` into the place; a string is cut to the place's capacity (specification 02
    /// §6).
    fn store(&self, value: &T);
}

/// A variable that lies in a cell of its own type: a unit test's of any type
/// (`InOut::new(&Cell::new(2.5))`), and the generated code's of a type the areas store as itself,
/// an integer or a real.
impl<T: Copy> Place<T> for Cell<T> {
    #[inline]
    fn load(&self) -> T {
        self.get()
    }

    #[inline]
    fn store(&self, value: &T) {
        self.set(*value);
    }
}

/// The values the areas store in another form (specification 44 §6.2): a `BOOL` in a byte, a bit
/// string, a character, a duration, a date and a time of day in their integers.
macro_rules! placed_in_another_form {
    ($($ty:ty),*) => {$(
        impl Place<$ty> for <$ty as Placed>::Area {
            #[inline]
            fn load(&self) -> $ty {
                <$ty as Placed>::load(self)
            }

            #[inline]
            fn store(&self, value: &$ty) {
                <$ty as Placed>::store(value, self);
            }
        }
    )*};
}

placed_in_another_form!(
    bool, Byte, Word, Dword, Lword, Char, WChar, Time, LTime, Date, LDate, Tod, LTod, Dt, Ldt
);

/// A `STRING[N]` lies as its length and its bytes.
impl<const N: usize> Place<Str<N>> for trust_plc_abi::Str<N> {
    #[inline]
    fn load(&self) -> Str<N> {
        <Str<N> as Placed>::load(self)
    }

    #[inline]
    fn store(&self, value: &Str<N>) {
        <Str<N> as Placed>::store(value, self);
    }
}

/// A `WSTRING[N]` lies as its length and its code units.
impl<const N: usize> Place<WStr<N>> for trust_plc_abi::WStr<N> {
    #[inline]
    fn load(&self) -> WStr<N> {
        <WStr<N> as Placed>::load(self)
    }

    #[inline]
    fn store(&self, value: &WStr<N>) {
        <WStr<N> as Placed>::store(value, self);
    }
}

/// A function's or a method's in-out of a `STRING[N]`, the caller's string, which the CODESYS
/// profile lets be of any capacity (`docs/IEC_DEVIATIONS.md` DEV-035): it reads the caller's text as
/// an assignment to a `STRING[N]` does and writes a value as an assignment to the caller's string
/// does, each cut to the longest prefix of whole characters the target's capacity holds
/// (specification 10 §2.3.1), with the program library's assignment.
impl<const N: usize> Place<Str<N>> for trust_plc_abi::StrView<'_> {
    fn load(&self) -> Str<N> {
        let text = trust_plc_abi::Str::<N>::new();
        trust_plc_lib::assign(text.view(), *self);
        <Str<N> as Placed>::load(&text)
    }

    fn store(&self, value: &Str<N>) {
        let text = trust_plc_abi::Str::<N>::new();
        <Str<N> as Placed>::store(value, &text);
        trust_plc_lib::assign(*self, text.view());
    }
}

/// A function's or a method's in-out of a `WSTRING[N]`, as a `STRING[N]`'s.
impl<const N: usize> Place<WStr<N>> for trust_plc_abi::WStrView<'_> {
    fn load(&self) -> WStr<N> {
        let text = trust_plc_abi::WStr::<N>::new();
        trust_plc_lib::assign_wide(text.view(), *self);
        <WStr<N> as Placed>::load(&text)
    }

    fn store(&self, value: &WStr<N>) {
        let text = trust_plc_abi::WStr::<N>::new();
        <WStr<N> as Placed>::store(value, &text);
        trust_plc_lib::assign_wide(*self, text.view());
    }
}

/// An array lies as its elements, one after the other, each in its own place.
impl<T: Copy, E: Place<T>, const A: i64, const N: usize> Place<Array<T, A, N>> for [E; N] {
    fn load(&self) -> Array<T, A, N> {
        Array::from_items(self.each_ref().map(<E as Place<T>>::load))
    }

    fn store(&self, value: &Array<T, A, N>) {
        for (element, item) in self.iter().zip(value.iter()) {
            <E as Place<T>>::store(element, item);
        }
    }
}
