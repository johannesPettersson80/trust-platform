//! The I/O state as Live Values shows it (specification 25 §4.3).

use crate::protocol::{IoStateEntry, IoStateEventBody};

use super::display_io_state;

fn entry(
    address: &str,
    value: &str,
    value_type: Option<&str>,
    source: Option<&str>,
) -> IoStateEntry {
    IoStateEntry {
        name: Some(format!("name of {address}")),
        address: address.to_owned(),
        source: source.map(str::to_owned),
        value_type: value_type.map(str::to_owned),
        value: value.to_owned(),
        forced: false,
        writable: Some(true),
    }
}

/// A truth value is `TRUE` or `FALSE` and a number its digits, `USInt` included; any other value
/// stays as the runtime wrote it. Catches `Bool(true)` in the Value column, and an unsigned short
/// shown as `USInt(5)`, as before 26 October 2026.
#[test]
fn values_are_shown_in_iec_text() {
    let body = IoStateEventBody {
        scan: Some(7),
        inputs: vec![
            entry("%IX0.0", "Bool(true)", Some("bool"), None),
            entry("%IX0.1", " BOOL(FALSE) ", None, None),
            entry("%IW0", "Int(-12)", Some("int"), None),
            entry("%IB2", "USInt(5)", None, None),
            entry("%ID1", "LReal(2.5)", None, None),
            entry("%IX0.2", "Time(1s)", None, None),
            entry("%IX0.3", "unresolved", None, None),
        ],
        outputs: Vec::new(),
        memory: Vec::new(),
    };
    let shown = display_io_state(&body);
    let values: Vec<&str> = shown
        .inputs
        .iter()
        .map(|entry| entry.value.as_str())
        .collect();
    assert_eq!(
        values,
        ["TRUE", "FALSE", "-12", "5", "2.5", "Time(1s)", "unresolved"]
    );
    assert_eq!(shown.scan, Some(7));
    assert_eq!(shown.inputs[0].value_type.as_deref(), Some("BOOL"));
    assert_eq!(shown.inputs[2].value_type.as_deref(), Some("INT"));
    assert_eq!(shown.inputs[0].writable, Some(true));
}

/// An entry without an address or a value is left out, and the globals an ADS tag binds are listed
/// apart, in the order inputs, outputs, memory. Catches an ADS tag shown twice, among the outputs
/// and as an ADS value, and an empty row.
#[test]
fn ads_globals_are_listed_apart() {
    let body = IoStateEventBody {
        scan: None,
        inputs: vec![
            entry("", "Bool(true)", None, None),
            entry("%IX0.0", "", None, None),
            entry("global:Speed", "Real(1.5)", None, Some("ADS line1.speed")),
        ],
        outputs: vec![
            entry("global:Mode", "Int(2)", None, Some(" ADS ")),
            entry("global:Other", "Int(3)", None, Some("ADSX")),
            entry("%QX0.0", "Bool(false)", None, Some("ADS tag")),
        ],
        memory: vec![entry("global:Flag", "Bool(true)", None, Some("ads"))],
    };
    let shown = display_io_state(&body);
    assert!(shown.inputs.is_empty());
    let addresses = |entries: &[IoStateEntry]| -> Vec<String> {
        entries.iter().map(|entry| entry.address.clone()).collect()
    };
    assert_eq!(addresses(&shown.outputs), ["global:Other", "%QX0.0"]);
    assert_eq!(
        addresses(&shown.ads),
        ["global:Speed", "global:Mode", "global:Flag"]
    );
    assert!(shown.memory.is_empty());
}
