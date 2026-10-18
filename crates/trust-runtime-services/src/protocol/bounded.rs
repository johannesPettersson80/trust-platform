//! A successful response written piece by piece within a bound (specification 34, PORT-05).

use alloc::string::String;
use core::fmt::{self, Write};

use super::ControlResponse;

/// A successful response written piece by piece into one buffer of at most `bound` bytes, for a
/// link that moves replies of at most that length: a reply that does not fit is noticed at its
/// first byte beyond the bound, before it is ever built whole.
///
/// The buffer takes the whole bound when it is created and never grows past it. A piece that
/// does not fit is dropped and marks the reply as overflowed, and every later piece is dropped
/// too.
pub(crate) struct BoundedReply {
    text: String,
    bound: usize,
    overflowed: bool,
}

impl BoundedReply {
    /// The successful response to request `id`, written up to its result.
    pub(crate) fn ok(id: u64, bound: usize) -> Self {
        let mut text = String::new();
        // A bound too large to reserve leaves the text to grow as it is written, never past the
        // bound.
        let _ = text.try_reserve_exact(bound);
        let mut reply = Self {
            text,
            bound,
            overflowed: false,
        };
        // Writing into a bounded reply never fails; past the bound it records the overflow.
        let _ = ControlResponse::write_ok_start(&mut reply, id);
        reply
    }

    /// Appends `piece`, or, when it does not fit the bound, drops it and every later piece.
    pub(crate) fn push(&mut self, piece: &str) {
        if self.overflowed || piece.len() > self.bound.saturating_sub(self.text.len()) {
            self.overflowed = true;
            return;
        }
        self.text.push_str(piece);
    }

    /// Appends the JSON text of `value`, exactly as serializing it writes it.
    pub(crate) fn push_json(&mut self, value: &serde_json::Value) {
        // A value displays as its compact JSON text, and writing into the reply never fails.
        let _ = write!(self, "{value}");
    }

    /// Whether a piece did not fit the bound.
    pub(crate) const fn overflowed(&self) -> bool {
        self.overflowed
    }

    /// The whole response: what was written, then the end of `served`, the audited successful
    /// response to the same request, which carries the audit record id. `None` when it does not
    /// fit the bound.
    pub(crate) fn finish(mut self, served: &ControlResponse) -> Option<String> {
        let _ = served.write_ok_end(&mut self);
        (!self.overflowed).then_some(self.text)
    }
}

impl Write for BoundedReply {
    /// Never fails: a piece beyond the bound marks the reply as overflowed instead, so a
    /// serializer that writes into the reply always runs to its end.
    fn write_str(&mut self, piece: &str) -> fmt::Result {
        self.push(piece);
        Ok(())
    }
}
