//! `variables.write` (specification 11 §6.8.1, "Writing variables"; specification 48 RU-63) through
//! the shared request pipeline, on a running runtime with access control. Its program is the typed
//! fixture's (`Setpoint : REAL`, `Level : INT`, `Mode : Color`, `Name : STRING[5]` and the constant
//! `Limit`), whose service table lets `recipe` write `Setpoint`, `Mode` and `Name` and `dosing`
//! write `Level`. Every expected value is worked out from the specification.

use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec;
use alloc::vec::Vec;
use core::cell::RefCell;

use serde_json::{json, Value as Json};
use smol_str::SmolStr;
use trust_runtime_core::scheduler::ResourceState;
use trust_runtime_core::value::{EnumValue, Value};

use crate::access::pairing::Pairings;
use crate::access::AccessRole;
use crate::dispatch::dispatch_value;
use crate::live_values::queues::{PendingVarTarget, REQUEST_QUEUE_CAPACITY};
use crate::live_values::typed_fixture::image_with;
use crate::protocol::ControlResponse;
use crate::tests::fake_host::{FakeHost, MemoryPairings};

/// The token of 32 zero bytes: 43 `A`, which the in-memory pairings draw.
const TOKEN: &str = "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA";

/// The control token, which grants Admin and is no service token.
const ADMIN: &str = "admin-token";

/// The fixture's services and what each may write.
const SERVICES: &[(&str, &[&str])] = &[
    ("recipe", &["Setpoint", "Mode", "Name"]),
    ("dosing", &["Level"]),
];

/// A running runtime whose [`TOKEN`] is the service token of `service`, with `role`.
fn host(service: &str, role: AccessRole) -> FakeHost {
    let mut host = FakeHost::running();
    host.requires_auth = true;
    host.control_token = Some(ADMIN);
    host.pairing = Some((TOKEN, role));
    let pairings = RefCell::new(Pairings::open(MemoryPairings, 0));
    let created = pairings
        .borrow_mut()
        .service(service, Some(role), 1_000)
        .expect("the service token");
    assert_eq!(created.token, TOKEN);
    host.pairings = Some(pairings);
    host.program_image = Some(image_with(SERVICES));
    host
}

fn write(host: &FakeHost, auth: &str, writes: &Json) -> ControlResponse {
    dispatch_value(
        json!({"id": 1, "type": "variables.write", "auth": auth, "params": {"writes": writes}}),
        host,
        None,
    )
}

/// The writes the next cycle's start applies: each target as the request named it, with its value.
fn queued(host: &FakeHost) -> Vec<(String, Value)> {
    host.live_values
        .borrow()
        .pending_var_writes
        .iter()
        .map(|write| match &write.target {
            PendingVarTarget::Global(path) => (path.to_string(), write.value.clone()),
            other => panic!("a global's write: {other:?}"),
        })
        .collect()
}

#[test]
fn a_service_writes_its_set_at_the_next_cycle_start() {
    // recipe writes 21.5 into Setpoint, Blue into Mode, spelled `mode`, and `pump` into Name,
    // spelled `NAME`: the reply names recipe and 3, and the next cycle's start writes the three
    // values of their declared types, REAL, Color (Blue = 2) and STRING[5], with no force. Catches a
    // set written in part, a path compared with case, and a value of the request's form instead of
    // the variable's type.
    let host = host("recipe", AccessRole::Engineer);
    let response = write(
        &host,
        TOKEN,
        &json!([
            {"path": "Setpoint", "value": 21.5},
            {"path": "mode", "value": "Blue"},
            {"path": "NAME", "value": "pump"},
        ]),
    );
    assert_eq!(
        response.result,
        Some(json!({"service": "recipe", "count": 3})),
        "{:?}",
        response.error
    );
    assert_eq!(
        queued(&host),
        vec![
            (String::from("Setpoint"), Value::Real(21.5)),
            (
                String::from("mode"),
                Value::Enum(alloc::boxed::Box::new(EnumValue::from_canonical_parts(
                    SmolStr::new("Color"),
                    SmolStr::new("Blue"),
                    2,
                ))),
            ),
            (String::from("NAME"), Value::String("pump".into())),
        ]
    );
    assert!(host.live_values.borrow().forced_vars.is_empty());
}

#[test]
fn a_path_or_a_value_the_service_may_not_write_refuses_the_whole_set() {
    // Level is dosing's, Limit a constant and Nothing no variable: none is recipe's. A text of six
    // characters is longer than Name's STRING[5]. Each refusal names its path and comes after a
    // write the set would have allowed, and nothing is queued. Catches a set written up to its
    // first refusal, and a service that writes another's variable.
    for (writes, refusal) in [
        (
            json!([{"path": "Setpoint", "value": 1}, {"path": "Level", "value": 3}]),
            "service recipe may not write Level",
        ),
        (
            json!([{"path": "Setpoint", "value": 1}, {"path": "Limit", "value": 3}]),
            "service recipe may not write Limit",
        ),
        (
            json!([{"path": "Setpoint", "value": 1}, {"path": "Nothing", "value": 3}]),
            "service recipe may not write Nothing",
        ),
        (
            json!([{"path": "Setpoint", "value": 1}, {"path": "Name", "value": "pumpss"}]),
            "invalid value for Name: '\"pumpss\"' is longer than STRING[5] holds",
        ),
    ] {
        let host = host("recipe", AccessRole::Engineer);
        assert_eq!(write(&host, TOKEN, &writes).error.as_deref(), Some(refusal));
        assert_eq!(queued(&host), Vec::new(), "{refusal}");
    }
}

#[test]
fn only_an_engineers_service_token_writes() {
    // A Viewer's service token is refused by its role; the control token is Admin but speaks for
    // no service; a service token of a service without writes writes nothing. Catches a write any
    // credential could send, and a reading service that could write.
    let viewer = host("recipe", AccessRole::Viewer);
    let refused = write(&viewer, TOKEN, &json!([{"path": "Setpoint", "value": 1}]));
    assert_eq!(refused.error_code.as_deref(), Some("insufficient_role"));
    let admin = host("recipe", AccessRole::Engineer);
    assert_eq!(
        write(&admin, ADMIN, &json!([{"path": "Setpoint", "value": 1}]))
            .error
            .as_deref(),
        Some("variables.write needs a service token")
    );
    let monitor = host("monitor", AccessRole::Engineer);
    assert_eq!(
        write(&monitor, TOKEN, &json!([{"path": "Setpoint", "value": 1}]))
            .error
            .as_deref(),
        Some("service monitor may write no variable")
    );
    for host in [&viewer, &admin, &monitor] {
        assert_eq!(queued(host), Vec::new());
    }
}

#[test]
fn a_write_needs_a_running_program() {
    // A stopped resource runs no cycle that would apply the set: refused with its state, and
    // nothing is queued for a later start.
    let host = host("recipe", AccessRole::Engineer);
    host.state.set(ResourceState::Stopped);
    assert_eq!(
        write(&host, TOKEN, &json!([{"path": "Setpoint", "value": 1}]))
            .error
            .as_deref(),
        Some("the program does not run: stopped")
    );
    assert_eq!(queued(&host), Vec::new());
}

#[test]
fn the_parameters_are_exactly_writes_of_paths_and_values() {
    // No writes, another key, an empty set, 257 values and a write with a third key are refused
    // before the token is looked at. Catches a request read in part.
    let host = host("recipe", AccessRole::Engineer);
    let many: Vec<Json> = (0..257)
        .map(|_| json!({"path": "Setpoint", "value": 1}))
        .collect();
    for (params, refusal) in [
        (
            json!({}),
            "invalid params: writes must be an array of {path, value} objects",
        ),
        (
            json!({"writes": [], "extra": 1}),
            "invalid params: only writes is taken",
        ),
        (
            json!({"writes": []}),
            "invalid params: writes must hold 1 to 256 values",
        ),
        (
            json!({"writes": many}),
            "invalid params: writes must hold 1 to 256 values",
        ),
        (
            json!({"writes": [{"path": "Setpoint", "value": 1, "at": 0}]}),
            "invalid params: each write is exactly a path and a value",
        ),
    ] {
        let response = dispatch_value(
            json!({"id": 1, "type": "variables.write", "auth": TOKEN, "params": params}),
            &host,
            None,
        );
        assert_eq!(response.error.as_deref(), Some(refusal), "{params}");
    }
}

#[test]
fn a_set_the_queue_cannot_take_whole_is_refused_whole() {
    // With 1,023 other writes waiting, the queue has room for one more target: recipe's set of two
    // new targets is refused and none of it is queued; a set whose targets wait already replaces
    // their values in place and needs no room. Catches a set queued up to the queue's end.
    let host = host("recipe", AccessRole::Engineer);
    for index in 0..REQUEST_QUEUE_CAPACITY - 1 {
        host.live_values
            .borrow_mut()
            .try_upsert_var_write(
                PendingVarTarget::Global(SmolStr::new(format!("Other{index}"))),
                Value::Int(0),
            )
            .expect("room");
    }
    let set = json!([{"path": "Setpoint", "value": 1}, {"path": "Mode", "value": "Red"}]);
    assert_eq!(
        write(&host, TOKEN, &set).error.as_deref(),
        Some("request queue full")
    );
    assert_eq!(
        host.live_values.borrow().pending_var_writes.len(),
        REQUEST_QUEUE_CAPACITY - 1
    );
    let one = json!([{"path": "Setpoint", "value": 1}]);
    assert_eq!(
        write(&host, TOKEN, &one).result,
        Some(json!({"service": "recipe", "count": 1}))
    );
    assert_eq!(
        write(&host, TOKEN, &one).result,
        Some(json!({"service": "recipe", "count": 1})),
        "a set of waiting targets replaces them in place"
    );
    assert_eq!(
        host.live_values.borrow().pending_var_writes.len(),
        REQUEST_QUEUE_CAPACITY
    );
}
