// Modbus TCP frames against the Modbus Messaging on TCP/IP Implementation Guide V1.0b (modbus.org,
// 24 October 2006) and the frame size of the Modbus Application Protocol Specification V1.1b3
// §4.1. The MBAP header (V1.0b §3.1.3) is seven bytes, each field high byte first: the transaction
// identifier (2), the protocol identifier (2, 0 for Modbus), the length (2, "a byte count of the
// following fields, including the Unit Identifier and data fields") and the unit identifier (1).

use trust_modbus::tcp::{self, MbapHeader, MAX_ADU_LEN};
use trust_modbus::{Error, ExceptionCode, FunctionCode, Registers, Request, Response};

// The V1.1b3 §6.3 example request, "read registers 108-110": 03 00 6B 00 03.
const REQUEST: Request<'static> = Request::ReadHoldingRegisters {
    address: 0x006B,
    quantity: 3,
};

// The V1.1b3 §6.3 example response, 03 06 02 2B 00 00 00 64 (8 bytes), from unit 0x11 in
// transaction 1, with its header worked out by hand: transaction 00 01, protocol 00 00, length
// 1 + 8 = 9 (00 09), unit 11.
const RESPONSE: [u8; 15] = [
    0x00, 0x01, 0x00, 0x00, 0x00, 0x09, 0x11, 0x03, 0x06, 0x02, 0x2B, 0x00, 0x00, 0x00, 0x64,
];

/// `RESPONSE` with the byte at `index` changed to `byte`.
fn changed(index: usize, byte: u8) -> [u8; 15] {
    let mut frame = RESPONSE;
    frame[index] = byte;
    frame
}

// V1.0b §4.4.1.2, "the MODBUS request ADU encoding for reading the register # 5 in a remote
// server": Transaction Identifier 0x15 0x01, Protocol Identifier 0x0000, Length 0x0006, Unit
// Identifier 0xFF, Function Code 0x03, Starting Address 0x0004, Quantity of Registers 0x0001. The
// length 6 is the unit identifier and the 5-byte PDU; without the unit identifier it would be 5.
#[test]
fn request_frame_matches_the_guide_example_and_catches_a_length_without_the_unit() {
    let request = Request::ReadHoldingRegisters {
        address: 0x0004,
        quantity: 0x0001,
    };
    let mut out = [0_u8; MAX_ADU_LEN];
    let len = tcp::encode_request(0x1501, 0xFF, &request, &mut out).expect("the example encodes");
    assert_eq!(
        out[..len],
        [0x15, 0x01, 0x00, 0x00, 0x00, 0x06, 0xFF, 0x03, 0x00, 0x04, 0x00, 0x01]
    );
}

// The PDU starts after the seven header bytes, so register 108 is 02 2B = 555 and registers 109
// and 110 are 0 and 100 (V1.1b3 §6.3). A decoder that took the unit identifier 11 as the
// function code would refuse the frame.
#[test]
fn response_frame_catches_the_unit_identifier_read_as_the_pdu() {
    let response = tcp::decode_response(0x0001, &REQUEST, &RESPONSE).expect("the response answers");
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

// V1.0b §3.1.3: the server copies the request's transaction identifier into its response. A
// response with transaction 2 answers another request than transaction 1, such as a late answer
// to an earlier one.
#[test]
fn transaction_identifier_catches_an_answer_to_another_request() {
    assert_eq!(
        tcp::decode_response(0x0001, &REQUEST, &changed(1, 0x02)),
        Err(Error::TransactionMismatch {
            expected: 1,
            actual: 2,
        })
    );
}

// V1.0b §4.4.1.3: a client may ignore the unit identifier of a server directly on TCP/IP, and
// truST does (specification 11 §6.5), so a server that answers with another unit identifier is
// still read. Catches a client that drops the data of a device that echoes its unit id wrongly.
#[test]
fn unit_identifier_of_the_answer_is_not_compared() {
    assert!(tcp::decode_response(0x0001, &REQUEST, &changed(6, 0x12)).is_ok());
}

// V1.0b §3.1.3: "The MODBUS protocol is identified by the value 0"; §4.4.1.3: "After the
// verification of the Protocol Identifier that must be 0x0000".
#[test]
fn protocol_identifier_catches_a_frame_of_another_protocol() {
    assert_eq!(
        tcp::decode_response(0x0001, &REQUEST, &changed(3, 0x01)),
        Err(Error::ProtocolId { actual: 1 })
    );
}

// Nine bytes follow the length field of `RESPONSE`: the unit identifier and the 8-byte PDU. A
// length of 8 or 10 does not count them.
#[test]
fn length_field_catches_a_header_that_disagrees_with_the_bytes_after_it() {
    for length in [8, 10] {
        assert_eq!(
            tcp::decode_response(0x0001, &REQUEST, &changed(5, length)),
            Err(Error::MbapLengthMismatch {
                length: u16::from(length),
                following: 9,
            })
        );
    }
}

// The length counts the unit identifier and the PDU: at least 1 + 1 (a function code) = 2, and at
// most 1 + 253 = 254, as "TCP MODBUS ADU = 253 bytes + MBAP (7 bytes) = 260 bytes" (V1.1b3 §4.1).
// A header with a larger length is refused before its bytes are read.
#[test]
fn header_length_bounds_catch_a_header_that_would_read_up_to_65535_bytes() {
    let header = |high: u8, low: u8| MbapHeader::decode([0x00, 0x01, 0x00, 0x00, high, low, 0x11]);
    assert_eq!(header(0x00, 0x01), Err(Error::MbapLength { length: 1 }));
    assert_eq!(header(0x00, 0x02).map(MbapHeader::pdu_len), Ok(1));
    assert_eq!(header(0x00, 0xFE).map(MbapHeader::pdu_len), Ok(253));
    assert_eq!(header(0x00, 0xFF), Err(Error::MbapLength { length: 255 }));
    assert_eq!(
        header(0xFF, 0xFF),
        Err(Error::MbapLength { length: 65_535 })
    );
    assert_eq!(
        MbapHeader::decode([0x12, 0x34, 0x00, 0x00, 0x00, 0x09, 0x11]),
        Ok(MbapHeader {
            transaction_id: 0x1234,
            length: 9,
            unit_id: 0x11,
        })
    );
}

// V1.1b3 §4.1: a Modbus TCP frame is at most 253 + 7 = 260 bytes, and a response is at least the
// header and a function code, 7 + 1 = 8.
#[test]
fn frame_size_bounds_catch_frames_past_260_bytes() {
    let long = [0_u8; 261];
    assert_eq!(
        tcp::decode_response(0x0001, &REQUEST, &long),
        Err(Error::FrameLength {
            len: 261,
            min: 8,
            max: 260,
        })
    );
    assert_eq!(
        tcp::decode_response(0x0001, &REQUEST, &RESPONSE[..7]),
        Err(Error::FrameLength {
            len: 7,
            min: 8,
            max: 260,
        })
    );
}

// The largest request of these functions, Write Multiple Registers with 123 registers, has a PDU
// of 1 + 2 + 2 + 1 + 2 x 123 = 252 bytes (V1.1b3 §6.12), so its frame is 7 + 252 = 259 bytes and
// its length 1 + 252 = 253 (00 FD), not the 259 of the whole frame.
#[test]
fn largest_request_catches_a_length_that_counts_the_header() {
    let bytes = [0_u8; 246];
    let request = Request::WriteMultipleRegisters {
        address: 0,
        values: Registers::new(&bytes).expect("123 registers"),
    };
    let mut out = [0_u8; 259];
    assert_eq!(tcp::encode_request(7, 1, &request, &mut out), Ok(259));
    assert_eq!(out[..7], [0x00, 0x07, 0x00, 0x00, 0x00, 0xFD, 0x01]);
}

// V1.1b3 §7: the exception response is the function code plus 0x80 and the exception code, here
// 83 02, so the length is 1 + 2 = 3.
#[test]
fn exception_frame_catches_an_exception_response_taken_as_data() {
    let frame = [0x00, 0x01, 0x00, 0x00, 0x00, 0x03, 0x11, 0x83, 0x02];
    assert_eq!(
        tcp::decode_response(0x0001, &REQUEST, &frame),
        Err(Error::Exception {
            function: FunctionCode::ReadHoldingRegisters,
            code: ExceptionCode::IllegalDataAddress,
        })
    );
}

// A refused request leaves the buffer as it was: 126 registers are past the 125 of V1.1b3 §6.3,
// and the guide example's frame takes 7 + 5 = 12 bytes.
#[test]
fn refused_request_catches_a_header_written_before_the_check() {
    let mut out = [0xAA_u8; MAX_ADU_LEN];
    let too_many = Request::ReadHoldingRegisters {
        address: 0,
        quantity: 126,
    };
    assert_eq!(
        tcp::encode_request(1, 1, &too_many, &mut out),
        Err(Error::QuantityOutOfRange {
            function: FunctionCode::ReadHoldingRegisters,
            quantity: 126,
            max: 125,
        })
    );
    assert_eq!(out, [0xAA; MAX_ADU_LEN]);

    let mut short = [0xAA_u8; 11];
    let example = Request::ReadHoldingRegisters {
        address: 0x0004,
        quantity: 0x0001,
    };
    assert_eq!(
        tcp::encode_request(0x1501, 0xFF, &example, &mut short),
        Err(Error::BufferTooSmall {
            needed: 12,
            available: 11,
        })
    );
    assert_eq!(short, [0xAA; 11]);
}
