//! The requests of a raw exchange (specification 11 §6.5): the quantity that the image takes, the
//! quantity limits of V1.1b3, the requests in address order, and a range past address 65535
//! refused at configuration.

use alloc::format;
use alloc::vec::Vec;
use core::ops::Range;

use trust_modbus::FunctionCode;

use crate::io_drivers::modbus::plan::RawPlan;
use crate::io_drivers::modbus::ModbusTcpParams;
use crate::io_drivers::ParamsError;

/// The plan of `function` from `start` for an image of `len` bytes.
fn plan(function: FunctionCode, start: u16, len: usize) -> RawPlan {
    RawPlan::new(function, start, len, "input").expect("a range within the addresses")
}

/// The requests of `plan`, in order: each one's address and quantity, and its image bytes.
fn requests(plan: RawPlan) -> Vec<((u16, u16), Range<usize>)> {
    (0..plan.requests())
        .map(|index| {
            let request = plan.request(index).expect("a request of the plan");
            (request, plan.bytes(index))
        })
        .collect()
}

/// The refusal of `key` for `reason`.
fn refused(key: &str, reason: &str) -> Result<(), ParamsError> {
    Err(ParamsError {
        key: key.into(),
        reason: reason.into(),
    })
}

/// "Register quantity is derived from the process image size (`ceil(bytes / 2)`)", and coils are
/// one a bit, the first in the least significant bit of the first byte: 5 bytes are 3 input
/// registers, 3 bytes 2 holding registers, 2 bytes 16 coils, 1 byte 8 coils; FC05 writes the first
/// byte's coil and FC06 the register of the first two bytes; an empty image sends nothing, as a
/// quantity of 0 is not Modbus (V1.1b3 §6). Catches a quantity counted in bytes, the last register
/// of an odd image lost, and a request of quantity 0.
#[test]
fn the_quantity_follows_the_image_size() {
    assert_eq!(
        requests(plan(FunctionCode::ReadInputRegisters, 0, 5)),
        [((0, 3), 0..6)]
    );
    assert_eq!(
        requests(plan(FunctionCode::WriteMultipleRegisters, 4, 3)),
        [((4, 2), 0..4)]
    );
    assert_eq!(
        requests(plan(FunctionCode::ReadCoils, 0, 2)),
        [((0, 16), 0..2)]
    );
    assert_eq!(
        requests(plan(FunctionCode::WriteMultipleCoils, 9, 1)),
        [((9, 8), 0..1)]
    );
    assert_eq!(
        requests(plan(FunctionCode::WriteSingleCoil, 7, 4)),
        [((7, 1), 0..1)]
    );
    assert_eq!(
        requests(plan(FunctionCode::WriteSingleRegister, 7, 1)),
        [((7, 1), 0..2)]
    );
    for function in [
        FunctionCode::ReadCoils,
        FunctionCode::ReadDiscreteInputs,
        FunctionCode::ReadHoldingRegisters,
        FunctionCode::ReadInputRegisters,
        FunctionCode::WriteSingleCoil,
        FunctionCode::WriteSingleRegister,
        FunctionCode::WriteMultipleCoils,
        FunctionCode::WriteMultipleRegisters,
    ] {
        assert_eq!(plan(function, 0, 0).requests(), 0, "{function}");
    }
}

/// "A process image larger than one request is exchanged in several requests of at most that many,
/// in address order": 300 bytes are 150 input registers, 125 from address 0 (bytes 0 to 249) and 25
/// from 125 (bytes 250 to 299); 251 bytes from 100 are 126 holding registers, 125 and then 1, whose
/// high byte is the image's last; 300 bytes are 2,400 coils, 2,000 and 400 for FC01, or 1,968 (246
/// bytes) and 432 for FC15; and for FC16 150 registers are 123 (246 bytes) and 27. Exactly the
/// limit is one request. Catches a request above V1.1b3's quantity, requests out of address order,
/// and a request that starts at the wrong image byte.
#[test]
fn an_image_larger_than_one_request_is_split_in_address_order() {
    assert_eq!(
        requests(plan(FunctionCode::ReadInputRegisters, 0, 300)),
        [((0, 125), 0..250), ((125, 25), 250..300)]
    );
    assert_eq!(
        requests(plan(FunctionCode::ReadHoldingRegisters, 100, 251)),
        [((100, 125), 0..250), ((225, 1), 250..252)]
    );
    assert_eq!(
        requests(plan(FunctionCode::ReadCoils, 0, 300)),
        [((0, 2000), 0..250), ((2000, 400), 250..300)]
    );
    assert_eq!(
        requests(plan(FunctionCode::WriteMultipleCoils, 0, 300)),
        [((0, 1968), 0..246), ((1968, 432), 246..300)]
    );
    assert_eq!(
        requests(plan(FunctionCode::WriteMultipleRegisters, 0, 300)),
        [((0, 123), 0..246), ((123, 27), 246..300)]
    );
    assert_eq!(
        requests(plan(FunctionCode::ReadDiscreteInputs, 10, 250)),
        [((10, 2000), 0..250)]
    );
    assert_eq!(
        requests(plan(FunctionCode::WriteMultipleRegisters, 0, 246)),
        [((0, 123), 0..246)]
    );
}

/// "A range that reaches past address 65,535 is refused at configuration": 12 bytes from 65530 are
/// 6 input registers, 65530 to 65535, the last address, and 13 bytes are 7, one too many; 1 byte
/// from 65528 is 8 coils, to 65535, and 2 bytes are 16; an output image of 2 bytes from 65535 is 1
/// holding register and one of 3 bytes is 2; FC05 writes one coil whatever the image's size.
/// Catches a range that wraps around to address 0, and the last address refused.
#[test]
fn a_raw_range_past_65535_is_refused_at_configuration() {
    let params = |fields: &str| {
        let text = format!(r#"{{"address":"10.0.0.5:502",{fields}}}"#);
        ModbusTcpParams::from_json(&text).expect("valid parameters")
    };
    let registers = params(r#""input_start":65530"#);
    assert_eq!(registers.check_images(12, 0), Ok(()));
    assert_eq!(
        registers.check_images(13, 0),
        refused(
            "input_start",
            "7 input registers from address 65530 for the 13-byte input image run past address \
             65535"
        )
    );
    let coils = params(r#""input_start":65528,"input_function":"read_coils""#);
    assert_eq!(coils.check_images(1, 0), Ok(()));
    assert_eq!(
        coils.check_images(2, 0),
        refused(
            "input_start",
            "16 coils from address 65528 for the 2-byte input image run past address 65535"
        )
    );
    let outputs = params(r#""output_start":65535"#);
    assert_eq!(outputs.check_images(0, 2), Ok(()));
    assert_eq!(
        outputs.check_images(0, 3),
        refused(
            "output_start",
            "2 holding registers from address 65535 for the 3-byte output image run past address \
             65535"
        )
    );
    let single = params(r#""output_start":65535,"output_function":"write_single_coil""#);
    assert_eq!(single.check_images(0, 100), Ok(()));
}
