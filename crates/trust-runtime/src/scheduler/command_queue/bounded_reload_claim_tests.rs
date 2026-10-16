//! Bounded exhaustive check of the reload claim: every interleaving of take and withdraw from the
//! scan side and the caller side, for sequences of up to six calls.
//!
//! This file sits inside the scheduler because `ReloadClaim::take` and `withdraw` are `pub(crate)`,
//! so an integration test under `rt/tests` cannot call them.
//!
//! Specification: `docs/specs/11-runtime-engine.md`, section 6.7.1 ("Online-change transaction"): a
//! reload is applied only when the resource takes it before the caller withdraws it, and "a reload
//! therefore never commits after its caller was told that it timed out".
//!
//! Production under test: `ReloadClaim::take` (`scheduler/command_queue.rs:73`) and
//! `ReloadClaim::withdraw` (`scheduler/command_queue.rs:85`). One `AtomicU8` moves from PENDING to
//! TAKEN or to WITHDRAWN by compare-and-exchange; clones share that atomic.
//!
//! Model. The state is PENDING, TAKEN or WITHDRAWN. A take or a withdraw succeeds exactly when the
//! state is PENDING, and then the state becomes TAKEN or WITHDRAWN. Every other call fails and changes
//! nothing. Each call is one atomic read-modify-write, so any interleaving of the scan side (which
//! takes) and the caller side (which withdraws), each possibly retrying, is a sequence of such calls.
//! The original claim stands for the scan side and a clone for the caller side. The oracle is the
//! winning call, if any.
//!
//! Bounds. Four calls: take or withdraw through the original or through the clone. Every sequence of
//! one to six calls from a fresh pending claim is run: 4 + 16 + 64 + 256 + 1,024 + 4,096 = 5,460.
//!
//! Invariants checked on every call: it succeeds exactly when no earlier call has succeeded, so each
//! claim is taken or withdrawn exactly once, and later calls through either handle fail.

use super::ReloadClaim;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Call {
    Take,
    Withdraw,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Handle {
    Original,
    Cloned,
}

const CALLS: [(Handle, Call); 4] = [
    (Handle::Original, Call::Take),
    (Handle::Original, Call::Withdraw),
    (Handle::Cloned, Call::Take),
    (Handle::Cloned, Call::Withdraw),
];

#[test]
fn a_reload_claim_is_taken_or_withdrawn_exactly_once_in_every_interleaving() {
    let mut sequences = 0_u32;
    for length in 1..=6_u32 {
        for code in 0..CALLS.len().pow(length) {
            let original = ReloadClaim::default();
            let cloned = original.clone();
            let mut winner: Option<Call> = None;
            let mut digits = code;
            for _ in 0..length {
                let (handle, call) = CALLS[digits % CALLS.len()];
                digits /= CALLS.len();
                let claim = match handle {
                    Handle::Original => &original,
                    Handle::Cloned => &cloned,
                };
                let succeeded = match call {
                    Call::Take => claim.take(),
                    Call::Withdraw => claim.withdraw(),
                };
                assert_eq!(
                    succeeded,
                    winner.is_none(),
                    "{call:?} through {handle:?} after the winner {winner:?}"
                );
                if succeeded {
                    winner = Some(call);
                }
            }
            sequences += 1;
        }
    }
    assert_eq!(sequences, 5_460);
}
