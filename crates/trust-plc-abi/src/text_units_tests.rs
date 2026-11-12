//! A `WSTRING`'s code units by their offsets under the CODESYS profile (specification 44 §7.3;
//! `docs/IEC_DEVIATIONS.md` DEV-046): the reference that `ADR(ws[i])` takes, the `WORD` it reaches,
//! and the length that the units written through it give the text, which ends at its first 0 unit.

use core::cell::Cell;

use crate::kernel::WITHOUT_DEBUGGER;
use crate::module::run;
use crate::{plain_struct, Cx, FaultCode, Plain, RefValue, WStr, AREA_GLOBALS};

plain_struct! {
    struct WideTexts {
        count: Cell<u16>,
        text: WStr<4>,
    }
}

fn context() -> Cx {
    Cx::new(&WITHOUT_DEBUGGER)
}

/// The fault and its numbers of a dereference as `T` of the reference to the code units of
/// `WideTexts::text` from its unit `char_index`.
fn deref_outcome<T: Plain>(char_index: u16) -> (u32, [i64; 3]) {
    let mut words = [0u64; 2];
    let mut cx = context();
    cx.set_area(AREA_GLOBALS, words.as_mut_ptr().cast(), 16);
    let result = run(&raw mut cx, |cx, areas| {
        let texts: &WideTexts = areas.view(cx, AREA_GLOBALS)?;
        let units = areas.refer_text_units(cx, texts.text.view(), 0)?;
        let _: &T = areas.deref(
            cx,
            &RefValue {
                char_index,
                ..units
            },
        )?;
        Ok(())
    });
    (result, cx.fault.detail)
}

/// A `WSTRING`'s code units are named by a reference to the whole string, its length and its
/// units, from the unit `char_index` (specification 44 §7.3): `text`, a `WSTRING[4]` after a 2-byte
/// `count`, lies at offset 2 and spans 2 + 2 × 4 = 10 bytes. A `WORD` from unit 0 or unit 3 is
/// reached through it; a byte (alignment 1, 8 bytes from unit 0), a `DWORD` (alignment 4) and two
/// `WORD`s from unit 3 (2 bytes left there, 4 asked for) are refused with `TypeMismatch`, the bytes
/// there and the bytes asked for in the record. Catches a unit counted as a byte (unit 3 reached at
/// byte 3 of the units), and a reference past the string's last unit.
#[test]
fn a_reference_to_a_wide_strings_units_names_the_string_and_reaches_a_word() {
    let mut words = [0u64; 2];
    let mut cx = context();
    cx.set_area(AREA_GLOBALS, words.as_mut_ptr().cast(), 16);
    let result = run(&raw mut cx, |cx, areas| {
        let texts: &WideTexts = areas.view(cx, AREA_GLOBALS)?;
        let units = areas.refer_text_units(cx, texts.text.view(), 0)?;
        assert_eq!(
            units,
            RefValue {
                area: AREA_GLOBALS,
                kind: RefValue::TEXT_UNITS,
                char_index: 0,
                offset: 2,
                extent: 10,
                generation: 1,
            }
        );
        let first: &Cell<u16> = areas.deref(cx, &units)?;
        first.set(0x00C4);
        let fourth: &Cell<u16> = areas.deref(
            cx,
            &RefValue {
                char_index: 3,
                ..units
            },
        )?;
        fourth.set(0x20AC);
        assert_eq!(
            (texts.text.data[0].get(), texts.text.data[3].get()),
            (0x00C4, 0x20AC)
        );
        Ok(())
    });
    assert_eq!(result, 0);
    let mismatch = FaultCode::TypeMismatch as u32;
    assert_eq!(deref_outcome::<Cell<u8>>(0), (mismatch, [8, 1, 0]));
    assert_eq!(deref_outcome::<Cell<u32>>(0), (mismatch, [8, 4, 0]));
    assert_eq!(deref_outcome::<[Cell<u16>; 2]>(3), (mismatch, [2, 4, 0]));
    assert_eq!(deref_outcome::<Cell<u16>>(3).0, 0);
}

/// A `WORD` written through the reference sets the length that the units give the text, as CODESYS
/// reads a `WSTRING` to its first 0 unit, and a reference of another kind sets none. From an empty
/// `WSTRING[4]`: 'H' at unit 0 gives the length 1; 'i' at unit 1 through a reference to a variable
/// leaves 1, through the string's reference gives 2; '!' at unit 3, after the 0 at unit 2, leaves 2;
/// '?' over that 0 extends the text through the '!' to the capacity, 4; a 0 at unit 1 ends it at 1.
/// Catches a write that leaves the length as it was, units counted as bytes (unit 1 would be byte 1
/// of 'H'), and a reference to a variable whose writes are taken as a string's.
#[test]
fn a_word_written_through_the_reference_sets_the_length_of_the_wide_text() {
    let mut words = [0u64; 2];
    let mut cx = context();
    cx.set_area(AREA_GLOBALS, words.as_mut_ptr().cast(), 16);
    let result = run(&raw mut cx, |cx, areas| {
        let texts: &WideTexts = areas.view(cx, AREA_GLOBALS)?;
        let units = areas.refer_text_units(cx, texts.text.view(), 0)?;
        let at = |char_index: u16| RefValue {
            char_index,
            ..units
        };
        let first: &Cell<u16> = areas.deref(cx, &at(0))?;
        first.set(0x48);
        areas.bytes_written(cx, &at(0), 2)?;
        assert_eq!(texts.text.len.get(), 1);
        let second: &Cell<u16> = areas.deref(cx, &at(1))?;
        second.set(0x69);
        let variable = RefValue {
            kind: RefValue::VARIABLE,
            ..at(1)
        };
        areas.bytes_written(cx, &variable, 2)?;
        assert_eq!(texts.text.len.get(), 1);
        areas.bytes_written(cx, &at(1), 2)?;
        assert_eq!(texts.text.len.get(), 2);
        let fourth: &Cell<u16> = areas.deref(cx, &at(3))?;
        fourth.set(0x21);
        areas.bytes_written(cx, &at(3), 2)?;
        assert_eq!(texts.text.len.get(), 2);
        let third: &Cell<u16> = areas.deref(cx, &at(2))?;
        third.set(0x3F);
        areas.bytes_written(cx, &at(2), 2)?;
        assert_eq!(texts.text.len.get(), 4);
        second.set(0);
        areas.bytes_written(cx, &at(1), 2)?;
        assert_eq!(texts.text.len.get(), 1);
        Ok(())
    });
    assert_eq!(result, 0);
}

/// The fault and its numbers of a reference to the code units of `WideTexts::text`, a `WSTRING[4]`,
/// from its unit `first`, and the reference's index when it is taken.
fn refer_outcome(first: i64) -> (u32, [i64; 3], u16) {
    let mut words = [0u64; 2];
    let mut cx = context();
    cx.set_area(AREA_GLOBALS, words.as_mut_ptr().cast(), 16);
    let mut index = u16::MAX;
    let result = run(&raw mut cx, |cx, areas| {
        let texts: &WideTexts = areas.view(cx, AREA_GLOBALS)?;
        index = areas
            .refer_text_units(cx, texts.text.view(), first)?
            .char_index;
        Ok(())
    });
    (result, cx.fault.detail, index)
}

/// `ADR(ws[i])` under the CODESYS profile names the string's units from unit `i`, counted from 0
/// (DEV-046): units 0 and 3 of a `WSTRING[4]` are named with those indices; -1 and 4, which the
/// string does not hold, are refused with `IndexOutOfBounds` and the bounds 0 to 3. Catches an index
/// counted from 1, and bounds counted in bytes (0 to 7).
#[test]
fn a_reference_to_a_wide_strings_unit_names_it_from_zero_within_the_capacity() {
    assert_eq!(refer_outcome(0), (0, [0, 0, 0], 0));
    assert_eq!(refer_outcome(3), (0, [0, 0, 0], 3));
    let out = FaultCode::IndexOutOfBounds as u32;
    assert_eq!(refer_outcome(-1), (out, [-1, 0, 3], u16::MAX));
    assert_eq!(refer_outcome(4), (out, [4, 0, 3], u16::MAX));
}
