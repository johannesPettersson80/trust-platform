// The PDUs of function codes 1 to 6, 15 and 16 against the Modbus Application Protocol
// Specification V1.1b3 (modbus.org, 26 April 2012): its example frames byte for byte, its quantity
// limits at each boundary, its exception codes, and the checks that a response answers its
// request. Every expected byte is copied from the specification's example tables or worked out by
// hand from its field tables in the comment above the assertion.

use trust_modbus::pdu::{self, MAX_PDU_LEN};
use trust_modbus::{Bits, Error, ExceptionCode, FunctionCode, Registers, Request, Response};

/// The PDU that `request` encodes to.
fn encode(request: &Request<'_>) -> Vec<u8> {
    let mut out = [0_u8; MAX_PDU_LEN];
    let len = pdu::encode_request(request, &mut out).expect("a request within the limits encodes");
    out[..len].to_vec()
}

/// The coils or inputs of a read response.
fn bits_of(response: Response<'_>) -> Bits<'_> {
    let Response::Bits(bits) = response else {
        panic!("expected coils or inputs, got {response:?}");
    };
    bits
}

/// The register values of a read response.
fn registers_of(response: Response<'_>) -> Vec<u16> {
    let Response::Registers(registers) = response else {
        panic!("expected registers, got {response:?}");
    };
    (0..registers.quantity())
        .map(|index| {
            registers
                .get(index)
                .expect("a register within the quantity")
        })
        .collect()
}

/// The states of all the bits that a read response gives.
fn states_of(bits: Bits<'_>) -> Vec<bool> {
    (0..bits.quantity())
        .map(|index| bits.get(index).expect("a bit within the quantity"))
        .collect()
}

// §6.1, "a request to read discrete outputs 20-38": request 01 00 13 00 13, response
// 01 03 CD 6B 05. Coil 20 has the address 19 (0x13), and coils 20 to 38 are 19 coils (0x13).
// Each byte is read from its least significant bit ("Output 27 is the MSB of this byte, and output
// 20 is the LSB"):
// CD = 1100 1101 gives coils 20 to 27 = 1 0 1 1 0 0 1 1,
// 6B = 0110 1011 gives coils 28 to 35 = 1 1 0 1 0 1 1 0,
// 05 = 0000 0101 gives coils 36 to 38 = 1 0 1; its five high bits are zero filling.
#[test]
fn read_coils_example_catches_coils_read_from_the_high_end_of_a_byte() {
    let request = Request::ReadCoils {
        address: 0x0013,
        quantity: 0x0013,
    };
    assert_eq!(encode(&request), [0x01, 0x00, 0x13, 0x00, 0x13]);

    let response = pdu::decode_response(&request, &[0x01, 0x03, 0xCD, 0x6B, 0x05])
        .expect("the example response answers its request");
    let states = states_of(bits_of(response));
    assert_eq!(
        states[..8],
        [true, false, true, true, false, false, true, true]
    );
    assert_eq!(
        states[8..16],
        [true, true, false, true, false, true, true, false]
    );
    assert_eq!(states[16..], [true, false, true]);
}

// §6.2, "a request to read discrete inputs 197-218": request 02 00 C4 00 16, response
// 02 03 AC DB 35. Input 197 has the address 196 (0xC4), and inputs 197 to 218 are 22 (0x16):
// AC = 1010 1100 gives inputs 197 to 204 = 0 0 1 1 0 1 0 1,
// DB = 1101 1011 gives inputs 205 to 212 = 1 1 0 1 1 0 1 1,
// 35 = 0011 0101 gives inputs 213 to 218 = 1 0 1 0 1 1; its two high bits are zero filling, so
// there is no 23rd input (index 22).
#[test]
fn read_discrete_inputs_example_catches_padding_bits_read_as_inputs() {
    let request = Request::ReadDiscreteInputs {
        address: 0x00C4,
        quantity: 0x0016,
    };
    assert_eq!(encode(&request), [0x02, 0x00, 0xC4, 0x00, 0x16]);

    let response = pdu::decode_response(&request, &[0x02, 0x03, 0xAC, 0xDB, 0x35])
        .expect("the example response answers its request");
    let bits = bits_of(response);
    let states = states_of(bits);
    assert_eq!(
        states[..8],
        [false, false, true, true, false, true, false, true]
    );
    assert_eq!(
        states[8..16],
        [true, true, false, true, true, false, true, true]
    );
    assert_eq!(states[16..], [true, false, true, false, true, true]);
    assert_eq!(bits.get(22), None);
}

// §6.3, "a request to read registers 108-110": request 03 00 6B 00 03, response
// 03 06 02 2B 00 00 00 64. "The contents of register 108 are shown as the two byte values of
// 02 2B hex, or 555 decimal. The contents of registers 109-110 are 00 00 and 00 64 hex, or 0 and
// 100 decimal." Read low byte first, 02 2B would be 0x2B02 = 11,010.
#[test]
fn read_holding_registers_example_catches_registers_read_low_byte_first() {
    let request = Request::ReadHoldingRegisters {
        address: 0x006B,
        quantity: 0x0003,
    };
    assert_eq!(encode(&request), [0x03, 0x00, 0x6B, 0x00, 0x03]);

    let response =
        pdu::decode_response(&request, &[0x03, 0x06, 0x02, 0x2B, 0x00, 0x00, 0x00, 0x64])
            .expect("the example response answers its request");
    assert_eq!(registers_of(response), [555, 0, 100]);
}

// §6.4, "a request to read input register 9": request 04 00 08 00 01, response 04 02 00 0A, "or
// 10 decimal". The function code 04 is not the 03 of holding registers.
#[test]
fn read_input_registers_example_catches_input_and_holding_registers_confused() {
    let request = Request::ReadInputRegisters {
        address: 0x0008,
        quantity: 0x0001,
    };
    assert_eq!(encode(&request), [0x04, 0x00, 0x08, 0x00, 0x01]);

    let response = pdu::decode_response(&request, &[0x04, 0x02, 0x00, 0x0A])
        .expect("the example response answers its request");
    assert_eq!(registers_of(response), [10]);
}

// §6.5, "a request to write Coil 173 ON": request 05 00 AC FF 00, and the response is its echo.
// "A value of FF 00 hex requests the output to be ON. A value of 00 00 requests it to be OFF."
#[test]
fn write_single_coil_example_catches_on_sent_as_anything_but_ff00() {
    let on = Request::WriteSingleCoil {
        address: 0x00AC,
        value: true,
    };
    assert_eq!(encode(&on), [0x05, 0x00, 0xAC, 0xFF, 0x00]);
    assert_eq!(
        pdu::decode_response(&on, &[0x05, 0x00, 0xAC, 0xFF, 0x00]),
        Ok(Response::Written)
    );

    let off = Request::WriteSingleCoil {
        address: 0x00AC,
        value: false,
    };
    assert_eq!(encode(&off), [0x05, 0x00, 0xAC, 0x00, 0x00]);
}

// §6.6, "a request to write register 2 to 00 03 hex": request 06 00 01 00 03, and the response is
// its echo. The address 00 01 comes before the value 00 03.
#[test]
fn write_single_register_example_catches_the_address_and_value_swapped() {
    let request = Request::WriteSingleRegister {
        address: 0x0001,
        value: 0x0003,
    };
    assert_eq!(encode(&request), [0x06, 0x00, 0x01, 0x00, 0x03]);
    assert_eq!(
        pdu::decode_response(&request, &[0x06, 0x00, 0x01, 0x00, 0x03]),
        Ok(Response::Written)
    );
}

// §6.11, "a request to write a series of 10 coils starting at coil 20": request
// 0F 00 13 00 0A 02 CD 01, response 0F 00 13 00 0A. Ten coils take 10 / 8 = 1 byte and one more
// for the remainder: the byte count is 02, not the 0A of the coils.
#[test]
fn write_multiple_coils_example_catches_a_byte_count_of_coils_instead_of_bytes() {
    let packed = [0xCD, 0x01];
    let request = Request::WriteMultipleCoils {
        address: 0x0013,
        values: Bits::new(10, &packed).expect("ten coils in two bytes"),
    };
    assert_eq!(
        encode(&request),
        [0x0F, 0x00, 0x13, 0x00, 0x0A, 0x02, 0xCD, 0x01]
    );
    assert_eq!(
        pdu::decode_response(&request, &[0x0F, 0x00, 0x13, 0x00, 0x0A]),
        Ok(Response::Written)
    );
}

// §6.12, "a request to write two registers starting at 2 to 00 0A and 01 02 hex": request
// 10 00 01 00 02 04 00 0A 01 02, response 10 00 01 00 02. Two registers take 2 x 2 = 4 bytes:
// the byte count is 04, not the 02 of the registers.
#[test]
fn write_multiple_registers_example_catches_a_byte_count_of_registers_instead_of_bytes() {
    let bytes = [0x00, 0x0A, 0x01, 0x02];
    let request = Request::WriteMultipleRegisters {
        address: 0x0001,
        values: Registers::new(&bytes).expect("two registers in four bytes"),
    };
    assert_eq!(
        encode(&request),
        [0x10, 0x00, 0x01, 0x00, 0x02, 0x04, 0x00, 0x0A, 0x01, 0x02]
    );
    assert_eq!(
        pdu::decode_response(&request, &[0x10, 0x00, 0x01, 0x00, 0x02]),
        Ok(Response::Written)
    );
}

// §7, "Example of a client request and server exception response": request 01 04 A1 00 01,
// response 81 02. 81 is the function code 01 plus 80; 02 "specifies an illegal data address".
#[test]
fn exception_example_catches_an_exception_response_taken_as_data() {
    let request = Request::ReadCoils {
        address: 0x04A1,
        quantity: 0x0001,
    };
    assert_eq!(encode(&request), [0x01, 0x04, 0xA1, 0x00, 0x01]);
    assert_eq!(
        pdu::decode_response(&request, &[0x81, 0x02]),
        Err(Error::Exception {
            function: FunctionCode::ReadCoils,
            code: ExceptionCode::IllegalDataAddress,
        })
    );
}

// §7, table "MODBUS Exception Codes": 01 ILLEGAL FUNCTION, 02 ILLEGAL DATA ADDRESS, 03 ILLEGAL
// DATA VALUE, 04 SERVER DEVICE FAILURE, 05 ACKNOWLEDGE, 06 SERVER DEVICE BUSY, 08 MEMORY PARITY
// ERROR, 0A GATEWAY PATH UNAVAILABLE, 0B GATEWAY TARGET DEVICE FAILED TO RESPOND. The table has
// no code 07 and none above 0B.
#[test]
fn exception_codes_catch_a_code_or_name_other_than_the_specification_table() {
    let table = [
        (0x01, ExceptionCode::IllegalFunction, "ILLEGAL FUNCTION"),
        (
            0x02,
            ExceptionCode::IllegalDataAddress,
            "ILLEGAL DATA ADDRESS",
        ),
        (0x03, ExceptionCode::IllegalDataValue, "ILLEGAL DATA VALUE"),
        (
            0x04,
            ExceptionCode::ServerDeviceFailure,
            "SERVER DEVICE FAILURE",
        ),
        (0x05, ExceptionCode::Acknowledge, "ACKNOWLEDGE"),
        (0x06, ExceptionCode::ServerDeviceBusy, "SERVER DEVICE BUSY"),
        (
            0x08,
            ExceptionCode::MemoryParityError,
            "MEMORY PARITY ERROR",
        ),
        (
            0x0A,
            ExceptionCode::GatewayPathUnavailable,
            "GATEWAY PATH UNAVAILABLE",
        ),
        (
            0x0B,
            ExceptionCode::GatewayTargetDeviceFailedToRespond,
            "GATEWAY TARGET DEVICE FAILED TO RESPOND",
        ),
    ];
    for (code, exception, name) in table {
        assert_eq!(ExceptionCode::from_code(code), exception);
        assert_eq!(exception.code(), code);
        assert_eq!(exception.name(), Some(name));
    }
    for code in [0x00, 0x07, 0x0C] {
        assert_eq!(ExceptionCode::from_code(code), ExceptionCode::Other(code));
        assert_eq!(ExceptionCode::Other(code).name(), None);
    }
}

// The exception of the §7 example, as a message for the operator: it names the code's name from
// the §7 table.
#[test]
fn exception_message_catches_an_exception_reported_without_its_name() {
    let error = Error::Exception {
        function: FunctionCode::ReadCoils,
        code: ExceptionCode::IllegalDataAddress,
    };
    assert!(
        error.to_string().contains("ILLEGAL DATA ADDRESS"),
        "{error}"
    );
}

// §7: an exception response carries "exactly 80 hexadecimal higher" than the request's function
// code. 83 is the exception of Read Holding Registers, so it does not answer Read Coils.
#[test]
fn exception_of_another_function_catches_any_high_bit_taken_as_this_requests_exception() {
    let request = Request::ReadCoils {
        address: 0x04A1,
        quantity: 0x0001,
    };
    assert_eq!(
        pdu::decode_response(&request, &[0x83, 0x02]),
        Err(Error::UnexpectedFunction {
            expected: FunctionCode::ReadCoils,
            actual: 0x83,
        })
    );
}

// §7: an exception response is the function code and the exception code, two bytes.
#[test]
fn exception_without_its_code_catches_an_invented_exception_code() {
    let request = Request::ReadCoils {
        address: 0x04A1,
        quantity: 0x0001,
    };
    assert_eq!(
        pdu::decode_response(&request, &[0x81]),
        Err(Error::ResponseLength {
            expected: 2,
            actual: 1,
        })
    );
    assert_eq!(
        pdu::decode_response(&request, &[0x81, 0x02, 0x00]),
        Err(Error::ResponseLength {
            expected: 2,
            actual: 3,
        })
    );
}

/// The read request of `function` for `quantity` from `address`.
fn read(function: FunctionCode, address: u16, quantity: u16) -> Request<'static> {
    match function {
        FunctionCode::ReadCoils => Request::ReadCoils { address, quantity },
        FunctionCode::ReadDiscreteInputs => Request::ReadDiscreteInputs { address, quantity },
        FunctionCode::ReadHoldingRegisters => Request::ReadHoldingRegisters { address, quantity },
        FunctionCode::ReadInputRegisters => Request::ReadInputRegisters { address, quantity },
        other => panic!("{other} is not a read"),
    }
}

// The request tables: §6.1 "Quantity of coils 1 to 2000 (0x7D0)", §6.2 "Quantity of Inputs 1 to
// 2000 (0x7D0)", §6.3 "Quantity of Registers 1 to 125 (0x7D)", §6.4 "Quantity of Input Registers
// 0x0001 to 0x007D". At the maximum the quantity field is 07 D0 or 00 7D.
#[test]
fn read_quantity_limits_catch_an_off_by_one_at_each_maximum() {
    let limits: [(FunctionCode, u16, [u8; 2]); 4] = [
        (FunctionCode::ReadCoils, 2000, [0x07, 0xD0]),
        (FunctionCode::ReadDiscreteInputs, 2000, [0x07, 0xD0]),
        (FunctionCode::ReadHoldingRegisters, 125, [0x00, 0x7D]),
        (FunctionCode::ReadInputRegisters, 125, [0x00, 0x7D]),
    ];
    for (function, max, max_bytes) in limits {
        assert_eq!(function.max_quantity(), max);
        let encoded = encode(&read(function, 0, max));
        assert_eq!(encoded[3..], max_bytes, "{function}");
        for quantity in [0, max + 1] {
            assert_eq!(
                read(function, 0, quantity).validate(),
                Err(Error::QuantityOutOfRange {
                    function,
                    quantity,
                    max,
                })
            );
        }
    }
}

// §6.11 "Quantity of Outputs 0x0001 to 0x07B0" (1,968): 1,968 coils take 1968 / 8 = 246 bytes
// (0xF6), and 1,969 take 246 and one more, 247. §6.12 "Quantity of Registers 0x0001 to 0x007B"
// (123): 123 registers take 2 x 123 = 246 bytes (0xF6), and 124 take 248. Either request at its
// maximum is 1 + 2 + 2 + 1 + 246 = 252 bytes.
#[test]
fn write_quantity_limits_catch_an_off_by_one_at_each_maximum() {
    let packed = [0_u8; 247];
    let coils = |quantity: u16, len: usize| Request::WriteMultipleCoils {
        address: 0,
        values: Bits::new(quantity, &packed[..len]).expect("the bytes of the quantity"),
    };
    let encoded = encode(&coils(1968, 246));
    assert_eq!(encoded.len(), 252);
    assert_eq!(encoded[..6], [0x0F, 0x00, 0x00, 0x07, 0xB0, 0xF6]);
    for (quantity, len) in [(0, 0), (1969, 247)] {
        assert_eq!(
            coils(quantity, len).validate(),
            Err(Error::QuantityOutOfRange {
                function: FunctionCode::WriteMultipleCoils,
                quantity,
                max: 1968,
            })
        );
    }

    let bytes = [0_u8; 248];
    let registers = |len: usize| Request::WriteMultipleRegisters {
        address: 0,
        values: Registers::new(&bytes[..len]).expect("whole registers"),
    };
    let encoded = encode(&registers(246));
    assert_eq!(encoded.len(), 252);
    assert_eq!(encoded[..6], [0x10, 0x00, 0x00, 0x00, 0x7B, 0xF6]);
    for (quantity, len) in [(0, 0), (124, 248)] {
        assert_eq!(
            registers(len).validate(),
            Err(Error::QuantityOutOfRange {
                function: FunctionCode::WriteMultipleRegisters,
                quantity,
                max: 123,
            })
        );
    }
}

// §6: addresses run from 0x0000 to 0xFFFF, so a read from 0xFFFF (65,535) of 1 register ends at
// 65,535 + 1 = 65,536, the end of the addresses, and of 2 registers would need address 65,536.
// 2,000 coils from 0xF830 (63,536) end at 63,536 + 2,000 = 65,536; from 0xF831 they do not fit.
#[test]
fn address_plus_quantity_catches_requests_past_address_65535() {
    let last = Request::ReadHoldingRegisters {
        address: 0xFFFF,
        quantity: 1,
    };
    assert_eq!(last.validate(), Ok(()));
    let past = Request::ReadHoldingRegisters {
        address: 0xFFFF,
        quantity: 2,
    };
    assert_eq!(
        past.validate(),
        Err(Error::AddressRangeOverflow {
            address: 0xFFFF,
            quantity: 2,
        })
    );

    let to_the_end = Request::ReadCoils {
        address: 0xF830,
        quantity: 2000,
    };
    assert_eq!(to_the_end.validate(), Ok(()));
    let one_past = Request::ReadCoils {
        address: 0xF831,
        quantity: 2000,
    };
    assert_eq!(
        one_past.validate(),
        Err(Error::AddressRangeOverflow {
            address: 0xF831,
            quantity: 2000,
        })
    );
}

// A refused request leaves the buffer as it was: no partial frame can be sent.
#[test]
fn refused_request_catches_bytes_written_before_the_check() {
    let mut out = [0xAA_u8; 8];
    let none = Request::ReadCoils {
        address: 0,
        quantity: 0,
    };
    assert_eq!(
        pdu::encode_request(&none, &mut out),
        Err(Error::QuantityOutOfRange {
            function: FunctionCode::ReadCoils,
            quantity: 0,
            max: 2000,
        })
    );
    assert_eq!(out, [0xAA; 8]);

    // A read request is 5 bytes (§6.1: function code, 2-byte address, 2-byte quantity).
    let mut short = [0xAA_u8; 4];
    let one = Request::ReadCoils {
        address: 0,
        quantity: 1,
    };
    assert_eq!(
        pdu::encode_request(&one, &mut short),
        Err(Error::BufferTooSmall {
            needed: 5,
            available: 4,
        })
    );
    assert_eq!(short, [0xAA; 4]);
}

// §6.3: 3 registers have the byte count 2 x 3 = 6. §6.1: 19 coils have the byte count 19 / 8 = 2
// and one more for the remainder, 3, which a division rounded down would make 2.
#[test]
fn byte_count_catches_a_response_for_another_quantity() {
    let registers = Request::ReadHoldingRegisters {
        address: 0x006B,
        quantity: 3,
    };
    assert_eq!(
        pdu::decode_response(&registers, &[0x03, 0x04, 0x02, 0x2B, 0x00, 0x00]),
        Err(Error::ByteCountMismatch {
            expected: 6,
            actual: 4,
        })
    );
    let coils = Request::ReadCoils {
        address: 0x0013,
        quantity: 0x0013,
    };
    assert_eq!(
        pdu::decode_response(&coils, &[0x01, 0x02, 0xCD, 0x6B]),
        Err(Error::ByteCountMismatch {
            expected: 3,
            actual: 2,
        })
    );
}

// The §6.3 example response with the function code 04 of Read Input Registers does not answer a
// Read Holding Registers request.
#[test]
fn function_code_catches_a_response_to_another_request() {
    let request = Request::ReadHoldingRegisters {
        address: 0x006B,
        quantity: 3,
    };
    assert_eq!(
        pdu::decode_response(&request, &[0x04, 0x06, 0x02, 0x2B, 0x00, 0x00, 0x00, 0x64]),
        Err(Error::UnexpectedFunction {
            expected: FunctionCode::ReadHoldingRegisters,
            actual: 0x04,
        })
    );
}

// §6.3: the response to 3 registers is the function code, the byte count and 2 x 3 bytes,
// 1 + 1 + 6 = 8 bytes; one byte fewer or one more is not that response.
#[test]
fn response_length_catches_a_truncated_or_padded_read() {
    let request = Request::ReadHoldingRegisters {
        address: 0x006B,
        quantity: 3,
    };
    let cases: [(&[u8], usize); 4] = [
        (&[0x03, 0x06, 0x02, 0x2B, 0x00, 0x00, 0x00], 7),
        (&[0x03, 0x06, 0x02, 0x2B, 0x00, 0x00, 0x00, 0x64, 0x00], 9),
        (&[0x03], 1),
        (&[], 0),
    ];
    for (response, actual) in cases {
        assert_eq!(
            pdu::decode_response(&request, response),
            Err(Error::ResponseLength {
                expected: 8,
                actual,
            })
        );
    }
}

// §6.5 and §6.6: "The normal response is an echo of the request". The §6.5 example request
// 05 00 AC FF 00 is not answered by the address 00 AD nor by the value 00 00 (OFF); the §6.6
// example request 06 00 01 00 03 is not answered by the value 00 04, nor by four bytes.
#[test]
fn single_write_echo_catches_an_unchecked_address_or_value() {
    let coil = Request::WriteSingleCoil {
        address: 0x00AC,
        value: true,
    };
    assert_eq!(
        pdu::decode_response(&coil, &[0x05, 0x00, 0xAD, 0xFF, 0x00]),
        Err(Error::AddressMismatch {
            expected: 0x00AC,
            actual: 0x00AD,
        })
    );
    assert_eq!(
        pdu::decode_response(&coil, &[0x05, 0x00, 0xAC, 0x00, 0x00]),
        Err(Error::ValueMismatch {
            expected: 0xFF00,
            actual: 0x0000,
        })
    );
    let register = Request::WriteSingleRegister {
        address: 0x0001,
        value: 0x0003,
    };
    assert_eq!(
        pdu::decode_response(&register, &[0x06, 0x00, 0x01, 0x00, 0x04]),
        Err(Error::ValueMismatch {
            expected: 0x0003,
            actual: 0x0004,
        })
    );
    assert_eq!(
        pdu::decode_response(&register, &[0x06, 0x00, 0x01, 0x00]),
        Err(Error::ResponseLength {
            expected: 5,
            actual: 4,
        })
    );
}

// §6.11 and §6.12: the response returns the starting address and the quantity written. The §6.11
// example (10 coils from 0x13) is not answered by the quantity 9; the §6.12 example (2 registers
// from 0x01) is not answered by the address 2 nor by the quantity 1.
#[test]
fn multiple_write_echo_catches_an_unchecked_address_or_quantity() {
    let packed = [0xCD, 0x01];
    let coils = Request::WriteMultipleCoils {
        address: 0x0013,
        values: Bits::new(10, &packed).expect("ten coils in two bytes"),
    };
    assert_eq!(
        pdu::decode_response(&coils, &[0x0F, 0x00, 0x13, 0x00, 0x09]),
        Err(Error::QuantityMismatch {
            expected: 10,
            actual: 9,
        })
    );
    let bytes = [0x00, 0x0A, 0x01, 0x02];
    let registers = Request::WriteMultipleRegisters {
        address: 0x0001,
        values: Registers::new(&bytes).expect("two registers in four bytes"),
    };
    assert_eq!(
        pdu::decode_response(&registers, &[0x10, 0x00, 0x02, 0x00, 0x02]),
        Err(Error::AddressMismatch {
            expected: 1,
            actual: 2,
        })
    );
    assert_eq!(
        pdu::decode_response(&registers, &[0x10, 0x00, 0x01, 0x00, 0x01]),
        Err(Error::QuantityMismatch {
            expected: 2,
            actual: 1,
        })
    );
}

// §6.11: "Unused bits in the last data byte should be zero-filled." Ten coils use bits 0 and 1 of
// the second byte. FD = 1111 1101 sets coil 28 (bit 0) ON and coil 29 (bit 1) OFF, as the
// example's 01 does, and its six high bits must go out as 0: FD AND 0000 0011 = 01, so the frame
// is the example's.
#[test]
fn unused_coil_bits_catch_bits_past_the_quantity_sent_on_the_wire() {
    let packed = [0xCD, 0xFD];
    let request = Request::WriteMultipleCoils {
        address: 0x0013,
        values: Bits::new(10, &packed).expect("ten coils in two bytes"),
    };
    assert_eq!(
        encode(&request),
        [0x0F, 0x00, 0x13, 0x00, 0x0A, 0x02, 0xCD, 0x01]
    );
}

// §6.11: ten coils take 10 / 8 = 1 byte and one more for the remainder, 2; §6.12: a register is
// two bytes.
#[test]
fn value_lengths_catch_bits_or_registers_that_do_not_fill_their_bytes() {
    assert_eq!(
        Bits::new(10, &[0xCD]),
        Err(Error::PackedBitsLength {
            quantity: 10,
            len: 1,
        })
    );
    assert_eq!(
        Bits::new(10, &[0xCD, 0x01, 0x00]),
        Err(Error::PackedBitsLength {
            quantity: 10,
            len: 3,
        })
    );
    assert_eq!(
        Registers::new(&[0x00, 0x0A, 0x01]),
        Err(Error::RegisterBytes { len: 3 })
    );
}
