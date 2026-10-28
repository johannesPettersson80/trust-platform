//! The Live Values queue contract, with literal addresses, names and values (specification 11,
//! "Live Values" and "request queue full"; specification 34 §4.3.1).
//!
//! Every expected value here is worked out from the specification, not from the queue: a bounded
//! queue of 1,024 entries, an upsert that keeps the first position, an arrival-ordered drain, and
//! a forced snapshot that copies both force lists.

use alloc::format;
use alloc::vec;
use alloc::vec::Vec;

use trust_runtime_core::io_address::IoAddress;
use trust_runtime_core::memory::{FrameId, Generation, InstanceId};
use trust_runtime_core::value::Value;

use super::{ForcedVarTarget, LiveValueQueues, PendingVarTarget, REQUEST_QUEUE_CAPACITY};

fn empty_queues() -> LiveValueQueues {
    LiveValueQueues::new()
}

fn address(text: &str) -> IoAddress {
    IoAddress::parse(text).expect("test address")
}

#[test]
fn a_second_write_to_the_same_target_replaces_the_first_in_place() {
    let mut queues = empty_queues();
    queues
        .try_upsert_var_write(PendingVarTarget::Global("Speed".into()), Value::LInt(10))
        .expect("the first write fits the empty queue");
    queues
        .try_upsert_var_write(PendingVarTarget::Retain("Count".into()), Value::LInt(20))
        .expect("a second target fits");
    queues
        .try_upsert_var_write(PendingVarTarget::Global("Speed".into()), Value::LInt(30))
        .expect("replacing a queued target is never refused");

    let writes = queues.drain_var_writes();
    assert_eq!(writes.len(), 2, "a replace must not add an entry");
    assert!(
        matches!(&writes[0].target, PendingVarTarget::Global(name) if name == "Speed"),
        "the first target keeps the first position"
    );
    assert_eq!(writes[0].value, Value::LInt(30));
    assert!(matches!(&writes[1].target, PendingVarTarget::Retain(name) if name == "Count"));
    assert_eq!(writes[1].value, Value::LInt(20));
}

#[test]
fn drain_returns_writes_in_arrival_order_and_empties_the_queue() {
    let mut queues = empty_queues();
    queues
        .try_enqueue_io_write(address("%QW0"), Value::LInt(1))
        .expect("the first write fits");
    queues
        .try_enqueue_io_write(address("%QW2"), Value::LInt(2))
        .expect("the second write fits");
    queues
        .try_enqueue_io_write(address("%QW0"), Value::LInt(3))
        .expect("a repeated address is a separate write");

    let writes = queues.drain_io_writes();
    assert_eq!(
        writes,
        vec![
            (address("%QW0"), Value::LInt(1)),
            (address("%QW2"), Value::LInt(2)),
            (address("%QW0"), Value::LInt(3)),
        ],
        "one-shot I/O writes keep arrival order and duplicates"
    );
    assert_eq!(
        queues.drain_io_writes(),
        Vec::<(IoAddress, Value)>::new(),
        "the drain empties the queue"
    );
}

#[test]
fn the_1025th_distinct_write_is_refused_with_request_queue_full() {
    let mut queues = empty_queues();
    for index in 0..REQUEST_QUEUE_CAPACITY {
        queues
            .try_upsert_var_write(
                PendingVarTarget::Global(format!("v{index}").into()),
                Value::LInt(1),
            )
            .expect("a write within the bound fits");
    }

    let refused =
        queues.try_upsert_var_write(PendingVarTarget::Global("v1024".into()), Value::LInt(1));
    let error = refused.expect_err("the 1,025th distinct target must be refused");
    assert_eq!(format!("{error}"), "request queue full");

    queues
        .try_upsert_var_write(PendingVarTarget::Global("v0".into()), Value::LInt(2))
        .expect("replacing a queued target is never refused");

    let writes = queues.drain_var_writes();
    assert_eq!(
        writes.len(),
        REQUEST_QUEUE_CAPACITY,
        "the refused write must not be queued"
    );
    assert_eq!(
        writes[0].value,
        Value::LInt(2),
        "the re-write replaces v0 in place"
    );
    assert!(
        writes.iter().all(
            |write| !matches!(&write.target, PendingVarTarget::Global(name) if name == "v1024")
        ),
        "v1024 must not be queued"
    );
}

#[test]
fn a_force_replaces_an_earlier_force_and_release_removes_it() {
    let mut queues = empty_queues();
    queues
        .try_upsert_forced_var(ForcedVarTarget::Global("Speed".into()), Value::LInt(1))
        .expect("the first force fits");
    queues
        .try_upsert_forced_var(ForcedVarTarget::Global("Count".into()), Value::LInt(2))
        .expect("a second target fits");
    queues
        .try_upsert_forced_var(ForcedVarTarget::Global("Speed".into()), Value::LInt(3))
        .expect("re-forcing a forced target is never refused");

    let forced = queues.forced_snapshot();
    assert_eq!(forced.vars.len(), 2, "a re-force must not add an entry");
    assert!(
        matches!(&forced.vars[0].target, ForcedVarTarget::Global(name) if name == "Speed"),
        "the first target keeps the first position"
    );
    assert_eq!(forced.vars[0].value, Value::LInt(3));
    assert!(matches!(&forced.vars[1].target, ForcedVarTarget::Global(name) if name == "Count"));

    queues.clear_forced_vars(
        |target| matches!(target, ForcedVarTarget::Global(name) if name == "Speed"),
    );

    let after_release = queues.forced_snapshot();
    assert_eq!(
        after_release.vars.len(),
        1,
        "the release removes exactly the named target"
    );
    assert!(
        matches!(&after_release.vars[0].target, ForcedVarTarget::Global(name) if name == "Count")
    );
    assert_eq!(after_release.vars[0].value, Value::LInt(2));
}

#[test]
fn withdraw_removes_only_the_matching_write() {
    let mut queues = empty_queues();
    queues
        .try_enqueue_io_write(address("%QW0"), Value::LInt(1))
        .expect("the first write fits");
    queues
        .try_enqueue_io_write(address("%QW2"), Value::LInt(2))
        .expect("the second write fits");
    queues
        .try_enqueue_io_write(address("%QW0"), Value::LInt(3))
        .expect("a repeated address is a separate write");

    queues.withdraw_io_write(&address("%QW0"), &Value::LInt(1));
    queues.withdraw_io_write(&address("%QW0"), &Value::LInt(99));

    let writes = queues.drain_io_writes();
    assert_eq!(
        writes,
        vec![
            (address("%QW2"), Value::LInt(2)),
            (address("%QW0"), Value::LInt(3)),
        ],
        "only the %QW0 write carrying value 1 is withdrawn"
    );
}

#[test]
fn the_forced_snapshot_lists_every_force() {
    let mut queues = empty_queues();
    queues
        .try_upsert_forced_var(ForcedVarTarget::Global("Speed".into()), Value::LInt(1))
        .expect("the global force fits");
    queues
        .try_upsert_forced_var(ForcedVarTarget::Retain("Count".into()), Value::LInt(2))
        .expect("the retained force fits");
    queues
        .try_upsert_forced_io(address("%QW0"), Value::LInt(3))
        .expect("the I/O force fits");

    let forced = queues.forced_snapshot();
    assert_eq!(forced.vars.len(), 2);
    assert!(matches!(&forced.vars[0].target, ForcedVarTarget::Global(name) if name == "Speed"));
    assert!(matches!(&forced.vars[1].target, ForcedVarTarget::Retain(name) if name == "Count"));
    assert_eq!(forced.io, vec![(address("%QW0"), Value::LInt(3))]);
}

#[test]
fn pending_write_identity_includes_target_kind() {
    let mut queues = empty_queues();
    queues
        .try_upsert_var_write(PendingVarTarget::Global("same".into()), Value::LInt(1))
        .expect("global fits");
    queues
        .try_upsert_var_write(PendingVarTarget::Retain("same".into()), Value::LInt(2))
        .expect("retain fits");
    queues
        .try_upsert_var_write(
            PendingVarTarget::Instance(Generation::FIRST, InstanceId(7), "same".into()),
            Value::LInt(3),
        )
        .expect("instance fits");
    queues
        .try_upsert_var_write(
            PendingVarTarget::Local(FrameId(7), "same".into()),
            Value::LInt(4),
        )
        .expect("local fits");

    let writes = queues.drain_var_writes();
    assert_eq!(
        writes.len(),
        4,
        "the same name in four kinds is four targets"
    );
    assert!(matches!(&writes[0].target, PendingVarTarget::Global(name) if name == "same"));
    assert!(matches!(&writes[1].target, PendingVarTarget::Retain(name) if name == "same"));
    assert!(
        matches!(&writes[2].target, PendingVarTarget::Instance(_, InstanceId(7), name) if name == "same")
    );
    assert!(
        matches!(&writes[3].target, PendingVarTarget::Local(FrameId(7), name) if name == "same")
    );
}

#[test]
fn pending_instance_write_identity_includes_instance_id() {
    let mut queues = empty_queues();
    queues
        .try_upsert_var_write(
            PendingVarTarget::Instance(Generation::FIRST, InstanceId(1), "value".into()),
            Value::LInt(1),
        )
        .expect("instance 1 fits");
    queues
        .try_upsert_var_write(
            PendingVarTarget::Instance(Generation::FIRST, InstanceId(2), "value".into()),
            Value::LInt(2),
        )
        .expect("instance 2 fits");
    queues
        .try_upsert_var_write(
            PendingVarTarget::Instance(Generation::FIRST, InstanceId(1), "value".into()),
            Value::LInt(3),
        )
        .expect("replacing instance 1 fits");

    let writes = queues.drain_var_writes();
    assert_eq!(writes.len(), 2);
    assert!(matches!(
        &writes[0].target,
        PendingVarTarget::Instance(_, InstanceId(1), name) if name == "value"
    ));
    assert_eq!(writes[0].value, Value::LInt(3));
    assert!(matches!(
        &writes[1].target,
        PendingVarTarget::Instance(_, InstanceId(2), name) if name == "value"
    ));
}

#[test]
fn pending_local_write_identity_includes_frame_id() {
    let mut queues = empty_queues();
    queues
        .try_upsert_var_write(
            PendingVarTarget::Local(FrameId(1), "value".into()),
            Value::LInt(1),
        )
        .expect("frame 1 fits");
    queues
        .try_upsert_var_write(
            PendingVarTarget::Local(FrameId(2), "value".into()),
            Value::LInt(2),
        )
        .expect("frame 2 fits");
    queues
        .try_upsert_var_write(
            PendingVarTarget::Local(FrameId(1), "value".into()),
            Value::LInt(3),
        )
        .expect("replacing frame 1 fits");

    let writes = queues.drain_var_writes();
    assert_eq!(writes.len(), 2);
    assert!(
        matches!(&writes[0].target, PendingVarTarget::Local(FrameId(1), name) if name == "value")
    );
    assert_eq!(writes[0].value, Value::LInt(3));
    assert!(
        matches!(&writes[1].target, PendingVarTarget::Local(FrameId(2), name) if name == "value")
    );
}

#[test]
fn instance_force_identity_includes_instance_id() {
    let mut queues = empty_queues();
    queues
        .try_upsert_forced_var(
            ForcedVarTarget::Instance(Generation::FIRST, InstanceId(1), "value".into()),
            Value::LInt(1),
        )
        .expect("instance 1 fits");
    queues
        .try_upsert_forced_var(
            ForcedVarTarget::Instance(Generation::FIRST, InstanceId(2), "value".into()),
            Value::LInt(2),
        )
        .expect("instance 2 fits");
    queues
        .try_upsert_forced_var(
            ForcedVarTarget::Instance(Generation::FIRST, InstanceId(1), "value".into()),
            Value::LInt(3),
        )
        .expect("re-forcing instance 1 fits");

    let forced = queues.forced_snapshot().vars;
    assert_eq!(forced.len(), 2);
    assert!(matches!(
        &forced[0].target,
        ForcedVarTarget::Instance(_, InstanceId(1), name) if name == "value"
    ));
    assert_eq!(forced[0].value, Value::LInt(3));
    assert!(matches!(
        &forced[1].target,
        ForcedVarTarget::Instance(_, InstanceId(2), name) if name == "value"
    ));
}

#[test]
fn releases_are_exact_across_variable_target_kinds() {
    let mut queues = empty_queues();
    queues
        .try_upsert_forced_var(ForcedVarTarget::Global("same".into()), Value::LInt(1))
        .expect("global fits");
    queues
        .try_upsert_forced_var(ForcedVarTarget::Retain("same".into()), Value::LInt(2))
        .expect("retain fits");
    queues
        .try_upsert_forced_var(
            ForcedVarTarget::Instance(Generation::FIRST, InstanceId(7), "same".into()),
            Value::LInt(3),
        )
        .expect("instance 7 fits");
    queues
        .try_upsert_forced_var(
            ForcedVarTarget::Instance(Generation::FIRST, InstanceId(8), "same".into()),
            Value::LInt(4),
        )
        .expect("instance 8 fits");

    queues.clear_forced_vars(
        |target| matches!(target, ForcedVarTarget::Global(name) if name == "same"),
    );
    queues.clear_forced_vars(|target| {
        matches!(target, ForcedVarTarget::Instance(_, InstanceId(7), name) if name == "same")
    });

    let forced = queues.forced_snapshot().vars;
    assert_eq!(forced.len(), 2);
    assert!(matches!(&forced[0].target, ForcedVarTarget::Retain(name) if name == "same"));
    assert!(matches!(
        &forced[1].target,
        ForcedVarTarget::Instance(_, InstanceId(8), name) if name == "same"
    ));
}

#[test]
fn variable_release_is_idempotent_for_absent_target() {
    let mut queues = empty_queues();
    queues
        .try_upsert_forced_var(ForcedVarTarget::Global("a".into()), Value::LInt(1))
        .expect("global fits");

    queues.clear_forced_vars(
        |target| matches!(target, ForcedVarTarget::Global(name) if name == "missing"),
    );
    queues.clear_forced_vars(
        |target| matches!(target, ForcedVarTarget::Retain(name) if name == "missing"),
    );
    queues.clear_forced_vars(|target| {
        matches!(target, ForcedVarTarget::Instance(_, InstanceId(1), name) if name == "missing")
    });

    let forced = queues.forced_snapshot().vars;
    assert_eq!(forced.len(), 1);
    assert_eq!(forced[0].value, Value::LInt(1));
}

#[test]
fn io_force_replacement_keeps_original_position() {
    let mut queues = empty_queues();
    queues
        .try_upsert_forced_io(address("%QW0"), Value::LInt(1))
        .expect("%QW0 fits");
    queues
        .try_upsert_forced_io(address("%QW2"), Value::LInt(2))
        .expect("%QW2 fits");
    queues
        .try_upsert_forced_io(address("%QW0"), Value::LInt(3))
        .expect("re-forcing %QW0 fits");

    let forced = queues.forced_snapshot().io;
    assert_eq!(forced.len(), 2);
    assert_eq!(forced[0], (address("%QW0"), Value::LInt(3)));
    assert_eq!(forced[1], (address("%QW2"), Value::LInt(2)));
}

#[test]
fn io_force_queue_refuses_a_new_address_past_the_bound_and_replaces_a_forced_one() {
    let mut queues = empty_queues();
    for word in 0..REQUEST_QUEUE_CAPACITY {
        queues
            .try_upsert_forced_io(address(&format!("%MW{word}")), Value::LInt(1))
            .expect("a force within the bound fits");
    }

    let refused = queues.try_upsert_forced_io(address("%MW1024"), Value::LInt(1));
    assert!(refused.is_err(), "%MW1024 must be refused");
    queues
        .try_upsert_forced_io(address("%MW0"), Value::LInt(2))
        .expect("re-forcing a forced address is never refused");

    let forced = queues.forced_snapshot().io;
    assert_eq!(
        forced.len(),
        REQUEST_QUEUE_CAPACITY,
        "the refused force must not be applied"
    );
    let refused_address = address("%MW1024");
    let first_address = address("%MW0");
    assert!(forced
        .iter()
        .all(|(current, _)| *current != refused_address));
    assert_eq!(
        forced
            .iter()
            .find(|(current, _)| *current == first_address)
            .map(|(_, value)| value.clone()),
        Some(Value::LInt(2))
    );
}

#[test]
fn var_force_queue_refuses_a_new_target_past_the_bound_and_replaces_a_forced_one() {
    let mut queues = empty_queues();
    for index in 0..REQUEST_QUEUE_CAPACITY {
        queues
            .try_upsert_forced_var(
                ForcedVarTarget::Global(format!("f{index}").into()),
                Value::LInt(1),
            )
            .expect("a force within the bound fits");
    }

    let refused =
        queues.try_upsert_forced_var(ForcedVarTarget::Global("f1024".into()), Value::LInt(1));
    assert!(refused.is_err(), "f1024 must be refused");
    queues
        .try_upsert_forced_var(ForcedVarTarget::Global("f0".into()), Value::LInt(2))
        .expect("re-forcing a forced target is never refused");

    let forced = queues.forced_snapshot().vars;
    assert_eq!(
        forced.len(),
        REQUEST_QUEUE_CAPACITY,
        "the refused force must not be applied"
    );
    assert!(forced
        .iter()
        .all(|entry| !matches!(&entry.target, ForcedVarTarget::Global(name) if name == "f1024")));
    assert_eq!(
        forced
            .iter()
            .find(|entry| matches!(&entry.target, ForcedVarTarget::Global(name) if name == "f0"))
            .map(|entry| entry.value.clone()),
        Some(Value::LInt(2))
    );
}

#[test]
fn io_release_is_exact_and_idempotent() {
    let mut queues = empty_queues();
    queues
        .try_upsert_forced_io(address("%QW0"), Value::LInt(1))
        .expect("%QW0 fits");
    queues
        .try_upsert_forced_io(address("%QW2"), Value::LInt(2))
        .expect("%QW2 fits");

    queues.release_io(&address("%QW0"));
    queues.release_io(&address("%QW0"));
    queues.release_io(&address("%QW4"));

    assert_eq!(
        queues.forced_snapshot().io,
        vec![(address("%QW2"), Value::LInt(2))]
    );
}

#[test]
fn forced_snapshot_is_an_owned_copy() {
    let mut queues = empty_queues();
    queues
        .try_upsert_forced_var(ForcedVarTarget::Global("a".into()), Value::LInt(1))
        .expect("global fits");
    queues
        .try_upsert_forced_io(address("%QW0"), Value::LInt(2))
        .expect("I/O fits");

    let mut first = queues.forced_snapshot();
    first.vars[0].value = Value::LInt(99);
    first.io.clear();

    let second = queues.forced_snapshot();
    assert_eq!(second.vars[0].value, Value::LInt(1));
    assert_eq!(second.io, vec![(address("%QW0"), Value::LInt(2))]);
}

#[test]
fn runtime_mutation_clear_removes_every_queue_and_force() {
    let mut queues = empty_queues();
    queues
        .try_enqueue_io_write(address("%QW0"), Value::LInt(1))
        .expect("I/O fits");
    queues
        .try_upsert_var_write(PendingVarTarget::Global("global".into()), Value::LInt(2))
        .expect("global fits");
    queues
        .try_upsert_forced_var(ForcedVarTarget::Global("global".into()), Value::LInt(4))
        .expect("global force fits");
    queues
        .try_upsert_forced_io(address("%QW2"), Value::LInt(5))
        .expect("I/O force fits");

    queues.clear();

    assert_eq!(
        queues.drain_io_writes(),
        Vec::<(IoAddress, Value)>::new(),
        "clear empties the one-shot I/O queue"
    );
    assert!(queues.drain_var_writes().is_empty());
    let forced = queues.forced_snapshot();
    assert!(forced.vars.is_empty());
    assert_eq!(
        forced.io,
        Vec::<(IoAddress, Value)>::new(),
        "clear releases every I/O force"
    );
}
