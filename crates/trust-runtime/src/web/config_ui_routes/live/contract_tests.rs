use super::*;

#[test]
fn live_target_is_trimmed_and_receives_default_http_scheme() {
    assert_eq!(
        normalize_live_target("  runtime.example:8080  ").expect("target"),
        "http://runtime.example:8080"
    );
}

#[test]
fn explicit_http_and_https_schemes_are_preserved() {
    assert_eq!(
        normalize_live_target("http://runtime.example").expect("HTTP"),
        "http://runtime.example"
    );
    assert_eq!(
        normalize_live_target("https://runtime.example").expect("HTTPS"),
        "https://runtime.example"
    );
}

#[test]
fn one_terminal_slash_is_removed() {
    assert_eq!(
        normalize_live_target("https://runtime.example/").expect("target"),
        "https://runtime.example"
    );
}

#[test]
fn ipv4_ipv6_and_hostname_authorities_are_accepted() {
    for target in [
        "127.0.0.1:8080",
        "[::1]:8080",
        "runtime.example",
        "runtime.example:8443",
    ] {
        assert!(normalize_live_target(target).is_ok(), "{target}");
    }
}

#[test]
fn empty_target_is_rejected() {
    for target in ["", " ", "\t\n"] {
        assert!(normalize_live_target(target).is_err(), "{target:?}");
    }
}

#[test]
fn scheme_without_authority_is_rejected() {
    for target in ["http://", "https://", "http:", "https:"] {
        assert!(normalize_live_target(target).is_err(), "{target}");
    }
}

#[test]
fn unsupported_or_case_variant_schemes_are_rejected() {
    for target in [
        "ftp://runtime.example",
        "ws://runtime.example",
        "HTTP://runtime.example",
        "HTTPS://runtime.example",
    ] {
        assert!(normalize_live_target(target).is_err(), "{target}");
    }
}

#[test]
fn live_target_rejects_user_info() {
    assert!(normalize_live_target("http://user@runtime.example").is_err());
    assert!(normalize_live_target("http://user:pass@runtime.example").is_err());
}

#[test]
fn live_target_rejects_paths_queries_and_fragments() {
    for target in [
        "http://runtime.example/api",
        "http://runtime.example?x=1",
        "http://runtime.example#fragment",
    ] {
        assert!(normalize_live_target(target).is_err(), "{target}");
    }
}

#[test]
fn live_target_rejects_multiple_terminal_slashes() {
    assert!(normalize_live_target("http://runtime.example//").is_err());
}

#[test]
fn empty_live_manager_snapshot_has_stable_shape() {
    let snapshot = config_ui_live_targets_snapshot_with_guard(&ConfigUiLiveManagerState::default());
    assert_eq!(
        snapshot,
        json!({
            "ok": true,
            "profiles": [],
            "active_target": null,
            "connected": false,
            "last_error": null,
            "updated_at_ns": 0,
        })
    );
}

#[test]
fn live_manager_snapshot_orders_profiles_by_normalized_target_key() {
    let mut state = ConfigUiLiveManagerState::default();
    state.profiles.insert(
        "https://z.example".to_string(),
        ConfigUiLiveTargetProfile {
            target: "https://z.example".to_string(),
            label: "Z".to_string(),
        },
    );
    state.profiles.insert(
        "http://a.example".to_string(),
        ConfigUiLiveTargetProfile {
            target: "http://a.example".to_string(),
            label: "A".to_string(),
        },
    );

    let snapshot = config_ui_live_targets_snapshot_with_guard(&state);
    assert_eq!(snapshot["profiles"][0]["target"], "http://a.example");
    assert_eq!(snapshot["profiles"][1]["target"], "https://z.example");
}

#[test]
fn live_manager_snapshot_preserves_connection_projection() {
    let state = ConfigUiLiveManagerState {
        active_target: Some("https://runtime.example".to_string()),
        active_token: Some("secret-token".to_string()),
        connected: true,
        last_error: None,
        last_runtime_cloud: Some(json!({"ok": true})),
        updated_at_ns: 42,
        ..ConfigUiLiveManagerState::default()
    };

    let snapshot = config_ui_live_targets_snapshot_with_guard(&state);
    assert_eq!(snapshot["active_target"], "https://runtime.example");
    assert_eq!(snapshot["connected"], true);
    assert_eq!(snapshot["updated_at_ns"], 42);
}

#[test]
fn live_manager_snapshot_never_serializes_credential_or_cached_state() {
    let state = ConfigUiLiveManagerState {
        active_target: Some("https://runtime.example".to_string()),
        active_token: Some("secret-token".to_string()),
        connected: false,
        last_error: Some("offline".to_string()),
        last_runtime_cloud: Some(json!({"credential": "cached-secret"})),
        updated_at_ns: 42,
        ..ConfigUiLiveManagerState::default()
    };

    let encoded = config_ui_live_targets_snapshot_with_guard(&state).to_string();
    assert!(!encoded.contains("secret-token"));
    assert!(!encoded.contains("cached-secret"));
    assert!(!encoded.contains("active_token"));
    assert!(!encoded.contains("last_runtime_cloud"));
    assert!(encoded.contains("offline"));
}
