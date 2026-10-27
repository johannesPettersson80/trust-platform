//! Two control links behind one (specification 34 ARCH-12): a chip on a cable and on a network
//! serves both, its USB link and its network link, with one resource loop. [`LinkPair`] polls them in
//! turn, so neither starves the other, and sends each reply to the link its request came from.

use trust_runtime_core::ports::{ControlLink, LinkFrame, PortError};

/// Which of the two links a frame came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Side {
    First,
    Second,
}

/// Two control links served as one. Both move frames of the same length, the shorter of the two.
pub struct LinkPair<A, B> {
    first: A,
    second: B,
    /// The link the frame polled last came from, which its reply goes to.
    last: Side,
    /// The link polled first in the next poll.
    next: Side,
}

impl<A: ControlLink, B: ControlLink> LinkPair<A, B> {
    /// The pair of `first` and `second`.
    pub fn new(first: A, second: B) -> Self {
        Self {
            first,
            second,
            last: Side::First,
            next: Side::First,
        }
    }

    /// The first link.
    pub fn first_mut(&mut self) -> &mut A {
        &mut self.first
    }

    /// The second link.
    pub fn second_mut(&mut self) -> &mut B {
        &mut self.second
    }

    fn poll_side(&mut self, side: Side, buffer: &mut [u8]) -> Option<LinkFrame> {
        let frame = match side {
            Side::First => self.first.poll_frame(buffer),
            Side::Second => self.second.poll_frame(buffer),
        }?;
        self.last = side;
        // The other link is asked first next time, so a busy link cannot starve the other.
        self.next = match side {
            Side::First => Side::Second,
            Side::Second => Side::First,
        };
        Some(frame)
    }
}

impl<A: ControlLink, B: ControlLink> ControlLink for LinkPair<A, B> {
    fn max_frame_len(&self) -> usize {
        self.first.max_frame_len().min(self.second.max_frame_len())
    }

    fn poll_frame(&mut self, buffer: &mut [u8]) -> Option<LinkFrame> {
        let (one, other) = match self.next {
            Side::First => (Side::First, Side::Second),
            Side::Second => (Side::Second, Side::First),
        };
        self.poll_side(one, buffer)
            .or_else(|| self.poll_side(other, buffer))
    }

    fn send_frame(&mut self, frame: &[u8]) -> Result<(), PortError> {
        match self.last {
            Side::First => self.first.send_frame(frame),
            Side::Second => self.second.send_frame(frame),
        }
    }

    fn sending(&self) -> bool {
        self.first.sending() || self.second.sending()
    }
}

#[cfg(test)]
mod tests {
    use alloc::collections::VecDeque;
    use alloc::vec::Vec;

    use trust_runtime_core::ports::PeerClass;

    use super::*;

    /// A link with frames queued to arrive, which records the replies sent on it.
    #[derive(Default)]
    struct Queued {
        arriving: VecDeque<Vec<u8>>,
        sent: Vec<Vec<u8>>,
        sending: bool,
    }

    impl ControlLink for Queued {
        fn max_frame_len(&self) -> usize {
            8_192
        }

        fn poll_frame(&mut self, buffer: &mut [u8]) -> Option<LinkFrame> {
            let frame = self.arriving.pop_front()?;
            buffer[..frame.len()].copy_from_slice(&frame);
            Some(LinkFrame::Frame {
                len: frame.len(),
                peer: PeerClass::Local,
            })
        }

        fn send_frame(&mut self, frame: &[u8]) -> Result<(), PortError> {
            self.sent.push(frame.to_vec());
            Ok(())
        }

        fn sending(&self) -> bool {
            self.sending
        }
    }

    fn queued(frames: &[&[u8]]) -> Queued {
        Queued {
            arriving: frames.iter().map(|frame| frame.to_vec()).collect(),
            ..Queued::default()
        }
    }

    /// Requests on both links are taken in turn, and each reply goes to the link its request came
    /// from. Catches a reply sent to the USB link for a request that came over the network, and a
    /// busy link that starves the other.
    #[test]
    fn requests_alternate_and_replies_go_back_where_they_came_from() {
        let mut pair = LinkPair::new(queued(&[b"u1", b"u2"]), queued(&[b"n1"]));
        let mut buffer = [0u8; 8_192];
        let mut order = Vec::new();
        while let Some(LinkFrame::Frame { len, .. }) = pair.poll_frame(&mut buffer) {
            let request = buffer[..len].to_vec();
            pair.send_frame(&[b"re-", request.as_slice()].concat())
                .expect("the reply is sent");
            order.push(request);
        }
        assert_eq!(order, [b"u1".to_vec(), b"n1".to_vec(), b"u2".to_vec()]);
        assert_eq!(pair.first.sent, [b"re-u1".to_vec(), b"re-u2".to_vec()]);
        assert_eq!(pair.second.sent, [b"re-n1".to_vec()]);
    }

    /// The pair is sending while either link is. Catches a reply on the network link that the loop
    /// stops driving because the USB link is idle.
    #[test]
    fn the_pair_is_sending_while_either_link_is() {
        let mut pair = LinkPair::new(queued(&[]), queued(&[]));
        assert!(!pair.sending());
        pair.second_mut().sending = true;
        assert!(pair.sending());
    }
}
