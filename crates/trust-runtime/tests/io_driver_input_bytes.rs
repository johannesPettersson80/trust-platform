//! Specification 42, §5 item 4: each `io.toml` driver declares the bytes of the input image it
//! fills, from its own configuration, so a page value located there turns stale while its link
//! exchanges no data. A driver that fills the image from its start declares every byte.

use std::fs;
use std::ops::Range;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use trust_runtime::io::{EthercatIoDriver, GpioDriver, IoDriver, ModbusTcpDriver, MqttIoDriver};

/// Every byte of the input image.
const ALL: Range<usize> = 0..usize::MAX;

fn params(text: &str) -> toml::Value {
    toml::from_str(text).expect("the parameters parse")
}

#[test]
fn modbus_declares_its_points_or_the_whole_image_for_a_block() {
    let block = ModbusTcpDriver::from_params(&params(
        "address = \"127.0.0.1:1502\"\nunit_id = 1\ninput_start = 0\noutput_start = 0\n",
    ))
    .expect("a block driver");
    assert_eq!(block.input_bytes(), vec![ALL]);
    let points = ModbusTcpDriver::from_params(&params(
        r#"
address = "127.0.0.1:1502"
unit_id = 1
[[input_points]]
image_offset = 0
image_bit = 3
address = 2
function = "read_coils"
data_type = "bool"
[[input_points]]
image_offset = 4
address = 0
function = "read_holding_registers"
data_type = "u32"
"#,
    ))
    .expect("a point driver");
    assert_eq!(points.input_bytes(), vec![0..1, 4..8]);
}

#[test]
fn mqtt_declares_its_input_points_or_the_whole_image_for_a_raw_topic() {
    let raw = MqttIoDriver::from_params(&params(
        "broker = \"127.0.0.1:1\"\ntopic_in = \"plant/in\"\n",
    ))
    .expect("a raw driver");
    assert_eq!(raw.input_bytes(), vec![ALL]);
    let points = MqttIoDriver::from_params(&params(
        r#"
broker = "127.0.0.1:1"
[[input_points]]
topic = "line/level"
image_offset = 2
data_type = "u16"
[[input_points]]
topic = "line/run"
image_offset = 6
image_bit = 1
data_type = "bool"
"#,
    ))
    .expect("a point driver");
    assert_eq!(points.input_bytes(), vec![2..4, 6..7]);
}

#[test]
fn ethercat_declares_the_input_image_of_its_modules() {
    let driver = EthercatIoDriver::from_params(&params(
        r#"
adapter = "mock"
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
    ))
    .expect("an EtherCAT driver");
    assert_eq!(driver.input_bytes(), vec![0..1]);
}

#[test]
fn gpio_declares_the_bytes_of_its_input_lines() {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let base =
        std::env::temp_dir().join(format!("trust-gpio-bytes-{}-{stamp}", std::process::id()));
    for line in [4, 5, 9, 17] {
        let dir: PathBuf = base.join(format!("gpio{line}"));
        fs::create_dir_all(&dir).expect("gpio dir");
        fs::write(dir.join("direction"), "in").expect("direction");
        fs::write(dir.join("value"), "0").expect("value");
    }
    let driver = GpioDriver::from_params(&params(&format!(
        r#"
backend = "sysfs"
sysfs_base = "{}"
[[inputs]]
address = "%IX0.0"
line = 4
[[inputs]]
address = "%IX0.1"
line = 5
[[inputs]]
address = "%IX2.3"
line = 9
[[outputs]]
address = "%QX0.0"
line = 17
"#,
        base.display().to_string().replace('\\', "/")
    )))
    .expect("a GPIO driver");
    assert_eq!(driver.input_bytes(), vec![0..1, 2..3]);
    let _ = fs::remove_dir_all(&base);
}
