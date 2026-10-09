//! Specification 42, §4 item 9: each `io.toml` driver describes its protocol's own state in words,
//! which the `Comm.Link` block gives as `Detail`.

use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use trust_runtime::io::{EthercatIoDriver, GpioDriver, IoDriver};

/// EtherCAT: what discovery found on the bus.
#[test]
fn ethercat_detail_says_what_discovery_found_on_the_bus() {
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
    assert_eq!(
        driver.detail().as_deref(),
        Some("ethercat discovered [EK1100@0, EL1008@1, EL2008@2] on adapter 'mock' (I=1B O=1B)")
    );
}

/// GPIO: where it reads and writes, and how many lines it serves.
#[test]
fn gpio_detail_names_the_chip_and_the_lines_it_serves() {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let base: PathBuf = std::env::temp_dir().join(format!("trust-gpio-detail-{nanos}"));
    for line in [4, 5, 17] {
        let dir = base.join(format!("gpio{line}"));
        fs::create_dir_all(&dir).expect("gpio dir");
        fs::write(dir.join("direction"), "in").expect("direction");
        fs::write(dir.join("value"), "0").expect("value");
    }
    let params: toml::Value = toml::from_str(&format!(
        r#"
backend = "sysfs"
sysfs_base = "{}"
[[inputs]]
address = "%IX0.0"
line = 4
[[inputs]]
address = "%IX0.1"
line = 5
[[outputs]]
address = "%QX0.0"
line = 17
"#,
        base.display().to_string().replace('\\', "/")
    ))
    .expect("parse params");
    let driver = GpioDriver::from_params(&params).expect("gpio driver");
    assert_eq!(
        driver.detail().map(|text| text.to_string()),
        Some(format!(
            "sysfs {}: 2 input lines, 1 output line",
            base.display()
        ))
    );
    let _ = fs::remove_dir_all(&base);
}
