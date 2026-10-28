//! The device runner's Live Values (specification 11, "Live Values"; specification 34 §4.3.1):
//! where a queued write and an active force reach the process image, how a refused write is
//! reported, the boundaries that clear the queues and the forces, and the generation the Live Values
//! carry after a restart. The board probe (`led := button` every 10 ms) runs compiled behind the
//! cycle port.

use alloc::vec::Vec;

use serde_json::{json, Value};
use trust_runtime_core::error::RuntimeError;
use trust_runtime_core::memory::Generation;
use trust_runtime_core::scheduler::ResourceState;
use trust_runtime_core::watchdog::FaultPolicy;

use super::program::Program;
use super::ResourceCycle;
use crate::live_values::var_requests::VariableQueuePort;
use crate::tests::runner_ports::{Fixture, TestRunner};

/// `io.write` of `value` to `address`.
fn io_write(id: u64, address: &str, value: &str) -> Value {
    json!({"id": id, "type": "io.write", "params": {"address": address, "value": value}})
}

/// `io.force` of `value` on `address`.
fn io_force(id: u64, address: &str, value: &str) -> Value {
    json!({"id": id, "type": "io.force", "params": {"address": address, "value": value}})
}

/// `io.unforce` of `address`.
fn io_unforce(id: u64, address: &str) -> Value {
    json!({"id": id, "type": "io.unforce", "params": {"address": address}})
}

/// `io.list`.
fn io_list(id: u64) -> Value {
    json!({"id": id, "type": "io.list"})
}

/// `restart` in `mode`.
fn restart(id: u64, mode: &str) -> Value {
    json!({"id": id, "type": "restart", "params": {"mode": mode}})
}

/// `shutdown`.
fn shutdown(id: u64) -> Value {
    json!({"id": id, "type": "shutdown"})
}

/// The `%QX0.0` entry of an `io.list` reply.
fn output_led(reply: &Value) -> &Value {
    &reply["result"]["outputs"][0]
}

/// The installation generation of the program the loop runs, and the one its Live Values carry.
fn generations(runner: &TestRunner<'_>) -> (Generation, Generation) {
    let Program::Installed(cycle) = &runner.cycle else {
        panic!("a program is installed");
    };
    (cycle.generation(), runner.live.generation())
}

/// Spec 11, "Live Values": a queued one-shot I/O write reaches the process image at
/// `BeforeInputSample`, before the scan writes its mapped outputs.
///
/// The scan writes its mapped outputs (`led := button`, `FALSE`) after the queued writes are
/// applied, so the program's own value replaces the queued `TRUE`. A write applied after the
/// outputs are written, as `AfterOutputSample` would, leaves `TRUE` in the image instead.
#[test]
fn a_queued_output_write_reaches_the_image_before_the_outputs_are_written() {
    let fixture = Fixture::board_probe();
    let mut runner = fixture.runner();
    // The first scan, due at power-on, runs before a request that arrives then (STATE-08).
    fixture.pass_at(&mut runner, 0);
    let reply = fixture.call_at(&mut runner, 0, &io_write(1, "%QX0.0", "TRUE"));
    assert_eq!(reply["result"], json!({"status": "queued"}));
    fixture.pass_at(&mut runner, 10);

    let list = fixture.call(&mut runner, &io_list(2));
    assert_eq!(
        output_led(&list)["value"],
        json!("Bool(false)"),
        "the queued output write reached the image before the outputs were written, so the \
         program's mapped output replaced it"
    );
    assert_eq!(
        output_led(&list)["forced"],
        json!(false),
        "a one-shot write is not a force"
    );
}

/// Spec 11, "Live Values": a force is applied again after the outputs are sampled, so it holds
/// the output across scans until it is released.
///
/// The program writes `led := button` (`FALSE`) in every due scan. A force applied only before the
/// program runs, or one that is released but still applied, leaves the wrong value.
#[test]
fn a_force_holds_across_scans_until_it_is_released() {
    let fixture = Fixture::board_probe();
    let mut runner = fixture.runner();
    // The first scan, due at power-on, runs before a request that arrives then (STATE-08).
    fixture.pass_at(&mut runner, 0);
    let reply = fixture.call_at(&mut runner, 0, &io_force(1, "%QX0.0", "TRUE"));
    assert_eq!(reply["result"], json!({"status": "forced"}));

    fixture.pass_at(&mut runner, 10);
    fixture.pass_at(&mut runner, 20);
    let list = fixture.call(&mut runner, &io_list(2));
    assert_eq!(
        output_led(&list)["value"],
        json!("Bool(true)"),
        "the force is applied again after the outputs are sampled"
    );
    assert_eq!(output_led(&list)["forced"], json!(true));

    let reply = fixture.call(&mut runner, &io_unforce(3, "%QX0.0"));
    assert_eq!(reply["result"], json!({"status": "released"}));
    fixture.pass_at(&mut runner, 30);
    let list = fixture.call(&mut runner, &io_list(4));
    assert_eq!(
        output_led(&list)["value"],
        json!("Bool(false)"),
        "the program's own value returns once the force is released"
    );
    assert_eq!(output_led(&list)["forced"], json!(false));
}

/// Spec 11, "Live Values": a request a scan boundary refuses is reported as a `write_rejected`
/// event and changes nothing.
///
/// `probe_depth` is an `INT`; a `BOOL` write to it is refused at `CycleStart`. A refused write that
/// is not reported, or one reported with another target, fails here.
#[test]
fn a_refused_write_becomes_a_write_rejected_event() {
    let fixture = Fixture::board_probe();
    let mut runner = fixture.runner();
    // The first scan, due at power-on, runs before a request that arrives then (STATE-08).
    fixture.pass_at(&mut runner, 0);
    let reply = fixture.call_at(
        &mut runner,
        0,
        &json!({
            "id": 1,
            "type": "set",
            "params": {"target": "global:probe_depth", "value": "TRUE"},
        }),
    );
    assert_eq!(reply["result"], json!({"status": "queued"}));

    // The queued write reaches the scan at 10 ms, the first scan with a cycle start after it.
    fixture.pass_at(&mut runner, 10);
    let events = fixture.call(&mut runner, &json!({"id": 2, "type": "events.tail"}));
    assert_eq!(
        events["result"]["events"],
        json!([{
            "type": "write_rejected",
            "target": "probe_depth",
            "error": "type mismatch",
            "time_ns": 10_000_000,
        }]),
        "the refused write is reported at the scan at 10 ms, the first to start after it; the \
         store keeps no event that every scan makes"
    );
}

/// Spec 11 §6.9.2: a fault's safe state and a restart clear the queued writes and the active
/// forces.
///
/// A queued input write and a force are waiting when the scan faults; the fault clears them, and
/// the restart clears them again. A boundary that kept them leaves the cleared input write to
/// reach the program after the restart, so `led` would be `TRUE` instead of `FALSE`.
#[test]
fn safe_state_and_restart_clear_the_queues_and_forces() {
    let fixture = Fixture::board_probe();
    fixture.script.borrow_mut().fault_policy = FaultPolicy::SafeHalt;
    let mut runner = fixture.runner();
    // The first scan, due at power-on, runs before a request that arrives then (STATE-08).
    fixture.pass_at(&mut runner, 0);
    let _ = fixture.call_at(&mut runner, 0, &io_force(1, "%QX0.0", "TRUE"));
    let _ = fixture.call_at(&mut runner, 1, &io_write(2, "%IX0.0", "TRUE"));
    assert_eq!(
        fixture.live.forced_snapshot().io.len(),
        1,
        "the force is active before the fault"
    );

    fixture
        .script
        .borrow_mut()
        .failing_scans
        .push_back(RuntimeError::DivisionByZero);
    fixture.pass_at(&mut runner, 10);
    assert_eq!(runner.resource_state(), ResourceState::Faulted);
    assert_eq!(
        fixture.live.forced_snapshot().io,
        Vec::new(),
        "the fault's safe state cleared the force"
    );

    // Sent before the faulted wait's poll at 20 ms, which takes it. A request that arrives at a
    // due poll waits for the poll (STATE-08).
    let _ = fixture.call_at(&mut runner, 15, &restart(3, "warm"));
    fixture.pass_at(&mut runner, 30);
    assert_eq!(runner.resource_state(), ResourceState::Running);
    assert_eq!(
        fixture.live.forced_snapshot().io,
        Vec::new(),
        "the restart cleared the force"
    );
    fixture.pass_at(&mut runner, 40);
    let list = fixture.call(&mut runner, &io_list(4));
    assert_eq!(
        output_led(&list)["value"],
        json!("Bool(false)"),
        "the cleared input write and force no longer reach the program"
    );
    assert_eq!(output_led(&list)["forced"], json!(false));
}

/// Specification 34 STATE-08 and UPDATE-04 (installation generations): every restart that commits
/// installs the next generation of the program's state, and the loop gives its Live Values that
/// generation before the next cycle; a restart that fails changes none.
///
/// The board probe starts in generation 0. A warm `restart` queued at 5 ms commits at the boundary at
/// 10 ms (generation 1), a cold one queued at 15 ms at the boundary at 20 ms (2); a scan that faults
/// at 30 ms leaves the resource faulted, and a warm `restart` queued at 35 ms commits at the poll of
/// the faulted wait at 40 ms (3); a restart that fails at the boundary at 50 ms leaves both in 3.
/// Wrong behaviour caught: Live Values that keep the generation of the program's first start (0), so
/// that every force by instance identity after a restart is refused as stale; Live Values updated for
/// one kind of restart and not another; Live Values moved before the restart commits.
#[test]
fn the_live_values_carry_the_generation_of_each_restart() {
    let fixture = Fixture::board_probe();
    let mut runner = fixture.runner();
    fixture.pass_at(&mut runner, 0);
    assert_eq!(generations(&runner), (Generation(0), Generation(0)));

    let queued = fixture.call_at(&mut runner, 5, &restart(1, "warm"));
    assert_eq!(queued["result"], json!({"status": "restart queued"}));
    fixture.pass_at(&mut runner, 10);
    assert_eq!(
        generations(&runner),
        (Generation(1), Generation(1)),
        "a warm restart"
    );

    let queued = fixture.call_at(&mut runner, 15, &restart(2, "cold"));
    assert_eq!(queued["result"], json!({"status": "restart queued"}));
    fixture.pass_at(&mut runner, 20);
    assert_eq!(
        generations(&runner),
        (Generation(2), Generation(2)),
        "a cold restart"
    );

    fixture
        .script
        .borrow_mut()
        .failing_scans
        .push_back(RuntimeError::DivisionByZero);
    fixture.pass_at(&mut runner, 30);
    assert_eq!(runner.resource_state(), ResourceState::Faulted);
    assert_eq!(
        generations(&runner),
        (Generation(2), Generation(2)),
        "a fault installs no generation"
    );
    let queued = fixture.call_at(&mut runner, 35, &restart(3, "warm"));
    assert_eq!(queued["result"], json!({"status": "restart queued"}));
    fixture.pass_at(&mut runner, 40);
    assert_eq!(runner.resource_state(), ResourceState::Running);
    assert_eq!(
        generations(&runner),
        (Generation(3), Generation(3)),
        "the recovery of a faulted resource"
    );

    fixture
        .script
        .borrow_mut()
        .failing_restarts
        .push_back(RuntimeError::Overflow);
    let queued = fixture.call_at(&mut runner, 45, &restart(4, "cold"));
    assert_eq!(queued["result"], json!({"status": "restart queued"}));
    fixture.pass_at(&mut runner, 50);
    assert_eq!(runner.resource_state(), ResourceState::Faulted);
    assert_eq!(
        generations(&runner),
        (Generation(3), Generation(3)),
        "a restart that fails changes no generation"
    );
}

/// Specification 34 STATE-08 and UPDATE-04: the automatic restart of the fault policy is a restart in
/// place like the others. The policy is `restart`; the scan at 10 ms faults, the loop restarts the
/// program warm and goes on running, in generation 1, and its Live Values carry it. Wrong behaviour
/// caught: Live Values that follow the restarts a client asks for and not the one the loop makes
/// itself.
#[test]
fn the_live_values_carry_the_generation_of_an_automatic_restart() {
    let fixture = Fixture::board_probe();
    fixture.script.borrow_mut().fault_policy = FaultPolicy::Restart;
    let mut runner = fixture.runner();
    fixture.pass_at(&mut runner, 0);
    assert_eq!(generations(&runner), (Generation(0), Generation(0)));

    fixture
        .script
        .borrow_mut()
        .failing_scans
        .push_back(RuntimeError::DivisionByZero);
    fixture.pass_at(&mut runner, 10);
    assert_eq!(runner.resource_state(), ResourceState::Running);
    assert_eq!(generations(&runner), (Generation(1), Generation(1)));
}

/// Specification 34 STATE-08 and UPDATE-04: a `restart` of a program that a `shutdown` ended starts it
/// again in place when no newer container is installed, and that restart installs the next generation
/// too. The board probe runs two scans and stops at 20 ms; the cold `restart` at 27 ms starts it in
/// generation 1. Wrong behaviour caught: Live Values that follow a restart of a running program and
/// not the start of a stopped one, the way a chip is restarted after `shutdown`.
#[test]
fn the_live_values_carry_the_generation_of_a_restart_after_a_stop() {
    let fixture = Fixture::board_probe();
    let mut runner = fixture.runner();
    fixture.pass_at(&mut runner, 0);
    fixture.pass_at(&mut runner, 10);
    let _ = fixture.call_at(&mut runner, 15, &shutdown(1));
    fixture.pass_at(&mut runner, 20);
    assert_eq!(runner.resource_state(), ResourceState::Stopped);
    assert_eq!(generations(&runner), (Generation(0), Generation(0)));

    let reply = fixture.call_at(&mut runner, 27, &restart(2, "cold"));
    assert_eq!(reply["result"], json!({"status": "restart queued"}));
    assert_eq!(runner.resource_state(), ResourceState::Running);
    assert_eq!(generations(&runner), (Generation(1), Generation(1)));
}

/// Specification 34 STATE-08 and UPDATE-04: the composition may restart the program warm before it
/// builds the loop, as it does for a program that keeps retained values, and that restart has
/// installed generation 1. The loop that takes the program gives its Live Values that generation.
/// Wrong behaviour caught: Live Values that start in generation 0 whatever the program is in, which
/// refuse every force by instance identity on a chip that booted warm.
#[test]
fn a_loop_takes_the_generation_of_a_program_the_composition_restarted_warm() {
    let fixture = Fixture::board_probe();
    let runner = fixture.power_on_restarted_warm();
    assert_eq!(generations(&runner), (Generation(1), Generation(1)));
}
