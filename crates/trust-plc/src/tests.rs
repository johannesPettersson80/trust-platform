//! The SDK computes and faults as Structured Text does (specification 48 §5.3, §5.5; specification
//! 44 §6.5). Every expected value is the standard's or the specification's, worked out by hand.

// The expected values are exact: the standard's and the specification's, compared bit for bit.
#![allow(clippy::float_cmp)]

use trust_plc_abi::kernel::WITHOUT_DEBUGGER;

use crate::{checked, math, Cx, Fault, FaultCode};

fn cx() -> Cx {
    Cx::new(&WITHOUT_DEBUGGER)
}

fn code<T: core::fmt::Debug>(result: Result<T, Fault>) -> u32 {
    result.expect_err("the operation faults").raw()
}

#[test]
fn integer_arithmetic_faults_where_structured_text_faults() {
    // Catches an SDK operation that wraps or saturates as Rust's operators do, where Structured
    // Text ends the scan with the fault of specification 44 §6.5.
    let mut cx = cx();
    assert_eq!(checked::add(&mut cx, 2i16, 3), Ok(5));
    assert_eq!(
        code(checked::add(&mut cx, i16::MAX, 1)),
        FaultCode::Overflow as u32
    );
    assert_eq!(
        cx.fault.code,
        FaultCode::Overflow as u32,
        "the fault is recorded for the kernel"
    );
    assert_eq!(
        code(checked::sub(&mut cx, i8::MIN, 1)),
        FaultCode::Overflow as u32
    );
    assert_eq!(
        code(checked::mul(&mut cx, 1_000_000i32, 1_000_000)),
        FaultCode::Overflow as u32
    );
    assert_eq!(
        code(checked::add(&mut cx, u8::MAX, 1)),
        FaultCode::Overflow as u32
    );
    assert_eq!(
        code(checked::neg(&mut cx, i8::MIN)),
        FaultCode::Overflow as u32
    );
    assert_eq!(
        code(checked::abs(&mut cx, i16::MIN)),
        FaultCode::Overflow as u32
    );
}

#[test]
fn division_truncates_toward_zero_and_mod_keeps_the_dividends_sign() {
    // Catches a division that rounds down or a MOD with the divisor's sign: IEC 61131-3 §6.6.2.5.8
    // and specification 05 give -7 / 2 = -3 and -7 MOD 2 = -1; a zero divisor faults, each with its
    // own code, and the minimum divided by -1 is outside the type.
    let mut cx = cx();
    assert_eq!(checked::div(&mut cx, -7i32, 2), Ok(-3));
    assert_eq!(checked::rem(&mut cx, -7i32, 2), Ok(-1));
    assert_eq!(checked::rem(&mut cx, 7i32, -2), Ok(1));
    assert_eq!(
        code(checked::div(&mut cx, 7i32, 0)),
        FaultCode::DivisionByZero as u32
    );
    assert_eq!(
        code(checked::rem(&mut cx, 7i32, 0)),
        FaultCode::ModuloByZero as u32
    );
    assert_eq!(
        code(checked::div(&mut cx, i32::MIN, -1)),
        FaultCode::Overflow as u32
    );
}

#[test]
fn real_arithmetic_faults_on_a_result_that_is_not_finite() {
    // Catches a real operation that returns an infinity or a NaN, which Structured Text never
    // stores, and a real division by zero that gives an infinity: specification 44 §6.5 faults it
    // with `DivisionByZero`, as an integer one (IEC 61131-3 §6.6.2.5.8).
    let mut cx = cx();
    assert_eq!(checked::fadd(&mut cx, 0.5f32, 0.25), Ok(0.75));
    assert_eq!(
        code(checked::fmul(&mut cx, f32::MAX, 2.0)),
        FaultCode::Overflow as u32
    );
    assert_eq!(
        code(checked::fdiv(&mut cx, 1.0f64, 0.0)),
        FaultCode::DivisionByZero as u32
    );
    assert_eq!(
        code(checked::fdiv(&mut cx, f64::MAX, 0.5)),
        FaultCode::Overflow as u32
    );
    assert_eq!(
        code(checked::finite(&mut cx, f64::NAN)),
        FaultCode::Overflow as u32
    );
    assert_eq!(checked::fneg(1.5f32), -1.5);
}

#[test]
fn elementary_functions_give_the_standards_values_and_fault_outside_their_domain() {
    // Catches a wrapper that calls another function or lets a NaN or an infinity through.
    let mut cx = cx();
    assert_eq!(math::sqrt(&mut cx, 4.0f32), Ok(2.0));
    assert_eq!(math::exp(&mut cx, 0.0f64), Ok(1.0));
    assert_eq!(math::ln(&mut cx, 1.0f64), Ok(0.0));
    assert_eq!(math::log(&mut cx, 100.0f64), Ok(2.0));
    assert_eq!(math::sin(&mut cx, 0.0f32), Ok(0.0));
    assert_eq!(math::cos(&mut cx, 0.0f64), Ok(1.0));
    assert_eq!(
        code(math::sqrt(&mut cx, -1.0f64)),
        FaultCode::Overflow as u32
    );
    assert_eq!(code(math::ln(&mut cx, 0.0f32)), FaultCode::Overflow as u32);
    assert_eq!(
        code(math::asin(&mut cx, 2.0f64)),
        FaultCode::Overflow as u32
    );
    assert_eq!(math::expt64(&mut cx, 2.0, 10.0), Ok(1024.0));
    assert_eq!(
        code(math::expt32(&mut cx, f32::MAX, 2.0)),
        FaultCode::Overflow as u32
    );
}

#[test]
#[allow(clippy::cast_possible_truncation)]
fn a_reals_elementary_function_is_computed_in_binary64_and_rounded_once() {
    // Catches an implementation through libm's binary32 functions (`sinf` and the rest), which
    // differ from the binary64 result rounded once in the last bit for some arguments: specification
    // 44 computes a REAL's elementary function in binary64, so that a Rust POU and Structured Text
    // give the same REAL on every device (specification 44 NX-04).
    let mut cx = cx();
    for step in 0..20_000i32 {
        #[allow(clippy::cast_precision_loss)]
        let x = -10.0f32 + step as f32 * 0.001;
        let expected_sin = libm::sin(f64::from(x)) as f32;
        let expected_exp = libm::exp(f64::from(x)) as f32;
        assert_eq!(
            math::sin(&mut cx, x).map(f32::to_bits),
            Ok(expected_sin.to_bits()),
            "SIN({x})"
        );
        assert_eq!(
            math::exp(&mut cx, x).map(f32::to_bits),
            Ok(expected_exp.to_bits()),
            "EXP({x})"
        );
    }
}
