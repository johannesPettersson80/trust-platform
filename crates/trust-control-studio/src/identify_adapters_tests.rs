//! The run's live sample waits for the runtime's next coherent scan (runtime protocol annex E3 rule 8; specification 35, B3.4
//! step 1).

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use serde_json::{json, Value};
use trust_control::recording::EvidenceSource;

use super::RuntimeSampler;
use crate::runtime_client::fake_runtime::{answer, tank_loop, FakeRuntime};
use crate::runtime_client::INSPECTION_RETRIES;

/// The runtime's answer to an inspection before its next scan (E3).
fn waiting(params: &Value) -> Value {
    json!({
        "available": false, "program_revision": "program",
        "loop_revision": params["loop_revision"], "bindings": params["bindings"],
        "reason": "Waiting for the next coherent runtime scan."
    })
}

/// The runtime's answer with scan 7 of every requested binding, each value good (E3).
fn scanned(params: &Value) -> Value {
    let count = params["bindings"].as_array().map_or(0, Vec::len);
    json!({
        "available": true, "program_revision": "program",
        "loop_revision": params["loop_revision"], "bindings": params["bindings"],
        "scan": 7, "runtime_time_ns": 70_000_000, "captured_at_ms": 1000,
        "values": vec![json!(1.0); count], "forced": vec![false; count],
        "quality": vec!["good"; count]
    })
}

#[test]
fn a_runs_first_sample_waits_for_the_scan_after_it_took_over_the_inspection_watch() {
    // Catches: the runtime keeps one inspection watch, so the first read after another reader's watch answers "Waiting for the
    // next coherent runtime scan." (E3 rule 1). The run took that answer as no sample, and Automatic refused the loop with "the
    // loop's Auto/Manual mode is unavailable" although the next scan was a few milliseconds away.
    let served = Arc::new(AtomicUsize::new(0));
    let counter = Arc::clone(&served);
    let fake = FakeRuntime::start(move |request| match request["type"].as_str() {
        Some("control.signals.inspect") => {
            let params = &request["params"];
            let reply = if counter.fetch_add(1, Ordering::SeqCst) < 2 {
                waiting(params)
            } else {
                scanned(params)
            };
            Some(answer(request, reply))
        }
        _ => None,
    });
    let connection = fake.connection(None);
    let loop_definition = tank_loop();
    let sampler = RuntimeSampler::new(
        &connection,
        &loop_definition,
        EvidenceSource::SimulatedPlant,
    );

    let scan = sampler
        .scan()
        .expect("the runtime answers")
        .expect("the scan that follows the two waits");

    assert_eq!(scan.scan, 7, "the sample is the runtime's next scan");
    let inspections = fake.requests_of("control.signals.inspect");
    assert_eq!(
        inspections.len(),
        3,
        "two answers that wait for the next scan, then the scan (E3 rule 8)"
    );
    assert_eq!(
        inspections[0]["params"].get("expected_program_revision"),
        None,
        "the first read names no program revision"
    );
    assert_eq!(
        inspections[1]["params"]["expected_program_revision"],
        json!("program"),
        "each later read is pinned to the program revision the first answer named (E11.4)"
    );
}

#[test]
fn a_sample_the_runtime_never_scans_is_unavailable_after_the_retries() {
    // Catches a run that waits without end for a runtime that never completes a scan: after the first read and twenty more, the
    // sample is unavailable, and the run's own refusal names the cause (E3 rule 8, B3.9).
    let fake = FakeRuntime::start(|request| match request["type"].as_str() {
        Some("control.signals.inspect") => Some(answer(request, waiting(&request["params"]))),
        _ => None,
    });
    let connection = fake.connection(None);
    let loop_definition = tank_loop();
    let sampler = RuntimeSampler::new(
        &connection,
        &loop_definition,
        EvidenceSource::SimulatedPlant,
    );

    assert!(
        sampler.scan().expect("the runtime answers").is_none(),
        "no scan within the retries is no sample"
    );
    assert_eq!(
        fake.requests_of("control.signals.inspect").len(),
        INSPECTION_RETRIES + 1,
        "the first read and twenty more (E3 rule 8)"
    );
    assert_eq!(INSPECTION_RETRIES, 20, "E3 rule 8 names twenty more reads");
}
