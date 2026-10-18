//! Request lines out of the bytes a link receives (specification 34, PORT-05).

use super::ring::ByteRing;

/// The next request line a link received.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Line {
    /// A request frame of this many bytes, now at the start of the caller's buffer.
    Frame(usize),
    /// A request longer than the frame; its bytes were dropped up to its line feed.
    Oversized,
}

/// Cuts the bytes a link receives into request lines, in a ring of `N` bytes.
///
/// A line is the bytes before a line feed, with one carriage return directly before the line feed
/// removed, as trust-runtime reads its control socket; an empty line is a frame of zero bytes.
/// The ring holds one whole line of the longest frame, [`Self::MAX_FRAME_LEN`] bytes, with its
/// carriage return and line feed. A line that fills the ring without a line feed is longer than
/// any frame: the assembler empties the ring, drops the rest of the line as it arrives, and
/// reports the line once as [`Line::Oversized`], before the lines that came after it.
#[derive(Debug, Clone)]
pub struct LineAssembler<const N: usize> {
    ring: ByteRing<N>,
    /// The bytes at the front of the ring known to hold no line feed, so that each poll reads only
    /// the bytes that arrived since.
    scanned: usize,
    /// The rest of an oversized line is being dropped up to its line feed. The ring is empty
    /// meanwhile.
    discarding: bool,
    /// Oversized lines that have ended and are not reported yet; they came before every byte the
    /// ring holds.
    oversized: usize,
}

impl<const N: usize> LineAssembler<N> {
    /// The longest request frame, in bytes without the line end.
    pub const MAX_FRAME_LEN: usize = N - 2;

    /// An assembler that has received nothing.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            ring: ByteRing::new(),
            scanned: 0,
            discarding: false,
            oversized: 0,
        }
    }

    /// How many bytes [`Self::receive`] takes now at the least: the ring's free space, or, while
    /// the rest of an oversized line is dropped, a whole ring of bytes.
    #[must_use]
    pub const fn receive_capacity(&self) -> usize {
        if self.discarding {
            N
        } else {
            self.ring.free()
        }
    }

    /// Takes bytes the link received, oldest first, and returns how many it took. The rest stay
    /// with the sender until the loop has taken lines, so no byte is lost. The bytes of an
    /// oversized line are taken and dropped.
    pub fn receive(&mut self, bytes: &[u8]) -> usize {
        let mut dropped = 0;
        if self.discarding {
            let Some(end) = bytes.iter().position(|&byte| byte == b'\n') else {
                return bytes.len();
            };
            self.discarding = false;
            self.oversized = self.oversized.saturating_add(1);
            dropped = end + 1;
        }
        dropped + self.ring.push(&bytes[dropped..])
    }

    /// The next request line, oldest first, with its frame copied to the start of `buffer`, or
    /// `None` until a line has ended. A frame longer than [`Self::MAX_FRAME_LEN`] bytes, or than
    /// `buffer`, is [`Line::Oversized`].
    pub fn next_line(&mut self, buffer: &mut [u8]) -> Option<Line> {
        if self.oversized > 0 {
            self.oversized -= 1;
            return Some(Line::Oversized);
        }
        let end = self.line_end()?;
        let len = if end > 0 && self.ring.get(end - 1) == Some(b'\r') {
            end - 1
        } else {
            end
        };
        let line = match buffer.get_mut(..len) {
            Some(frame) if len <= Self::MAX_FRAME_LEN => {
                self.ring.copy_front(frame);
                Line::Frame(len)
            }
            _ => Line::Oversized,
        };
        self.ring.discard(end + 1);
        self.scanned = 0;
        Some(line)
    }

    /// Starts dropping a line that cannot fit, without taking a line: a ring that is full without
    /// a line feed holds the start of a line longer than any frame. The link calls this while it
    /// sends a reply and takes no request, so such a line never holds back the sender.
    pub fn drop_overlong(&mut self) {
        self.line_end();
    }

    /// Drops every byte received and every oversized line not reported yet, for a new session of
    /// the sender.
    pub fn reset(&mut self) {
        self.ring.clear();
        self.scanned = 0;
        self.discarding = false;
        self.oversized = 0;
    }

    /// The offset of the line feed that ends the oldest line, once it has arrived. A ring that is
    /// full without one is emptied, and the rest of its line is dropped as it arrives.
    fn line_end(&mut self) -> Option<usize> {
        if let Some(end) = self.ring.find(b'\n', self.scanned) {
            self.scanned = end;
            return Some(end);
        }
        self.scanned = self.ring.len();
        if self.ring.is_full() {
            self.ring.clear();
            self.scanned = 0;
            self.discarding = true;
        }
        None
    }
}

impl<const N: usize> Default for LineAssembler<N> {
    fn default() -> Self {
        Self::new()
    }
}
