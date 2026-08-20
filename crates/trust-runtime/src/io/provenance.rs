/// Annotate configured I/O bindings with user-facing source provenance.
///
/// The runtime applies I/O drivers against the whole process image, so an
/// exact per-row driver can only be stated when a project has one enabled
/// driver. Multi-driver projects stay conservative instead of inventing a
/// source that may be wrong.
pub fn annotate_io_binding_sources(
    io: &mut IoInterface,
    drivers: &[crate::config::IoDriverConfig],
) {
    let enabled = drivers
        .iter()
        .filter(|driver| driver.enabled)
        .collect::<Vec<_>>();
    io.set_binding_sources(|address| {
        if matches!(address.area, IoArea::Memory) {
            return Some(SmolStr::new("Internal memory"));
        }
        match enabled.as_slice() {
            [] => None,
            [driver] => io_source_label_for_driver_address(driver, address),
            _ => Some(SmolStr::new("Multiple I/O drivers")),
        }
    });
}

#[must_use]
pub fn io_source_label_for_driver_address(
    driver: &crate::config::IoDriverConfig,
    address: &IoAddress,
) -> Option<SmolStr> {
    if matches!(address.area, IoArea::Memory) {
        return Some(SmolStr::new("Internal memory"));
    }
    let name = driver.name.trim().to_ascii_lowercase();
    match name.as_str() {
        "simulated" | "sim" | "noop" => Some(SmolStr::new("Simulated I/O")),
        "loopback" => Some(SmolStr::new("Loopback I/O")),
        "modbus-tcp" | "modbus_tcp" => {
            let endpoint = driver_param_str(&driver.params, "address").unwrap_or("configured endpoint");
            let register = match address.area {
                IoArea::Input => driver_param_u16(&driver.params, "input_start", 0)
                    .saturating_add((address.byte / 2).min(u16::MAX.into()) as u16),
                IoArea::Output => driver_param_u16(&driver.params, "output_start", 0)
                    .saturating_add((address.byte / 2).min(u16::MAX.into()) as u16),
                IoArea::Memory => return Some(SmolStr::new("Internal memory")),
            };
            let direction = match address.area {
                IoArea::Input => "input reg",
                IoArea::Output => "output reg",
                IoArea::Memory => "memory",
            };
            Some(SmolStr::new(format!(
                "Modbus {endpoint} · {direction} {register}"
            )))
        }
        "mqtt" | "mqtt-tcp" => {
            let topic = match address.area {
                IoArea::Input => driver_param_str(&driver.params, "topic_in").unwrap_or("trust/io/in"),
                IoArea::Output => {
                    driver_param_str(&driver.params, "topic_out").unwrap_or("trust/io/out")
                }
                IoArea::Memory => return Some(SmolStr::new("Internal memory")),
            };
            Some(SmolStr::new(format!("MQTT topic {topic}")))
        }
        "ethercat" | "ether-cat" | "ecat" => Some(SmolStr::new("EtherCAT process image")),
        "gpio" => Some(SmolStr::new(format!(
            "GPIO line {}",
            address.byte.saturating_mul(8) + u32::from(address.bit)
        ))),
        "ads" | "ads-client" | "beckhoff-ads" => Some(SmolStr::new("ADS mapped symbol")),
        "opcua" | "opcua-client" => Some(SmolStr::new("OPC UA mapped node")),
        other => Some(SmolStr::new(format!("{} I/O driver", display_driver_name(other)))),
    }
}

fn driver_param_str<'a>(params: &'a toml::Value, key: &str) -> Option<&'a str> {
    params
        .as_table()
        .and_then(|table| table.get(key))
        .and_then(toml::Value::as_str)
}

fn driver_param_u16(params: &toml::Value, key: &str, default: u16) -> u16 {
    params
        .as_table()
        .and_then(|table| table.get(key))
        .and_then(toml::Value::as_integer)
        .and_then(|value| u16::try_from(value).ok())
        .unwrap_or(default)
}

fn display_driver_name(name: &str) -> String {
    name.split(['-', '_'])
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(first) => format!("{}{}", first.to_ascii_uppercase(), chars.as_str()),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}
