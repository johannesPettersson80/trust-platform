use std::path::Path;

use crate::ads::diagnostics::{
    CredentialChannelClassification, DoctorOverall, DoctorSkipReason, DoctorStepId,
    DoctorStepStatus, EvidenceFreshness, LocalIdentity, LocalNetworkClassification, NextActionKind,
    ProductionEvidence, RouteActionAvailability, RouteArtifactKind, TargetIdentity,
    ADS_DIAGNOSTICS_SCHEMA_VERSION,
};
use crate::ads::onboarding::wire::{
    AdsOnboardingWire, AdsReadUpdateSample, AdsRouteRequirement, AdsRouterProbe,
    MockAdsOnboardingScenario, MockAdsOnboardingWire,
};
use trust_ads_core::{
    AdsDataTypeDescriptor, IecDataType, PointQuality, PointStatus, QualityState, SymbolDescriptor,
};
use trust_runtime_core::value::Value;

use super::*;

mod doctor;
mod native_router;

#[test]
fn route_plan_uses_runtime_host_identity_and_generates_all_fallbacks() {
    let plan = build_route_plan(route_plan_request(
        CredentialChannelClassification::TrustedSameHost,
        local_identity(),
    ));

    assert_eq!(plan.route_name, "trust-runtime-line-controller-1");
    assert_eq!(plan.local.chosen_ip, "192.168.10.20");
    assert_eq!(plan.local.ams_net_id, "192.168.10.20.1.1");
    assert_eq!(plan.automatic_route, RouteActionAvailability::Available);

    let kinds = plan
        .artifacts
        .iter()
        .map(|artifact| artifact.kind)
        .collect::<Vec<_>>();
    assert_eq!(
        kinds,
        vec![
            RouteArtifactKind::Powershell,
            RouteArtifactKind::StaticRoutesXml,
            RouteArtifactKind::ManualSteps,
            RouteArtifactKind::RemovalPowershell,
        ]
    );

    let xml = artifact(&plan, RouteArtifactKind::StaticRoutesXml)
        .content
        .as_str();
    assert!(xml.contains("<Name>trust-runtime-line-controller-1</Name>"));
    assert!(xml.contains("<Address>192.168.10.20</Address>"));
    assert!(xml.contains("<NetId>192.168.10.20.1.1</NetId>"));
    assert!(xml.contains("<Flags>0</Flags>"));

    let powershell = artifact(&plan, RouteArtifactKind::Powershell)
        .content
        .as_str();
    assert!(powershell.contains("Administrator"));
    assert!(powershell.contains("C:\\TwinCAT\\3.1\\Target\\StaticRoutes.xml"));
    assert!(powershell.contains("$runtimeRoots = @("));
    assert!(powershell.contains("Join-Path $_ '3.1\\StaticRoutes.xml'"));
    assert!(powershell.contains("Runtimes"));
    assert!(powershell.contains("RemoteConnections"));
    assert!(powershell.contains("<Flags>0</Flags>"));
    assert!(powershell.contains("ChildNodes | Where-Object"));
    assert!(!powershell.contains("SelectNodes(\"Route[Name='$RouteName']\")"));
    assert!(powershell.contains("Unrelated ADS routes were preserved"));
    assert!(powershell.contains("StaticRoutes.xml encoding/BOM"));
    assert!(powershell.contains("Restart the TwinCAT router or Usermode Runtime"));

    let manual = artifact(&plan, RouteArtifactKind::ManualSteps)
        .content
        .as_str();
    assert!(manual.contains("Do not use Broadcast Search"));
    assert!(manual.contains("ADS Error 1861"));
    assert!(manual.contains("AMS Net ID: 192.168.10.20.1.1"));

    for generated in &plan.artifacts {
        assert!(!generated.content.contains("not-persisted"));
        assert!(!generated.content.contains("password"));
    }
}

#[test]
fn route_artifacts_preserve_unrelated_routes_and_report_encoding_changes() {
    let plan = build_route_plan(route_plan_request(
        CredentialChannelClassification::TrustedSameHost,
        local_identity(),
    ));
    let powershell = artifact(&plan, RouteArtifactKind::Powershell)
        .content
        .as_str();
    let removal = artifact(&plan, RouteArtifactKind::RemovalPowershell)
        .content
        .as_str();

    for script in [powershell, removal] {
        assert!(script.contains("Copy-Item -Path $routeFile -Destination $backup -Force"));
        assert!(script.contains("ChildNodes | Where-Object"));
        assert!(script.contains(".InnerText -eq $RouteName"));
        assert!(script.contains("Unrelated ADS routes were preserved"));
        assert!(script.contains("StaticRoutes.xml encoding/BOM"));
        assert!(script.contains("UTF-8 BOM"));
        assert!(script.contains("UTF-16 LE BOM"));
    }
}

#[test]
fn route_plan_disables_automatic_route_for_untrusted_or_nat_identity() {
    let untrusted = build_route_plan(route_plan_request(
        CredentialChannelClassification::UntrustedRemotePlainTcp,
        local_identity(),
    ));
    assert_eq!(
        untrusted.automatic_route,
        RouteActionAvailability::DisabledUntrustedChannel
    );

    let mut nat = local_identity();
    nat.classification = LocalNetworkClassification::NatSuspect;
    let nat_plan = build_route_plan(route_plan_request(
        CredentialChannelClassification::TrustedSameHost,
        nat,
    ));
    assert_eq!(
        nat_plan.automatic_route,
        RouteActionAvailability::DisabledNatOrPublic
    );
}

#[test]
fn route_add_policy_rejects_untrusted_channel_before_wire_call() {
    let mut wire = NoCallWire;
    let error = add_route_with_channel_policy(
        &mut wire,
        &route_add_request(target_identity(), local_identity()),
        CredentialChannelClassification::UntrustedRemotePlainTcp,
    )
    .expect_err("untrusted channel must be rejected");

    assert_eq!(error.kind, OnboardingWireErrorKind::UnsupportedOperation);
}

#[test]
fn route_add_policy_rejects_nat_identity_before_wire_call() {
    let mut local = local_identity();
    local.classification = LocalNetworkClassification::NatSuspect;
    let mut wire = NoCallWire;

    let error = add_route_with_channel_policy(
        &mut wire,
        &route_add_request(target_identity(), local),
        CredentialChannelClassification::TrustedSameHost,
    )
    .expect_err("NAT route must be rejected");

    assert_eq!(error.kind, OnboardingWireErrorKind::NatOrPublic);
}

#[test]
fn route_add_policy_allows_trusted_channel_to_call_wire() {
    let mut wire = MockAdsOnboardingWire::default();

    add_route_with_channel_policy(
        &mut wire,
        &route_add_request(target_identity(), local_identity()),
        CredentialChannelClassification::TrustedSameHost,
    )
    .expect("trusted route-add");
}

#[test]
fn wire_error_classification_keeps_remediation_machine_readable() {
    let error = OnboardingWireError::new(
        OnboardingWireErrorKind::RouteMissing,
        "target rejected route",
    );
    let classification = error.classification();

    assert_eq!(classification.next_action.kind, NextActionKind::AddRoute);
    assert!(classification.remediation.contains("static ADS route"));
    assert!(classification.blocks_production_ready);
}

#[test]
fn default_local_ams_net_id_uses_selected_ipv4_address() {
    assert_eq!(
        derive_default_ams_net_id("192.168.10.20"),
        Some("192.168.10.20.1.1".to_string())
    );
    assert_eq!(derive_default_ams_net_id("not-an-ip"), None);
    assert_eq!(derive_default_ams_net_id("999.168.10.20"), None);
}

#[test]
fn manual_discovery_path_does_not_depend_on_udp_identify() {
    let mut wire = MockAdsOnboardingWire::new(MockAdsOnboardingScenario::WrongIp);
    let request = DiscoveryRequest::manual_with_ams_net_id("192.168.10.5", "5.23.91.12.1.1");

    let results = discover_targets(&mut wire, &request).expect("manual discovery");

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].source, DiscoverySource::Manual);
    assert_eq!(results[0].target.ip, "192.168.10.5");
    assert_eq!(results[0].target.ams_net_id, "5.23.91.12.1.1");
    assert_eq!(results[0].target.preferred_ams_port, None);
    assert!(results[0].target.responding_ads_ports.is_empty());
}

#[test]
fn directed_identify_collects_target_fields_from_wire() {
    let mut wire = MockAdsOnboardingWire::default();
    let request = DiscoveryRequest::manual("192.168.10.5");

    let results = discover_targets(&mut wire, &request).expect("directed identify");

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].source, DiscoverySource::DirectedIdentify);
    assert_eq!(results[0].target.name.as_deref(), Some("CX-1234"));
    assert_eq!(results[0].target.ip, "192.168.10.5");
    assert_eq!(results[0].target.ams_net_id, "5.23.91.12.1.1");
    assert_eq!(results[0].target.tc_version.as_deref(), Some("3.1.4024"));
    assert_eq!(results[0].target.preferred_ams_port, None);
    assert!(results[0].target.responding_ads_ports.is_empty());
}

#[test]
fn directed_broadcast_is_optional_and_deduplicated() {
    let mut wire = MockAdsOnboardingWire::default();
    let mut request = DiscoveryRequest::manual_with_ams_net_id("192.168.10.5", "5.23.91.12.1.1");
    request.include_broadcast = true;
    request.broadcast_targets = vec!["192.168.10.255".to_string()];

    let results = discover_targets(&mut wire, &request).expect("discovery");

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].source, DiscoverySource::Manual);
    assert_eq!(
        directed_broadcast_target("192.168.10.20", 24).as_deref(),
        Some("192.168.10.255")
    );
    assert_eq!(
        directed_broadcast_target("10.10.5.20", 16).as_deref(),
        Some("10.10.255.255")
    );
    assert_eq!(directed_broadcast_target("192.168.10.20", 33), None);

    let candidates = vec![
        RuntimeAddressCandidate {
            ip: "192.168.10.20".to_string(),
            nic: Some("eth0".to_string()),
            prefix_len: Some(24),
            broadcast: None,
        },
        RuntimeAddressCandidate {
            ip: "10.10.5.20".to_string(),
            nic: Some("eth1".to_string()),
            prefix_len: Some(16),
            broadcast: Some("10.10.255.254".to_string()),
        },
        RuntimeAddressCandidate {
            ip: "192.168.77.10".to_string(),
            nic: Some("eth0".to_string()),
            prefix_len: Some(24),
            broadcast: Some("192.168.77.10".to_string()),
        },
        RuntimeAddressCandidate {
            ip: "127.0.0.1".to_string(),
            nic: Some("lo".to_string()),
            prefix_len: Some(8),
            broadcast: None,
        },
    ];
    assert_eq!(
        directed_broadcast_targets_from_candidates(&candidates),
        vec![
            "192.168.10.255".to_string(),
            "10.10.255.254".to_string(),
            "192.168.77.255".to_string()
        ]
    );
}

#[test]
fn directed_broadcast_collects_multiple_ads_targets_from_one_subnet() {
    let mut wire = MultiBroadcastWire::new(vec![
        TargetIdentity {
            name: Some("trust-runtime".to_string()),
            ip: "192.168.77.10".to_string(),
            ams_net_id: "192.168.77.10.1.1".to_string(),
            ams_port: 851,
            tc_version: Some("1.0.0".to_string()),
        },
        TargetIdentity {
            name: Some("TwinCAT-XAE".to_string()),
            ip: "192.168.50.42".to_string(),
            ams_net_id: "10.20.30.40.1.1".to_string(),
            ams_port: 851,
            tc_version: Some("3.1.4026".to_string()),
        },
    ]);
    let request = DiscoveryRequest {
        target: None,
        target_ams_net_id: None,
        ams_port: Some(851),
        target_name: None,
        include_broadcast: true,
        broadcast_targets: vec!["192.168.77.255".to_string()],
        timeout_ms: None,
    };

    let results = discover_targets(&mut wire, &request).expect("broadcast discovery");

    assert_eq!(results.len(), 2);
    assert!(results
        .iter()
        .all(|result| result.source == DiscoverySource::DirectedBroadcast));
    assert!(results
        .iter()
        .any(|result| result.target.ams_net_id == "192.168.77.10.1.1"));
    assert!(results
        .iter()
        .any(|result| result.target.ams_net_id == "10.20.30.40.1.1"));
}

#[test]
fn empty_broadcast_reply_is_normal_zero_device_result_without_warning() {
    let mut wire = MultiBroadcastWire::new(Vec::new());
    let request = DiscoveryRequest {
        target: None,
        target_ams_net_id: None,
        ams_port: Some(851),
        target_name: None,
        include_broadcast: true,
        broadcast_targets: vec!["192.168.77.255".to_string()],
        timeout_ms: None,
    };

    let report = discover_targets_report(&mut wire, &request).expect("empty broadcast result");

    assert!(report.results.is_empty());
    assert!(report.warnings.is_empty());
}

#[test]
fn identity_derivation_selects_runtime_host_source_and_candidates() {
    let request = IdentityRequest {
        target_ip: "192.168.10.5".to_string(),
        local_net_id_override: None,
    };

    let identity = derive_runtime_identity_from_source(
        &request,
        "192.168.10.20",
        Some("line-controller-1".to_string()),
        Some("eth0".to_string()),
        vec![
            RuntimeAddressCandidate {
                ip: "192.168.10.20".to_string(),
                nic: Some("eth0".to_string()),
                prefix_len: Some(24),
                broadcast: Some("192.168.10.255".to_string()),
            },
            RuntimeAddressCandidate {
                ip: "100.64.0.3".to_string(),
                nic: Some("tailscale0".to_string()),
                prefix_len: Some(10),
                broadcast: None,
            },
        ],
    )
    .expect("derive identity");

    assert_eq!(identity.chosen_ip, "192.168.10.20");
    assert_eq!(identity.ams_net_id, "192.168.10.20.1.1");
    assert_eq!(identity.classification, LocalNetworkClassification::Lan);
    assert_eq!(identity.candidates.len(), 2);
    assert!(identity
        .candidates
        .iter()
        .any(|candidate| candidate.selected && candidate.nic.as_deref() == Some("eth0")));
    assert_eq!(
        auto_route_availability_for_identity(&identity),
        RouteActionAvailability::Available
    );
}

#[test]
fn identity_derivation_honors_advanced_local_net_id_override() {
    let request = IdentityRequest {
        target_ip: "192.168.10.5".to_string(),
        local_net_id_override: Some("1.2.3.4.5.6".to_string()),
    };

    let identity =
        derive_runtime_identity_from_source(&request, "192.168.10.20", None, None, Vec::new())
            .expect("derive identity");

    assert_eq!(identity.ams_net_id, "1.2.3.4.5.6");
}

#[test]
fn network_classification_covers_lan_vpn_tailscale_loopback_public_and_nat() {
    assert_eq!(
        classify_local_address("192.168.10.20", Some("eth0")),
        LocalNetworkClassification::Lan
    );
    assert_eq!(
        classify_local_address("10.8.0.2", Some("wg0")),
        LocalNetworkClassification::Vpn
    );
    assert_eq!(
        classify_local_address("100.64.0.3", Some("eth0")),
        LocalNetworkClassification::Tailscale
    );
    assert_eq!(
        classify_local_address("127.0.0.1", Some("lo")),
        LocalNetworkClassification::Loopback
    );
    assert_eq!(
        classify_local_address("8.8.8.8", Some("eth0")),
        LocalNetworkClassification::Public
    );

    let request = IdentityRequest {
        target_ip: "8.8.8.8".to_string(),
        local_net_id_override: None,
    };
    let identity = derive_runtime_identity_from_source(
        &request,
        "192.168.10.20",
        None,
        Some("eth0".to_string()),
        Vec::new(),
    )
    .expect("derive identity");
    assert_eq!(
        identity.classification,
        LocalNetworkClassification::NatSuspect
    );
    assert_eq!(
        auto_route_availability_for_identity(&identity),
        RouteActionAvailability::DisabledNatOrPublic
    );
}

#[test]
fn source_probe_rejects_invalid_target_ip_without_network_io() {
    let error = resolve_os_source_ip("not-an-ip").expect_err("invalid target must fail");

    assert!(error.to_string().contains("invalid target IP"));
}

#[test]
fn onboarding_boundary_keeps_raw_ads_types_out_of_public_schema() {
    let ads_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/host/ads");
    for relative in [
        "diagnostics.rs",
        "onboarding/discover.rs",
        "onboarding/doctor.rs",
        "onboarding/errors.rs",
        "onboarding/identity.rs",
        "onboarding/import.rs",
        "onboarding/route.rs",
    ] {
        let path = ads_root.join(relative);
        let source = std::fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
        let without_local_ads_module = source.replace("crate::ads::", "crate_ads_mod__");
        assert!(
            !without_local_ads_module.contains("ads::"),
            "{} leaks raw ads crate types",
            path.display()
        );
    }

    let contract = std::fs::read_to_string(ads_root.join("onboarding/wire/contract.rs"))
        .expect("read onboarding wire contract");
    let trait_boundary = contract
        .split_once("pub trait AdsOnboardingWire")
        .and_then(|(_, rest)| rest.split_once("pub enum DirectedIdentityTransport"))
        .map(|(boundary, _)| boundary)
        .expect("locate onboarding wire trait boundary");
    assert!(!trait_boundary.contains("AdsTransport"));
    assert!(!trait_boundary.contains("transport::"));

    let debug_ads_bootstrap = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../trust-debug/src/session/source_and_parse_helpers.rs"),
    )
    .expect("read trust-debug ADS bootstrap");
    assert!(!debug_ads_bootstrap.contains("AdsRsTransport::new"));
    assert!(debug_ads_bootstrap.contains("HostAdsTransport::new"));
}

fn assert_wire_failure(
    scenario: MockAdsOnboardingScenario,
    expected: OnboardingWireErrorKind,
    action: impl FnOnce(
        &mut MockAdsOnboardingWire,
        &TargetIdentity,
        &LocalIdentity,
    ) -> Result<(), OnboardingWireError>,
) {
    let mut wire = MockAdsOnboardingWire::new(scenario);
    let target = target_identity();
    let local = local_identity();
    let error = action(&mut wire, &target, &local).expect_err("scenario should fail");

    assert_eq!(error.kind, expected);
    assert!(error.classification().blocks_production_ready);
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

fn route_add_request(target: TargetIdentity, local: LocalIdentity) -> RouteAddRequest {
    RouteAddRequest {
        route_name: "trust-runtime-line-controller-1".to_string(),
        target,
        local,
        credentials: RouteCredentials {
            username: "Administrator".to_string(),
            password: "not-persisted".to_string(),
        },
    }
}

fn route_plan_request(
    channel: CredentialChannelClassification,
    local: LocalIdentity,
) -> RoutePlanRequest {
    RoutePlanRequest {
        role: RoutePlanRole::Client,
        route_name: "trust-runtime-line-controller-1".to_string(),
        target: target_identity(),
        local,
        channel,
    }
}

#[test]
fn server_route_artifacts_do_not_include_client_mode_1861_warning() {
    let mut request = route_plan_request(
        CredentialChannelClassification::TrustedSameHost,
        local_identity(),
    );
    request.role = RoutePlanRole::Server;
    let plan = build_route_plan(request);
    let manual = artifact(&plan, RouteArtifactKind::ManualSteps)
        .content
        .as_str();
    let powershell = artifact(&plan, RouteArtifactKind::Powershell);

    assert!(manual.contains("truST ADS server"));
    assert!(!manual.contains("truST is an ADS client"));
    assert!(!manual.contains("ADS Error 1861"));
    assert_eq!(powershell.label, "Download PowerShell for TwinCAT station");
    assert!(powershell
        .content
        .contains("Run this on the external ADS client / TwinCAT engineering station"));
    assert!(powershell.content.contains("not on the truST runtime host"));
}

fn artifact(
    plan: &crate::ads::diagnostics::RoutePlan,
    kind: RouteArtifactKind,
) -> &crate::ads::diagnostics::RouteArtifact {
    plan.artifacts
        .iter()
        .find(|artifact| artifact.kind == kind)
        .unwrap_or_else(|| panic!("missing route artifact {kind:?}"))
}

fn step(
    report: &crate::ads::diagnostics::DoctorReport,
    id: DoctorStepId,
) -> &crate::ads::diagnostics::DoctorStep {
    report
        .steps
        .iter()
        .find(|step| step.id == id)
        .unwrap_or_else(|| panic!("missing doctor step {id:?}"))
}

fn production_evidence() -> ProductionEvidence {
    ProductionEvidence {
        doctor_timestamp_ms: 1_700_000_000_000,
        doctor_schema_version: ADS_DIAGNOSTICS_SCHEMA_VERSION,
        runtime_identity_hash: "sha256:runtime".to_string(),
        target_identity_hash: Some("sha256:target".to_string()),
        allowed_clients_hash: None,
        ads_config_hash: "sha256:config".to_string(),
        symbol_snapshot_hash: "sha256:snapshot".to_string(),
        generated_st_hash: Some("sha256:st".to_string()),
        deployed_ads_config_hash: Some("sha256:deployed".to_string()),
        runtime_ads_status_hash: Some("sha256:status".to_string()),
        external_client_verified: false,
        external_client_kind: None,
        external_client_name: None,
        external_client_timestamp_ms: None,
        discoverable: false,
        freshness: EvidenceFreshness {
            stale_after_ms: 300_000,
            expires_at_ms: Some(1_700_000_300_000),
            runtime_clock_warning: None,
        },
    }
}

fn test_write_probe() -> GuardedWriteProbe {
    GuardedWriteProbe {
        symbol: "MAIN.Temperature".to_string(),
        data_type: AdsDataTypeDescriptor::scalar("REAL", IecDataType::Real),
        value: Value::Real(21.5),
    }
}

fn active_device_snapshot() -> ActiveAdsDeviceSnapshot {
    ActiveAdsDeviceSnapshot {
        connection_name: "line1".to_string(),
        target: target_identity(),
        local: Some(local_identity()),
        state: crate::ads::diagnostics::AdsConnectionStatusState::Connected,
        point_statuses: vec![PointStatus {
            point_name: "line1_temp".to_string(),
            quality: PointQuality::good(123),
        }],
        symbol_version: Some(7),
    }
}

struct MultiBroadcastWire {
    inner: MockAdsOnboardingWire,
    targets: Vec<TargetIdentity>,
}

impl MultiBroadcastWire {
    fn new(targets: Vec<TargetIdentity>) -> Self {
        Self {
            inner: MockAdsOnboardingWire::default(),
            targets,
        }
    }
}

impl AdsOnboardingWire for MultiBroadcastWire {
    fn udp_identify(&mut self, target_ip: &str) -> Result<TargetIdentity, OnboardingWireError> {
        self.inner.udp_identify(target_ip)
    }

    fn udp_identify_all(
        &mut self,
        _target_ip: &str,
    ) -> Result<Vec<TargetIdentity>, OnboardingWireError> {
        Ok(self.targets.clone())
    }

    fn tcp_probe_48898(&mut self, target_ip: &str) -> Result<(), OnboardingWireError> {
        self.inner.tcp_probe_48898(target_ip)
    }

    fn check_route(
        &mut self,
        target: &TargetIdentity,
        local: &LocalIdentity,
    ) -> Result<(), OnboardingWireError> {
        self.inner.check_route(target, local)
    }

    fn verify_ams_target(&mut self, target: &TargetIdentity) -> Result<(), OnboardingWireError> {
        self.inner.verify_ams_target(target)
    }

    fn read_state(&mut self, target: &TargetIdentity) -> Result<String, OnboardingWireError> {
        self.inner.read_state(target)
    }

    fn upload_symbols(
        &mut self,
        target: &TargetIdentity,
    ) -> Result<Vec<SymbolDescriptor>, OnboardingWireError> {
        self.inner.upload_symbols(target)
    }

    fn resolve_handle(
        &mut self,
        target: &TargetIdentity,
        symbol: &str,
    ) -> Result<u32, OnboardingWireError> {
        self.inner.resolve_handle(target, symbol)
    }

    fn sumup_read(
        &mut self,
        target: &TargetIdentity,
        handles: &[u32],
    ) -> Result<Vec<Vec<u8>>, OnboardingWireError> {
        self.inner.sumup_read(target, handles)
    }

    fn guarded_write_probe(
        &mut self,
        target: &TargetIdentity,
        probe: &GuardedWriteProbe,
    ) -> Result<(), OnboardingWireError> {
        self.inner.guarded_write_probe(target, probe)
    }

    fn subscribe_notification(
        &mut self,
        target: &TargetIdentity,
        symbol: &str,
    ) -> Result<(), OnboardingWireError> {
        self.inner.subscribe_notification(target, symbol)
    }

    fn read_update_sample(
        &mut self,
        target: &TargetIdentity,
        symbol: &str,
    ) -> Result<AdsReadUpdateSample, OnboardingWireError> {
        self.inner.read_update_sample(target, symbol)
    }

    fn symbol_version(&mut self, target: &TargetIdentity) -> Result<u32, OnboardingWireError> {
        self.inner.symbol_version(target)
    }

    fn add_route(&mut self, request: &RouteAddRequest) -> Result<(), OnboardingWireError> {
        self.inner.add_route(request)
    }

    fn remove_route(&mut self, request: &RouteRemoveRequest) -> Result<(), OnboardingWireError> {
        self.inner.remove_route(request)
    }
}

struct NoCallWire;

impl AdsOnboardingWire for NoCallWire {
    fn udp_identify(&mut self, _target_ip: &str) -> Result<TargetIdentity, OnboardingWireError> {
        panic!("active-device doctor opened duplicate UDP identify")
    }

    fn tcp_probe_48898(&mut self, _target_ip: &str) -> Result<(), OnboardingWireError> {
        panic!("active-device doctor opened duplicate TCP probe")
    }

    fn check_route(
        &mut self,
        _target: &TargetIdentity,
        _local: &LocalIdentity,
    ) -> Result<(), OnboardingWireError> {
        panic!("active-device doctor opened duplicate route check")
    }

    fn verify_ams_target(&mut self, _target: &TargetIdentity) -> Result<(), OnboardingWireError> {
        panic!("active-device doctor opened duplicate AMS target check")
    }

    fn read_state(&mut self, _target: &TargetIdentity) -> Result<String, OnboardingWireError> {
        panic!("active-device doctor opened duplicate read_state")
    }

    fn upload_symbols(
        &mut self,
        _target: &TargetIdentity,
    ) -> Result<Vec<SymbolDescriptor>, OnboardingWireError> {
        panic!("active-device doctor opened duplicate symbol upload")
    }

    fn resolve_handle(
        &mut self,
        _target: &TargetIdentity,
        _symbol: &str,
    ) -> Result<u32, OnboardingWireError> {
        panic!("active-device doctor opened duplicate handle resolve")
    }

    fn sumup_read(
        &mut self,
        _target: &TargetIdentity,
        _handles: &[u32],
    ) -> Result<Vec<Vec<u8>>, OnboardingWireError> {
        panic!("active-device doctor opened duplicate sum-up read")
    }

    fn guarded_write_probe(
        &mut self,
        _target: &TargetIdentity,
        _probe: &GuardedWriteProbe,
    ) -> Result<(), OnboardingWireError> {
        panic!("active-device doctor opened duplicate guarded write")
    }

    fn subscribe_notification(
        &mut self,
        _target: &TargetIdentity,
        _symbol: &str,
    ) -> Result<(), OnboardingWireError> {
        panic!("active-device doctor opened duplicate notification")
    }

    fn read_update_sample(
        &mut self,
        _target: &TargetIdentity,
        _symbol: &str,
    ) -> Result<AdsReadUpdateSample, OnboardingWireError> {
        panic!("active-device doctor opened duplicate read update")
    }

    fn symbol_version(&mut self, _target: &TargetIdentity) -> Result<u32, OnboardingWireError> {
        panic!("active-device doctor opened duplicate symbol version")
    }

    fn add_route(&mut self, _request: &RouteAddRequest) -> Result<(), OnboardingWireError> {
        panic!("active-device doctor opened duplicate AddRoute")
    }

    fn remove_route(&mut self, _request: &RouteRemoveRequest) -> Result<(), OnboardingWireError> {
        panic!("active-device doctor opened duplicate route removal")
    }
}
