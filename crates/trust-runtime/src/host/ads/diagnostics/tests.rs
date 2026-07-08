use std::path::PathBuf;

use serde_json::json;
use trust_ads_core::{
    AdsDataTypeDescriptor, IecDataType, SymbolDescriptor, SymbolFlag, SymbolSnapshot,
};

use super::*;

#[test]
fn serializes_documented_missing_route_shape_deterministically() {
    let report = missing_route_report();
    let json = serde_json::to_string_pretty(&report).expect("serialize report");

    assert_eq!(
        json,
        r#"{
  "schema_version": 2,
  "role": "client",
  "ran_from": "runtime-host",
  "target": {
    "name": "CX-1234",
    "ip": "192.168.10.5",
    "ams_net_id": "5.23.91.12.1.1",
    "ams_port": 851,
    "tc_version": "3.1.4024"
  },
  "local": {
    "host_name": "line-controller-1",
    "chosen_ip": "192.168.10.20",
    "ams_net_id": "192.168.10.20.1.1",
    "nic": "eth0",
    "candidates": [],
    "classification": "lan"
  },
  "transport": "plain",
  "writes_enabled": false,
  "steps": [
    {
      "id": "route_present",
      "title": "Route back to truST",
      "status": "fail",
      "skip_reason": null,
      "detail": "The PLC does not have a route back to 192.168.10.20.1.1.",
      "evidence": {
        "local_ams_net_id": "192.168.10.20.1.1",
        "local_ip": "192.168.10.20",
        "target_ip": "192.168.10.5"
      },
      "remediation": "Add a static ADS route on the PLC for this truST runtime host.",
      "next_action": {
        "kind": "add_route"
      },
      "blocks_production_ready": true
    }
  ],
  "overall": "fail",
  "production_ready": false,
  "summary": "1 problem: PLC has no route back to truST."
}"#
    );
}

#[test]
fn committed_golden_fixtures_match_rust_schema() {
    for (name, report) in fixture_reports() {
        let path = fixture_path(name);
        let expected = std::fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("read fixture {}: {error}", path.display()));
        let actual = format!(
            "{}\n",
            serde_json::to_string_pretty(&report).expect("serialize fixture")
        );

        assert_eq!(actual, expected, "fixture drifted: {}", path.display());
    }
}

#[test]
fn schema_reserves_server_role_without_server_modules() {
    let value = serde_json::to_value(DoctorRole::Server).expect("serialize role");
    assert_eq!(value, json!("server"));
}

#[test]
fn skip_reasons_match_contract_names() {
    let reasons = [
        DoctorSkipReason::BlockedByPreviousStep,
        DoctorSkipReason::ActiveDevice,
        DoctorSkipReason::WritesDisabled,
        DoctorSkipReason::NotSupportedByTarget,
        DoctorSkipReason::NotRequested,
        DoctorSkipReason::Cancelled,
        DoctorSkipReason::ServerDisabled,
        DoctorSkipReason::NoSymbolsExposed,
        DoctorSkipReason::NoClientsAllowed,
        DoctorSkipReason::ExternalClientPending,
    ];
    let names: Vec<String> = reasons
        .into_iter()
        .map(|reason| {
            serde_json::to_value(reason)
                .expect("serialize")
                .as_str()
                .unwrap()
                .to_string()
        })
        .collect();

    assert_eq!(
        names,
        vec![
            "blocked_by_previous_step",
            "active_device",
            "writes_disabled",
            "not_supported_by_target",
            "not_requested",
            "cancelled",
            "server_disabled",
            "no_symbols_exposed",
            "no_clients_allowed",
            "external_client_pending",
        ]
    );
}

#[test]
fn next_action_kinds_match_contract_names() {
    let actions = [
        NextActionKind::None,
        NextActionKind::PickTarget,
        NextActionKind::FixLocalIp,
        NextActionKind::AddRoute,
        NextActionKind::OpenSetup,
        NextActionKind::DownloadPowershell,
        NextActionKind::CopyXml,
        NextActionKind::OpenRuntimePane,
        NextActionKind::EnableWrite,
        NextActionKind::UseSecure,
        NextActionKind::Deploy,
        NextActionKind::RerunDoctor,
        NextActionKind::ConfigureExpose,
        NextActionKind::AddAllowedClient,
        NextActionKind::OpenFirewall,
        NextActionKind::WaitForClient,
    ];
    let names: Vec<String> = actions
        .into_iter()
        .map(|action| {
            serde_json::to_value(action)
                .expect("serialize")
                .as_str()
                .unwrap()
                .to_string()
        })
        .collect();

    assert_eq!(
        names,
        vec![
            "none",
            "pick_target",
            "fix_local_ip",
            "add_route",
            "open_setup",
            "download_powershell",
            "copy_xml",
            "open_runtime_pane",
            "enable_write",
            "use_secure",
            "deploy",
            "rerun_doctor",
            "configure_expose",
            "add_allowed_client",
            "open_firewall",
            "wait_for_client",
        ]
    );
}

#[test]
fn server_step_ids_match_contract_names() {
    let steps = [
        DoctorStepId::LocalIdentity,
        DoctorStepId::BindExposure,
        DoctorStepId::ListenerBound,
        DoctorStepId::UdpIdentifyAnswer,
        DoctorStepId::SymbolsExposed,
        DoctorStepId::ClientsAllowed,
        DoctorStepId::SymbolServe,
        DoctorStepId::SelfReadState,
        DoctorStepId::SelfHandleResolve,
        DoctorStepId::SelfSumupRead,
        DoctorStepId::SelfNotification,
        DoctorStepId::SelfWriteGuarded,
        DoctorStepId::ParserLimits,
        DoctorStepId::AllowlistEnforced,
        DoctorStepId::ExternalClientVerified,
    ];
    let names: Vec<String> = steps
        .into_iter()
        .map(|step| {
            serde_json::to_value(step)
                .expect("serialize")
                .as_str()
                .unwrap()
                .to_string()
        })
        .collect();

    assert_eq!(
        names,
        vec![
            "local_identity",
            "bind_exposure",
            "listener_bound",
            "udp_identify_answer",
            "symbols_exposed",
            "clients_allowed",
            "symbol_serve",
            "self_read_state",
            "self_handle_resolve",
            "self_sumup_read",
            "self_notification",
            "self_write_guarded",
            "parser_limits",
            "allowlist_enforced",
            "external_client_verified",
        ]
    );
}

#[test]
fn error_classifier_maps_common_failures_to_remediation_and_actions() {
    let route = classify_onboarding_failure(OnboardingFailureKind::RouteMissing);
    assert_eq!(route.next_action.kind, NextActionKind::AddRoute);
    assert!(route.remediation.contains("static ADS route"));
    assert!(route.blocks_production_ready);

    let secure = classify_onboarding_failure(OnboardingFailureKind::SecureRequired);
    assert_eq!(secure.next_action.kind, NextActionKind::UseSecure);
    assert!(secure.explanation.contains("Secure ADS"));

    let fingerprint = classify_ads_error_code(1861).expect("known ADS code");
    assert_eq!(fingerprint.kind, OnboardingFailureKind::Fingerprint1861);
    assert_eq!(
        fingerprint.ads_error,
        Some(AdsErrorInfo::new(1861, "ADSERR_DEVICE_INVALIDCONTEXT"))
    );
}

#[test]
fn credential_channel_classification_enforces_secret_boundary() {
    assert!(CredentialChannelClassification::TrustedSameHost.permits_credentials());
    assert!(CredentialChannelClassification::TrustedHttpsAdmin.permits_credentials());
    assert!(CredentialChannelClassification::LocalCliDirectAddRoute.permits_credentials());
    assert!(!CredentialChannelClassification::UntrustedRemotePlainTcp.permits_credentials());
    assert!(!CredentialChannelClassification::UntrustedPlainHttpNetwork.permits_credentials());
}

#[test]
fn reports_contain_no_secret_fields_by_construction() {
    let route = RoutePlan {
        route_name: "trust-runtime-line-controller-1".to_string(),
        target: target_identity(),
        local: local_identity(),
        channel: CredentialChannelClassification::UntrustedRemotePlainTcp,
        automatic_route: RouteActionAvailability::DisabledUntrustedChannel,
        artifacts: vec![RouteArtifact {
            kind: RouteArtifactKind::ManualSteps,
            label: "Manual route steps".to_string(),
            filename: None,
            content_type: "text/plain".to_string(),
            content: "Add route for 192.168.10.20.1.1".to_string(),
        }],
    };
    let report_json = serde_json::to_string(&missing_route_report()).expect("report JSON");
    let route_json = serde_json::to_string(&route).expect("route JSON");
    let fixture_json = fixture_reports()
        .into_iter()
        .map(|(_, report)| serde_json::to_string(&report).expect("fixture JSON"))
        .collect::<Vec<_>>()
        .join("\n");
    let combined = format!("{report_json}\n{route_json}\n{fixture_json}").to_ascii_lowercase();

    for forbidden in ["password", "secret", "username", "token"] {
        assert!(
            !combined.contains(forbidden),
            "schema leaked forbidden field marker {forbidden}"
        );
    }

    for value in [
        serde_json::to_value(missing_route_report()).expect("report value"),
        serde_json::to_value(route).expect("route value"),
    ] {
        assert_no_forbidden_keys(&value);
    }

    for (_, report) in fixture_reports() {
        let value = serde_json::to_value(report).expect("fixture value");
        assert_no_forbidden_keys(&value);
    }
}

#[test]
fn evidence_hash_inputs_are_stable_and_complete() {
    let fields: Vec<&str> = EVIDENCE_HASH_INPUTS
        .iter()
        .map(|input| input.field)
        .collect();

    assert_eq!(
        fields,
        vec![
            "runtime_identity_hash",
            "target_identity_hash",
            "allowed_clients_hash",
            "ads_config_hash",
            "symbol_snapshot_hash",
            "generated_st_hash",
            "deployed_ads_config_hash",
            "runtime_ads_status_hash",
        ]
    );
    assert!(EVIDENCE_HASH_INPUTS
        .iter()
        .all(|input| !input.input.is_empty()));
}

#[test]
fn production_evidence_builder_hashes_declared_inputs() {
    let status = AdsStatusReport {
        schema_version: ADS_DIAGNOSTICS_SCHEMA_VERSION,
        role: DoctorRole::Client,
        overall: AdsStatusOverall::Healthy,
        runtime_identity_hash: None,
        deployed_ads_config_hash: Some(sha256_evidence_hash(b"ads toml")),
        connections: vec![AdsConnectionStatus {
            name: "line1".to_string(),
            target: Some(target_identity()),
            state: AdsConnectionStatusState::Connected,
            point_count: 1,
            degraded_points: 0,
            last_good_value_ms: Some(1781234567000),
            symbol_version: Some(7),
            summary: "Connected.".to_string(),
        }],
        summary: "ADS connections healthy.".to_string(),
    };
    let snapshots = vec![SymbolSnapshot::new(
        "line1",
        vec![SymbolDescriptor::new(
            "MAIN.Temperature",
            AdsDataTypeDescriptor::scalar("REAL", IecDataType::Real),
            0x4020,
            0,
            4,
        )
        .with_flag(SymbolFlag::Read)],
    )];

    let evidence = build_production_evidence(ProductionEvidenceInput {
        doctor_timestamp_ms: 1781234567000,
        runtime_identity: &local_identity(),
        target_identity: &target_identity(),
        ads_toml: "ads toml",
        symbol_snapshots: &snapshots,
        generated_st: Some("generated st"),
        deployed_ads_toml: Some("ads toml"),
        runtime_ads_status: Some(&status),
        stale_after_ms: 86_400_000,
        expires_at_ms: Some(1781320967000),
        runtime_clock_warning: None,
    })
    .expect("evidence");

    assert_eq!(
        evidence.doctor_schema_version,
        ADS_DIAGNOSTICS_SCHEMA_VERSION
    );
    assert_eq!(evidence.ads_config_hash, sha256_evidence_hash(b"ads toml"));
    assert_eq!(
        evidence.deployed_ads_config_hash.as_deref(),
        Some(evidence.ads_config_hash.as_str())
    );
    assert_eq!(
        evidence.generated_st_hash.as_deref(),
        Some(sha256_evidence_hash(b"generated st").as_str())
    );
    assert!(evidence.runtime_identity_hash.starts_with("sha256:"));
    assert!(evidence
        .target_identity_hash
        .as_deref()
        .expect("target hash")
        .starts_with("sha256:"));
    assert!(evidence.allowed_clients_hash.is_none());
    assert!(evidence.symbol_snapshot_hash.starts_with("sha256:"));
    assert!(evidence.runtime_ads_status_hash.is_some());
    assert_eq!(evidence.freshness.expires_at_ms, Some(1781320967000));
}

#[test]
fn production_readiness_requires_matching_deployed_status() {
    let status = healthy_ads_status();
    let evidence = production_evidence_for_status(&status, Some(1781320967000));

    let report = evaluate_production_readiness(Some(&evidence), Some(&status), 1781234567000)
        .expect("readiness");

    assert_eq!(report.state, ProductionReadinessState::Ready);
    assert!(report.reasons.is_empty());
}

#[test]
fn production_readiness_needs_recheck_on_mismatch_fault_or_expiry() {
    let status = healthy_ads_status();
    let mut evidence = production_evidence_for_status(&status, Some(1781234567001));
    let mut mismatched_status = status.clone();
    mismatched_status.deployed_ads_config_hash = Some("sha256:different".to_string());

    let mismatch =
        evaluate_production_readiness(Some(&evidence), Some(&mismatched_status), 1781234567000)
            .expect("mismatch readiness");
    assert_eq!(mismatch.state, ProductionReadinessState::NeedsRecheck);
    assert!(mismatch
        .reasons
        .contains(&ProductionReadinessReason::DeployedAdsConfigMismatch));
    assert!(mismatch
        .reasons
        .contains(&ProductionReadinessReason::RuntimeAdsStatusChanged));

    let mut faulted_status = status.clone();
    faulted_status.overall = AdsStatusOverall::Faulted;
    faulted_status.connections[0].state = AdsConnectionStatusState::Faulted;
    let faulted =
        evaluate_production_readiness(Some(&evidence), Some(&faulted_status), 1781234567000)
            .expect("faulted readiness");
    assert!(faulted
        .reasons
        .contains(&ProductionReadinessReason::RuntimeAdsFaulted));

    evidence.freshness.expires_at_ms = Some(1781234567000);
    let expired = evaluate_production_readiness(Some(&evidence), Some(&status), 1781234567001)
        .expect("expired readiness");
    assert!(expired
        .reasons
        .contains(&ProductionReadinessReason::EvidenceExpired));
}

#[test]
fn production_readiness_is_not_ready_without_evidence() {
    let report = evaluate_production_readiness(None, Some(&healthy_ads_status()), 0)
        .expect("missing evidence readiness");

    assert_eq!(report.state, ProductionReadinessState::NotReady);
    assert_eq!(
        report.reasons,
        vec![ProductionReadinessReason::MissingEvidence]
    );
}

#[test]
fn production_ready_requires_pass_and_evidence() {
    let mut report = DoctorReport::new(DoctorVantage::RuntimeHost, DiagnosticTransport::Plain)
        .with_steps(vec![DoctorStep::new(
            DoctorStepId::Tcp48898,
            "TwinCAT router reachable",
            DoctorStepStatus::Pass,
            "Router reachable.",
        )]);

    assert_eq!(report.overall, DoctorOverall::Pass);
    assert!(!report.production_ready);

    report = report.with_evidence(ProductionEvidence {
        doctor_timestamp_ms: 1781234567000,
        doctor_schema_version: ADS_DIAGNOSTICS_SCHEMA_VERSION,
        runtime_identity_hash: "sha256:runtime".to_string(),
        target_identity_hash: Some("sha256:target".to_string()),
        allowed_clients_hash: None,
        ads_config_hash: "sha256:ads".to_string(),
        symbol_snapshot_hash: "sha256:symbols".to_string(),
        generated_st_hash: Some("sha256:st".to_string()),
        deployed_ads_config_hash: Some("sha256:deployed".to_string()),
        runtime_ads_status_hash: Some("sha256:status".to_string()),
        external_client_verified: false,
        external_client_kind: None,
        external_client_name: None,
        external_client_timestamp_ms: None,
        discoverable: false,
        freshness: EvidenceFreshness {
            stale_after_ms: 86_400_000,
            expires_at_ms: None,
            runtime_clock_warning: None,
        },
    });

    assert!(report.production_ready);
}

#[test]
fn server_production_ready_requires_independent_client_evidence() {
    let base_step = DoctorStep::new(
        DoctorStepId::SelfSumupRead,
        "Loopback read",
        DoctorStepStatus::Pass,
        "Loopback self-client read succeeded.",
    );
    let snapshot = SymbolSnapshot::new(
        "server",
        vec![SymbolDescriptor::new(
            "global.setpoint",
            AdsDataTypeDescriptor::scalar("REAL", IecDataType::Real),
            0x4020,
            0,
            4,
        )
        .with_flag(SymbolFlag::Read)],
    );
    let allowed_clients = json!([
        {
            "ams_net_id": "5.23.91.12.1.1",
            "source_cidr": "192.168.10.0/24"
        }
    ]);
    let self_test_only = build_server_production_evidence(ServerProductionEvidenceInput {
        doctor_timestamp_ms: 1781234567000,
        runtime_identity: &local_identity(),
        allowed_clients: &allowed_clients,
        ads_server_config: "ads server config",
        symbol_snapshot: &snapshot,
        deployed_ads_server_config: Some("ads server config"),
        runtime_ads_status: Some(&healthy_ads_server_status()),
        external_client_verified: false,
        external_client_kind: None,
        external_client_name: None,
        external_client_timestamp_ms: None,
        discoverable: true,
        stale_after_ms: 86_400_000,
        expires_at_ms: None,
        runtime_clock_warning: None,
    })
    .expect("self-test evidence");
    let mut report = DoctorReport::for_role(
        DoctorRole::Server,
        DoctorVantage::RuntimeHost,
        DiagnosticTransport::Plain,
    )
    .with_steps(vec![base_step.clone()])
    .with_evidence(self_test_only);

    assert_eq!(report.overall, DoctorOverall::Pass);
    assert!(!report.production_ready);
    let evidence = report.evidence.as_ref().expect("evidence");
    assert!(evidence.allowed_clients_hash.is_some());
    assert!(evidence.target_identity_hash.is_none());
    assert!(evidence.discoverable);

    let pyads_external = build_server_production_evidence(ServerProductionEvidenceInput {
        doctor_timestamp_ms: 1781234567000,
        runtime_identity: &local_identity(),
        allowed_clients: &allowed_clients,
        ads_server_config: "ads server config",
        symbol_snapshot: &snapshot,
        deployed_ads_server_config: Some("ads server config"),
        runtime_ads_status: Some(&healthy_ads_server_status()),
        external_client_verified: true,
        external_client_kind: Some("pyads"),
        external_client_name: Some("ci-pyads"),
        external_client_timestamp_ms: Some(1781234567999),
        discoverable: true,
        stale_after_ms: 86_400_000,
        expires_at_ms: None,
        runtime_clock_warning: None,
    })
    .expect("pyads external evidence");
    report = DoctorReport::for_role(
        DoctorRole::Server,
        DoctorVantage::RuntimeHost,
        DiagnosticTransport::Plain,
    )
    .with_steps(vec![base_step])
    .with_evidence(pyads_external);

    assert!(!report.production_ready);
    let evidence = report.evidence.as_ref().expect("evidence");
    assert!(evidence.external_client_verified);
    assert_eq!(evidence.external_client_kind.as_deref(), Some("pyads"));

    let twincat_external = build_server_production_evidence(ServerProductionEvidenceInput {
        doctor_timestamp_ms: 1781234567000,
        runtime_identity: &local_identity(),
        allowed_clients: &allowed_clients,
        ads_server_config: "ads server config",
        symbol_snapshot: &snapshot,
        deployed_ads_server_config: Some("ads server config"),
        runtime_ads_status: Some(&healthy_ads_server_status()),
        external_client_verified: true,
        external_client_kind: Some("twincat"),
        external_client_name: Some("engineering-station"),
        external_client_timestamp_ms: Some(1781234567999),
        discoverable: true,
        stale_after_ms: 86_400_000,
        expires_at_ms: None,
        runtime_clock_warning: None,
    })
    .expect("TwinCAT external evidence");
    report = DoctorReport::for_role(
        DoctorRole::Server,
        DoctorVantage::RuntimeHost,
        DiagnosticTransport::Plain,
    )
    .with_steps(vec![DoctorStep::new(
        DoctorStepId::SelfSumupRead,
        "Loopback read",
        DoctorStepStatus::Pass,
        "Loopback self-client read succeeded.",
    )])
    .with_evidence(twincat_external);

    assert!(report.production_ready);
    let evidence = report.evidence.as_ref().expect("evidence");
    assert!(evidence.external_client_verified);
    assert_eq!(evidence.external_client_kind.as_deref(), Some("twincat"));
}

#[test]
fn v1_client_evidence_json_still_deserializes_with_server_defaults() {
    let json = r#"{
          "doctor_timestamp_ms": 1781234567000,
          "doctor_schema_version": 2,
          "runtime_identity_hash": "sha256:runtime",
          "target_identity_hash": "sha256:target",
          "ads_config_hash": "sha256:ads",
          "symbol_snapshot_hash": "sha256:symbols",
          "freshness": { "stale_after_ms": 300000 }
        }"#;

    let evidence: ProductionEvidence = serde_json::from_str(json).expect("v1 evidence");

    assert_eq!(
        evidence.target_identity_hash.as_deref(),
        Some("sha256:target")
    );
    assert!(evidence.allowed_clients_hash.is_none());
    assert!(!evidence.external_client_verified);
    assert!(!evidence.discoverable);
}

fn fixture_reports() -> Vec<(&'static str, DoctorReport)> {
    vec![
        ("pass", pass_report()),
        ("missing_route", missing_route_report()),
        ("untrusted_channel", untrusted_channel_report()),
        ("active_device", active_device_report()),
        ("authoring_only", authoring_only_report()),
        ("secure_required", secure_required_report()),
    ]
}

fn fixture_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/ads_onboarding")
        .join(format!("{name}.json"))
}

fn pass_report() -> DoctorReport {
    DoctorReport::new(DoctorVantage::RuntimeHost, DiagnosticTransport::Plain)
        .with_target(target_identity())
        .with_local(local_identity())
        .with_steps(vec![
            pass_step(
                DoctorStepId::UdpIdentify,
                "Find PLC on network",
                "TwinCAT target answered directed ADS identify.",
            ),
            pass_step(
                DoctorStepId::RoutePresent,
                "Route back to truST",
                "PLC accepts the route for 192.168.10.20.1.1.",
            ),
            pass_step(
                DoctorStepId::SymbolUpload,
                "Read TwinCAT symbols",
                "Symbol table upload returned compatible symbols.",
            ),
            pass_step(
                DoctorStepId::SumupRead,
                "Read selected values",
                "SUMUP read returned values for selected symbols.",
            ),
        ])
        .with_evidence(production_evidence())
        .with_summary("ADS connection is production ready from runtime host line-controller-1.")
}

fn untrusted_channel_report() -> DoctorReport {
    let step = DoctorStep::new(
            DoctorStepId::RoutePresent,
            "Route back to truST",
            DoctorStepStatus::Fail,
            "TwinCAT credentials cannot be forwarded over the selected remote plain TCP control endpoint.",
        )
        .with_remediation(
            "Run route setup from this computer directly to the PLC, open trusted setup web, or use the generated artifacts.",
        )
        .with_next_action(NextAction::new(NextActionKind::DownloadPowershell));

    DoctorReport::new(DoctorVantage::VscodeCli, DiagnosticTransport::Plain)
        .with_target(target_identity())
        .with_local(local_identity())
        .with_steps(vec![step])
        .with_summary(
            "Route-add needs a trusted setup channel; automatic credential forwarding is disabled.",
        )
}

fn active_device_report() -> DoctorReport {
    let step = DoctorStep::skipped(
            DoctorStepId::Notification,
            "Notification probe",
            DoctorSkipReason::ActiveDevice,
            "The ADS connection is already active; full notification probing requires an explicit pause.",
        )
        .with_remediation("Use live ADS status or pause the device before running the full doctor.")
        .with_next_action(NextAction::new(NextActionKind::RerunDoctor));

    DoctorReport::new(DoctorVantage::RuntimeHost, DiagnosticTransport::Plain)
            .with_target(target_identity())
            .with_local(local_identity())
            .with_steps(vec![step])
            .with_summary("Active ADS device detected; full doctor was not allowed to open a duplicate AMS connection.")
}

fn authoring_only_report() -> DoctorReport {
    let step = DoctorStep::skipped(
        DoctorStepId::RoutePresent,
        "Route back to truST",
        DoctorSkipReason::NotRequested,
        "Authoring-only import does not run the runtime-host route check.",
    )
    .with_remediation("Run the ADS Doctor from the selected runtime host before deployment.")
    .with_next_action(NextAction::new(NextActionKind::OpenRuntimePane));

    DoctorReport::new(
        DoctorVantage::VscodeAuthoringOnly,
        DiagnosticTransport::Plain,
    )
    .with_target(target_identity())
    .with_steps(vec![step])
    .with_summary("Symbols were imported for authoring only; production readiness is not proven.")
}

fn secure_required_report() -> DoctorReport {
    let step = DoctorStep::failed(
        DoctorStepId::AmsTarget,
        "ADS target policy",
        "The target requires Secure ADS.",
        classify_onboarding_failure(OnboardingFailureKind::SecureRequired),
    );

    DoctorReport::new(DoctorVantage::RuntimeHost, DiagnosticTransport::Plain)
        .with_target(target_identity())
        .with_local(local_identity())
        .with_steps(vec![step])
        .with_summary("Target requires Secure ADS, which this client does not support yet.")
}

fn missing_route_report() -> DoctorReport {
    let step = DoctorStep::failed(
        DoctorStepId::RoutePresent,
        "Route back to truST",
        "The PLC does not have a route back to 192.168.10.20.1.1.",
        classify_onboarding_failure(OnboardingFailureKind::RouteMissing),
    )
    .with_evidence("target_ip", json!("192.168.10.5"))
    .with_evidence("local_ip", json!("192.168.10.20"))
    .with_evidence("local_ams_net_id", json!("192.168.10.20.1.1"));

    DoctorReport::new(DoctorVantage::RuntimeHost, DiagnosticTransport::Plain)
        .with_target(target_identity())
        .with_local(local_identity())
        .with_steps(vec![step])
        .with_summary("1 problem: PLC has no route back to truST.")
}

fn pass_step(id: DoctorStepId, title: &str, detail: &str) -> DoctorStep {
    DoctorStep::new(id, title, DoctorStepStatus::Pass, detail)
}

fn production_evidence() -> ProductionEvidence {
    ProductionEvidence {
        doctor_timestamp_ms: 1781234567000,
        doctor_schema_version: ADS_DIAGNOSTICS_SCHEMA_VERSION,
        runtime_identity_hash: "sha256:runtime".to_string(),
        target_identity_hash: Some("sha256:target".to_string()),
        allowed_clients_hash: None,
        ads_config_hash: "sha256:ads".to_string(),
        symbol_snapshot_hash: "sha256:symbols".to_string(),
        generated_st_hash: Some("sha256:generated-st".to_string()),
        deployed_ads_config_hash: Some("sha256:deployed-ads".to_string()),
        runtime_ads_status_hash: Some("sha256:ads-status".to_string()),
        external_client_verified: false,
        external_client_kind: None,
        external_client_name: None,
        external_client_timestamp_ms: None,
        discoverable: false,
        freshness: EvidenceFreshness {
            stale_after_ms: 86_400_000,
            expires_at_ms: Some(1781320967000),
            runtime_clock_warning: None,
        },
    }
}

fn healthy_ads_status() -> AdsStatusReport {
    AdsStatusReport {
        schema_version: ADS_DIAGNOSTICS_SCHEMA_VERSION,
        role: DoctorRole::Client,
        overall: AdsStatusOverall::Healthy,
        runtime_identity_hash: None,
        deployed_ads_config_hash: Some(sha256_evidence_hash(b"ads toml")),
        connections: vec![AdsConnectionStatus {
            name: "line1".to_string(),
            target: Some(target_identity()),
            state: AdsConnectionStatusState::Connected,
            point_count: 1,
            degraded_points: 0,
            last_good_value_ms: Some(1781234567000),
            symbol_version: Some(7),
            summary: "Connected.".to_string(),
        }],
        summary: "ADS connections healthy.".to_string(),
    }
}

fn healthy_ads_server_status() -> AdsStatusReport {
    AdsStatusReport {
        schema_version: ADS_DIAGNOSTICS_SCHEMA_VERSION,
        role: DoctorRole::Server,
        overall: AdsStatusOverall::Healthy,
        runtime_identity_hash: Some("sha256:runtime".to_string()),
        deployed_ads_config_hash: Some(sha256_evidence_hash(b"ads server config")),
        connections: Vec::new(),
        summary: "ADS server healthy.".to_string(),
    }
}

fn production_evidence_for_status(
    status: &AdsStatusReport,
    expires_at_ms: Option<u64>,
) -> ProductionEvidence {
    let snapshots = vec![SymbolSnapshot::new(
        "line1",
        vec![SymbolDescriptor::new(
            "MAIN.Temperature",
            AdsDataTypeDescriptor::scalar("REAL", IecDataType::Real),
            0x4020,
            0,
            4,
        )
        .with_flag(SymbolFlag::Read)],
    )];
    build_production_evidence(ProductionEvidenceInput {
        doctor_timestamp_ms: 1781234567000,
        runtime_identity: &local_identity(),
        target_identity: &target_identity(),
        ads_toml: "ads toml",
        symbol_snapshots: &snapshots,
        generated_st: Some("generated st"),
        deployed_ads_toml: Some("ads toml"),
        runtime_ads_status: Some(status),
        stale_after_ms: 86_400_000,
        expires_at_ms,
        runtime_clock_warning: None,
    })
    .expect("production evidence")
}

fn target_identity() -> TargetIdentity {
    TargetIdentity {
        name: Some("CX-1234".to_string()),
        ip: "192.168.10.5".to_string(),
        ams_net_id: "5.23.91.12.1.1".to_string(),
        ams_port: 851,
        tc_version: Some("3.1.4024".to_string()),
    }
}

fn local_identity() -> LocalIdentity {
    LocalIdentity {
        host_name: Some("line-controller-1".to_string()),
        chosen_ip: "192.168.10.20".to_string(),
        ams_net_id: "192.168.10.20.1.1".to_string(),
        nic: Some("eth0".to_string()),
        candidates: Vec::new(),
        classification: LocalNetworkClassification::Lan,
    }
}

fn assert_no_forbidden_keys(value: &serde_json::Value) {
    match value {
        serde_json::Value::Object(map) => {
            for (key, nested) in map {
                let normalized = key.to_ascii_lowercase();
                for forbidden in ["password", "secret", "credential", "username", "token"] {
                    assert!(
                        !normalized.contains(forbidden),
                        "schema leaked forbidden key marker {forbidden}: {key}"
                    );
                }
                assert_no_forbidden_keys(nested);
            }
        }
        serde_json::Value::Array(items) => {
            for item in items {
                assert_no_forbidden_keys(item);
            }
        }
        _ => {}
    }
}
