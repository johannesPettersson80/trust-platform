//! The names of the links the runtime runs, as Devices & Connections names them
//! (specification 42, §2).

/// The protocol of an `io.toml` driver, by its name or one of its aliases, which take their
/// protocol's name (specification 42, §2): every driver of the runtime's registry runs a link.
#[must_use]
pub fn link_protocol(driver: &str) -> Option<&'static str> {
    match driver.trim().to_ascii_lowercase().as_str() {
        "modbus-tcp" | "modbus_tcp" => Some("modbus_tcp"),
        "modbus-rtu" | "modbus_rtu" => Some("modbus_rtu"),
        "mqtt" | "mqtt-tcp" => Some("mqtt"),
        "ethercat" | "ether-cat" | "ecat" => Some("ethercat"),
        "gpio" => Some("gpio"),
        "simulated" | "sim" | "noop" => Some("simulated"),
        "loopback" => Some("loopback"),
        "analog" => Some("analog"),
        "can" => Some("can"),
        "counter" => Some("counter"),
        "pwm" => Some("pwm"),
        _ => None,
    }
}

/// The link name of the `io.toml` driver at `position`, for example `modbus_tcp:0`.
#[must_use]
pub fn io_link_name(driver: &str, position: usize) -> Option<String> {
    link_protocol(driver).map(|protocol| format!("{protocol}:{position}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::io::IoDriverRegistry;

    /// Specification 42, §2: a link is named `<protocol>:<position>` for every driver of
    /// `io.toml` that specification 11 §6.5 names, the drivers every runtime shares (`analog`,
    /// `can`, `counter`, `pwm`) included, and an alias takes its protocol's name. Catches a driver
    /// of the registry that runs no link, an alias that names a link of its own, and a position
    /// counted from 1.
    #[test]
    fn every_driver_of_the_registry_and_each_alias_names_its_protocols_link() {
        for driver in IoDriverRegistry::default_registry().canonical_driver_names() {
            assert!(
                link_protocol(&driver).is_some(),
                "the driver {driver} runs a link"
            );
        }
        for (driver, link) in [
            ("modbus-tcp", "modbus_tcp:0"),
            ("modbus_tcp", "modbus_tcp:0"),
            ("modbus-rtu", "modbus_rtu:0"),
            ("modbus_rtu", "modbus_rtu:0"),
            ("MQTT", "mqtt:0"),
            ("mqtt-tcp", "mqtt:0"),
            ("ether-cat", "ethercat:0"),
            ("ecat", "ethercat:0"),
            ("sim", "simulated:0"),
            ("noop", "simulated:0"),
            ("loopback", "loopback:0"),
            ("gpio", "gpio:0"),
            ("analog", "analog:0"),
            ("can", "can:0"),
            ("counter", "counter:0"),
            ("pwm", "pwm:0"),
        ] {
            assert_eq!(io_link_name(driver, 0).as_deref(), Some(link), "{driver}");
        }
        assert_eq!(io_link_name("mqtt", 3).as_deref(), Some("mqtt:3"));
        assert_eq!(link_protocol("none"), None, "`none` attaches no driver");
    }
}
