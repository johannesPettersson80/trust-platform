//! Enforce the frozen H hosted performance and PLC outcome comparison.
use anyhow::{ensure, Context, Result};
use serde_json::{json, Value};

const MEDIAN_RATIO_MAX: f64 = 1.10;
const P99_RATIO_MAX: f64 = 1.25;
const REQUIRED_WATCHES: [&str; 5] = [
    "g_motion_bench_cycles",
    "g_motion_bench_completed_sequences",
    "g_motion_bench_last_error",
    "g_motion_bench_power_on",
    "g_motion_bench_is_homed",
];

fn report(input: &Value) -> Result<&Value> {
    ensure!(
        input["benchmark"] == "project",
        "expected a project benchmark"
    );
    let report = input.get("report").context("missing project report")?;
    for key in ["project", "resource_name", "execution_backend"] {
        ensure!(
            report[key].as_str().is_some_and(|text| !text.is_empty()),
            "missing benchmark identity {key}"
        );
    }
    for key in [
        "samples",
        "warmup_cycles",
        "total_cycles",
        "budget_overruns",
    ] {
        ensure!(
            report[key].as_u64().is_some(),
            "missing unsigned benchmark field {key}"
        );
    }
    ensure!(
        report["watched_globals"]
            .as_object()
            .is_some_and(|values| !values.is_empty()),
        "missing watched PLC values"
    );
    for watch in REQUIRED_WATCHES {
        ensure!(
            !report["watched_globals"][watch].is_null(),
            "missing required PLC watch {watch}"
        );
    }
    for key in ["register_programs_executed", "register_program_fallbacks"] {
        ensure!(
            report["vm_profile"][key].as_u64().is_some(),
            "missing register observation {key}"
        );
    }
    for key in ["hot_block_threshold", "cache_capacity", "block_executions"] {
        ensure!(
            report["vm_profile"]["tier1_specialized_executor"][key]
                .as_u64()
                .is_some(),
            "missing tier-1 observation {key}"
        );
    }
    for key in ["input_artifact_sha256", "workload_sha256"] {
        let digest = report[key]
            .as_str()
            .context("missing benchmark input digest")?;
        ensure!(
            digest.len() == 64 && digest.bytes().all(|byte| byte.is_ascii_hexdigit()),
            "invalid benchmark input digest {key}"
        );
    }
    let configuration = &report["configuration"];
    let cycle_budget_ns = configuration["cycle_interval_nanos"]
        .as_u64()
        .filter(|value| *value > 0)
        .context("missing positive cycle budget")?;
    ensure!(
        configuration["resource_label"]
            .as_str()
            .is_some_and(|label| !label.is_empty())
            && configuration["io"] == "none"
            && configuration["register"] == true
            && configuration["tier1"] == true
            && configuration["lowering_capacity"]
                .as_u64()
                .is_some_and(|capacity| capacity > 0),
        "missing benchmark configuration"
    );
    let trace = report["cycle_trace"]
        .as_array()
        .context("missing complete cycle trace")?;
    ensure!(trace.len() == 1152, "incomplete cycle trace");
    for (index, row) in trace.iter().enumerate() {
        ensure!(
            row["cycle"].as_u64() == Some(index as u64 + 1),
            "nonsequential cycle trace"
        );
        ensure!(
            row["logical_time_nanos"].as_i64().is_some(),
            "missing logical cycle time"
        );
        ensure!(
            row["faulted"] == false && row.get("last_fault") == Some(&Value::Null),
            "missing or faulted cycle state"
        );
        for area in ["inputs", "outputs", "memory"] {
            ensure!(
                row["process_images"][area]
                    .as_array()
                    .is_some_and(|bytes| bytes
                        .iter()
                        .all(|byte| byte.as_u64().is_some_and(|value| value <= 255))),
                "missing or invalid process image {area}"
            );
        }
        ensure!(
            row["task_overruns"]
                .as_array()
                .is_some_and(|rows| rows
                    .iter()
                    .all(|task| task["name"].as_str().is_some()
                        && task["missed"].as_u64() == Some(0))),
            "missing or nonzero task misses in cycle trace"
        );
        for watch in REQUIRED_WATCHES {
            ensure!(
                !row["watched_globals"][watch].is_null(),
                "missing trace watch {watch}"
            );
        }
    }
    ensure!(
        trace.last().context("empty trace")?["watched_globals"] == report["watched_globals"],
        "final PLC values differ from cycle trace"
    );
    let mut samples: Vec<f64> = report["samples_us"]
        .as_array()
        .context("missing raw latency samples")?
        .iter()
        .map(|value| value.as_f64().context("invalid raw latency sample"))
        .collect::<Result<_>>()?;
    ensure!(
        samples.len() == 1024
            && samples
                .iter()
                .all(|value| value.is_finite() && *value > 0.0),
        "invalid raw latency allocation"
    );
    let samples_ns: Vec<u64> = report["samples_ns"]
        .as_array()
        .context("missing raw nanoseconds")?
        .iter()
        .map(|value| {
            value
                .as_u64()
                .filter(|value| *value > 0)
                .context("invalid nanosecond sample")
        })
        .collect::<Result<_>>()?;
    ensure!(
        samples_ns.len() == samples.len()
            && samples_ns
                .iter()
                .zip(&samples)
                .all(|(nanos, micros)| *nanos as f64 / 1000.0 == *micros),
        "inconsistent raw latency units"
    );
    ensure!(
        samples_ns
            .iter()
            .filter(|nanos| **nanos > cycle_budget_ns)
            .count() as u64
            == report["budget_overruns"]
                .as_u64()
                .context("missing overrun count")?,
        "cycle overrun count differs from raw samples"
    );
    samples.sort_by(f64::total_cmp);
    ensure!(
        latency(report, "p50_us")? == samples[511] && latency(report, "p99_us")? == samples[1013],
        "latency quantiles do not match nearest-rank raw samples"
    );
    Ok(report)
}

fn latency(report: &Value, key: &str) -> Result<f64> {
    let value = report["cycle_latency"][key]
        .as_f64()
        .with_context(|| format!("missing latency {key}"))?;
    ensure!(value.is_finite() && value > 0.0, "invalid latency {key}");
    Ok(value)
}

pub(super) fn compare(baseline: &Value, candidate: &Value) -> Result<Value> {
    let baseline = report(baseline)?;
    let candidate = report(candidate)?;
    let median_ratio = latency(candidate, "p50_us")? / latency(baseline, "p50_us")?;
    let p99_ratio = latency(candidate, "p99_us")? / latency(baseline, "p99_us")?;
    ensure!(
        median_ratio.is_finite() && p99_ratio.is_finite(),
        "invalid latency ratio"
    );
    let mut failures = Vec::new();
    if median_ratio > MEDIAN_RATIO_MAX {
        failures.push("median regression");
    }
    if p99_ratio > P99_RATIO_MAX {
        failures.push("p99 regression");
    }
    for key in [
        "project",
        "resource_name",
        "execution_backend",
        "samples",
        "warmup_cycles",
        "total_cycles",
        "watched_globals",
        "input_artifact_sha256",
        "workload_sha256",
        "configuration",
        "cycle_trace",
    ] {
        if baseline[key] != candidate[key] {
            failures.push(key);
        }
    }
    for item in [baseline, candidate] {
        if item["samples"] != 1024 || item["warmup_cycles"] != 128 {
            failures.push("sample allocation");
        }
        if item["total_cycles"] != 1152 {
            failures.push("cycle allocation");
        }
        if item["budget_overruns"] != 0 {
            failures.push("budget overrun");
        }
        if item["vm_profile"]["enabled"] != true {
            failures.push("register telemetry disabled");
        }
        if item["vm_profile"]["register_programs_executed"] == 0 {
            failures.push("no register execution");
        }
        if item["vm_profile"]["register_program_fallbacks"] != 0 {
            failures.push("register fallback");
        }
        let tier1 = &item["vm_profile"]["tier1_specialized_executor"];
        if tier1["enabled"] != true || tier1["block_executions"] == 0 {
            failures.push("no tier-1 execution");
        }
        if item["watched_globals"]
            .as_object()
            .is_some_and(|values| values.values().any(Value::is_null))
        {
            failures.push("unresolved PLC watch");
        }
    }
    for key in ["hot_block_threshold", "cache_capacity"] {
        if baseline["vm_profile"]["tier1_specialized_executor"][key]
            != candidate["vm_profile"]["tier1_specialized_executor"][key]
        {
            failures.push(key);
        }
    }
    Ok(
        json!({"passed":failures.is_empty(), "median_ratio":median_ratio, "p99_ratio":p99_ratio,
        "median_ratio_max":MEDIAN_RATIO_MAX, "p99_ratio_max":P99_RATIO_MAX, "failures":failures}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    fn input() -> Value {
        let mut value = json!({"benchmark":"project", "report": {"samples":1024, "warmup_cycles":128,
            "project":"same-workload", "resource_name":"R", "execution_backend":"bytecode-vm",
            "total_cycles":1152, "budget_overruns":0, "watched_globals":{
                "g_motion_bench_cycles":1152, "g_motion_bench_completed_sequences":42,
                "g_motion_bench_last_error":0, "g_motion_bench_power_on":false,
                "g_motion_bench_is_homed":false},
            "cycle_latency":{"p50_us":10.0,"p99_us":20.0},
            "vm_profile":{"enabled":true,"register_programs_executed":1152,"register_program_fallbacks":0,
                "tier1_specialized_executor":{"enabled":true,"hot_block_threshold":16,"cache_capacity":512,"block_executions":1000}}}});
        value["report"]["input_artifact_sha256"] = json!("a".repeat(64));
        value["report"]["workload_sha256"] = json!("b".repeat(64));
        value["report"]["configuration"] = json!({"resource_label":"R","cycle_interval_nanos":10000000,"io":"none",
            "register":true,"tier1":true,"lowering_capacity":256});
        let watched = value["report"]["watched_globals"].clone();
        value["report"]["cycle_trace"] = json!((0..1152)
            .map(|index| json!({
                "cycle":index+1, "logical_time_nanos":index*10000000_i64,
                "faulted":false, "last_fault":null, "process_images":{"inputs":[],"outputs":[],"memory":[]},
            "task_overruns":[], "watched_globals":watched
            }))
            .collect::<Vec<_>>());
        set_latency(&mut value, 10.0, 20.0);
        value
    }
    fn set_latency(value: &mut Value, median: f64, p99: f64) {
        let mut samples = vec![median; 1013];
        samples.extend(vec![p99; 11]);
        value["report"]["samples_ns"] = json!(samples
            .iter()
            .map(|micros| (*micros * 1000.0).round() as u64)
            .collect::<Vec<_>>());
        value["report"]["samples_us"] = json!(samples);
        value["report"]["cycle_latency"] = json!({"p50_us":median,"p99_us":p99});
    }
    #[test]
    fn compares_both_latency_quantiles_at_the_frozen_boundaries() {
        let baseline = input();
        let mut candidate = input();
        set_latency(&mut candidate, 11.0, 25.0);
        assert_eq!(compare(&baseline, &candidate).unwrap()["passed"], true);
        for (field, value) in [("p50_us", 11.01), ("p99_us", 25.01)] {
            let mut rejected = candidate.clone();
            let (median, p99) = if field == "p50_us" {
                (value, 25.0)
            } else {
                (11.0, value)
            };
            set_latency(&mut rejected, median, p99);
            assert_eq!(compare(&baseline, &rejected).unwrap()["passed"], false);
        }
    }
    #[test]
    fn process_success_cannot_hide_changed_plc_values_or_fallbacks() {
        let baseline = input();
        for candidate in [
            {
                let mut value = input();
                value["report"]["cycle_trace"][17]["watched_globals"]
                    ["g_motion_bench_completed_sequences"] = json!(41);
                value
            },
            {
                let mut value = input();
                value["report"]["vm_profile"]["register_program_fallbacks"] = json!(1);
                value
            },
            {
                let mut value = input();
                value["report"]["budget_overruns"] = json!(1);
                value["report"]["samples_ns"][0] = json!(20000000);
                value["report"]["samples_us"][0] = json!(20000.0);
                value
            },
            {
                let mut value = input();
                value["report"]["vm_profile"]["register_programs_executed"] = json!(0);
                value
            },
            {
                let mut value = input();
                value["report"]["total_cycles"] = json!(1024);
                value
            },
            {
                let mut value = input();
                value["report"]["vm_profile"]["tier1_specialized_executor"]["enabled"] =
                    json!(false);
                value
            },
            {
                let mut value = input();
                value["report"]["vm_profile"]["tier1_specialized_executor"]["block_executions"] =
                    json!(0);
                value
            },
            {
                let mut value = input();
                value["report"]["vm_profile"]["tier1_specialized_executor"]["cache_capacity"] =
                    json!(513);
                value
            },
        ] {
            assert_eq!(compare(&baseline, &candidate).unwrap()["passed"], false);
        }
    }
    #[test]
    fn artifact_identity_full_trace_and_raw_samples_are_required() {
        let baseline = input();
        let mut changed = input();
        changed["report"]["input_artifact_sha256"] = json!("c".repeat(64));
        assert_eq!(compare(&baseline, &changed).unwrap()["passed"], false);
        for key in [
            "cycle_trace",
            "samples_us",
            "input_artifact_sha256",
            "configuration",
        ] {
            let mut missing = input();
            missing["report"][key] = Value::Null;
            assert!(compare(&baseline, &missing).is_err());
        }
        let mut changed_time = input();
        changed_time["report"]["cycle_trace"][17]["logical_time_nanos"] = json!(123);
        assert_eq!(compare(&baseline, &changed_time).unwrap()["passed"], false);
        let mut changed_misses = input();
        changed_misses["report"]["cycle_trace"][17]["task_overruns"] =
            json!([{"name":"T","missed":1}]);
        assert!(compare(&baseline, &changed_misses).is_err());
        for key in ["last_fault", "process_images"] {
            let mut truncated = input();
            truncated["report"]["cycle_trace"][7]
                .as_object_mut()
                .unwrap()
                .remove(key);
            assert!(compare(&baseline, &truncated).is_err());
        }
        let mut false_zero = input();
        false_zero["report"]["samples_ns"][0] = json!(20000000);
        false_zero["report"]["samples_us"][0] = json!(20000.0);
        assert!(compare(&baseline, &false_zero).is_err());
        let mut forged = input();
        forged["report"]["cycle_latency"]["p99_us"] = json!(1.0);
        assert!(compare(&baseline, &forged).is_err());
        let mut faulted = input();
        faulted["report"]["cycle_trace"][9]["faulted"] = json!(true);
        assert!(compare(&baseline, &faulted).is_err());
    }
    #[test]
    fn missing_and_invalid_latency_cannot_be_accepted() {
        let baseline = input();
        for invalid in [Value::Null, json!(0), json!(-1)] {
            let mut candidate = input();
            candidate["report"]["cycle_latency"]["p99_us"] = invalid;
            assert!(compare(&baseline, &candidate).is_err());
        }
        let mut candidate = input();
        candidate["report"]["vm_profile"] = Value::Null;
        assert!(compare(&baseline, &candidate).is_err());
        let mut candidate = input();
        candidate["report"]["vm_profile"]["tier1_specialized_executor"] = Value::Null;
        assert!(compare(&baseline, &candidate).is_err());
        let mut candidate = input();
        candidate["report"]["watched_globals"]["g_motion_bench_cycles"] = Value::Null;
        assert!(compare(&baseline, &candidate).is_err());
    }
}
