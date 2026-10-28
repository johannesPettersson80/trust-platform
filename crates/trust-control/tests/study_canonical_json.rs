//! The canonical text of a retained run (specification 35, A8.3 and B10.2): the deterministic JSON that evidence equality compares.
//! Every retained draft canonicalizes each stored run several times, so the cost must stay close to one pass over the run's text
//! (`control-studio-canonical-json.test.ts`, "canonicalizes a long retained run in one pass"). The rules of the order are pinned by the
//! unit tests of `study.rs`; these tests use a run as long as a Validate campaign stores it.

use std::hint::black_box;
use std::time::{Duration, Instant};

use serde_json::{json, Value};
use trust_control::study::canonical_json;

/// A retained verification run as a Validate campaign stores it: three channels and `samples` samples, 20 ms apart.
fn recorded_run(samples: u32) -> Value {
    let channels: Vec<Value> = ["P1.AngleRad", "P1.TorqueNm", "P1.VelocityRadS"]
        .iter()
        .map(|id| json!({ "unit": "x", "id": id }))
        .collect();
    let rows: Vec<Value> = (0..samples)
        .map(|index| {
            let step = f64::from(index);
            json!({
                "timeSeconds": step / 50.0,
                "values": {
                    "P1.AngleRad": { "value": (step / 40.0).sin(), "quality": "good" },
                    "P1.TorqueNm": { "value": (step / 40.0).cos(), "quality": "good" },
                    "P1.VelocityRadS": { "value": step / 1000.0, "quality": "good" }
                }
            })
        })
        .collect();
    json!({
        "conditions": { "durationSeconds": f64::from(samples) / 50.0, "seed": 7 },
        "channels": channels,
        "samples": rows
    })
}

/// The shortest of `runs` timings of `work`.
fn best_of(runs: u32, mut work: impl FnMut()) -> Duration {
    (0..runs)
        .map(|_| {
            let started = Instant::now();
            work();
            started.elapsed()
        })
        .min()
        .unwrap_or_default()
}

#[test]
fn a_long_retained_run_has_sorted_keys_at_every_depth_and_reads_back_to_the_run() {
    let run = recorded_run(12_000);
    let text = canonical_json(&run);
    assert!(
        text.len() > 1_000_000,
        "a 12,000-sample run is more than a megabyte of text: {}",
        text.len()
    );
    assert!(
        text.starts_with(r#"{"channels":[{"id":"P1.AngleRad","unit":"x"}"#),
        "the members of the run and of its channels are in key order: {}",
        &text[..80]
    );
    assert!(
        text.contains(r#""samples":[{"timeSeconds":0.0,"values":{"P1.AngleRad":{"quality":"good","value":0.0}"#),
        "the members of a sample and of its values are in key order"
    );
    let read_back: Value = serde_json::from_str(&text).expect("the canonical text is JSON");
    assert_eq!(read_back, run, "the canonical text is the run");
    assert_eq!(
        canonical_json(&read_back),
        text,
        "canonicalizing the canonical text changes nothing"
    );
}

#[test]
fn canonicalizing_four_times_the_samples_takes_about_four_times_as_long() {
    let small = recorded_run(4_000);
    let large = recorded_run(16_000);
    let small_time = best_of(3, || {
        assert_ne!(black_box(canonical_json(black_box(&small))), "");
    });
    let large_time = best_of(3, || {
        assert_ne!(black_box(canonical_json(black_box(&large))), "");
    });
    let growth = large_time.as_secs_f64() / small_time.as_secs_f64().max(1e-6);
    assert!(
        growth < 10.0,
        "four times the samples took {growth:.1} times as long: a pass that grows faster than the text is not one pass"
    );
}
