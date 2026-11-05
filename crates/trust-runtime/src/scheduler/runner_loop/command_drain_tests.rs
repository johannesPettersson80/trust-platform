//! Which commands a cycle boundary serves (specification 11 section 6.9): the ones queued when it
//! begins. The tests build a program, which needs the toolchain of specification 44 §4.6 at
//! `TRUST_TOOLCHAIN`.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Barrier, Mutex};
use std::thread;

use super::*;

// Catches checklist W35: a boundary that serves commands until the queue is empty never ends while
// a client queues a new request as soon as there is room, so that no cycle runs. Two Windows CI
// runs timed out that way waiting for a cycle (4 and 5 November 2026).
#[test]
fn a_boundary_serves_only_the_commands_queued_when_it_begins() {
    let (sender, commands) = command_channel();
    let mut runner = ResourceRunner::new(
        crate::runtime::empty_runtime(),
        StdClock::new(),
        Duration::from_millis(10),
    );
    runner.command_rx = Some(commands);
    // The queue is full when the boundary begins; these snapshots answer on `queued`.
    let (queued, queued_answers) = std::sync::mpsc::channel();
    for _ in 0..RESOURCE_COMMAND_CAPACITY {
        let snapshot = ResourceCommand::Snapshot {
            respond_to: queued.clone(),
        };
        try_queue(&sender, snapshot).expect("room in the queue");
    }
    // A client queues a new snapshot, answered on `later`, whenever there is room: until the
    // boundary ends, or once it has queued ten queues of them, so that the test always ends.
    let (later, later_answers) = std::sync::mpsc::channel();
    let ended = Arc::new(AtomicBool::new(false));
    let started = Arc::new(Barrier::new(2));
    let client = {
        let ended = Arc::clone(&ended);
        let started = Arc::clone(&started);
        thread::spawn(move || {
            started.wait();
            let mut sent = 0;
            while !ended.load(Ordering::Acquire) && sent < 10 * RESOURCE_COMMAND_CAPACITY {
                let snapshot = ResourceCommand::Snapshot {
                    respond_to: later.clone(),
                };
                if try_queue(&sender, snapshot).is_ok() {
                    sent += 1;
                }
            }
        })
    };
    started.wait();
    let state = Arc::new(Mutex::new(ResourceState::Running));
    drain_commands(&mut runner, &state, &mut false);
    ended.store(true, Ordering::Release);
    client.join().expect("the client ends");
    assert_eq!(
        queued_answers.try_iter().count(),
        RESOURCE_COMMAND_CAPACITY,
        "every command queued when the boundary began is served"
    );
    assert_eq!(
        later_answers.try_iter().count(),
        0,
        "a command queued while the boundary serves waits for the next boundary"
    );
}
