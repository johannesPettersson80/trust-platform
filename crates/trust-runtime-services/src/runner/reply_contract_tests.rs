//! The loop's waits while its control link still holds reply bytes (specification 34, PORT-05): a
//! link whose reply advances only when the loop polls it is driven to its end by the loop's own
//! passes, at most a millisecond apart, and an idle link keeps the loop's own waits.

use alloc::rc::Rc;
use alloc::vec::Vec;
use core::cell::RefCell;
use core::time::Duration;

use serde_json::{json, Value};
use trust_runtime_core::ports::{ControlLink, LinkFrame, PortError};

use crate::link::DEVICE_FRAME_LEN;
use crate::tests::runner_ports::{ms, Fixture};

/// The bytes the fake link hands to its hardware on one poll: one USB-Serial/JTAG FIFO, as the C6
/// link writes.
const CHUNK: usize = 64;

/// The reply the fake link has to hand to its hardware, in bytes. A long `status` reply of the
/// bench is about this long and spans many FIFOs, so it is finished only by many polls.
const REPLY_BYTES: usize = 2_000;

/// A control link that hands its reply to the hardware only while the loop polls it: each poll
/// hands one [`CHUNK`] bytes out of a [`REPLY_BYTES`]-byte reply, it never hands out a request while
/// the reply is in flight, and it never wakes the loop.
#[derive(Clone)]
struct SlowLink {
    state: Rc<RefCell<SlowState>>,
}

struct SlowState {
    /// The reply bytes still to hand to the hardware.
    pending: usize,
    /// The polls the loop made.
    polls: u32,
}

impl SlowLink {
    /// A link with a reply of `bytes` bytes in flight.
    fn with_reply(bytes: usize) -> Self {
        Self {
            state: Rc::new(RefCell::new(SlowState {
                pending: bytes,
                polls: 0,
            })),
        }
    }

    /// The polls the loop made since the link was built.
    fn polls(&self) -> u32 {
        self.state.borrow().polls
    }
}

impl ControlLink for SlowLink {
    fn max_frame_len(&self) -> usize {
        DEVICE_FRAME_LEN
    }

    fn poll_frame(&mut self, _buffer: &mut [u8]) -> Option<LinkFrame> {
        let mut state = self.state.borrow_mut();
        state.polls += 1;
        state.pending = state.pending.saturating_sub(CHUNK);
        // A line link hands out no request while its reply is in flight, and this link has none.
        None
    }

    fn send_frame(&mut self, _frame: &[u8]) -> Result<(), PortError> {
        Ok(())
    }

    fn sending(&self) -> bool {
        self.state.borrow().pending > 0
    }
}

fn request(id: u64, kind: &str) -> Value {
    json!({"id": id, "type": kind})
}

#[test]
fn a_reply_in_flight_is_finished_without_a_later_request() {
    // Specification 34, PORT-05: while the link still holds reply bytes it has not handed to its
    // hardware the loop services it at least once a millisecond, so a reply in flight reaches its
    // end without depending on the link's interrupts. Catches a loop that waits for its next
    // boundary (the 10 ms cycle here) or its watchdog feed interval (1 s) while the link still
    // holds reply bytes, so the tail of the reply reaches the host only when the next request
    // wakes the loop.
    let fixture = Fixture::board_probe();
    let link = SlowLink::with_reply(REPLY_BYTES);
    let mut runner = fixture.runner_with_link(link.clone());

    let mut at = Duration::ZERO;
    let mut polled_at = Vec::new();
    let mut waits = Vec::new();
    loop {
        // The loop's wait would sleep until its deadline; move the clock there so the next pass
        // starts when a real wait would have returned.
        fixture.clock.set(at);
        let _ = fixture.wait.take_calls();
        runner.run_pass().expect("the test wait does not fail");
        polled_at.push(at);
        if !link.sending() {
            break;
        }
        let deadlines = fixture.wait.take_deadlines();
        assert_eq!(
            deadlines.len(),
            1,
            "a pass that served no request waits once"
        );
        waits.push(deadlines[0]);
        at = deadlines[0];
    }

    // 2,000 bytes handed out 64 bytes at a time: 32 polls, the last one finishing the reply.
    assert_eq!(link.polls(), 32);
    assert_eq!(polled_at.len(), 32);
    for pair in polled_at.windows(2) {
        let gap = pair[1]
            .checked_sub(pair[0])
            .expect("the loop polls the link on a monotonic clock");
        assert!(
            gap <= ms(1),
            "the loop polled the link {} ms after the previous poll, longer than the 1 ms a reply \
             in flight is allowed to wait",
            gap.as_millis()
        );
    }
    assert_eq!(
        waits.last(),
        Some(&ms(31)),
        "each wait while the reply was in flight is 1 ms from its pass, not the 10 ms boundary"
    );
}

#[test]
fn an_idle_link_keeps_the_runners_waits() {
    // Specification 34, PORT-05: only a link with reply bytes it has not handed to its hardware
    // caps the loop's wait at one millisecond; an idle link keeps the loop's own waits. Catches a
    // loop that caps every wait at one millisecond, polling its link a thousand times a second
    // with nothing to send.
    let fixture = Fixture::board_probe();
    let mut runner = fixture.runner();
    fixture.pass_at(&mut runner, 0);
    let _ = fixture.wait.take_calls();

    fixture.clock.set(ms(3));
    fixture.link.send(&request(1, "status"));
    runner.run_pass().expect("the test wait does not fail");
    assert_eq!(fixture.link.take_replies().len(), 1);
    assert_eq!(
        fixture.wait.take_deadlines(),
        [ms(10)],
        "an idle link keeps the loop's wait for the next boundary, not one millisecond"
    );
}
