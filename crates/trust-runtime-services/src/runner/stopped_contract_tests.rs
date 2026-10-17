//! A device without an installed program (specification 34, STATE-08): the loop serves the link
//! in the Stopped state, answers every request as while Stopped, and refuses `restart` with the
//! reason until a program is installed.

use serde_json::{json, Value};
use trust_runtime_core::scheduler::ResourceState;

use crate::tests::runner_ports::{ms, CycleCall, Fixture, IoStep};

const REASON: &str = "the application region holds no program";

fn request(id: u64, kind: &str) -> Value {
    json!({"id": id, "type": kind})
}

#[test]
fn a_device_without_a_program_answers_as_stopped_and_refuses_to_start_with_the_reason() {
    let fixture = Fixture::board_probe();
    let mut runner = fixture.without_program(REASON);
    assert_eq!(runner.resource_state(), ResourceState::Stopped);
    assert_eq!(runner.last_error(), None);

    let status = fixture.call_at(&mut runner, 5, &request(1, "status"));
    assert_eq!(status["result"]["state"], json!("stopped"));
    assert_eq!(
        status["result"]["fault"],
        Value::Null,
        "no program is no fault"
    );
    let health = fixture.call(&mut runner, &request(2, "health"));
    assert_eq!(health["result"]["ok"], json!(false));
    assert_eq!(health["result"]["state"], json!("stopped"));

    for (id, mode) in [(3, "cold"), (4, "warm")] {
        let reply = fixture.call(
            &mut runner,
            &json!({"id": id, "type": "restart", "params": {"mode": mode}}),
        );
        assert_eq!(reply["id"], json!(id), "{mode}");
        assert_eq!(reply["ok"], json!(false), "{mode}");
        assert_eq!(
            reply["error"],
            json!("no installed program: the application region holds no program"),
            "{mode}"
        );
        assert_eq!(reply["error_code"], json!("resource_not_running"), "{mode}");
    }
    assert_eq!(runner.resource_state(), ResourceState::Stopped);
    for (id, kind) in [(5, "pause"), (6, "resume")] {
        let reply = fixture.call(&mut runner, &request(id, kind));
        assert_eq!(
            reply["error"],
            json!("resource is stopped"),
            "{kind} answers as trust-runtime answers it while stopped"
        );
    }
    let shutdown = fixture.call(&mut runner, &request(7, "shutdown"));
    assert_eq!(shutdown["result"], json!({"status": "stopping"}));
    assert_eq!(runner.resource_state(), ResourceState::Stopped);

    let events = fixture.call(&mut runner, &request(8, "events.tail"));
    assert_eq!(events["result"]["events"], json!([]));
    let tasks = fixture.call(&mut runner, &request(9, "tasks.stats"));
    assert_eq!(tasks["result"]["tasks"], json!([]));
    assert_eq!(
        fixture.take_calls(),
        [] as [CycleCall; 0],
        "there is no program to scan, stop or restart"
    );
    assert_eq!(fixture.io.take_steps(), [] as [IoStep; 0], "and no I/O");
}

#[test]
fn a_device_without_a_program_waits_for_the_link_and_feeds_its_watchdog() {
    let fixture = Fixture::board_probe();
    let mut runner = fixture.without_program(REASON);
    for at in [0, 100, 1_000] {
        fixture.pass_at(&mut runner, at);
    }
    assert_eq!(
        fixture.wait.take_deadlines(),
        [ms(1_000), ms(1_100), ms(2_000)],
        "without a program there is no boundary: each pass waits for the link, and at most \
         the watchdog's feed interval (TIME-04)"
    );
}
