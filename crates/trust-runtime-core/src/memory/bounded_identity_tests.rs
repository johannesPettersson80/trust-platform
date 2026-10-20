//! Bounded exhaustive check of instance identity allocation and release.
//!
//! This file sits inside the crate because the identity counter is private and the boundary case
//! needs `set_next_instance_id_for_test`, which exists only under `cfg(test)`.
//!
//! Specification: `docs/specs/34-runtime-portability.md`, HOST-02 (instance identities are 32-bit;
//! creating an instance beyond that space fails with `Overflow` and never wraps an identity or
//! panics; a later top-level entry issues again only identities released inside the entry before
//! it), §4.3.1 (activation and instance identities per top-level entry), the lifecycle-marker
//! rule (every `u32` instance identity, including `u32::MAX`, is representable), and the Scope B-R3
//! lifetime rules (capacity follows live entries; identity exhaustion stays a checked fault).
//!
//! Production under test: `VariableStorage::try_create_instance`, `remove_instance`,
//! `begin_activation_entry` with the counter's entry floor (the one counter the flat store's
//! `CellStorage::begin_execution_entry` begins an entry on) and `reset_runtime_values`.
//!
//! Model. A counter issues identities. `create` takes the counter value and advances it. `remove`
//! frees a live identity. A floor marks where the current top-level entry started. `begin entry`
//! moves the counter back to the floor when no live identity is at or above it, so the identities
//! the finished entry released are issued again; otherwise the floor moves up to the counter and
//! every live identity stays below it. `reset(restart = true)` clears every live instance and sets
//! the counter to zero, as `reset_runtime_values(true)` does; `reset(restart = false)` clears the
//! instances and keeps the counter. Both resets put the floor at the counter. The oracle is the
//! counter, the floor and the set of live identities.
//!
//! Bounds. Seven operations: create; remove identity 0, 1 or 2 (live, freed or never issued); begin
//! an entry; reset keeping the counter; reset restarting it. Every sequence of length six from an
//! empty storage is run: 7^6 = 117,649 sequences. Identities stay far below u32::MAX, so no
//! exhaustion happens here.
//!
//! Invariants after every operation: a creation issues the oracle's counter value and never a live
//! identity; a removal reports an instance exactly when its identity is live; the live set equals
//! the oracle; a reset empties the live set.
//!
//! The boundary test at the end follows HOST-02: identities up to `u32::MAX - 1` are issued, and
//! the creation after the last one fails with `Overflow` without wrapping or replacing an instance.

use super::*;
use crate::error::RuntimeError;
use alloc::collections::BTreeSet;

/// One storage operation of a bounded sequence.
#[derive(Clone, Copy, Debug)]
enum Op {
    Create,
    Remove(u32),
    BeginEntry,
    Reset { restart: bool },
}

const OPS: [Op; 7] = [
    Op::Create,
    Op::Remove(0),
    Op::Remove(1),
    Op::Remove(2),
    Op::BeginEntry,
    Op::Reset { restart: false },
    Op::Reset { restart: true },
];

/// The counter, the entry floor and the live identities that the production must match.
#[derive(Default)]
struct Model {
    next: u32,
    floor: u32,
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
        Op::BeginEntry => {
            storage
                .begin_activation_entry()
                .expect("no activation is reserved in this model");
            storage.identities.begin_entry();
            if model.live.range(model.floor..).next().is_none() {
                model.next = model.floor;
            } else {
                model.floor = model.next;
            }
        }
        Op::Reset { restart } => {
            storage.reset_runtime_values(restart);
            model.live.clear();
            if restart {
                model.next = 0;
            }
            model.floor = model.next;
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
fn identity_allocation_matches_the_entry_counter_model_on_every_bounded_sequence() {
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
    assert_eq!(sequences, 117_649);
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
