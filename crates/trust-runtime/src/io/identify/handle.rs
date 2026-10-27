//! The hosted identify state, shared by everything that asks for it or drives it (specification
//! 11, "Identify"; specification 34, SERVICE-07).
//!
//! One [`IdentifyHandle`] is made at composition and cloned to the control state, which answers
//! `identify` and `status` through it, and to the runtime, whose scan and whose waits drive the
//! output through it. A program (re)load gives the same handle its new output. The handle holds
//! the [`HostIdentify`] of the project's `[runtime.identify] output`, if it has one, and whether
//! the drivers last got the output image with the identify bit set.
//!
//! Two threads touch it: the control server's request threads, which hold the state for the
//! length of one start or one question, and the one thread of the resource, which ticks the blink.
//! The resource thread never waits for a request: when a request holds the state at that moment,
//! the tick is left for the next point (the next scan or the next poll of a wait).

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError, TryLockError};

use trust_runtime_services::identify::IdentifyState;

use super::{HostIdentify, ImageBit};
use crate::io::IoAddress;

/// What a pass in which no scan runs has to hand the drivers (specification 11, "Identify",
/// "The blink").
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum IdleOutput {
    /// The drivers already hold the output image as the blink wants it.
    Unchanged,
    /// The committed output image is to be handed to the drivers again, with this bit set.
    Hand(Option<ImageBit>),
}

/// The identify state of one runtime: none while the runtime has no identify output.
///
/// Clones share the state.
#[derive(Debug, Clone, Default)]
pub struct IdentifyHandle {
    slot: Arc<Mutex<Option<HostIdentify>>>,
    /// Whether the output image that the drivers got last had the identify bit set. Only the
    /// resource thread, which hands out the outputs, writes it.
    bit_handed_out: Arc<AtomicBool>,
}

impl IdentifyHandle {
    /// A handle without an identify output.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the identify state: the output that `[runtime.identify] output` names, or `None` for a
    /// runtime without one, which then answers `identify_not_configured`. A run of the state it
    /// replaces ends, and its output is driven off first.
    pub fn set(&self, identify: Option<HostIdentify>) {
        let mut slot = self.lock();
        if let Some(replaced) = slot.as_mut()
            && let Err(error) = replaced.cancel()
        {
            tracing::warn!("identify output could not be driven off: {error}");
        }
        *slot = identify;
        self.bit_handed_out.store(false, Ordering::Release);
    }

    /// Runs `use_identify` on the identify state, or returns `None` when the runtime has no
    /// identify output. The control state answers `identify` and `status` through it.
    pub fn with_identify<R>(
        &self,
        use_identify: impl FnOnce(&mut dyn IdentifyState) -> R,
    ) -> Option<R> {
        self.lock().as_mut().map(|identify| use_identify(identify))
    }

    /// Applies the blink where the scan hands the outputs to the drivers, and returns the bit of
    /// the output image that this publication sets, while the blink has an image output on.
    ///
    /// The scan never waits for a control request: when a request holds the identify state at this
    /// moment, the publication leaves the blink as it was and offers no bit. A failure to drive
    /// the output ends identify and is logged; it never fails the scan.
    pub(crate) fn publication(&self) -> Option<ImageBit> {
        let mut slot = match self.slot.try_lock() {
            Ok(slot) => slot,
            Err(TryLockError::Poisoned(poisoned)) => poisoned.into_inner(),
            Err(TryLockError::WouldBlock) => {
                self.bit_handed_out.store(false, Ordering::Release);
                return None;
            }
        };
        let identify = slot.as_mut()?;
        if let Err(error) = identify.tick() {
            tracing::warn!("identify output could not be driven, identify ends: {error}");
        }
        let bit = identify.output().image_bit_when_on();
        self.bit_handed_out.store(bit.is_some(), Ordering::Release);
        bit
    }

    /// Applies the blink in a pass in which no scan runs (the resource is paused, stopped or
    /// faulted), and says what the drivers are to be handed. A GPIO line is driven by the tick
    /// itself, so it needs nothing handed.
    ///
    /// An image output that `held_by_safe_state` says the safe state holds keeps its safe value:
    /// the run goes on, but the bit is not set. The pass never waits for a control request.
    pub(crate) fn idle_publication(
        &self,
        held_by_safe_state: impl FnOnce(&IoAddress) -> bool,
    ) -> IdleOutput {
        let mut slot = match self.slot.try_lock() {
            Ok(slot) => slot,
            Err(TryLockError::Poisoned(poisoned)) => poisoned.into_inner(),
            Err(TryLockError::WouldBlock) => return IdleOutput::Unchanged,
        };
        let Some(identify) = slot.as_mut() else {
            return IdleOutput::Unchanged;
        };
        if let Err(error) = identify.tick() {
            tracing::warn!("identify output could not be driven, identify ends: {error}");
        }
        let output = identify.output();
        let held = output.image_address().is_some_and(held_by_safe_state);
        let wanted = if held {
            None
        } else {
            output.image_bit_when_on()
        };
        if wanted.is_some() == self.bit_handed_out.load(Ordering::Acquire) {
            return IdleOutput::Unchanged;
        }
        self.bit_handed_out
            .store(wanted.is_some(), Ordering::Release);
        IdleOutput::Hand(wanted)
    }

    /// Records that the drivers were handed an output image without the identify bit, as the safe
    /// state of a stop or a fault does.
    pub(crate) fn image_handed_out_without_bit(&self) {
        self.bit_handed_out.store(false, Ordering::Release);
    }

    /// Whether `identify` runs now. The scheduler's paused wait polls faster while it does.
    pub(crate) fn is_identifying(&self) -> bool {
        self.slot
            .try_lock()
            .ok()
            .and_then(|slot| slot.as_ref().map(IdentifyState::is_identifying))
            .unwrap_or(false)
    }

    /// Ends the run and drives the output off, as the platform does when its process ends.
    /// Returns whether the drivers hold an output image with the bit set, which the caller then
    /// replaces by the committed image.
    pub(crate) fn end(&self) -> bool {
        let mut slot = self.lock();
        if let Some(identify) = slot.as_mut()
            && let Err(error) = identify.cancel()
        {
            tracing::warn!("identify output could not be driven off: {error}");
        }
        self.bit_handed_out.swap(false, Ordering::AcqRel)
    }

    /// The address of the output image bit that identify drives, or `None` when the runtime has
    /// no identify output or drives a GPIO line. A replacement program is checked against it.
    pub(crate) fn image_address(&self) -> Option<IoAddress> {
        self.lock().as_ref()?.output().image_address().cloned()
    }

    fn lock(&self) -> MutexGuard<'_, Option<HostIdentify>> {
        self.slot.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

#[cfg(test)]
mod tests;
