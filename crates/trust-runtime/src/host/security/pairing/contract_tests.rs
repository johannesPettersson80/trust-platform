use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use super::*;

static NEXT_PATH: AtomicU64 = AtomicU64::new(1);

fn temp_path(label: &str) -> PathBuf {
    std::env::temp_dir()
        .join(format!(
            "trust-pairing-contract-{}-{label}-{}",
            std::process::id(),
            NEXT_PATH.fetch_add(1, Ordering::Relaxed)
        ))
        .join("pairing.json")
}

fn clock(initial: u64) -> (Arc<AtomicU64>, Arc<dyn Fn() -> u64 + Send + Sync>) {
    let time = Arc::new(AtomicU64::new(initial));
    let source = {
        let time = Arc::clone(&time);
        Arc::new(move || time.load(Ordering::SeqCst)) as Arc<dyn Fn() -> u64 + Send + Sync>
    };
    (time, source)
}

fn token(id: &str, value: &str, created_at: u64, expires_at: u64) -> PairingToken {
    PairingToken {
        id: id.into(),
        token: value.into(),
        created_at,
        enabled: true,
        role: AccessRole::Operator,
        expires_at,
    }
}

fn cleanup(path: &Path) {
    if let Some(parent) = path.parent() {
        let _ = fs::remove_dir_all(parent);
    }
}

#[test]
fn generated_pairing_codes_have_exact_six_digit_shape() {
    for _ in 0..32 {
        let code = generate_code();
        assert_eq!(code.len(), 6);
        assert!(code.bytes().all(|byte| byte.is_ascii_digit()));
    }
}

#[test]
fn generated_tokens_encode_exact_random_byte_width_without_padding() {
    for _ in 0..16 {
        let token = generate_token();
        assert_eq!(
            URL_SAFE_NO_PAD
                .decode(token.as_bytes())
                .expect("decode token")
                .len(),
            TOKEN_BYTES
        );
        assert!(!token.contains('='));
        assert!(token
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_')));
    }
}

#[test]
fn token_tail_masks_all_but_last_four_characters() {
    assert_eq!(mask_tail("abcdefgh"), "…efgh");
    assert_eq!(mask_tail("abcd"), "…abcd");
    assert_eq!(mask_tail("abc"), "…abc");
    assert_eq!(mask_tail(""), "…");
    assert_eq!(mask_tail("åäö日本語"), "…ö日本語");
}

#[test]
fn default_pairing_role_is_operator_and_admin_is_reduced() {
    assert_eq!(default_token_role(), AccessRole::Operator);
    assert_eq!(sanitize_requested_role(None), AccessRole::Operator);
    assert_eq!(
        sanitize_requested_role(Some(AccessRole::Viewer)),
        AccessRole::Viewer
    );
    assert_eq!(
        sanitize_requested_role(Some(AccessRole::Operator)),
        AccessRole::Operator
    );
    assert_eq!(
        sanitize_requested_role(Some(AccessRole::Engineer)),
        AccessRole::Engineer
    );
    assert_eq!(
        sanitize_requested_role(Some(AccessRole::Admin)),
        AccessRole::Engineer
    );
}

#[test]
fn absent_pairing_file_loads_an_empty_token_set() {
    let path = temp_path("absent");
    assert!(load_tokens(&path).expect("missing file").is_empty());
    let store = PairingStore::with_clock(path.clone(), Arc::new(|| 1_000));
    assert!(store.list().is_empty());
    cleanup(&path);
}

#[test]
fn malformed_pairing_file_grants_no_token_authority() {
    let path = temp_path("malformed");
    fs::create_dir_all(path.parent().expect("parent")).expect("create parent");
    fs::write(&path, b"{ definitely not json").expect("write malformed file");

    assert!(load_tokens(&path)
        .expect("malformed is fail-closed")
        .is_empty());
    let store = PairingStore::with_clock(path.clone(), Arc::new(|| 1_000));
    assert!(!store.validate("anything"));
    assert!(store.list().is_empty());
    cleanup(&path);
}

#[test]
fn missing_role_in_legacy_file_defaults_to_operator() {
    let path = temp_path("default-role");
    fs::create_dir_all(path.parent().expect("parent")).expect("create parent");
    fs::write(
        &path,
        br#"{
  "tokens": [{
    "id": "legacy",
    "token": "legacy-token",
    "created_at": 100,
    "enabled": true,
    "expires_at": 1000
  }]
}"#,
    )
    .expect("write legacy file");

    let loaded = load_tokens(&path).expect("load tokens");
    assert_eq!(loaded.len(), 1);
    assert_eq!(loaded[0].role, AccessRole::Operator);
    cleanup(&path);
}

#[test]
fn legacy_zero_expiry_derives_creation_plus_ttl() {
    let mut tokens = vec![token("legacy", "token", 100, 0)];
    normalize_loaded_tokens(&mut tokens, 200);
    assert_eq!(
        tokens[0].expires_at,
        100_u64.saturating_add(PAIRING_TOKEN_TTL_SECS)
    );
}

#[test]
fn legacy_expiry_derivation_saturates_at_u64_max() {
    let mut tokens = vec![token("legacy", "token", u64::MAX - 1, 0)];
    normalize_loaded_tokens(&mut tokens, 1);
    assert_eq!(tokens[0].expires_at, u64::MAX);
}

#[test]
fn expired_legacy_token_is_not_temporarily_revived() {
    let mut tokens = vec![token("legacy", "token", 1, 0)];
    normalize_loaded_tokens(&mut tokens, PAIRING_TOKEN_TTL_SECS + 100);
    assert_eq!(
        tokens[0].expires_at,
        1_u64.saturating_add(PAIRING_TOKEN_TTL_SECS)
    );
    assert!(prune_expired_tokens(
        &mut tokens,
        PAIRING_TOKEN_TTL_SECS + 100
    ));
    assert!(tokens.is_empty());
}

#[test]
fn expiry_pruning_is_inclusive_at_exact_expiry_second() {
    let mut tokens = vec![
        token("expired", "a", 1, 99),
        token("boundary", "b", 1, 100),
        token("future", "c", 1, 101),
    ];
    assert!(prune_expired_tokens(&mut tokens, 100));
    assert_eq!(
        tokens
            .iter()
            .map(|entry| entry.id.as_str())
            .collect::<Vec<_>>(),
        ["boundary", "future"]
    );
    assert!(!prune_expired_tokens(&mut tokens, 100));
}

#[test]
fn starting_pairing_uses_exact_ttl_and_replaces_pending_session() {
    let path = temp_path("start");
    let (time, clock) = clock(1_000);
    let store = PairingStore::with_clock(path.clone(), clock);

    let first = store.start_pairing();
    assert_eq!(first.expires_at, 1_000 + PAIRING_CODE_TTL_SECS);
    assert_eq!(first.code.len(), 6);

    time.store(1_001, Ordering::SeqCst);
    let second = store.start_pairing();
    assert_eq!(second.expires_at, 1_001 + PAIRING_CODE_TTL_SECS);
    let guard = store.state.lock().expect("pairing state");
    assert_eq!(
        guard.pending.as_ref().map(|pending| pending.code.as_str()),
        Some(second.code.as_str())
    );
    assert_eq!(
        guard.pending.as_ref().map(|pending| pending.expires_at),
        Some(second.expires_at)
    );
    drop(guard);
    cleanup(&path);
}

#[test]
fn wrong_code_does_not_consume_pending_pairing_session() {
    let path = temp_path("wrong-code");
    let store = PairingStore::with_clock(path.clone(), Arc::new(|| 1_000));
    let code = store.start_pairing();

    assert_eq!(store.claim("wrong", None), None);
    assert!(store.claim(&code.code, None).is_some());
    cleanup(&path);
}

#[test]
fn claim_trims_code_and_consumes_successful_session() {
    let path = temp_path("trim-code");
    let store = PairingStore::with_clock(path.clone(), Arc::new(|| 1_000));
    let code = store.start_pairing();

    let token = store
        .claim(&format!(" \t{}\n", code.code), None)
        .expect("trimmed claim");
    assert!(store.validate(&token));
    assert_eq!(store.claim(&code.code, None), None);
    cleanup(&path);
}

#[test]
fn pairing_code_remains_valid_at_exact_expiry_second() {
    let path = temp_path("code-boundary");
    let (time, clock) = clock(1_000);
    let store = PairingStore::with_clock(path.clone(), clock);
    let code = store.start_pairing();

    time.store(code.expires_at, Ordering::SeqCst);
    assert!(store.claim(&code.code, None).is_some());
    cleanup(&path);
}

#[test]
fn expired_code_is_consumed_and_cannot_be_retried() {
    let path = temp_path("code-expired");
    let (time, clock) = clock(1_000);
    let store = PairingStore::with_clock(path.clone(), clock);
    let code = store.start_pairing();

    time.store(code.expires_at + 1, Ordering::SeqCst);
    assert_eq!(store.claim(&code.code, None), None);
    time.store(code.expires_at, Ordering::SeqCst);
    assert_eq!(store.claim(&code.code, None), None);
    cleanup(&path);
}

#[test]
fn claims_preserve_allowed_roles_and_reduce_admin() {
    for (requested, expected) in [
        (None, AccessRole::Operator),
        (Some(AccessRole::Viewer), AccessRole::Viewer),
        (Some(AccessRole::Operator), AccessRole::Operator),
        (Some(AccessRole::Engineer), AccessRole::Engineer),
        (Some(AccessRole::Admin), AccessRole::Engineer),
    ] {
        let path = temp_path(expected.as_str());
        let store = PairingStore::with_clock(path.clone(), Arc::new(|| 1_000));
        let code = store.start_pairing();
        let token = store.claim(&code.code, requested).expect("claim");
        assert_eq!(store.validate_with_role(&token), Some(expected));
        cleanup(&path);
    }
}

#[test]
fn claimed_token_has_exact_lifetime_and_masked_summary() {
    let path = temp_path("summary");
    let store = PairingStore::with_clock(path.clone(), Arc::new(|| 1_000));
    let code = store.start_pairing();
    let token = store
        .claim(&code.code, Some(AccessRole::Viewer))
        .expect("claim");

    let list = store.list();
    assert_eq!(list.len(), 1);
    assert!(list[0].enabled);
    assert_eq!(list[0].created_at, 1_000);
    assert_eq!(list[0].expires_at, 1_000 + PAIRING_TOKEN_TTL_SECS);
    assert_eq!(list[0].role, AccessRole::Viewer);
    assert_eq!(list[0].tail, mask_tail(&token));
    let serialized = serde_json::to_value(&list[0]).expect("serialize summary");
    assert!(serialized.get("token").is_none());
    assert_eq!(serialized.get("role"), Some(&serde_json::json!("viewer")));
    cleanup(&path);
}

#[test]
fn token_is_valid_through_exact_expiry_then_pruned() {
    let path = temp_path("token-boundary");
    let (time, clock) = clock(1_000);
    let store = PairingStore::with_clock(path.clone(), clock);
    let code = store.start_pairing();
    let token = store.claim(&code.code, None).expect("claim");
    let expires_at = 1_000 + PAIRING_TOKEN_TTL_SECS;

    time.store(expires_at, Ordering::SeqCst);
    assert!(store.validate(&token));
    time.store(expires_at + 1, Ordering::SeqCst);
    assert!(!store.validate(&token));
    assert!(store.list().is_empty());
    cleanup(&path);
}

#[test]
fn disabled_token_never_validates_but_remains_listed_until_expiry() {
    let path = temp_path("disabled");
    let store = PairingStore::with_clock(path.clone(), Arc::new(|| 1_000));
    let code = store.start_pairing();
    let token = store.claim(&code.code, None).expect("claim");
    let id = store.list()[0].id.clone();

    assert!(store.revoke(&id));
    assert!(!store.validate(&token));
    let list = store.list();
    assert_eq!(list.len(), 1);
    assert!(!list[0].enabled);
    cleanup(&path);
}

#[test]
fn revoke_is_exact_and_absent_id_is_idempotently_false() {
    let path = temp_path("revoke");
    let store = PairingStore::with_clock(path.clone(), Arc::new(|| 1_000));
    {
        let mut state = store.state.lock().expect("pairing state");
        state.tokens = vec![
            token("one", "token-one", 1_000, 2_000),
            token("two", "token-two", 1_000, 2_000),
        ];
    }

    assert!(!store.revoke("missing"));
    assert!(store.revoke("one"));
    assert!(!store.validate("token-one"));
    assert!(store.validate("token-two"));
    assert!(!store.revoke("missing"));
    cleanup(&path);
}

#[test]
fn revoke_all_counts_only_enabled_transitions() {
    let path = temp_path("revoke-all");
    let store = PairingStore::with_clock(path.clone(), Arc::new(|| 1_000));
    {
        let mut state = store.state.lock().expect("pairing state");
        let first = token("one", "token-one", 1_000, 2_000);
        let mut second = token("two", "token-two", 1_000, 2_000);
        second.enabled = false;
        let third = token("three", "token-three", 1_000, 2_000);
        state.tokens = vec![first, second, third];
    }

    assert_eq!(store.revoke_all(), 2);
    assert_eq!(store.revoke_all(), 0);
    assert!(store.list().iter().all(|entry| !entry.enabled));
    cleanup(&path);
}

#[test]
fn enabled_token_limit_rejects_claim_without_adding_token() {
    let path = temp_path("limit");
    let store = PairingStore::with_clock(path.clone(), Arc::new(|| 1_000));
    {
        let mut state = store.state.lock().expect("pairing state");
        state.tokens = (0..PAIRING_MAX_TOKENS)
            .map(|index| {
                token(
                    &format!("id-{index}"),
                    &format!("token-{index}"),
                    1_000,
                    2_000,
                )
            })
            .collect();
    }
    let code = store.start_pairing();

    assert_eq!(store.claim(&code.code, None), None);
    assert_eq!(store.list().len(), PAIRING_MAX_TOKENS);
    cleanup(&path);
}

#[test]
fn disabled_tokens_do_not_count_toward_enabled_limit() {
    let path = temp_path("disabled-limit");
    let store = PairingStore::with_clock(path.clone(), Arc::new(|| 1_000));
    {
        let mut state = store.state.lock().expect("pairing state");
        state.tokens = (0..PAIRING_MAX_TOKENS)
            .map(|index| {
                let mut entry = token(
                    &format!("id-{index}"),
                    &format!("token-{index}"),
                    1_000,
                    2_000,
                );
                entry.enabled = false;
                entry
            })
            .collect();
    }
    let code = store.start_pairing();

    assert!(store.claim(&code.code, None).is_some());
    assert_eq!(store.list().len(), PAIRING_MAX_TOKENS + 1);
    cleanup(&path);
}

#[test]
fn claims_in_same_clock_second_receive_unique_ids() {
    let path = temp_path("unique-id");
    let store = PairingStore::with_clock(path.clone(), Arc::new(|| 1_000));
    for _ in 0..2 {
        let code = store.start_pairing();
        assert!(store.claim(&code.code, None).is_some());
    }

    let ids = store
        .list()
        .into_iter()
        .map(|entry| entry.id)
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(ids.len(), 2);
    cleanup(&path);
}

#[test]
fn successful_claim_persists_and_reloads_exact_authority() {
    let path = temp_path("reload");
    let store = PairingStore::with_clock(path.clone(), Arc::new(|| 1_000));
    let code = store.start_pairing();
    let token = store
        .claim(&code.code, Some(AccessRole::Engineer))
        .expect("claim");
    drop(store);

    let loaded = PairingStore::with_clock(path.clone(), Arc::new(|| 1_001));
    assert_eq!(
        loaded.validate_with_role(&token),
        Some(AccessRole::Engineer)
    );
    assert_eq!(loaded.list().len(), 1);
    cleanup(&path);
}

#[test]
fn successful_revoke_persists_across_reload() {
    let path = temp_path("revoke-reload");
    let store = PairingStore::with_clock(path.clone(), Arc::new(|| 1_000));
    let code = store.start_pairing();
    let token = store.claim(&code.code, None).expect("claim");
    let id = store.list()[0].id.clone();
    assert!(store.revoke(&id));
    drop(store);

    let loaded = PairingStore::with_clock(path.clone(), Arc::new(|| 1_001));
    assert!(!loaded.validate(&token));
    assert_eq!(loaded.list().len(), 1);
    assert!(!loaded.list()[0].enabled);
    cleanup(&path);
}

#[test]
fn claim_fails_without_memory_authority_when_persistence_fails() {
    let path = temp_path("save-failure");
    let parent = path.parent().expect("parent");
    fs::create_dir_all(parent.parent().expect("grandparent")).expect("create grandparent");
    fs::write(parent, b"not a directory").expect("block pairing parent");
    let store = PairingStore::with_clock(path.clone(), Arc::new(|| 1_000));
    let code = store.start_pairing();

    let claimed = store.claim(&code.code, None);
    assert_eq!(claimed, None);
    assert!(store.list().is_empty());
    let _ = fs::remove_file(parent);
}

#[cfg(unix)]
#[test]
fn persisted_pairing_file_is_mode_0600() {
    use std::os::unix::fs::PermissionsExt;

    let path = temp_path("mode");
    save_tokens(&path, &[token("one", "secret", 1_000, 2_000)]).expect("save tokens");
    let mode = fs::metadata(&path).expect("metadata").permissions().mode() & 0o777;
    assert_eq!(mode, 0o600);
    cleanup(&path);
}

#[test]
fn pairing_debug_projection_never_includes_pending_code_or_tokens() {
    let path = temp_path("debug");
    let store = PairingStore::with_clock(path.clone(), Arc::new(|| 1_000));
    let code = store.start_pairing();
    let token = store.claim(&code.code, None).expect("claim");

    let debug = format!("{store:?}");
    assert!(debug.contains("PairingStore"));
    assert!(debug.contains("path"));
    assert!(!debug.contains(&code.code));
    assert!(!debug.contains(&token));
    cleanup(&path);
}
