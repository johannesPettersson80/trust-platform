//! The pairing requests (specification 11 §6.9.6; specification 34 S5): `pair.start` (Admin) starts
//! a six-digit code, `pair.claim` (Operator) trades it for a role-scoped token, `pair.list` (Admin)
//! reports the tokens without their secrets, and `pair.revoke` (Admin) disables one token, or every
//! token with the id `all`. The rules are in [`crate::access::pairing`]; this module reads the
//! parameters and answers through the platform's pairings at its wall clock, in whole Unix seconds.

use alloc::format;
use alloc::string::{String, ToString};

use serde::Deserialize;
use serde_json::{json, Value};
use trust_runtime_core::ports::WallClock;

use crate::access::AccessRole;
use crate::ports::AccessPort;
use crate::protocol::ControlResponse;

/// The refusal of a runtime that keeps no pairings.
const UNAVAILABLE: &str = "pairing unavailable";

/// The refusal of `pair.start` and `pair.claim` while the wall clock is not set: a code or a token
/// needs the time it expires at.
const CLOCK_NOT_SET: &str =
    "pairing needs the device's clock: set it with a time server or clock.set";

/// The parameters of `pair.claim`: the code, and the role to grant, Operator when absent.
#[derive(Debug, Deserialize)]
struct PairClaimParams {
    code: String,
    role: Option<String>,
}

/// The parameters of `pair.revoke`: the token's id, or `all`.
#[derive(Debug, Deserialize)]
struct PairRevokeParams {
    id: String,
}

/// The wall clock in whole Unix seconds, `None` while it is not set.
fn now_secs<H: WallClock>(host: &H) -> Option<u64> {
    host.utc_ms().map(|utc_ms| utc_ms / 1_000)
}

fn refusal(id: u64, text: &str) -> ControlResponse {
    ControlResponse::error(id, text.to_string())
}

/// `pair.start`: `{code, expires_at}`, a new code that replaces any pending one.
pub(super) fn start<H: AccessPort + WallClock>(id: u64, host: &H) -> ControlResponse {
    let Some(pairings) = host.pairing() else {
        return refusal(id, UNAVAILABLE);
    };
    let Some(now) = now_secs(host) else {
        return refusal(id, CLOCK_NOT_SET);
    };
    match pairings.start(now) {
        Ok(code) => ControlResponse::ok(
            id,
            json!({ "code": code.code, "expires_at": code.expires_at }),
        ),
        Err(error) => ControlResponse::error(id, error.to_string()),
    }
}

/// `pair.claim {code, role?}`: `{token}`. The parameters are read first, then the pairings, the
/// role and the clock are checked, in that order.
pub(super) fn claim<H: AccessPort + WallClock>(
    id: u64,
    params: Option<Value>,
    host: &H,
) -> ControlResponse {
    let params: PairClaimParams = match params {
        Some(value) => match serde_json::from_value(value) {
            Ok(parsed) => parsed,
            Err(err) => return ControlResponse::error(id, format!("invalid params: {err}")),
        },
        None => return refusal(id, "missing params"),
    };
    let Some(pairings) = host.pairing() else {
        return refusal(id, UNAVAILABLE);
    };
    let requested_role = match params.role.as_deref().map(AccessRole::parse) {
        None => None,
        Some(Some(role)) => Some(role),
        Some(None) => return refusal(id, "invalid role"),
    };
    let Some(now) = now_secs(host) else {
        return refusal(id, CLOCK_NOT_SET);
    };
    match pairings.claim(&params.code, requested_role, now) {
        Ok(token) => ControlResponse::ok(id, json!({ "token": token })),
        Err(error) => ControlResponse::error(id, error.to_string()),
    }
}

/// `pair.list`: `{tokens}`. Without a set clock no row can be known to have expired, so none is
/// removed.
pub(super) fn list<H: AccessPort + WallClock>(id: u64, host: &H) -> ControlResponse {
    let Some(pairings) = host.pairing() else {
        return refusal(id, UNAVAILABLE);
    };
    let tokens = pairings.list(now_secs(host).unwrap_or(0));
    ControlResponse::ok(id, json!({ "tokens": tokens }))
}

/// `pair.revoke {id}`: `{status: "revoked", id}`, or `{status: "revoked", count}` for the id
/// `all`; `unknown token id` when no token has the id. A revocation needs no clock, as `pair.list`.
pub(super) fn revoke<H: AccessPort + WallClock>(
    id: u64,
    params: Option<Value>,
    host: &H,
) -> ControlResponse {
    let params: PairRevokeParams = match params {
        Some(value) => match serde_json::from_value(value) {
            Ok(parsed) => parsed,
            Err(err) => return ControlResponse::error(id, format!("invalid params: {err}")),
        },
        None => return refusal(id, "missing params"),
    };
    let Some(pairings) = host.pairing() else {
        return refusal(id, UNAVAILABLE);
    };
    let now = now_secs(host).unwrap_or(0);
    if params.id == "all" {
        return match pairings.revoke_all(now) {
            Ok(count) => ControlResponse::ok(id, json!({ "status": "revoked", "count": count })),
            Err(error) => ControlResponse::error(id, error.to_string()),
        };
    }
    match pairings.revoke(&params.id, now) {
        Ok(true) => ControlResponse::ok(id, json!({ "status": "revoked", "id": params.id })),
        Ok(false) => refusal(id, "unknown token id"),
        Err(error) => ControlResponse::error(id, error.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use alloc::string::{String, ToString};
    use core::cell::RefCell;

    use serde_json::json;

    use super::*;
    use crate::access::pairing::{PairingFile, PairingPort, PairingService, Pairings};
    use crate::access::AuthFailure;
    use crate::dispatch::dispatch_value;
    use crate::protocol::ControlRequest;
    use crate::tests::fake_host::FakeHost;

    /// The host's wall clock: 1,792,108,800,999 ms is the whole second 1,792,108,800.
    const UTC_MS: u64 = 1_792_108_800_999;

    /// A port that keeps nothing: an empty file, writes that succeed or fail with `save_error`, and
    /// zero bytes as random numbers, so the code is `000000` and the token 43 `A`.
    struct MemoryPort {
        save_error: Option<&'static str>,
    }

    impl PairingPort for MemoryPort {
        fn load(&mut self) -> Result<PairingFile, String> {
            Ok(PairingFile::default())
        }

        fn save(&mut self, _file: &PairingFile) -> Result<(), String> {
            self.save_error
                .map_or(Ok(()), |error| Err(error.to_string()))
        }

        fn random(&mut self, bytes: &mut [u8]) -> Result<(), String> {
            bytes.fill(0);
            Ok(())
        }
    }

    /// An Admin's host with or without pairings and a clock.
    struct Host {
        pairings: Option<RefCell<Pairings<MemoryPort>>>,
        utc_ms: Option<u64>,
    }

    impl Host {
        fn new(save_error: Option<&'static str>) -> Self {
            Self {
                pairings: Some(RefCell::new(Pairings::open(MemoryPort { save_error }, 0))),
                utc_ms: Some(UTC_MS),
            }
        }

        fn without_pairings() -> Self {
            Self {
                pairings: None,
                utc_ms: Some(UTC_MS),
            }
        }
    }

    impl AccessPort for Host {
        fn request_role(
            &self,
            _request: &ControlRequest,
            _client: Option<&str>,
        ) -> Result<AccessRole, AuthFailure> {
            Ok(AccessRole::Admin)
        }

        fn debug_enabled(&self) -> bool {
            true
        }

        fn pairing(&self) -> Option<&dyn PairingService> {
            self.pairings
                .as_ref()
                .map(|pairings| pairings as &dyn PairingService)
        }
    }

    impl WallClock for Host {
        fn utc_ms(&self) -> Option<u64> {
            self.utc_ms
        }
    }

    /// A runtime without pairings answers each request `pairing unavailable`, as trust-runtime did
    /// without a bundle. Catches a request answered as if pairings existed.
    #[test]
    fn without_pairings_every_request_answers_pairing_unavailable() {
        let host = Host::without_pairings();
        for response in [
            start(1, &host),
            claim(2, Some(json!({"code": "000000"})), &host),
            list(3, &host),
            revoke(4, Some(json!({"id": "all"})), &host),
        ] {
            assert!(!response.ok);
            assert_eq!(response.error.as_deref(), Some("pairing unavailable"));
        }
    }

    /// The shared pipeline answers `pair.*` itself and never hands it to the platform. Catches a
    /// pairing request that falls through to `unsupported request`.
    #[test]
    fn the_shared_pipeline_answers_the_pairing_requests() {
        for kind in ["pair.start", "pair.list"] {
            let host = FakeHost::running();
            let response = dispatch_value(json!({"id": 9, "type": kind}), &host, None);
            assert_eq!(
                response.error.as_deref(),
                Some("pairing unavailable"),
                "{kind}"
            );
            assert!(host.extension_calls.borrow().is_empty(), "{kind}");
        }
    }

    /// Without a set clock `pair.start` and `pair.claim` are refused with the clock's text, so no
    /// code or token is made without the time it expires at; `pair.list` and `pair.revoke` still
    /// answer. Catches a code or token dated from an unset clock, and a revocation that waits for
    /// the clock.
    #[test]
    fn start_and_claim_need_the_clock_and_list_and_revoke_do_not() {
        let mut host = Host::new(None);
        host.utc_ms = None;
        let clock =
            Some("pairing needs the device's clock: set it with a time server or clock.set");
        assert_eq!(start(1, &host).error.as_deref(), clock);
        assert_eq!(
            claim(2, Some(json!({"code": "000000"})), &host)
                .error
                .as_deref(),
            clock
        );
        assert_eq!(list(3, &host).result, Some(json!({"tokens": []})));
        assert_eq!(
            revoke(4, Some(json!({"id": "all"})), &host).result,
            Some(json!({"status": "revoked", "count": 0}))
        );
    }

    /// The replies of a full pairing at 1,792,108,800 s: the code `000000` valid through
    /// 1,792,108,800 + 300 = 1,792,109,100; the token of 32 zero bytes, 43 `A`, for the role
    /// `Viewer` matched without regard to case; its row valid through 1,792,108,800 + 2,592,000 =
    /// 1,794,700,800; then the revocation of its id, an unknown id and `all`. Catches a changed
    /// reply shape, the clock's milliseconds taken as seconds, and a revoke-all that counts a
    /// disabled token.
    #[test]
    fn the_replies_carry_the_code_the_token_the_list_and_the_revocations() {
        let host = Host::new(None);
        assert_eq!(
            start(1, &host).result,
            Some(json!({"code": "000000", "expires_at": 1_792_109_100_u64}))
        );
        assert_eq!(
            claim(2, Some(json!({"code": "123456"})), &host)
                .error
                .as_deref(),
            Some("invalid or expired code")
        );
        let token = "A".repeat(43);
        assert_eq!(
            claim(
                3,
                Some(json!({"code": " 000000 ", "role": "Viewer"})),
                &host
            )
            .result,
            Some(json!({"token": token}))
        );
        assert_eq!(
            list(4, &host).result,
            Some(json!({"tokens": [{
                "id": "pair-1792108800-AAAAAAAA",
                "enabled": true,
                "created_at": 1_792_108_800_u64,
                "expires_at": 1_794_700_800_u64,
                "role": "viewer",
                "tail": "…AAAA",
            }]}))
        );
        assert_eq!(
            revoke(5, Some(json!({"id": "pair-1792108800-AAAAAAAA"})), &host).result,
            Some(json!({"status": "revoked", "id": "pair-1792108800-AAAAAAAA"}))
        );
        assert_eq!(
            revoke(6, Some(json!({"id": "pair-0-unknown"})), &host)
                .error
                .as_deref(),
            Some("unknown token id")
        );
        assert_eq!(
            revoke(7, Some(json!({"id": "all"})), &host).result,
            Some(json!({"status": "revoked", "count": 0}))
        );
    }

    /// The parameters are checked before the pairings, and the role after them, with trust-runtime's
    /// texts. Catches a reordered check or a changed text.
    #[test]
    fn the_parameters_are_checked_first_and_the_role_after_the_pairings() {
        let host = Host::without_pairings();
        assert_eq!(
            claim(1, None, &host).error.as_deref(),
            Some("missing params")
        );
        assert_eq!(
            claim(2, Some(json!({"role": "viewer"})), &host)
                .error
                .as_deref(),
            Some("invalid params: missing field `code`")
        );
        assert_eq!(
            revoke(3, None, &host).error.as_deref(),
            Some("missing params")
        );
        assert_eq!(
            revoke(4, Some(json!({})), &host).error.as_deref(),
            Some("invalid params: missing field `id`")
        );
        assert_eq!(
            claim(5, Some(json!({"code": "000000", "role": "root"})), &host)
                .error
                .as_deref(),
            Some("pairing unavailable")
        );

        let host = Host::new(None);
        assert_eq!(
            claim(6, Some(json!({"code": "000000", "role": "root"})), &host)
                .error
                .as_deref(),
            Some("invalid role")
        );
    }

    /// A write that fails answers the port's text, here a chip's full record, and stores no token.
    /// Catches a failed write reported as a token.
    #[test]
    fn a_write_that_fails_answers_the_ports_text() {
        let host = Host::new(Some("the device has no room for another pairing"));
        assert!(start(1, &host).ok);
        let refused = claim(2, Some(json!({"code": "000000"})), &host);
        assert!(!refused.ok);
        assert_eq!(
            refused.error.as_deref(),
            Some("the device has no room for another pairing")
        );
        assert_eq!(list(3, &host).result, Some(json!({"tokens": []})));
    }
}
