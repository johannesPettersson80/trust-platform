//! Limit and admission contracts of isolated simulations (specification 35, A2.6 and A7).
use super::{
    record_period::{period_count, period_time_seconds, RecordPeriod},
    run_contracts::RunRequest,
    snapshot::{admit_simulated_host, PHYSICAL_HOST_REFUSAL},
};
use serde_json::{json, Value};

fn run_request(overrides: Value) -> RunRequest {
    let mut request = json!({
        "expected_project_revision": "sha256:revision",
        "execution": "isolated-simulation",
        "step_ms": 1000,
        "duration_ms": 2000,
        "seed": 7,
        "max_total_cycles": 3,
        "wall_budget_ms": 5000,
        "initial_values": {},
        "events": [],
        "observations": [{"symbol": "Main.PV", "unit": "C"}],
        "parameter_overrides": []
    });
    for (key, value) in overrides.as_object().expect("request overrides") {
        request[key.as_str()] = value.clone();
    }
    serde_json::from_value(request).expect("run request deserializes")
}

fn refusal(overrides: Value) -> String {
    run_request(overrides)
        .validate()
        .expect_err("the request must be refused")
}

fn channels(count: usize) -> Value {
    Value::Array(
        (0..count)
            .map(|index| json!({"symbol": format!("Main.S{index}"), "unit": "1"}))
            .collect(),
    )
}

#[test]
fn a_24_hour_isolated_run_passes_validation() {
    let request = run_request(json!({
        "step_ms": 1000,
        "duration_ms": 86_400_000,
        "max_total_cycles": 86_401,
        "wall_budget_ms": 600_000
    }));
    assert!(
        request.validate().is_ok(),
        "24 h at 1 s is within every limit"
    );
}

#[test]
fn a_duration_beyond_24_hours_is_refused_naming_the_limit() {
    let message = refusal(json!({
        "step_ms": 1,
        "duration_ms": 86_400_001,
        "max_total_cycles": 86_400_002
    }));
    assert!(message.contains("86400000"), "{message}");
}

#[test]
fn a_run_beyond_100000_samples_is_refused_unless_record_every_averages_it() {
    let overrides = json!({"step_ms": 1, "duration_ms": 100_000, "max_total_cycles": 100_001});
    let message = refusal(overrides.clone());
    assert!(message.contains("100001 samples per run"), "{message}");
    assert!(message.contains("100000"), "{message}");
    let mut averaged = overrides;
    averaged["record_every"] = json!(2);
    assert!(run_request(averaged).validate().is_ok());
}

#[test]
fn channels_times_samples_at_1600000_are_admitted_and_above_are_refused_naming_the_limit() {
    let at_limit = json!({"step_ms": 1, "duration_ms": 99_999, "max_total_cycles": 100_000, "observations": channels(16)});
    assert!(
        run_request(at_limit).validate().is_ok(),
        "16 channels × 100000 samples is the limit"
    );
    let message = refusal(json!({
        "step_ms": 1,
        "duration_ms": 60_000,
        "max_total_cycles": 60_001,
        "observations": channels(30)
    }));
    assert!(message.contains("channel samples"), "{message}");
    assert!(message.contains("1600000"), "{message}");
}

#[test]
fn observation_units_are_bounded_because_every_header_carries_them() {
    let message =
        refusal(json!({"observations": [{"symbol": "Main.PV", "unit": "u".repeat(257)}]}));
    assert!(
        message.contains("257 bytes") && message.contains("256"),
        "{message}"
    );
    assert!(
        run_request(json!({"observations": [{"symbol": "Main.PV", "unit": "u".repeat(256)}]}))
            .validate()
            .is_ok()
    );
}

#[test]
fn the_derived_output_budgets_are_the_documented_values() {
    use super::worker_checkpoint::MAX_CHECKPOINT;
    assert_eq!(MAX_CHECKPOINT, 56_324_288, "S × 126 + L × 27 + 512 KiB");
    assert_eq!(
        super::worker::MAX_OUTPUT,
        60_397_152,
        "the larger run plus the 1 MiB reserve"
    );
}

/// The largest valid run (S = 100,000 samples, L = 1,600,000 channel samples) written through the
/// real encoder and bounded by the derived budgets. Every value is the longest JSON number and
/// carries a non-good quality, and the header carries the admitted metadata at its maxima.
#[test]
fn a_run_at_the_maximum_channels_times_samples_fits_the_derived_budget() {
    use super::worker_checkpoint::{
        sample_row, FRAME_BYTES, HEADER_BYTES, MAX_CHECKPOINT, QUALITY_BAD, SAMPLE_BYTES,
        TIME_BYTES, VALUE_BYTES,
    };
    let samples = super::execution_settings::MAX_RUN_SAMPLES as usize;
    let channels = 16_usize;
    assert_eq!(samples * channels, 1_600_000, "the maximum channel samples");
    let longest = -1.2345678901234567e-300_f64;
    let text_len = |value: &Value| serde_json::to_string(value).unwrap().len() as u64;
    assert!(
        text_len(&json!(longest)) <= 24,
        "no f64 is written in more than 24 bytes"
    );
    let readings: Vec<(Option<f64>, u64)> = (0..channels)
        .map(|_| (Some(longest), QUALITY_BAD))
        .collect();
    let row = sample_row((samples - 1) as u64, longest, &readings);
    let row_bytes = text_len(&row);
    assert!(
        row_bytes <= SAMPLE_BYTES + channels as u64 * VALUE_BYTES,
        "a sample is at most {SAMPLE_BYTES} + channels × {VALUE_BYTES} bytes, not {row_bytes}"
    );
    let symbol = "s".repeat(256);
    let text = |prefix: &str| format!("{prefix}{}", "x".repeat(256 - prefix.len()));
    let mut header = json!({
        "id": "run-0",
        "evidence": "ST-executed",
        "parameters": (0..64)
            .map(|index| (format!("{symbol}{index}"), json!(longest)))
            .collect::<serde_json::Map<String, Value>>(),
        "conditions": {
            "initialState": (0..64)
                .map(|index| (format!("{symbol}{index}"), json!(longest)))
                .collect::<serde_json::Map<String, Value>>(),
            "events": (0..1024)
                .map(|_| json!({"atSeconds": longest, "channel": symbol, "value": longest}))
                .collect::<Vec<_>>(),
            "durationSeconds": 99.999,
            "seed": 9_007_199_254_740_991_u64,
            "modelRevision": text("model-"),
            "programRevision": format!("sha256:{}", "0".repeat(64)),
            "parameterSchemaRevision": format!("sha256:{}", "0".repeat(64)),
            "modelProvenance": "p".repeat(4096),
            "simulationConfigRevision": format!("sha256:{}", "0".repeat(64)),
            "sampleTimesSeconds": [],
        },
        "channels": (0..channels)
            .map(|index| json!({"id": format!("{symbol}{index}"), "unit": "u".repeat(256)}))
            .collect::<Vec<_>>(),
        "samples": [],
    });
    header["conditions"]["sampleTimesSeconds"] = Value::Array(vec![json!(longest); samples]);
    let header_bytes = text_len(&header);
    assert!(
        header_bytes <= HEADER_BYTES + samples as u64 * TIME_BYTES,
        "the header's metadata stays within its 512 KiB allowance beyond the planned times"
    );
    // Each frame carries its prefix and newline; the header frame is one as well.
    let checkpoint = header_bytes + FRAME_BYTES + samples as u64 * (row_bytes + FRAME_BYTES);
    assert!(
        checkpoint <= MAX_CHECKPOINT as u64,
        "{checkpoint} exceeds {MAX_CHECKPOINT}"
    );
    let streamed_run = checkpoint + header_bytes + 4096;
    assert!(streamed_run <= super::worker::MAX_OUTPUT, "{streamed_run}");
    // A search trial retains its complete run in one result.
    let trial = header_bytes + samples as u64 * (row_bytes + 1);
    assert!(trial <= super::worker::MAX_OUTPUT, "{trial}");
}

#[test]
fn more_than_64_observed_channels_are_refused_naming_the_limit() {
    let message = refusal(json!({"observations": channels(65)}));
    assert!(message.contains("65 observed channels"), "{message}");
    assert!(message.contains("limit of 64"), "{message}");
}

#[test]
fn wall_budgets_and_total_scans_are_refused_naming_their_limits() {
    let per_run = refusal(json!({"wall_budget_ms": 600_001}));
    assert!(per_run.contains("600000"), "{per_run}");
    let job = refusal(json!({"wall_budget_ms": 3_600_001}));
    assert!(job.contains("3600000"), "{job}");
    let scans = refusal(json!({"max_total_cycles": 10_000_001}));
    assert!(scans.contains("10000000"), "{scans}");
}

#[test]
fn record_every_stays_within_its_period_limit() {
    assert!(refusal(json!({"record_every": 0})).contains("record_every"));
    let beyond = refusal(json!({"record_every": 100_001}));
    assert!(beyond.contains("100000"), "{beyond}");
}

#[test]
fn a_record_period_is_the_mean_of_its_grid_points_and_timed_at_their_mean() {
    // Ten grid points of 1 ms in periods of three: [0..2], [3..5], [6..8] and [9].
    assert_eq!(period_count(10, 3), 4);
    assert_eq!(period_time_seconds(0, 3, 9, 1), 0.001);
    assert_eq!(period_time_seconds(1, 3, 9, 1), 0.004);
    assert_eq!(period_time_seconds(3, 3, 9, 1), 0.009);
    let mut period = RecordPeriod::default();
    period.push(&[1.0, 10.0]);
    period.push(&[2.0, 20.0]);
    period.push(&[3.0, 30.0]);
    assert_eq!(period.means(), vec![2.0, 20.0]);
}

#[test]
fn a_held_value_comes_back_exactly_from_a_record_period() {
    // 0.1 + 0.1 + 0.1 is 0.30000000000000004 in floating point, so its sum divided by three is not 0.1 (A4.2).
    let mut period = RecordPeriod::default();
    for _ in 0..3 {
        period.push(&[0.1, 1.0]);
    }
    assert_eq!(period.means(), vec![0.1, 1.0]);
    let mut missing = RecordPeriod::default();
    missing.push(&[f64::NAN]);
    missing.push(&[1.0]);
    assert!(
        missing.means()[0].is_nan(),
        "a non-finite reading is never hidden"
    );
}

#[test]
fn a_runtime_reporting_simulation_and_simulated_io_is_admitted_however_it_started() {
    let native = simulated_native();
    assert!(admit_simulated_host(Some(&native)).is_ok());
}

#[test]
fn a_physical_unknown_or_absent_host_is_refused_with_the_physical_target_message() {
    let mut physical = simulated_native();
    physical.simulation_only_io = Some(false);
    let mut unknown = simulated_native();
    unknown.simulation_only_io = None;
    let mut disabled = simulated_native();
    disabled.status.active = false;
    for native in [Some(&physical), Some(&unknown), Some(&disabled), None] {
        let message = admit_simulated_host(native).expect_err("the host must be refused");
        assert_eq!(message.as_str(), PHYSICAL_HOST_REFUSAL);
    }
    assert!(PHYSICAL_HOST_REFUSAL.contains("physical target's host"));
    assert!(PHYSICAL_HOST_REFUSAL.contains("local analysis runtime"));
}

fn simulated_native() -> crate::simulation::SimulationRuntimeSnapshot {
    let config = crate::simulation::SimulationConfig::from_toml_text(
        r#"[simulation]
enabled = true
plant_step_ms = 20
[[plants]]
id = "engineer-plant"
model = "first_order"
command_sources = ["%QL0"]
feedback_targets = ["%IL0"]
initial_state = [0.0]
[plants.parameters]
gain = 1.0
time_constant_s = 0.1
"#,
        "limit contract",
    )
    .expect("simulation configuration");
    crate::simulation::SimulationController::new(config)
        .runtime_snapshot_for(&crate::Runtime::new())
}
