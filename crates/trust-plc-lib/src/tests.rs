//! The library's results and faults, each worked out by hand from specification 05, 07 and 08 and
//! the runtime rules this library carries over (specification 44 §6.8, §9).

use core::cell::Cell;
use core::cmp::Ordering;

use trust_plc_abi::kernel::WITHOUT_DEBUGGER;
use trust_plc_abi::{Cx, Fault, FaultCode, Plain, Str};

use crate::*;

fn cx() -> Cx {
    Cx::new(&WITHOUT_DEBUGGER)
}

fn code<T: core::fmt::Debug>(result: Result<T, Fault>) -> u32 {
    result.expect_err("a fault").raw()
}

const OVERFLOW: u32 = FaultCode::Overflow as u32;

/// Integers are checked in their own type (specification 05 §3): `INT` 32,767 + 1 overflows, where
/// a wrapping add would give -32,768; `/` truncates toward zero and `MOD` takes the dividend's
/// sign; `MIN MOD -1` is 0 where Rust's remainder refuses it; `MIN / -1` overflows.
#[test]
fn integers_fault_where_the_type_ends() {
    let mut cx = cx();
    assert_eq!(code(add(&mut cx, 32_767i16, 1)), OVERFLOW);
    assert_eq!(cx.fault.detail[..2], [32_767, 1]);
    assert_eq!(add(&mut cx, 32_766i16, 1), Ok(32_767));
    assert_eq!(code(sub(&mut cx, 0u16, 1)), OVERFLOW);
    assert_eq!(code(mul(&mut cx, 256i16, 128)), OVERFLOW);
    assert_eq!(div(&mut cx, -7i32, 2), Ok(-3));
    assert_eq!(rem(&mut cx, -7i32, 2), Ok(-1));
    assert_eq!(rem(&mut cx, 7i32, -2), Ok(1));
    assert_eq!(rem(&mut cx, i16::MIN, -1), Ok(0));
    assert_eq!(code(div(&mut cx, i16::MIN, -1)), OVERFLOW);
    assert_eq!(
        code(div(&mut cx, 5i32, 0)),
        FaultCode::DivisionByZero as u32
    );
    assert_eq!(code(rem(&mut cx, 5u8, 0)), FaultCode::ModuloByZero as u32);
    assert_eq!(pow(&mut cx, 2i32, 10), Ok(1024));
    assert_eq!(code(pow(&mut cx, 2i32, -1)), FaultCode::TypeMismatch as u32);
    assert_eq!(code(pow(&mut cx, 1u64, 5_000_000_000)), OVERFLOW);
    assert_eq!(code(pow(&mut cx, 2i16, 15)), OVERFLOW);
    assert_eq!(code(neg(&mut cx, i8::MIN)), OVERFLOW);
    assert_eq!(code(abs(&mut cx, i16::MIN)), OVERFLOW);
    assert_eq!(abs(&mut cx, -5i64), Ok(5));
}

/// A real result must be finite: `REAL` 3E38 * 10 is an overflow, not an infinity, and a division
/// by zero is a fault even for reals, where IEEE 754 would give an infinity.
#[test]
fn reals_fault_on_infinities_and_division_by_zero() {
    let mut cx = cx();
    assert_eq!(code(fmul(&mut cx, 3.0e38f32, 10.0)), OVERFLOW);
    assert_eq!(fmul(&mut cx, 3.0e38f64, 10.0), Ok(3.0e39));
    assert_eq!(
        code(fdiv(&mut cx, 1.0f32, 0.0)),
        FaultCode::DivisionByZero as u32
    );
    assert_eq!(fdiv(&mut cx, 1.0f32, 4.0), Ok(0.25));
    assert_eq!(fadd(&mut cx, 0.1f32, 0.2), Ok(0.1f32 + 0.2f32));
    // 2 ** 10 and 9 ** 0.5 are exact in binary64 and in binary32.
    assert_eq!(fpow(&mut cx, 2.0f32, 10.0), Ok(1024.0));
    assert_eq!(fpow(&mut cx, 9.0f32, 0.5), Ok(3.0));
    assert_eq!(code(fpow(&mut cx, 10.0f32, 39.0)), OVERFLOW);
}

/// A real becomes an integer by rounding to the nearest, a tie to the even one (Table 22): 2.5 is
/// 2, 3.5 is 4, -2.5 is -2, and 32,767.5 rounds to 32,768, which is no `INT`. `TRUNC` cuts toward
/// zero. Rounding half away from zero would give 3 for 2.5.
#[test]
fn reals_round_to_even_integers() {
    let mut cx = cx();
    let to_int = |cx: &mut Cx, value: f64| real_to_int::<i16>(cx, value, Rounding::Nearest);
    assert_eq!(to_int(&mut cx, 2.5), Ok(2));
    assert_eq!(to_int(&mut cx, 3.5), Ok(4));
    assert_eq!(to_int(&mut cx, -2.5), Ok(-2));
    assert_eq!(to_int(&mut cx, -3.5), Ok(-4));
    assert_eq!(to_int(&mut cx, 2.4), Ok(2));
    assert_eq!(to_int(&mut cx, 2.6), Ok(3));
    assert_eq!(to_int(&mut cx, -0.5), Ok(0));
    assert_eq!(code(to_int(&mut cx, 32_767.5)), OVERFLOW);
    assert_eq!(to_int(&mut cx, -32_768.5), Ok(-32_768));
    assert_eq!(code(to_int(&mut cx, f64::NAN)), OVERFLOW);
    assert_eq!(
        real_to_int::<i32>(&mut cx, -2.7, Rounding::Truncate),
        Ok(-2)
    );
    assert_eq!(real_to_int::<u8>(&mut cx, -0.4, Rounding::Nearest), Ok(0));
    assert_eq!(
        code(real_to_int::<u8>(&mut cx, -0.6, Rounding::Nearest)),
        OVERFLOW
    );
    assert_eq!(
        real_to_int::<u64>(&mut cx, 1.8e19, Rounding::Nearest),
        Ok(18_000_000_000_000_000_000)
    );
    assert_eq!(round_ties_to_even(0.5), 0.0);
    assert_eq!(round_ties_to_even(1.5), 2.0);
    assert!(round_ties_to_even(-0.5).is_sign_negative());
}

/// An integer converts when its value fits the destination, else it overflows; an integer becomes a
/// `REAL` through binary64, as the runtime converts: 2^60 + 2^36 + 1 rounds to 2^60 + 2^36 in
/// binary64, a tie in binary32 that goes to the even 2^60, where one direct rounding would give
/// 2^60 + 2^37.
#[test]
fn integers_convert_by_value() {
    let mut cx = cx();
    assert_eq!(code(sint_to::<i16>(&mut cx, 40_000)), OVERFLOW);
    assert_eq!(sint_to::<i16>(&mut cx, -32_768), Ok(-32_768));
    assert_eq!(code(sint_to::<u16>(&mut cx, -1)), OVERFLOW);
    assert_eq!(code(uint_to::<i16>(&mut cx, 70_000)), OVERFLOW);
    assert_eq!(uint_to::<u32>(&mut cx, 70_000), Ok(70_000));
    let wide = (1i64 << 60) + (1 << 36) + 1;
    #[allow(clippy::cast_precision_loss)]
    let expected = (1u64 << 60) as f32;
    assert_eq!(sint_to_real32(wide).to_bits(), expected.to_bits());
    assert_eq!(sint_to_real64(-3), -3.0);
    assert_eq!(uint_to_real32(16_777_217), 16_777_216.0);
    assert_eq!(
        real32_to_real::<f64>(&mut cx, 0.1f32),
        Ok(f64::from(0.1f32))
    );
    assert_eq!(code(real64_to_real32::<f32>(&mut cx, 1.0e39)), OVERFLOW);
    assert_eq!(real64_to_real32::<f32>(&mut cx, 0.25), Ok(0.25));
    assert_eq!(bits_to_real32(&mut cx, 0x3F80_0000), Ok(1.0));
    assert_eq!(code(bits_to_real32(&mut cx, 0x7F80_0000)), OVERFLOW);
    assert_eq!(bits_to_real64(&mut cx, 0x4000_0000_0000_0000), Ok(2.0));
}

/// `TIME_TO_DWORD` gives whole milliseconds (specification 07 §2.8): T#1500ms is 1500, 1,999,999 ns
/// is 1, a negative duration and one beyond `u32::MAX` milliseconds overflow.
#[test]
fn durations_convert_to_whole_milliseconds() {
    let mut cx = cx();
    assert_eq!(time_to_dword(&mut cx, 1_500_000_000), Ok(1500));
    assert_eq!(time_to_dword(&mut cx, 1_999_999), Ok(1));
    assert_eq!(code(time_to_dword(&mut cx, -1)), OVERFLOW);
    assert_eq!(
        code(time_to_dword(&mut cx, 4_294_967_296_000_000)),
        OVERFLOW
    );
    assert_eq!(time_to_dword(&mut cx, 4_294_967_295_999_999), Ok(u32::MAX));
    assert_eq!(dword_to_time(1500), 1_500_000_000);
}

fn text<const N: usize>(value: &Str<N>) -> ([u8; 64], usize) {
    let view = value.view();
    let mut bytes = [0u8; 64];
    for (index, byte) in bytes.iter_mut().enumerate().take(view.len()) {
        *byte = view.byte(index);
    }
    (bytes, view.len())
}

fn assert_text<const N: usize>(value: &Str<N>, expected: &str) {
    let (bytes, len) = text(value);
    assert_eq!(&bytes[..len], expected.as_bytes());
}

/// A text is cut at its variable's capacity at the last whole character (specification 02 §6):
/// 'Milk LTLT' in a `STRING[4]` is 'Milk'; 'Grüße' in a `STRING[3]` is 'Gr', since 'ü' takes two
/// bytes and only one is left; a cut inside the character would leave invalid UTF-8.
#[test]
fn texts_are_cut_at_whole_characters() {
    let four = Str::<4>::new();
    assign_bytes(four.view(), b"Milk LTLT");
    assert_text(&four, "Milk");
    let three = Str::<3>::new();
    assign_bytes(three.view(), "Grüße".as_bytes());
    assert_text(&three, "Gr");
    let wide = Str::<16>::new();
    assign_bytes(wide.view(), "Grüße".as_bytes());
    assert_eq!(len(wide.view()), 5);
    assign(four.view(), wide.view());
    assert_text(&four, "Grü");
    // A string assigned to itself keeps its value.
    assign(wide.view(), wide.view());
    assert_text(&wide, "Grüße");
}

static LONG: [u8; 40_000] = [b'x'; 40_000];

/// `CONCAT` writes its parts one after the other and is cut at the destination; a result longer
/// than 65,535 bytes is a fault before anything is written (specification 07 §8.1).
#[test]
fn concat_joins_and_cuts() {
    let mut cx = cx();
    let value = Str::<12>::new();
    assign_bytes(value.view(), b"63.0");
    let line = Str::<10>::new();
    concat(
        &mut cx,
        line.view(),
        &[Piece::Bytes(b"Heating "), Piece::Str(value.view())],
    )
    .unwrap();
    assert_text(&line, "Heating 63");
    assert_eq!(
        code(concat(
            &mut cx,
            line.view(),
            &[Piece::Bytes(&LONG), Piece::Bytes(&LONG)]
        )),
        FaultCode::StringCapacityExceeded as u32
    );
    assert_text(&line, "Heating 63");
    assert_eq!(cx.fault.detail[0], 80_000);
}

/// Integers become their decimal digits with a minus sign: -120, 0 and the 20 characters of the
/// smallest `LINT`.
#[test]
fn integers_become_decimal_text() {
    let value = Str::<24>::new();
    write_int(value.view(), -120);
    assert_text(&value, "-120");
    write_uint(value.view(), 0);
    assert_text(&value, "0");
    write_int(value.view(), i64::MIN);
    assert_text(&value, "-9223372036854775808");
    write_uint(value.view(), u64::MAX);
    assert_text(&value, "18446744073709551615");
    let short = Str::<2>::new();
    write_uint(short.view(), 12_345);
    assert_text(&short, "12");
}

/// Strings compare by their bytes, a prefix first.
#[test]
fn strings_compare_by_bytes() {
    let a = Str::<8>::new();
    let b = Str::<8>::new();
    assign_bytes(a.view(), b"abc");
    assign_bytes(b.view(), b"abd");
    assert_eq!(compare(a.view(), b.view()), Ordering::Less);
    assign_bytes(b.view(), b"ab");
    assert_eq!(compare(a.view(), b.view()), Ordering::Greater);
    assign_bytes(b.view(), b"abc");
    assert_eq!(compare(a.view(), b.view()), Ordering::Equal);
}

/// A string of capacity `N` holding `value`.
fn string<const N: usize>(value: &str) -> Str<N> {
    let text = Str::<N>::new();
    assign_bytes(text.view(), value.as_bytes());
    text
}

/// The string functions of Table 34 count characters, not bytes (DEV-017): `'Grüße'` is 7 bytes and
/// 5 characters. `LEN` is 5; `LEFT(3)` is `'Grü'` and `RIGHT(2)` `'ße'`; `MID(L 2, P 3)` is `'üß'`.
/// A count below one is no character and one past the end is every character; a position below one
/// is the first character and one past the end gives nothing. Counting bytes would give `LEN` 7 and
/// cut `'ü'` in half.
#[test]
fn string_functions_count_characters() {
    let mut cx = cx();
    let greeting = string::<16>("Grüße");
    assert_eq!(len_int(&mut cx, greeting.view()), Ok(5));
    assert_eq!(len_int(&mut cx, string::<4>("").view()), Ok(0));
    let out = Str::<16>::new();
    left(&mut cx, out.view(), greeting.view(), 3).unwrap();
    assert_text(&out, "Grü");
    left(&mut cx, out.view(), greeting.view(), -2).unwrap();
    assert_text(&out, "");
    left(&mut cx, out.view(), greeting.view(), 99).unwrap();
    assert_text(&out, "Grüße");
    right(&mut cx, out.view(), greeting.view(), 2).unwrap();
    assert_text(&out, "ße");
    right(&mut cx, out.view(), greeting.view(), 0).unwrap();
    assert_text(&out, "");
    mid(&mut cx, out.view(), greeting.view(), 2, 3).unwrap();
    assert_text(&out, "üß");
    mid(&mut cx, out.view(), greeting.view(), 2, 0).unwrap();
    assert_text(&out, "Gr");
    mid(&mut cx, out.view(), greeting.view(), 10, 4).unwrap();
    assert_text(&out, "ße");
    mid(&mut cx, out.view(), greeting.view(), 2, 6).unwrap();
    assert_text(&out, "");
    mid(&mut cx, out.view(), greeting.view(), 0, 1).unwrap();
    assert_text(&out, "");
}

/// `INSERT(IN1, IN2, P)` puts IN2 after the P-th character, at the start for P below one and at the
/// end past it; `DELETE(IN, L, P)` and `REPLACE(IN1, IN2, L, P)` leave the text as it is from a
/// position past the end, and `REPLACE` with L 0 inserts at P. `'ABCD'` with `'xy'` after 2 is
/// `'ABxyCD'`; `'ABCDE'` less 2 from 2 is `'ADE'`, and with `'xy'` for 2 from 2 `'AxyDE'`.
#[test]
fn string_functions_insert_delete_and_replace_by_position() {
    let mut cx = cx();
    let out = Str::<16>::new();
    let abcd = string::<8>("ABCD");
    let abcde = string::<8>("ABCDE");
    let xy = string::<4>("xy");
    insert(&mut cx, out.view(), abcd.view(), xy.view(), 2).unwrap();
    assert_text(&out, "ABxyCD");
    insert(&mut cx, out.view(), abcd.view(), xy.view(), 0).unwrap();
    assert_text(&out, "xyABCD");
    insert(&mut cx, out.view(), abcd.view(), xy.view(), 10).unwrap();
    assert_text(&out, "ABCDxy");
    delete(&mut cx, out.view(), abcde.view(), 2, 2).unwrap();
    assert_text(&out, "ADE");
    delete(&mut cx, out.view(), abcde.view(), 0, 2).unwrap();
    assert_text(&out, "ABCDE");
    delete(&mut cx, out.view(), abcde.view(), 2, 6).unwrap();
    assert_text(&out, "ABCDE");
    delete(&mut cx, out.view(), abcde.view(), 2, 0).unwrap();
    assert_text(&out, "CDE");
    replace(&mut cx, out.view(), abcde.view(), xy.view(), 2, 2).unwrap();
    assert_text(&out, "AxyDE");
    replace(&mut cx, out.view(), abcde.view(), xy.view(), 0, 2).unwrap();
    assert_text(&out, "AxyBCDE");
    replace(&mut cx, out.view(), abcde.view(), xy.view(), 2, 6).unwrap();
    assert_text(&out, "ABCDE");
}

/// A string function's result longer than 65,535 bytes is a fault before anything is written
/// (specification 07 §8.1): two texts of 40,000 bytes give 80,000.
#[test]
fn string_functions_refuse_results_above_the_maximum() {
    let mut cx = cx();
    let long = Str::<40_000>::new();
    assign_bytes(long.view(), &LONG);
    let out = string::<8>("kept");
    assert_eq!(
        code(insert(&mut cx, out.view(), long.view(), long.view(), 1)),
        FaultCode::StringCapacityExceeded as u32
    );
    assert_eq!(cx.fault.detail[0], 80_000);
    assert_text(&out, "kept");
    // An empty text inserted keeps the result within the maximum.
    let empty = Str::<4>::new();
    insert(&mut cx, out.view(), long.view(), empty.view(), 1).unwrap();
    assert_text(&out, "xxxxxxxx");
}

/// `FIND` gives the character position of the first occurrence, 0 for none and 1 for an empty
/// text: `'CA'` in `'ABCABC'` at 3, `'ße'` in `'Grüße'` at 4 (byte 5), `'AB'` in `'A'` at 0.
#[test]
fn find_counts_characters_before_the_match() {
    let mut cx = cx();
    let find_in = |cx: &mut Cx, text: &str, wanted: &str| {
        find(cx, string::<16>(text).view(), string::<16>(wanted).view())
    };
    assert_eq!(find_in(&mut cx, "ABCABC", "CA"), Ok(3));
    assert_eq!(find_in(&mut cx, "Grüße", "ße"), Ok(4));
    assert_eq!(find_in(&mut cx, "ABC", "x"), Ok(0));
    assert_eq!(find_in(&mut cx, "ABC", ""), Ok(1));
    assert_eq!(find_in(&mut cx, "", ""), Ok(1));
    assert_eq!(find_in(&mut cx, "A", "AB"), Ok(0));
}

/// `text[i]` is the i-th character, from 1 (specification 02 §6): `'Grüße'`[3] is `'ü'`, 252. An
/// index below one or past the fifth character is out of bounds, naming the index and 1 to the
/// count, or to the largest `LINT` below one, as the runtime does; `'€'` is no `CHAR` and
/// overflows.
#[test]
fn a_character_of_a_string_counts_from_one() {
    let mut cx = cx();
    let greeting = string::<16>("Grüße");
    assert_eq!(char_at(&mut cx, greeting.view(), 1), Ok(b'G'));
    assert_eq!(char_at(&mut cx, greeting.view(), 3), Ok(252));
    assert_eq!(
        code(char_at(&mut cx, greeting.view(), 0)),
        FaultCode::IndexOutOfBounds as u32
    );
    assert_eq!(cx.fault.detail, [0, 1, i64::MAX]);
    assert_eq!(
        code(char_at(&mut cx, greeting.view(), 6)),
        FaultCode::IndexOutOfBounds as u32
    );
    assert_eq!(cx.fault.detail, [6, 1, 5]);
    assert_eq!(code(char_at(&mut cx, string::<4>("€").view(), 1)), OVERFLOW);
}

/// `text[i] := c` replaces the i-th character; `'ü'` takes two bytes, so `'ABC'` becomes `'AüC'`, 4
/// bytes, and in a `STRING[3]` it is cut to `'Aü'` at the last whole character. Replacing `'ü'` by
/// `'x'` moves the rest back. An index out of bounds leaves the text as it was.
#[test]
fn writing_a_character_keeps_whole_characters() {
    let mut cx = cx();
    let roomy = string::<8>("ABC");
    set_char(&mut cx, roomy.view(), 2, 252).unwrap();
    assert_text(&roomy, "AüC");
    set_char(&mut cx, roomy.view(), 2, b'x').unwrap();
    assert_text(&roomy, "AxC");
    let tight = string::<3>("ABC");
    set_char(&mut cx, tight.view(), 2, 252).unwrap();
    assert_text(&tight, "Aü");
    set_char(&mut cx, tight.view(), 1, 233).unwrap();
    assert_text(&tight, "é");
    assert_eq!(
        code(set_char(&mut cx, tight.view(), 3, b'y')),
        FaultCode::IndexOutOfBounds as u32
    );
    assert_text(&tight, "é");
}

const MS: i64 = 1_000_000;

/// `TON` with `PT` T#100ms (specification 08 §5): the first call measures nothing, `Q` rises when
/// 100 ms have passed with `IN` TRUE, `ET` stops at `PT`, and `IN` FALSE resets both.
#[test]
fn ton_delays_the_rise() {
    let timer = Ton::ZEROED;
    timer.PT.set(100 * MS);
    let step = |now: i64, input: bool| {
        timer.IN.set(u8::from(input));
        ton(&timer, now * MS);
        (timer.Q.get(), timer.ET.get() / MS)
    };
    assert_eq!(step(0, true), (0, 0));
    assert_eq!(step(50, true), (0, 50));
    assert_eq!(step(100, true), (1, 100));
    assert_eq!(step(150, true), (1, 100));
    assert_eq!(step(160, false), (0, 0));
    // A negative preset counts as zero: Q at once.
    timer.PT.set(-5);
    assert_eq!(step(170, true), (1, 0));
}

/// `TOF` with `PT` T#30ms: the time from the last call with `IN` TRUE counts, `Q` falls when 30 ms
/// have passed with `IN` FALSE and `ET` stays at `PT`.
#[test]
fn tof_delays_the_fall() {
    let timer = Tof::ZEROED;
    timer.PT.set(30 * MS);
    let step = |now: i64, input: bool| {
        timer.IN.set(u8::from(input));
        tof(&timer, now * MS);
        (timer.Q.get(), timer.ET.get() / MS)
    };
    assert_eq!(step(0, true), (1, 0));
    assert_eq!(step(10, false), (1, 10));
    assert_eq!(step(20, false), (1, 20));
    assert_eq!(step(40, false), (0, 30));
    assert_eq!(step(50, false), (0, 30));
    assert_eq!(step(60, true), (1, 0));
}

/// `TP` with `PT` T#30ms: a rising edge starts a pulse of 30 ms whatever `IN` does; the call that
/// ends it keeps `ET` at `PT`, the next call with `IN` FALSE resets it.
#[test]
fn tp_makes_a_pulse() {
    let timer = Tp::ZEROED;
    timer.PT.set(30 * MS);
    let step = |now: i64, input: bool| {
        timer.IN.set(u8::from(input));
        tp(&timer, now * MS);
        (timer.Q.get(), timer.ET.get() / MS)
    };
    assert_eq!(step(0, true), (1, 0));
    assert_eq!(step(10, false), (1, 10));
    assert_eq!(step(40, false), (0, 30));
    assert_eq!(step(50, false), (0, 0));
}

/// `R_TRIG` gives one TRUE on a rising `CLK`; `F_TRIG` gives TRUE at its first call with `CLK`
/// FALSE, as the standard's body with `M` FALSE at the start does, and then on each falling `CLK`.
#[test]
fn edges_follow_the_standard_bodies() {
    let rising = RTrig::ZEROED;
    let mut seen = [0u8; 4];
    for (index, clk) in [false, true, true, false].into_iter().enumerate() {
        rising.CLK.set(u8::from(clk));
        r_trig(&rising);
        seen[index] = rising.Q.get();
    }
    assert_eq!(seen, [0, 1, 0, 0]);
    let falling = FTrig::ZEROED;
    for (index, clk) in [false, false, true, false].into_iter().enumerate() {
        falling.CLK.set(u8::from(clk));
        f_trig(&falling);
        seen[index] = falling.Q.get();
    }
    assert_eq!(seen, [1, 0, 0, 1]);
}

/// `LIMIT` raises before it caps, so a minimum above the maximum gives the maximum; `MAX` and `MIN`
/// keep the first of equal values, which shows in the sign of a zero.
#[test]
fn selection_keeps_the_runtime_order() {
    assert_eq!(limit(1, 5, 2), 2);
    assert_eq!(limit(1, 0, 2), 1);
    assert_eq!(limit(3, 5, 2), 2);
    assert_eq!(limit(3, 1, 2), 2);
    assert!(max(-0.0f64, 0.0).is_sign_negative());
    assert!(min(0.0f64, -0.0).is_sign_positive());
    assert_eq!(max(3, 7), 7);
    assert_eq!(min(3, 7), 3);
}

/// The function block types lie as the type table will describe them: inputs, outputs, then the
/// hidden state, each at its natural alignment.
#[test]
fn function_blocks_lie_in_declaration_order() {
    assert_eq!(core::mem::offset_of!(Ton, IN), 0);
    assert_eq!(core::mem::offset_of!(Ton, PT), 8);
    assert_eq!(core::mem::offset_of!(Ton, Q), 16);
    assert_eq!(core::mem::offset_of!(Ton, ET), 24);
    assert_eq!(core::mem::size_of::<Ton>(), 48);
    assert_eq!(core::mem::size_of::<RTrig>(), 3);
    let trigger = RTrig::ZEROED;
    let _: &Cell<u8> = &trigger.M;
}

/// Dates count days from 1970-01-01, a Thursday (specification 07 §9): 2026-10-23 is day 20,749
/// (56 years with 14 leap days, then 295 days), 1,792,713,600,000 ms, a Friday; 2000-02-29 is day
/// 11,016; 1900 is no leap year; a split counts toward the calendar, so the millisecond before 1970
/// is 1969-12-31 23:59:59.999; a `TIME_OF_DAY` is not wrapped, so -1 ms splits into a millisecond
/// of -1; the day of the earliest `DATE_AND_TIME` has no midnight in range.
#[test]
fn dates_count_days_from_1970() {
    let mut cx = cx();
    assert_eq!(concat_date(&mut cx, 2026, 10, 23), Ok(1_792_713_600_000));
    assert_eq!(day_of_week(1_792_713_600_000), 5);
    assert_eq!(day_of_week(0), 4);
    assert_eq!(day_of_week(-86_400_000), 3);
    assert_eq!(concat_date(&mut cx, 2000, 2, 29), Ok(951_782_400_000));
    assert_eq!(split_date(951_782_400_000), [2000, 2, 29]);
    assert_eq!(code(concat_date(&mut cx, 1900, 2, 29)), OVERFLOW);
    assert_eq!(code(concat_date(&mut cx, 2000, 13, 1)), OVERFLOW);
    assert_eq!(split_dt(-1), [1969, 12, 31, 23, 59, 59, 999]);
    assert_eq!(concat_dt(&mut cx, 1969, 12, 31, 23, 59, 59, 999), Ok(-1));
    assert_eq!(concat_tod(&mut cx, 23, 59, 59, 999), Ok(86_399_999));
    assert_eq!(code(concat_tod(&mut cx, 24, 0, 0, 0)), OVERFLOW);
    assert_eq!(concat_ltod(&mut cx, 0, 0, 1, 5), Ok(1_005_000_000));
    assert_eq!(split_tod(&mut cx, -1), Ok([0, 0, 0, -1]));
    assert_eq!(code(split_tod(&mut cx, i64::MAX)), OVERFLOW);
    assert_eq!(dt_to_date(&mut cx, -1), Ok(-86_400_000));
    assert_eq!(code(dt_to_date(&mut cx, i64::MIN)), OVERFLOW);
    assert_eq!(ldt_to_date(-1), -86_400_000);
    assert_eq!(ldt_to_tod(-1), 86_399_999);
    assert_eq!(nanos_to_ticks(-1), -1);
    assert_eq!(
        concat_date_ltod(&mut cx, 86_400_000, 5),
        Ok(86_400_000_000_005)
    );
}

/// A duration added to a millisecond value counts whole milliseconds toward zero, so -1.5 ms
/// moves it back by 1; a result outside the type is a date and time range fault; two dates apart
/// are nanoseconds, and 9.3E12 ms (about 295 years) is more than a `TIME` holds; a duration scaled
/// by an unsigned factor keeps its sign and reaches the most negative `TIME`; a real factor cuts
/// toward zero and an infinite one is an overflow (specification 07 §9).
#[test]
fn durations_move_dates_in_whole_milliseconds() {
    let mut cx = cx();
    let range = FaultCode::DateTimeRange as u32;
    assert_eq!(ticks_add(&mut cx, 0, -1_500_000, true), Ok(-1));
    assert_eq!(ticks_add(&mut cx, 0, 1_999_999, true), Ok(1));
    assert_eq!(ticks_add(&mut cx, 10, 3_000_000, false), Ok(7));
    assert_eq!(code(ticks_add(&mut cx, i64::MAX, 1_000_000, true)), range);
    assert_eq!(code(concat_date_tod(&mut cx, i64::MAX, 1)), range);
    assert_eq!(ticks_diff(&mut cx, 86_400_000, 0), Ok(86_400_000_000_000));
    assert_eq!(code(ticks_diff(&mut cx, 9_300_000_000_000, 0)), OVERFLOW);
    assert_eq!(scale_unsigned(&mut cx, i64::MIN, 1, true), Ok(i64::MIN));
    assert_eq!(scale_unsigned(&mut cx, -5, 2, false), Ok(-2));
    assert_eq!(
        code(scale_signed(&mut cx, 5, 0, false)),
        FaultCode::DivisionByZero as u32
    );
    assert_eq!(scale_real(&mut cx, 1_000, 0.5, true), Ok(500));
    assert_eq!(scale_real(&mut cx, -3, 2.0, false), Ok(-1));
    assert_eq!(
        code(scale_real(&mut cx, 1_000, f64::INFINITY, true)),
        OVERFLOW
    );
    assert_eq!(code(scale_real(&mut cx, i64::MAX, 2.0, true)), OVERFLOW);
    assert_eq!(
        code(scale_real(&mut cx, 1_000, 0.0, false)),
        FaultCode::DivisionByZero as u32
    );
}
