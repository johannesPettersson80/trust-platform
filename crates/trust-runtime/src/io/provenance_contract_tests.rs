use super::*;

fn provenance_contract_driver(
    name: &str,
    enabled: bool,
    params: toml::Value,
) -> crate::config::IoDriverConfig {
    crate::config::IoDriverConfig {
        name: SmolStr::new(name),
        params,
        enabled,
    }
}

fn provenance_contract_empty_params() -> toml::Value {
    toml::Value::Table(toml::map::Map::new())
}

fn provenance_contract_address(text: &str) -> IoAddress {
    IoAddress::parse(text).unwrap_or_else(|error| panic!("parse {text}: {error}"))
}

#[test]
fn provenance_contract_canonical_and_alias_driver_names_share_labels() {
    let input = provenance_contract_address("%IX0.0");
    for (name, expected) in [
        ("simulated", "Simulated I/O"),
        ("sim", "Simulated I/O"),
        ("noop", "Simulated I/O"),
        ("loopback", "Loopback I/O"),
        ("mqtt", "MQTT topic trust/io/in"),
        ("mqtt-tcp", "MQTT topic trust/io/in"),
        ("ethercat", "EtherCAT process image"),
        ("ether-cat", "EtherCAT process image"),
        ("ecat", "EtherCAT process image"),
    ] {
        let driver = provenance_contract_driver(name, true, provenance_contract_empty_params());
        assert_eq!(
            io_source_label_for_driver_address(&driver, &input).as_deref(),
            Some(expected),
            "{name}"
        );
    }
}

#[test]
fn provenance_contract_memory_is_internal_for_every_driver_family() {
    let memory = provenance_contract_address("%MX2.3");
    for name in [
        "simulated",
        "modbus-tcp",
        "mqtt",
        "gpio",
        "ethercat",
        "unknown-driver",
    ] {
        let driver = provenance_contract_driver(name, true, provenance_contract_empty_params());
        assert_eq!(
            io_source_label_for_driver_address(&driver, &memory).as_deref(),
            Some("Internal memory"),
            "{name}"
        );
    }
}

#[test]
fn provenance_contract_modbus_uses_directional_base_and_word_offset() {
    let params: toml::Value = toml::toml! {
        address = "10.0.0.5:1502"
        input_start = 100
        output_start = 200
    }
    .into();
    for name in ["modbus-tcp", " MODBUS_TCP "] {
        let driver = provenance_contract_driver(name, true, params.clone());
        assert_eq!(
            io_source_label_for_driver_address(&driver, &provenance_contract_address("%IW6"))
                .as_deref(),
            Some("Modbus 10.0.0.5:1502 · input reg 103")
        );
        assert_eq!(
            io_source_label_for_driver_address(&driver, &provenance_contract_address("%QW8"))
                .as_deref(),
            Some("Modbus 10.0.0.5:1502 · output reg 204")
        );
    }
}

#[test]
fn provenance_contract_modbus_and_mqtt_defaults_are_directional() {
    let modbus = provenance_contract_driver("modbus-tcp", true, provenance_contract_empty_params());
    assert_eq!(
        io_source_label_for_driver_address(&modbus, &provenance_contract_address("%IX2.0"))
            .as_deref(),
        Some("Modbus configured endpoint · input reg 1")
    );
    assert_eq!(
        io_source_label_for_driver_address(&modbus, &provenance_contract_address("%QX4.0"))
            .as_deref(),
        Some("Modbus configured endpoint · output reg 2")
    );

    let mqtt = provenance_contract_driver("mqtt", true, provenance_contract_empty_params());
    assert_eq!(
        io_source_label_for_driver_address(&mqtt, &provenance_contract_address("%IX0.0"))
            .as_deref(),
        Some("MQTT topic trust/io/in")
    );
    assert_eq!(
        io_source_label_for_driver_address(&mqtt, &provenance_contract_address("%QX0.0"))
            .as_deref(),
        Some("MQTT topic trust/io/out")
    );
}

#[test]
fn provenance_contract_gpio_projects_absolute_process_image_bit() {
    let driver = provenance_contract_driver("gpio", true, provenance_contract_empty_params());
    assert_eq!(
        io_source_label_for_driver_address(&driver, &provenance_contract_address("%QX3.5"))
            .as_deref(),
        Some("GPIO line 29")
    );
}

#[test]
fn provenance_contract_zero_enabled_drivers_make_no_external_claim() {
    let mut io = IoInterface::new();
    io.bind("input", provenance_contract_address("%IX0.0"));
    annotate_io_binding_sources(&mut io, &[]);
    assert_eq!(io.bindings()[0].source, None);
}

#[test]
fn provenance_contract_disabled_drivers_do_not_affect_single_source() {
    let mut io = IoInterface::new();
    io.bind("input", provenance_contract_address("%IX0.0"));
    let drivers = [
        provenance_contract_driver("gpio", false, provenance_contract_empty_params()),
        provenance_contract_driver("simulated", true, provenance_contract_empty_params()),
        provenance_contract_driver("mqtt", false, provenance_contract_empty_params()),
    ];

    annotate_io_binding_sources(&mut io, &drivers);

    assert_eq!(io.bindings()[0].source.as_deref(), Some("Simulated I/O"));
}

#[test]
fn provenance_contract_multiple_enabled_drivers_stay_conservative() {
    let mut io = IoInterface::new();
    io.bind("input", provenance_contract_address("%IX0.0"));
    let drivers = [
        provenance_contract_driver("simulated", true, provenance_contract_empty_params()),
        provenance_contract_driver("mqtt", true, provenance_contract_empty_params()),
    ];

    annotate_io_binding_sources(&mut io, &drivers);

    assert_eq!(
        io.bindings()[0].source.as_deref(),
        Some("Multiple I/O drivers")
    );
}

#[test]
fn provenance_contract_memory_remains_internal_with_zero_or_many_drivers() {
    for drivers in [
        Vec::new(),
        vec![
            provenance_contract_driver("simulated", true, provenance_contract_empty_params()),
            provenance_contract_driver("mqtt", true, provenance_contract_empty_params()),
        ],
    ] {
        let mut io = IoInterface::new();
        io.bind("memory", provenance_contract_address("%MX0.0"));
        annotate_io_binding_sources(&mut io, &drivers);
        assert_eq!(io.bindings()[0].source.as_deref(), Some("Internal memory"));
    }
}

#[test]
fn provenance_contract_unknown_driver_label_is_readable_but_not_health_claim() {
    let driver =
        provenance_contract_driver("custom-field_bus", true, provenance_contract_empty_params());
    assert_eq!(
        io_source_label_for_driver_address(&driver, &provenance_contract_address("%IX0.0"))
            .as_deref(),
        Some("Custom Field Bus I/O driver")
    );
}
