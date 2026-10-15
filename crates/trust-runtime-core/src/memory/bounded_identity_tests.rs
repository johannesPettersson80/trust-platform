//! Bounded exhaustive check of instance identity allocation and release.
//!
//! This file sits inside the crate because the identity counter is private and the boundary case
//! needs `set_next_instance_id_for_test`, which exists only under `cfg(test)`.
//!
//! Specification: `docs/specs/34-runtime-portability.md`, HOST-02 (instance identities are 32-bit;
//! creating an instance beyond that space fails with `Overflow` and never wraps an identity or
//! panics), the lifecycle-marker rule (every `u32` instance identity, including `u32::MAX`, is
//! representable), and the Scope B-R3 lifetime rules (frame and instance identities keep their
//! monotonic u32 domain; removed identities stay invalid; capacity follows live entries; identity
//! exhaustion stays a checked fault).
//!
//! Production under test: `VariableStorage::try_create_instance` (`memory/instances.rs:6`),
//! `remove_instance` (`memory/instances.rs:49`) and `reset_runtime_values` (`memory/storage.rs:37`).
//!
//! Model. A monotonic counter issues identities. `create` takes the counter value and advances it.
//! `remove` frees a live identity, and while the counter runs a freed identity is never issued
//! again. `reset(restart = true)` clears every live instance and sets the counter to zero, as
//! `reset_runtime_values(true)` does; `reset(restart = false)` clears the instances and keeps the
//! counter. The oracle is the counter plus the set of live identities.
//!
//! Bounds. Six operations: create; remove identity 0, 1 or 2 (live, freed or never issued); reset
//! keeping the counter; reset restarting it. Every sequence of length six from an empty storage is
//! run: 6^6 = 46,656 sequences. Identities stay far below u32::MAX, so no exhaustion happens here.
//!
//! Invariants after every operation: a creation issues the counter value and never a live identity;
//! a removal reports an instance exactly when its identity is live; the live set equals the oracle;
//! a reset empties the live set.
//!
//! The boundary test at the end follows HOST-02 and the lifecycle-marker rule: identity `u32::MAX`
//! is issuable, and the creation after it fails with `Overflow`. The product fails one identity
//! early, at `u32::MAX` (`memory/instances.rs:11-14`), and `memory/live_tests.rs` pins that behaviour.
//! The two disagree; the boundary test keeps the specification's expectation and the report records
//! the contradiction.

use super::*;
use crate::error::RuntimeError;
use alloc::collections::BTreeSet;

/// One storage operation of a bounded sequence.
#[derive(Clone, Copy, Debug)]
enum Op {
    Create,
    Remove(u32),
    Reset { restart: bool },
}

const OPS: [Op; 6] = [
    Op::Create,
    Op::Remove(0),
    Op::Remove(1),
    Op::Remove(2),
    Op::Reset { restart: false },
    Op::Reset { restart: true },
];

/// The counter and the live identities that the production must match.
#[derive(Default)]
struct Model {
    next: u32,
    live: BTreeSet<u32>,
}

fn apply(storage: &mut VariableStorage, model: &mut Model, op: Op) {
    match op {
        Op::Create => {
            let expected = model.next;
            let created = storage
                .try_create_instance("FB")
                .expect("the counter is far from exhaustion");
            assert_eq!(created, InstanceId(expected), "{op:?}");
            assert!(
                model.live.insert(expected),
                "identity {expected} was issued while still live"
            );
            model.next += 1;
        }
        Op::Remove(id) => {
            let removed = storage.remove_instance(InstanceId(id)).is_some();
            assert_eq!(removed, model.live.remove(&id), "remove identity {id}");
        }
        Op::Reset { restart } => {
            storage.reset_runtime_values(restart);
            model.live.clear();
            if restart {
                model.next = 0;
            }
        }
    }
    for id in 0..=5 {
        assert_eq!(
            storage.get_instance(InstanceId(id)).is_some(),
            model.live.contains(&id),
            "identity {id} after {op:?}"
        );
    }
    assert_eq!(
        storage.instances().len(),
        model.live.len(),
        "live instance count after {op:?}"
    );
}

#[test]
fn identity_allocation_matches_a_monotonic_counter_on_every_bounded_sequence() {
    const LENGTH: u32 = 6;
    let mut sequences = 0_u32;
    for code in 0..OPS.len().pow(LENGTH) {
        let mut storage = VariableStorage::new();
        let mut model = Model::default();
        let mut digits = code;
        for _ in 0..LENGTH {
            apply(&mut storage, &mut model, OPS[digits % OPS.len()]);
            digits /= OPS.len();
        }
        sequences += 1;
    }
    assert_eq!(sequences, 46_656);
}

#[test]
fn the_allocator_hands_out_every_identity_below_u32_max_then_overflows() {
    // HOST-02: identities 0 through u32::MAX - 1 are handed out, u32::MAX never is, and the
    // creation after the last one fails with `Overflow` without wrapping or replacing an instance.
    let mut storage = VariableStorage::new();
    storage.set_next_instance_id_for_test(u32::MAX - 2);
    assert_eq!(
        storage.try_create_instance("LAST_A"),
        Ok(InstanceId(u32::MAX - 2))
    );
    assert_eq!(
        storage.try_create_instance("LAST_B"),
        Ok(InstanceId(u32::MAX - 1))
    );
    assert_eq!(
        storage.try_create_instance("BEYOND"),
        Err(RuntimeError::Overflow)
    );
    assert!(storage.get_instance(InstanceId(u32::MAX)).is_none());
    assert!(storage.get_instance(InstanceId(u32::MAX - 1)).is_some());
}
