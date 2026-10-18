//! The control link of a scan that the debugger holds (specification 34, PORT-05; specification
//! 11 §6.9, "Hold duration"), with the board probe on the real engine behind the cycle port: a
//! reply in flight is driven to its end within a millisecond per packet while the scan is held,
//! and a new host session, such as a USB bus reset, drops only the reply being sent and releases
//! nothing. The link here models the USB session; the other tests of the hold are in
//! `hold_contract_tests.rs`, whose helpers and timeline these tests share.

use alloc::collections::VecDeque;
use alloc::rc::Rc;
use alloc::string::ToString;
use alloc::vec::Vec;
use core::cell::RefCell;
use core::time::Duration;

use serde_json::{json, Value};
use trust_runtime_core::ports::{ControlLink, LinkFrame, MonotonicClock, PeerClass, PortError};
use trust_runtime_core::scheduler::ResourceState;

use super::hold_contract_tests::{assert_waited_for_the_held_scan, on_led_assignment, request};
use crate::tests::runner_ports::{ms, Fixture, TestClock};

/// How many bytes of a reply the fake link hands to its hardware on one poll: one USB-Serial/JTAG
/// FIFO, as the C6 link writes.
const CHUNK: usize = 64;

struct SessionState {
    /// The requests that reach the link, each at its time on the clock.
    incoming: VecDeque<(Duration, Vec<u8>)>,
    /// The bytes of the reply in flight that the link has not handed to its hardware.
    pending: usize,
    /// The replies the loop sent, and whether a new session dropped each.
    replies: Vec<(Value, bool)>,
    /// The times of the polls made while a reply was in flight.
    drain_polls: Vec<Duration>,
}

/// A control link that models the USB session: a reply goes out one [`CHUNK`] per poll, no request
/// reaches the loop while a reply is in flight, and a new session drops the reply being sent
/// (specification 34, PORT-05).
#[derive(Clone)]
struct SessionLink {
    state: Rc<RefCell<SessionState>>,
    clock: TestClock,
}

impl SessionLink {
    fn new(clock: TestClock) -> Self {
        Self {
            state: Rc::new(RefCell::new(SessionState {
                incoming: VecDeque::new(),
                pending: 0,
                replies: Vec::new(),
                drain_polls: Vec::new(),
            })),
            clock,
        }
    }

    /// `request` reaches the link when the clock reaches `at`.
    fn arrive_at(&self, at: Duration, request: &Value) {
        self.state
            .borrow_mut()
            .incoming
            .push_back((at, request.to_string().into_bytes()));
    }

    /// A USB bus reset: a new host session drops the reply being sent.
    fn new_session(&self) {
        let mut state = self.state.borrow_mut();
        if state.pending > 0
            && let Some(reply) = state.replies.last_mut()
        {
            reply.1 = true;
        }
        state.pending = 0;
    }

    /// The replies that reached a host.
    fn delivered(&self) -> Vec<Value> {
        self.state
            .borrow()
            .replies
            .iter()
            .filter(|(_, dropped)| !dropped)
            .map(|(reply, _)| reply.clone())
            .collect()
    }

    /// How many replies a new session dropped.
    fn dropped(&self) -> usize {
        self.state
            .borrow()
            .replies
            .iter()
            .filter(|(_, dropped)| *dropped)
            .count()
    }

    /// The times of the polls made while a reply was in flight.
    fn drain_polls(&self) -> Vec<Duration> {
        self.state.borrow().drain_polls.clone()
    }
}

impl ControlLink for SessionLink {
    fn max_frame_len(&self) -> usize {
        4_096
    }

    fn poll_frame(&mut self, buffer: &mut [u8]) -> Option<LinkFrame> {
        let mut state = self.state.borrow_mut();
        let now = self.clock.now().since_origin();
        if state.pending > 0 {
            state.drain_polls.push(now);
            state.pending = state.pending.saturating_sub(CHUNK);
            // A line link gives the loop no request while a reply is in flight.
            return None;
        }
        if state.incoming.front().is_none_or(|(at, _)| *at > now) {
            return None;
        }
        let (_, frame) = state.incoming.pop_front()?;
        buffer.get_mut(..frame.len())?.copy_from_slice(&frame);
        Some(LinkFrame::Frame {
            len: frame.len(),
            peer: PeerClass::Local,
        })
    }

    fn send_frame(&mut self, frame: &[u8]) -> Result<(), PortError> {
        let reply = serde_json::from_slice(frame).expect("every reply is one JSON object");
        let mut state = self.state.borrow_mut();
        state.replies.push((reply, false));
        // The reply and its line end.
        state.pending = frame.len() + 1;
        Ok(())
    }

    fn sending(&self) -> bool {
        self.state.borrow().pending > 0
    }
}

/// A host's `request` that reaches `link` at `at` and wakes the loop's wait then, as the link's
/// interrupt does.
fn host_sends(fixture: &Fixture, link: &SessionLink, at: Duration, request: &Value) {
    link.arrive_at(at, request);
    fixture.wait.at(at, || {});
}

#[test]
fn a_new_host_session_keeps_the_hold() {
    // Specification 11 §6.9 ("Hold duration"): neither a detach, an attach, a lost connection nor an
    // elapsed timeout releases a hold, however long the operator waits; specification 34 PORT-05: a
    // new host session, such as a USB bus reset, drops only the reply being sent. The first host
    // asks for `status` at 1 s and a bus reset drops the reply part-way. Ten minutes pass with
    // nobody there, a new host asks again and still finds the scan held at its breakpoint, and only
    // its `resume` releases it. Wrong behaviour caught: a loop that releases the hold when the
    // session ends or after a time, and one that repeats or restarts the scan.
    let fixture = Fixture::board_probe().with_debugger();
    let link = SessionLink::new(fixture.clock.clone());
    let mut runner = fixture.runner_with_link(link.clone());
    runner.shell().debug.add_breakpoint(on_led_assignment());
    fixture.wait.advance_time();
    host_sends(&fixture, &link, ms(1_000), &request(1, "status"));
    {
        let link = link.clone();
        fixture.wait.at(ms(1_005), move || link.new_session());
    }
    host_sends(&fixture, &link, ms(600_000), &request(2, "status"));
    host_sends(&fixture, &link, ms(601_000), &request(3, "resume"));
    runner.run_pass().expect("the test wait does not fail");
    assert_waited_for_the_held_scan(&fixture);

    runner.run_pass().expect("the test wait does not fail");

    assert_eq!(
        link.dropped(),
        1,
        "the bus reset dropped the reply in flight"
    );
    let delivered = link.delivered();
    assert_eq!(
        delivered.len(),
        2,
        "the second status and the resume: {delivered:?}"
    );
    assert_eq!(delivered[0]["result"]["state"], json!("paused"));
    assert_eq!(
        delivered[0]["result"]["debug_hold"],
        json!({"reason": "breakpoint"}),
        "ten minutes and a new session later the scan is still held"
    );
    assert_eq!(delivered[1]["result"], json!({"status": "running"}));
    assert_eq!(
        fixture.take_scan_times(),
        [0, 10],
        "the held scan was not repeated"
    );
    assert_eq!(runner.resource_state(), ResourceState::Running);
}

#[test]
fn a_reply_in_flight_is_serviced_within_one_millisecond_while_held() {
    // Specification 34 PORT-05 ("One reply in flight"): while the link still holds reply bytes it has
    // not handed to its hardware the loop waits no longer than a millisecond, so a reply in flight is
    // driven to its end within a millisecond per packet without depending on the link's interrupts;
    // a hold keeps that rule. The `status` reply of about two kilobytes goes out 64 bytes per poll.
    // Wrong behaviour caught: a hold that waits for the feed interval of 1,000 ms while the reply is
    // in flight, so that the tail of the reply reaches the host seconds late.
    let fixture = Fixture::board_probe().with_debugger();
    let link = SessionLink::new(fixture.clock.clone());
    let mut runner = fixture.runner_with_link(link.clone());
    runner.shell().debug.add_breakpoint(on_led_assignment());
    fixture.wait.advance_time();
    host_sends(&fixture, &link, ms(1_000), &request(1, "status"));
    host_sends(&fixture, &link, ms(3_000), &request(2, "resume"));
    runner.run_pass().expect("the test wait does not fail");
    assert_waited_for_the_held_scan(&fixture);

    runner.run_pass().expect("the test wait does not fail");

    // The polls that drove the `status` reply, which went out at 1 s; the short reply to the
    // `resume` at 3 s needs one poll.
    let polls: Vec<Duration> = link
        .drain_polls()
        .into_iter()
        .filter(|at| *at < ms(2_000))
        .collect();
    assert!(
        polls.len() >= 10,
        "the reply took {} polls, too few to prove the rule",
        polls.len()
    );
    for pair in polls.windows(2) {
        let gap = pair[1]
            .checked_sub(pair[0])
            .expect("the loop polls the link on a monotonic clock");
        assert!(
            gap <= ms(1),
            "the loop polled the link {} ms after the previous poll, longer than the millisecond a \
             reply in flight may wait",
            gap.as_millis()
        );
    }
    assert_eq!(
        link.delivered().len(),
        2,
        "the status and the resume were answered"
    );
}
