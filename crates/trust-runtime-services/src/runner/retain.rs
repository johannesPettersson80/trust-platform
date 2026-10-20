//! A chip's retained values and the checkpoint log they are saved to (specification 34 §10,
//! specification 11 §6.7).
//!
//! [`RetainStore`] joins the shared capture (`trust_runtime_core::retain::capture::RetainCapture`)
//! to the checkpoint persister
//! (`trust_runtime_core::retain::checkpoint::CheckpointPersister`) over the platform's checkpoint
//! store. The installed program's cycle captures the retained values at the pre-output-sample
//! checkpoint and offers them to the persister ([`RetainStore::capture`]); the resource loop
//! advances a pending checkpoint by one step between scans ([`RetainStore::step`]); a stop
//! completes it before the resource reports Stopped ([`RetainStore::save`]); and a warm start
//! decodes the newest committed checkpoint with the shared codec ([`RetainStore::recover`]). A
//! failure a step or a start latched is handed out once, by [`RetainStore::capture`], and fails
//! that scan before physical outputs.

use alloc::boxed::Box;

use trust_runtime_core::error::RuntimeError;
use trust_runtime_core::ports::CheckpointStore;
use trust_runtime_core::retain::capture::RetainCapture;
use trust_runtime_core::retain::checkpoint::{
    CheckpointFlash, CheckpointIdentity, CheckpointLog, CheckpointPersister, FlashError,
    FlashGeometry, Slot,
};
use trust_runtime_core::retain::codec::decode_snapshot_payload_v3;
use trust_runtime_core::retain::RetainSnapshot;
use trust_runtime_core::value::Duration;
use trust_runtime_core::vm::{CycleAccess, RuntimeState};
use trust_runtime_core::watchdog::RetainMode;

/// The checkpoint configuration of an installed program (specification 34 §10).
pub struct RetainConfig {
    /// `[runtime.retain] mode`: `File` saves into the checkpoint store, `None` saves nothing, as a
    /// computer without a retain store.
    pub mode: RetainMode,
    /// The platform's checkpoint store.
    pub store: CheckpointStore,
    /// The identity of the installed application: the first 16 bytes of its bytecode hash and of
    /// its retain-schema identity (specification 34 §5.1).
    pub identity: CheckpointIdentity,
    /// `[runtime.retain] save_interval_ms`.
    pub save_interval: Duration,
    /// The admitted retain image size, reserving the persister's payload buffer.
    pub image_bytes: usize,
}

/// A chip's retained values and the checkpoint log they are saved to (specification 34 §10).
pub struct RetainStore {
    /// The shared capture holding the due rule and the staged STRN v3 payload.
    capture: RetainCapture,
    /// The persister over the platform's checkpoint flash, or `None` for `mode = "none"`.
    persister: Option<CheckpointPersister<OwnedFlash>>,
    /// The most bytes one persister step may program between two scans.
    program_budget: usize,
}

impl RetainStore {
    /// Opens the checkpoint log over the platform's flash, or, for `mode = "none"`, a store that
    /// persists nothing, as a computer without a retain store (specification 34 §10).
    ///
    /// # Errors
    ///
    /// [`RuntimeError::RetainStore`] or [`RuntimeError::RetainCorruption`] when the checkpoint
    /// flash cannot be read.
    pub fn open(config: RetainConfig) -> Result<Self, RuntimeError> {
        let program_budget = config.store.program_budget;
        if config.mode == RetainMode::None {
            return Ok(Self {
                capture: RetainCapture::new(),
                persister: None,
                program_budget,
            });
        }
        let log =
            CheckpointLog::open(OwnedFlash(config.store.flash)).map_err(RuntimeError::from)?;
        let mut capture = RetainCapture::new();
        capture.configure(Some(config.save_interval), Duration::ZERO);
        Ok(Self {
            capture,
            persister: Some(CheckpointPersister::new(
                log,
                config.identity,
                config.image_bytes,
            )),
            program_budget,
        })
    }

    /// Reserves the capture for a program's largest retained payload, `bytes`
    /// ([`PreparedModule::retained_payload_bytes`](trust_runtime_core::vm::PreparedModule::retained_payload_bytes)):
    /// the staged payload and the one a save leaves the next capture, so that no capture grows them
    /// (specification 34 MEM-09), and the program's memory figure counts them (MEM-12). The room
    /// already reserved is kept. A store that persists nothing captures nothing and reserves nothing.
    ///
    /// # Errors
    ///
    /// [`RuntimeError::Overflow`] when the heap has no room for them; the capture is unchanged.
    pub fn reserve(&mut self, bytes: usize) -> Result<(), RuntimeError> {
        if self.persister.is_none() {
            return Ok(());
        }
        self.capture.reserve(bytes)
    }

    /// Whether the store persists the retained values: a checkpoint log over the platform's flash,
    /// not `mode = "none"`.
    #[must_use]
    pub fn persists(&self) -> bool {
        self.persister.is_some()
    }

    /// Captures the finished scan's retained values at the pre-output-sample checkpoint and offers
    /// them to the persister, so a save begins when the project's interval says it is due and the
    /// image changed (specification 34 §10, specification 11 §6.7).
    ///
    /// # Errors
    ///
    /// The [`RuntimeError::RetainStore`] of a save that did not begin, or of one an earlier step
    /// latched: the caller fails this scan with it before physical outputs.
    pub fn capture(&mut self, access: &mut CycleAccess<'_, '_>) -> Result<(), RuntimeError> {
        let Some(persister) = self.persister.as_mut() else {
            return Ok(());
        };
        access.capture_retain(&mut self.capture)?;
        self.capture.mark_dirty();
        let now = access.logical_time();
        let offer = persister.offer(&mut self.capture, now);
        if let Some(error) = persister.take_failure() {
            return Err(error);
        }
        offer
    }

    /// Advances a pending checkpoint by at most one log step, with the platform's per-step program
    /// budget (specification 34 §10). The resource loop calls it between scans, never inside one. A
    /// failed step latches a retain-store failure, which the next scan's
    /// [`capture`](Self::capture) hands to the scan.
    pub fn step(&mut self) {
        let Some(persister) = self.persister.as_mut() else {
            return;
        };
        // The persister latches the failure, so it is not lost; the next capture hands it out.
        let _ = persister.step(self.program_budget);
    }

    /// Captures the values the program holds now and completes the pending checkpoint and the
    /// newest image, all steps at once, so the save completes before the resource reports Stopped
    /// (specification 34 §10, specification 11 §6.7).
    ///
    /// # Errors
    ///
    /// The [`RuntimeError::RetainStore`] of a save that did not complete; the newest committed
    /// checkpoint stays readable.
    pub fn save(&mut self, state: &mut RuntimeState<'_>) -> Result<(), RuntimeError> {
        let Some(persister) = self.persister.as_mut() else {
            return Ok(());
        };
        state.capture_retain(&mut self.capture)?;
        let now = state.current_time();
        let finish = persister.finish(&mut self.capture, now);
        if let Some(error) = persister.take_failure() {
            return Err(error);
        }
        finish
    }

    /// Decodes the newest committed checkpoint through the shared codec, or an empty snapshot when
    /// the log holds none, which starts the retained values with their declared initial values
    /// (specification 34 §10, specification 11 §6.7).
    ///
    /// The checkpoint's payload is the bare STRN v3 payload the shared codec writes, so it is
    /// decoded by the shared payload rule; the persister keeps it as the image last written when it
    /// was written for the installed program's identity, and an unchanged image is then not written
    /// again ([`RetainStore::capture`]).
    ///
    /// # Errors
    ///
    /// [`RuntimeError::RetainCorruption`] when the newest committed checkpoint fails its checks,
    /// which fails the warm start as a damaged retain file does; [`RuntimeError::RetainStore`] when
    /// the flash cannot be read.
    pub fn recover(&mut self) -> Result<RetainSnapshot, RuntimeError> {
        let Some(persister) = self.persister.as_mut() else {
            return Ok(RetainSnapshot::default());
        };
        // The checkpoint is read into the persister's own payload buffer, so a warm start takes no
        // second buffer of the largest payload (specification 34 MEM-09).
        match persister.recover_with(decode_snapshot_payload_v3)? {
            Some((_, snapshot)) => snapshot,
            None => Ok(RetainSnapshot::default()),
        }
    }
}

/// The platform's checkpoint flash, owned by the persister, which holds its flash by value.
struct OwnedFlash(Box<dyn CheckpointFlash>);

impl CheckpointFlash for OwnedFlash {
    fn geometry(&self) -> FlashGeometry {
        self.0.geometry()
    }

    fn read(&mut self, slot: Slot, offset: u32, bytes: &mut [u8]) -> Result<(), FlashError> {
        self.0.read(slot, offset, bytes)
    }

    fn program(&mut self, slot: Slot, offset: u32, bytes: &[u8]) -> Result<(), FlashError> {
        self.0.program(slot, offset, bytes)
    }

    fn erase(&mut self, slot: Slot, offset: u32) -> Result<(), FlashError> {
        self.0.erase(slot, offset)
    }
}
