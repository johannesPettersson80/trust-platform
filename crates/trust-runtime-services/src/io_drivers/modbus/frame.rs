//! The Modbus TCP frames that come on a connection, in pieces of any size: the seven bytes of the
//! MBAP header first, which `trust-modbus` checks, then as many bytes as its length counts
//! (Modbus Messaging on TCP/IP V1.0b §3.1.3). A frame is at most 260 bytes, so the assembler needs
//! no heap.

use trust_modbus::tcp::{MbapHeader, HEADER_LEN, MAX_ADU_LEN};
use trust_modbus::Error as WireError;

/// The frame being received.
#[derive(Debug, Clone)]
pub(super) struct Assembler {
    /// The frame's bytes so far.
    buffer: [u8; MAX_ADU_LEN],
    /// How many bytes of `buffer` count.
    len: usize,
}

impl Assembler {
    /// No bytes yet.
    pub(super) const fn new() -> Self {
        Self {
            buffer: [0; MAX_ADU_LEN],
            len: 0,
        }
    }

    /// Forgets the bytes so far, as when the connection is given up.
    pub(super) fn clear(&mut self) {
        self.len = 0;
    }

    /// Takes bytes from the start of `input` into the frame, and gives how many it took and, once
    /// the frame is complete, its header; `take` then gives the frame.
    ///
    /// The error is the header's when it is not Modbus or its length is outside 2 to 254: the
    /// stream cannot be framed any further.
    pub(super) fn push(&mut self, input: &[u8]) -> (usize, Result<Option<MbapHeader>, WireError>) {
        let mut taken = 0;
        if self.len < HEADER_LEN {
            taken = self.fill(HEADER_LEN, input);
            if self.len < HEADER_LEN {
                return (taken, Ok(None));
            }
        }
        let Some(head) = self.buffer.first_chunk::<HEADER_LEN>() else {
            return (taken, Ok(None));
        };
        let header = match MbapHeader::decode(*head) {
            Ok(header) => header,
            Err(error) => return (taken, Err(error)),
        };
        let end = HEADER_LEN + header.pdu_len();
        taken += self.fill(end, input.get(taken..).unwrap_or_default());
        (taken, Ok((self.len >= end).then_some(header)))
    }

    /// The complete frame's bytes, the header's length of them counting, and an empty assembler
    /// for the next frame.
    pub(super) fn take(&mut self) -> [u8; MAX_ADU_LEN] {
        self.len = 0;
        self.buffer
    }

    /// Copies bytes from the start of `input` until the frame has `end` bytes, and gives how many.
    fn fill(&mut self, end: usize, input: &[u8]) -> usize {
        let count = end.saturating_sub(self.len).min(input.len());
        let start = self.len;
        match (
            self.buffer.get_mut(start..start + count),
            input.get(..count),
        ) {
            (Some(target), Some(source)) => {
                target.copy_from_slice(source);
                self.len += count;
                count
            }
            _ => 0,
        }
    }
}
