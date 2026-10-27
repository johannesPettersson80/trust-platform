//! `[runtime.identify] output` (specification 11, "Identify"): the two forms it takes and the
//! configuration errors for everything else.

use super::parser::parse_runtime_toml_from_text;
use super::{IdentifyOutputConfig, RuntimeConfig};
use crate::io::IoAddress;

/// The smallest `runtime.toml` the parser accepts.
const BASE: &str = r#"
[bundle]
version = 1

[resource]
name = "main"
cycle_interval_ms = 100

[runtime.control]
endpoint = "unix:///tmp/trust-runtime.sock"

[runtime.log]
level = "info"

[runtime.retain]
mode = "none"
save_interval_ms = 1000

[runtime.watchdog]
enabled = false
timeout_ms = 5000
action = "halt"

[runtime.fault]
policy = "halt"
"#;

fn parse(extra: &str) -> Result<RuntimeConfig, crate::error::RuntimeError> {
    parse_runtime_toml_from_text(&format!("{BASE}\n{extra}\n"), "runtime.toml")
}

fn output_of(text: &str) -> Option<IdentifyOutputConfig> {
    parse(&format!("[runtime.identify]\noutput = \"{text}\""))
        .expect("the setting is accepted")
        .identify_output
}

#[test]
fn identify_output_is_read_as_an_io_address_or_a_gpio_line() {
    // Specification 11, "Identify", the output: `%QX<byte>.<bit>` names one bit of the output
    // image and `gpio:<line>` a GPIO line by its number; surrounding spaces and the case of
    // `gpio:` do not matter. Catches a setting that is ignored, an address that loses its byte or
    // bit, and a line number that is read as an address.
    let Some(IdentifyOutputConfig::Address(address)) = output_of("%QX3.7") else {
        panic!("an I/O address is read as an address");
    };
    assert_eq!(address, IoAddress::parse("%QX3.7").expect("the address"));
    assert_eq!((address.byte, address.bit), (3, 7));

    assert_eq!(
        output_of("gpio:17"),
        Some(IdentifyOutputConfig::GpioLine(17))
    );
    assert_eq!(
        output_of("  GPIO:4 "),
        Some(IdentifyOutputConfig::GpioLine(4))
    );
}

#[test]
fn a_project_without_the_setting_has_no_identify_output() {
    // Specification 11, "Identify": without `[runtime.identify] output` the runtime has no
    // identify output. A missing section and a section without `output` both mean none. Catches
    // a default output that nobody configured.
    assert_eq!(parse("").expect("no section").identify_output, None);
    assert_eq!(
        parse("[runtime.identify]")
            .expect("an empty section")
            .identify_output,
        None
    );
}

#[test]
fn identify_output_in_another_form_is_a_configuration_error() {
    // Specification 11, "Identify", the output: the setting is one of the two forms; another form
    // is a configuration error that names the key. A wildcard, an input, a marker, a byte, word or
    // double word, a hierarchical address, a bit past 7, a shorthand without the bit (`%QX5`) or
    // with leading zeros, a pin name (a line on a computer is a number), a negative or fractional
    // or too large line number, a bare number and an empty string are all refused. Catches an
    // output that the runtime could not drive, or that would be taken for another address.
    for text in [
        "",
        "   ",
        "%Q*",
        "%IX0.0",
        "%MX0.0",
        "%QB0",
        "%QW0",
        "%QD0",
        "%QX1.2.3",
        "%QX0.8",
        "%QX0",
        "%QX5",
        "%QX00.7",
        "QX0.0",
        "17",
        "gpio:",
        "gpio:PD12",
        "gpio:-1",
        "gpio:1.5",
        "gpio:99999999999",
        "gpio: 17",
        "usb:1",
    ] {
        let Err(error) = parse(&format!("[runtime.identify]\noutput = \"{text}\"")) else {
            panic!("'{text}' is refused");
        };
        assert!(
            error.to_string().contains("runtime.identify.output"),
            "'{text}': {error}"
        );
    }
}

#[test]
fn an_unknown_key_in_the_identify_section_is_refused() {
    // Specification 11, "Identify": the section holds `output` and nothing else. Catches a
    // misspelled key (`outputs`, `line`) that is silently ignored and leaves the runtime without
    // an identify output.
    let error = parse("[runtime.identify]\nline = 17").expect_err("an unknown key");
    assert!(error.to_string().contains("unknown field"), "{error}");
}

#[test]
fn the_output_is_named_as_runtime_toml_writes_it() {
    // Specification 11, "Identify", load: the errors name the output as `runtime.toml` writes it.
    // Catches a name in a debug format.
    assert_eq!(
        output_of("%QX0.7").map(|o| o.to_string()).as_deref(),
        Some("%QX0.7")
    );
    assert_eq!(
        output_of("GPIO:17").map(|o| o.to_string()).as_deref(),
        Some("gpio:17")
    );
}
