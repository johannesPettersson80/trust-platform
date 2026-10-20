//! The retain capture shared by computers and chips (specification 11 §6.7, specification 34 §10).
//!
//! One rule decides when a retained image is written: a configured interval, a dirty mark, and a
//! write only when the image changed (specification 11 §6.7). The hosted `RetainManager` and a
//! chip's persistence adapter share it, together with the STRN v3 payload bytes it stages.
//!
//! The staged payload is reserved when the engine is constructed, with room for the largest image,
//! and refilled in place by every capture, so a platform that captures at its persistence checkpoint
//! allocates nothing, at its first capture neither (specification 34 MEM-04, MEM-09).

use alloc::vec::Vec;
use core::mem;

use smol_str::SmolStr;

use crate::collections::OrderedMap;
use crate::error::RuntimeError;
use crate::retain::codec::PayloadWriter;
use crate::value::{Duration, Value};

/// The retention rule and the staged retained payload (specification 11 §6.7).
///
/// [`should_save`](Self::should_save) answers the platform's due question: the configured interval
/// elapsed while the values are dirty. [`image_changed`](Self::image_changed) answers whether the
/// staged payload differs from the payload a store accepted, which is the rule "only writes when
/// retained values have changed" (specification 11 §6.7). [`accept`](Self::accept) records a
/// payload a store wrote; [`unchanged`](Self::unchanged) records an image that did not change and
/// so needs no write.
#[derive(Debug)]
pub struct RetainCapture {
    interval: Option<Duration>,
    last_save: Duration,
    dirty: bool,
    /// The STRN v3 payload this capture is filling.
    staged: Vec<u8>,
    /// The payload a store accepted; the unchanged-image skip compares against it.
    accepted: Option<Vec<u8>>,
    /// The buffer the first accepted payload leaves the next capture, reserved with the staged one.
    spare: Vec<u8>,
}

impl Default for RetainCapture {
    fn default() -> Self {
        Self {
            interval: None,
            last_save: Duration::ZERO,
            dirty: false,
            staged: Vec::new(),
            accepted: None,
            spare: Vec::new(),
        }
    }
}

impl RetainCapture {
    /// A capture with no interval, no staged payload and no accepted payload.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Replace the interval and start its cadence at `now`, with nothing dirty and no accepted
    /// payload, so the next due save writes.
    ///
    /// This is the hosted adapter's reconfiguration rule (specification 11 §6.7).
    pub fn configure(&mut self, interval: Option<Duration>, now: Duration) {
        self.interval = interval;
        self.last_save = now;
        self.dirty = false;
        // The accepted payload's buffer is kept as the spare for the next accepted one.
        if let Some(mut accepted) = self.accepted.take()
            && accepted.capacity() > self.spare.capacity()
        {
            accepted.clear();
            self.spare = accepted;
        }
    }

    /// Room for a payload of `bytes` bytes in the staged buffer and in the buffer the next accepted
    /// payload leaves the next capture, the accepted payload's once a store accepted one and the
    /// spare before, so that no capture grows either (specification 34 MEM-09). Room already
    /// reserved is kept.
    ///
    /// # Errors
    ///
    /// [`RuntimeError::Overflow`] when the room cannot be reserved.
    pub fn reserve(&mut self, bytes: usize) -> Result<(), RuntimeError> {
        let next = match self.accepted.as_mut() {
            Some(accepted) => accepted,
            None => &mut self.spare,
        };
        for buffer in [&mut self.staged, next] {
            buffer
                .try_reserve_exact(bytes.saturating_sub(buffer.len()))
                .map_err(|_| RuntimeError::Overflow)?;
        }
        Ok(())
    }

    /// Set the save interval without restarting the cadence or clearing the accepted payload.
    pub fn set_interval(&mut self, interval: Option<Duration>) {
        self.interval = interval;
    }

    /// The configured save interval, read back for inspection.
    #[must_use]
    pub fn interval(&self) -> Option<Duration> {
        self.interval
    }

    /// Mark the retained values changed since the last capture.
    pub fn mark_dirty(&mut self) {
        self.dirty = true;
    }

    /// Whether the interval elapsed and the retained values are dirty.
    #[must_use]
    pub fn should_save(&self, now: Duration) -> bool {
        let Some(interval) = self.interval else {
            return false;
        };
        if !self.dirty {
            return false;
        }
        if interval.as_nanos() <= 0 {
            return true;
        }
        let elapsed = now.as_nanos().saturating_sub(self.last_save.as_nanos());
        elapsed >= interval.as_nanos()
    }

    /// Whether the staged payload differs from the payload a store accepted (or none was accepted
    /// yet).
    #[must_use]
    pub fn image_changed(&self) -> bool {
        self.accepted.as_deref() != Some(self.staged.as_slice())
    }

    /// Refill the staged payload through `fill`, reusing the buffer it reserved.
    ///
    /// A capture visits the retained declarations in order and appends one entry for each; the
    /// buffer is cleared first, so a smaller declaration set leaves no stale entry.
    ///
    /// # Errors
    ///
    /// The error `fill` returns. The capture then holds no staged payload, so a value it could not
    /// visit completely can never be written.
    pub fn refill<T, E>(
        &mut self,
        fill: impl FnOnce(&mut PayloadWriter<'_>) -> Result<T, E>,
    ) -> Result<T, E> {
        let mut staged = mem::take(&mut self.staged);
        staged.clear();
        let result = {
            let mut writer = PayloadWriter::new(&mut staged);
            let result = fill(&mut writer);
            if result.is_ok() {
                writer.finish();
            }
            result
        };
        if result.is_err() {
            // A failed visit leaves the reserved buffer without a partial payload.
            staged.clear();
        }
        self.staged = staged;
        result
    }

    /// Stage the named `values` a platform captured elsewhere as this capture's STRN v3 payload,
    /// keeping the reserved buffer.
    ///
    /// The hosted adapter captures the image through the engine's own snapshot, so it stages that
    /// snapshot's values here to use this capture's unchanged-image skip and its cadence
    /// (specification 11 §6.7).
    ///
    /// # Errors
    ///
    /// [`RuntimeError::RetainStore`] when a value cannot be persisted. The capture then holds no
    /// staged payload.
    pub fn stage_values(
        &mut self,
        values: &OrderedMap<SmolStr, Value>,
    ) -> Result<(), RuntimeError> {
        self.refill(|payload| {
            for (name, value) in values {
                payload.entry(name.as_str(), value)?;
            }
            Ok(())
        })
    }

    /// The staged STRN v3 payload, as of the last capture.
    #[must_use]
    pub fn staged_payload(&self) -> &[u8] {
        &self.staged
    }

    /// Whether a store has accepted a payload.
    #[must_use]
    pub fn has_image(&self) -> bool {
        self.accepted.is_some()
    }

    /// The logical time of the last accepted or unchanged payload.
    #[must_use]
    pub fn last_save(&self) -> Duration {
        self.last_save
    }

    /// Whether the retained values are marked changed since the last capture.
    #[must_use]
    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    /// A store accepted the staged payload: remember it as the written payload, take the
    /// checkpoint, and reserve the buffer the next capture refills.
    pub fn accept(&mut self, now: Duration) {
        let staged = mem::take(&mut self.staged);
        let capacity = staged.capacity();
        self.staged = match self.accepted.replace(staged) {
            Some(mut previous) => {
                previous.clear();
                previous
            }
            // The reserved spare takes the next payload when it has the room.
            None if self.spare.capacity() >= capacity => mem::take(&mut self.spare),
            None => Vec::with_capacity(capacity),
        };
        self.dirty = false;
        self.last_save = now;
    }

    /// The staged payload equals the written payload: the checkpoint is taken without a write
    /// (specification 11 §6.7).
    pub fn unchanged(&mut self, now: Duration) {
        self.dirty = false;
        self.last_save = now;
    }
}
