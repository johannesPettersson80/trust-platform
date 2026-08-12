use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use std::time::{SystemTime, UNIX_EPOCH};

use super::*;

fn session(expires_at: u64) -> IdeSessionEntry {
    IdeSessionEntry {
        role: IdeRole::Viewer,
        expires_at,
        open_paths: BTreeSet::new(),
    }
}

#[test]
fn expiry_removes_deadline_equal_to_current_second() {
    let mut state = IdeStateInner::default();
    state.sessions.insert("expired".to_string(), session(100));
    state.sessions.insert("future".to_string(), session(101));

    prune_expired(&mut state, 100);
    assert!(!state.sessions.contains_key("expired"));
    assert!(state.sessions.contains_key("future"));
}

#[test]
fn pruning_without_expired_sessions_preserves_state() {
    let mut state = IdeStateInner::default();
    state.sessions.insert("future".to_string(), session(101));
    prune_expired(&mut state, 100);
    assert_eq!(state.sessions.len(), 1);
    assert!(state.sessions.contains_key("future"));
}

#[test]
fn session_removal_clears_all_session_owned_state() {
    let mut state = IdeStateInner::default();
    state.sessions.insert("remove".to_string(), session(100));
    state.frontend_telemetry_by_session.insert(
        "remove".to_string(),
        WebIdeFrontendTelemetry {
            bootstrap_failures: 1,
            ..WebIdeFrontendTelemetry::default()
        },
    );
    state
        .analysis_cache
        .insert("remove".to_string(), IdeAnalysisCacheEntry::default());
    state.documents.insert(
        "main.st".to_string(),
        IdeDocumentEntry {
            content: String::new(),
            version: 1,
            opened_by: BTreeSet::from(["remove".to_string(), "keep".to_string()]),
        },
    );

    remove_session(&mut state, "remove");
    assert!(!state.sessions.contains_key("remove"));
    assert!(!state.frontend_telemetry_by_session.contains_key("remove"));
    assert!(!state.analysis_cache.contains_key("remove"));
    assert_eq!(
        &state.documents["main.st"].opened_by,
        &BTreeSet::from(["keep".to_string()])
    );
}

#[test]
fn session_removal_preserves_other_session_authority_and_cache() {
    let mut state = IdeStateInner::default();
    state.sessions.insert("remove".to_string(), session(100));
    state.sessions.insert("keep".to_string(), session(200));
    state
        .analysis_cache
        .insert("remove".to_string(), IdeAnalysisCacheEntry::default());
    state
        .analysis_cache
        .insert("keep".to_string(), IdeAnalysisCacheEntry::default());

    remove_session(&mut state, "remove");
    assert!(state.sessions.contains_key("keep"));
    assert!(state.analysis_cache.contains_key("keep"));
}

#[test]
fn removing_unknown_session_is_idempotent() {
    let mut state = IdeStateInner::default();
    state.sessions.insert("keep".to_string(), session(200));
    remove_session(&mut state, "unknown");
    assert_eq!(state.sessions.len(), 1);
}

#[test]
fn generated_session_token_is_url_safe_unpadded_32_byte_value() {
    for _ in 0..16 {
        let token = generate_token();
        assert_eq!(
            URL_SAFE_NO_PAD
                .decode(token.as_bytes())
                .expect("decode token")
                .len(),
            32
        );
        assert!(!token.contains('='));
        assert!(token
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_')));
    }
}

#[test]
fn current_time_is_a_unix_epoch_second() {
    let before = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_secs();
    let observed = now_secs();
    let after = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_secs();
    assert!((before..=after).contains(&observed));
}
