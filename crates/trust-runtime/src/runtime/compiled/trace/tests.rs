//! The changes a cycle trace keeps from the runtime's own writers between scans (specification 48
//! RU-59).

use indexmap::IndexMap;
use smol_str::SmolStr;
use trust_runtime_core::trace::{Change, ChangeKind, ChangeSource};

use super::{ended_trace, record_cycles};
use crate::harness::TestHarness;
use crate::value::Value;

/// Specification 48 RU-59 and Appendix C, C.6: the mesh writes a subscribed value between two
/// scans, so its write is the next cycle's change from the mesh (source 5), as the bytes the
/// variable then holds. Worked by hand: `Main.Level := 5`, an INT, written between the two cycles
/// of a recording, is its second cycle's change, the bytes 05 00. Catches the mesh's write not
/// recorded, recorded as another source's, or in the cycle before.
#[test]
fn the_meshs_write_is_recorded_as_the_next_cycles_change() {
    let mut harness =
        TestHarness::from_source("PROGRAM Main VAR Level : INT; END_VAR END_PROGRAM").unwrap();
    harness.cycle();
    let traces = record_cycles(harness.runtime_mut(), 2);
    harness.cycle();
    harness.runtime_mut().apply_mesh_updates(&IndexMap::from([(
        SmolStr::new("Main.Level"),
        Value::Int(5),
    )]));
    harness.cycle();

    let trace = ended_trace(&traces);
    assert_eq!(trace.cycles[0].changes, vec![]);
    assert_eq!(
        trace.cycles[1].changes,
        vec![Change {
            source: ChangeSource::Mesh,
            kind: ChangeKind::VariableWrite {
                path: "Main.Level".into(),
                type_name: "INT".into(),
                value: vec![5, 0],
            },
        }]
    );
}
