//! The GPIO driver drives the safe outputs when the watchdog faults the resource (specification 34
//! STATE-02).
//!
//! The tests need the toolchain of specification 44 §4.6 at TRUST_TOOLCHAIN.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use trust_runtime::harness::TestHarness;
use trust_runtime::io::{GpioDriver, IoAddress, IoSafeState};
use trust_runtime::value::Value;

fn temp_sysfs_base() -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("trust-gpio-safe-{nanos}"))
}

fn create_gpio_files(base: &Path, line: u32) -> std::io::Result<()> {
    let gpio_dir = base.join(format!("gpio{line}"));
    fs::create_dir_all(&gpio_dir)?;
    fs::write(gpio_dir.join("direction"), "out")?;
    fs::write(gpio_dir.join("value"), "0")?;
    Ok(())
}

#[test]
fn gpio_safe_state_writes_outputs_on_fault() {
    let base = temp_sysfs_base();
    create_gpio_files(&base, 17).expect("create gpio files");

    let mut params = toml::map::Map::new();
    params.insert("backend".into(), toml::Value::String("sysfs".to_string()));
    params.insert(
        "sysfs_base".into(),
        toml::Value::String(base.display().to_string()),
    );
    let outputs = toml::Value::Array(vec![toml::Value::Table(toml::map::Map::from_iter([
        ("address".into(), toml::Value::String("%QX0.0".to_string())),
        ("line".into(), toml::Value::Integer(17)),
        ("initial".into(), toml::Value::Boolean(true)),
    ]))]);
    params.insert("outputs".into(), outputs);
    let params = toml::Value::Table(params);

    let driver = GpioDriver::from_params(&params).expect("gpio driver");
    let value_path = base.join("gpio17").join("value");
    let initial = fs::read_to_string(&value_path).expect("read value");
    assert_eq!(initial.trim(), "1");

    // A program whose output image is one byte.
    let mut runtime = TestHarness::from_source(
        "VAR_GLOBAL\n    lamp AT %QX0.0 : BOOL;\nEND_VAR\n\nPROGRAM Main\nEND_PROGRAM\n",
    )
    .expect("the one-output program compiles")
    .into_runtime();
    runtime.add_io_driver("gpio", Box::new(driver));

    let address = IoAddress::parse("%QX0.0").expect("address");
    runtime
        .write_io(&address, Value::Bool(true))
        .expect("write output");

    let mut safe_state = IoSafeState::default();
    safe_state.outputs.push((address, Value::Bool(false)));
    runtime.set_io_safe_state(safe_state);

    let _ = runtime.watchdog_timeout();

    let after = fs::read_to_string(&value_path).expect("read value");
    assert_eq!(after.trim(), "0");

    let _ = fs::remove_dir_all(&base);
}
