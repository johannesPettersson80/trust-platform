#[cfg(all(feature = "ethercat-wire", unix))]
use std::thread;
#[cfg(all(feature = "ethercat-wire", unix))]
use std::time::Duration;

use trust_runtime::io::{EthercatIoDriver, IoDriver, IoDriverHealth};

#[test]
fn ethercat_mock_profile_maps_ek1100_elx008_process_image() {
    let params: toml::Value = toml::from_str(
        r#"
adapter = "mock"
timeout_ms = 250
cycle_warn_ms = 250
mock_inputs = ["01"]
[[modules]]
model = "EK1100"
slot = 0
[[modules]]
model = "EL1008"
slot = 1
channels = 8
[[modules]]
model = "EL2008"
slot = 2
channels = 8
"#,
    )
    .expect("parse params");
    let mut driver = EthercatIoDriver::from_params(&params).expect("driver");

    let mut inputs = [0u8; 1];
    driver.read_inputs(&mut inputs).expect("read inputs");
    assert_eq!(inputs, [0x01], "input image should map from mock frame");

    driver.write_outputs(&[0xAA]).expect("write outputs");
    let health = driver.health();
    assert!(
        matches!(health, IoDriverHealth::Ok),
        "healthy cycle should keep status ok, found {health:?}"
    );
}

#[test]
fn ethercat_cycle_warn_threshold_reports_degraded_health() {
    let params: toml::Value = toml::from_str(
        r#"
adapter = "mock"
cycle_warn_ms = 1
mock_latency_ms = 5
[[modules]]
model = "EK1100"
slot = 0
[[modules]]
model = "EL1008"
slot = 1
[[modules]]
model = "EL2008"
slot = 2
"#,
    )
    .expect("parse params");
    let mut driver = EthercatIoDriver::from_params(&params).expect("driver");

    let mut inputs = [0u8; 1];
    driver.read_inputs(&mut inputs).expect("read inputs");

    assert!(
        matches!(driver.health(), IoDriverHealth::Degraded { .. }),
        "latency above cycle_warn_ms should surface degraded health"
    );
}

#[test]
fn ethercat_warn_policy_degrades_without_runtime_cycle_error() {
    let params: toml::Value = toml::from_str(
        r#"
adapter = "mock"
on_error = "warn"
mock_fail_write = true
[[modules]]
model = "EK1100"
slot = 0
[[modules]]
model = "EL1008"
slot = 1
[[modules]]
model = "EL2008"
slot = 2
"#,
    )
    .expect("parse params");
    let mut driver = EthercatIoDriver::from_params(&params).expect("driver");

    driver
        .write_outputs(&[0x01])
        .expect("warn policy should degrade health without faulting the scan");
    assert!(
        matches!(driver.health(), IoDriverHealth::Degraded { .. }),
        "write failure under warn policy should degrade health"
    );
}

#[test]
fn ethercat_ignore_policy_degrades_without_runtime_cycle_error() {
    let params: toml::Value = toml::from_str(
        r#"
adapter = "mock"
on_error = "ignore"
mock_fail_write = true
[[modules]]
model = "EK1100"
slot = 0
[[modules]]
model = "EL1008"
slot = 1
[[modules]]
model = "EL2008"
slot = 2
"#,
    )
    .expect("parse params");
    let mut driver = EthercatIoDriver::from_params(&params).expect("driver");

    driver
        .write_outputs(&[0x01])
        .expect("ignore policy should degrade health without faulting the scan");
    assert!(
        matches!(driver.health(), IoDriverHealth::Degraded { .. }),
        "write failure should keep degraded health"
    );
}

#[test]
fn ethercat_image_size_mismatch_faults_under_warn_policy() {
    let params: toml::Value = toml::from_str(
        r#"
adapter = "mock"
on_error = "warn"
[[modules]]
model = "EK1100"
slot = 0
[[modules]]
model = "EL1008"
slot = 1
[[modules]]
model = "EL2008"
slot = 2
"#,
    )
    .expect("parse params");
    let mut driver = EthercatIoDriver::from_params(&params).expect("driver");

    let err = driver
        .write_outputs(&[])
        .expect_err("image-size mismatch must fault under warn policy");
    assert!(
        err.to_string().contains("image too small"),
        "expected image-size error, got {err}"
    );
    assert!(
        matches!(driver.health(), IoDriverHealth::Faulted { .. }),
        "image-size mismatch should fault health"
    );
}

#[cfg(all(feature = "ethercat-wire", unix))]
#[test]
#[ignore = "runtime-safety EtherCAT PduStorage baseline; explicitly run for storage evidence"]
fn ethercat_missing_adapter_records_pdu_storage_retry_baseline() {
    let params: toml::Value = toml::from_str(
        r#"
adapter = "trust-missing-ethercat-baseline0"
timeout_ms = 10
on_error = "warn"
[[modules]]
model = "EK1100"
slot = 0
[[modules]]
model = "EL1008"
slot = 1
[[modules]]
model = "EL2008"
slot = 2
"#,
    )
    .expect("parse missing-adapter params");
    let mut inputs = [0u8; 1];
    let initial_rss = current_rss_kib();
    let mut max_rss = initial_rss;

    println!(
        "ETHPDU_BASELINE start adapter=trust-missing-ethercat-baseline0 initial_rss_kib={}",
        format_optional_kib(initial_rss)
    );

    for attempt in 0..6 {
        let before = current_rss_kib();
        let mut driver =
            EthercatIoDriver::from_params(&params).expect("missing hardware must defer startup");
        let err = driver
            .read_inputs(&mut inputs)
            .expect_err("missing adapter should report unavailable hardware");
        let after = current_rss_kib();
        max_rss = max_optional(max_rss, after);
        println!(
            "ETHPDU_BASELINE fresh_construct attempt={attempt} before_rss_kib={} after_rss_kib={} error={:?}",
            format_optional_kib(before),
            format_optional_kib(after),
            err.to_string()
        );
    }

    let mut driver =
        EthercatIoDriver::from_params(&params).expect("missing hardware must defer startup");
    for (attempt, delay_ms) in [300_u64, 600, 1_100].into_iter().enumerate() {
        thread::sleep(Duration::from_millis(delay_ms));
        let before = current_rss_kib();
        let err = driver
            .read_inputs(&mut inputs)
            .expect_err("deferred missing adapter retry should fail");
        let after = current_rss_kib();
        max_rss = max_optional(max_rss, after);
        println!(
            "ETHPDU_BASELINE deferred_retry attempt={attempt} slept_ms={delay_ms} before_rss_kib={} after_rss_kib={} error={:?}",
            format_optional_kib(before),
            format_optional_kib(after),
            err.to_string()
        );
    }

    let final_rss = current_rss_kib();
    max_rss = max_optional(max_rss, final_rss);
    println!(
        "ETHPDU_BASELINE summary initial_rss_kib={} final_rss_kib={} max_rss_kib={} fresh_construct_attempts=6 deferred_retry_attempts=3",
        format_optional_kib(initial_rss),
        format_optional_kib(final_rss),
        format_optional_kib(max_rss)
    );
}

#[cfg(all(feature = "ethercat-wire", unix))]
#[test]
#[ignore = "red test for runtime-safety EtherCAT bounded post-allocation retry policy"]
fn ethercat_missing_adapter_post_allocation_failure_is_terminal_until_rebuild() {
    let params: toml::Value = toml::from_str(
        r#"
adapter = "trust-missing-ethercat-baseline0"
timeout_ms = 10
on_error = "warn"
[[modules]]
model = "EK1100"
slot = 0
[[modules]]
model = "EL1008"
slot = 1
[[modules]]
model = "EL2008"
slot = 2
"#,
    )
    .expect("parse missing-adapter params");
    let mut driver =
        EthercatIoDriver::from_params(&params).expect("missing hardware must defer startup");
    let mut inputs = [0u8; 1];

    let first_error = driver
        .read_inputs(&mut inputs)
        .expect_err("post-allocation construction failure should be terminal")
        .to_string();
    assert!(
        first_error.contains("ethercat hardware unavailable until driver rebuild"),
        "expected terminal hardware-unavailable message, got {first_error}"
    );

    thread::sleep(Duration::from_millis(300));
    let retry_error = driver
        .read_inputs(&mut inputs)
        .expect_err("terminal post-allocation failure should not retry construction")
        .to_string();
    assert!(
        retry_error.contains("ethercat hardware unavailable until driver rebuild"),
        "expected terminal hardware-unavailable message after retry window, got {retry_error}"
    );
    assert!(
        !retry_error.contains("retry in"),
        "terminal hardware-unavailable state should not schedule another retry window; got {retry_error}"
    );
}

#[cfg(all(feature = "ethercat-wire", unix, target_os = "linux"))]
fn current_rss_kib() -> Option<u64> {
    let status = std::fs::read_to_string("/proc/self/status").ok()?;
    status.lines().find_map(|line| {
        let value = line.strip_prefix("VmRSS:")?.trim();
        value.split_whitespace().next()?.parse::<u64>().ok()
    })
}

#[cfg(all(feature = "ethercat-wire", unix, not(target_os = "linux")))]
fn current_rss_kib() -> Option<u64> {
    None
}

#[cfg(all(feature = "ethercat-wire", unix))]
fn max_optional(left: Option<u64>, right: Option<u64>) -> Option<u64> {
    match (left, right) {
        (Some(left), Some(right)) => Some(left.max(right)),
        (Some(value), None) | (None, Some(value)) => Some(value),
        (None, None) => None,
    }
}

#[cfg(all(feature = "ethercat-wire", unix))]
fn format_optional_kib(value: Option<u64>) -> String {
    value
        .map(|value| value.to_string())
        .unwrap_or_else(|| "unavailable".to_string())
}
