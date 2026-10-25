//! The parameter table: the keys, defaults, aliases and refusals of the computer's driver
//! (specification 11 §6.5; specification 34 §5.1.1, section 8).

use alloc::format;
use alloc::vec;

use crate::io_drivers::modbus::{
    Endian, InputFunction, InputPoint, ModbusTcpAddress, ModbusTcpParams, OutputFunction,
    OutputPoint, PointType,
};
use crate::io_drivers::{OnError, ParamsError};

/// The refusal of `key` for `reason`.
fn refused(key: &str, reason: &str) -> Result<ModbusTcpParams, ParamsError> {
    Err(ParamsError {
        key: key.into(),
        reason: reason.into(),
    })
}

/// The parameters of the server `10.0.0.5:502` with `fields`, more JSON members.
fn server_with(fields: &str) -> Result<ModbusTcpParams, ParamsError> {
    ModbusTcpParams::from_json(&format!(r#"{{"address":"10.0.0.5:502",{fields}}}"#))
}

/// Only `address` given: unit 1, both starts 0, a timeout of 500 ms, `on_error = "fault"`, FC04 for
/// the inputs and FC16 for the outputs ("Default profile uses input registers (FC04) ... multiple
/// holding-register writes (FC16)"), and no points: the computer's defaults. Catches a default that
/// differs between a chip and a computer, such as unit 0 or holding registers for the inputs.
#[test]
fn an_address_alone_takes_the_computer_defaults() {
    assert_eq!(
        ModbusTcpParams::from_json(r#"{"address":"192.168.1.10:502"}"#),
        Ok(ModbusTcpParams {
            address: ModbusTcpAddress {
                ip: [192, 168, 1, 10],
                port: 502,
            },
            unit_id: 1,
            input_start: 0,
            output_start: 0,
            timeout_ms: 500,
            on_error: OnError::Fault,
            input_function: InputFunction::InputRegisters,
            output_function: OutputFunction::MultipleRegisters,
            input_points: vec![],
            output_points: vec![],
        })
    );
}

/// Every key read into its own field. The input point names no function, so it takes the
/// driver's `input_function`, FC03; the output point names FC15 and no type, so it is a `bool`
/// with scale 1 and offset 0; orders not given are big. Catches a key read into another field, a
/// point that ignores the driver's function, and a coil point given a register type.
#[test]
fn a_full_table_reads_into_its_values() {
    let params = ModbusTcpParams::from_json(concat!(
        r#"{"address":"10.1.2.3:1502","unit_id":17,"input_start":100,"output_start":200,"#,
        r#""timeout_ms":250,"on_error":"warn","input_function":"read_holding_registers","#,
        r#""output_function":"write_single_register","input_points":[{"image_offset":2,"#,
        r#""address":40,"data_type":"i32","scale":0.5,"offset":-3.0,"byte_order":"little","#,
        r#""word_order":"little"}],"output_points":[{"image_offset":6,"address":7,"#,
        r#""function":"write_multiple_coils","image_bit":5}]}"#
    ));
    assert_eq!(
        params,
        Ok(ModbusTcpParams {
            address: ModbusTcpAddress {
                ip: [10, 1, 2, 3],
                port: 1502,
            },
            unit_id: 17,
            input_start: 100,
            output_start: 200,
            timeout_ms: 250,
            on_error: OnError::Warn,
            input_function: InputFunction::HoldingRegisters,
            output_function: OutputFunction::SingleRegister,
            input_points: vec![InputPoint {
                image_offset: 2,
                image_bit: 0,
                address: 40,
                function: InputFunction::HoldingRegisters,
                data_type: PointType::I32,
                scale: 0.5,
                offset: -3.0,
                byte_order: Endian::Little,
                word_order: Endian::Little,
            }],
            output_points: vec![OutputPoint {
                image_offset: 6,
                image_bit: 5,
                address: 7,
                function: OutputFunction::MultipleCoils,
                data_type: PointType::Bool,
                scale: 1.0,
                offset: 0.0,
                byte_order: Endian::Big,
                word_order: Endian::Big,
            }],
        })
    );
}

/// The functions' names and the computer's aliases, trimmed, in any case and with `-` as `_`:
/// `coil` and `coils` are FC01 for the inputs but FC05 and FC15 for the outputs, and `0f` and `10`
/// are the hexadecimal codes of FC15 and FC16. A function of the other direction is refused
/// ("An unknown or direction-incompatible function is rejected during configuration"). Catches an
/// alias that a computer takes and a chip refuses, an alias read in the wrong direction, and a write
/// function accepted for the inputs.
#[test]
fn function_aliases_select_the_computer_functions() {
    for (word, function) in [
        ("read_coils", InputFunction::Coils),
        (" COIL ", InputFunction::Coils),
        ("coils", InputFunction::Coils),
        ("FC01", InputFunction::Coils),
        ("Read-Discrete-Inputs", InputFunction::DiscreteInputs),
        ("discrete", InputFunction::DiscreteInputs),
        ("02", InputFunction::DiscreteInputs),
        ("holding", InputFunction::HoldingRegisters),
        ("fc03", InputFunction::HoldingRegisters),
        ("input_registers", InputFunction::InputRegisters),
        ("04", InputFunction::InputRegisters),
    ] {
        let params = server_with(&format!(r#""input_function":"{word}""#));
        assert_eq!(
            params.map(|params| params.input_function),
            Ok(function),
            "{word}"
        );
    }
    for (word, function) in [
        ("coil", OutputFunction::SingleCoil),
        ("fc05", OutputFunction::SingleCoil),
        ("Single-Register", OutputFunction::SingleRegister),
        ("06", OutputFunction::SingleRegister),
        ("coils", OutputFunction::MultipleCoils),
        ("0F", OutputFunction::MultipleCoils),
        ("15", OutputFunction::MultipleCoils),
        ("registers", OutputFunction::MultipleRegisters),
        ("10", OutputFunction::MultipleRegisters),
        ("FC16", OutputFunction::MultipleRegisters),
    ] {
        let params = server_with(&format!(r#""output_function":"{word}""#));
        assert_eq!(
            params.map(|params| params.output_function),
            Ok(function),
            "{word}"
        );
    }
    let inputs = "must be read_coils, read_discrete_inputs, read_holding_registers or \
                  read_input_registers, not ";
    let outputs = "must be write_single_coil, write_single_register, write_multiple_coils or \
                   write_multiple_registers, not ";
    assert_eq!(
        server_with(r#""input_function":"write_single_coil""#),
        refused("input_function", &format!(r#"{inputs}"write_single_coil""#))
    );
    assert_eq!(
        server_with(r#""input_function":4"#),
        refused("input_function", &format!("{inputs}4"))
    );
    assert_eq!(
        server_with(r#""output_function":"read_coils""#),
        refused("output_function", &format!(r#"{outputs}"read_coils""#))
    );
}

/// The root's keys are checked fail-closed, with the key and the reason: the table and `address`
/// are required, an unknown key is refused like the other shared drivers', the address is an IPv4
/// address with a port other than 0, and each number is a JSON integer in its range. Catches a
/// misspelt key ignored, a unit identifier or a start address cut to fit, a timeout of 0 that fails
/// every request, and a misspelt policy taken as `fault`.
#[test]
fn a_root_key_that_breaks_its_rule_is_refused() {
    let address = "an IPv4 address with a port from 1 to 65535, such as \"192.168.1.10:502\"";
    assert_eq!(
        ModbusTcpParams::from_json("[]"),
        refused("params", "must be a table, not []")
    );
    assert_eq!(
        server_with(r#""port":502"#),
        refused(
            "port",
            "unknown key; the modbus-tcp driver takes address, input_function, input_points, \
             input_start, on_error, output_function, output_points, output_start, timeout_ms and \
             unit_id"
        )
    );
    assert_eq!(
        ModbusTcpParams::from_json("{}"),
        refused("address", &format!("is required, {address}"))
    );
    for given in [
        r#""plc.local:502""#,
        r#""192.168.1.10""#,
        r#""192.168.1.10:0""#,
        r#""192.168.1.10:65536""#,
        r#""[::1]:502""#,
        "502",
    ] {
        assert_eq!(
            ModbusTcpParams::from_json(&format!(r#"{{"address":{given}}}"#)),
            refused("address", &format!("must be {address}, not {given}"))
        );
    }
    assert_eq!(
        server_with(r#""unit_id":256"#),
        refused("unit_id", "must be an integer from 0 to 255, not 256")
    );
    assert_eq!(
        server_with(r#""unit_id":1.0"#),
        refused("unit_id", "must be an integer from 0 to 255, not 1.0")
    );
    assert_eq!(
        server_with(r#""input_start":65536"#),
        refused(
            "input_start",
            "must be an integer from 0 to 65535, not 65536"
        )
    );
    assert_eq!(
        server_with(r#""output_start":-1"#),
        refused("output_start", "must be an integer from 0 to 65535, not -1")
    );
    assert_eq!(
        server_with(r#""timeout_ms":0"#),
        refused(
            "timeout_ms",
            "must be an integer from 1 to 4294967295, not 0"
        )
    );
    assert_eq!(
        server_with(r#""on_error":"halt""#),
        refused(
            "on_error",
            r#"must be "fault", "warn" or "ignore", not "halt""#
        )
    );
}

/// `on_error` reads as the `can` driver reads it: trimmed, in any case, with `warning` for `warn`
/// (specification 11 §6.5: "`fault` (the default), `warn` or `ignore`, as for Modbus"). Catches a
/// Modbus `on_error` that refuses what the other drivers take, and the edges of the ranges cut
/// short: unit 0 and 255, address 65535 and a timeout of 1 ms.
#[test]
fn on_error_reads_as_the_can_driver_reads_it_and_the_ranges_reach_their_ends() {
    for (word, policy) in [
        (" Warning ", OnError::Warn),
        ("IGNORE", OnError::Ignore),
        ("Fault", OnError::Fault),
    ] {
        let params = server_with(&format!(r#""on_error":"{word}""#));
        assert_eq!(params.map(|params| params.on_error), Ok(policy), "{word}");
    }
    let ends = server_with(r#""unit_id":0,"input_start":65535,"timeout_ms":1"#)
        .expect("the ends are taken");
    assert_eq!(
        (ends.unit_id, ends.input_start, ends.timeout_ms),
        (0, 65_535, 1)
    );
    let ends = server_with(r#""unit_id":255,"output_start":65535"#).expect("the ends are taken");
    assert_eq!((ends.unit_id, ends.output_start), (255, 65_535));
}
