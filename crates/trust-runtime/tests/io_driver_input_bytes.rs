//! Specification 42, §5 item 4: each `io.toml` driver declares the bytes of the input image it
//! fills, from its own configuration, so a page value located there turns stale while its link
//! exchanges no data. A driver that fills the image from its start declares every byte.
//! `trust-runtime validate`, which constructs no driver, reads the same bytes from the parameters
//! alone (specification 11 §6.5).

use std::fs;
use std::ops::Range;
use std::path::PathBuf;

use trust_runtime::io::{
    EthercatIoDriver, GpioDriver, IoDriver, IoDriverRegistry, ModbusTcpDriver, MqttIoDriver,
};

/// Every byte of the input image.
const ALL: Range<usize> = 0..usize::MAX;

fn params(text: &str) -> toml::Value {
    toml::from_str(text).expect("the parameters parse")
}

/// The bytes that `validate` reads from `params` for the driver `driver`, while the output image
/// the drivers get is `outputs` bytes long.
fn declared(driver: &str, params: &toml::Value, outputs: usize) -> Vec<Range<usize>> {
    IoDriverRegistry::default_registry()
        .declared_inputs(driver, params, outputs)
        .expect("the parameters' bytes")
}

#[test]
fn modbus_declares_its_points_or_the_whole_image_for_a_block() {
    let block_params =
        params("address = \"127.0.0.1:1502\"\nunit_id = 1\ninput_start = 0\noutput_start = 0\n");
    let block = ModbusTcpDriver::from_params(&block_params).expect("a block driver");
    assert_eq!(block.input_bytes(), vec![ALL]);
    assert_eq!(declared("modbus-tcp", &block_params, 0), vec![ALL]);
    let points_params = params(
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
    );
    let points = ModbusTcpDriver::from_params(&points_params).expect("a point driver");
    assert_eq!(points.input_bytes(), vec![0..1, 4..8]);
    assert_eq!(declared("modbus-tcp", &points_params, 0), vec![0..1, 4..8]);
}

#[test]
fn mqtt_declares_its_input_points_or_the_whole_image_for_a_raw_topic() {
    let raw_params = params("broker = \"127.0.0.1:1\"\ntopic_in = \"plant/in\"\n");
    let raw = MqttIoDriver::from_params(&raw_params).expect("a raw driver");
    assert_eq!(raw.input_bytes(), vec![ALL]);
    assert_eq!(declared("mqtt", &raw_params, 0), vec![ALL]);
    let points_params = params(
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
    );
    let points = MqttIoDriver::from_params(&points_params).expect("a point driver");
    assert_eq!(points.input_bytes(), vec![2..4, 6..7]);
    assert_eq!(declared("mqtt", &points_params, 0), vec![2..4, 6..7]);
}

#[test]
fn ethercat_declares_the_input_image_of_its_modules() {
    let ethercat_params = params(
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
    );
    let driver = EthercatIoDriver::from_params(&ethercat_params).expect("an EtherCAT driver");
    assert_eq!(driver.input_bytes(), vec![0..1]);
    assert_eq!(declared("ethercat", &ethercat_params, 0), vec![0..1]);
}

#[test]
fn gpio_declares_the_bytes_of_its_input_lines() {
    let base = trust_test_support::unique_temp_path("trust-gpio-bytes");
    for line in [4, 5, 9, 17] {
        let dir: PathBuf = base.join(format!("gpio{line}"));
        fs::create_dir_all(&dir).expect("gpio dir");
        fs::write(dir.join("direction"), "in").expect("direction");
        fs::write(dir.join("value"), "0").expect("value");
    }
    let gpio_params = params(&format!(
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
    ));
    let driver = GpioDriver::from_params(&gpio_params).expect("a GPIO driver");
    assert_eq!(driver.input_bytes(), vec![0..1, 2..3]);
    assert_eq!(declared("gpio", &gpio_params, 0), vec![0..1, 2..3]);
    let _ = fs::remove_dir_all(&base);
}

#[test]
fn the_shared_drivers_and_the_loopback_declare_their_bytes_from_their_parameters() {
    // Specification 42, §5 item 4, read from the parameters alone as `validate` reads them on any
    // computer, also where the analog, counter and CAN drivers cannot run: analog inputs at %IW2
    // and %IW6 fill their words, 2..4 and 6..8; a count at %ID4 its double word, 4..8; CAN
    // receives of 4 bytes at %IB0 and 2 at %IB8 fill 0..4 and 8..10; the loopback fills as far as
    // a 3-byte output image reaches, 0..3, and nothing without outputs; the simulated driver
    // fills nothing. Catches a driver that `validate` reads as filling nothing, a word or a double
    // word counted short, and a loopback read as filling the whole image.
    let analog = params(
        "[[inputs]]\naddress = \"%IW2\"\nline = \"GPIO2\"\n\
         [[inputs]]\naddress = \"%IW6\"\nline = \"iio:device0/in_voltage1\"\n",
    );
    assert_eq!(declared("analog", &analog, 0), vec![2..4, 6..8]);
    let counter = params("[[inputs]]\naddress = \"%ID4\"\nline = \"GPIO5\"\n");
    assert_eq!(declared("counter", &counter, 0), vec![4..8]);
    let can = params(
        "bitrate = 500000\nline = \"can0\"\n\
         [[receive]]\naddress = \"%IB0\"\nid = 385\nlength = 4\n\
         [[receive]]\naddress = \"%IB8\"\nid = 386\nlength = 2\n",
    );
    assert_eq!(declared("can", &can, 0), vec![0..4, 8..10]);
    let none = params("");
    assert_eq!(declared("loopback", &none, 3), vec![0..3]);
    assert_eq!(declared("loopback", &none, 0), Vec::<Range<usize>>::new());
    assert_eq!(declared("simulated", &none, 3), Vec::<Range<usize>>::new());
}
