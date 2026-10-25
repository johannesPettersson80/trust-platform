//! The wide texts' results and faults, each worked out by hand from specification 02 §6, 07 §2.5
//! and §8.1 and DEV-017: a `WSTRING` holds UTF-16 code units, and its functions count characters,
//! a surrogate pair one.

use core::cmp::Ordering;

use trust_plc_abi::kernel::WITHOUT_DEBUGGER;
use trust_plc_abi::{Cx, Fault, FaultCode, Str, WStr};

use crate::*;

fn cx() -> Cx {
    Cx::new(&WITHOUT_DEBUGGER)
}

fn code<T: core::fmt::Debug>(result: Result<T, Fault>) -> u32 {
    result.expect_err("a fault").raw()
}

const OVERFLOW: u32 = FaultCode::Overflow as u32;
const OUT_OF_BOUNDS: u32 = FaultCode::IndexOutOfBounds as u32;
const TOO_LONG: u32 = FaultCode::StringCapacityExceeded as u32;

/// 40,000 `'x'`.
static LONG: [u16; 40_000] = [0x78; 40_000];

/// 20,000 `'🙂'`, U+1F642, each the pair 16#D83D 16#DE42: 40,000 code units.
static SMILES: [u16; 40_000] = {
    let mut units = [0xD83D; 40_000];
    let mut index = 1;
    while index < units.len() {
        units[index] = 0xDE42;
        index += 2;
    }
    units
};

/// The UTF-16 code units of `value`, and how many.
fn encode(value: &str) -> ([u16; 64], usize) {
    let mut units = [0u16; 64];
    for (slot, unit) in units.iter_mut().zip(value.encode_utf16()) {
        *slot = unit;
    }
    (units, value.encode_utf16().count())
}

/// A `WSTRING[N]` holding the code units `units`, lone surrogates among them.
fn wstring_units<const N: usize>(units: &[u16]) -> WStr<N> {
    let text = WStr::<N>::new();
    assign_units(text.view(), units);
    text
}

/// A `WSTRING[N]` holding `value`.
fn wstring<const N: usize>(value: &str) -> WStr<N> {
    let (units, used) = encode(value);
    wstring_units(&units[..used])
}

/// A `STRING[N]` holding `bytes`, which need not be UTF-8.
fn string<const N: usize>(bytes: &[u8]) -> Str<N> {
    let text = Str::<N>::new();
    assign_bytes(text.view(), bytes);
    text
}

/// Asserts that `text` holds exactly the code units `expected`.
fn assert_units<const N: usize>(text: &WStr<N>, expected: &[u16]) {
    let view = text.view();
    assert_eq!(view.len(), expected.len(), "{expected:X?}");
    for (index, unit) in expected.iter().enumerate() {
        assert_eq!(
            view.data[index].get(),
            *unit,
            "unit {index} of {expected:X?}"
        );
    }
}

/// Asserts that `text` holds exactly the code units of `expected`.
fn assert_wide<const N: usize>(text: &WStr<N>, expected: &str) {
    let (units, used) = encode(expected);
    assert_units(text, &units[..used]);
}

/// Asserts that `text` holds exactly the bytes `expected`.
fn assert_bytes<const N: usize>(text: &Str<N>, expected: &[u8]) {
    let view = text.view();
    assert_eq!(view.len(), expected.len(), "{expected:X?}");
    for (index, byte) in expected.iter().enumerate() {
        assert_eq!(view.byte(index), *byte, "byte {index} of {expected:X?}");
    }
}

/// A `WSTRING[n]` holds `n` code units and a longer text is cut at the last whole character
/// (specification 02 §6): `'A🙂'` is `'A'` and the pair 16#D83D 16#DE42, three code units, so a
/// `WSTRING[2]` holds `'A'` alone, where a cut at the capacity would keep 16#D83D, half a pair and
/// no character. A `WSTRING[1]` holds nothing of `'🙂'`, and a `WSTRING[3]` one `'🙂'` of
/// `'🙂🙂'`. An assignment, a concatenation and a conversion from a `STRING` cut the same way, and a
/// string assigned to itself keeps its value.
#[test]
fn truncation_never_splits_a_surrogate_pair() {
    let mut cx = cx();
    let two = WStr::<2>::new();
    assign_units(two.view(), &[0x41, 0xD83D, 0xDE42]);
    assert_units(&two, &[0x41]);
    assert_units(&wstring::<1>("🙂"), &[]);
    let roomy = wstring::<8>("A🙂");
    assert_units(&roomy, &[0x41, 0xD83D, 0xDE42]);
    assign_wide(two.view(), roomy.view());
    assert_units(&two, &[0x41]);
    let pairs = wstring::<8>("🙂🙂");
    let three = WStr::<3>::new();
    assign_wide(three.view(), pairs.view());
    assert_units(&three, &[0xD83D, 0xDE42]);
    let parts = [WPiece::Units(&[0x41]), WPiece::WStr(pairs.view())];
    concat_wide(&mut cx, three.view(), &parts).unwrap();
    assert_units(&three, &[0x41, 0xD83D, 0xDE42]);
    concat_wide(&mut cx, two.view(), &parts).unwrap();
    assert_units(&two, &[0x41]);
    string_to_wstring(two.view(), string::<8>("A🙂".as_bytes()).view());
    assert_units(&two, &[0x41]);
    assign_wide(roomy.view(), roomy.view());
    assert_units(&roomy, &[0x41, 0xD83D, 0xDE42]);
}

/// `LEN` counts characters, a surrogate pair one (DEV-017): `'A🙂ü𝄞'` is four characters in six
/// code units, where a count of code units gives 6. 20,000 `'🙂'` are 20,000 characters, an `INT`,
/// where their 40,000 code units would overflow one; 40,000 `'x'` are more characters than an `INT`
/// holds, an overflow naming the count.
#[test]
fn len_counts_a_surrogate_pair_as_one_character() {
    let mut cx = cx();
    let text = wstring::<8>("A🙂ü𝄞");
    assert_eq!(len_wide(text.view()), 4);
    assert_eq!(len_wide_int(&mut cx, text.view()), Ok(4));
    assert_eq!(len_wide_int(&mut cx, WStr::<4>::new().view()), Ok(0));
    let smiles = WStr::<40_000>::new();
    assign_units(smiles.view(), &SMILES);
    assert_eq!(len_wide_int(&mut cx, smiles.view()), Ok(20_000));
    let long = WStr::<40_000>::new();
    assign_units(long.view(), &LONG);
    assert_eq!(code(len_wide_int(&mut cx, long.view())), OVERFLOW);
    assert_eq!(cx.fault.detail[0], 40_000);
}

/// `text[i]` is the i-th character, from 1, read as a `WCHAR` (specification 02 §6): the second
/// character of `'A🙂ü'`, U+1F642, is above U+FFFF, which no `WCHAR` holds, so the read overflows
/// naming the character, where a read of code units would give the pair's first half, 16#D83D. The
/// third character is `'ü'`, 16#FC, where the third code unit is the pair's second half. An index
/// below one or past the third character is out of bounds, naming 1 to the count, or to the largest
/// `LINT` below one.
#[test]
fn a_character_above_u_ffff_read_as_a_wchar_overflows() {
    let mut cx = cx();
    let text = wstring::<8>("A🙂ü");
    assert_eq!(char_at_wide(&mut cx, text.view(), 1), Ok(0x41));
    assert_eq!(code(char_at_wide(&mut cx, text.view(), 2)), OVERFLOW);
    assert_eq!(cx.fault.detail, [0x1_F642, 0, 0]);
    assert_eq!(char_at_wide(&mut cx, text.view(), 3), Ok(0xFC));
    assert_eq!(code(char_at_wide(&mut cx, text.view(), 0)), OUT_OF_BOUNDS);
    assert_eq!(cx.fault.detail, [0, 1, i64::MAX]);
    assert_eq!(code(char_at_wide(&mut cx, text.view(), 4)), OUT_OF_BOUNDS);
    assert_eq!(cx.fault.detail, [4, 1, 3]);
}

/// Texts compare by the Unicode scalar values of their characters, a prefix first, as `STRING`s
/// compare by their UTF-8 bytes: `'Ａ'`, U+FF21, comes before `'🙂'`, U+1F642, although its code
/// unit 16#FF21 is above the pair's first, 16#D83D, so an order of code units would put it after.
/// `'b'` comes after `'ab'`, the first characters deciding before the lengths.
#[test]
fn comparison_orders_by_scalar_value_not_by_code_unit() {
    let fullwidth = wstring::<4>("Ａ");
    let smile = wstring::<4>("🙂");
    assert_eq!(compare_wide(fullwidth.view(), smile.view()), Ordering::Less);
    assert_eq!(
        compare_wide(smile.view(), fullwidth.view()),
        Ordering::Greater
    );
    let prefix = wstring::<4>("A");
    let longer = wstring::<4>("A🙂");
    assert_eq!(compare_wide(prefix.view(), longer.view()), Ordering::Less);
    assert_eq!(
        compare_wide(longer.view(), prefix.view()),
        Ordering::Greater
    );
    assert_eq!(
        compare_wide(longer.view(), wstring::<8>("A🙂").view()),
        Ordering::Equal
    );
    assert_eq!(
        compare_wide(wstring::<4>("b").view(), wstring::<4>("ab").view()),
        Ordering::Greater
    );
}

/// `LEFT`, `RIGHT` and `MID` count characters (DEV-017; specification 07 §8.1): `'a🙂b𝄞c'` is five
/// characters in seven code units. `LEFT(2)` is `'a🙂'` and `RIGHT(2)` `'𝄞c'`, where counting code
/// units would cut a pair in half, and `MID(L 2, P 2)` is `'🙂b'`. A count below one is no character
/// and one past the end is every character; a position below one is the first character and one
/// past the end gives nothing; a length past the end stops there.
#[test]
fn left_right_and_mid_count_characters_not_code_units() {
    let mut cx = cx();
    let text = wstring::<8>("a🙂b𝄞c");
    let out = WStr::<16>::new();
    left_wide(&mut cx, out.view(), text.view(), 2).unwrap();
    assert_wide(&out, "a🙂");
    left_wide(&mut cx, out.view(), text.view(), -2).unwrap();
    assert_wide(&out, "");
    left_wide(&mut cx, out.view(), text.view(), 99).unwrap();
    assert_wide(&out, "a🙂b𝄞c");
    right_wide(&mut cx, out.view(), text.view(), 2).unwrap();
    assert_wide(&out, "𝄞c");
    right_wide(&mut cx, out.view(), text.view(), 0).unwrap();
    assert_wide(&out, "");
    right_wide(&mut cx, out.view(), text.view(), i64::MAX).unwrap();
    assert_wide(&out, "a🙂b𝄞c");
    mid_wide(&mut cx, out.view(), text.view(), 2, 2).unwrap();
    assert_wide(&out, "🙂b");
    mid_wide(&mut cx, out.view(), text.view(), 2, 0).unwrap();
    assert_wide(&out, "a🙂");
    mid_wide(&mut cx, out.view(), text.view(), 10, 4).unwrap();
    assert_wide(&out, "𝄞c");
    mid_wide(&mut cx, out.view(), text.view(), 2, 6).unwrap();
    assert_wide(&out, "");
    mid_wide(&mut cx, out.view(), text.view(), 0, 1).unwrap();
    assert_wide(&out, "");
}

/// `INSERT(IN1, IN2, P)` puts IN2 after the P-th character, at the start for P below one and at
/// the end from P = `LEN(IN1)`; `DELETE` and `REPLACE` leave the text as it is from a position past
/// the end, `DELETE` with L below one too, and `REPLACE` with L below one inserts at P
/// (specification 07 §8.1). `'🙂B𝄞D'` is four characters in six code units: `'x'` after its first
/// character is `'🙂xB𝄞D'`, where after its first code unit it would split the pair; without its
/// third character it is `'🙂BD'`, and with `'x'` for its second and third `'🙂xD'`.
#[test]
fn insert_delete_and_replace_count_characters_not_code_units() {
    let mut cx = cx();
    let out = WStr::<16>::new();
    let text = wstring::<8>("🙂B𝄞D");
    let inserted = wstring::<4>("x");
    insert_wide(&mut cx, out.view(), text.view(), inserted.view(), 1).unwrap();
    assert_wide(&out, "🙂xB𝄞D");
    insert_wide(&mut cx, out.view(), text.view(), inserted.view(), 0).unwrap();
    assert_wide(&out, "x🙂B𝄞D");
    insert_wide(&mut cx, out.view(), text.view(), inserted.view(), 4).unwrap();
    assert_wide(&out, "🙂B𝄞Dx");
    insert_wide(&mut cx, out.view(), text.view(), inserted.view(), 10).unwrap();
    assert_wide(&out, "🙂B𝄞Dx");
    delete_wide(&mut cx, out.view(), text.view(), 1, 3).unwrap();
    assert_wide(&out, "🙂BD");
    delete_wide(&mut cx, out.view(), text.view(), 2, 0).unwrap();
    assert_wide(&out, "𝄞D");
    delete_wide(&mut cx, out.view(), text.view(), 9, 3).unwrap();
    assert_wide(&out, "🙂B");
    delete_wide(&mut cx, out.view(), text.view(), 0, 2).unwrap();
    assert_wide(&out, "🙂B𝄞D");
    delete_wide(&mut cx, out.view(), text.view(), 1, 5).unwrap();
    assert_wide(&out, "🙂B𝄞D");
    replace_wide(&mut cx, out.view(), text.view(), inserted.view(), 2, 2).unwrap();
    assert_wide(&out, "🙂xD");
    replace_wide(&mut cx, out.view(), text.view(), inserted.view(), 1, -3).unwrap();
    assert_wide(&out, "xB𝄞D");
    replace_wide(&mut cx, out.view(), text.view(), inserted.view(), 0, 3).unwrap();
    assert_wide(&out, "🙂Bx𝄞D");
    replace_wide(&mut cx, out.view(), text.view(), inserted.view(), 1, 5).unwrap();
    assert_wide(&out, "🙂B𝄞D");
}

/// A string function's result longer than 65,535 code units is a fault before anything is written
/// (specification 07 §8.1): two texts of 40,000 code units inserted give 80,000, and one replacing
/// a character of the other 79,999. 40,000 and 25,535 code units join, 65,535 in all, where 40,000
/// and 25,536 do not.
#[test]
fn results_above_65535_code_units_are_refused() {
    let mut cx = cx();
    let long = WStr::<40_000>::new();
    assign_units(long.view(), &LONG);
    let out = wstring::<8>("kept");
    assert_eq!(
        code(insert_wide(
            &mut cx,
            out.view(),
            long.view(),
            long.view(),
            1
        )),
        TOO_LONG
    );
    assert_eq!(cx.fault.detail[0], 80_000);
    assert_wide(&out, "kept");
    assert_eq!(
        code(replace_wide(
            &mut cx,
            out.view(),
            long.view(),
            long.view(),
            1,
            1
        )),
        TOO_LONG
    );
    assert_eq!(cx.fault.detail[0], 79_999);
    assert_wide(&out, "kept");
    let over = [WPiece::Units(&LONG), WPiece::Units(&LONG[..25_536])];
    assert_eq!(code(concat_wide(&mut cx, out.view(), &over)), TOO_LONG);
    assert_eq!(cx.fault.detail[0], 65_536);
    assert_wide(&out, "kept");
    let most = [WPiece::Units(&LONG), WPiece::Units(&LONG[..25_535])];
    concat_wide(&mut cx, out.view(), &most).unwrap();
    assert_wide(&out, "xxxxxxxx");
    // An empty text inserted keeps the result within the maximum.
    let kept = wstring::<8>("kept");
    insert_wide(
        &mut cx,
        kept.view(),
        long.view(),
        WStr::<4>::new().view(),
        1,
    )
    .unwrap();
    assert_wide(&kept, "xxxxxxxx");
}

/// `FIND` gives the character position of the first occurrence, 0 for none and 1 for an empty text
/// (specification 07 §8.1): `'b'` in `'a🙂b🙂c'` is at 3, after the pair, where a count of code
/// units gives 4, and `'🙂c'` at 4. Half of a pair is no character of the text, so neither
/// 16#D83D nor 16#DE42 alone is found in it, where a search of code units finds both.
#[test]
fn find_counts_characters_and_never_matches_half_a_pair() {
    let mut cx = cx();
    let text = wstring::<8>("a🙂b🙂c");
    let find_in =
        |cx: &mut Cx, wanted: &[u16]| find_wide(cx, text.view(), wstring_units::<8>(wanted).view());
    assert_eq!(find_in(&mut cx, &[0x62]), Ok(3));
    assert_eq!(find_in(&mut cx, &[0xD83D, 0xDE42, 0x63]), Ok(4));
    assert_eq!(find_in(&mut cx, &[0xD83D, 0xDE42]), Ok(2));
    assert_eq!(find_in(&mut cx, &[0x78]), Ok(0));
    assert_eq!(find_in(&mut cx, &[]), Ok(1));
    assert_eq!(find_in(&mut cx, &[0xD83D]), Ok(0));
    assert_eq!(find_in(&mut cx, &[0xDE42]), Ok(0));
    let empty = WStr::<4>::new();
    assert_eq!(find_wide(&mut cx, empty.view(), empty.view()), Ok(1));
    assert_eq!(
        find_wide(&mut cx, wstring::<4>("a").view(), wstring::<4>("ab").view()),
        Ok(0)
    );
}

/// `text[i] := c` replaces the i-th character by one code unit (specification 02 §6): the second
/// character of `'a🙂c'` is the pair, so `'b'` written there gives `'abc'`, three code units, the
/// rest moved back, where a write of the second code unit would leave half the pair. `'Ä'` over the
/// `'a'` of a full `WSTRING[3]` `'abc'` keeps the text whole, one code unit for one. An index out of
/// bounds, and half of a surrogate pair, which is no character, leave the text as it was.
#[test]
fn writing_a_character_over_a_pair_moves_the_rest_back() {
    let mut cx = cx();
    let text = wstring::<8>("a🙂c");
    set_char_wide(&mut cx, text.view(), 2, 0x62).unwrap();
    assert_wide(&text, "abc");
    set_char_wide(&mut cx, text.view(), 1, 0x20AC).unwrap();
    assert_wide(&text, "€bc");
    let tail = wstring::<8>("ab🙂");
    set_char_wide(&mut cx, tail.view(), 3, 0x7A).unwrap();
    assert_wide(&tail, "abz");
    let full = wstring::<3>("abc");
    set_char_wide(&mut cx, full.view(), 1, 0xC4).unwrap();
    assert_wide(&full, "Äbc");
    assert_eq!(
        code(set_char_wide(&mut cx, full.view(), 4, 0x78)),
        OUT_OF_BOUNDS
    );
    assert_eq!(cx.fault.detail, [4, 1, 3]);
    assert_eq!(
        code(set_char_wide(&mut cx, full.view(), 0, 0x78)),
        OUT_OF_BOUNDS
    );
    assert_eq!(cx.fault.detail, [0, 1, i64::MAX]);
    assert_eq!(
        code(set_char_wide(&mut cx, full.view(), 2, 0xD800)),
        FaultCode::TypeMismatch as u32
    );
    assert_eq!(cx.fault.detail, [0xD800, 0, 0]);
    assert_wide(&full, "Äbc");
}

/// `STRING_TO_WSTRING` and `WSTRING_TO_STRING` convert by character and cut at the destination's
/// capacity at the last whole character (specification 02 §6; 07 §2.5): `'Grüße€🙂'` is 14 bytes of
/// UTF-8 and 8 code units. A `WSTRING[7]` holds `'Grüße€'`, since `'🙂'` needs two code units and one
/// is left; a `STRING[13]` holds `'Grüße€'` too, `'🙂'` needing four bytes where three are left, and
/// a `STRING[9]` holds `'Grüße'`, `'€'` needing three. A conversion of bytes or code units one by
/// one would split characters. `WCHAR_TO_WSTRING` of 16#20AC is `'€'`.
#[test]
fn conversions_keep_whole_characters() {
    let utf8 = string::<16>("Grüße€🙂".as_bytes());
    let utf16 = WStr::<16>::new();
    string_to_wstring(utf16.view(), utf8.view());
    assert_units(
        &utf16,
        &[0x47, 0x72, 0xFC, 0xDF, 0x65, 0x20AC, 0xD83D, 0xDE42],
    );
    let seven = WStr::<7>::new();
    string_to_wstring(seven.view(), utf8.view());
    assert_wide(&seven, "Grüße€");
    let mut cx = cx();
    let back = Str::<16>::new();
    wstring_to_string(&mut cx, back.view(), utf16.view()).unwrap();
    assert_bytes(&back, "Grüße€🙂".as_bytes());
    let thirteen = Str::<13>::new();
    wstring_to_string(&mut cx, thirteen.view(), utf16.view()).unwrap();
    assert_bytes(&thirteen, "Grüße€".as_bytes());
    let nine = Str::<9>::new();
    wstring_to_string(&mut cx, nine.view(), utf16.view()).unwrap();
    assert_bytes(&nine, "Grüße".as_bytes());
    let euro = WStr::<4>::new();
    assign_wchar(&mut cx, euro.view(), 0x20AC).unwrap();
    assert_wide(&euro, "€");
    // Half of a surrogate pair is no character: `WCHAR_TO_WSTRING` refuses it and writes nothing.
    assert!(assign_wchar(&mut cx, euro.view(), 0xD800).is_err());
    assert_wide(&euro, "€");
}

/// Half of a surrogate pair, which no function of the library writes but the memory of a `WSTRING`
/// can hold, is one character of its own, of its code unit's value (the specifications are
/// silent): `'A'`, 16#D800, `'B'` is three characters, where reading 16#D800 as the start of a pair
/// would join `'B'` to it; its second character reads as 16#D800, orders after U+D7FF and before
/// U+E000 and `'🙂'`, and becomes U+FFFD, the bytes 16#EF 16#BF 16#BD, in a `STRING`. A low and a
/// high surrogate in that order are two characters; a high and a low one are one character also
/// where two parts of a result meet, which a cut does not split.
#[test]
fn a_lone_surrogate_is_a_character_of_its_own() {
    let mut cx = cx();
    let lone = wstring_units::<4>(&[0xD800]);
    assert_units(&lone, &[0xD800]);
    let text = wstring_units::<8>(&[0x41, 0xD800, 0x42]);
    assert_eq!(len_wide(text.view()), 3);
    assert_eq!(char_at_wide(&mut cx, text.view(), 2), Ok(0xD800));
    assert_eq!(char_at_wide(&mut cx, text.view(), 3), Ok(0x42));
    let below = wstring_units::<4>(&[0xD7FF]);
    let above = wstring_units::<4>(&[0xE000]);
    assert_eq!(compare_wide(lone.view(), below.view()), Ordering::Greater);
    assert_eq!(compare_wide(lone.view(), above.view()), Ordering::Less);
    assert_eq!(
        compare_wide(lone.view(), wstring::<4>("🙂").view()),
        Ordering::Less
    );
    let converted = Str::<16>::new();
    wstring_to_string(&mut cx, converted.view(), text.view()).unwrap();
    assert_bytes(&converted, &[0x41, 0xEF, 0xBF, 0xBD, 0x42]);
    let reversed = wstring_units::<4>(&[0xDE42, 0xD83D]);
    assert_eq!(len_wide(reversed.view()), 2);
    wstring_to_string(&mut cx, converted.view(), reversed.view()).unwrap();
    assert_bytes(&converted, &[0xEF, 0xBF, 0xBD, 0xEF, 0xBF, 0xBD]);
    let halves = [WPiece::Units(&[0xD83D]), WPiece::Units(&[0xDE42])];
    let joined = WStr::<4>::new();
    concat_wide(&mut cx, joined.view(), &halves).unwrap();
    assert_eq!(len_wide(joined.view()), 1);
    assert_wide(&joined, "🙂");
    let one = WStr::<1>::new();
    concat_wide(&mut cx, one.view(), &halves).unwrap();
    assert_units(&one, &[]);
}

/// Bytes of a `STRING` that are no UTF-8 become U+FFFD in a `WSTRING`, one for each character as
/// the `STRING` functions count them (the specifications are silent; no text of the runtime holds
/// such bytes): a lead byte without its continuation byte, the bytes 16#ED 16#A0 16#80 that would
/// encode the surrogate 16#D800, an overlong encoding of U+0000, a code point above U+10FFFF and a
/// continuation byte at the start. Decoding them bit by bit would put a surrogate, a NUL and a
/// value no `WSTRING` can hold into the text.
#[test]
fn bytes_that_are_no_utf8_convert_to_u_fffd() {
    let out = WStr::<8>::new();
    string_to_wstring(out.view(), string::<8>(&[0x41, 0xC3, 0x42]).view());
    assert_units(&out, &[0x41, 0xFFFD, 0x42]);
    string_to_wstring(out.view(), string::<8>(&[0xED, 0xA0, 0x80]).view());
    assert_units(&out, &[0xFFFD]);
    string_to_wstring(out.view(), string::<8>(&[0xC0, 0x80, 0x41]).view());
    assert_units(&out, &[0xFFFD, 0x41]);
    string_to_wstring(out.view(), string::<8>(&[0xF4, 0x90, 0x80, 0x80]).view());
    assert_units(&out, &[0xFFFD]);
    string_to_wstring(out.view(), string::<8>(&[0x80, 0x41]).view());
    assert_units(&out, &[0xFFFD, 0x41]);
}
