//! A Rust POU's in-outs and references read and write the variable they name, a reference with
//! Structured Text's checks (specification 48 RU-29; specification 44 §6.5, §7.3). Every expected
//! value and fault is the specification's, worked out by hand.

use core::cell::Cell;

use trust_plc_abi::kernel::WITHOUT_DEBUGGER;
use trust_plc_abi::module::GENERATION;
use trust_plc_abi::{FaultCode, RefValue, AREA_GLOBALS};

use super::{reference, reference_value, InOut, Ref};
use crate::{Array, Cx};

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
        let level: Ref<f64> = reference(variable(8, 8, GENERATION));
        level.set(&mut cx, 6.5).expect("the variable is written");
        assert_eq!(level.get(&mut cx), Ok(6.5));
        let null: Ref<f64> = Ref::NULL;
        assert!(null.is_null() && !level.is_null());
        assert_eq!(code(null.get(&mut cx)), FaultCode::NullReference as u32);
        let past = reference::<f64>(variable(32, 8, GENERATION));
        assert_eq!(code(past.get(&mut cx)), FaultCode::ReferenceLifetime as u32);
        let stale = reference::<f64>(variable(8, 8, GENERATION + 1));
        assert_eq!(
            code(stale.set(&mut cx, 1.0)),
            FaultCode::ReferenceLifetime as u32
        );
        let narrow = reference::<f64>(variable(8, 4, GENERATION));
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
