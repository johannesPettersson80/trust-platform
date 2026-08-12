use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use super::*;
use crate::security::pairing::PairingStore;
use smol_str::SmolStr;

const SOURCE: &str = r#"
PROGRAM Main
END_PROGRAM
"#;

static NEXT_PATH: AtomicU64 = AtomicU64::new(1);

fn state() -> ControlState {
    crate::control::tests::hmi_test_state(SOURCE)
}

fn request(auth: Option<&str>) -> ControlRequest {
    ControlRequest {
        id: 1,
        r#type: "status".into(),
        params: None,
        auth: auth.map(str::to_string),
        request_id: None,
    }
}

fn temp_pairing_path(label: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "trust-auth-contract-{}-{label}-{}.json",
        std::process::id(),
        NEXT_PATH.fetch_add(1, Ordering::Relaxed)
    ))
}

fn pairing_store(role: AccessRole) -> (Arc<PairingStore>, String, PathBuf) {
    let path = temp_pairing_path("pairing");
    let store = Arc::new(PairingStore::with_clock(path.clone(), Arc::new(|| 1_000)));
    let code = store.start_pairing();
    let token = store.claim(&code.code, Some(role)).expect("pairing token");
    (store, token, path)
}

#[test]
fn auth_failures_have_stable_distinct_messages_and_codes() {
    assert_eq!(AuthFailure::MissingToken.message(), "missing auth token");
    assert_eq!(AuthFailure::MissingToken.code(), "missing_auth_token");
    assert_eq!(AuthFailure::InvalidToken.message(), "invalid auth token");
    assert_eq!(AuthFailure::InvalidToken.code(), "invalid_auth_token");
    assert_ne!(AuthFailure::MissingToken, AuthFailure::InvalidToken);
}

#[test]
fn configured_primary_token_grants_admin_only_on_exact_match() {
    let mut state = state();
    state.auth_token = Arc::new(Mutex::new(Some(SmolStr::new("Admin-Token"))));
    state.control_requires_auth = true;

    assert_eq!(
        resolve_request_role(&request(Some("Admin-Token")), &state, Some("tcp:1")),
        Ok(AccessRole::Admin)
    );
    for provided in [
        "admin-token",
        " Admin-Token",
        "Admin-Token ",
        "Admin-Token\0",
    ] {
        assert_eq!(
            resolve_request_role(&request(Some(provided)), &state, Some("unix")),
            Err(AuthFailure::InvalidToken),
            "provided={provided:?}"
        );
    }
}

#[test]
fn configured_primary_token_distinguishes_missing_from_invalid() {
    let mut state = state();
    state.auth_token = Arc::new(Mutex::new(Some(SmolStr::new("secret"))));
    state.control_requires_auth = true;

    assert_eq!(
        resolve_request_role(&request(None), &state, Some("unix")),
        Err(AuthFailure::MissingToken)
    );
    assert_eq!(
        resolve_request_role(&request(Some("wrong")), &state, Some("unix")),
        Err(AuthFailure::InvalidToken)
    );
}

#[test]
fn valid_pairing_token_is_fallback_when_primary_token_differs() {
    let (store, token, path) = pairing_store(AccessRole::Viewer);
    let mut state = state();
    state.auth_token = Arc::new(Mutex::new(Some(SmolStr::new("admin-token"))));
    state.control_requires_auth = true;
    state.pairing = Some(store);

    assert_eq!(
        resolve_request_role(&request(Some(&token)), &state, Some("tcp:1")),
        Ok(AccessRole::Viewer)
    );
    let _ = std::fs::remove_file(path);
}

#[test]
fn primary_token_takes_precedence_over_any_pairing_role() {
    let (store, _token, path) = pairing_store(AccessRole::Viewer);
    let mut state = state();
    state.auth_token = Arc::new(Mutex::new(Some(SmolStr::new("admin-token"))));
    state.control_requires_auth = true;
    state.pairing = Some(store);

    assert_eq!(
        resolve_request_role(&request(Some("admin-token")), &state, Some("tcp:1")),
        Ok(AccessRole::Admin)
    );
    let _ = std::fs::remove_file(path);
}

#[test]
fn pairing_token_grants_stored_role_without_primary_token() {
    for requested in [
        AccessRole::Viewer,
        AccessRole::Operator,
        AccessRole::Engineer,
    ] {
        let (store, token, path) = pairing_store(requested);
        let mut state = state();
        state.auth_token = Arc::new(Mutex::new(None));
        state.control_requires_auth = true;
        state.pairing = Some(store);

        assert_eq!(
            resolve_request_role(&request(Some(&token)), &state, Some("tcp:1")),
            Ok(requested)
        );
        let _ = std::fs::remove_file(path);
    }
}

#[test]
fn required_auth_without_primary_token_still_distinguishes_failure_kind() {
    let mut state = state();
    state.auth_token = Arc::new(Mutex::new(None));
    state.control_requires_auth = true;
    state.pairing = None;

    assert_eq!(
        resolve_request_role(&request(None), &state, None),
        Err(AuthFailure::MissingToken)
    );
    assert_eq!(
        resolve_request_role(&request(Some("unknown")), &state, None),
        Err(AuthFailure::InvalidToken)
    );
}

#[test]
fn optional_auth_accepts_valid_pairing_before_transport_default() {
    let (store, token, path) = pairing_store(AccessRole::Operator);
    let mut state = state();
    state.auth_token = Arc::new(Mutex::new(None));
    state.control_requires_auth = false;
    state.pairing = Some(store);

    assert_eq!(
        resolve_request_role(&request(Some(&token)), &state, Some("127.0.0.1:9000")),
        Ok(AccessRole::Operator)
    );
    let _ = std::fs::remove_file(path);
}

#[test]
fn optional_auth_rejects_no_token_only_by_transport_authority() {
    let mut state = state();
    state.auth_token = Arc::new(Mutex::new(None));
    state.control_requires_auth = false;
    state.pairing = None;

    for client in [Some("127.0.0.1:9000"), Some("[::1]:9000"), Some("host:1")] {
        assert_eq!(
            resolve_request_role(&request(None), &state, client),
            Ok(AccessRole::Viewer),
            "client={client:?}"
        );
    }
    for client in [None, Some("unix"), Some("local"), Some("")] {
        assert_eq!(
            resolve_request_role(&request(None), &state, client),
            Ok(AccessRole::Admin),
            "client={client:?}"
        );
    }
}

#[test]
fn optional_auth_ignores_invalid_supplied_token_only_when_auth_is_not_required() {
    let mut state = state();
    state.auth_token = Arc::new(Mutex::new(None));
    state.control_requires_auth = false;
    state.pairing = None;

    assert_eq!(
        resolve_request_role(&request(Some("unknown")), &state, Some("tcp:1")),
        Ok(AccessRole::Viewer)
    );
    assert_eq!(
        resolve_request_role(&request(Some("unknown")), &state, Some("unix")),
        Ok(AccessRole::Admin)
    );
}

#[test]
fn untrusted_transport_classifier_uses_exact_local_and_network_rules() {
    assert!(!control_client_is_untrusted_transport(None));
    assert!(!control_client_is_untrusted_transport(Some("unix")));
    assert!(!control_client_is_untrusted_transport(Some("local")));
    assert!(!control_client_is_untrusted_transport(Some("")));
    assert!(control_client_is_untrusted_transport(Some("tcp:9000")));
    assert!(control_client_is_untrusted_transport(Some(
        "127.0.0.1:9000"
    )));
    assert!(control_client_is_untrusted_transport(Some("[::1]:9000")));
    assert!(control_client_is_untrusted_transport(Some("unix:other")));
}

#[test]
fn poisoned_credential_state_fails_closed() {
    let mut state = state();
    state.auth_token = Arc::new(Mutex::new(Some(SmolStr::new("secret"))));
    state.control_requires_auth = false;
    let token = Arc::clone(&state.auth_token);
    let poisoned = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
        let _guard = token.lock().expect("lock before poison");
        panic!("intentional poison");
    }));
    assert!(poisoned.is_err());

    assert_eq!(
        resolve_request_role(&request(None), &state, Some("unix")),
        Err(AuthFailure::InvalidToken)
    );
    assert_eq!(
        resolve_request_role(&request(Some("secret")), &state, Some("unix")),
        Err(AuthFailure::InvalidToken)
    );
}
