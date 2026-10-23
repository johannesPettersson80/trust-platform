//! A response written in parts within a bound (specification 34, PORT-05).

use alloc::string::String;
use core::fmt::{self, Write};
use core::str::Utf8Error;

use super::ControlResponse;
use crate::transfer::{PartReserve, ReplyParts};

/// Why a [`ReplyWriter`] stopped taking text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Stopped {
    /// A piece did not fit the bound.
    Overflow,
    /// The heap had no room for the next part; `written` bytes were written.
    NoRoom { written: usize },
}

/// What a reply came to once its text was written.
#[derive(Debug)]
pub(crate) enum Written {
    /// The reply fits the frame: its text as the one line `String` that is sent, joined from its
    /// parts only when it needed more than one.
    Line(String),
    /// The reply is longer than the frame: its parts, never joined, for the session to hold.
    Parts(ReplyParts),
    /// The reply is longer than the bound.
    Overflow,
    /// The heap had no room for the reply: for the next part, after `written` bytes, or for the
    /// line that joins a reply of more than one part.
    NoRoom {
        /// The bytes of the reply written when the heap had no room.
        written: usize,
    },
    /// The parts do not join into text. Every piece written is text, so this does not happen; it is
    /// reported, as a serialization failure is, instead of being passed over.
    NotText(Utf8Error),
}

/// The bytes a writer gathers before it writes them into its parts: a serializer writes a reply in
/// pieces of a few bytes, and writing each into the parts costs more than copying it here
/// (specification 34 PORT-02).
const GATHER_BYTES: usize = 256;

/// A response written piece by piece into parts of 4,096 bytes, for a link that moves replies of at
/// most a frame and a session that holds a longer one up to a bound.
///
/// Each part is taken from the heap when the writing reaches it, with a fallible reservation, and
/// never ahead of the text, so a short reply takes one part and a long reply never takes a block
/// longer than a part. Pieces are gathered in a block of [`GATHER_BYTES`] bytes and written into
/// the parts when it is full and at the end. A piece that does not fit the bound marks the writer
/// as overflowed, and a part the heap has no room for marks it as without room; the two are kept
/// apart, and either drops every later piece. Writing never fails, as `serde_json` requires without
/// `std`: [`Self::finish`] says which of them happened, or hands over the text.
pub(crate) struct ReplyWriter {
    parts: ReplyParts,
    bound: usize,
    reserve: PartReserve,
    stopped: Option<Stopped>,
    gathered: [u8; GATHER_BYTES],
    gathered_len: usize,
}

impl ReplyWriter {
    /// A writer that takes at most `bound` bytes, each part from `reserve`.
    pub(crate) const fn new(bound: usize, reserve: PartReserve) -> Self {
        Self {
            parts: ReplyParts::new(),
            bound,
            reserve,
            stopped: None,
            gathered: [0; GATHER_BYTES],
            gathered_len: 0,
        }
    }

    /// The bytes written, the gathered ones included.
    const fn written(&self) -> usize {
        self.parts.len() + self.gathered_len
    }

    /// Writes `bytes` into the parts, or records that the heap had no room.
    fn write_parts(&mut self, bytes: &[u8]) {
        if self.parts.try_write_with(bytes, self.reserve).is_err() {
            self.stopped = Some(Stopped::NoRoom {
                written: self.parts.len(),
            });
        }
    }

    /// Writes the gathered bytes into the parts.
    fn flush(&mut self) {
        if self.gathered_len == 0 || self.stopped.is_some() {
            return;
        }
        let gathered = self.gathered;
        let len = self.gathered_len;
        self.gathered_len = 0;
        self.write_parts(&gathered[..len]);
    }

    /// The writer of the successful response to request `id`, written up to its result.
    pub(crate) fn ok(id: u64, bound: usize, reserve: PartReserve) -> Self {
        let mut writer = Self::new(bound, reserve);
        // Writing into the writer never fails; a refusal is recorded and reported by `finish`.
        let _ = ControlResponse::write_ok_start(&mut writer, id);
        writer
    }

    /// Appends `piece`, or, when it does not fit, records why and drops it and every later piece.
    pub(crate) fn push(&mut self, piece: &str) {
        // Writing into the writer never fails; a refusal is recorded and reported by `finish`.
        let _ = self.write_str(piece);
    }

    /// Appends the JSON text of `value`, exactly as serializing it writes it.
    pub(crate) fn push_json(&mut self, value: &serde_json::Value) {
        // Writing into the writer never fails; a refusal is recorded and reported by `finish`.
        let _ = write!(self, "{value}");
    }

    /// Whether a piece was refused, for the bound or for lack of room. A writer that is refused
    /// takes nothing more.
    pub(crate) const fn refused(&self) -> bool {
        self.stopped.is_some()
    }

    /// What the reply came to. A reply of at most `frame` bytes is the one line that is sent, and a
    /// longer one is its parts.
    pub(crate) fn finish(mut self, frame: usize) -> Written {
        self.flush();
        match self.stopped {
            Some(Stopped::Overflow) => Written::Overflow,
            Some(Stopped::NoRoom { written }) => Written::NoRoom { written },
            None if self.parts.len() > frame => Written::Parts(self.parts),
            None => line(self.parts),
        }
    }
}

/// The text of a reply that fits a frame, as the one `String` that is sent.
pub(super) fn line(parts: ReplyParts) -> Written {
    let written = parts.len();
    match parts.into_bytes().map(String::from_utf8) {
        Ok(Ok(line)) => Written::Line(line),
        Ok(Err(error)) => Written::NotText(error.utf8_error()),
        Err(_) => Written::NoRoom { written },
    }
}

impl Write for ReplyWriter {
    /// Never fails: a piece beyond the bound, or one the heap has no room for, is recorded and
    /// dropped with every later piece instead. Without `std`, `serde_json` requires its writers to be
    /// infallible, so a serializer that writes into the writer always runs to its end. A part of the
    /// text may end inside a character of `piece`, because the parts are bytes.
    fn write_str(&mut self, piece: &str) -> fmt::Result {
        if self.stopped.is_some() {
            return Ok(());
        }
        if piece.len() > self.bound.saturating_sub(self.written()) {
            self.flush();
            if self.stopped.is_none() {
                self.stopped = Some(Stopped::Overflow);
            }
            return Ok(());
        }
        let bytes = piece.as_bytes();
        if bytes.len() > GATHER_BYTES - self.gathered_len {
            self.flush();
        }
        if bytes.len() >= GATHER_BYTES {
            self.write_parts(bytes);
        } else if let Some(room) = self
            .gathered
            .get_mut(self.gathered_len..self.gathered_len + bytes.len())
        {
            room.copy_from_slice(bytes);
            self.gathered_len += bytes.len();
        }
        Ok(())
    }
}
