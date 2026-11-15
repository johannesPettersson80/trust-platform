//! A Rust POU's in-outs and references read and write the variable they name, a reference with
//! Structured Text's checks (specification 48 RU-29; specification 44 §6.5, §7.3). Every expected
//! value and fault is the specification's, worked out by hand.

use core::cell::Cell;

use trust_plc_abi::kernel::WITHOUT_DEBUGGER;
use trust_plc_abi::module::GENERATION;
use trust_plc_abi::{FaultCode, RefValue, AREA_GLOBALS};

use super::{reference, reference_value, InOut, Ref};
use crate::{Array, Cx, Str, WStr};

/// A reference of kind VARIABLE into the globals, at `offset` with `extent` bytes, of `generation`.
fn variable(offset: u32, extent: u32, generation: u32) -> RefValue {
    RefValue {
        area: AREA_GLOBALS,
        kind: RefValue::VARIABLE,
        char_index: 0,
        offset,
        extent,
        generation,
    }
}

fn code<T: core::fmt::Debug>(result: Result<T, crate::Fault>) -> u32 {
    result.expect_err("the access faults").raw()
}

#[test]
fn an_in_out_reads_and_writes_the_callers_variable_at_once() {
    // RU-29: a function's in-out is the caller's variable itself: a write is in the variable at
    // once, before the function returns. Catches an in-out that works on a copy.
    let level = Cell::new(2.5_f64);
    let in_out: InOut<'_, f64> = InOut::new(&level);
    assert_eq!(in_out.get(), 2.5);
    in_out.set(in_out.get() + 1.25);
    assert_eq!(level.get(), 3.75);
    let cells = [Cell::new(1_i16), Cell::new(2), Cell::new(3)];
    let array: InOut<'_, Array<i16, 1, 3>> = InOut::new(&cells);
    let mut values = array.get();
    *values.get_mut(2).expect("element 2, the second") = 20;
    array.set(values);
    assert_eq!(cells.iter().map(Cell::get).collect::<Vec<_>>(), [1, 20, 3]);
}

#[test]
fn a_reference_reads_and_writes_its_variable_with_structured_texts_checks() {
    // RU-29 and specification 44 §6.5: through a reference into the globals, `get` and `set` reach
    // the variable at its offset; `NULL` faults `NullReference`, a reference past the area's end
    // or of another generation `ReferenceLifetime`, one whose extent is no `T`'s `TypeMismatch`,
    // and a faulted `set` writes nothing. Catches a dereference that skips a check.
    let mut globals = [0_u64; 4];
    let mut raw = trust_plc_abi::Cx::new(&WITHOUT_DEBUGGER);
    raw.set_area(AREA_GLOBALS, globals.as_mut_ptr().cast::<u8>(), 32);
    {
        let mut cx = Cx::of(raw);
        let level: Ref<f64> = reference::<f64, Cell<f64>>(variable(8, 8, GENERATION));
        level.set(&mut cx, 6.5).expect("the variable is written");
        assert_eq!(level.get(&mut cx), Ok(6.5));
        let null: Ref<f64> = Ref::NULL;
        assert!(null.is_null() && !level.is_null());
        assert_eq!(code(null.get(&mut cx)), FaultCode::NullReference as u32);
        let past = reference::<f64, Cell<f64>>(variable(32, 8, GENERATION));
        assert_eq!(code(past.get(&mut cx)), FaultCode::ReferenceLifetime as u32);
        let stale = reference::<f64, Cell<f64>>(variable(8, 8, GENERATION + 1));
        assert_eq!(
            code(stale.set(&mut cx, 1.0)),
            FaultCode::ReferenceLifetime as u32
        );
        let narrow = reference::<f64, Cell<f64>>(variable(8, 4, GENERATION));
        assert_eq!(code(narrow.get(&mut cx)), FaultCode::TypeMismatch as u32);
        assert_eq!(level.get(&mut cx), Ok(6.5), "no faulted write changed it");
        // A reference is copied and compared as Structured Text's are, and keeps its raw form.
        let copy = level;
        assert!(copy == level && copy != null);
        assert_eq!(reference_value(copy), variable(8, 8, GENERATION));
    }
    // The context, and with it the area's pointer, is gone: the buffer holds what `set` wrote.
    assert_eq!(f64::from_bits(globals[1]), 6.5);
}

/// A structure as a unit test holds one: the SDK's in-out takes a cell of it.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Point {
    x: f64,
    y: i16,
}

#[test]
fn an_in_out_takes_a_variable_of_any_type_and_the_areas_forms() {
    // RU-29, RU-56: a unit test's in-out is a cell of its own of any type, a structure's too, and
    // a write changes every field at once; the generated code's place of a BOOL is its byte, 0 or 1
    // (specification 44 §6.2). Catches an in-out that takes only the areas' forms, or a BOOL read
    // as its byte's value.
    let point = Cell::new(Point { x: 1.5, y: 2 });
    let in_out: InOut<'_, Point> = InOut::new(&point);
    let mut moved = in_out.get();
    moved.x += 3.0;
    moved.y -= 7;
    in_out.set(moved);
    assert_eq!(point.get(), Point { x: 4.5, y: -5 });
    let byte = Cell::new(2_u8);
    let flag: InOut<'_, bool> = InOut::new(&byte);
    assert!(flag.get(), "a byte other than 0 is TRUE");
    flag.set(false);
    assert_eq!(byte.get(), 0);
    flag.set(true);
    assert_eq!(byte.get(), 1);
}

#[test]
fn a_reference_to_an_array_reads_and_writes_every_element_through_its_place() {
    // RU-29 and specification 44 §6.5, §7.3: a reference to `ARRAY[1..3] OF INT` names its 6
    // bytes; `set` writes the three elements into the area at once and `get` reads them back, and
    // a reference of 4 bytes is no such array, `TypeMismatch`, and writes nothing. Catches a
    // dereference of a compound type that reads one element or skips the extent's check.
    let mut globals = [0_u64; 2];
    let mut raw = trust_plc_abi::Cx::new(&WITHOUT_DEBUGGER);
    raw.set_area(AREA_GLOBALS, globals.as_mut_ptr().cast::<u8>(), 16);
    {
        let mut cx = Cx::of(raw);
        let counts = reference::<Array<i16, 1, 3>, [Cell<i16>; 3]>(variable(2, 6, GENERATION));
        counts
            .set(&mut cx, Array::from_items([7, -8, 9]))
            .expect("the array is written");
        assert_eq!(counts.get(&mut cx), Ok(Array::from_items([7, -8, 9])));
        let short = reference::<Array<i16, 1, 3>, [Cell<i16>; 3]>(variable(2, 4, GENERATION));
        assert_eq!(
            code(short.set(&mut cx, Array::filled(0))),
            FaultCode::TypeMismatch as u32
        );
    }
    // Bytes 2 to 7 of the area hold 7, -8 and 9, in the target's byte order.
    let bytes: Vec<u8> = globals.iter().flat_map(|word| word.to_ne_bytes()).collect();
    let elements: Vec<i16> = bytes[2..8]
        .chunks(2)
        .map(|pair| i16::from_ne_bytes([pair[0], pair[1]]))
        .collect();
    assert_eq!(elements, [7, -8, 9]);
}

#[test]
fn a_string_in_out_reads_and_writes_the_callers_string_of_any_capacity() {
    // RU-29, docs/IEC_DEVIATIONS.md DEV-035: a function's string in-out is the caller's string of
    // any capacity. Its text reads as an assignment to the in-out's STRING[5] does, cut to the
    // longest prefix of whole characters 5 bytes hold: 'abcdé' (é is 2 bytes) reads 'abcd'; a value
    // writes as an assignment to the caller's string does: 'xyz' into its STRING[8] is 'xyz',
    // 'hello' into a STRING[2] is 'he', and 'abcde' into a WSTRING[3] is 'abc'. Catches a read that
    // splits a character or keeps more than the in-out holds, and a write past the caller's
    // capacity.
    let wide = trust_plc_abi::Str::<8>::new();
    trust_plc_lib::assign_bytes(wide.view(), "abcdé".as_bytes());
    let view = wide.view();
    let text: InOut<'_, Str<5>> = InOut::new(&view);
    assert_eq!(text.get().as_str(), Some("abcd"));
    text.set(Str::from_text("xyz"));
    assert_eq!(
        (view.len(), [view.byte(0), view.byte(1), view.byte(2)]),
        (3, *b"xyz")
    );
    let narrow = trust_plc_abi::Str::<2>::new();
    let view = narrow.view();
    let short: InOut<'_, Str<5>> = InOut::new(&view);
    short.set(Str::from_text("hello"));
    assert_eq!((view.len(), [view.byte(0), view.byte(1)]), (2, *b"he"));
    let units = trust_plc_abi::WStr::<3>::new();
    let view = units.view();
    let wide_text: InOut<'_, WStr<5>> = InOut::new(&view);
    wide_text.set(WStr::from_text("abcde"));
    assert_eq!(wide_text.get().chars().collect::<String>(), "abc");
}
