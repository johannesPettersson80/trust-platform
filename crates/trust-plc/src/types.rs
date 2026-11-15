//! The IEC types a Rust POU's variables have beside Rust's own (specification 48 RU-25): the bit
//! strings `BYTE` to `LWORD`, which are not integers, the characters `CHAR` and `WCHAR`, and the
//! durations `TIME` and `LTIME`. Each holds the value as the program's areas store it
//! (specification 44 §6.2), so that a Rust POU and the Structured Text beside it read the same
//! bits.

use core::cell::Cell;
use core::ops::{BitAnd, BitOr, BitXor, Not};

use crate::{checked, Cx, Fault};

/// A value as the program's areas store it, which the generated code reads into a Rust POU's
/// variable and writes back (specification 48 RU-28). Every type of RU-25's table has it.
pub trait Stored: Copy {
    /// The stored form: an integer, a real or `bool`.
    type Raw: Copy;
    /// The value of the stored form `raw`.
    fn from_raw(raw: Self::Raw) -> Self;
    /// The stored form of the value.
    fn into_raw(self) -> Self::Raw;
}

macro_rules! stored_as_itself {
    ($($ty:ty),*) => {$(
        impl Stored for $ty {
            type Raw = $ty;
            #[inline]
            fn from_raw(raw: $ty) -> Self {
                raw
            }
            #[inline]
            fn into_raw(self) -> $ty {
                self
            }
        }
    )*};
}

stored_as_itself!(bool, i8, i16, i32, i64, u8, u16, u32, u64, f32, f64);

/// A value of a Rust POU's variable as it lies in its place, one cell or more: the generated code
/// reads a string or an array of the instance from its place with it and writes it back
/// (specification 48 RU-28). Every type of RU-25's table has it, its place the type the generated
/// code gives the variable (specification 44 §6.2).
pub trait Placed: Sized {
    /// The generated code's type of the place.
    type Area;
    /// The value the place `area` holds.
    fn load(area: &Self::Area) -> Self;
    /// Writes the value into the place `area`.
    fn store(&self, area: &Self::Area);
}

/// A `BOOL` lies in a byte, 0 or 1 (specification 44 §6.2).
impl Placed for bool {
    type Area = Cell<u8>;
    #[inline]
    fn load(area: &Cell<u8>) -> Self {
        area.get() != 0
    }
    #[inline]
    fn store(&self, area: &Cell<u8>) {
        area.set(u8::from(*self));
    }
}

/// Every other elementary value lies in a cell of its stored form.
macro_rules! placed_in_its_cell {
    ($($ty:ty),*) => {$(
        impl Placed for $ty {
            type Area = Cell<<$ty as Stored>::Raw>;
            #[inline]
            fn load(area: &Self::Area) -> Self {
                <$ty as Stored>::from_raw(area.get())
            }
            #[inline]
            fn store(&self, area: &Self::Area) {
                area.set(Stored::into_raw(*self));
            }
        }
    )*};
}

placed_in_its_cell!(i8, i16, i32, i64, u8, u16, u32, u64, f32, f64);
placed_in_its_cell!(Byte, Word, Dword, Lword, Char, WChar, Time, LTime);

macro_rules! bit_string {
    ($(#[$doc:meta])* $name:ident, $raw:ty, $bits:expr) => {
        $(#[$doc])*
        #[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
        #[repr(transparent)]
        pub struct $name(pub $raw);

        impl $name {
            /// The number of its bits.
            pub const BITS: u32 = $bits;

            /// Bit `index`, counted from 0 at the least significant; `false` past the last.
            #[must_use]
            pub const fn bit(self, index: u32) -> bool {
                index < Self::BITS && (self.0 >> index) & 1 == 1
            }

            /// The bit string with bit `index` set to `value`; unchanged past the last.
            #[must_use]
            pub const fn with_bit(self, index: u32, value: bool) -> Self {
                if index >= Self::BITS {
                    return self;
                }
                let mask = 1 << index;
                Self(if value { self.0 | mask } else { self.0 & !mask })
            }
        }

        impl Stored for $name {
            type Raw = $raw;
            #[inline]
            fn from_raw(raw: $raw) -> Self {
                Self(raw)
            }
            #[inline]
            fn into_raw(self) -> $raw {
                self.0
            }
        }

        impl BitAnd for $name {
            type Output = Self;
            fn bitand(self, other: Self) -> Self {
                Self(self.0 & other.0)
            }
        }

        impl BitOr for $name {
            type Output = Self;
            fn bitor(self, other: Self) -> Self {
                Self(self.0 | other.0)
            }
        }

        impl BitXor for $name {
            type Output = Self;
            fn bitxor(self, other: Self) -> Self {
                Self(self.0 ^ other.0)
            }
        }

        impl Not for $name {
            type Output = Self;
            fn not(self) -> Self {
                Self(!self.0)
            }
        }
    };
}

bit_string!(
    /// `BYTE`: a string of 8 bits, which `AND`, `OR`, `XOR` and `NOT` combine bit by bit (IEC
    /// §6.4.2, Table 10).
    Byte,
    u8,
    8
);
bit_string!(
    /// `WORD`: a string of 16 bits.
    Word,
    u16,
    16
);
bit_string!(
    /// `DWORD`: a string of 32 bits.
    Dword,
    u32,
    32
);
bit_string!(
    /// `LWORD`: a string of 64 bits.
    Lword,
    u64,
    64
);

/// `CHAR`: one byte of a `STRING` (IEC §6.4.2, Table 10).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[repr(transparent)]
pub struct Char(pub u8);

/// `WCHAR`: one UTF-16 code unit of a `WSTRING`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[repr(transparent)]
pub struct WChar(pub u16);

impl Stored for Char {
    type Raw = u8;
    #[inline]
    fn from_raw(raw: u8) -> Self {
        Self(raw)
    }
    #[inline]
    fn into_raw(self) -> u8 {
        self.0
    }
}

impl Stored for WChar {
    type Raw = u16;
    #[inline]
    fn from_raw(raw: u16) -> Self {
        Self(raw)
    }
    #[inline]
    fn into_raw(self) -> u16 {
        self.0
    }
}

macro_rules! duration {
    ($(#[$doc:meta])* $name:ident) => {
        $(#[$doc])*
        #[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
        #[repr(transparent)]
        pub struct $name(i64);

        impl $name {
            /// No time.
            pub const ZERO: Self = Self(0);

            /// The duration of `nanos` nanoseconds.
            #[must_use]
            pub const fn from_nanos(nanos: i64) -> Self {
                Self(nanos)
            }

            /// The duration in nanoseconds.
            #[must_use]
            pub const fn as_nanos(self) -> i64 {
                self.0
            }

            /// The duration of `millis` milliseconds; a duration beyond the type is an overflow.
            ///
            /// # Errors
            ///
            /// `Overflow` for a duration beyond the type (specification 05 §3).
            pub fn from_millis(cx: &mut Cx, millis: i64) -> Result<Self, Fault> {
                checked::mul(cx, millis, 1_000_000).map(Self)
            }

            /// The sum of two durations, `Overflow` beyond the type, as Structured Text's `+` of
            /// durations faults (specification 05 §3).
            ///
            /// # Errors
            ///
            /// `Overflow` for a sum beyond the type.
            pub fn checked_add(self, cx: &mut Cx, other: Self) -> Result<Self, Fault> {
                checked::add(cx, self.0, other.0).map(Self)
            }

            /// The difference of two durations, `Overflow` beyond the type.
            ///
            /// # Errors
            ///
            /// `Overflow` for a difference beyond the type.
            pub fn checked_sub(self, cx: &mut Cx, other: Self) -> Result<Self, Fault> {
                checked::sub(cx, self.0, other.0).map(Self)
            }
        }

        impl Stored for $name {
            type Raw = i64;
            #[inline]
            fn from_raw(raw: i64) -> Self {
                Self(raw)
            }
            #[inline]
            fn into_raw(self) -> i64 {
                self.0
            }
        }
    };
}

duration!(
    /// `TIME`: a duration in nanoseconds, as the areas store it (specification 44 §6.2).
    Time
);
duration!(
    /// `LTIME`: a duration in nanoseconds.
    LTime
);
