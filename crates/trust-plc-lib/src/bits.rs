//! The bit-shift functions of IEC §6.6.2.5.7 (Table 26) with the runtime's rules (specification 07
//! §4): a count at or above the bit string's width shifts every bit out, a rotation counts modulo
//! the width, and a negative count is a type mismatch.

use trust_plc_abi::module::fault;
use trust_plc_abi::{Cx, Fault, FaultCode};

/// A bit string of `BYTE`, `WORD`, `DWORD` or `LWORD`.
pub trait Bits: Copy {
    /// The bits of the type.
    const WIDTH: u64;
    /// The bits, zero-extended.
    fn to_bits(self) -> u64;
    /// The low bits of `bits`.
    #[must_use]
    fn from_bits(bits: u64) -> Self;
}

macro_rules! bits {
    ($($t:ty),*) => {
        $(
            impl Bits for $t {
                const WIDTH: u64 = <$t>::BITS as u64;
                fn to_bits(self) -> u64 {
                    u64::from(self)
                }
                #[allow(clippy::cast_possible_truncation)]
                fn from_bits(bits: u64) -> Self {
                    bits as $t
                }
            }
        )*
    };
}

bits!(u8, u16, u32, u64);

/// `SHL(IN, N)`.
#[must_use]
pub fn shl<T: Bits>(value: T, count: u64) -> T {
    if count >= T::WIDTH {
        T::from_bits(0)
    } else {
        T::from_bits(value.to_bits() << count)
    }
}

/// `SHR(IN, N)`.
#[must_use]
pub fn shr<T: Bits>(value: T, count: u64) -> T {
    if count >= T::WIDTH {
        T::from_bits(0)
    } else {
        T::from_bits(value.to_bits() >> count)
    }
}

/// `ROL(IN, N)`.
#[must_use]
pub fn rol<T: Bits>(value: T, count: u64) -> T {
    let shift = count % T::WIDTH;
    let bits = value.to_bits();
    if shift == 0 {
        value
    } else {
        T::from_bits((bits << shift) | (bits >> (T::WIDTH - shift)))
    }
}

/// `ROR(IN, N)`.
#[must_use]
pub fn ror<T: Bits>(value: T, count: u64) -> T {
    let shift = count % T::WIDTH;
    let bits = value.to_bits();
    if shift == 0 {
        value
    } else {
        T::from_bits((bits >> shift) | (bits << (T::WIDTH - shift)))
    }
}

/// `SHL` or `SHR` of a `BOOL`, one bit wide: any count but 0 shifts it out.
#[must_use]
pub fn shift_bool(value: bool, count: u64) -> bool {
    count == 0 && value
}

/// The count of a shift given in a signed type.
///
/// # Errors
///
/// `TypeMismatch` with the count for a negative count.
pub fn shift_count(cx: &mut Cx, count: i64) -> Result<u64, Fault> {
    u64::try_from(count).map_err(|_| fault(cx, FaultCode::TypeMismatch, count, 0, 0))
}
