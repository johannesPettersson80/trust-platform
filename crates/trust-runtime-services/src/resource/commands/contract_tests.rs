//! Command semantics: the queue refusal texts, what each kind of command does while running and
//! while faulted, and the reload claim.

use alloc::string::ToString;
use alloc::vec::Vec;

use super::*;

const CLASSES: [CommandClass; 5] = [
    CommandClass::Pause,
    CommandClass::Resume,
    CommandClass::Reload,
    CommandClass::VariableWrite,
    CommandClass::Service,
];

#[test]
fn queue_refusals_have_stable_texts_and_a_fixed_bound() {
    assert_eq!(RESOURCE_COMMAND_CAPACITY, 1_024);
    assert_eq!(
        CommandSendError::Full.to_string(),
        "resource command queue is full"
    );
    assert_eq!(
        CommandSendError::Closed.to_string(),
        "command channel closed"
    );
}

#[test]
fn the_running_loop_pauses_resumes_and_applies_everything_else() {
    let effects = CLASSES
        .into_iter()
        .map(running_command_effect)
        .collect::<Vec<_>>();
    assert_eq!(
        effects,
        [
            RunningCommandEffect::Pause,
            RunningCommandEffect::Resume,
            RunningCommandEffect::Apply,
            RunningCommandEffect::Apply,
            RunningCommandEffect::Apply,
        ]
    );
}

#[test]
fn the_faulted_wait_ignores_pause_discards_writes_and_recovers_by_reload() {
    let effects = CLASSES
        .into_iter()
        .map(faulted_command_effect)
        .collect::<Vec<_>>();
    assert_eq!(
        effects,
        [
            FaultedCommandEffect::Ignore,
            FaultedCommandEffect::Ignore,
            FaultedCommandEffect::Recover,
            FaultedCommandEffect::Discard,
            FaultedCommandEffect::Apply,
        ]
    );
}

#[test]
fn a_reload_claim_is_decided_exactly_once_in_every_order() {
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum Call {
        Take,
        Withdraw,
    }

    // Every sequence of one to six calls from a pending claim: the first call wins and every
    // later call fails and changes nothing.
    let mut sequences = 0_u32;
    for length in 1..=6_u32 {
        for code in 0..2_usize.pow(length) {
            let mut state = ReloadClaimState::default();
            let mut winner = None;
            for position in 0..length {
                let call = if (code >> position) & 1 == 0 {
                    Call::Take
                } else {
                    Call::Withdraw
                };
                let next = match call {
                    Call::Take => state.take(),
                    Call::Withdraw => state.withdraw(),
                };
                assert_eq!(
                    next.is_some(),
                    winner.is_none(),
                    "{call:?} after {winner:?}"
                );
                if let Some(next) = next {
                    state = next;
                    winner = Some(call);
                }
            }
            let expected = match winner {
                Some(Call::Take) => ReloadClaimState::Taken,
                Some(Call::Withdraw) => ReloadClaimState::Withdrawn,
                None => ReloadClaimState::Pending,
            };
            assert_eq!(state, expected);
            sequences += 1;
        }
    }
    assert_eq!(sequences, 126);
}

#[test]
fn reload_claim_codes_are_one_byte_and_distinct() {
    assert_eq!(ReloadClaimState::Pending.code(), 0);
    assert_eq!(ReloadClaimState::Taken.code(), 1);
    assert_eq!(ReloadClaimState::Withdrawn.code(), 2);
}

#[test]
fn reload_claim_codes_decode_to_their_state_and_nothing_else() {
    for state in [
        ReloadClaimState::Pending,
        ReloadClaimState::Taken,
        ReloadClaimState::Withdrawn,
    ] {
        assert_eq!(ReloadClaimState::from_code(state.code()), Some(state));
    }
    for code in 3..=u8::MAX {
        assert_eq!(ReloadClaimState::from_code(code), None, "code {code}");
    }
}
