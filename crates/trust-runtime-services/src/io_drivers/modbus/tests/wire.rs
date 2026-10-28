//! The wire side of the exchange (specification 11 §6.5; Modbus Application Protocol V1.1b3;
//! Modbus Messaging on TCP/IP V1.0b): the request of each function, the checks of each response
//! before its data is taken, frames in pieces and of other transactions, large raw images in
//! several requests, and the point maps on the wire.

use alloc::format;
use alloc::vec;

use trust_runtime_core::error::RuntimeError;

use super::{faulted, transport, Bench};
use crate::io_drivers::modbus::ModbusPoll;

/// "A response with another transaction id is discarded and the client reads on until its own or
/// its timeout (V1.0b §4.4.1.3). The unit id of a TCP response is not compared": transaction 9's
/// complete answer with EE EE comes first, then the request's own answer from unit 0xFF, both cut
/// across three reads of 4, 11 and 7 bytes. Only the own answer's 11 22 reach the image. Catches
/// another transaction's data taken, a failure instead of reading on, a unit identifier compared,
/// and a frame that must arrive in one piece.
#[test]
fn a_response_of_another_transaction_is_discarded_and_reading_goes_on() {
    let mut bench = Bench::new("");
    assert_eq!(bench.exchange.request_inputs(0, 2), 40);
    bench.connect(0);
    bench.sent(1);
    let bytes = [
        0x00, 0x09, 0x00, 0x00, 0x00, 0x05, 0x01, 0x04, 0x02, 0xEE, 0xEE, 0x00, 0x01, 0x00, 0x00,
        0x00, 0x05, 0xFF, 0x04, 0x02, 0x11, 0x22,
    ];
    bench.exchange.received(2, &bytes[..4]);
    bench.exchange.received(2, &bytes[4..15]);
    assert!(!bench.exchange.inputs_settled());
    bench.exchange.received(3, &bytes[15..]);
    assert!(bench.exchange.inputs_settled());
    let mut image = [0; 2];
    assert_eq!(bench.exchange.read_inputs(3, &mut image), Ok(()));
    assert_eq!(image, [0x11, 0x22]);
}

/// "Every response is checked before its data is taken ... A response that fails a check is a
/// transport error under `on_error`, and no byte of it reaches the image": the function 03 to a
/// request of 04, a byte count of 4 for one register, the protocol 1 and the length 255 of an MBAP
/// header, and a write's echo of address 1 for 0 each fail as `IoTransport` with the words of
/// `trust-modbus`, give the connection up, and leave the image as it was. Catches the data of a
/// wrong answer taken, a check failure reported as an exception or a data error, and a connection
/// kept after its framing broke.
#[test]
fn a_response_that_fails_a_check_is_a_transport_error_and_reaches_no_byte() {
    let refused = "modbus tcp 10.0.0.5:502: refused response: ";
    for (answer, reason) in [
        (
            &[
                0x00, 0x01, 0x00, 0x00, 0x00, 0x05, 0x01, 0x03, 0x02, 0x11, 0x22,
            ][..],
            "function code 0x03 does not answer 0x04 Read Input Registers",
        ),
        (
            &[
                0x00, 0x01, 0x00, 0x00, 0x00, 0x05, 0x01, 0x04, 0x04, 0x11, 0x22,
            ][..],
            "the byte count is 4, not 2",
        ),
        (
            &[
                0x00, 0x01, 0x00, 0x01, 0x00, 0x05, 0x01, 0x04, 0x02, 0x11, 0x22,
            ][..],
            "the MBAP protocol identifier is 1, not 0",
        ),
        (
            &[0x00, 0x01, 0x00, 0x00, 0x00, 0xFF, 0x01][..],
            "the MBAP length 255 is not 2 to 254",
        ),
    ] {
        let mut bench = Bench::new("");
        assert_eq!(bench.exchange.request_inputs(0, 2), 40);
        bench.connect(0);
        bench.sent(1);
        bench.exchange.received(2, answer);
        let mut image = [0xAA, 0xBB];
        assert_eq!(
            bench.exchange.read_inputs(2, &mut image),
            transport(&format!("{refused}{reason}"))
        );
        assert_eq!(image, [0xAA, 0xBB], "{reason}");
        assert_eq!(bench.poll(2), ModbusPoll::Close, "{reason}");
    }
    let mut bench = Bench::new("");
    assert_eq!(bench.exchange.write_outputs(0, &[0x12, 0x34]), 40);
    bench.connect(0);
    bench.sent(1);
    bench.exchange.received(
        2,
        &[
            0x00, 0x01, 0x00, 0x00, 0x00, 0x06, 0x01, 0x10, 0x00, 0x01, 0x00, 0x01,
        ],
    );
    assert_eq!(
        bench.exchange.output_result(2),
        transport(&format!("{refused}the echoed address 1 is not 0"))
    );
    assert_eq!(bench.poll(2), ModbusPoll::Close);
}

/// Each function of specification 11 sends its V1.1b3 request: FC01 reads the 8 coils of a 1-byte
/// image, 01 0000 0008, and the answer's byte 8D lands as it is, coil 0 in bit 0 ("Coil payloads
/// are Modbus-packed with the first process-image bit in the least-significant bit"); FC05 writes
/// bit 0 of the image to coil 2 as FF00 (V1.1b3 §6.5); FC06 writes 12 34 to register 1; FC15
/// writes the 8 coils of 05 from coil 1, 0F 0001 0008 01 05; FC16 writes AA BB CC as the two
/// registers AABB and CC00. Catches a coil value other than FF00, the bits of a coil byte
/// reversed, and the last register of an odd image lost.
#[test]
fn each_function_sends_its_request() {
    let mut bench = Bench::new(r#""input_function":"read_coils""#);
    assert_eq!(bench.exchange.request_inputs(0, 1), 40);
    bench.connect(0);
    assert_eq!(
        bench.sent(1),
        [0x00, 0x01, 0x00, 0x00, 0x00, 0x06, 0x01, 0x01, 0x00, 0x00, 0x00, 0x08]
    );
    bench.exchange.received(
        2,
        &[0x00, 0x01, 0x00, 0x00, 0x00, 0x04, 0x01, 0x01, 0x01, 0x8D],
    );
    let mut image = [0];
    assert_eq!(bench.exchange.read_inputs(2, &mut image), Ok(()));
    assert_eq!(image, [0x8D]);
    for (fields, image, request) in [
        (
            r#""output_function":"write_single_coil","output_start":2"#,
            &[0x01][..],
            &[
                0x00, 0x01, 0x00, 0x00, 0x00, 0x06, 0x01, 0x05, 0x00, 0x02, 0xFF, 0x00,
            ][..],
        ),
        (
            r#""output_function":"write_single_register","output_start":1"#,
            &[0x12, 0x34][..],
            &[
                0x00, 0x01, 0x00, 0x00, 0x00, 0x06, 0x01, 0x06, 0x00, 0x01, 0x12, 0x34,
            ][..],
        ),
        (
            r#""output_function":"write_multiple_coils","output_start":1"#,
            &[0x05][..],
            &[
                0x00, 0x01, 0x00, 0x00, 0x00, 0x08, 0x01, 0x0F, 0x00, 0x01, 0x00, 0x08, 0x01, 0x05,
            ][..],
        ),
        (
            r#""output_function":"write_multiple_registers""#,
            &[0xAA, 0xBB, 0xCC][..],
            &[
                0x00, 0x01, 0x00, 0x00, 0x00, 0x0B, 0x01, 0x10, 0x00, 0x00, 0x00, 0x02, 0x04, 0xAA,
                0xBB, 0xCC, 0x00,
            ][..],
        ),
    ] {
        let mut bench = Bench::new(fields);
        assert_eq!(bench.exchange.write_outputs(0, image), 40);
        bench.connect(0);
        assert_eq!(bench.sent(1), request, "{fields}");
    }
}

/// "A process image larger than one request is exchanged in several requests of at most that many,
/// in address order, within the one exchange of a cycle": an input image of 300 bytes is 150 input
/// registers, read as 125 from 0 (04 0000 007D) and then 25 from 125 (04 007D 0019); the answers
/// carry 250 bytes of 01 (length 1 + 2 + 250 = 253, FD; byte count FA) and 50 of 02 (length 53,
/// 35; byte count 32), which fill bytes 0 to 249 and 250 to 299. Catches a request above the 125
/// registers of V1.1b3 §6.4, data put at the wrong image bytes, and a refresh settled after its
/// first request.
#[test]
fn a_large_raw_image_is_read_in_requests_in_address_order() {
    let mut bench = Bench::new("");
    assert_eq!(bench.exchange.request_inputs(0, 300), 40);
    bench.connect(0);
    assert_eq!(
        bench.sent(1),
        [0x00, 0x01, 0x00, 0x00, 0x00, 0x06, 0x01, 0x04, 0x00, 0x00, 0x00, 0x7D]
    );
    let mut first = vec![0x00, 0x01, 0x00, 0x00, 0x00, 0xFD, 0x01, 0x04, 0xFA];
    first.extend([0x01; 250]);
    bench.exchange.received(2, &first);
    assert!(!bench.exchange.inputs_settled());
    assert_eq!(
        bench.sent(2),
        [0x00, 0x02, 0x00, 0x00, 0x00, 0x06, 0x01, 0x04, 0x00, 0x7D, 0x00, 0x19]
    );
    let mut second = vec![0x00, 0x02, 0x00, 0x00, 0x00, 0x35, 0x01, 0x04, 0x32];
    second.extend([0x02; 50]);
    bench.exchange.received(3, &second);
    assert!(bench.exchange.inputs_settled());
    let mut image = [0; 300];
    assert_eq!(bench.exchange.read_inputs(3, &mut image), Ok(()));
    assert_eq!(image[..250], [0x01; 250]);
    assert_eq!(image[250..], [0x02; 50]);
}

/// The same for the outputs: an image of 300 bytes, 246 of 01 and 54 of 02, is 150 holding
/// registers, written as 123 from 0 with 246 bytes (10 0000 007B F6; length 1 + 252 = 253, FD; a
/// frame of 259 bytes) and then 27 from 123 with 54 bytes (10 007B 001B 36; length 1 + 60 = 61,
/// 3D; 67 bytes), and the handoff settles when both are written. Catches one request above the 123
/// registers of V1.1b3 §6.12, bytes taken from the wrong part of the image, and a handoff settled
/// after its first request.
#[test]
fn a_large_raw_image_is_written_in_requests_in_address_order() {
    let mut image = [0x01; 300];
    image[246..].fill(0x02);
    let mut bench = Bench::new("");
    assert_eq!(bench.exchange.write_outputs(0, &image), 40);
    bench.connect(0);
    let first = bench.sent(1);
    assert_eq!(first.len(), 259);
    assert_eq!(
        first[..13],
        [0x00, 0x01, 0x00, 0x00, 0x00, 0xFD, 0x01, 0x10, 0x00, 0x00, 0x00, 0x7B, 0xF6]
    );
    assert_eq!(first[13..], [0x01; 246]);
    bench.exchange.received(
        2,
        &[
            0x00, 0x01, 0x00, 0x00, 0x00, 0x06, 0x01, 0x10, 0x00, 0x00, 0x00, 0x7B,
        ],
    );
    assert!(!bench.exchange.outputs_settled());
    let second = bench.sent(2);
    assert_eq!(second.len(), 67);
    assert_eq!(
        second[..13],
        [0x00, 0x02, 0x00, 0x00, 0x00, 0x3D, 0x01, 0x10, 0x00, 0x7B, 0x00, 0x1B, 0x36]
    );
    assert_eq!(second[13..], [0x02; 54]);
    bench.exchange.received(
        3,
        &[
            0x00, 0x02, 0x00, 0x00, 0x00, 0x06, 0x01, 0x10, 0x00, 0x7B, 0x00, 0x1B,
        ],
    );
    assert!(bench.exchange.outputs_settled());
    assert_eq!(bench.exchange.output_result(3), Ok(()));
}

/// A point map reads each point with its own request, in `input_points` order, and takes a refresh
/// only when every point converted ("Reject the complete mapped read before changing any byte of
/// the caller's input image"): a `u16` 42 at holding register 0 and an `f32` 1.5 (`0x3FC0_0000`) at
/// registers 1 and 2 give 2A 00 and 00 00 C0 3F in the image; when the `f32` is a quiet NaN
/// (`0x7FC0_0000`), the whole refresh fails with `IoDriver`, the 43 of the first point does not reach
/// the image, and the connection stays, as the server answered. Catches a partial batch used as
/// input, a NaN that becomes a `REAL`, and a connection dropped for a bad value.
#[test]
fn a_mapped_read_takes_every_point_or_none() {
    let mut bench = Bench::new(concat!(
        r#""input_points":[{"image_offset":0,"address":0,"function":"holding"},"#,
        r#"{"image_offset":2,"address":1,"function":"holding","data_type":"f32"}]"#
    ));
    assert_eq!(bench.exchange.request_inputs(0, 6), 40);
    bench.connect(0);
    assert_eq!(
        bench.sent(1),
        [0x00, 0x01, 0x00, 0x00, 0x00, 0x06, 0x01, 0x03, 0x00, 0x00, 0x00, 0x01]
    );
    bench.exchange.received(
        2,
        &[
            0x00, 0x01, 0x00, 0x00, 0x00, 0x05, 0x01, 0x03, 0x02, 0x00, 0x2A,
        ],
    );
    assert_eq!(
        bench.sent(2),
        [0x00, 0x02, 0x00, 0x00, 0x00, 0x06, 0x01, 0x03, 0x00, 0x01, 0x00, 0x02]
    );
    bench.exchange.received(
        3,
        &[
            0x00, 0x02, 0x00, 0x00, 0x00, 0x07, 0x01, 0x03, 0x04, 0x3F, 0xC0, 0x00, 0x00,
        ],
    );
    let mut image = [0xA5; 6];
    assert_eq!(bench.exchange.read_inputs(3, &mut image), Ok(()));
    assert_eq!(image, [0x2A, 0x00, 0x00, 0x00, 0xC0, 0x3F]);
    assert_eq!(bench.exchange.request_inputs(10, 6), 50);
    assert_eq!(bench.sent(10)[..2], [0x00, 0x03]);
    bench.exchange.received(
        11,
        &[
            0x00, 0x03, 0x00, 0x00, 0x00, 0x05, 0x01, 0x03, 0x02, 0x00, 0x2B,
        ],
    );
    assert_eq!(bench.sent(11)[..2], [0x00, 0x04]);
    bench.exchange.received(
        12,
        &[
            0x00, 0x04, 0x00, 0x00, 0x00, 0x07, 0x01, 0x03, 0x04, 0x7F, 0xC0, 0x00, 0x00,
        ],
    );
    let mut image = [0xA5; 6];
    assert_eq!(
        bench.exchange.read_inputs(12, &mut image),
        Err(RuntimeError::IoDriver(
            "modbus mapped F32 wire value must be finite".into()
        ))
    );
    assert_eq!(image, [0xA5; 6]);
    assert_eq!(bench.exchange.request_inputs(20, 6), 60);
    assert_eq!(bench.sent(20)[..2], [0x00, 0x05]);
}

/// A point map writes each point with its own request, in `output_points` order. The computer's
/// test map: a coil at 2 from bit 1 of byte 0 (FC15: 0F 0002 0001 01 01); a `u32` 501 at byte 1
/// with scale 0.5, offset 1 and little bytes and words, `(501 − 1) / 0.5 = 1000` = `0x0000_03E8`,
/// sent E8 03 00 00 (FC16); and a `u16` 60 at byte 5 with scale 0.5, offset 10 and little bytes,
/// `(60 − 10) / 0.5 = 100` = 0x0064, sent 64 00 (FC06). Catches points written out of order, the
/// scaling inverted the wrong way, and a coil taken from another bit.
#[test]
fn mapped_outputs_are_written_in_point_order() {
    let mut bench = Bench::new(concat!(
        r#""output_points":[{"image_offset":0,"image_bit":1,"address":2,"#,
        r#""function":"write_multiple_coils"},{"image_offset":1,"address":0,"data_type":"u32","#,
        r#""scale":0.5,"offset":1.0,"byte_order":"little","word_order":"little"},"#,
        r#"{"image_offset":5,"address":3,"function":"write_single_register","scale":0.5,"#,
        r#""offset":10.0,"byte_order":"little"}]"#
    ));
    let image = [0b0000_0010, 0xF5, 0x01, 0x00, 0x00, 0x3C, 0x00];
    assert_eq!(bench.exchange.write_outputs(0, &image), 40);
    bench.connect(0);
    assert_eq!(
        bench.sent(1),
        [0x00, 0x01, 0x00, 0x00, 0x00, 0x08, 0x01, 0x0F, 0x00, 0x02, 0x00, 0x01, 0x01, 0x01]
    );
    bench.exchange.received(
        2,
        &[
            0x00, 0x01, 0x00, 0x00, 0x00, 0x06, 0x01, 0x0F, 0x00, 0x02, 0x00, 0x01,
        ],
    );
    assert_eq!(
        bench.sent(2),
        [
            0x00, 0x02, 0x00, 0x00, 0x00, 0x0B, 0x01, 0x10, 0x00, 0x00, 0x00, 0x02, 0x04, 0xE8,
            0x03, 0x00, 0x00,
        ]
    );
    bench.exchange.received(
        3,
        &[
            0x00, 0x02, 0x00, 0x00, 0x00, 0x06, 0x01, 0x10, 0x00, 0x00, 0x00, 0x02,
        ],
    );
    let single = [
        0x00, 0x03, 0x00, 0x00, 0x00, 0x06, 0x01, 0x06, 0x00, 0x03, 0x64, 0x00,
    ];
    assert_eq!(bench.sent(3), single);
    bench.exchange.received(4, &single);
    assert!(bench.exchange.outputs_settled());
    assert_eq!(bench.exchange.output_result(4), Ok(()));
}

/// "A finite f64 result that becomes non-finite at f32 width is rejected before process-image
/// mutation or Modbus wire encoding": `f32::MAX` with scale 0.5 is `f32::MAX / 0.5`, about 6.8e38,
/// beyond an `f32`; the handoff settles at once with `IoDriver`, its deadline is its own time, and
/// no connection or request is made. Catches an infinite value sent as a register payload, and a
/// connection made for a handoff that cannot be written.
#[test]
fn an_output_that_does_not_convert_reaches_no_wire() {
    let mut bench = Bench::new(
        r#""output_points":[{"image_offset":0,"address":0,"data_type":"f32","scale":0.5}]"#,
    );
    assert_eq!(bench.exchange.write_outputs(0, &f32::MAX.to_le_bytes()), 0);
    assert_eq!(bench.poll(0), ModbusPoll::Wait(None));
    let narrowing = "modbus mapped F32 wire value must remain finite after narrowing";
    assert_eq!(
        bench.exchange.output_result(0),
        Err(RuntimeError::IoDriver(narrowing.into()))
    );
    assert_eq!(bench.exchange.output_health(), faulted(narrowing));
}
