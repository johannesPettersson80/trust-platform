use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use tiny_http::{Header, Request, TestRequest};

use super::*;

static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(1);

fn header(name: &str, value: &str) -> Header {
    Header::from_bytes(name.as_bytes(), value.as_bytes()).expect("valid header")
}

fn request(remote: &str, headers: &[(&str, &str)]) -> Request {
    let mut request =
        TestRequest::new().with_remote_addr(remote.parse::<SocketAddr>().expect("socket"));
    for (name, value) in headers {
        request = request.with_header(header(name, value));
    }
    request.into()
}

fn primary_token(value: Option<&str>) -> Arc<Mutex<Option<smol_str::SmolStr>>> {
    Arc::new(Mutex::new(value.map(Into::into)))
}

fn response_json(response: Response<std::io::Cursor<Vec<u8>>>) -> serde_json::Value {
    serde_json::from_slice(response.into_reader().get_ref()).expect("JSON response")
}

fn response_has_json_content_type(response: &Response<std::io::Cursor<Vec<u8>>>) -> bool {
    response.headers().iter().any(|header| {
        header.field.equiv("Content-Type")
            && header
                .value
                .as_str()
                .eq_ignore_ascii_case("application/json")
    })
}

struct PairingFixture {
    root: PathBuf,
    store: PairingStore,
    token: String,
}

impl PairingFixture {
    fn new(role: AccessRole) -> Self {
        let root = std::env::temp_dir().join(format!(
            "trust-web-auth-contract-{}-{}",
            std::process::id(),
            NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed)
        ));
        let store = PairingStore::with_clock(root.join("pairing.json"), Arc::new(|| 1_000));
        let code = store.start_pairing();
        let token = store.claim(&code.code, Some(role)).expect("pairing token");
        Self { root, store, token }
    }
}

impl Drop for PairingFixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

#[test]
fn local_mode_grants_admin_to_ipv4_loopback() {
    let request = request("127.0.0.1:4100", &[]);
    let result = check_auth_with_role(
        &request,
        WebAuthMode::Local,
        &primary_token(Some("ignored")),
        None,
        AccessRole::Admin,
    );
    assert_eq!(result, Ok((AccessRole::Admin, None)));
}

#[test]
fn local_mode_grants_admin_to_ipv6_loopback() {
    let request = request("[::1]:4100", &[]);
    let result = check_auth_with_role(
        &request,
        WebAuthMode::Local,
        &primary_token(None),
        None,
        AccessRole::Admin,
    );
    assert_eq!(result, Ok((AccessRole::Admin, None)));
}

#[test]
fn local_mode_rejects_non_loopback_even_with_a_token_header() {
    let request = request("192.0.2.10:4100", &[("X-Trust-Token", "primary")]);
    let result = check_auth_with_role(
        &request,
        WebAuthMode::Local,
        &primary_token(Some("primary")),
        None,
        AccessRole::Viewer,
    );
    assert_eq!(result, Err("unauthorized"));
}

#[test]
fn token_mode_primary_token_grants_admin_and_is_forwarded() {
    let request = request("192.0.2.10:4100", &[("X-Trust-Token", "primary")]);
    let result = check_auth_with_role(
        &request,
        WebAuthMode::Token,
        &primary_token(Some("primary")),
        None,
        AccessRole::Admin,
    );
    assert_eq!(result, Ok((AccessRole::Admin, Some("primary".to_string()))));
}

#[test]
fn token_header_name_is_ascii_case_insensitive() {
    let request = request("192.0.2.10:4100", &[("x-TrUsT-tOkEn", "primary")]);
    assert_eq!(
        check_auth(
            &request,
            WebAuthMode::Token,
            &primary_token(Some("primary")),
            None,
            AccessRole::Engineer,
        ),
        Ok(Some("primary".to_string()))
    );
}

#[test]
fn token_mode_rejects_a_missing_header() {
    let request = request("127.0.0.1:4100", &[]);
    assert_eq!(
        check_auth(
            &request,
            WebAuthMode::Token,
            &primary_token(Some("primary")),
            None,
            AccessRole::Viewer,
        ),
        Err("unauthorized")
    );
}

#[test]
fn token_mode_rejects_an_unknown_header_value() {
    let request = request("127.0.0.1:4100", &[("X-Trust-Token", "unknown")]);
    assert_eq!(
        check_auth(
            &request,
            WebAuthMode::Token,
            &primary_token(Some("primary")),
            None,
            AccessRole::Viewer,
        ),
        Err("unauthorized")
    );
}

#[test]
fn token_values_are_byte_exact_and_not_trimmed() {
    let request = request("127.0.0.1:4100", &[("X-Trust-Token", " primary ")]);
    assert_eq!(
        check_auth(
            &request,
            WebAuthMode::Token,
            &primary_token(Some("primary")),
            None,
            AccessRole::Viewer,
        ),
        Err("unauthorized")
    );
}

#[test]
fn pairing_token_grants_its_stored_role_and_is_forwarded() {
    let pairing = PairingFixture::new(AccessRole::Engineer);
    let request = request(
        "192.0.2.10:4100",
        &[("X-Trust-Token", pairing.token.as_str())],
    );
    assert_eq!(
        check_auth_with_role(
            &request,
            WebAuthMode::Token,
            &primary_token(Some("primary")),
            Some(&pairing.store),
            AccessRole::Engineer,
        ),
        Ok((AccessRole::Engineer, Some(pairing.token.clone())))
    );
}

#[test]
fn pairing_role_below_the_requirement_is_forbidden() {
    let pairing = PairingFixture::new(AccessRole::Viewer);
    let request = request(
        "192.0.2.10:4100",
        &[("X-Trust-Token", pairing.token.as_str())],
    );
    assert_eq!(
        check_auth_with_role(
            &request,
            WebAuthMode::Token,
            &primary_token(None),
            Some(&pairing.store),
            AccessRole::Operator,
        ),
        Err("forbidden")
    );
}

#[test]
fn pairing_token_is_rejected_when_no_pairing_store_is_configured() {
    let request = request("127.0.0.1:4100", &[("X-Trust-Token", "pairing")]);
    assert_eq!(
        check_auth(
            &request,
            WebAuthMode::Token,
            &primary_token(None),
            None,
            AccessRole::Viewer,
        ),
        Err("unauthorized")
    );
}

#[test]
fn poisoned_primary_credential_state_fails_closed_before_pairing() {
    let pairing = PairingFixture::new(AccessRole::Engineer);
    let primary = primary_token(Some("primary"));
    let poison_target = Arc::clone(&primary);
    let _ = std::thread::spawn(move || {
        let _guard = poison_target.lock().expect("lock before poison");
        panic!("poison credential state");
    })
    .join();
    let request = request(
        "127.0.0.1:4100",
        &[("X-Trust-Token", pairing.token.as_str())],
    );

    assert_eq!(
        check_auth(
            &request,
            WebAuthMode::Token,
            &primary,
            Some(&pairing.store),
            AccessRole::Viewer,
        ),
        Err("unauthorized")
    );
}

#[test]
fn unauthorized_response_has_stable_status_and_payload() {
    let response = auth_error_response("unauthorized");
    assert_eq!(response.status_code(), StatusCode(401));
    assert_eq!(
        response_json(response),
        json!({"ok": false, "error": "unauthorized"})
    );
}

#[test]
fn forbidden_response_has_stable_status_and_payload() {
    let response = auth_error_response("forbidden");
    assert_eq!(response.status_code(), StatusCode(403));
    assert_eq!(
        response_json(response),
        json!({"ok": false, "error": "forbidden"})
    );
}

#[test]
fn authentication_denials_are_declared_as_json() {
    assert!(response_has_json_content_type(&auth_error_response(
        "unauthorized"
    )));
    assert!(response_has_json_content_type(&auth_error_response(
        "forbidden"
    )));
}

#[test]
fn ide_session_header_name_is_ascii_case_insensitive() {
    let request = request("127.0.0.1:4100", &[("x-TrUsT-iDe-SeSsIoN", "session-17")]);
    assert_eq!(ide_session_token(&request).as_deref(), Some("session-17"));
}

#[test]
fn ide_session_value_is_preserved_exactly() {
    let request = request("127.0.0.1:4100", &[("X-Trust-Ide-Session", " session-17 ")]);
    assert_eq!(ide_session_token(&request).as_deref(), Some(" session-17 "));
}

#[test]
fn absent_ide_session_header_returns_none() {
    let request = request("127.0.0.1:4100", &[]);
    assert_eq!(ide_session_token(&request), None);
}
