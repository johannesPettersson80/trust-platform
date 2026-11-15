//! The SDK computes and faults as Structured Text does (specification 48 §5.3, §5.5; specification
//! 44 §6.5). Every expected value is the standard's or the specification's, worked out by hand.

// The expected values are exact: the standard's and the specification's, compared bit for bit.
#![allow(clippy::float_cmp)]

use trust_plc_abi::kernel::WITHOUT_DEBUGGER;

use crate::{
    checked, math, Byte, Char, Cx, Date, Dt, Dword, Fault, FaultCode, LDate, LTime, LTod, Ldt,
    Stored, Time, Tod, Word,
};

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

#[test]
fn bit_strings_combine_bit_by_bit_and_durations_fault_beyond_their_range() {
    // Catches a bit string that computes as an integer, or a duration that wraps: IEC §6.4.2,
    // Table 10 gives BYTE to LWORD AND, OR, XOR and NOT bit by bit, and specification 05 §3 faults
    // a duration beyond its type with `Overflow`; TIME is stored in nanoseconds (specification 44
    // §6.2).
    let (a, b) = (Byte(0b1100_1010), Byte(0b1010_0110));
    assert_eq!(a & b, Byte(0b1000_0010));
    assert_eq!(a | b, Byte(0b1110_1110));
    assert_eq!(a ^ b, Byte(0b0110_1100));
    assert_eq!(!a, Byte(0b0011_0101));
    assert!(a.bit(1) && !a.bit(0) && !a.bit(8));
    assert_eq!(Word(0).with_bit(15, true), Word(0x8000));
    assert_eq!(Word(0x8000).with_bit(15, false), Word(0));
    assert_eq!(Dword(1).with_bit(32, true), Dword(1));
    let mut cx = cx();
    let half = Time::from_millis(&mut cx, 500).expect("500 ms");
    assert_eq!(half.as_nanos(), 500_000_000);
    assert_eq!(
        half.checked_add(&mut cx, Time::from_nanos(250_000_000)),
        Ok(Time::from_nanos(750_000_000))
    );
    assert_eq!(
        code(Time::from_nanos(i64::MAX).checked_add(&mut cx, Time::from_nanos(1))),
        FaultCode::Overflow as u32
    );
    assert_eq!(
        code(Time::from_millis(&mut cx, i64::MAX)),
        FaultCode::Overflow as u32
    );
    assert_eq!(Stored::into_raw(half), 500_000_000_i64);
    assert_eq!(<Char as Stored>::from_raw(b'A'), Char(b'A'));
}

#[test]
fn dates_are_built_split_and_moved_as_structured_text_does() {
    // Catches a day counted off by one, a leap day lost, a time of day wrapped at midnight, a
    // duration rounded instead of cut, a long form left in milliseconds and a date that does not
    // exist accepted: IEC Tables 36 and 37 with the runtime's encoding (specification 44 §6.2;
    // specification 07 §9), each value worked out by hand. 2024-02-29 is day 54 × 365 + 13 + 59 =
    // 19,782 after 1970-01-01, 1,709,164,800,000 ms, a Thursday (4); 12:30:15.250 is 45,015,250 ms.
    let mut cx = cx();
    let day = Date::from_ymd(&mut cx, 2024, 2, 29).expect("a leap day");
    assert_eq!(Stored::into_raw(day), 1_709_164_800_000_i64);
    assert_eq!(day.ymd(), [2024, 2, 29]);
    assert_eq!(day.day_of_week(), 4);
    assert_eq!(
        code(Date::from_ymd(&mut cx, 2023, 2, 29)),
        FaultCode::Overflow as u32
    );
    let next = Date::from_ymd(&mut cx, 2024, 3, 1).expect("the next day");
    assert_eq!(
        next.since(&mut cx, day),
        Ok(Time::from_nanos(86_400_000_000_000))
    );
    let long_day = LDate::from_ymd(&mut cx, 2024, 2, 29).expect("a long date");
    assert_eq!(Stored::into_raw(long_day), 1_709_164_800_000_000_000_i64);
    assert_eq!(long_day.ymd(), [2024, 2, 29]);
    assert_eq!(Date::default(), <Date as Stored>::from_raw(0));

    let clock = Tod::from_hms(&mut cx, 12, 30, 15, 250).expect("a time of day");
    assert_eq!(Stored::into_raw(clock), 45_015_250_i64);
    assert_eq!(clock.hms(&mut cx), Ok([12, 30, 15, 250]));
    // 23:59:59 + 2 s is 86,401,000 ms, not wrapped; + 1.9 ms adds one whole millisecond.
    let late = Tod::from_hms(&mut cx, 23, 59, 59, 0).expect("late");
    assert_eq!(
        late.checked_add(&mut cx, Time::from_nanos(2_000_000_000))
            .map(Stored::into_raw),
        Ok(86_401_000_i64)
    );
    assert_eq!(
        clock
            .checked_add(&mut cx, Time::from_nanos(1_900_000))
            .map(Stored::into_raw),
        Ok(45_015_251_i64)
    );
    assert_eq!(
        clock.checked_sub(&mut cx, Time::from_nanos(250_000_000)),
        Tod::from_hms(&mut cx, 12, 30, 15, 0)
    );
    assert_eq!(
        code(Tod::from_hms(&mut cx, 24, 0, 0, 0)),
        FaultCode::Overflow as u32
    );

    let moment = Dt::new(&mut cx, day, clock).expect("a moment");
    assert_eq!(Stored::into_raw(moment), 1_709_209_815_250_i64);
    assert_eq!(Dt::from_millis(1_709_209_815_250), moment);
    assert_eq!(moment.as_millis(), 1_709_209_815_250);
    assert_eq!(moment.parts(), [2024, 2, 29, 12, 30, 15, 250]);
    assert_eq!(moment.date(&mut cx), Ok(day));
    assert_eq!(moment.tod(), clock);
    assert_eq!(
        Dt::from_parts(&mut cx, 2024, 2, 29, 12, 30, 15, 250),
        Ok(moment)
    );
    let later = moment
        .checked_add(&mut cx, Time::from_nanos(60_000_000_000))
        .expect("a minute later");
    assert_eq!(
        later.since(&mut cx, moment),
        Ok(Time::from_nanos(60_000_000_000))
    );
    assert_eq!(
        code(Dt::from_parts(&mut cx, 2024, 2, 30, 0, 0, 0, 0)),
        FaultCode::Overflow as u32
    );
    assert_eq!(
        code(<Dt as Stored>::from_raw(i64::MAX).checked_add(&mut cx, Time::from_nanos(1_000_000))),
        FaultCode::DateTimeRange as u32
    );

    let precise = LTod::from_hms(&mut cx, 12, 30, 15, 250)
        .and_then(|tod| tod.checked_add(&mut cx, LTime::from_nanos(1)))
        .expect("a nanosecond later");
    assert_eq!(Stored::into_raw(precise), 45_015_250_000_001_i64);
    assert_eq!(LTod::from_nanos(45_015_250_000_001), precise);
    assert_eq!(precise.hms(), [12, 30, 15, 250]);
    let long_moment = Ldt::new(&mut cx, day, precise).expect("a long moment");
    assert_eq!(Stored::into_raw(long_moment), 1_709_209_815_250_000_001_i64);
    assert_eq!(long_moment.date(), day);
    assert_eq!(long_moment.tod(), precise);
    assert_eq!(long_moment.parts(), [2024, 2, 29, 12, 30, 15, 250]);
    let whole = Ldt::from_parts(&mut cx, 2024, 2, 29, 12, 30, 15, 250).expect("a long moment");
    assert_eq!(Stored::into_raw(whole), 1_709_209_815_250_000_000_i64);
    assert_eq!(long_moment.since(&mut cx, whole), Ok(LTime::from_nanos(1)));
}
