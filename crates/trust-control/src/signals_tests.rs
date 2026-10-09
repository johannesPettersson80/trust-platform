//! Annex E3 and specification 35, A3.1 and A3.4: signal reads and the freshness of readings.

// Process times and BOOL values are exact, so they are compared exactly.
#![allow(clippy::float_cmp)]

use serde_json::{json, Value};

use super::*;
use crate::limits::CONNECTION_LOSS_TIMEOUT_MS;

fn bindings() -> Vec<SignalBinding> {
    vec![
        SignalBinding {
            id: "pv".into(),
            address: None,
            symbol: Some("Main.PV".into()),
            iec_type: "LREAL".into(),
        },
        SignalBinding {
            id: "mode".into(),
            address: None,
            symbol: Some("Main.Manual".into()),
            iec_type: "BOOL".into(),
        },
    ]
}

fn bindings_json() -> Value {
    json!([{ "id": "pv", "symbol": "Main.PV", "iec_type": "LREAL" }, { "id": "mode", "symbol": "Main.Manual", "iec_type": "BOOL" }])
}

fn available(loop_revision: &str, program_revision: &str) -> Value {
    json!({ "program_revision": program_revision, "loop_revision": loop_revision, "bindings": bindings_json(),
        "available": true, "scan": 42, "runtime_time_ns": 42_000_000, "captured_at_ms": 1_700_000_000_000_u64,
        "values": [21.5, 1], "forced": [false, false], "quality": ["good", "good"] })
}

#[test]
fn requests_the_exact_bindings_and_retains_one_coherent_available_scan() {
    let request = InspectionRequest::new("sha256:loop-1", Some("sha256:program-1"), bindings())
        .expect("valid");
    assert_eq!(
        serde_json::to_value(&request).expect("json"),
        json!({ "loop_revision": "sha256:loop-1", "expected_program_revision": "sha256:program-1", "bindings": bindings_json() })
    );
    let Inspection::Available(sample) = request
        .decode(&available("sha256:loop-1", "sha256:program-1"))
        .expect("decoded")
    else {
        panic!("a scan was available");
    };
    assert_eq!(
        (sample.scan, sample.values.clone()),
        (42, vec![Some(21.5), Some(1.0)])
    );
    assert_eq!(sample.usable("pv"), Some(21.5));
    assert_eq!(sample.usable("mode"), Some(1.0));
}

#[test]
fn retains_pending_provenance_without_manufacturing_a_sample() {
    let request = InspectionRequest::new("sha256:loop-2", None, bindings()).expect("valid");
    let pending = json!({ "program_revision": "sha256:program-2", "loop_revision": "sha256:loop-2", "bindings": bindings_json(),
        "available": false, "reason": "Waiting for the next coherent runtime scan." });
    assert_eq!(
        request.decode(&pending).expect("decoded"),
        Inspection::Pending {
            program_revision: "sha256:program-2".into(),
            reason: "Waiting for the next coherent runtime scan.".into()
        }
    );
}

#[test]
fn a_captured_answer_without_a_scan_is_pending_even_without_its_reason() {
    let request = InspectionRequest::new("sha256:loop-2", None, bindings()).expect("valid");
    let captured = json!({ "program_revision": "sha256:program-2", "loop_revision": "sha256:loop-2", "bindings": bindings_json(),
        "available": false, "scan": null, "runtime_time_ns": -1, "captured_at_ms": 1, "values": [null, null],
        "forced": [false, false], "quality": ["missing", "missing"] });
    assert_eq!(
        request.decode(&captured).expect("decoded"),
        Inspection::Pending {
            program_revision: "sha256:program-2".into(),
            reason: "Waiting for the next coherent runtime scan.".into()
        }
    );
    let mut negative = available("sha256:loop-2", "sha256:program-2");
    negative["runtime_time_ns"] = json!(-1);
    assert!(
        request.decode(&negative).is_err(),
        "an available scan carries its runtime time"
    );
}

#[test]
fn rejects_duplicate_binding_ids_and_ambiguous_selectors_before_calling_the_runtime() {
    let mut duplicate = bindings();
    duplicate[1].id = "pv".into();
    assert!(InspectionRequest::new("sha256:loop", None, duplicate).is_err());
    let mut both = bindings();
    both[0].address = Some("%IL0".into());
    assert!(InspectionRequest::new("sha256:loop", None, both).is_err());
    assert!(InspectionRequest::new("sha256:loop", None, Vec::new()).is_err());
    let many = (0..33).map(|index| SignalBinding {
        id: format!("s{index}"),
        address: None,
        symbol: Some("Main.X".into()),
        iec_type: "LREAL".into(),
    });
    assert!(
        InspectionRequest::new("sha256:loop", None, many.collect()).is_err(),
        "at most 32 channels"
    );
}

#[test]
fn rejects_a_mismatched_imprecise_or_misaligned_response() {
    let request = InspectionRequest::new("sha256:loop-3", Some("sha256:program-3"), bindings())
        .expect("valid");
    let valid = available("sha256:loop-3", "sha256:program-3");
    let with = |key: &str, value: Value| {
        let mut changed = valid.clone();
        changed[key] = value;
        changed
    };
    for invalid in [
        with("loop_revision", json!("sha256:other")),
        with("program_revision", json!("sha256:program-other")),
        with("scan", json!(9_007_199_254_740_992_u64)),
        with("quality", json!(["good"])),
        with(
            "bindings",
            json!([{ "id": "pv", "symbol": "Main.PV", "iec_type": "LREAL" }, { "id": "mode", "address": "%MX0.0", "iec_type": "BOOL" }]),
        ),
    ] {
        let failure = request.decode(&invalid).expect_err("refused");
        assert!(
            failure.message.contains("does not match the protocol"),
            "{}",
            failure.message
        );
    }
}

#[test]
fn preserves_bad_and_missing_values_but_never_calls_them_usable() {
    let request = InspectionRequest::new("sha256:loop-4", None, bindings()).expect("valid");
    let mut response = available("sha256:loop-4", "sha256:program-4");
    response["values"] = json!([null, 0.5]);
    response["quality"] = json!(["missing", "good"]);
    let Inspection::Available(sample) = request.decode(&response).expect("decoded") else {
        panic!("available")
    };
    assert_eq!(sample.values, vec![None, Some(0.5)]);
    assert_eq!(sample.usable("pv"), None);
    assert_eq!(sample.usable("mode"), None, "a BOOL is 0 or 1");
    let mut forced = sample.clone();
    forced.values[0] = Some(3.0);
    forced.quality[0] = Quality::Good;
    forced.forced[0] = true;
    assert_eq!(forced.usable("pv"), None, "a forced value is never usable");
}

#[test]
fn measures_process_time_from_the_runtimes_own_timestamps_not_from_the_host_clock() {
    let mut clock = ReadingClock::new(0);
    assert_eq!(
        clock.observe(0, 0),
        0.0,
        "the first reading starts the process time"
    );
    assert_eq!(
        clock.observe(2_500_000_000, 0),
        2_500.0,
        "2.5 s of runtime time is 2500 ms, whatever the wall clock says"
    );
}

#[test]
fn stops_when_no_newer_runtime_value_arrives_within_the_connection_loss_timeout() {
    let mut clock = ReadingClock::new(0);
    clock.observe(1_000_000_000, 0);
    assert_eq!(
        clock.observe(1_000_000_000, 4_000),
        0.0,
        "a repeated scan is not a newer value"
    );
    assert!(
        clock
            .ensure_fresh(4_000, CONNECTION_LOSS_TIMEOUT_MS)
            .is_ok(),
        "4 s without a newer value is within the 5 s timeout"
    );
    let failure = clock
        .ensure_fresh(6_000, CONNECTION_LOSS_TIMEOUT_MS)
        .expect_err("6 s without a newer value ends the wait");
    assert_eq!(failure.message, "The runtime stopped sending values.");
}

/// A loop with an addressed mode bit and a parameter record whose mode variable is a symbol.
fn loop_with_modes() -> LoopDefinition {
    let text = include_str!("../../../examples/control-pid-commissioning/control-loop.json");
    LoopDefinition::decode(text).expect("the example loop decodes")
}

#[test]
fn one_watch_reads_the_loops_signals_and_its_mode_variables_once() {
    let loop_definition = loop_with_modes();
    let bindings = loop_inspection_bindings(&loop_definition);
    let ids: Vec<&str> = bindings.iter().map(|binding| binding.id.as_str()).collect();
    assert_eq!(
        &ids[..3],
        ["setpoint", "processVariable", "manipulatedVariable"]
    );
    for binding in &bindings {
        assert!(
            binding.address.is_some() != binding.symbol.is_some(),
            "{} has exactly one selector",
            binding.id
        );
    }
    let mode = loop_definition
        .controller
        .mode_signal
        .as_deref()
        .expect("the example declares a mode signal");
    let mode_id =
        loop_inspection_mode_id(&loop_definition, &bindings, mode).expect("the mode is read");
    assert_eq!(
        bindings
            .iter()
            .filter(|binding| binding.id == mode_id)
            .count(),
        1
    );
    assert!(
        InspectionRequest::new(&loop_definition.revision, None, bindings).is_ok(),
        "the selection is a valid request"
    );
}

#[test]
fn a_mode_bit_address_is_its_own_address_and_a_symbol_has_none() {
    let loop_definition = loop_with_modes();
    assert_eq!(
        controller_mode_address(&loop_definition, "%mx0.1"),
        Some("%MX0.1".into())
    );
    assert_eq!(
        controller_mode_address(&loop_definition, "%MW0"),
        None,
        "only a bit address is a mode address"
    );
    assert_eq!(controller_mode_address(&loop_definition, "Main.Auto"), None);
}
