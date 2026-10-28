//! A chip's two firmware slots (specification 34 TARGET-04; specification 11, the firmware update
//! contract): truST on a chip is updated over the control connection as a program is installed.
//!
//! While the resource is Stopped, `transfer.begin {"for": "firmware"}` opens an upload that the loop
//! writes into the firmware slot the chip does not run from, in the steps of an upload written into
//! the program store (specification 34 §5.1), with the image's first bytes kept by the session.
//! `firmware.install {transfer}` then has the chip check the image where it lies, programs the kept
//! bytes and reads them back, selects the slot for the next boot on trial, writes the reply and resets
//! the chip into the new firmware. The new firmware confirms itself once it has booted; a firmware
//! that does not, because it panicked or hung, is replaced by the one before it at the next reset.
//! What the chip knows of its slots, `capabilities` reports ([`FirmwareFacts`]).
//!
//! The chip supplies its slots' flash and the chip-specific parts ([`FirmwareDevice`]); the rules
//! here run on every chip alike. Nothing here reads a link or a clock.

use alloc::boxed::Box;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use serde_json::{json, Value};
use trust_runtime_core::ports::{ProgramFlash, ProgramStore};
use trust_runtime_core::retain::checkpoint::Slot;

/// What a firmware image carries, as the chip's check read it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FirmwareImage {
    /// The truST version of the firmware.
    pub version: String,
}

/// What a chip knows of its firmware, which `capabilities` reports as `firmware` (specification 11,
/// the runtime device capabilities contract).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FirmwareFacts {
    /// The slot the firmware runs from.
    pub slot: Slot,
    /// The running firmware's truST version.
    pub version: String,
    /// Whether the firmware has confirmed itself: false while it runs on trial after an update, until
    /// its boot is done.
    pub confirmed: bool,
    /// The version of a newer firmware that the boot loader stopped booting because it did not confirm
    /// itself, when the other slot holds one.
    pub rolled_back: Option<String>,
}

impl FirmwareFacts {
    /// The `firmware` member of `capabilities`.
    #[must_use]
    pub fn to_json(&self) -> Value {
        json!({
            "slot": slot_name(self.slot),
            "version": self.version,
            "confirmed": self.confirmed,
            "rolled_back": self.rolled_back,
        })
    }
}

/// The name of a firmware slot in replies: `a` or `b`.
#[must_use]
pub const fn slot_name(slot: Slot) -> &'static str {
    match slot {
        Slot::A => "a",
        Slot::B => "b",
    }
}

/// The other slot.
#[must_use]
pub const fn other(slot: Slot) -> Slot {
    match slot {
        Slot::A => Slot::B,
        Slot::B => Slot::A,
    }
}

/// The chip-specific parts of a firmware update.
pub trait FirmwareDevice {
    /// The slot the running firmware runs from.
    fn running(&self) -> Slot;

    /// Checks the firmware image of `bytes` bytes written into `slot` of `flash`, whose first bytes,
    /// which the slot does not hold yet, are `head`, and returns what it carries.
    ///
    /// # Errors
    ///
    /// The refusal, as the text of the reply.
    fn check(
        &mut self,
        flash: &mut dyn ProgramFlash,
        slot: Slot,
        head: &[u8],
        bytes: usize,
    ) -> Result<FirmwareImage, String>;

    /// Selects `slot` for the next boot, on trial: the boot loader boots it, and boots the slot that
    /// runs now again when the new firmware is reset before it confirms itself. The loop then has the
    /// platform reset the chip (`RunnerPlatform::reset_device`).
    ///
    /// # Errors
    ///
    /// The flash failure, as the text of the reply.
    fn activate(&mut self, slot: Slot) -> Result<(), String>;
}

/// A chip's firmware slots, which the device gives its loop (specification 34 TARGET-04).
pub struct FirmwareStore {
    /// The flash of the two slots, `Slot::A` and `Slot::B`, and the step budget of one write.
    pub slots: ProgramStore,
    /// The chip-specific parts.
    pub device: Box<dyn FirmwareDevice>,
}

/// The reply to a `firmware.install` whose commit ended: the new firmware's version and slot, and
/// that the chip now resets into it.
#[must_use]
pub fn installed_reply(image: &FirmwareImage, slot: Slot) -> Value {
    json!({
        "status": "installed",
        "version": image.version,
        "slot": slot_name(slot),
        "restarting": true,
    })
}

/// The commit of a checked firmware upload, one step per pass of the loop: the kept first bytes are
/// programmed, then read back, then the slot is selected for the next boot.
#[derive(Debug)]
pub(crate) struct FirmwareCommit {
    slot: Slot,
    /// The upload's first bytes, padded with erased bytes to whole program units.
    head: Vec<u8>,
    phase: Phase,
}

/// Where a firmware commit is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Phase {
    Program,
    Verify,
    Activate,
}

/// How far a commit went in one step.
pub(crate) enum CommitStep {
    /// It goes on in the next pass.
    Pending(FirmwareCommit),
    /// The slot is selected: the chip resets into it once the reply is sent.
    Done,
}

impl FirmwareCommit {
    /// The commit of an image written into `slot` but for its first bytes `head`.
    pub(crate) fn new(store: &FirmwareStore, slot: Slot, head: &[u8]) -> Self {
        let unit = store.slots.flash.geometry().program_unit() as usize;
        let mut padded = head.to_vec();
        padded.resize(head.len().next_multiple_of(unit.max(1)), 0xFF);
        Self {
            slot,
            head: padded,
            phase: Phase::Program,
        }
    }

    /// Runs the next step.
    ///
    /// # Errors
    ///
    /// The failure of the step, as the text of the reply; the slot is then not selected, and the
    /// firmware that runs stays the one the chip boots (DEPLOY-03).
    pub(crate) fn step(mut self, store: &mut FirmwareStore) -> Result<CommitStep, String> {
        match self.phase {
            Phase::Program => {
                store
                    .slots
                    .flash
                    .program(self.slot, 0, &self.head)
                    .map_err(|_| String::from("the firmware slot's flash program failed"))?;
                self.phase = Phase::Verify;
            }
            Phase::Verify => {
                let mut read = alloc::vec![0u8; self.head.len()];
                store
                    .slots
                    .flash
                    .read(self.slot, 0, &mut read)
                    .map_err(|_| String::from("the firmware slot's flash read failed"))?;
                if read != self.head {
                    return Err("the programmed bytes do not read back as written".to_string());
                }
                self.phase = Phase::Activate;
            }
            Phase::Activate => {
                store.device.activate(self.slot)?;
                return Ok(CommitStep::Done);
            }
        }
        Ok(CommitStep::Pending(self))
    }
}

#[cfg(test)]
mod tests;
