//! The bounded resource command queue and the reload claim (specification 11 sections 6.6, 6.7.1
//! and 6.9).
//!
//! Every request that reaches the scan thread goes through this queue. A sender never blocks: a
//! full queue refuses the command at once with [`CommandSendError::Full`], which the control plane
//! answers with the stable code `resource_busy`.

use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::mpsc::{sync_channel, Receiver, SyncSender, TrySendError};
use std::sync::Arc;

use super::ResourceCommand;

/// Commands the resource queue holds. A command beyond this bound is refused, not queued
/// (specification 11 section 6.9).
pub const RESOURCE_COMMAND_CAPACITY: usize = 1_024;

/// Why the resource queue did not take a command. Nothing was queued in either case.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandSendError {
    /// The queue already holds [`RESOURCE_COMMAND_CAPACITY`] commands.
    Full,
    /// The scan thread no longer receives commands.
    Closed,
}

impl std::fmt::Display for CommandSendError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Full => formatter.write_str("resource command queue is full"),
            Self::Closed => formatter.write_str("command channel closed"),
        }
    }
}

impl std::error::Error for CommandSendError {}

/// Creates the bounded queue that carries commands to one scan thread.
pub(super) fn command_channel() -> (SyncSender<ResourceCommand>, Receiver<ResourceCommand>) {
    sync_channel(RESOURCE_COMMAND_CAPACITY)
}

/// Queues a command without blocking. A full queue or a closed receiver is returned as an error.
pub(super) fn try_queue(
    sender: &SyncSender<ResourceCommand>,
    command: ResourceCommand,
) -> Result<(), CommandSendError> {
    sender.try_send(command).map_err(|error| match error {
        TrySendError::Full(_) => CommandSendError::Full,
        TrySendError::Disconnected(_) => CommandSendError::Closed,
    })
}

const CLAIM_PENDING: u8 = 0;
const CLAIM_TAKEN: u8 = 1;
const CLAIM_WITHDRAWN: u8 = 2;

/// Decides whether one queued reload is applied or withdrawn. The caller withdraws it when its wait
/// ends first; the scan thread takes it before applying it. Exactly one of the two succeeds, so a
/// reload is never applied after its caller was told that it timed out (specification 11 section
/// 6.7.1).
#[derive(Debug, Clone)]
pub struct ReloadClaim(Arc<AtomicU8>);

impl Default for ReloadClaim {
    fn default() -> Self {
        Self(Arc::new(AtomicU8::new(CLAIM_PENDING)))
    }
}

impl ReloadClaim {
    /// Scan-thread side: true when the reload may be applied. A withdrawn reload is never applied.
    pub(crate) fn take(&self) -> bool {
        self.0
            .compare_exchange(
                CLAIM_PENDING,
                CLAIM_TAKEN,
                Ordering::AcqRel,
                Ordering::Acquire,
            )
            .is_ok()
    }

    /// Caller side: true when the reload was withdrawn before the scan thread took it.
    pub(crate) fn withdraw(&self) -> bool {
        self.0
            .compare_exchange(
                CLAIM_PENDING,
                CLAIM_WITHDRAWN,
                Ordering::AcqRel,
                Ordering::Acquire,
            )
            .is_ok()
    }
}

#[cfg(test)]
mod bounded_reload_claim_tests;
