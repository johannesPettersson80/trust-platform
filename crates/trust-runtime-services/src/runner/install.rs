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
//! check with its program store, built from [`check_retained_values`], which reads the newest
//! checkpoint through a handle on the checkpoint slots.
//!
//! The same state tells the loop which container its program came from, so that a `restart` of the
//! Stopped resource can find a newer committed container and load it in place
//! ([`Runner::with_program_loader`](crate::runner::Runner::with_program_loader)), and it answers
//! `program.active` from the store in bounded reads.

mod retained;

pub use retained::{check_install, check_retained_values, newest_checkpoint};

use alloc::boxed::Box;
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::cell::RefCell;

use smol_str::SmolStr;
use trust_runtime_core::deployment::DeviceDescriptor;
use trust_runtime_core::error::RuntimeError;
use trust_runtime_core::ports::ProgramStore;

use crate::installation::Admitted;
use crate::loading::identity::{self, IdentityError};
use crate::loading::install::{self, Install, InstallError, Progress};
use crate::loading::select::{self, Booted, ScanError, SelectError};
use crate::ports::{ProgramIdentities, ProgramIdentity, TransferSessionError};
use crate::protocol::ControlResponse;
use crate::transfer::TransferSession;

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
    /// The commit that runs, one step in each pass.
    pending: Option<Pending>,
    /// The request being answered began the commit; the loop takes this once it has answered.
    began: bool,
    /// What the loop knows of the container its program came from, once it can load another in place.
    reload: Option<Reload>,
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

/// A commit that runs, and the reply its request is owed.
struct Pending {
    commit: Install,
    /// Set by the loop right after the request is answered.
    reply: Option<Owed>,
}

/// The request that an install answers, and the reply it gets when the commit succeeds.
struct Owed {
    /// The request number, which the failure of a step is answered with.
    id: u64,
    /// The `installed` reply, written when the request was answered.
    installed: String,
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

    /// The identities `program.active` reports, read from the store in bounded reads. `None` when the
    /// loop keeps no store, and the platform answers.
    pub(super) fn identities(&self) -> Option<Result<ProgramIdentities, IdentityError>> {
        self.with(|parts| parts.store.as_mut().map(identity::identities))
            .flatten()
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
    /// The refusal as plain text: no program store, a commit that already runs, a container the
    /// store cannot take, or a heap that cannot hold a copy of the container.
    pub(super) fn begin(&self, container: &[u8]) -> Result<ProgramIdentity, String> {
        self.with(|parts| parts.begin(container))
            .unwrap_or_else(|| Err(String::from("the program store is in use")))
    }

    /// Whether the request that was answered began a commit. It is true once.
    pub(super) fn take_began(&self) -> bool {
        self.with(|parts| core::mem::take(&mut parts.began))
            .unwrap_or(false)
    }

    /// Holds `installed`, the reply to request `id`, until the commit ends. Returns false when no
    /// commit runs.
    pub(super) fn hold_reply(&self, id: u64, installed: String) -> bool {
        self.with(|parts| {
            parts.pending.as_mut().is_some_and(|pending| {
                pending.reply = Some(Owed { id, installed });
                true
            })
        })
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

    /// Begins the commit of `container` in the store.
    fn begin(&mut self, container: &[u8]) -> Result<ProgramIdentity, String> {
        let store = self
            .store
            .as_mut()
            .ok_or_else(|| String::from("the platform does not store programs"))?;
        if self.pending.is_some() {
            return Err(String::from("a program install is already running"));
        }
        let checked = install::Checked::of(container).map_err(|error| error.to_string())?;
        let commit = install::start(store, copy_of(container)?, checked)
            .map_err(|error| error.to_string())?;
        let committed = commit.identity();
        self.pending = Some(Pending {
            commit,
            reply: None,
        });
        self.began = true;
        Ok(ProgramIdentity {
            sequence: committed.sequence,
            program_sha256: committed.program_sha256,
            bytes: committed.program_bytes,
        })
    }

    /// Advances the commit by one step: the store's next erase, program, read-back or commit point.
    fn step(&mut self) -> Step {
        let Some(store) = self.store.as_mut() else {
            return Step::Idle;
        };
        let Some(Pending { commit, reply }) = self.pending.take() else {
            return Step::Idle;
        };
        match commit.step(store) {
            Ok(Progress::Pending(commit)) => {
                self.pending = Some(Pending { commit, reply });
                Step::Running
            }
            Ok(Progress::Committed(_)) => Step::Finished(reply.map(|owed| owed.installed)),
            Err(error) => Step::Finished(reply.map(|owed| failure_reply(owed.id, &error))),
        }
    }
}

/// A copy of `container` for the commit, which the heap may refuse. The commit keeps the container
/// until its last step, and the request's own copy ends with the request.
fn copy_of(container: &[u8]) -> Result<Vec<u8>, String> {
    let mut copy = Vec::new();
    copy.try_reserve_exact(container.len()).map_err(|_| {
        format!(
            "no room to install a container of {} bytes",
            container.len()
        )
    })?;
    copy.extend_from_slice(container);
    Ok(copy)
}

/// The reply to request `id` when a step of its commit failed: the step's error. The reply carries no
/// audit record id, because the request was audited when it was answered.
fn failure_reply(id: u64, error: &InstallError) -> String {
    serde_json::to_string(&ControlResponse::error(id, error.to_string())).unwrap_or_else(|_| {
        String::from(r#"{"id":0,"ok":false,"error":"response serialization failed"}"#)
    })
}
