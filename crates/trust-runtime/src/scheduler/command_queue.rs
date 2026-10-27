//! The bounded resource command queue and the reload claim (specification 11 sections 6.6, 6.7.1
//! and 6.9).
//!
//! Every request that reaches the scan thread goes through this queue. A sender never blocks: a
//! full queue refuses the command at once with [`CommandSendError::Full`], which the control plane
//! answers with the stable code `resource_busy`.

use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::mpsc::{sync_channel, Receiver, SyncSender, TrySendError};
use std::sync::Arc;

use trust_runtime_services::resource::commands::ReloadClaimState;

use super::ResourceCommand;

// The queue bound and its refusal are shared runtime code (spec 34 ARCH-11): a command beyond
// `RESOURCE_COMMAND_CAPACITY` is refused, not queued (specification 11 section 6.9).
pub use trust_runtime_services::resource::commands::{CommandSendError, RESOURCE_COMMAND_CAPACITY};

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

/// Decides whether one queued reload is applied or withdrawn. The caller withdraws it when its wait
/// ends first; the scan thread takes it before applying it. Exactly one of the two succeeds, so a
/// reload is never applied after its caller was told that it timed out (specification 11 section
/// 6.7.1). The states and their transitions are the shared `ReloadClaimState`; the scan thread and
/// the waiting caller share it as one byte moved by compare-and-exchange.
#[derive(Debug, Clone)]
pub struct ReloadClaim(Arc<AtomicU8>);

impl Default for ReloadClaim {
    fn default() -> Self {
        Self(Arc::new(AtomicU8::new(ReloadClaimState::Pending.code())))
    }
}

impl ReloadClaim {
    /// Scan-thread side: true when the reload may be applied. A withdrawn reload is never applied.
    pub(crate) fn take(&self) -> bool {
        self.decide(ReloadClaimState::take)
    }

    /// Caller side: true when the reload was withdrawn before the scan thread took it.
    pub(crate) fn withdraw(&self) -> bool {
        self.decide(ReloadClaimState::withdraw)
    }

    /// Moves the claim by `transition`, one of the shared claim transitions, in one atomic step;
    /// false when the transition refuses because the claim was decided already.
    fn decide(&self, transition: fn(ReloadClaimState) -> Option<ReloadClaimState>) -> bool {
        self.0
            .try_update(Ordering::AcqRel, Ordering::Acquire, |code| {
                ReloadClaimState::from_code(code)
                    .and_then(transition)
                    .map(ReloadClaimState::code)
            })
            .is_ok()
    }
}

#[cfg(test)]
mod bounded_reload_claim_tests;
