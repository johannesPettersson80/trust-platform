//! One reply in flight, the stall rule with its line-feed repair, a new host session, and the
//! back-pressure of a full receive ring (specification 34, PORT-05), on a test clock; and whole
//! USB sessions in which a host writes and reads at random rates.

use alloc::collections::VecDeque;
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use core::time::Duration;

use trust_runtime_core::ports::{LinkFrame, MonotonicInstant, PeerClass, TimeSpan};

use super::{LineLink, LinkCounters, ReplyBuffer, ReplyRefused, REPLY_STALL_LIMIT};
use crate::tests::rng::Rng;

/// A link for the tests: frames of at most [`SMALL_FRAME`] bytes.
type SmallLink = LineLink<18>;
const SMALL_FRAME: usize = 16;

fn at(ms: u64) -> MonotonicInstant {
    MonotonicInstant::from_origin(Duration::from_millis(ms))
}

fn small_link() -> SmallLink {
    SmallLink::new(PeerClass::Local)
}

/// Takes the next request, which must have arrived.
fn take(link: &mut SmallLink, now: MonotonicInstant) -> Vec<u8> {
    let mut buffer = [0_u8; SMALL_FRAME];
    match link.poll_frame(&mut buffer, now) {
        Some(LinkFrame::Frame { len, .. }) => buffer[..len].to_vec(),
        other => panic!("expected a request, got {other:?}"),
    }
}

/// Moves up to `count` bytes of the reply in flight to the hardware, as a transmit interrupt
/// does, and returns them.
fn transmit(link: &mut SmallLink, count: usize) -> Vec<u8> {
    let moved: Vec<u8> = link.unsent().iter().take(count).copied().collect();
    link.mark_sent(moved.len());
    moved
}

#[test]
fn a_reply_is_sent_with_its_line_feed_and_counted_once_the_hardware_took_it_whole() {
    let mut link = small_link();
    assert_eq!(link.receive(b"status\n"), 7);
    assert_eq!(take(&mut link, at(0)), b"status");
    assert_eq!(link.send_frame(br#"{"id":1}"#, at(0)), Ok(()));
    assert!(link.reply_in_flight());
    assert_eq!(link.unsent(), b"{\"id\":1}\n");
    assert_eq!(transmit(&mut link, 3), b"{\"i");
    assert_eq!(
        link.counters().replies,
        0,
        "a reply counts once it is whole"
    );
    assert_eq!(transmit(&mut link, 64), b"d\":1}\n");
    assert!(!link.reply_in_flight());
    assert_eq!(link.unsent(), b"");
    assert_eq!(link.counters().replies, 1);
}

#[test]
fn while_a_reply_is_in_flight_the_link_hands_out_no_request() {
    let mut link = small_link();
    let mut buffer = [0_u8; SMALL_FRAME];
    assert_eq!(link.receive(b"one\ntwo\n"), 8);
    assert_eq!(take(&mut link, at(0)), b"one");
    link.send_frame(b"r1", at(0))
        .expect("no reply is in flight yet");
    assert_eq!(
        link.poll_frame(&mut buffer, at(1)),
        None,
        "the second request waits for the reply"
    );
    assert_eq!(transmit(&mut link, 2), b"r1");
    assert_eq!(
        link.poll_frame(&mut buffer, at(2)),
        None,
        "the reply's line feed is not sent yet"
    );
    assert_eq!(transmit(&mut link, 1), b"\n");
    assert_eq!(take(&mut link, at(3)), b"two");
}

#[test]
fn a_reply_longer_than_the_frame_or_sent_during_another_is_refused_and_counted() {
    let mut link = small_link();
    assert_eq!(
        link.send_frame(&[b'r'; SMALL_FRAME + 1], at(0)),
        Err(ReplyRefused::TooLong)
    );
    assert!(!link.reply_in_flight(), "a refused reply leaves nothing");
    assert_eq!(link.send_frame(&[b'r'; SMALL_FRAME], at(0)), Ok(()));
    assert_eq!(link.send_frame(b"second", at(0)), Err(ReplyRefused::Busy));
    assert_eq!(
        link.unsent().len(),
        SMALL_FRAME + 1,
        "the reply in flight is unchanged"
    );
    assert_eq!(link.counters().refused_replies, 2);
}

#[test]
fn a_reply_that_keeps_making_progress_is_never_dropped() {
    let mut link = small_link();
    let mut buffer = [0_u8; SMALL_FRAME];
    link.send_frame(&[b'r'; SMALL_FRAME], at(0))
        .expect("no reply is in flight yet");
    let mut now = 0;
    let mut received = Vec::new();
    while link.reply_in_flight() {
        now += 900;
        received.extend(transmit(&mut link, 1));
        assert_eq!(link.poll_frame(&mut buffer, at(now)), None);
    }
    assert_eq!(received, [&[b'r'; SMALL_FRAME][..], &b"\n"[..]].concat());
    assert_eq!(
        now,
        17 * 900,
        "17 bytes, one every 0.9 seconds: more than 15 seconds in all"
    );
    assert_eq!(link.counters().stalled_replies, 0);
    assert_eq!(link.counters().replies, 1);
}

#[test]
fn a_reply_without_progress_for_one_second_is_dropped_and_the_link_takes_requests_again() {
    assert_eq!(REPLY_STALL_LIMIT, TimeSpan::new(Duration::from_secs(1)));
    let mut link = small_link();
    let mut buffer = [0_u8; SMALL_FRAME];
    assert_eq!(link.receive(b"a\nb\n"), 4);
    assert_eq!(take(&mut link, at(0)), b"a");
    link.send_frame(b"reply a", at(0))
        .expect("no reply is in flight yet");
    assert_eq!(link.poll_frame(&mut buffer, at(999)), None);
    assert!(
        link.reply_in_flight(),
        "999 ms without progress: still in flight"
    );
    assert_eq!(
        take(&mut link, at(1_000)),
        b"b",
        "after one second the reply is dropped and the next request is taken"
    );
    assert_eq!(link.counters().stalled_replies, 1);
    link.send_frame(b"reply b", at(1_000))
        .expect("the stalled reply is gone");
    assert_eq!(
        link.unsent(),
        b"reply b\n",
        "no byte of the dropped reply reached the hardware, so nothing needs repair"
    );
}

#[test]
fn after_a_reply_dropped_part_way_the_next_reply_starts_with_a_line_feed() {
    let mut link = small_link();
    let mut buffer = [0_u8; SMALL_FRAME];
    link.send_frame(b"abcdefgh", at(0))
        .expect("no reply is in flight yet");
    let mut host = transmit(&mut link, 3);
    assert_eq!(
        link.poll_frame(&mut buffer, at(600)),
        None,
        "this poll sees the progress"
    );
    assert_eq!(link.poll_frame(&mut buffer, at(1_599)), None);
    assert!(link.reply_in_flight());
    assert_eq!(link.poll_frame(&mut buffer, at(1_600)), None);
    assert!(
        !link.reply_in_flight(),
        "dropped one second after the poll that last saw progress"
    );
    link.send_frame(b"xyz", at(1_600))
        .expect("the stalled reply is gone");
    host.extend(transmit(&mut link, 64));
    assert_eq!(
        host, b"abc\nxyz\n",
        "the stale fragment ends as a line of its own and the next reply is whole"
    );
}

#[test]
fn a_repair_line_feed_that_was_never_sent_carries_over_to_the_next_reply() {
    let mut reply = ReplyBuffer::<18>::new();
    reply.start(b"first", at(0)).expect("nothing in flight");
    assert!(!reply.mark_sent(2));
    assert!(!reply.poll(at(500)), "this poll sees the progress");
    assert!(reply.poll(at(1_500)), "no progress for one second");
    assert!(reply.repair_due());
    reply
        .start(b"second", at(1_500))
        .expect("nothing in flight");
    assert_eq!(reply.unsent(), b"\nsecond\n");
    assert!(reply.poll(at(2_500)), "dropped before its first byte");
    assert!(
        reply.repair_due(),
        "the repair line feed was never sent, so the next reply still needs it"
    );
    reply.start(b"third", at(2_500)).expect("nothing in flight");
    assert_eq!(reply.unsent(), b"\nthird\n");
    assert!(reply.mark_sent(7), "the reply is whole");
    assert!(!reply.repair_due());
    reply
        .start(b"fourth", at(3_000))
        .expect("nothing in flight");
    assert_eq!(reply.unsent(), b"fourth\n");
    assert!(reply.abandon());
    assert!(
        !reply.repair_due(),
        "a reply dropped before its first byte leaves nothing stale"
    );
    assert!(!reply.abandon(), "nothing is in flight");
    assert!(!reply.mark_sent(1), "nothing is in flight");
    assert!(!reply.poll(at(9_000)), "nothing is in flight");
    assert_eq!(ReplyBuffer::<18>::default().unsent(), b"");
}

#[test]
fn a_new_host_session_drops_the_reply_in_flight_and_counts_it() {
    let mut link = small_link();
    link.send_frame(b"old reply", at(0))
        .expect("no reply is in flight yet");
    assert_eq!(transmit(&mut link, 2), b"ol");
    link.reset_session();
    assert!(!link.reply_in_flight());
    assert_eq!(
        (link.counters().sessions, link.counters().abandoned_replies),
        (1, 1)
    );
    link.reset_session();
    assert_eq!(
        (link.counters().sessions, link.counters().abandoned_replies),
        (2, 1),
        "a session without a reply in flight drops none"
    );
    link.send_frame(b"new", at(10))
        .expect("the old reply is gone");
    assert_eq!(
        link.unsent(),
        b"\nnew\n",
        "part of the old reply may still be in the hardware"
    );
}

#[test]
fn a_full_receive_ring_leaves_bytes_with_the_hardware_and_loses_none() {
    let mut link = small_link();
    let mut buffer = [0_u8; SMALL_FRAME];
    let requests: Vec<String> = (0..12).map(|index| format!("request {index}")).collect();
    let wire: Vec<u8> = requests
        .iter()
        .flat_map(|request| [request.as_bytes(), &b"\r\n"[..]].concat())
        .collect();
    let mut pending = wire.as_slice();
    let mut received = Vec::new();
    let mut now = 0;
    while received.len() < requests.len() {
        now += 1;
        assert!(now < 10_000, "every request arrives");
        let offered = &pending[..pending.len().min(7)];
        let capacity = link.receive_capacity();
        let taken = link.receive(offered);
        assert_eq!(
            taken,
            capacity.min(offered.len()),
            "the link takes what fits"
        );
        pending = &pending[taken..];
        if let Some(LinkFrame::Frame { len, .. }) = link.poll_frame(&mut buffer, at(now)) {
            received.push(String::from_utf8(buffer[..len].to_vec()).expect("text requests"));
            link.send_frame(b"ok", at(now))
                .expect("one reply at a time");
        }
        transmit(&mut link, 1);
    }
    assert_eq!(received, requests);
    assert!(
        link.counters().receive_full > 0,
        "the ring was full and the hardware kept the rest"
    );
}

#[test]
fn a_line_too_long_to_hold_is_dropped_while_a_reply_is_in_flight() {
    let mut link = small_link();
    let mut buffer = [0_u8; SMALL_FRAME];
    link.send_frame(b"busy", at(0))
        .expect("no reply is in flight yet");
    assert_eq!(link.receive(&[b'x'; 30]), 18, "the ring takes what fits");
    assert_eq!(link.poll_frame(&mut buffer, at(1)), None);
    assert_eq!(
        link.receive_capacity(),
        18,
        "the overlong line is dropped although no request is taken"
    );
    assert_eq!(link.receive(&[b'x'; 12]), 12);
    assert_eq!(link.receive(b"\nok\n"), 4);
    assert_eq!(
        link.poll_frame(&mut buffer, at(2)),
        None,
        "still one reply in flight"
    );
    assert_eq!(transmit(&mut link, 64), b"busy\n");
    assert_eq!(
        link.poll_frame(&mut buffer, at(3)),
        Some(LinkFrame::Oversized {
            peer: PeerClass::Local
        })
    );
    assert_eq!(take(&mut link, at(3)), b"ok");
}

/// What a host saw in one simulated USB session.
struct Session {
    /// The reply each request should get, in order: `<number>:<frame length>`, or
    /// `<number>:big` for a request longer than the frame.
    expected: Vec<String>,
    /// Every byte the host read.
    read: Vec<u8>,
    counters: LinkCounters,
}

/// One USB session on a test clock of one millisecond per step. The host writes `count` request
/// lines in chunks of random size and reads the replies through a FIFO of eight bytes; with
/// `pauses`, it stops reading for 1.5 seconds after half a second, and again at random. The
/// device's interrupts move bytes between the hardware and the link, and its loop answers each
/// request with its number and the length of its frame.
fn run_session(seed: u64, count: usize, pauses: bool) -> Session {
    const FIFO: usize = 8;
    let mut rng = Rng::new(seed);
    let mut link = small_link();
    let mut written = Vec::new();
    let mut expected = Vec::new();
    for number in 0..count {
        let len = rng.between(0, SMALL_FRAME + 8);
        written.extend((0..len).map(|_| *rng.pick(b"a{\"")));
        if rng.one_in(2) {
            written.push(b'\r');
        }
        written.push(b'\n');
        expected.push(if len <= SMALL_FRAME {
            format!("{number}:{len}")
        } else {
            format!("{number}:big")
        });
    }
    let mut sent = 0;
    let mut answered = 0;
    let mut fifo = VecDeque::new();
    let mut read = Vec::new();
    let mut buffer = [0_u8; SMALL_FRAME];
    let mut reading_from = 0;
    let mut now = 0_u64;
    while answered < count || link.reply_in_flight() || !fifo.is_empty() {
        now += 1;
        assert!(now < 2_000_000, "the session ends");
        let chunk = rng.between(0, 12).min(written.len() - sent);
        sent += link.receive(&written[sent..sent + chunk]);
        while let Some(polled) = link.poll_frame(&mut buffer, at(now)) {
            let reply = match polled {
                LinkFrame::Frame { len, .. } => format!("{answered}:{len}"),
                LinkFrame::Oversized { .. } => format!("{answered}:big"),
            };
            answered += 1;
            link.send_frame(reply.as_bytes(), at(now))
                .expect("one reply at a time, within the frame");
        }
        let room = FIFO - fifo.len();
        let moved = link
            .unsent()
            .iter()
            .take(room.min(rng.between(0, 4)))
            .copied()
            .collect::<Vec<u8>>();
        link.mark_sent(moved.len());
        fifo.extend(moved);
        if pauses && now >= reading_from && (now == 500 || rng.one_in(3_000)) {
            reading_from = now + 1_500;
        }
        if now >= reading_from {
            for _ in 0..rng.between(0, FIFO) {
                if let Some(byte) = fifo.pop_front() {
                    read.push(byte);
                }
            }
        }
    }
    Session {
        expected,
        read,
        counters: link.counters(),
    }
}

#[test]
fn a_usb_session_answers_every_request_once_and_in_order() {
    for seed in 1..=4 {
        let session = run_session(seed, 300, false);
        let whole: Vec<u8> = session
            .expected
            .iter()
            .flat_map(|reply| [reply.as_bytes(), &b"\n"[..]].concat())
            .collect();
        assert_eq!(
            String::from_utf8_lossy(&session.read),
            String::from_utf8_lossy(&whole),
            "seed={seed}"
        );
        let counters = session.counters;
        assert_eq!(
            counters.requests + counters.oversized_requests,
            300,
            "seed={seed}"
        );
        assert_eq!(
            (counters.replies, counters.stalled_replies),
            (300, 0),
            "seed={seed}"
        );
    }
}

#[test]
fn a_host_that_stops_reading_loses_only_stalled_replies_and_never_reads_a_mixed_line() {
    let mut stalled = 0;
    for seed in 11..=14 {
        let session = run_session(seed, 300, true);
        let counters = session.counters;
        assert_eq!(
            counters.replies + counters.stalled_replies,
            300,
            "seed={seed}: every request was answered once"
        );
        stalled += counters.stalled_replies;
        let mut next = 0;
        let mut whole = 0;
        // The last line has no line feed when the last reply was dropped part-way.
        for line in session.read.split(|&byte| byte == b'\n') {
            if line.is_empty() {
                continue;
            }
            let Some(offset) = session.expected[next..]
                .iter()
                .position(|reply| reply.as_bytes().starts_with(line))
            else {
                panic!(
                    "seed={seed}: {:?} is neither a reply nor the start of one after reply {next}",
                    String::from_utf8_lossy(line)
                );
            };
            if session.expected[next + offset].as_bytes() == line {
                whole += 1;
            }
            next += offset + 1;
        }
        assert!(
            whole >= counters.replies,
            "seed={seed}: every reply that was not dropped arrived whole"
        );
    }
    assert!(stalled > 0, "the host's pauses dropped replies");
}
