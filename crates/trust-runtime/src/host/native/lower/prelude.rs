/// The program library's and the interface's Rust names every generated crate uses.
pub(in crate::native) const PRELUDE: &str = "use core::cell::Cell;
use trust_plc_abi::{self as abi, plain_struct, Cx, Fault, Plain, Str, WStr};
use trust_plc_abi::module::{fault, recurse, run, stmt as st, tick, Areas};
use trust_plc_lib as lib;

type R<T> = Result<T, Fault>;

/// A value copied into a place of its type: a scalar, a string, an array, a structure.
trait Assign {
    fn assign(&self, source: &Self);
}

impl<T: Copy> Assign for Cell<T> {
    #[inline]
    fn assign(&self, source: &Self) {
        self.set(source.get());
    }
}

impl<const N: usize> Assign for Str<N> {
    #[inline]
    fn assign(&self, source: &Self) {
        lib::assign(self.view(), source.view());
    }
}

impl<const N: usize> Assign for WStr<N> {
    #[inline]
    fn assign(&self, source: &Self) {
        lib::assign_wide(self.view(), source.view());
    }
}

impl<T: Assign, const N: usize> Assign for [T; N] {
    #[inline]
    fn assign(&self, source: &Self) {
        for (target, value) in self.iter().zip(source.iter()) {
            target.assign(value);
        }
    }
}

impl<const N: usize> Assign for abi::OpenRef<N> {
    #[inline]
    fn assign(&self, source: &Self) {
        self.copy_from(source);
    }
}

/// Whether two references or interface values name the same variable or instance, whatever type
/// each was taken as (an instance of a derived type as its base spans less).
#[inline]
fn same_target(a: abi::RefValue, b: abi::RefValue) -> bool {
    a.kind == b.kind
        && a.area == b.area
        && a.char_index == b.char_index
        && a.offset == b.offset
        && a.generation == b.generation
}

/// The members of a `STRUCT OVERLAP` (specification 02 §3.6; specification 44 §6.2): views of the
/// bytes each takes of the structure's backing, read and written in the target's byte order, so
/// that a write through one member is read through every member that shares its bits.
mod ov {
    use core::cell::Cell;
    use core::marker::PhantomData;

    /// The bytes of a member of the elementary type `T`.
    pub struct At<'a, T> {
        bytes: &'a [Cell<u8>],
        value: PhantomData<T>,
    }

    impl<'a, T> At<'a, T> {
        #[inline]
        pub fn new(bytes: &'a [Cell<u8>]) -> Self {
            Self {
                bytes,
                value: PhantomData,
            }
        }
    }

    macro_rules! views {
        ($($t:ty),*) => {$(
            impl At<'_, $t> {
                #[inline]
                pub fn get(&self) -> $t {
                    let mut bytes = [0u8; core::mem::size_of::<$t>()];
                    for (byte, cell) in bytes.iter_mut().zip(self.bytes) {
                        *byte = cell.get();
                    }
                    <$t>::from_ne_bytes(bytes)
                }

                #[inline]
                pub fn set(&self, value: $t) {
                    for (cell, byte) in self.bytes.iter().zip(value.to_ne_bytes()) {
                        cell.set(byte);
                    }
                }
            }
        )*};
    }

    views!(u8, i8, u16, i16, u32, i32, u64, i64, f32, f64);

    /// A `BOOL` member: one bit of a byte, read and written as a stored `BOOL`, 0 or 1.
    pub struct Bit<'a> {
        byte: &'a Cell<u8>,
        bit: u8,
    }

    impl<'a> Bit<'a> {
        #[inline]
        pub fn new(byte: &'a Cell<u8>, bit: u8) -> Self {
            Self { byte, bit }
        }

        #[inline]
        pub fn get(&self) -> u8 {
            (self.byte.get() >> self.bit) & 1
        }

        #[inline]
        pub fn set(&self, value: u8) {
            let mask = 1u8 << self.bit;
            let byte = self.byte.get();
            self.byte.set(if value != 0 { byte | mask } else { byte & !mask });
        }
    }
}
";
