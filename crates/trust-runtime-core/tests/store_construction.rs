//! The charge of the store's construction (specification 34 §6.2, MEM-10, MEM-12; C1-9d2b-1, R6).
//!
//! The program is the bounded-C corpus program `initializers`, compiled from
//! `crates/trust-runtime/tests/fixtures/bounded-c/initializers/src/main.st`:
//!
//! ```st
//! TYPE Pair : STRUCT a : INT; b : INT; END_STRUCT END_TYPE
//! VAR_GLOBAL
//!     ticks : DINT := DINT#0;  table : ARRAY[1..3] OF INT := [INT#4, INT#5, INT#6];
//!     origin : Pair := (a := INT#1, b := INT#2);
//!     corners : ARRAY[1..2] OF Pair := [(a := INT#3, b := INT#4), (a := INT#5, b := INT#6)];
//! END_VAR
//! VAR_GLOBAL CONSTANT capacity : INT := INT#9; END_VAR
//! FUNCTION Pick : INT ... VAR_STAT calls : INT := INT#0; END_VAR ... END_FUNCTION
//! FUNCTION_BLOCK Tracker VAR history : ARRAY[1..3] OF INT := [INT#1, INT#2, INT#3]; END_VAR ... END_FUNCTION_BLOCK
//! PROGRAM Initializers
//!     VAR CONSTANT gain : INT := INT#2; END_VAR
//!     VAR tracker : Tracker; slot : INT := INT#1; result : INT; END_VAR ... END_PROGRAM
//! ```
//!
//! Its STORAGE_LAYOUT has 19 declarations (the globals `ticks`, `table`, `origin`, `corners` and `capacity`,
//! the program root, the static `calls`, the program's four members, the tracker's `history` and the frame
//! variables of the two functions) and its CONSTRUCTION_ROOTS has 8 records (`ticks`, `table`, `origin`,
//! `corners`, `capacity`, the program, the tracker, `calls`).

use trust_runtime_core::error::RuntimeError;
use trust_runtime_core::memory::regions::CellStorage;
use trust_runtime_core::memory::Generation;
use trust_runtime_core::vm::{PreparationLimits, PreparedModule};

const INITIALIZERS: &[u8] =
    include_bytes!("../../trust-runtime/tests/fixtures/bounded-c/initializers/program.stbc");

#[test]
fn range_tables_are_charged_before_they_are_reserved() {
    // Specification 34 §6.2 and O18 (R6, C1-9d2b-2 R5): construction charges the memory it holds with the
    // region's flat block, once and before it reserves anything. Worked out by hand from the declarations
    // above and MEM-12:
    //
    // - the block: the globals in declaration order, `ticks` (a DINT) at 0, `table` (three INTs) at 4,
    //   `origin` (two INTs) at 10, `corners` (two Pairs) at 14, `capacity` at 22 and `calls` at 24, end at
    //   26; the program's root starts at the next multiple of 8, 32, with `gain` at 0, `tracker` (an
    //   8-byte instance leaf) at 8, `slot` at 16 and `result` at 18, 20 bytes rounded up to 24; the
    //   tracker's root follows at 56 with `history` (three INTs), 6 bytes rounded up to 8. That is 64
    //   bytes. No reference leaf is persistent, so there is no reference table;
    // - the `globals` range table has one 12-byte entry for each of the 19 declarations, 228 bytes, and
    //   the `roots` table one for each of the 8 roots, 96 bytes;
    // - the instance table has one 28-byte entry for each of the 2 roots that have a template POU (the
    //   program and the tracker), 56 bytes;
    // - the lists the construction fills hold no text leaf and no reference leaf, and 10 initial values
    //   at 24 bytes, 240 bytes: the five globals other than `calls`, the program's four members and the
    //   tracker's `history`. The static `calls` has no initial value: its function's static action gives
    //   it one (specification 34 §6.2).
    //
    // The charge is 64 + 228 + 96 + 56 + 240 = 684 bytes, requested once. An allowance of 683 refuses it
    // and returns the allowance's error, so no store exists; an allowance of 684 admits the region. A
    // construction that left the instance table out would request 628 bytes, one that left the range
    // tables or the lists out 360 or 444, and one that still counted 19 cells of 16 bytes 924.
    let prepared = PreparedModule::from_bytes(INITIALIZERS, PreparationLimits::default())
        .expect("the initializers program prepares");
    let requests = std::cell::RefCell::new(Vec::new());
    let refused = CellStorage::construct(&prepared, Generation::FIRST, |bytes| {
        requests.borrow_mut().push(bytes);
        if bytes > 683 {
            Err(RuntimeError::Overflow)
        } else {
            Ok(())
        }
    });
    assert_eq!(refused.err(), Some(RuntimeError::Overflow));
    assert_eq!(*requests.borrow(), [684]);

    let admitted = CellStorage::construct(&prepared, Generation::FIRST, |bytes| {
        if bytes > 684 {
            Err(RuntimeError::Overflow)
        } else {
            Ok(())
        }
    });
    assert!(admitted.is_ok());
}
