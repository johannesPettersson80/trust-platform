//! The names of the links the runtime runs, as Devices & Connections names them
//! (specification 42, §2).

/// The protocol of an `io.toml` driver, by its name or alias.
#[must_use]
pub fn link_protocol(driver: &str) -> Option<&'static str> {
    match driver.trim().to_ascii_lowercase().as_str() {
        "modbus-tcp" | "modbus_tcp" => Some("modbus_tcp"),
        "mqtt" => Some("mqtt"),
        "ethercat" => Some("ethercat"),
        "gpio" => Some("gpio"),
        "simulated" | "sim" | "noop" => Some("simulated"),
        "loopback" => Some("loopback"),
        _ => None,
    }
}

/// The link name of the `io.toml` driver at `position`, for example `modbus_tcp:0`.
#[must_use]
pub fn io_link_name(driver: &str, position: usize) -> Option<String> {
    link_protocol(driver).map(|protocol| format!("{protocol}:{position}"))
}
