//! The retained image captured from the cell store (specification 11 §6.7, specification 34 §10 and
//! §6.2; C1-9d2b-2, R1 and R2). The capture writes each retained variable from the store the engine
//! holds: a value in cells is encoded from its cells, and the payload is the one the same program gives
//! on the `Value` store.
//!
//! The payload layout is the one of `memory::cell_access::retain_tests`: a `u32` entry count, then for
//! each entry a `u32` name length, the name and the tagged value, little-endian. The tags are `DINT` 4,
//! `REAL` 10 and `STRING` 24.
//!
//! The program is the bounded-C corpus program `retain` (`retain/src/main.st`):
//!
//! ```text
//! VAR_GLOBAL RETAIN counter : DINT := DINT#0; setpoint : REAL := REAL#20.0; END_VAR
//! VAR_GLOBAL ticks : DINT := DINT#0; END_VAR
//! PROGRAM Retained
//! VAR RETAIN runs : DINT; label : STRING[12] := 'boiler'; END_VAR
//! ticks := ticks + 1; counter := counter + 1; runs := runs + 1; setpoint := setpoint + 0.5;
//! END_PROGRAM
//! ```
//!
//! its program instance `Plant`. A scan adds 1 to `counter` and `runs` and 0.5 to `setpoint`.

use super::{build, global, member, prepare, services, EngineState, StoreKind, CORPUS, RETAIN};
use crate::bytecode::{StorageOwner, StorageRole};
use crate::memory::view::StoreMutate;
use crate::retain::capture::RetainCapture;
use crate::value::Duration;
use crate::vm::PreparedModule;

/// Whether the program declares a retained global or a retained variable in a program.
fn holds_retained_variables(prepared: &PreparedModule) -> bool {
    !prepared
        .retained_declarations(StorageOwner::Global, None)
        .is_empty()
        || prepared.roots.entries.iter().any(|root| {
            !prepared
                .retained_declarations(StorageOwner::Instance, root.template_pou_id)
                .is_empty()
        })
}

/// The retained payload `state` captures.
fn payload(state: &mut EngineState<'_>) -> Vec<u8> {
    let mut capture = RetainCapture::new();
    state
        .capture_retained(&mut capture)
        .expect("the retained image is captured");
    capture.staged_payload().to_vec()
}

/// Give `cells` the value of every retained variable that `values` holds, through the store-access seam, as
/// the scans of a program that ran on the cell store would have left them.
fn copy_retained(values: &EngineState<'_>, cells: &mut EngineState<'_>, prepared: &PreparedModule) {
    for &id in prepared.retained_declarations(StorageOwner::Global, None) {
        let name = &prepared.vm.strings[prepared.layout.entries[id as usize].name_idx as usize];
        let value = global(values, name);
        assert!(
            StoreMutate::set_global(cells, name.clone(), value),
            "{name}"
        );
    }
    for (index, root) in prepared.roots.entries.iter().enumerate() {
        let declaration = &prepared.layout.entries[root.declaration_idx as usize];
        if root.parent_root_idx.is_some() || declaration.role != StorageRole::ProgramRoot {
            continue;
        }
        let from = values.data.construction.roots[index].expect("the program has an instance");
        let to = cells.data.construction.roots[index].expect("the program has an instance");
        for &id in prepared.retained_declarations(StorageOwner::Instance, root.template_pou_id) {
            let name = &prepared.vm.strings[prepared.layout.entries[id as usize].name_idx as usize];
            let value = member(values, from, name);
            assert!(cells.write_instance_var(to, name.clone(), value), "{name}");
        }
    }
}

#[test]
fn captured_cells_encode_like_the_value_tree() {
    // Specification 11 §6.7 and specification 34 §10: the retained payload is the same bytes whether the
    // variables are held in cells or in the value tree. Each bounded-C program that declares retained
    // variables is built on both stores and captured twice: after construction, and after three scans.
    // The scans run on the `Value` store: a program does not scan on the cell store before the switch
    // (C1-9d3 converts the program root's read), so the cell store is given the retained values that
    // three scans leave, through the seam. Worked out by hand for `retain`, whose entries are `counter`,
    // `setpoint`, the program's `runs` and its `label`, in that order (the retained globals in declaration
    // order, then the program's variables under the key `@program/Plant/<name>`):
    //
    // - the count 4;
    // - `counter`: name length 7, then DINT 0 (tag 4); after three scans DINT 3;
    // - `setpoint`: name length 8, then REAL 20.0 (tag 10, 0x41A00000 as 00 00 A0 41); after three scans
    //   20.0 + 3 × 0.5 = 21.5, 0x41AC0000, as 00 00 AC 41;
    // - `@program/Plant/runs`: name length 9 + 5 + 1 + 4 = 19, then DINT 0; after three scans DINT 3;
    // - `@program/Plant/label`: name length 20, then STRING 'boiler' (tag 24, length 6, six bytes), which
    //   no scan changes.
    //
    // The two payloads are compared with each other and with these bytes, so a cell encoder that dropped
    // a variable, kept the values of construction, or encoded the label as a different tag, differs.
    let after = |counter: &[u8], setpoint: &[u8], runs: &[u8]| -> Vec<u8> {
        [
            &[4, 0, 0, 0][..],
            &[7, 0, 0, 0][..],
            &b"counter"[..],
            counter,
            &[8, 0, 0, 0][..],
            &b"setpoint"[..],
            setpoint,
            &[19, 0, 0, 0][..],
            &b"@program/Plant/runs"[..],
            runs,
            &[20, 0, 0, 0][..],
            &b"@program/Plant/label"[..],
            &[24, 6, 0, 0, 0][..],
            &b"boiler"[..],
        ]
        .concat()
    };
    let constructed = after(
        &[4, 0, 0, 0, 0],
        &[10, 0x00, 0x00, 0xA0, 0x41],
        &[4, 0, 0, 0, 0],
    );
    let scanned = after(
        &[4, 3, 0, 0, 0],
        &[10, 0x00, 0x00, 0xAC, 0x41],
        &[4, 3, 0, 0, 0],
    );

    let mut covered = Vec::new();
    for (name, bytes) in CORPUS {
        let prepared = prepare(bytes);
        if !holds_retained_variables(&prepared) {
            continue;
        }
        covered.push(name);
        let mut values =
            build(&prepared, StoreKind::Values).expect("the Value store constructs the program");
        let mut cells =
            build(&prepared, StoreKind::Cells).expect("the cell store constructs the program");
        let mut values = EngineState::borrow(&mut values, &prepared, &services::LOGICAL_ONLY);
        let mut cells = EngineState::borrow(&mut cells, &prepared, &services::LOGICAL_ONLY);

        let from_values = payload(&mut values);
        assert_eq!(
            payload(&mut cells),
            from_values,
            "{name} after construction"
        );
        if name == "retain" {
            assert_eq!(from_values, constructed, "{name} after construction");
        }

        for scan in 1..=3 {
            values
                .execute_cycle(Duration::from_millis(10 * scan))
                .expect("the Value store scans");
        }
        copy_retained(&values, &mut cells, &prepared);
        let from_values = payload(&mut values);
        assert_eq!(payload(&mut cells), from_values, "{name} after three scans");
        if name == "retain" {
            assert_eq!(from_values, scanned, "{name} after three scans");
        }
    }
    // The programs that retain: `retain`, and the saved main artifact, whose program retains `activations`.
    assert_eq!(covered, ["retain", "saved/main"]);
}

#[test]
fn the_cell_capture_is_charged_what_the_value_capture_is_charged() {
    // Specification 34 TIME-08 (the retained-value snapshot row): each retained entry is charged the copy
    // charge of its value, its key and four cells of 16 bytes, and a capture charges no more or less because
    // the value is held in cells. Worked out by hand for `retain` (all values are scalars or a text, so no
    // work unit is charged):
    //
    // - `counter`: a DINT copy allocates nothing, the key is 7 bytes: 0 + 7 + 64 = 71;
    // - `setpoint`: a REAL copy allocates nothing, the key is 8 bytes: 0 + 8 + 64 = 72;
    // - `@program/Plant/runs`: a DINT, the key is 9 + 5 + 1 + 4 = 19 bytes: 0 + 19 + 64 = 83;
    // - `@program/Plant/label`: the STRING 'boiler' copy allocates its 6 UTF-8 bytes, the key is 20 bytes:
    //   6 + 20 + 64 = 90.
    //
    // The capture charges 71 + 72 + 83 + 90 = 316 bytes on both stores. A cell view charged by its cell count,
    // or by the buffer's capacity of 12 bytes instead of the 6 bytes in use, would charge more.
    let prepared = prepare(RETAIN);
    for kind in [StoreKind::Values, StoreKind::Cells] {
        let mut data = build(&prepared, kind).expect("the store constructs");
        let mut state = EngineState::borrow(&mut data, &prepared, &services::LOGICAL_ONLY);
        let work_before = state.data.resources.work_budget.remaining();
        state.data.resources.construction_bytes.set(0);
        payload(&mut state);
        assert_eq!(
            state.data.resources.construction_bytes.get(),
            316,
            "{kind:?}: bytes"
        );
        assert_eq!(
            state.data.resources.work_budget.remaining(),
            work_before,
            "{kind:?}: work"
        );
    }
}
