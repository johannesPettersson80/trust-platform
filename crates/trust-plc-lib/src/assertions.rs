//! The assertions of a test (specification 07, MP-014; specification 44 §6.8): a failed assertion ends the
//! test with a fault whose record carries what the kernel needs to write the runtime's message:
//! the operands' bytes and the type table's indexes of their types, or the three reals of
//! `ASSERT_NEAR`.

use trust_plc_abi::module::fault;
use trust_plc_abi::{Cx, Fault, FaultCode};

/// The assertion of a failed comparison, as the kernel reads it from the fault record's third
/// number (specification 44 Appendix A).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Assertion {
    /// `ASSERT_TRUE(IN)`.
    True = 1,
    /// `ASSERT_FALSE(IN)`.
    False = 2,
    /// `ASSERT_EQUAL(EXPECTED, ACTUAL)`.
    Equal = 3,
    /// `ASSERT_NOT_EQUAL(EXPECTED, ACTUAL)`.
    NotEqual = 4,
    /// `ASSERT_GREATER(VALUE, BOUND)`.
    Greater = 5,
    /// `ASSERT_LESS(VALUE, BOUND)`.
    Less = 6,
    /// `ASSERT_GREATER_OR_EQUAL(VALUE, BOUND)`.
    GreaterOrEqual = 7,
    /// `ASSERT_LESS_OR_EQUAL(VALUE, BOUND)`.
    LessOrEqual = 8,
}

/// The fault of a failed comparison `assertion` of the operands whose bytes, little-endian and
/// widened to 64 bits, are `left` and `right`, of the types at `left_type` and `right_type` of the
/// type table: `AssertionFailed`, with the operands in the first two numbers and the assertion and
/// the types in the third (bits 0 to 7, 8 to 31 and 32 to 55).
#[must_use]
#[allow(clippy::cast_possible_wrap)]
pub fn assertion_failed(
    cx: &mut Cx,
    assertion: Assertion,
    left: u64,
    right: u64,
    left_type: u32,
    right_type: u32,
) -> Fault {
    let described = u64::from(assertion as u8)
        | (u64::from(left_type & 0x00FF_FFFF) << 8)
        | (u64::from(right_type & 0x00FF_FFFF) << 32);
    fault(
        cx,
        FaultCode::AssertionFailed,
        left as i64,
        right as i64,
        described as i64,
    )
}

/// `ASSERT_NEAR(EXPECTED, ACTUAL, DELTA)` of three reals in binary64, as the runtime decides it: a
/// value that is not finite is an overflow; a negative `DELTA`, or a difference above `DELTA` and
/// the rounding tolerance of four units in the last place of the largest magnitude, fails.
///
/// # Errors
///
/// `Overflow` for a value that is not finite, `AssertionNear` with the three reals for a failure.
#[allow(clippy::cast_possible_wrap)]
pub fn assert_near(cx: &mut Cx, expected: f64, actual: f64, delta: f64) -> Result<(), Fault> {
    if !expected.is_finite() || !actual.is_finite() || !delta.is_finite() {
        return Err(fault(cx, FaultCode::Overflow, 0, 0, 0));
    }
    let difference = libm::fabs(expected - actual);
    let tolerance = f64::EPSILON
        * libm::fmax(
            libm::fmax(libm::fabs(expected), libm::fabs(actual)),
            libm::fmax(delta, 1.0),
        )
        * 4.0;
    if delta >= 0.0 && (difference <= delta || difference - delta <= tolerance) {
        return Ok(());
    }
    Err(fault(
        cx,
        FaultCode::AssertionNear,
        expected.to_bits() as i64,
        actual.to_bits() as i64,
        delta.to_bits() as i64,
    ))
}

/// A scalar operand of an assertion as the fault record carries it: its bytes, little-endian, in
/// the low bytes of a `u64`.
pub trait Operand: Copy {
    /// The operand's bytes.
    fn bits(self) -> u64;
}

macro_rules! signed_operand {
    ($($t:ty => $unsigned:ty),*) => {$(
        impl Operand for $t {
            #[allow(clippy::cast_sign_loss)]
            fn bits(self) -> u64 {
                u64::from(self as $unsigned)
            }
        }
    )*};
}

macro_rules! unsigned_operand {
    ($($t:ty),*) => {$(
        impl Operand for $t {
            fn bits(self) -> u64 {
                u64::from(self)
            }
        }
    )*};
}

signed_operand!(i8 => u8, i16 => u16, i32 => u32, i64 => u64);
unsigned_operand!(u8, u16, u32, bool);

impl Operand for u64 {
    fn bits(self) -> u64 {
        self
    }
}

impl Operand for f32 {
    fn bits(self) -> u64 {
        u64::from(self.to_bits())
    }
}

impl Operand for f64 {
    fn bits(self) -> u64 {
        self.to_bits()
    }
}
