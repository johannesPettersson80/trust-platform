//! The host's identify output and the load check (specification 11, "Identify").

use smol_str::SmolStr;
use trust_runtime_services::identify::{IdentifyDuration, IdentifyState};

use super::*;
use crate::io::IoSafeState;

fn address(text: &str) -> IoAddress {
    IoAddress::parse(text).expect("a valid address")
}

/// The identify output `%QX0.7`, the bit that most of these tests ask about.
fn bit_7() -> IdentifyOutputConfig {
    IdentifyOutputConfig::Address(address("%QX0.7"))
}

fn no_io() -> IoConfig {
    IoConfig {
        drivers: Vec::new(),
        safe_state: IoSafeState::default(),
    }
}

fn io_with(drivers: Vec<(&str, bool, &str)>) -> IoConfig {
    IoConfig {
        drivers: drivers
            .into_iter()
            .map(|(name, enabled, params)| IoDriverConfig {
                name: SmolStr::new(name),
                params: toml::from_str(params).expect("driver parameters"),
                enabled,
            })
            .collect(),
        safe_state: IoSafeState::default(),
    }
}

/// The text of the refusal, which is an invalid-config error.
fn refusal(result: Result<(), RuntimeError>) -> String {
    match result {
        Err(RuntimeError::InvalidConfig(text)) => text.to_string(),
        other => panic!("expected an invalid-config refusal, got {other:?}"),
    }
}

#[test]
fn program_using_the_identify_output_is_refused_at_load() {
    // Specification 11, "Identify", load: a `%Q` binding of the program that covers the output
    // refuses the project with an error that names the output and the binding. The output is
    // `%QX0.7`; a bit binding of the same bit, a byte, a word, a double word, a long word and a
    // four-byte window at byte 0 all cover it, so each is refused with the binding's own text.
    // Catches a check that compares only whole addresses (a word binding would hide the
    // output), one that misses a window, and one that names no binding.
    for (binding, text) in [
        (address("%QX0.7"), "%QX0.7"),
        (address("%QB0"), "%QB0"),
        (address("%QW0"), "%QW0"),
        (address("%QD0"), "%QD0"),
        (address("%QL0"), "%QL0"),
        (
            IoAddress {
                area: IoArea::Output,
                size: IoSize::Bytes(4),
                byte: 0,
                bit: 0,
                path: vec![0],
                wildcard: false,
            },
            "%QB0",
        ),
    ] {
        let message = refusal(check_identify_output(&bit_7(), &[binding], 8, &no_io()));
        assert_eq!(
            message,
            format!("identify output %QX0.7 is used by the program: it binds {text}")
        );
    }
}

#[test]
fn a_binding_that_does_not_cover_the_output_does_not_refuse_the_project() {
    // Specification 11, "Identify", load: only a `%Q` binding that covers the bit uses the
    // output. Another bit of the same byte, a binding in another byte, an input and a marker at
    // the same address, a word that ends before the byte (`%QW0` covers bytes 0 and 1, the
    // output is in byte 2) and a hierarchical binding do not. Catches a check that refuses every
    // output binding, one that mixes the areas, and one that reads a word as one byte.
    let output = IdentifyOutputConfig::Address(address("%QX2.7"));
    let bindings = [
        address("%QX2.6"),
        address("%QB3"),
        address("%IX2.7"),
        address("%MX2.7"),
        address("%QW0"),
        address("%QX2.1.7"),
    ];
    assert_eq!(
        check_identify_output(&output, &bindings, 8, &no_io()),
        Ok(())
    );
}

#[test]
fn an_output_beyond_the_output_image_is_refused() {
    // Specification 11, "Identify", load: the byte of the output must lie in the output image
    // that the program's bindings make, so the drivers always get an image of that length. With
    // a 1-byte image `%QX1.0` is refused (`byte` in the singular), with a 3-byte image
    // `%QX3.0` is refused (`bytes`) and `%QX2.0` is accepted, and with an empty image every
    // output is refused. Catches an off-by-one at the end of the image and a wrong plural.
    let beyond = |text: &str, len: usize| {
        refusal(check_identify_output(
            &IdentifyOutputConfig::Address(address(text)),
            &[],
            len,
            &no_io(),
        ))
    };
    assert_eq!(
        beyond("%QX1.0", 1),
        "identify output %QX1.0 is outside the output process image of 1 byte"
    );
    assert_eq!(
        beyond("%QX3.0", 3),
        "identify output %QX3.0 is outside the output process image of 3 bytes"
    );
    assert_eq!(
        beyond("%QX0.0", 0),
        "identify output %QX0.0 is outside the output process image of 0 bytes"
    );
    assert_eq!(
        check_identify_output(
            &IdentifyOutputConfig::Address(address("%QX2.0")),
            &[],
            3,
            &no_io()
        ),
        Ok(())
    );
}

#[test]
fn an_io_toml_mapping_onto_the_identify_gpio_line_refuses_the_project() {
    // Specification 11, "Identify", load: an enabled `gpio` driver of `io.toml` that maps the line
    // as an input or an output uses it, and the project is refused with the line named. Line 17
    // is an output and line 5 an input of the driver, so both are refused; line 6 is free; a
    // disabled `gpio` driver and a driver of another name map nothing. Catches a check that
    // reads only the outputs, one that reads disabled drivers, and one that matches by position
    // instead of by line.
    let gpio = r#"
outputs = [ { address = "%QX0.0", line = 17 } ]
inputs = [ { address = "%IX0.0", line = 5 } ]
"#;
    let io = io_with(vec![
        ("gpio", true, gpio),
        ("modbus-tcp", true, "address = \"127.0.0.1:502\""),
    ]);
    for line in [17, 5] {
        assert_eq!(
            refusal(check_identify_output(
                &IdentifyOutputConfig::GpioLine(line),
                &[],
                0,
                &io
            )),
            format!(
                "identify output gpio:{line} is used by the project: io.toml maps GPIO line {line}"
            )
        );
    }
    assert_eq!(
        check_identify_output(&IdentifyOutputConfig::GpioLine(6), &[], 0, &io),
        Ok(())
    );

    let disabled = io_with(vec![("gpio", false, gpio)]);
    assert_eq!(
        check_identify_output(&IdentifyOutputConfig::GpioLine(17), &[], 0, &disabled),
        Ok(())
    );
}

#[test]
fn the_identify_image_bit_is_set_in_a_copy_of_the_image() {
    // Specification 11, "Identify", the output: the runtime sets the bit in a copy of the output
    // image that it hands to the I/O drivers; the program's own image never holds it. Byte 1, bit
    // 7 of `[0x01, 0x00]` gives `[0x01, 0x80]` in the copy, the original stays `[0x01, 0x00]`, a
    // bit already set stays set, and a bit beyond the image gives no copy at all. Catches a bit
    // set in the wrong byte or position, a write into the program's image, and an image that
    // grows.
    let bit = ImageBit::of(&address("%QX1.7")).expect("one flat bit");
    let image = [0x01_u8, 0x00];

    assert_eq!(bit.set_in(&image), Some(vec![0x01, 0x80]));
    assert_eq!(image, [0x01, 0x00]);
    assert_eq!(bit.set_in(&[0x01, 0x80]), Some(vec![0x01, 0x80]));
    assert_eq!(bit.set_in(&[0x01]), None);

    let low = ImageBit::of(&address("%QX0.0")).expect("one flat bit");
    assert_eq!(low.set_in(&[0x00]), Some(vec![0x01]));
}

#[test]
fn an_image_output_offers_its_bit_only_while_the_blink_has_it_on() {
    // Specification 11, "Identify": driving the image output on makes the next publication set
    // the bit, and driving it off takes the bit away again; a GPIO output never offers an image
    // bit. Catches a bit that stays set after the blink turned off.
    let mut output = HostIdentifyOutput::Image {
        address: address("%QX0.7"),
        bit: ImageBit::of(&address("%QX0.7")).expect("one flat bit"),
        on: false,
    };
    assert_eq!(output.image_bit_when_on(), None);
    output.drive(true).expect("an image bit is always ready");
    assert!(output.image_bit_when_on().is_some());
    output.drive(false).expect("an image bit is always ready");
    assert_eq!(output.image_bit_when_on(), None);

    let line = GpioIdentifyLine::new(None, 17).expect("the default chip");
    assert_eq!(HostIdentifyOutput::Gpio(line).image_bit_when_on(), None);
}

#[test]
fn the_host_builds_an_identify_state_that_blinks_on_the_first_tick() {
    // Specification 11, "Identify": a started identify turns an image output on at the first
    // publication, and a finished or cancelled one turns it off. Started for 60 s on the host
    // clock, `is_identifying` holds at once, the first tick offers the bit, and a cancel takes it
    // away and ends the run. Catches an identify state that is built without its output.
    let mut identify = host_identify(&bit_7(), &no_io()).expect("an image output builds");
    let duration = IdentifyDuration::new(60).expect("60 s is the longest duration");
    assert!(!identify.is_identifying());

    identify.start(duration).expect("an image bit is ready");
    assert!(identify.is_identifying());
    identify.tick().expect("an image bit is driven");
    assert_eq!(
        identify.output().image_bit_when_on(),
        ImageBit::of(&address("%QX0.7"))
    );

    identify.cancel().expect("an image bit is driven");
    assert!(!identify.is_identifying());
    assert_eq!(identify.output().image_bit_when_on(), None);
}

#[test]
fn a_gpio_line_takes_the_chip_of_the_first_enabled_gpio_driver() {
    // Specification 11, "Identify", the output: the line is a line of the chip and backend that
    // the first enabled `gpio` driver names, and parameters that driver would refuse are
    // refused. A disabled driver is skipped, so its bad parameters do not matter; an enabled
    // one with an unknown backend fails the build. Catches a line built on the default chip
    // while the project names another.
    let line = IdentifyOutputConfig::GpioLine(17);
    let skipped = io_with(vec![("gpio", false, "backend = \"spi\"")]);
    assert!(host_identify(&line, &skipped).is_ok());
    assert!(host_identify(&line, &no_io()).is_ok());

    let bad = io_with(vec![("gpio", true, "backend = \"spi\"")]);
    assert!(host_identify(&line, &bad).is_err());
}
