//! The install of a program through the control link, in steps (specification 34 §5.1, §11.2,
//! PORT-05 and STATE-08).
//!
//! A device gives its loop a transfer session and a program store
//! ([`Runner::with_transfers`](crate::runner::Runner::with_transfers) and
//! [`Runner::with_program_store`](crate::runner::Runner::with_program_store)). The shared request rule
//! of `program.install` ([`crate::loading::program_install`]) then runs on the loop: it takes the
//! complete transfer, runs the checks, and begins the commit (`InstallState::begin`). The loop does
//! not answer the request then. The commit is the reply in flight (PORT-05): the loop reads no further
//! request from its link, advances the commit by one step in each pass (`InstallState::step`), and
//! writes the `installed` reply, or the failure of the step that failed, when the commit ends. The
//! reply waits in the loop and not in the link, because the link holds a reply only once it is whole.
//!
//! The retained-value check of the install (decision G13) is the device's own: it gives the loop the
//! check with its program store, built from [`check_image_retained_values`], which reads the newest
//! checkpoint through a handle on the checkpoint slots.
//!
//! While the resource is Stopped, an upload is written into the store as its parts arrive (§5.1): the
//! erase of `transfer.begin` and the write of each part run in steps as a commit does, and their
//! replies wait in the same way. `program.install` of such an upload checks it where it lies in its
//! slot and commits only its header and magic.
//!
//! The same state tells the loop which container its program came from, so that a `restart` of the
//! Stopped resource can find a newer committed container and load it in place
//! ([`Runner::with_program_loader`](crate::runner::Runner::with_program_loader)), and it answers
//! `program.active` from the store in bounded reads, which it keeps until the store is written.

mod retained;

pub use retained::{check_image_retained_values, newest_checkpoint, newest_checkpoint_payload_len};

use alloc::boxed::Box;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::cell::RefCell;

use smol_str::SmolStr;
use trust_runtime_core::deployment::DeviceDescriptor;
use trust_runtime_core::error::RuntimeError;
use trust_runtime_core::ports::ProgramStore;
use trust_runtime_core::retain::checkpoint::{FlashError, Slot};

use crate::firmware::{CommitStep, FirmwareCommit, FirmwareImage, FirmwareStore};
use crate::installation::Admitted;
use crate::loading::identity::{self, IdentityError};
use crate::loading::install::{self, Checked, Install, InstallError, Progress};
use crate::loading::select::{self, Booted, ScanError, SelectError};
use crate::loading::upload::{self, StoreWrite};
use crate::ports::{ProgramIdentities, ProgramIdentity, TransferSessionError};
use crate::protocol::ControlResponse;
use crate::transfer::{Received, StoredUpload, TransferError, TransferId, TransferSession};

/// What a pass of the loop learns of the commit that runs.
pub(super) enum Step {
    /// No commit runs.
    Idle,
    /// A step ran, and the commit goes on in the next pass.
    Running,
    /// The commit ended. The reply to write is the `installed` reply of its request, or the failure
    /// of the step that failed. A commit that nobody is owed a reply for has none, which is a defect
    /// of the loop.
    Finished(Option<String>),
    /// A firmware commit ended with its slot selected: the reply is written, and the chip resets into
    /// the new firmware once the link has sent it (specification 34 TARGET-04).
    FinishedReset(Option<String>),
    /// The install state was in use, which the call structure of the loop rules out.
    InUse,
}

/// What the loop installs a program with.
///
/// Each part is given by the device when it builds its loop. A loop without a part answers as a
/// platform without it: the transfer operations go to the platform's own session, and a loop without
/// a program store answers `program.install` with `unsupported request`.
#[derive(Default)]
pub(super) struct InstallState(RefCell<Parts>);

/// The parts of the install state, behind its one cell.
#[derive(Default)]
struct Parts {
    /// The transfers of this boot.
    transfers: Option<TransferSession>,
    /// The two program slots.
    store: Option<ProgramStore>,
    /// The device's retained-value check of an install (decision G13).
    retained_check: Option<RetainedCheck>,
    /// The chip's firmware slots, which a firmware upload is written into (TARGET-04).
    firmware: Option<FirmwareStore>,
    /// The upload written into the program store or the firmware slots, and its slot (§5.1).
    upload: Option<Upload>,
    /// The commit or the write that runs, one step in each pass.
    pending: Option<Pending>,
    /// The request being answered began the commit; the loop takes this once it has answered.
    began: bool,
    /// What the loop knows of the container its program came from, once it can load another in place.
    reload: Option<Reload>,
    /// The identities `program.active` reports, kept from their reading until the store is written
    /// (specification 34 §11.2).
    identities: Option<ProgramIdentities>,
    /// The request being answered asked for the device to reset after its reply.
    reset: bool,
}

/// An upload written into flash as its parts arrive (§5.1, TARGET-04).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Upload {
    transfer: TransferId,
    slot: Slot,
    /// Whether it is written into the firmware slots rather than the program store.
    firmware: bool,
}

/// What the loop knows of the container its program came from (specification 34 STATE-08).
struct Reload {
    /// The install sequence of the container that the loop's program was loaded from, or that the
    /// boot refused. `None` when the program came from no container, or the store held none.
    loaded: Option<u64>,
}

/// The retained-value check that the device gives the loop: the warm start of an admitted program with
/// the newest checkpoint (specification 11 §6.7.1).
type RetainedCheck = Box<dyn FnMut(&Admitted<'_>) -> Result<(), RuntimeError>>;

/// A commit or a write that runs, and the reply its request is owed.
struct Pending {
    work: Work,
    /// Set by the loop right after the request is answered.
    reply: Option<Owed>,
}

/// The flash work of a request, which runs one step in each pass.
enum Work {
    /// The commit of `program.install`.
    Commit(Install),
    /// The commit of `firmware.install` (TARGET-04).
    Firmware(FirmwareCommit),
    /// The erase of `transfer.begin` (`part` false) or the write of a part (`part` true) of the
    /// upload `transfer`, written into the program store or, with `firmware`, the firmware slots
    /// (§5.1).
    Upload {
        transfer: TransferId,
        write: StoreWrite,
        part: bool,
        firmware: bool,
    },
}

/// The request that the work answers, and the reply it gets when the work succeeds.
struct Owed {
    /// The request number, which the failure of a step is answered with.
    id: u64,
    /// The reply written when the request was answered: `installed`, or the transfer's.
    reply: String,
}

impl InstallState {
    /// Runs `operation` on the parts, or `None` when they are in use.
    fn with<R>(&self, operation: impl FnOnce(&mut Parts) -> R) -> Option<R> {
        let mut parts = self.0.try_borrow_mut().ok()?;
        Some(operation(&mut parts))
    }

    /// Gives the loop the transfer session of this boot.
    pub(super) fn set_transfers(&self, session: TransferSession) {
        let _ = self.with(|parts| parts.transfers = Some(session));
    }

    /// Gives the loop the program store, and the retained-value check that the device built.
    pub(super) fn set_store(&self, store: ProgramStore, retained_check: RetainedCheck) {
        let _ = self.with(|parts| {
            parts.store = Some(store);
            parts.retained_check = Some(retained_check);
            parts.identities = None;
        });
    }

    /// Whether the loop keeps a transfer session of its own.
    pub(super) fn keeps_transfers(&self) -> bool {
        self.with(|parts| parts.transfers.is_some())
            .unwrap_or(false)
    }

    /// Runs `use_session` on the loop's transfer session.
    ///
    /// # Errors
    ///
    /// [`TransferSessionError::Unsupported`] when the loop keeps no session, and
    /// [`TransferSessionError::Unavailable`] when the session is in use.
    pub(super) fn with_transfers<R>(
        &self,
        use_session: impl FnOnce(&mut TransferSession) -> R,
    ) -> Result<R, TransferSessionError> {
        let mut parts = self
            .0
            .try_borrow_mut()
            .map_err(|_| TransferSessionError::Unavailable)?;
        let session = parts
            .transfers
            .as_mut()
            .ok_or(TransferSessionError::Unsupported)?;
        Ok(use_session(session))
    }

    /// Whether the loop keeps a program store that `program.install` commits to.
    pub(super) fn has_store(&self) -> bool {
        self.with(|parts| parts.store.is_some()).unwrap_or(false)
    }

    /// Lets the loop load a newer container in place, and says which container its program came from:
    /// `loaded` is its install sequence, or `None` when the program came from no container or the store
    /// held none.
    pub(super) fn set_loaded(&self, loaded: Option<u64>) {
        let _ = self.with(|parts| parts.reload = Some(Reload { loaded }));
    }

    /// The install sequence of the store's newest committed container, which a boot that could not
    /// start its program counts as the container that it refused (specification 34 STATE-08). `None`
    /// when the loop keeps no store, the store holds no committed container, or its containers cannot
    /// be ordered.
    pub(super) fn newest_sequence(&self) -> Option<u64> {
        self.with(|parts| {
            let store = parts.store.as_mut()?;
            let newest = select::order(store).ok()?.newest?;
            Some(newest.header.sequence)
        })
        .flatten()
    }

    /// Whether the store holds a committed container newer than the one the loop's program came from,
    /// which a `restart` of the Stopped resource then loads. False for a loop that cannot load in place
    /// or keeps no store (specification 34 STATE-08).
    ///
    /// # Errors
    ///
    /// The [`ScanError`] of a store whose containers cannot be ordered.
    pub(super) fn newer_container(&self) -> Result<bool, ScanError> {
        self.with(Parts::newer_container).unwrap_or(Ok(false))
    }

    /// Reads the newest committed container within `room` bytes of memory and admits it, as the boot
    /// does. `None` when the loop keeps no store, or the store is in use.
    pub(super) fn boot(
        &self,
        device: &DeviceDescriptor,
        room: usize,
    ) -> Option<Result<Booted, SelectError>> {
        self.with(|parts| {
            parts
                .store
                .as_mut()
                .map(|store| select::boot(store, device, room))
        })
        .flatten()
    }

    /// The identities `program.active` reports: those read since the store was last written, or read
    /// from the store now in bounded reads. `None` when the loop keeps no store, and the platform
    /// answers.
    pub(super) fn identities(&self) -> Option<Result<ProgramIdentities, IdentityError>> {
        self.with(Parts::identities).flatten()
    }

    /// The retained-value check of an install (decision G13), which the device built: the warm start
    /// of `admitted` with the newest checkpoint.
    ///
    /// # Errors
    ///
    /// The check's own error: the warm start's, a retained-value error of a damaged checkpoint, or
    /// the error of a program that cannot be built for the check. A loop without a program store has
    /// no check, which `program.install` does not reach.
    pub(super) fn check_retained(&self, admitted: &Admitted<'_>) -> Result<(), RuntimeError> {
        self.with(|parts| match parts.retained_check.as_mut() {
            Some(check) => check(admitted),
            None => Err(RuntimeError::ControlError(SmolStr::new_static(
                "the platform does not check retained values",
            ))),
        })
        .unwrap_or_else(|| {
            Err(RuntimeError::ControlError(SmolStr::new_static(
                "the install state is in use",
            )))
        })
    }

    /// Begins the commit of `container`, which the install's checks have passed, and returns the
    /// identity the commit gives the program. No byte is written until the first step.
    ///
    /// # Errors
    ///
    /// The refusal as plain text: no program store, a commit that already runs, or a container the
    /// store cannot take.
    pub(super) fn begin(&self, container: Vec<u8>) -> Result<ProgramIdentity, String> {
        self.with(|parts| parts.begin(container))
            .unwrap_or_else(|| Err(String::from("the program store is in use")))
    }

    /// Whether the request that was answered began a commit. It is true once.
    pub(super) fn take_began(&self) -> bool {
        self.with(|parts| core::mem::take(&mut parts.began))
            .unwrap_or(false)
    }

    /// Holds `reply`, the reply to request `id`, until the work ends. Returns false when no work
    /// runs.
    pub(super) fn hold_reply(&self, id: u64, reply: String) -> bool {
        self.with(|parts| {
            parts.pending.as_mut().is_some_and(|pending| {
                pending.reply = Some(Owed { id, reply });
                true
            })
        })
        .unwrap_or(false)
    }

    /// Opens an upload of `bytes` bytes whose SHA-256 is `digest` in the loop's session (PORT-05).
    /// While the resource is Stopped (`stopped`) and the store maps its slots for reading in place,
    /// the upload is written into the store as its parts arrive (§5.1): its slot is chosen now, and
    /// the erase of the slot's first unit begins, whose step the reply waits for. Otherwise the
    /// upload is held in memory.
    ///
    /// # Errors
    ///
    /// [`TransferSessionError::Unsupported`] when the loop keeps no session, and
    /// [`TransferSessionError::Unavailable`] when its state is in use. The refusal of the upload
    /// itself is the inner error's text.
    pub(super) fn begin_upload(
        &self,
        bytes: u64,
        digest: [u8; 32],
        stopped: bool,
    ) -> Result<Result<TransferId, String>, TransferSessionError> {
        let mut guard = self
            .0
            .try_borrow_mut()
            .map_err(|_| TransferSessionError::Unavailable)?;
        let parts = &mut *guard;
        let session = parts
            .transfers
            .as_mut()
            .ok_or(TransferSessionError::Unsupported)?;
        // The install checks an upload in the store where it lies in its slot, so a store that cannot
        // be read in place keeps its uploads in memory.
        let in_place = stopped
            && parts
                .store
                .as_mut()
                .is_some_and(|store| store.flash.map(Slot::A, 0, 1).is_some());
        let Some(store) = parts.store.as_mut().filter(|_| in_place) else {
            return Ok(session
                .begin(bytes, digest)
                .map_err(|error| error.to_string()));
        };
        let held = parts.reload.as_ref().and_then(|reload| reload.loaded);
        let slot = match upload::slot(store, held) {
            Ok(slot) => slot,
            Err(error) => return Ok(Err(error.to_string())),
        };
        let transfer = match session.begin_in_store(bytes, digest) {
            Ok(transfer) => transfer,
            Err(error) => return Ok(Err(error.to_string())),
        };
        parts.upload = Some(Upload {
            transfer,
            slot,
            firmware: false,
        });
        parts.pending = Some(Pending {
            work: Work::Upload {
                transfer,
                write: StoreWrite::begin(store, slot),
                part: false,
                firmware: false,
            },
            reply: None,
        });
        parts.began = true;
        Ok(Ok(transfer))
    }

    /// Takes the part at `offset` of the open upload `transfer` from its base64 `encoded` with its
    /// CRC-32 `crc`, and returns the next offset (PORT-05). A part of an upload written into the
    /// store is checked now and written in steps, whose last step the reply waits for; the next
    /// offset is then the one after the part. A repeat of one of its received parts is compared with
    /// the bytes the slot holds, where they are mapped.
    ///
    /// # Errors
    ///
    /// As [`Self::begin_upload`].
    pub(super) fn receive_part(
        &self,
        transfer: TransferId,
        offset: u64,
        encoded: &str,
        crc: u32,
    ) -> Result<Result<usize, String>, TransferSessionError> {
        let mut guard = self
            .0
            .try_borrow_mut()
            .map_err(|_| TransferSessionError::Unavailable)?;
        let parts = &mut *guard;
        let session = parts
            .transfers
            .as_mut()
            .ok_or(TransferSessionError::Unsupported)?;
        let received = match session.part(transfer, offset, encoded, crc) {
            Ok(Received::Next(next)) => return Ok(Ok(next)),
            Ok(received) => received,
            Err(error) => return Ok(Err(error.to_string())),
        };
        let upload = parts.upload.filter(|upload| upload.transfer == transfer);
        let len = session.part_bytes(transfer).map_or(0, <[u8]>::len);
        let store = match upload {
            Some(Upload { firmware: true, .. }) => {
                parts.firmware.as_mut().map(|firmware| &mut firmware.slots)
            }
            _ => parts.store.as_mut(),
        };
        let (Some(store), Some(upload), Ok(at), Ok(length)) =
            (store, upload, u32::try_from(offset), u32::try_from(len))
        else {
            return Ok(Err(TransferError::InStore.to_string()));
        };
        if let Received::Write { offset } = received {
            parts.pending = Some(Pending {
                work: Work::Upload {
                    transfer,
                    write: StoreWrite::part(store, upload.slot, at, length),
                    part: true,
                    firmware: upload.firmware,
                },
                reply: None,
            });
            parts.began = true;
            return Ok(Ok(offset + len));
        }
        // A repeat is compared with the bytes where the slot maps them, or as read from it.
        let stored = if let Some(mapped) = store.flash.map(upload.slot, at, length) {
            mapped.to_vec()
        } else {
            let mut read = alloc::vec![0u8; len];
            if store.flash.read(upload.slot, at, &mut read).is_err() {
                return Ok(Err(InstallError::Flash(FlashError::Read).to_string()));
            }
            read
        };
        Ok(session
            .repeated(transfer, &stored)
            .map_err(|error| error.to_string()))
    }

    /// The bytes of the complete upload `upload` where they lie in the slot it was written into, for
    /// the install's checks (§11.2). `None` when the store holds no such upload or cannot map it.
    pub(super) fn stored_region(&self, upload: &StoredUpload) -> Option<&'static [u8]> {
        self.with(|parts| {
            let written = parts
                .upload
                .filter(|written| written.transfer == upload.transfer && !written.firmware)?;
            let len = u32::try_from(upload.bytes).ok()?;
            parts.store.as_mut()?.flash.map(written.slot, 0, len)
        })
        .flatten()
    }

    /// Begins the commit of `upload`, written into the store and checked as `checked`, and returns
    /// the identity the commit gives the program: the commit programs only its header and magic
    /// (§5.1).
    ///
    /// # Errors
    ///
    /// The refusal as plain text: no program store, work that already runs, an upload the loop did
    /// not write, or a container the store cannot take.
    pub(super) fn begin_stored(
        &self,
        upload: StoredUpload,
        checked: Checked,
    ) -> Result<ProgramIdentity, String> {
        self.with(|parts| parts.begin_stored(upload, checked))
            .unwrap_or_else(|| Err(String::from("the program store is in use")))
    }

    /// Ends the upload written into the store, if one is open: a `restart` that starts the resource
    /// ends it, so no part of it is written while the program runs (PORT-05).
    pub(super) fn end_store_upload(&self) {
        let _ = self.with(|parts| {
            if let Some(session) = parts.transfers.as_mut() {
                session.end_store_upload();
            }
            parts.upload = None;
        });
    }

    /// Gives the loop the chip's firmware slots (specification 34 TARGET-04).
    pub(super) fn set_firmware(&self, firmware: FirmwareStore) {
        let _ = self.with(|parts| parts.firmware = Some(firmware));
    }

    /// Whether the device gave its loop firmware slots.
    pub(super) fn has_firmware(&self) -> bool {
        self.with(|parts| parts.firmware.is_some()).unwrap_or(false)
    }

    /// Whether `transfer` is an upload written into the firmware slots.
    pub(super) fn is_firmware_upload(&self, transfer: TransferId) -> bool {
        self.with(|parts| {
            parts
                .upload
                .is_some_and(|upload| upload.transfer == transfer && upload.firmware)
        })
        .unwrap_or(false)
    }

    /// Opens an upload of `bytes` bytes whose SHA-256 is `digest` that the loop writes into the
    /// firmware slot the chip does not run from as its parts arrive (TARGET-04, §5.1), and begins the
    /// erase of the slot's first unit, whose step the reply waits for. The caller has checked that the
    /// resource is Stopped.
    ///
    /// # Errors
    ///
    /// As [`Self::begin_upload`]; a loop without firmware slots refuses with `unsupported request`,
    /// and an upload longer than a slot with `transfer too large`.
    pub(super) fn begin_firmware_upload(
        &self,
        bytes: u64,
        digest: [u8; 32],
    ) -> Result<Result<TransferId, String>, TransferSessionError> {
        let mut guard = self
            .0
            .try_borrow_mut()
            .map_err(|_| TransferSessionError::Unavailable)?;
        let parts = &mut *guard;
        let session = parts
            .transfers
            .as_mut()
            .ok_or(TransferSessionError::Unsupported)?;
        let Some(firmware) = parts.firmware.as_mut() else {
            return Ok(Err(String::from("unsupported request")));
        };
        let max = firmware.slots.flash.geometry().slot_len();
        if bytes > u64::from(max) {
            return Ok(Err(alloc::format!(
                "transfer too large: {bytes}, the firmware slot takes at most {max}"
            )));
        }
        let slot = crate::firmware::other(firmware.device.running());
        let transfer = match session.begin_in_store(bytes, digest) {
            Ok(transfer) => transfer,
            Err(error) => return Ok(Err(error.to_string())),
        };
        parts.upload = Some(Upload {
            transfer,
            slot,
            firmware: true,
        });
        parts.pending = Some(Pending {
            work: Work::Upload {
                transfer,
                write: StoreWrite::begin(&firmware.slots, slot),
                part: false,
                firmware: true,
            },
            reply: None,
        });
        parts.began = true;
        Ok(Ok(transfer))
    }

    /// Has the chip check the firmware `upload` written into its slot, and begins the commit that
    /// programs its first bytes and selects the slot for the next boot (TARGET-04). Returns the image
    /// and the slot.
    ///
    /// # Errors
    ///
    /// The refusal as plain text: no firmware slots, work that already runs, an upload the loop did
    /// not write into the firmware slots, or the chip's refusal of the image.
    pub(super) fn begin_firmware(
        &self,
        upload: &StoredUpload,
    ) -> Result<(FirmwareImage, Slot), String> {
        self.with(|parts| parts.begin_firmware(upload))
            .unwrap_or_else(|| Err(String::from("the firmware slots are in use")))
    }

    /// Asks for the device to reset once the reply being written is sent (`device.reset`).
    pub(super) fn request_reset(&self) {
        let _ = self.with(|parts| parts.reset = true);
    }

    /// Whether the request just answered asked for a reset; true once.
    pub(super) fn take_reset_request(&self) -> bool {
        self.with(|parts| core::mem::take(&mut parts.reset))
            .unwrap_or(false)
    }

    /// Whether a commit runs, so that its reply is the one in flight and the loop reads no request.
    pub(super) fn reply_pending(&self) -> bool {
        self.with(|parts| parts.pending.is_some()).unwrap_or(false)
    }

    /// Advances the commit by one step.
    pub(super) fn step(&self) -> Step {
        self.with(Parts::step).unwrap_or(Step::InUse)
    }
}

impl Parts {
    /// The identities of the store: those kept since it was last written, or read now and kept. Only
    /// a step of the work writes the store, and every step drops them (specification 34 §11.2).
    fn identities(&mut self) -> Option<Result<ProgramIdentities, IdentityError>> {
        let store = self.store.as_mut()?;
        if let Some(kept) = self.identities {
            return Some(Ok(kept));
        }
        let read = identity::identities(store);
        if let Ok(identities) = read {
            self.identities = Some(identities);
        }
        Some(read)
    }

    /// Whether the store's newest committed container is newer than the one the program came from.
    fn newer_container(&mut self) -> Result<bool, ScanError> {
        let (Some(store), Some(reload)) = (self.store.as_mut(), self.reload.as_ref()) else {
            return Ok(false);
        };
        let newest = select::order(store)?.newest;
        Ok(newest.is_some_and(|newest| {
            reload
                .loaded
                .is_none_or(|loaded| newest.header.sequence > loaded)
        }))
    }

    /// Begins the commit of `container` in the store, which keeps it until the commit's last step.
    fn begin(&mut self, container: Vec<u8>) -> Result<ProgramIdentity, String> {
        let store = self
            .store
            .as_mut()
            .ok_or_else(|| String::from("the platform does not store programs"))?;
        if self.pending.is_some() {
            return Err(String::from("a program install is already running"));
        }
        let checked = install::Checked::of(&container).map_err(|error| error.to_string())?;
        // The held program's slot is never written (specification 34 §5.1, MEM-14).
        let held = self.reload.as_ref().and_then(|reload| reload.loaded);
        let commit =
            install::start(store, container, checked, held).map_err(|error| error.to_string())?;
        Ok(self.run_commit(commit))
    }

    /// Begins the commit of `upload`, written into the store (§5.1).
    fn begin_stored(
        &mut self,
        upload: StoredUpload,
        checked: Checked,
    ) -> Result<ProgramIdentity, String> {
        let store = self
            .store
            .as_mut()
            .ok_or_else(|| String::from("the platform does not store programs"))?;
        if self.pending.is_some() {
            return Err(String::from("a program install is already running"));
        }
        let Some(written) = self
            .upload
            .take()
            .filter(|written| written.transfer == upload.transfer && !written.firmware)
        else {
            return Err(TransferError::Unknown.to_string());
        };
        let commit =
            install::start_stored(store, written.slot, upload.header, upload.bytes, checked)
                .map_err(|error| error.to_string())?;
        Ok(self.run_commit(commit))
    }

    /// Checks the firmware `upload` and begins its commit (TARGET-04).
    fn begin_firmware(&mut self, upload: &StoredUpload) -> Result<(FirmwareImage, Slot), String> {
        let firmware = self
            .firmware
            .as_mut()
            .ok_or_else(|| String::from("unsupported request"))?;
        if self.pending.is_some() {
            return Err(String::from("a firmware install is already running"));
        }
        let Some(written) = self
            .upload
            .take()
            .filter(|written| written.transfer == upload.transfer && written.firmware)
        else {
            return Err(TransferError::Unknown.to_string());
        };
        let image = firmware.device.check(
            &mut *firmware.slots.flash,
            written.slot,
            &upload.header,
            upload.bytes,
        )?;
        let commit = FirmwareCommit::new(firmware, written.slot, &upload.header);
        self.pending = Some(Pending {
            work: Work::Firmware(commit),
            reply: None,
        });
        self.began = true;
        Ok((image, written.slot))
    }

    /// Makes `commit` the work that runs, and returns the identity it gives the program.
    fn run_commit(&mut self, commit: Install) -> ProgramIdentity {
        let committed = commit.identity();
        self.pending = Some(Pending {
            work: Work::Commit(commit),
            reply: None,
        });
        self.began = true;
        ProgramIdentity {
            sequence: committed.sequence,
            program_sha256: committed.program_sha256,
            bytes: committed.program_bytes,
        }
    }

    /// Advances the work by one step: the store's next erase, program, read-back or commit point.
    fn step(&mut self) -> Step {
        let Some(Pending { work, reply }) = self.pending.take() else {
            return Step::Idle;
        };
        let (transfer, mut write, part, firmware) = match work {
            Work::Commit(commit) => {
                let Some(store) = self.store.as_mut() else {
                    return Step::Finished(None);
                };
                // The step may write the store, so the identities it held are read again when asked
                // for.
                self.identities = None;
                return match commit.step(store) {
                    Ok(Progress::Pending(commit)) => {
                        self.pending = Some(Pending {
                            work: Work::Commit(commit),
                            reply,
                        });
                        Step::Running
                    }
                    Ok(Progress::Committed(_)) => Step::Finished(reply.map(|owed| owed.reply)),
                    Err(error) => {
                        Step::Finished(reply.map(|owed| failure_reply(owed.id, &error.to_string())))
                    }
                };
            }
            Work::Firmware(commit) => {
                let Some(store) = self.firmware.as_mut() else {
                    return Step::Finished(None);
                };
                return match commit.step(store) {
                    Ok(CommitStep::Pending(commit)) => {
                        self.pending = Some(Pending {
                            work: Work::Firmware(commit),
                            reply,
                        });
                        Step::Running
                    }
                    Ok(CommitStep::Done) => Step::FinishedReset(reply.map(|owed| owed.reply)),
                    Err(error) => Step::Finished(reply.map(|owed| failure_reply(owed.id, &error))),
                };
            }
            Work::Upload {
                transfer,
                write,
                part,
                firmware,
            } => (transfer, write, part, firmware),
        };
        let store = if firmware {
            self.firmware.as_mut().map(|firmware| &mut firmware.slots)
        } else {
            self.identities = None;
            self.store.as_mut()
        };
        let Some(store) = store else {
            return Step::Finished(None);
        };
        let Some(session) = self.transfers.as_mut() else {
            return Step::Finished(None);
        };
        // A begin writes no bytes; a part writes the bytes that wait in the session's part buffer.
        let bytes = if part {
            session.part_bytes(transfer)
        } else {
            Some(&[][..])
        };
        let Some(bytes) = bytes else {
            return Step::Finished(None);
        };
        match write.step(store, bytes) {
            Ok(false) => {
                self.pending = Some(Pending {
                    work: Work::Upload {
                        transfer,
                        write,
                        part,
                        firmware,
                    },
                    reply,
                });
                Step::Running
            }
            Ok(true) if part => match session.written(transfer) {
                Ok(_) => Step::Finished(reply.map(|owed| owed.reply)),
                Err(error) => {
                    Step::Finished(reply.map(|owed| failure_reply(owed.id, &error.to_string())))
                }
            },
            Ok(true) => Step::Finished(reply.map(|owed| owed.reply)),
            Err(error) => {
                // A begin whose erase failed leaves no upload open; a part whose write failed leaves
                // its upload at the same next offset, so the client sends the part again.
                if !part {
                    let _ = session.abort(transfer);
                    self.upload = None;
                }
                Step::Finished(reply.map(|owed| failure_reply(owed.id, &error.to_string())))
            }
        }
    }
}

/// The reply to request `id` when a step of its work failed: the step's error. The reply carries no
/// audit record id, because the request was audited when it was answered.
fn failure_reply(id: u64, error: &str) -> String {
    serde_json::to_string(&ControlResponse::error(id, error.to_string())).unwrap_or_else(|_| {
        String::from(r#"{"id":0,"ok":false,"error":"response serialization failed"}"#)
    })
}
