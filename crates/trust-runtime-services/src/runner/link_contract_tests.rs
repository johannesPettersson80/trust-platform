//! The resource loop's control link (specification 34, §7, ARCH-12 and STATE-08): the requests
//! only the platform answers, the access rights of the link's peers, the frames that cannot be
//! request lines, the replies that do not fit a frame and the bound on the requests of one pass,
//! with the board probe program on the real engine.

use alloc::string::ToString;
use alloc::vec;
use core::net::{IpAddr, Ipv4Addr, SocketAddr};

use serde_json::{json, Value};
use trust_runtime_core::ports::PeerClass;

use super::FRAMES_PER_PASS;
use crate::tests::runner_ports::{ms, Fixture, TestPlatform, WaitCall};

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
    assert_eq!(fixture.link.take_replies().len(), FRAMES_PER_PASS);
    assert_eq!(
        fixture.take_scan_times(),
        [10],
        "the due scan runs in the same pass"
    );
    assert_eq!(
        fixture.wait.take_calls(),
        [] as [WaitCall; 0],
        "a pass that reached the bound serves the rest at once instead of waiting"
    );
    runner.run_pass().expect("the test wait does not fail");
    assert_eq!(fixture.link.take_replies().len(), 10 - FRAMES_PER_PASS);
    assert_eq!(fixture.wait.take_deadlines(), vec![ms(20)]);
}

#[test]
fn a_reply_longer_than_the_link_moves_is_replaced_by_an_error_and_never_cut() {
    let fixture = Fixture::with_frames_of(256);
    let mut runner = fixture.runner();
    for at in [0, 10, 20] {
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
        "ten events do not fit a 256-byte frame; transfers in parts come with deployment"
    );
    let fits = fixture.call(
        &mut runner,
        &json!({"id": 2, "type": "events.tail", "params": {"limit": 1}}),
    );
    assert_eq!(
        fits["result"]["events"],
        json!([{"type": "cycle_end", "cycle": 2, "time_ns": 20_000_000}])
    );
}
