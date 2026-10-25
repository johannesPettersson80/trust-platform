//! The resource loop's control link (specification 34, §7, ARCH-12 and STATE-08): the requests
//! only the platform answers, the access rights of the link's peers, the frames that cannot be
//! request lines, the replies that do not fit a frame and the bound on the requests of one pass,
//! with the board probe program on the compiled cycle.

use alloc::string::ToString;
use alloc::vec;
use core::net::{IpAddr, Ipv4Addr, SocketAddr};
use core::time::Duration;

use serde_json::{json, Value};
use trust_runtime_core::ports::{ControlLink, LinkFrame, PeerClass, PortError};

use super::FRAMES_PER_PASS;
use crate::tests::runner_ports::{ms, Fixture, TestClock, TestLink, TestPlatform, WaitCall};

fn request(id: u64, kind: &str) -> Value {
    json!({"id": id, "type": kind})
}

#[test]
fn requests_the_device_does_not_serve_answer_unsupported_until_the_platform_serves_them() {
    let fixture = Fixture::board_probe();
    let mut runner = fixture.runner();
    fixture.pass_at(&mut runner, 0);
    let answered = fixture.call(&mut runner, &request(1, "config.get"));
    assert_eq!(answered["result"], json!({"answered_by": "platform"}));
    let unsupported = fixture.call(&mut runner, &request(2, "bytecode.reload"));
    assert_eq!(
        unsupported,
        json!({"id": 2, "ok": false, "error": "unsupported request", "audit_id": "audit-2"})
    );
}

#[test]
fn the_access_rights_follow_the_peer_of_the_link() {
    let network = PeerClass::Network(SocketAddr::new(
        IpAddr::V4(Ipv4Addr::new(192, 0, 2, 1)),
        9000,
    ));
    let fixture = Fixture::board_probe();
    let mut runner = fixture.runner();
    fixture.pass_at(&mut runner, 0);
    let local = fixture.call(&mut runner, &request(1, "status"));
    assert_eq!(
        local["result"]["access"]["role"],
        json!("admin"),
        "a client on USB counts as local, as trust-runtime's local socket"
    );
    fixture
        .link
        .send_bytes(request(2, "status").to_string().as_bytes(), network);
    runner.run_pass().expect("the test wait does not fail");
    let remote = fixture.link.take_replies().remove(0);
    assert_eq!(remote["result"]["access"]["role"], json!("viewer"));

    let fixture = Fixture::board_probe();
    let mut runner = fixture.runner_with(TestPlatform {
        control_token: Some("secret"),
        ..TestPlatform::default()
    });
    fixture.pass_at(&mut runner, 0);
    let refused = fixture.call(&mut runner, &request(3, "status"));
    assert_eq!(refused["error_code"], json!("missing_auth_token"));
    let accepted = fixture.call(
        &mut runner,
        &json!({"id": 4, "type": "status", "auth": "secret"}),
    );
    assert_eq!(accepted["result"]["access"]["role"], json!("admin"));
}

#[test]
fn frames_that_cannot_be_request_lines_get_invalid_request_without_an_audit_record() {
    let fixture = Fixture::with_frames_of(256);
    let mut runner = fixture.runner();
    fixture.pass_at(&mut runner, 0);
    fixture.link.send_bytes(&[b' '; 257], PeerClass::Local);
    fixture
        .link
        .send_bytes(&[0xff, b'{', b'}'], PeerClass::Local);
    fixture.link.send_bytes(b"not json", PeerClass::Local);
    runner.run_pass().expect("the test wait does not fail");
    let replies = fixture.link.take_replies();
    assert_eq!(
        replies,
        [
            json!({"id": 0, "ok": false, "error": "invalid request: request longer than 256 bytes"}),
            json!({"id": 0, "ok": false, "error": "invalid request: invalid utf-8 sequence of 1 bytes from index 0"}),
            json!({"id": 0, "ok": false, "error": "invalid request: expected ident at line 1 column 2"}),
        ]
    );
    let audited = fixture.call(&mut runner, &request(1, "health"));
    assert_eq!(
        audited["audit_id"],
        json!("audit-1"),
        "none of the refused frames was audited"
    );
}

#[test]
fn a_busy_link_cannot_hold_back_a_due_scan() {
    let fixture = Fixture::board_probe();
    let mut runner = fixture.runner();
    fixture.pass_at(&mut runner, 0);
    let _ = fixture.take_calls();
    let _ = fixture.wait.take_calls();

    fixture.clock.set(ms(10));
    for id in 0..10 {
        fixture.link.send(&request(id, "health"));
    }
    runner.run_pass().expect("the test wait does not fail");
    assert_eq!(
        fixture.take_scan_times(),
        [10],
        "the due scan runs before any queued request is answered"
    );
    assert_eq!(
        fixture.link.take_replies(),
        [] as [Value; 0],
        "no request is started while the scan is due"
    );
    assert_eq!(
        fixture.wait.take_calls(),
        [] as [WaitCall; 0],
        "a pass that held a request back serves the rest at once instead of waiting"
    );
    runner.run_pass().expect("the test wait does not fail");
    assert_eq!(fixture.link.take_replies().len(), FRAMES_PER_PASS);
    runner.run_pass().expect("the test wait does not fail");
    assert_eq!(fixture.link.take_replies().len(), 10 - FRAMES_PER_PASS);
    assert_eq!(fixture.wait.take_deadlines(), vec![ms(20)]);
}

/// A control link whose every reply takes `time` on the test clock, as a `status` reply takes on
/// the chip's flash. Its requests and replies are the test link's.
struct SlowReplies {
    link: TestLink,
    clock: TestClock,
    time: Duration,
}

impl ControlLink for SlowReplies {
    fn max_frame_len(&self) -> usize {
        self.link.max_frame_len()
    }

    fn poll_frame(&mut self, buffer: &mut [u8]) -> Option<LinkFrame> {
        self.link.poll_frame(buffer)
    }

    fn send_frame(&mut self, frame: &[u8]) -> Result<(), PortError> {
        self.clock.advance(self.time);
        self.link.send_frame(frame)
    }
}

#[test]
fn a_due_cycle_is_not_delayed_by_queued_requests() {
    // Specification 34, STATE-08: a due cycle waits at most for the request already being
    // answered. The first reply takes 10 ms, so the scan due at 10 ms falls due after request 0 is
    // answered: the scan must run before request 1 is answered, not after all eight.
    let fixture = Fixture::board_probe();
    let slow = SlowReplies {
        link: fixture.link.clone(),
        clock: fixture.clock.clone(),
        time: ms(10),
    };
    let mut runner = fixture.runner_with_link(slow);
    runner.run_pass().expect("the test wait does not fail");
    assert_eq!(
        fixture.take_scan_times(),
        [0],
        "the power-on scan runs at 0 ms"
    );
    for id in 0..8 {
        fixture.link.send(&request(id, "health"));
    }
    runner.run_pass().expect("the test wait does not fail");
    assert_eq!(
        fixture.take_scan_times(),
        [10],
        "the scan due at 10 ms runs in that pass"
    );
    let replies = fixture.link.take_replies();
    assert_eq!(
        replies.len(),
        1,
        "only request 0 is answered before the scan; a loop that checks the boundary once per pass \
         would answer all eight first"
    );
    assert_eq!(replies[0]["id"], json!(0));
}

#[test]
fn a_reply_longer_than_the_link_moves_is_replaced_by_an_error_and_never_cut() {
    let fixture = Fixture::with_frames_of(256);
    let mut runner = fixture.runner();
    // Each scan after the first comes two activations of the 10 ms task late, so each records an
    // overrun; the store keeps no event that every scan makes (specification 11, the event store).
    for at in [0, 30, 60, 90, 120] {
        fixture.pass_at(&mut runner, at);
    }
    let reply = fixture.call(&mut runner, &request(1, "events.tail"));
    assert_eq!(
        reply,
        json!({
            "id": 1,
            "ok": false,
            "error": "reply longer than 256 bytes",
            "audit_id": "audit-1",
        }),
        "four overruns, a reply of about 330 bytes, do not fit a 256-byte frame; transfers in \
         parts come with deployment"
    );
    let fits = fixture.call(
        &mut runner,
        &json!({"id": 2, "type": "events.tail", "params": {"limit": 1}}),
    );
    assert_eq!(
        fits["result"]["events"],
        json!([{"type": "task_overrun", "name": "Scan", "missed": 2, "time_ns": 120_000_000}])
    );
}
