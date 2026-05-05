#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ethercat_config_defaults_cover_ek1100_elx008() {
        let config = EthercatConfig::from_params(&toml::Value::Table(toml::map::Map::new()))
            .expect("default config");
        assert_eq!(config.adapter.as_str(), "mock");
        assert_eq!(config.expected_input_bytes, 1);
        assert_eq!(config.expected_output_bytes, 1);
        assert!(config
            .modules
            .iter()
            .any(|module| module.model.as_str() == "EK1100"));
    }

    #[test]
    fn ethercat_config_accepts_hardware_adapter_name() {
        let params: toml::Value = toml::from_str("adapter = 'eth0'").expect("parse params");
        let config = EthercatConfig::from_params(&params).expect("hardware adapter should parse");
        assert_eq!(config.adapter.as_str(), "eth0");
    }

    #[test]
    fn ethercat_driver_mock_reads_and_writes_images() {
        let params: toml::Value = toml::from_str(
            r#"
adapter = "mock"
mock_inputs = ["01", "00"]
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
        driver.read_inputs(&mut inputs).expect("read");
        assert_eq!(inputs, [0x01]);
        driver.write_outputs(&[0xAA]).expect("write");
        assert!(matches!(driver.health(), IoDriverHealth::Ok));
    }

    #[test]
    fn ethercat_driver_fault_policy_propagates_driver_failure() {
        let params: toml::Value = toml::from_str(
            r#"
adapter = "mock"
mock_fail_read = true
on_error = "fault"
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
        let err = driver
            .read_inputs(&mut inputs)
            .expect_err("fault policy should fail cycle");
        assert!(err.to_string().contains("ethercat read"));
        assert!(matches!(driver.health(), IoDriverHealth::Faulted { .. }));
    }

    #[test]
    fn ethercat_driver_warn_policy_degrades_and_reports_error() {
        let params: toml::Value = toml::from_str(
            r#"
adapter = "mock"
mock_fail_write = true
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
        let mut inputs = [0u8; 1];
        driver.read_inputs(&mut inputs).expect("read");
        driver
            .write_outputs(&[0x01])
            .expect_err("warn policy should report write failure");
        assert!(matches!(driver.health(), IoDriverHealth::Degraded { .. }));
    }

    #[cfg(all(feature = "ethercat-wire", unix))]
    #[test]
    fn ethercat_hardware_open_failure_faults_without_blocking_startup() {
        let params: toml::Value = toml::from_str(
            r#"
adapter = "definitely-missing-adapter"
on_error = "warn"
"#,
        )
        .expect("parse params");
        let mut driver = EthercatIoDriver::from_params(&params)
            .expect("driver creation should not fail when hardware is missing");
        let mut inputs = [0u8; 1];
        driver
            .read_inputs(&mut inputs)
            .expect_err("warn policy should report missing hardware");
        assert!(matches!(driver.health(), IoDriverHealth::Faulted { .. }));
    }
}
