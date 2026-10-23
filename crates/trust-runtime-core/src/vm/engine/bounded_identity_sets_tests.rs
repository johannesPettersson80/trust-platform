//! Bounded exhaustive model check of the engine's identity containers.
//!
//! This file sits inside the engine because the containers are `pub(super)` wrappers over the
//! sorted `Entries` table, which no integration test can reach.
//!
//! Specification: `docs/specs/34-runtime-portability.md`, Scope B-R3 (sorted live entries use binary
//! search, reserve appends in identity order, arbitrary public removals remain supported) and the
//! lifecycle-marker rule (ascending key order; every `u32` instance identity, including `u32::MAX`,
//! is representable; a refused insertion leaves the markers unchanged).
//!
//! Production under test: `InstanceSet` (`vm/engine/identities.rs:6`), `InstanceLifetimes`
//! (`vm/engine/identities.rs:66`) and `OwnedInstances` (`vm/engine/identities.rs:104`), over
//! `Entries` (`vm/engine/ordered.rs:5`), whose `insert` replaces and returns the previous value
//! (`vm/engine/ordered.rs:86`).
//!
//! Model. Each container is compared with the obvious ordered collection after every operation:
//! `InstanceSet` with a `BTreeSet<u32>` (insert reports a new identity, remove reports presence,
//! retain keeps the predicate, iteration is ascending); `InstanceLifetimes` with a `BTreeMap<u32,
//! Option<u32>>` (insert replaces, restore updates only an existing identity, remove deletes,
//! retain keeps the predicate); `OwnedInstances` with a `BTreeMap<u32, Vec<u32>>` (push appends in
//! order and keeps duplicates, take removes the owner's list in push order, count and snapshot read
//! it in order).
//!
//! Bounds. Identities {0, 1, u32::MAX} for the set and the owned lists; {0, u32::MAX} for lifetimes,
//! with owners {none, FrameId(0), FrameId(u32::MAX)} for lifetimes and {FrameId(0), FrameId(u32::MAX)}
//! for owned lists. Every sequence is run: 7 operations of length 5 (16,807), 15 operations of length
//! 4 (50,625) and 8 operations of length 5 (32,768).
//!
//! Assumptions. Growth is never refused in this domain, so every fallible insertion is expected to
//! succeed. Refusals are covered by the containers' own reservation tests.

use super::identities::{InstanceLifetimes, InstanceSet, OwnedInstances};
use crate::memory::{FrameId, InstanceId};
use alloc::{
    collections::{BTreeMap, BTreeSet},
    vec::Vec,
};

const SET_IDS: [u32; 3] = [0, 1, u32::MAX];
const LIFETIME_IDS: [u32; 2] = [0, u32::MAX];
const LIFETIME_OWNERS: [Option<u32>; 3] = [None, Some(0), Some(u32::MAX)];
const OWNER_IDS: [u32; 2] = [0, u32::MAX];
const OWNED_IDS: [u32; 3] = [0, 1, u32::MAX];

#[derive(Clone, Copy, Debug)]
enum SetOp {
    Insert(u32),
    Remove(u32),
    RetainOdd,
}

#[derive(Clone, Copy, Debug)]
enum LifetimeOp {
    Insert(u32, Option<u32>),
    Restore(u32, Option<u32>),
    Remove(u32),
    RetainUnowned,
}

#[derive(Clone, Copy, Debug)]
enum OwnedOp {
    Push(u32, u32),
    Take(u32),
}

fn set_ops() -> Vec<SetOp> {
    let mut ops: Vec<SetOp> = SET_IDS.iter().map(|&id| SetOp::Insert(id)).collect();
    ops.extend(SET_IDS.iter().map(|&id| SetOp::Remove(id)));
    ops.push(SetOp::RetainOdd);
    ops
}

fn lifetime_ops() -> Vec<LifetimeOp> {
    let mut ops = Vec::new();
    for &id in &LIFETIME_IDS {
        for &owner in &LIFETIME_OWNERS {
            ops.push(LifetimeOp::Insert(id, owner));
            ops.push(LifetimeOp::Restore(id, owner));
        }
        ops.push(LifetimeOp::Remove(id));
    }
    ops.push(LifetimeOp::RetainUnowned);
    ops
}

fn owned_ops() -> Vec<OwnedOp> {
    let mut ops = Vec::new();
    for &owner in &OWNER_IDS {
        for &id in &OWNED_IDS {
            ops.push(OwnedOp::Push(owner, id));
        }
    }
    ops.extend(OWNER_IDS.iter().map(|&owner| OwnedOp::Take(owner)));
    ops
}

#[test]
fn instance_sets_match_an_ordered_set_on_every_bounded_sequence() {
    let ops = set_ops();
    const LENGTH: u32 = 5;
    let mut sequences = 0_u32;
    for code in 0..ops.len().pow(LENGTH) {
        let mut set = InstanceSet::default();
        let mut model = BTreeSet::new();
        let mut digits = code;
        for _ in 0..LENGTH {
            let op = ops[digits % ops.len()];
            digits /= ops.len();
            match op {
                SetOp::Insert(id) => {
                    let inserted = set
                        .insert(InstanceId(id))
                        .expect("growth is not refused in this domain");
                    assert_eq!(inserted, model.insert(id), "{op:?}");
                }
                SetOp::Remove(id) => {
                    assert_eq!(set.remove(&InstanceId(id)), model.remove(&id), "{op:?}");
                }
                SetOp::RetainOdd => {
                    set.retain(|id| id.0 % 2 == 1);
                    model.retain(|id| *id % 2 == 1);
                }
            }
            assert_eq!(set.len(), model.len(), "{op:?}");
            let listed: Vec<u32> = set.iter().map(|id| id.0).collect();
            let expected: Vec<u32> = model.iter().copied().collect();
            assert_eq!(listed, expected, "ascending contents after {op:?}");
            for id in SET_IDS {
                assert_eq!(
                    set.contains(&InstanceId(id)),
                    model.contains(&id),
                    "identity {id} after {op:?}"
                );
            }
        }
        sequences += 1;
    }
    assert_eq!(sequences, 16_807);
}

#[test]
fn instance_lifetimes_match_an_ordered_map_on_every_bounded_sequence() {
    let ops = lifetime_ops();
    const LENGTH: u32 = 4;
    let mut sequences = 0_u32;
    for code in 0..ops.len().pow(LENGTH) {
        let mut lifetimes = InstanceLifetimes::default();
        let mut model: BTreeMap<u32, Option<u32>> = BTreeMap::new();
        let mut digits = code;
        for _ in 0..LENGTH {
            let op = ops[digits % ops.len()];
            digits /= ops.len();
            match op {
                LifetimeOp::Insert(id, owner) => {
                    lifetimes
                        .insert(InstanceId(id), owner.map(FrameId))
                        .expect("growth is not refused in this domain");
                    model.insert(id, owner);
                }
                LifetimeOp::Restore(id, owner) => {
                    lifetimes.restore(InstanceId(id), owner.map(FrameId));
                    if let Some(slot) = model.get_mut(&id) {
                        *slot = owner;
                    }
                }
                LifetimeOp::Remove(id) => {
                    lifetimes.remove(&InstanceId(id));
                    model.remove(&id);
                }
                LifetimeOp::RetainUnowned => {
                    lifetimes.retain(|_, owner| owner.is_none());
                    model.retain(|_, owner| owner.is_none());
                }
            }
            assert_eq!(lifetimes.len(), model.len(), "{op:?}");
            for id in LIFETIME_IDS {
                assert_eq!(
                    lifetimes.contains_key(&InstanceId(id)),
                    model.contains_key(&id),
                    "identity {id} after {op:?}"
                );
                assert_eq!(
                    lifetimes.get(&InstanceId(id)),
                    model.get(&id).copied().map(|owner| owner.map(FrameId)),
                    "owner of identity {id} after {op:?}"
                );
            }
        }
        sequences += 1;
    }
    assert_eq!(sequences, 50_625);
}

#[test]
fn owned_instance_lists_match_an_ordered_multimap_on_every_bounded_sequence() {
    let ops = owned_ops();
    const LENGTH: u32 = 5;
    let mut sequences = 0_u32;
    for code in 0..ops.len().pow(LENGTH) {
        let mut owned = OwnedInstances::default();
        let mut model: BTreeMap<u32, Vec<u32>> = BTreeMap::new();
        let mut digits = code;
        for _ in 0..LENGTH {
            let op = ops[digits % ops.len()];
            digits /= ops.len();
            match op {
                OwnedOp::Push(owner, id) => {
                    owned
                        .push(FrameId(owner), InstanceId(id))
                        .expect("growth is not refused in this domain");
                    model.entry(owner).or_default().push(id);
                }
                OwnedOp::Take(owner) => {
                    let taken: Vec<u32> = owned.take(FrameId(owner)).map(|id| id.0).collect();
                    let expected = model.remove(&owner).unwrap_or_default();
                    assert_eq!(taken, expected, "{op:?}");
                }
            }
            for owner in OWNER_IDS {
                let expected = model.get(&owner).cloned().unwrap_or_default();
                assert_eq!(
                    owned.count(FrameId(owner)),
                    expected.len(),
                    "count of owner {owner} after {op:?}"
                );
                let mut snapshot = Vec::new();
                owned.copy_snapshot(FrameId(owner), &mut snapshot);
                let listed: Vec<u32> = snapshot.iter().map(|id| id.0).collect();
                assert_eq!(listed, expected, "list of owner {owner} after {op:?}");
            }
        }
        sequences += 1;
    }
    assert_eq!(sequences, 32_768);
}
