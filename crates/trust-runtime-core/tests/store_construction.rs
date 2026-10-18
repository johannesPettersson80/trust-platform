//! The charge of the cell store's construction (specification 34 §6.2, MEM-10; C1-9d2b-1, R6).
//!
//! The program is the bounded-C corpus program `initializers`, compiled from
//! `crates/trust-runtime/tests/fixtures/bounded-c/initializers/src/main.st`:
//!
//! ```st
//! VAR_GLOBAL ticks : DINT := DINT#0; table : ARRAY[1..3] OF INT := [INT#4, INT#5, INT#6]; END_VAR
//! FUNCTION Pick : INT ... VAR_STAT calls : INT := INT#0; END_VAR ... END_FUNCTION
//! FUNCTION_BLOCK Tracker VAR history : ARRAY[1..3] OF INT := [INT#1, INT#2, INT#3]; END_VAR ... END_FUNCTION_BLOCK
//! PROGRAM Initializers VAR tracker : Tracker; slot : INT := INT#1; result : INT; END_VAR ... END_PROGRAM
//! ```
//!
//! Its STORAGE_LAYOUT has 15 declarations (the globals `ticks` and `table`, the program root, the static
//! `calls`, the program's three members, the tracker's `history` and the frame variables of the two
//! functions) and its CONSTRUCTION_ROOTS has 5 records (`ticks`, `table`, the program, the tracker,
//! `calls`).

use trust_runtime_core::error::RuntimeError;
use trust_runtime_core::memory::regions::CellStorage;
use trust_runtime_core::memory::Generation;
use trust_runtime_core::vm::{PreparationLimits, PreparedModule};

const INITIALIZERS: &[u8] =
    include_bytes!("../../trust-runtime/tests/fixtures/bounded-c/initializers/program.stbc");

#[test]
fn range_tables_are_charged_before_they_are_reserved() {
    // Specification 34 §6.2 and O18 (R6): construction charges the memory it holds with the region's cells,
    // once and before it reserves anything. Worked out by hand from the declarations above:
    //
    // - cells: the globals `ticks` (1), `table` (3) and `calls` (1) take 5; the program holds `tracker`
    //   (one instance cell), `slot` and `result`, 3; the tracker holds `history`, 3. That is 11 cells
    //   at 16 bytes, 176 bytes. No reference leaf and no text leaf is persistent, so there are no
    //   reference records, no text buffers and no reference table;
    // - the `globals` range table has one 12-byte entry for each of the 15 declarations, 180 bytes, and
    //   the `roots` table one for each of the 5 roots, 60 bytes;
    // - the lists the construction fills hold no text leaf and no reference leaf, and 6 initial values
    //   at 24 bytes, 144 bytes: `ticks` and `table`, then the program's three members and the tracker's
    //   `history`. The static `calls` has no initial value: its function's static action gives it one
    //   (specification 34 §6.2).
    //
    // The charge is 176 + 180 + 60 + 144 = 560 bytes, requested once. An allowance of 559 refuses it and
    // returns the allowance's error, so no store exists; an allowance of 560 admits the region. A
    // construction that left the range tables or the lists out would request 320 or 416 bytes.
    let prepared = PreparedModule::from_bytes(INITIALIZERS, PreparationLimits::default())
        .expect("the initializers program prepares");
    let requests = std::cell::RefCell::new(Vec::new());
    let refused = CellStorage::construct(&prepared, Generation::FIRST, |bytes| {
        requests.borrow_mut().push(bytes);
        if bytes > 559 {
            Err(RuntimeError::Overflow)
        } else {
            Ok(())
        }
    });
    assert_eq!(refused.err(), Some(RuntimeError::Overflow));
    assert_eq!(*requests.borrow(), [560]);

    let admitted = CellStorage::construct(&prepared, Generation::FIRST, |bytes| {
        if bytes > 560 {
            Err(RuntimeError::Overflow)
        } else {
            Ok(())
        }
    });
    assert!(admitted.is_ok());
}
