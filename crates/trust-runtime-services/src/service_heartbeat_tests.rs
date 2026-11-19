//! `service.heartbeat` (specification 11 §6.8.1, "Service heartbeats"; specification 48 RU-63)
//! through the shared request pipeline, on a running runtime with access control whose pairings
//! hold the service token of `recipe`. Every expected value is worked out from the specification.

use alloc::string::{String, ToString};
use alloc::vec;
use alloc::vec::Vec;
use core::cell::RefCell;

use serde_json::json;
use trust_runtime_core::scheduler::ResourceState;
use trust_runtime_core::value::Value;

use crate::access::pairing::Pairings;
use crate::access::AccessRole;
use crate::dispatch::dispatch_value;
use crate::live_values::queues::PendingVarTarget;
use crate::protocol::ControlResponse;
use crate::tests::fake_host::{FakeHost, MemoryPairings};
use crate::variables::{ReadRefusal, ReadSet};

/// The token of 32 zero bytes: 43 `A`, which the in-memory pairings draw.
const TOKEN: &str = "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA";

/// The control token, which grants Admin and is no service token.
const ADMIN: &str = "admin-token";

/// A running runtime with access control: [`TOKEN`] is the Viewer token of the service `recipe`,
/// and the last completed scan left `beat` in `recipe_heartbeat` (or `variables.read`'s refusal).
fn host(beat: Result<Value, ReadRefusal>) -> FakeHost {
    let mut host = FakeHost::running();
    host.requires_auth = true;
    host.control_token = Some(ADMIN);
    host.pairing = Some((TOKEN, AccessRole::Viewer));
    let pairings = RefCell::new(Pairings::open(MemoryPairings, 0));
    let created = pairings
        .borrow_mut()
        .service("recipe", None, 1_000)
        .expect("the service token");
    assert_eq!(created.token, TOKEN);
    host.pairings = Some(pairings);
    host.variables = beat.map(|value| ReadSet {
        scan: 7,
        time_ns: 70_000_000,
        values: vec![("UDINT".into(), value)],
    });
    host
}

fn heartbeat(host: &FakeHost, auth: &str) -> ControlResponse {
    dispatch_value(
        json!({"id": 1, "type": "service.heartbeat", "auth": auth}),
        host,
        None,
    )
}

/// A heartbeat with the parameters `params`, as the SDK sends it with its period.
fn heartbeat_with(host: &FakeHost, auth: &str, params: &serde_json::Value) -> ControlResponse {
    dispatch_value(
        json!({"id": 1, "type": "service.heartbeat", "auth": auth, "params": params}),
        host,
        None,
    )
}

/// The heartbeats the runtime noted for the fleet topology: each service with its period.
fn noted(host: &FakeHost) -> Vec<(String, Option<u64>)> {
    host.heartbeats.borrow().clone()
}

/// The writes the next cycle's start applies: each target with its value.
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
fn a_service_token_adds_one_to_its_heartbeat_at_the_next_cycle_start() {
    // The last completed scan left 41: the reply names recipe and 42, the read asked for
    // `recipe_heartbeat` alone, and the next cycle's start writes 42 into it, a write of the queue
    // and no force. A Viewer's token is enough. Catches a heartbeat taken from the request instead
    // of the token, a global named after something else, and a write the scan never applies.
    let host = host(Ok(Value::UDInt(41)));
    let response = heartbeat(&host, TOKEN);
    assert_eq!(
        response.result,
        Some(json!({"service": "recipe", "value": 42})),
        "{:?}",
        response.error
    );
    assert_eq!(
        *host.variable_reads.borrow(),
        vec![vec![String::from("recipe_heartbeat")]]
    );
    assert_eq!(
        queued(&host),
        vec![(String::from("recipe_heartbeat"), Value::UDInt(42))]
    );
    assert!(host.live_values.borrow().forced_vars.is_empty());
}

#[test]
fn the_heartbeat_wraps_from_the_largest_udint_to_zero() {
    // 4,294,967,295 + 1 wraps to 0, as a UDINT counter does. Catches an overflow that faults or
    // saturates, which would leave the heartbeat unchanged from then on.
    let host = host(Ok(Value::UDInt(u32::MAX)));
    assert_eq!(
        heartbeat(&host, TOKEN).result,
        Some(json!({"service": "recipe", "value": 0}))
    );
    assert_eq!(
        queued(&host),
        vec![(String::from("recipe_heartbeat"), Value::UDInt(0))]
    );
}

#[test]
fn only_a_service_token_sends_a_heartbeat() {
    // The control token is Admin but speaks for no service, and a claimed token of the same bytes
    // is no service token either: both are refused before anything is read or written. Catches a
    // heartbeat that any client could send for any service, which would keep a dead service
    // fresh.
    let admin = host(Ok(Value::UDInt(41)));
    assert_eq!(
        heartbeat(&admin, ADMIN).error.as_deref(),
        Some("service.heartbeat needs a service token")
    );
    let claimed = host(Ok(Value::UDInt(41)));
    {
        let mut pairings = Pairings::open(MemoryPairings, 0);
        let code = pairings.start(1_000).expect("a code").code;
        let token = pairings
            .claim(&code, Some(AccessRole::Viewer), 1_000)
            .expect("a claimed token");
        assert_eq!(token, TOKEN);
        *claimed.pairings.as_ref().expect("pairings").borrow_mut() = pairings;
    }
    assert_eq!(
        heartbeat(&claimed, TOKEN).error.as_deref(),
        Some("service.heartbeat needs a service token")
    );
    for host in [&admin, &claimed] {
        assert!(host.variable_reads.borrow().is_empty());
        assert_eq!(queued(host), Vec::new());
    }
}

#[test]
fn a_heartbeat_needs_a_running_program() {
    // A stopped resource runs no cycle that would apply the write: refused with its state, as
    // `status` names it, and nothing is queued for a later start. Catches a heartbeat that a
    // restart would apply long after it was sent.
    let host = host(Ok(Value::UDInt(41)));
    host.state.set(ResourceState::Stopped);
    assert_eq!(
        heartbeat(&host, TOKEN).error.as_deref(),
        Some("the program does not run: stopped")
    );
    assert_eq!(queued(&host), Vec::new());
}

#[test]
fn a_program_without_the_services_heartbeat_refuses_it() {
    // A program whose `recipe_heartbeat` is missing, or is no UDINT, declares no heartbeat for
    // recipe; before the first completed scan and during a debugger's hold the request is refused
    // as `variables.read` is. Nothing is queued in any case. Catches a heartbeat written into a
    // variable of another type, and one invented for a program that has none.
    for (beat, refusal) in [
        (
            Err(ReadRefusal::Unknown("recipe_heartbeat".into())),
            "the program declares no heartbeat for service recipe",
        ),
        (
            Ok(Value::Int(3)),
            "the program declares no heartbeat for service recipe",
        ),
        (Err(ReadRefusal::NoCompletedScan), "no completed scan"),
        (
            Err(ReadRefusal::Unavailable("a debugger holds the scan".into())),
            "variables unavailable: a debugger holds the scan",
        ),
    ] {
        let host = host(beat);
        assert_eq!(heartbeat(&host, TOKEN).error.as_deref(), Some(refusal));
        assert!(queued(&host).is_empty(), "{refusal}");
    }
}

#[test]
fn an_accepted_heartbeat_is_noted_with_the_period_it_names() {
    // Specification 11 §6.8.1: the runtime keeps each service's last accepted heartbeat with the
    // period it named, for the fleet topology's service clients (§6.8.1a); a heartbeat without
    // `period_ms` is noted without one. Catches a topology that never learns of a heartbeat, and
    // one that guesses a period the service did not name.
    let host = host(Ok(Value::UDInt(41)));
    let reply = heartbeat_with(&host, TOKEN, &json!({"period_ms": 500}));
    assert_eq!(reply.error, None);
    assert_eq!(
        reply.result,
        Some(json!({"service": "recipe", "value": 42}))
    );
    assert_eq!(heartbeat(&host, TOKEN).error, None);
    assert_eq!(
        noted(&host),
        vec![
            ("recipe".to_string(), Some(500)),
            ("recipe".to_string(), None)
        ]
    );
}

#[test]
fn a_period_that_is_no_whole_number_of_milliseconds_above_0_is_refused() {
    // Specification 11 §6.8.1: any `period_ms` but a whole number of milliseconds above 0 is
    // refused with its own words, after the token and before anything is written or noted; a
    // request from no service token is refused for that first. Catches a period of 0, which would
    // make every heartbeat late, and a heartbeat counted although its request was refused.
    for period in [json!(0), json!(-500), json!(1.5), json!("500"), json!(null)] {
        let host = host(Ok(Value::UDInt(41)));
        assert_eq!(
            heartbeat_with(&host, TOKEN, &json!({"period_ms": period}))
                .error
                .as_deref(),
            Some("period_ms is a whole number of milliseconds above 0"),
            "{period}"
        );
        assert!(queued(&host).is_empty(), "{period}");
        assert!(noted(&host).is_empty(), "{period}");
    }
    let host = host(Ok(Value::UDInt(41)));
    assert_eq!(
        heartbeat_with(&host, ADMIN, &json!({"period_ms": 0}))
            .error
            .as_deref(),
        Some("service.heartbeat needs a service token")
    );
}

#[test]
fn a_refused_heartbeat_is_not_noted() {
    // Specification 11 §6.8.1: only an accepted heartbeat is kept. A heartbeat refused because the
    // program does not run leaves the service's last heartbeat as it was. Catches a topology that
    // shows a service connected while its heartbeats are refused.
    let host = host(Ok(Value::UDInt(41)));
    host.state.set(ResourceState::Stopped);
    assert!(heartbeat_with(&host, TOKEN, &json!({"period_ms": 500}))
        .error
        .is_some());
    assert_eq!(noted(&host), Vec::new());
}
