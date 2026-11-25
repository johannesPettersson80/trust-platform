//! When `trust-runtime simulate` reads its first request (specification 11 §6.8.1, "Simulator for
//! machine tests"; checklist G302): once its resource has left Boot.

use std::cell::Cell;
use std::time::Duration;

use trust_runtime::scheduler::ResourceState;

use super::wait_until_started;

#[test]
fn the_simulator_reads_its_first_request_once_its_resource_has_left_boot() {
    // Worked by hand: a resource that reads Boot three times and then Running is waited for until
    // the fourth read, and the wait ends without an error. Catches a wait that lets the first
    // request in while the resource still boots (one read) and one that never ends.
    let reads = Cell::new(0);
    let state = || {
        reads.set(reads.get() + 1);
        if reads.get() <= 3 {
            ResourceState::Boot
        } else {
            ResourceState::Running
        }
    };
    wait_until_started(state, Duration::from_secs(60)).expect("the resource starts");
    assert_eq!(reads.get(), 4);
}

#[test]
fn a_resource_that_never_starts_ends_the_simulator_with_the_reason() {
    // Worked by hand: a resource that boots past the limit of 20 ms ends the simulator with the
    // reason and the limit, rather than answering requests it would refuse. Catches a wait without
    // a limit, which would hang the test's process.
    let error = wait_until_started(|| ResourceState::Boot, Duration::from_millis(20))
        .expect_err("a resource that never starts");
    assert_eq!(
        error.to_string(),
        "the simulator's resource did not start within 20ms"
    );
}
