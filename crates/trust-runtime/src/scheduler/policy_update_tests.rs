//! The scheduler policy set (specification 11 section 6.8.1). `config.set` sends the watchdog,
//! fault and retain-save policies as one command. A full queue refuses all of them, and the scan
//! thread applies all of them at one command boundary, both in the running loop and in the faulted
//! wait. These tests drive the real queue and the real apply path against a runtime.

use super::{
    apply_resource_command, faulted, CommandSendError, ResourceCommand, ResourceControl,
    ResourceRunner, SchedulerPolicies, StdClock, RESOURCE_COMMAND_CAPACITY,
};
use crate::value::Duration;
use crate::watchdog::{FaultPolicy, WatchdogAction, WatchdogPolicy};

fn fresh_runtime() -> crate::Runtime {
    crate::RuntimeBuilder::new()
        .build()
        .expect("admit the policy fixture")
}

fn policies_of(runtime: &crate::Runtime) -> SchedulerPolicies {
    SchedulerPolicies {
        watchdog: runtime.watchdog_policy(),
        fault_policy: runtime.fault_policy(),
        retain_save_interval: runtime.retain_save_interval(),
    }
}

/// A policy set that differs from `current` in every policy, so a change shows in each of them.
fn changed_policies(current: SchedulerPolicies) -> SchedulerPolicies {
    SchedulerPolicies {
        watchdog: WatchdogPolicy {
            enabled: !current.watchdog.enabled,
            timeout: Duration::from_millis(250),
            action: WatchdogAction::SafeHalt,
        },
        fault_policy: if current.fault_policy == FaultPolicy::Restart {
            FaultPolicy::Halt
        } else {
            FaultPolicy::Restart
        },
        retain_save_interval: match current.retain_save_interval {
            Some(_) => None,
            None => Some(Duration::from_millis(1500)),
        },
    }
}

#[test]
fn a_full_queue_refuses_the_whole_policy_set_and_applies_none_of_it() {
    let mut runtime = fresh_runtime();
    let before = policies_of(&runtime);
    let requested = changed_policies(before);
    let (control, queued) = ResourceControl::stub(StdClock::new());
    // `Pause` applies as a no-op, so filling the queue with it changes no policy.
    for _ in 0..RESOURCE_COMMAND_CAPACITY {
        control
            .send_command(ResourceCommand::Pause)
            .expect("the queue takes commands up to its bound");
    }
    assert_eq!(
        control.send_command(ResourceCommand::UpdateSchedulerPolicies(requested)),
        Err(CommandSendError::Full),
        "a full queue refuses the whole policy set"
    );

    // The scan thread drains the queue at its next command boundary. The refused set was never
    // queued, so draining the queue applies nothing.
    for command in queued.try_iter() {
        apply_resource_command(&mut runtime, command);
    }
    assert_eq!(
        policies_of(&runtime),
        before,
        "every policy is unchanged after the queue drains"
    );
}

#[test]
fn the_last_free_slot_takes_the_whole_policy_set_and_one_step_applies_it() {
    let mut runtime = fresh_runtime();
    let before = policies_of(&runtime);
    let requested = changed_policies(before);
    let (control, queued) = ResourceControl::stub(StdClock::new());
    for _ in 0..(RESOURCE_COMMAND_CAPACITY - 1) {
        control
            .send_command(ResourceCommand::Pause)
            .expect("the queue takes commands up to its bound");
    }
    control
        .send_command(ResourceCommand::UpdateSchedulerPolicies(requested))
        .expect("the one free slot takes the whole policy set");

    assert_eq!(
        policies_of(&runtime),
        before,
        "nothing changes until the scan thread drains the queue"
    );
    let drained: Vec<ResourceCommand> = queued.try_iter().collect();
    assert_eq!(
        drained.len(),
        RESOURCE_COMMAND_CAPACITY,
        "the policy set occupies exactly the last slot"
    );
    for command in drained {
        apply_resource_command(&mut runtime, command);
    }
    assert_eq!(
        policies_of(&runtime),
        requested,
        "all three policies change together"
    );
}

#[test]
fn the_faulted_wait_applies_the_whole_policy_set_without_ending_the_wait() {
    let runtime = fresh_runtime();
    let before = policies_of(&runtime);
    let requested = changed_policies(before);
    let mut runner = ResourceRunner::new(runtime, StdClock::new(), Duration::from_millis(1));

    let online_change_applied = faulted::serve_command(
        &mut runner,
        ResourceCommand::UpdateSchedulerPolicies(requested),
    );

    assert!(
        !online_change_applied,
        "a policy update is not an online change, so the faulted wait continues"
    );
    assert_eq!(
        policies_of(runner.runtime()),
        requested,
        "the faulted wait applies every policy"
    );
}
