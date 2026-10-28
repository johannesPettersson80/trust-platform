// Modbus RTU frames, CRC and silent intervals against the Modbus over Serial Line Specification and
// Implementation Guide V1.02 (modbus.org, 20 December 2006). The PDUs inside the frames are
// examples of the Modbus Application Protocol Specification V1.1b3.
//
// The CRCs of frames that no specification prints (the responses below) were worked out with an
// implementation of the V1.02 §6.2.2 procedure written apart from this crate; the CRC function
// itself is pinned by the worked example of V1.02 §6.2.2, by a one-byte CRC worked by hand step by
// step, and by the published check value of CRC-16/MODBUS.

use trust_modbus::rtu::{self, SilentIntervals, MAX_ADU_LEN};
use trust_modbus::{Error, ExceptionCode, FunctionCode, Registers, Request, Response};

// The V1.1b3 §6.3 example request, "read registers 108-110": 03 00 6B 00 03.
const REQUEST: Request<'static> = Request::ReadHoldingRegisters {
    address: 0x006B,
    quantity: 3,
};

// The V1.1b3 §6.3 example response 03 06 02 2B 00 00 00 64 from server 1: the address 01, the
// PDU, and its CRC 0x7A05 low byte first, 05 7A.
const RESPONSE: [u8; 11] = [
    0x01, 0x03, 0x06, 0x02, 0x2B, 0x00, 0x00, 0x00, 0x64, 0x05, 0x7A,
];

// V1.02 §6.2.2, "Example of CRC calculation (frame 02 07)": the register ends at
// 0001 0010 0100 0001 = 0x1241, and "The CRC 16 of the frame is then: 4112", low byte first.
#[test]
fn crc_of_the_serial_line_example_catches_a_wrong_preset_or_polynomial() {
    assert_eq!(rtu::crc16(&[0x02, 0x07]), 0x1241);
}

// The CRC of the one byte 01 by the procedure of V1.02 §6.2.2 ("Shift the CRC register one bit to
// the right ... If the LSB was 1: Exclusive OR the CRC register with the polynomial value 0xA001"):
// load FFFF; FFFF XOR 0001 = FFFE;
// shift 1: out 0, 7FFF;
// shift 2: out 1, 3FFF XOR A001 = 9FFE;
// shift 3: out 0, 4FFF;
// shift 4: out 1, 27FF XOR A001 = 87FE;
// shift 5: out 0, 43FF;
// shift 6: out 1, 21FF XOR A001 = 81FE;
// shift 7: out 0, 40FF;
// shift 8: out 1, 207F XOR A001 = 807E.
// Combining with A001 before the shift instead of after it gives 6FFF at shift 2.
#[test]
fn crc_of_one_byte_catches_the_polynomial_applied_before_the_shift() {
    assert_eq!(rtu::crc16(&[0x01]), 0x807E);
}

// The check value of CRC-16/MODBUS, the CRC of the nine ASCII bytes "123456789", is 0x4B37 (Greg
// Cook, Catalogue of parametrised CRC algorithms: width 16, polynomial 0x8005, whose 16 bits in
// reverse order are 0xA001, reflected input and output, initial value 0xFFFF, no final XOR). The
// same polynomial shifted toward the high bit, unreflected, gives 0xAEE7.
#[test]
fn crc_check_value_catches_a_crc_that_shifts_toward_the_high_bit() {
    assert_eq!(rtu::crc16(b"123456789"), 0x4B37);
}

// The CRC of 01 03 00 00 00 0A (server 1, Read Holding Registers from address 0, 10 registers) is
// 0xCDC5, and V1.02 §2.5.1.2 sends "the low-order byte of the field ... first, followed by the
// high-order byte": the frame ends C5 CD.
#[test]
fn request_frame_catches_a_crc_sent_high_byte_first() {
    assert_eq!(rtu::crc16(&[0x01, 0x03, 0x00, 0x00, 0x00, 0x0A]), 0xCDC5);
    let request = Request::ReadHoldingRegisters {
        address: 0,
        quantity: 10,
    };
    let mut out = [0_u8; MAX_ADU_LEN];
    let len = rtu::encode_request(1, &request, &mut out).expect("the request encodes");
    assert_eq!(out[..len], [0x01, 0x03, 0x00, 0x00, 0x00, 0x0A, 0xC5, 0xCD]);
}

// The response's registers come from between the address and the CRC: 555, 0 and 100
// (V1.1b3 §6.3).
#[test]
fn response_frame_catches_the_address_or_crc_read_as_registers() {
    let response = rtu::decode_response(1, &REQUEST, &RESPONSE).expect("the response answers");
    let Response::Registers(registers) = response else {
        panic!("expected registers, got {response:?}");
    };
    assert_eq!(
        [
            registers.get(0),
            registers.get(1),
            registers.get(2),
            registers.get(3)
        ],
        [Some(555), Some(0), Some(100), None]
    );
}

// V1.02 §2.5.1.2: "If the two values are not equal, an error results." With register 108 changed
// from 02 2B to 02 2C, the bytes give the CRC 0xBAB0, not the 0x7A05 the frame carries.
#[test]
fn crc_check_catches_a_corrupted_frame() {
    let mut corrupted = RESPONSE;
    corrupted[4] = 0x2C;
    assert_eq!(
        rtu::decode_response(1, &REQUEST, &corrupted),
        Err(Error::CrcMismatch {
            computed: 0xBAB0,
            received: 0x7A05,
        })
    );
}

// The same frame with its CRC high byte first, 7A 05: the bytes give 0x7A05, and the frame,
// read low byte first, carries 0x057A.
#[test]
fn crc_byte_order_catches_a_crc_read_high_byte_first() {
    let mut swapped = RESPONSE;
    swapped[9] = 0x7A;
    swapped[10] = 0x05;
    assert_eq!(
        rtu::decode_response(1, &REQUEST, &swapped),
        Err(Error::CrcMismatch {
            computed: 0x7A05,
            received: 0x057A,
        })
    );
}

// V1.02 §2.3: "When the slave returns its response, it places its own address in the response
// address field". The same response from server 2, with its CRC 0x8A11 (11 8A), does not answer a
// request to server 1.
#[test]
fn server_address_catches_an_answer_from_another_server() {
    let from_two = [
        0x02, 0x03, 0x06, 0x02, 0x2B, 0x00, 0x00, 0x00, 0x64, 0x11, 0x8A,
    ];
    assert_eq!(
        rtu::decode_response(1, &REQUEST, &from_two),
        Err(Error::UnitMismatch {
            expected: 1,
            actual: 2,
        })
    );
}

// The V1.1b3 §7 example: request 01 04 A1 00 01, exception response 81 02, here from server 1
// with its CRC 0x91C1 (C1 91).
#[test]
fn exception_frame_catches_an_exception_response_taken_as_data() {
    let request = Request::ReadCoils {
        address: 0x04A1,
        quantity: 1,
    };
    assert_eq!(
        rtu::decode_response(1, &request, &[0x01, 0x81, 0x02, 0xC1, 0x91]),
        Err(Error::Exception {
            function: FunctionCode::ReadCoils,
            code: ExceptionCode::IllegalDataAddress,
        })
    );
}

// V1.02 §2.1: "No response is returned to broadcast requests sent by the master. The broadcast
// requests are necessarily writing commands." §2.2: "The Address 0 is reserved as the broadcast
// address." The V1.1b3 §6.6 example request 06 00 01 00 03 broadcast: 00, the PDU, and its CRC
// 0xDA99 (99 DA).
#[test]
fn broadcast_catches_a_read_sent_to_every_server() {
    assert!(!rtu::expects_response(0));
    assert!(rtu::expects_response(1));
    assert!(rtu::expects_response(247));

    let mut out = [0xAA_u8; MAX_ADU_LEN];
    let read = Request::ReadHoldingRegisters {
        address: 0x0001,
        quantity: 1,
    };
    assert_eq!(
        rtu::encode_request(0, &read, &mut out),
        Err(Error::BroadcastRead {
            function: FunctionCode::ReadHoldingRegisters,
        })
    );
    assert_eq!(out, [0xAA; MAX_ADU_LEN]);

    let write = Request::WriteSingleRegister {
        address: 0x0001,
        value: 0x0003,
    };
    let len = rtu::encode_request(0, &write, &mut out).expect("a write may be broadcast");
    assert_eq!(out[..len], [0x00, 0x06, 0x00, 0x01, 0x00, 0x03, 0x99, 0xDA]);
    assert_eq!(
        rtu::decode_response(0, &write, &out[..len]),
        Err(Error::NoResponseToBroadcast)
    );
}

// V1.02 §2.2: "From 1 to 247: Slave individual addresses; From 248 to 255: Reserved". The V1.1b3
// §6.6 example request to server 247 (F7), with its CRC 0x9D8C (8C 9D).
#[test]
fn reserved_addresses_catch_a_request_to_248_or_above() {
    let write = Request::WriteSingleRegister {
        address: 0x0001,
        value: 0x0003,
    };
    let mut out = [0_u8; MAX_ADU_LEN];
    let len = rtu::encode_request(247, &write, &mut out).expect("247 is a server address");
    assert_eq!(out[..len], [0xF7, 0x06, 0x00, 0x01, 0x00, 0x03, 0x8C, 0x9D]);
    for address in [248, 255] {
        assert_eq!(
            rtu::encode_request(address, &write, &mut out),
            Err(Error::ReservedAddress { address })
        );
        assert_eq!(
            rtu::decode_response(address, &write, &out[..len]),
            Err(Error::ReservedAddress { address })
        );
    }
}

// V1.02 §2.5.1: "The maximum size of a MODBUS RTU frame is 256 bytes", and its least is the
// address, a function code and the CRC, 1 + 1 + 2 = 4 (figure 12: "Data 0 up to 252 byte(s)").
// The largest request of these functions, Write Multiple Registers with 123 registers, is
// 1 + (1 + 2 + 2 + 1 + 2 x 123) + 2 = 255 bytes.
#[test]
fn frame_size_bounds_catch_frames_past_256_bytes() {
    let long = [0_u8; 257];
    assert_eq!(
        rtu::decode_response(1, &REQUEST, &long),
        Err(Error::FrameLength {
            len: 257,
            min: 4,
            max: 256,
        })
    );
    assert_eq!(
        rtu::decode_response(1, &REQUEST, &RESPONSE[..3]),
        Err(Error::FrameLength {
            len: 3,
            min: 4,
            max: 256,
        })
    );

    let bytes = [0_u8; 246];
    let largest = Request::WriteMultipleRegisters {
        address: 0,
        values: Registers::new(&bytes).expect("123 registers"),
    };
    let mut out = [0_u8; MAX_ADU_LEN];
    assert_eq!(rtu::encode_request(1, &largest, &mut out), Ok(255));
}

// V1.02 §2.5.1.1 at 9,600 baud. An RTU character is 11 bits (§2.5.1), so
// t3.5 = 3.5 x 11 bits / 9,600 bit/s = 38.5 / 9,600 s = 0.0040104166... s = 4,010.4166... µs,
// rounded up to 4,011 µs, as a silence shorter than 3.5 characters would not separate frames;
// t1.5 = 1.5 x 11 bits / 9,600 bit/s = 16.5 / 9,600 s = 0.00171875 s = 1,718.75 µs, rounded up
// to 1,719 µs.
#[test]
fn silent_intervals_at_9600_baud_catch_intervals_rounded_down() {
    assert_eq!(
        rtu::silent_intervals(9600),
        Ok(SilentIntervals {
            inter_frame_delay_us: 4011,
            inter_character_timeout_us: 1719,
        })
    );
}

// V1.02 §2.5.1.1: the two timers "must be strictly respected when the baud rate is equal or lower
// than 19200 Bps". At 19,200 baud: t3.5 = 38.5 / 19,200 s = 2,005.208... µs, rounded up to
// 2,006 µs; t1.5 = 16.5 / 19,200 s = 859.375 µs, rounded up to 860 µs.
#[test]
fn silent_intervals_at_19200_baud_catch_the_fixed_values_used_too_early() {
    assert_eq!(
        rtu::silent_intervals(19_200),
        Ok(SilentIntervals {
            inter_frame_delay_us: 2006,
            inter_character_timeout_us: 860,
        })
    );
}

// V1.02 §2.5.1.1: "For baud rates greater than 19200 Bps ... it is recommended to use a value of
// 750µs for the inter-character time-out (t1.5) and a value of 1.750ms for inter-frame delay
// (t3.5)." At 115,200 baud character times would give 38.5 / 115,200 s = 334.2 µs and
// 16.5 / 115,200 s = 143.2 µs.
#[test]
fn silent_intervals_above_19200_baud_catch_character_times_at_high_rates() {
    for baud in [19_201, 115_200] {
        assert_eq!(
            rtu::silent_intervals(baud),
            Ok(SilentIntervals {
                inter_frame_delay_us: 1750,
                inter_character_timeout_us: 750,
            })
        );
    }
}

// At 1,200 baud: t3.5 = 38.5 / 1,200 s = 32,083.33... µs, rounded up to 32,084 µs; t1.5 =
// 16.5 / 1,200 s = 0.01375 s = 13,750 µs exactly, which rounding up leaves at 13,750 µs.
#[test]
fn silent_intervals_at_1200_baud_catch_an_exact_interval_rounded_past_itself() {
    assert_eq!(
        rtu::silent_intervals(1200),
        Ok(SilentIntervals {
            inter_frame_delay_us: 32_084,
            inter_character_timeout_us: 13_750,
        })
    );
}

// A character time is 11 bits divided by the baud rate, which 0 baud does not have.
#[test]
fn zero_baud_catches_a_division_by_zero() {
    assert_eq!(rtu::silent_intervals(0), Err(Error::ZeroBaudRate));
}
