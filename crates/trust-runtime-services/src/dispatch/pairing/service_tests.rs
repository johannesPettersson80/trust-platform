//! `pair.service` (specification 11 §6.9.6): the reply, the list's entries for service tokens and
//! the refusals, over the Admin's host of the pairing tests, whose port gives zero bytes as random
//! numbers, so that every token is 43 `A`, at the whole second 1,792,108,800.

use serde_json::json;

use super::tests::Host;
use super::{list, service};
use crate::dispatch::dispatch_value;
use crate::tests::fake_host::FakeHost;

/// `recipe` without a role is a Viewer; `mixer` asked as `Admin` is an Engineer. The list gives
/// both in the order they were created, with their names and no expiry. Catches a changed reply
/// shape, the token missing from the reply, and an Admin service token.
#[test]
fn the_reply_carries_the_id_the_role_and_the_token() {
    let host = Host::new(None);
    let token = "A".repeat(43);
    assert_eq!(
        service(1, Some(json!({"name": "recipe"})), &host).result,
        Some(json!({"id": "service-recipe", "role": "viewer", "token": token}))
    );
    assert_eq!(
        service(2, Some(json!({"name": "mixer", "role": "Admin"})), &host).result,
        Some(json!({"id": "service-mixer", "role": "engineer", "token": token}))
    );
    assert_eq!(
        list(3, &host).result,
        Some(json!({"tokens": [
            {
                "id": "service-recipe",
                "enabled": true,
                "created_at": 1_792_108_800_u64,
                "expires_at": null,
                "role": "viewer",
                "service": "recipe",
                "tail": "…AAAA",
            },
            {
                "id": "service-mixer",
                "enabled": true,
                "created_at": 1_792_108_800_u64,
                "expires_at": null,
                "role": "engineer",
                "service": "mixer",
                "tail": "…AAAA",
            },
        ]}))
    );
}

/// The parameters are read first, then the pairings, the role, the name and the clock are checked,
/// in that order, and a token that cannot be written is refused with the port's text; no refusal
/// stores a token. Catches a refusal that changes the pairings, and a check out of its order, such
/// as a bad name reported as the missing clock.
#[test]
fn the_refusals_come_in_the_order_of_the_checks() {
    let host = Host::new(None);
    let error = |response: crate::protocol::ControlResponse| response.error;
    assert_eq!(
        error(service(1, None, &host)).as_deref(),
        Some("missing params")
    );
    assert_eq!(
        error(service(2, Some(json!({"role": "viewer"})), &host)).as_deref(),
        Some("invalid params: missing field `name`")
    );
    assert_eq!(
        error(service(
            3,
            Some(json!({"name": "re-cipe", "role": "boss"})),
            &host
        ))
        .as_deref(),
        Some("invalid role")
    );
    assert_eq!(
        error(service(4, Some(json!({"name": "re-cipe"})), &host)).as_deref(),
        Some("invalid service name")
    );

    let mut no_clock = Host::new(None);
    no_clock.utc_ms = None;
    assert_eq!(
        error(service(5, Some(json!({"name": "re-cipe"})), &no_clock)).as_deref(),
        Some("invalid service name")
    );
    assert_eq!(
        error(service(6, Some(json!({"name": "recipe"})), &no_clock)).as_deref(),
        Some("pairing needs the device's clock: set it with a time server or clock.set")
    );
    assert_eq!(
        error(service(
            7,
            Some(json!({"name": "recipe"})),
            &Host::without_pairings()
        ))
        .as_deref(),
        Some("pairing unavailable")
    );
    let failing = Host::new(Some("disk full"));
    assert_eq!(
        error(service(8, Some(json!({"name": "recipe"})), &failing)).as_deref(),
        Some("disk full")
    );
    for host in [&host, &no_clock, &failing] {
        assert_eq!(list(9, host).result, Some(json!({"tokens": []})));
    }
}

/// The shared pipeline answers `pair.service` itself and never hands it to the platform. Catches
/// the request falling through to `unsupported request`.
#[test]
fn the_shared_pipeline_answers_pair_service() {
    let host = FakeHost::running();
    let response = dispatch_value(
        json!({"id": 9, "type": "pair.service", "params": {"name": "recipe"}}),
        &host,
        None,
    );
    assert_eq!(response.error.as_deref(), Some("pairing unavailable"));
    assert!(host.extension_calls.borrow().is_empty());
}
