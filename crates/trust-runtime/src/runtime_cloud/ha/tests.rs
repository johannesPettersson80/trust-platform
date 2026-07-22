use super::*;

fn dual_host_active_target(runtime_id: &str) -> RuntimeCloudHaTargetPolicy {
    RuntimeCloudHaTargetPolicy {
        role: RuntimeCloudHaRole::Active,
        lease_consistent_authority: true,
        lease_available: true,
        lease_owner_runtime_id: Some(runtime_id.to_string()),
        fencing_token: Some("fence-epoch-7".to_string()),
        fence_valid: true,
        ambiguous_leadership: false,
    }
}

#[test]
fn parse_action_ha_request_accepts_optional_payload() {
    let payload = serde_json::json!({
        "params": { "log.level": "debug" },
        "ha": {
            "profile": "dual_host",
            "group_id": "line-a-ha",
            "active_namespace": true,
            "command_seq": 17,
            "targets": {
                "runtime-a": {
                    "role": "active",
                    "lease_consistent_authority": true,
                    "lease_available": true,
                    "lease_owner_runtime_id": "runtime-a",
                    "fencing_token": "fence-17",
                    "fence_valid": true
                }
            }
        }
    });
    let parsed = parse_action_ha_request(&payload).expect("parse ha payload");
    let request = parsed.expect("ha payload should be present");
    assert_eq!(request.profile, RuntimeCloudHaProfile::DualHost);
    assert_eq!(request.group_id, "line-a-ha");
    assert!(request.active_namespace);
    assert_eq!(request.command_seq, Some(17));
}

#[test]
fn dual_host_requires_external_consistent_lease_authority() {
    let mut request = RuntimeCloudHaRequest {
        profile: RuntimeCloudHaProfile::DualHost,
        group_id: "line-a-ha".to_string(),
        active_namespace: true,
        command_seq: Some(4),
        targets: BTreeMap::new(),
    };
    let mut target = dual_host_active_target("runtime-a");
    target.lease_consistent_authority = false;
    request.targets.insert("runtime-a".to_string(), target);

    let decision = authority_decision("cmd_invoke", "runtime-a", &request);
    assert!(!decision.allowed);
    assert_eq!(decision.denial_code, Some(ReasonCode::LeaseUnavailable));
}

#[test]
fn lease_loss_for_active_runtime_requires_demoted_safe_behavior() {
    let mut request = RuntimeCloudHaRequest {
        profile: RuntimeCloudHaProfile::DualHost,
        group_id: "line-b-ha".to_string(),
        active_namespace: true,
        command_seq: Some(9),
        targets: BTreeMap::new(),
    };
    let mut target = dual_host_active_target("runtime-a");
    target.lease_available = false;
    request.targets.insert("runtime-a".to_string(), target);

    let decision = authority_decision("cmd_invoke", "runtime-a", &request);
    assert!(!decision.allowed);
    assert_eq!(decision.denial_code, Some(ReasonCode::LeaseUnavailable));
    assert!(decision
        .denial_reason
        .as_deref()
        .map(|value| value.contains("demoted_safe"))
        .unwrap_or(false));
}

#[test]
fn split_brain_candidates_are_detected_and_rejected() {
    let mut request = RuntimeCloudHaRequest {
        profile: RuntimeCloudHaProfile::DualHost,
        group_id: "line-c-ha".to_string(),
        active_namespace: true,
        command_seq: Some(12),
        targets: BTreeMap::new(),
    };
    request.targets.insert(
        "runtime-a".to_string(),
        dual_host_active_target("runtime-a"),
    );
    request.targets.insert(
        "runtime-b".to_string(),
        dual_host_active_target("runtime-b"),
    );

    let runtimes = request.split_brain_runtimes("cmd_invoke");
    assert_eq!(runtimes.len(), 2);
    assert!(runtimes.contains("runtime-a"));
    assert!(runtimes.contains("runtime-b"));
}

#[test]
fn split_brain_detection_uses_complete_group_view_not_action_subset() {
    let mut request = RuntimeCloudHaRequest {
        profile: RuntimeCloudHaProfile::DualHost,
        group_id: "line-c-subset-ha".to_string(),
        active_namespace: true,
        command_seq: Some(13),
        targets: BTreeMap::new(),
    };
    request.targets.insert(
        "runtime-a".to_string(),
        dual_host_active_target("runtime-a"),
    );
    request.targets.insert(
        "runtime-b".to_string(),
        dual_host_active_target("runtime-b"),
    );

    let runtimes = request.split_brain_runtimes("cmd_invoke");

    assert_eq!(runtimes.len(), 2);
    assert!(runtimes.contains("runtime-a"));
    assert!(runtimes.contains("runtime-b"));
}

#[test]
fn replay_guard_deduplicates_and_rejects_stale_sequences() {
    let mut coordinator = RuntimeCloudHaCoordinator::default();
    let mut request = RuntimeCloudHaRequest {
        profile: RuntimeCloudHaProfile::DualHost,
        group_id: "line-d-ha".to_string(),
        active_namespace: false,
        command_seq: Some(21),
        targets: BTreeMap::new(),
    };
    request.targets.insert(
        "runtime-a".to_string(),
        dual_host_active_target("runtime-a"),
    );

    let gate = coordinator
        .begin_dispatch("cmd_invoke", "req-21", "runtime-a", &request)
        .expect("dual-host command should return dispatch gate");
    let RuntimeCloudHaDispatchGate::Proceed(ticket) = gate else {
        panic!("first command should proceed");
    };
    coordinator.finish_dispatch(
        ticket,
        RuntimeCloudHaDispatchRecord {
            ok: true,
            denial_code: None,
            denial_reason: None,
            audit_id: Some("audit-21".to_string()),
            response: Some(serde_json::json!({ "ok": true })),
        },
    );
    assert_eq!(
        coordinator.last_applied_command_seq("line-d-ha", "runtime-a"),
        21
    );

    let dedupe_gate = coordinator
        .begin_dispatch("cmd_invoke", "req-21", "runtime-a", &request)
        .expect("duplicate request should return dispatch gate");
    let RuntimeCloudHaDispatchGate::Deduplicated(record) = dedupe_gate else {
        panic!("duplicate request_id must deduplicate");
    };
    assert!(record.ok);
    assert_eq!(record.audit_id.as_deref(), Some("audit-21"));

    request.command_seq = Some(20);
    let stale_gate = coordinator
        .begin_dispatch("cmd_invoke", "req-20", "runtime-a", &request)
        .expect("stale command should return dispatch gate");
    let RuntimeCloudHaDispatchGate::Denied(decision) = stale_gate else {
        panic!("stale command_seq should be denied");
    };
    assert_eq!(decision.denial_code, Some(ReasonCode::Conflict));
}

#[test]
fn crash_mid_command_can_retry_after_state_roundtrip() {
    let mut coordinator = RuntimeCloudHaCoordinator::default();
    let mut request = RuntimeCloudHaRequest {
        profile: RuntimeCloudHaProfile::DualHost,
        group_id: "line-e-ha".to_string(),
        active_namespace: false,
        command_seq: Some(31),
        targets: BTreeMap::new(),
    };
    request.targets.insert(
        "runtime-a".to_string(),
        dual_host_active_target("runtime-a"),
    );

    let gate = coordinator
        .begin_dispatch("cmd_invoke", "req-crash", "runtime-a", &request)
        .expect("first dispatch gate");
    let RuntimeCloudHaDispatchGate::Proceed(_) = gate else {
        panic!("first dispatch should proceed");
    };

    let encoded = serde_json::to_string(&coordinator).expect("serialize coordinator");
    let mut recovered: RuntimeCloudHaCoordinator =
        serde_json::from_str(&encoded).expect("deserialize coordinator");

    let retry_gate = recovered
        .begin_dispatch("cmd_invoke", "req-crash", "runtime-a", &request)
        .expect("retry dispatch gate");
    let RuntimeCloudHaDispatchGate::Proceed(ticket) = retry_gate else {
        panic!("pending request should retry for reconciliation");
    };
    recovered.finish_dispatch(
        ticket,
        RuntimeCloudHaDispatchRecord {
            ok: true,
            denial_code: None,
            denial_reason: None,
            audit_id: Some("audit-crash".to_string()),
            response: Some(serde_json::json!({ "ok": true })),
        },
    );
    assert_eq!(
        recovered.last_applied_command_seq("line-e-ha", "runtime-a"),
        31
    );
}

#[test]
fn live_in_flight_duplicate_is_denied_but_recovery_can_resume_once() {
    let mut coordinator = RuntimeCloudHaCoordinator::default();
    let mut request = RuntimeCloudHaRequest {
        profile: RuntimeCloudHaProfile::DualHost,
        group_id: "line-f-ha".to_string(),
        active_namespace: false,
        command_seq: Some(41),
        targets: BTreeMap::new(),
    };
    request.targets.insert(
        "runtime-a".to_string(),
        dual_host_active_target("runtime-a"),
    );

    let first = coordinator
        .begin_dispatch("cmd_invoke", "req-in-flight", "runtime-a", &request)
        .expect("first dispatch gate");
    assert!(matches!(first, RuntimeCloudHaDispatchGate::Proceed(_)));

    let duplicate = coordinator
        .begin_dispatch("cmd_invoke", "req-in-flight", "runtime-a", &request)
        .expect("duplicate dispatch gate");
    let RuntimeCloudHaDispatchGate::Denied(denial) = duplicate else {
        panic!("a live in-flight request must not start a second dispatch");
    };
    assert_eq!(denial.denial_code, Some(ReasonCode::Conflict));

    let encoded = serde_json::to_string(&coordinator).expect("serialize coordinator");
    let mut recovered: RuntimeCloudHaCoordinator =
        serde_json::from_str(&encoded).expect("deserialize coordinator");
    let resumed = recovered
        .begin_dispatch("cmd_invoke", "req-in-flight", "runtime-a", &request)
        .expect("recovery dispatch gate");
    assert!(matches!(resumed, RuntimeCloudHaDispatchGate::Proceed(_)));
}
