//! The line framing of a control link over a byte stream (specification 34, PORT-05): a line
//! feed ends a request and one carriage return before it is removed, an empty line is a request
//! of zero bytes, a request longer than the frame is dropped to its line feed and answered once in
//! its place, at every split position of the bytes and across the end of the receive ring, and
//! the chips' USB links move frames of 8,192 bytes.

use alloc::format;
use alloc::vec;
use alloc::vec::Vec;
use core::time::Duration;

use trust_runtime_core::ports::{LinkFrame, MonotonicInstant, PeerClass};

use super::{
    ByteRing, DeviceLineLink, Line, LineAssembler, LineLink, ReplyBuffer, DEVICE_FRAME_LEN,
    DEVICE_LINK_BUFFER_LEN,
};
use crate::dispatch::{prepare_line, prepare_line_within};
use crate::tests::fake_host::FakeHost;
use crate::tests::rng::Rng;

/// What the loop took from the link: a frame's bytes, or an oversized request.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Got {
    Frame(Vec<u8>),
    Oversized,
}

fn frame(bytes: &[u8]) -> Got {
    Got::Frame(bytes.to_vec())
}

/// A link for the tables: frames of at most [`SMALL_FRAME`] bytes.
type SmallLink = LineLink<18>;
const SMALL_FRAME: usize = 16;

fn at(ms: u64) -> MonotonicInstant {
    MonotonicInstant::from_origin(Duration::from_millis(ms))
}

/// Hands each of `chunks` to the link as a receive interrupt would, giving back to the hardware
/// what the link did not take, and takes every request that has arrived after each step.
fn assemble<const N: usize>(link: &mut LineLink<N>, chunks: &[&[u8]]) -> Vec<Got> {
    let mut buffer = vec![0; LineLink::<N>::MAX_FRAME_LEN];
    let mut got = Vec::new();
    for chunk in chunks {
        let mut rest: &[u8] = chunk;
        let mut idle_steps = 0;
        while !rest.is_empty() {
            let capacity = link.receive_capacity();
            let taken = link.receive(rest);
            assert!(
                taken >= capacity.min(rest.len()),
                "the link takes at least what it said it would"
            );
            rest = &rest[taken..];
            let before = got.len();
            take_requests(link, &mut buffer, &mut got);
            idle_steps = if taken == 0 && got.len() == before {
                idle_steps + 1
            } else {
                0
            };
            assert!(
                idle_steps < 2,
                "a link without a reply in flight always takes more bytes"
            );
        }
        take_requests(link, &mut buffer, &mut got);
    }
    got
}

/// Takes every request that has arrived, as the loop does in its passes.
fn take_requests<const N: usize>(link: &mut LineLink<N>, buffer: &mut [u8], got: &mut Vec<Got>) {
    while let Some(polled) = link.poll_frame(buffer, at(0)) {
        got.push(match polled {
            LinkFrame::Frame { len, peer } => {
                assert_eq!(peer, PeerClass::Local);
                frame(&buffer[..len])
            }
            LinkFrame::Oversized { peer } => {
                assert_eq!(peer, PeerClass::Local);
                Got::Oversized
            }
        });
    }
}

/// The requests in `input` by the rules of PORT-05, read straight from the whole input: each
/// line feed ends one, one carriage return before it is removed, and a frame longer than
/// `max_frame` is oversized. Bytes after the last line feed are no request yet.
fn reference(input: &[u8], max_frame: usize) -> Vec<Got> {
    let mut got = Vec::new();
    let mut rest = input;
    while let Some(end) = rest.iter().position(|&byte| byte == b'\n') {
        let line = &rest[..end];
        let line = line.strip_suffix(b"\r").unwrap_or(line);
        got.push(if line.len() > max_frame {
            Got::Oversized
        } else {
            frame(line)
        });
        rest = &rest[end + 1..];
    }
    got
}

fn small_link() -> SmallLink {
    SmallLink::new(PeerClass::Local)
}

#[test]
fn a_line_feed_ends_a_request_and_one_carriage_return_before_it_is_removed() {
    let mut link = small_link();
    let got = assemble(&mut link, &[b"a\nbc\r\nd\r\r\ne\rf\n\r\n\n"]);
    assert_eq!(
        got,
        vec![
            frame(b"a"),
            frame(b"bc"),
            frame(b"d\r"),
            frame(b"e\rf"),
            frame(b""),
            frame(b""),
        ],
        "an empty line, with or without its carriage return, is a request of zero bytes"
    );
    assert_eq!(link.counters().requests, 6);
}

#[test]
fn an_empty_line_gets_the_answer_trust_runtime_gives_an_empty_line() {
    let mut link = small_link();
    let mut buffer = [0_u8; SMALL_FRAME];
    assert_eq!(link.receive(b"\r\n"), 2);
    let Some(LinkFrame::Frame { len: 0, .. }) = link.poll_frame(&mut buffer, at(0)) else {
        panic!("an empty line is a request of zero bytes");
    };
    let request = core::str::from_utf8(&buffer[..0]).expect("no bytes are text");
    let host = FakeHost::running();
    let answer = prepare_line_within(request, &host, None, SMALL_FRAME * 8).response;
    assert_eq!(
        answer,
        r#"{"id":0,"ok":false,"error":"invalid request: EOF while parsing a value at line 1 column 0"}"#
    );
    assert_eq!(
        answer,
        prepare_line("", &host, None).response,
        "the same answer as trust-runtime's socket gives"
    );
}

#[test]
fn bytes_after_the_last_line_feed_wait_for_their_line_end() {
    let mut link = small_link();
    assert_eq!(assemble(&mut link, &[b"{\"id\""]), Vec::<Got>::new());
    assert_eq!(assemble(&mut link, &[b":1}\r"]), Vec::<Got>::new());
    assert_eq!(
        assemble(&mut link, &[b"\n"]),
        vec![frame(b"{\"id\":1}")],
        "a carriage return at the end of one chunk and its line feed in the next are one line end"
    );
}

#[test]
fn every_split_position_gives_the_requests_of_the_whole_input() {
    let long = [b'x'; 40];
    let mut inputs: Vec<Vec<u8>> = vec![
        b"status\n".to_vec(),
        b"a\r\n\r\n\nb\r\r\n".to_vec(),
        [&[b'f'; SMALL_FRAME][..], &b"\n"[..]].concat(),
        [&[b'f'; SMALL_FRAME][..], &b"\r\n"[..]].concat(),
        [&[b'o'; SMALL_FRAME + 1][..], &b"\n"[..]].concat(),
        [&[b'o'; SMALL_FRAME + 1][..], &b"\r\n"[..]].concat(),
        [&[b'o'; SMALL_FRAME][..], &b"\r\r\n"[..]].concat(),
        [&long[..], &b"\nnext\n"[..]].concat(),
        [&long[..], &b"\r\n"[..], &long[..], &b"\nlast\r\n"[..]].concat(),
    ];
    inputs.push(inputs.concat());
    for input in &inputs {
        let expected = reference(input, SMALL_FRAME);
        let mut whole = small_link();
        assert_eq!(assemble(&mut whole, &[input]), expected, "input={input:?}");
        for split in 0..=input.len() {
            let mut link = small_link();
            let (first, second) = input.split_at(split);
            assert_eq!(
                assemble(&mut link, &[first, second]),
                expected,
                "input={input:?} split={split}"
            );
        }
        let mut bytewise = small_link();
        let bytes: Vec<&[u8]> = input.chunks(1).collect();
        assert_eq!(assemble(&mut bytewise, &bytes), expected, "input={input:?}");
    }
}

#[test]
fn random_chunks_of_random_lines_give_the_requests_of_the_whole_input() {
    let mut rng = Rng::new(0x5EED_0001);
    let alphabet = [b'a', b'{', b'"', b'\r', 0xff];
    for case in 0..300 {
        let mut input = Vec::new();
        for _ in 0..rng.between(1, 12) {
            let len = rng.between(0, SMALL_FRAME + 6);
            input.extend((0..len).map(|_| *rng.pick(&alphabet)));
            if rng.one_in(2) {
                input.push(b'\r');
            }
            input.push(b'\n');
        }
        let mut chunks = Vec::new();
        let mut rest = input.as_slice();
        while !rest.is_empty() {
            let (chunk, tail) = rest.split_at(rng.between(1, 24).min(rest.len()));
            chunks.push(chunk);
            rest = tail;
        }
        let mut link = small_link();
        assert_eq!(
            assemble(&mut link, &chunks),
            reference(&input, SMALL_FRAME),
            "case={case} input={input:?}"
        );
    }
}

#[test]
fn a_request_longer_than_the_frame_is_dropped_to_its_line_feed_and_answered_once_in_its_place() {
    let mut link = small_link();
    let input = [
        &b"first\n"[..],
        &[b'x'; 50][..],
        &b"\r\nsecond\n"[..],
        &[b'y'; 30][..],
        &b"\n"[..],
        &[b'z'; 19][..],
        &b"\nthird\n"[..],
    ]
    .concat();
    assert_eq!(
        assemble(&mut link, &[&input]),
        vec![
            frame(b"first"),
            Got::Oversized,
            frame(b"second"),
            Got::Oversized,
            Got::Oversized,
            frame(b"third"),
        ]
    );
    assert_eq!(link.counters().oversized_requests, 3);
    assert_eq!(link.counters().requests, 3);
}

#[test]
fn an_oversized_request_that_arrives_in_many_parts_is_answered_once() {
    let mut link = small_link();
    let mut got = Vec::new();
    for _ in 0..10 {
        got.extend(assemble(&mut link, &[b"xxxxxxxxxx"]));
    }
    assert_eq!(
        got,
        Vec::<Got>::new(),
        "no answer before the line has ended"
    );
    assert_eq!(
        assemble(&mut link, &[b"xx\nok\n"]),
        vec![Got::Oversized, frame(b"ok")]
    );
}

#[test]
fn the_longest_frame_fits_with_and_without_a_carriage_return_and_one_byte_more_does_not() {
    let longest = [b'f'; DEVICE_FRAME_LEN];
    let over = [b'o'; DEVICE_FRAME_LEN + 1];
    for (sent, expected) in [
        ([&longest[..], &b"\n"[..]].concat(), frame(&longest)),
        ([&longest[..], &b"\r\n"[..]].concat(), frame(&longest)),
        ([&over[..], &b"\n"[..]].concat(), Got::Oversized),
        ([&over[..], &b"\r\n"[..]].concat(), Got::Oversized),
        ([&longest[..], &b"\r\r\n"[..]].concat(), Got::Oversized),
    ] {
        let mut link = DeviceLineLink::new(PeerClass::Local);
        let input = [&sent[..], &b"after\n"[..]].concat();
        assert_eq!(
            assemble(&mut link, &[&input]),
            vec![expected.clone(), frame(b"after")],
            "line of {} bytes",
            sent.len()
        );
        let mut split = DeviceLineLink::new(PeerClass::Local);
        let (first, second) = input.split_at(sent.len() - 1);
        assert_eq!(
            assemble(&mut split, &[first, second]),
            vec![expected, frame(b"after")],
            "line of {} bytes with its line end in the next chunk",
            sent.len()
        );
    }
}

#[test]
fn requests_that_wrap_around_the_end_of_the_receive_ring_are_whole() {
    let mut link = small_link();
    let mut expected = Vec::new();
    let mut got = Vec::new();
    for index in 0..40_usize {
        let request = format!("r{index}-{}", "w".repeat(index % 11));
        expected.push(frame(request.as_bytes()));
        let bytes = format!("{request}\r\n");
        let (first, second) = bytes.as_bytes().split_at(index % bytes.len());
        got.extend(assemble(&mut link, &[first, second]));
    }
    assert_eq!(got, expected);
}

#[test]
fn a_buffer_shorter_than_the_frame_reports_a_longer_request_as_oversized() {
    let mut assembler = LineAssembler::<18>::new();
    let mut buffer = [0_u8; 4];
    assert_eq!(assembler.receive(b"abcd\nabcde\n"), 11);
    assert_eq!(assembler.next_line(&mut buffer), Some(Line::Frame(4)));
    assert_eq!(&buffer, b"abcd");
    assert_eq!(assembler.next_line(&mut buffer), Some(Line::Oversized));
    assert_eq!(assembler.next_line(&mut buffer), None);
}

#[test]
fn a_new_session_drops_a_partial_request_and_the_unreported_oversized_ones() {
    let mut link = small_link();
    assert_eq!(assemble(&mut link, &[b"part of a req"]), Vec::<Got>::new());
    link.reset_session();
    assert_eq!(assemble(&mut link, &[b"new\n"]), vec![frame(b"new")]);

    let mut assembler = LineAssembler::<18>::new();
    let mut buffer = [0_u8; 16];
    assert_eq!(assembler.receive(&[b'x'; 18]), 18);
    assert_eq!(assembler.next_line(&mut buffer), None);
    assert_eq!(
        assembler.receive(b"xx\n"),
        3,
        "the rest of the oversized line is dropped"
    );
    assembler.reset();
    assert_eq!(assembler.receive(b"ok\n"), 3);
    assert_eq!(
        assembler.next_line(&mut buffer),
        Some(Line::Frame(2)),
        "the oversized line of the old session is not reported"
    );
    assert_eq!(link.counters().sessions, 1);
}

#[test]
fn the_byte_ring_keeps_its_bytes_in_order_across_the_end_of_its_storage() {
    let mut ring = ByteRing::<8>::new();
    assert_eq!((ring.len(), ring.free(), ring.is_empty()), (0, 8, true));
    assert_eq!(ring.push(b"abcdef"), 6);
    ring.discard(5);
    assert_eq!(ring.push(b"ghijklmno"), 7, "only what fits is taken");
    assert!(ring.is_full());
    assert_eq!(ring.push(b"p"), 0);
    assert_eq!(ring.as_slices(), (&b"fgh"[..], &b"ijklm"[..]));
    assert_eq!(ring.get(0), Some(b'f'));
    assert_eq!(ring.get(7), Some(b'm'));
    assert_eq!(ring.get(8), None);
    assert_eq!(ring.find(b'j', 0), Some(4));
    assert_eq!(ring.find(b'g', 2), None, "the search starts at its offset");
    assert_eq!(ring.find(b'm', 7), Some(7));
    let mut out = [0_u8; 6];
    assert_eq!(ring.copy_front(&mut out), 6);
    assert_eq!(&out, b"fghijk");
    ring.discard(100);
    assert_eq!((ring.len(), ring.free()), (0, 8));
    assert_eq!(ring.push(b"qr"), 2);
    let mut wide = [0_u8; 4];
    assert_eq!(ring.copy_front(&mut wide), 2, "no more than the ring holds");
    ring.clear();
    assert_eq!(ring.as_slices(), (&b""[..], &b""[..]));
    assert_eq!(ByteRing::<8>::default().len(), 0);
}

#[test]
fn the_chips_usb_link_moves_frames_of_8192_bytes_in_both_directions() {
    assert_eq!(DEVICE_FRAME_LEN, 8_192);
    assert_eq!(DEVICE_LINK_BUFFER_LEN, DEVICE_FRAME_LEN + 2);
    assert_eq!(DeviceLineLink::MAX_FRAME_LEN, DEVICE_FRAME_LEN);
    assert_eq!(
        LineAssembler::<DEVICE_LINK_BUFFER_LEN>::MAX_FRAME_LEN,
        DEVICE_FRAME_LEN
    );
    assert_eq!(
        ReplyBuffer::<DEVICE_LINK_BUFFER_LEN>::MAX_FRAME_LEN,
        DEVICE_FRAME_LEN
    );
    let size = core::mem::size_of::<DeviceLineLink>();
    assert!(
        (2 * DEVICE_LINK_BUFFER_LEN..17 * 1024).contains(&size),
        "the receive ring, the reply buffer and a few words: {size} bytes"
    );
}
