//! The retained image captured from the flat store (specification 11 §6.7, specification 34 §10 and
//! §6.2; C1-9d2b-2, R1 and R2; C1-19e). The capture writes each retained variable from its cells, and the
//! payload is the one the same program gave on the `Value` store, which is recorded (`expected.rs`).
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

use super::expected::Facts;
use super::{build, prepare, services, EngineState, CORPUS, RETAIN};
use crate::bytecode::StorageOwner;
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

/// The retained payload of every program that retains, after construction and after three scans.
fn retained_payloads(facts: &mut Facts) {
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
        let mut data = build(&prepared).expect("the store constructs the program");
        let mut state = EngineState::borrow(&mut data, &prepared, &services::LOGICAL_ONLY);

        let captured = payload(&mut state);
        facts.check(
            &format!("retained payloads/{name}/after construction"),
            &[format!("{captured:02x?}")],
        );
        if name == "retain" {
            assert_eq!(captured, constructed, "{name} after construction");
        }

        for scan in 1..=3 {
            state
                .execute_cycle(Duration::from_millis(10 * scan))
                .expect("the program scans");
        }
        let captured = payload(&mut state);
        facts.check(
            &format!("retained payloads/{name}/after three scans"),
            &[format!("{captured:02x?}")],
        );
        if name == "retain" {
            assert_eq!(captured, scanned, "{name} after three scans");
        }
    }
    // The programs that retain: `retain`; `retained_blocks`, whose instance `kept` and reference `follow`
    // stay out of the payload beside its `total` and its program's `runs`; `retained_values`, whose
    // arrays, structure and texts all go into it; `overlays` and `unions`, whose overlay and union go
    // into it; and the saved main artifact, whose program retains `activations`.
    assert_eq!(
        covered,
        [
            "retain",
            "retained_blocks",
            "retained_values",
            "overlays",
            "unions",
            "saved/main"
        ]
    );
}

#[test]
fn captured_cells_encode_like_the_value_tree() {
    // Specification 11 §6.7 and specification 34 §10: the retained payload is the same bytes whether the
    // variables are held in cells or in the value tree. Each bounded-C program that declares retained
    // variables is built and captured twice: after construction, and after three scans, and each payload
    // is checked against the one the `Value` store captured. The scans advance the retained variables in
    // the cells (C1-9d3a, R3), and the capture encodes them from there. Worked out by hand for `retain`,
    // whose entries are `counter`, `setpoint`, the program's `runs` and its `label`, in that order (the
    // retained globals in declaration order, then the program's variables under the key
    // `@program/Plant/<name>`):
    //
    // - the count 4;
    // - `counter`: name length 7, then DINT 0 (tag 4); after three scans DINT 3;
    // - `setpoint`: name length 8, then REAL 20.0 (tag 10, 0x41A00000 as 00 00 A0 41); after three scans
    //   20.0 + 3 × 0.5 = 21.5, 0x41AC0000, as 00 00 AC 41;
    // - `@program/Plant/runs`: name length 9 + 5 + 1 + 4 = 19, then DINT 0; after three scans DINT 3;
    // - `@program/Plant/label`: name length 20, then STRING 'boiler' (tag 24, length 6, six bytes), which
    //   no scan changes.
    //
    // The payloads are checked against these bytes and the recorded ones, so a cell encoder that dropped
    // a variable, kept the values of construction, or encoded the label as a different tag, differs, and so
    // does a store whose scans did not advance the variables its capture reads.
    let mut facts = Facts::compared();
    retained_payloads(&mut facts);
    facts.finish("retained payloads/");
}

#[test]
fn the_cell_capture_is_charged_what_the_value_capture_was_charged() {
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
    // The capture charges 71 + 72 + 83 + 90 = 316 bytes, as the value capture did. A cell view charged by
    // its cell count, or by the buffer's capacity of 12 bytes instead of the 6 bytes in use, would charge
    // more.
    let prepared = prepare(RETAIN);
    let mut data = build(&prepared).expect("the store constructs");
    let mut state = EngineState::borrow(&mut data, &prepared, &services::LOGICAL_ONLY);
    let work_before = state.data.resources.work_budget.remaining();
    state.data.resources.construction_bytes.set(0);
    payload(&mut state);
    assert_eq!(state.data.resources.construction_bytes.get(), 316, "bytes");
    assert_eq!(
        state.data.resources.work_budget.remaining(),
        work_before,
        "work"
    );
}

/// Record the facts of every test of this file.
pub(super) fn record(facts: &mut Facts) {
    retained_payloads(facts);
}
