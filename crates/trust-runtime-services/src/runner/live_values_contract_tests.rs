//! The device runner's Live Values (specification 11, "Live Values"; specification 34 §4.3.1):
//! where a queued write and an active force reach the process image, how a refused write is
//! reported, and the boundaries that clear the queues and the forces. The board probe
//! (`led := button` every 10 ms) runs on the real engine behind the cycle port.

use alloc::vec::Vec;

use serde_json::{json, Value};
use trust_runtime_core::error::RuntimeError;
use trust_runtime_core::scheduler::ResourceState;
use trust_runtime_core::watchdog::FaultPolicy;

use crate::live_values::var_requests::VariableQueuePort;
use crate::tests::runner_ports::Fixture;

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

/// The `%QX0.0` entry of an `io.list` reply.
fn output_led(reply: &Value) -> &Value {
    &reply["result"]["outputs"][0]
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
        json!([
            {"type": "cycle_end", "cycle": 1, "time_ns": 10_000_000},
            {"type": "task_end", "name": "Scan", "priority": 0, "time_ns": 10_000_000},
            {"type": "task_start", "name": "Scan", "priority": 0, "time_ns": 10_000_000},
            {"type": "cycle_start", "cycle": 1, "time_ns": 10_000_000},
            {
                "type": "write_rejected",
                "target": "probe_depth",
                "error": "type mismatch",
                "time_ns": 10_000_000,
            },
            {"type": "cycle_end", "cycle": 0, "time_ns": 0},
            {"type": "cycle_start", "cycle": 0, "time_ns": 0},
        ]),
        "the refused write is reported at the cycle start of the scan at 10 ms, before that scan's \
         cycle_start event"
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
