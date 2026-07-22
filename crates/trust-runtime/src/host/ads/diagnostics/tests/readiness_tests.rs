use super::*;

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
fn production_evidence_hashes_snapshot_sets_independent_of_connection_order() {
    let first = SymbolSnapshot::new(
        "line-a",
        vec![SymbolDescriptor::new(
            "MAIN.First",
            AdsDataTypeDescriptor::scalar("BOOL", IecDataType::Bool),
            0x4020,
            0,
            1,
        )],
    );
    let second = SymbolSnapshot::new(
        "line-b",
        vec![SymbolDescriptor::new(
            "MAIN.Second",
            AdsDataTypeDescriptor::scalar("REAL", IecDataType::Real),
            0x4020,
            4,
            4,
        )],
    );
    let status = healthy_ads_status();
    let build_hash = |snapshots: &[SymbolSnapshot]| {
        build_production_evidence(ProductionEvidenceInput {
            doctor_timestamp_ms: 1781234567000,
            runtime_identity: &local_identity(),
            target_identity: &target_identity(),
            ads_toml: "ads toml",
            symbol_snapshots: snapshots,
            generated_st: None,
            deployed_ads_toml: Some("ads toml"),
            runtime_ads_status: Some(&status),
            stale_after_ms: 86_400_000,
            expires_at_ms: None,
            runtime_clock_warning: None,
        })
        .expect("evidence")
        .symbol_snapshot_hash
    };

    assert_eq!(
        build_hash(&[first.clone(), second.clone()]),
        build_hash(&[second, first])
    );
}

#[test]
fn server_evidence_reports_allowed_client_serialization_failure() {
    struct FailingSerialize;

    impl serde::Serialize for FailingSerialize {
        fn serialize<S>(&self, _serializer: S) -> Result<S::Ok, S::Error>
        where
            S: serde::Serializer,
        {
            Err(serde::ser::Error::custom("deliberate evidence failure"))
        }
    }

    let snapshot = SymbolSnapshot::new("server", Vec::new());
    let error = build_server_production_evidence(ServerProductionEvidenceInput {
        doctor_timestamp_ms: 1781234567000,
        runtime_identity: &local_identity(),
        allowed_clients: &FailingSerialize,
        ads_server_config: "ads server config",
        symbol_snapshot: &snapshot,
        deployed_ads_server_config: Some("ads server config"),
        runtime_ads_status: Some(&healthy_ads_server_status()),
        external_client_verified: false,
        external_client_kind: None,
        external_client_name: None,
        external_client_timestamp_ms: None,
        discoverable: false,
        stale_after_ms: 86_400_000,
        expires_at_ms: None,
        runtime_clock_warning: None,
    })
    .expect_err("serialization must fail");

    assert!(matches!(error, ProductionEvidenceError::Serialize(_)));
    assert!(error.to_string().contains("deliberate evidence failure"));
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
fn production_readiness_honors_age_status_hash_and_clock_warning() {
    let status = healthy_ads_status();

    let mut stale = production_evidence_for_status(&status, None);
    stale.freshness.stale_after_ms = 1_000;
    let stale_report = evaluate_production_readiness(
        Some(&stale),
        Some(&status),
        stale.doctor_timestamp_ms + 1_001,
    )
    .expect("stale readiness");
    assert_eq!(stale_report.state, ProductionReadinessState::NeedsRecheck);
    assert!(stale_report
        .reasons
        .contains(&ProductionReadinessReason::EvidenceExpired));

    let mut missing_status_hash = production_evidence_for_status(&status, None);
    missing_status_hash.runtime_ads_status_hash = None;
    let missing_hash_report = evaluate_production_readiness(
        Some(&missing_status_hash),
        Some(&status),
        missing_status_hash.doctor_timestamp_ms,
    )
    .expect("missing status-hash readiness");
    assert_eq!(
        missing_hash_report.state,
        ProductionReadinessState::NeedsRecheck
    );
    assert!(missing_hash_report
        .reasons
        .contains(&ProductionReadinessReason::MissingRuntimeStatus));

    let mut clock_warning = production_evidence_for_status(&status, None);
    clock_warning.freshness.runtime_clock_warning =
        Some("runtime clock moved backwards".to_string());
    let clock_report = evaluate_production_readiness(
        Some(&clock_warning),
        Some(&status),
        clock_warning.doctor_timestamp_ms,
    )
    .expect("clock-warning readiness");
    assert_eq!(clock_report.state, ProductionReadinessState::NeedsRecheck);
    assert!(serde_json::to_value(&clock_report.reasons)
        .expect("reason JSON")
        .as_array()
        .expect("reason array")
        .contains(&json!("runtime_clock_warning")));

    let mut overflow = production_evidence_for_status(&status, None);
    overflow.doctor_timestamp_ms = u64::MAX - 1;
    overflow.freshness.stale_after_ms = 10;
    let overflow_report = evaluate_production_readiness(Some(&overflow), Some(&status), u64::MAX)
        .expect("overflow readiness");
    assert!(overflow_report
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
fn doctor_overall_fails_closed_for_empty_steps() {
    let report = DoctorReport::new(DoctorVantage::RuntimeHost, DiagnosticTransport::Plain)
        .with_steps(Vec::new());

    assert_eq!(report.overall, DoctorOverall::Fail);
    assert!(!report.production_ready);
}

#[test]
fn doctor_overall_keeps_nonblocking_failure_partial() {
    let mut failed = DoctorStep::new(
        DoctorStepId::Notification,
        "Optional notification probe",
        DoctorStepStatus::Fail,
        "Notifications are unavailable; polling remains usable.",
    );
    failed.blocks_production_ready = false;
    let report = DoctorReport::new(DoctorVantage::RuntimeHost, DiagnosticTransport::Plain)
        .with_steps(vec![failed]);

    assert_eq!(report.overall, DoctorOverall::Partial);
    assert!(!report.production_ready);
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
fn server_production_ready_rejects_incomplete_or_client_role_evidence() {
    let mut evidence = production_evidence();
    evidence.target_identity_hash = None;
    evidence.allowed_clients_hash = Some("sha256:allowed-clients".to_string());
    evidence.external_client_verified = true;
    evidence.external_client_kind = Some(" TwinCAT ".to_string());
    evidence.external_client_name = Some("engineering-station".to_string());
    evidence.external_client_timestamp_ms = Some(1781234567999);

    assert!(server_report_with_evidence(evidence.clone()).production_ready);

    let mut missing_name = evidence.clone();
    missing_name.external_client_name = None;
    assert!(!server_report_with_evidence(missing_name).production_ready);

    let mut blank_name = evidence.clone();
    blank_name.external_client_name = Some("   ".to_string());
    assert!(!server_report_with_evidence(blank_name).production_ready);

    let mut missing_timestamp = evidence.clone();
    missing_timestamp.external_client_timestamp_ms = None;
    assert!(!server_report_with_evidence(missing_timestamp).production_ready);

    let mut client_role_shape = evidence;
    client_role_shape.target_identity_hash = Some("sha256:target".to_string());
    client_role_shape.allowed_clients_hash = None;
    assert!(!server_report_with_evidence(client_role_shape).production_ready);
}

fn server_report_with_evidence(evidence: ProductionEvidence) -> DoctorReport {
    DoctorReport::for_role(
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
    .with_evidence(evidence)
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
