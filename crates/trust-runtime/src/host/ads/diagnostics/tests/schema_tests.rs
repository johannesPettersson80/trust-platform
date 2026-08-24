use super::*;

macro_rules! assert_wire_names {
    ($values:expr, [$($expected:literal),+ $(,)?]) => {{
        let names = $values
            .into_iter()
            .map(|value| {
                serde_json::to_value(value)
                    .expect("serialize wire enum")
                    .as_str()
                    .expect("wire enum string")
                    .to_string()
            })
            .collect::<Vec<_>>();
        assert_eq!(names, vec![$($expected),+]);
    }};
}

#[test]
fn diagnostic_wire_enums_keep_the_version_two_contract_names() {
    assert_wire_names!(
        [DoctorRole::Client, DoctorRole::Server],
        ["client", "server"]
    );
    assert_wire_names!(
        [
            DoctorVantage::RuntimeHost,
            DoctorVantage::SetupWebRuntimeHost,
            DoctorVantage::VscodeCli,
            DoctorVantage::VscodeAuthoringOnly,
            DoctorVantage::CliLocal,
        ],
        [
            "runtime-host",
            "setup-web-runtime-host",
            "vscode-cli",
            "vscode-authoring-only",
            "cli-local",
        ]
    );
    assert_wire_names!(
        [DiagnosticTransport::Plain, DiagnosticTransport::Secure],
        ["plain", "secure"]
    );
    assert_wire_names!(
        [
            DoctorOverall::Pass,
            DoctorOverall::Partial,
            DoctorOverall::Fail
        ],
        ["pass", "partial", "fail"]
    );
    assert_wire_names!(
        [
            DoctorStepStatus::Pass,
            DoctorStepStatus::Warn,
            DoctorStepStatus::Fail,
            DoctorStepStatus::Skip,
        ],
        ["pass", "warn", "fail", "skip"]
    );
    assert_wire_names!(
        [
            DoctorStepId::UdpIdentify,
            DoctorStepId::LocalIdentity,
            DoctorStepId::Tcp48898,
            DoctorStepId::RoutePresent,
            DoctorStepId::AmsTarget,
            DoctorStepId::ReadState,
            DoctorStepId::SymbolUpload,
            DoctorStepId::HandleResolve,
            DoctorStepId::SumupRead,
            DoctorStepId::WriteGuarded,
            DoctorStepId::Notification,
            DoctorStepId::SymbolVersion,
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
        ],
        [
            "udp_identify",
            "local_identity",
            "tcp_48898",
            "route_present",
            "ams_target",
            "read_state",
            "symbol_upload",
            "handle_resolve",
            "sumup_read",
            "write_guarded",
            "notification",
            "symbol_version",
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
    assert_wire_names!(
        [
            LocalNetworkClassification::Lan,
            LocalNetworkClassification::Vpn,
            LocalNetworkClassification::Tailscale,
            LocalNetworkClassification::Loopback,
            LocalNetworkClassification::Public,
            LocalNetworkClassification::NatSuspect,
            LocalNetworkClassification::Unknown,
        ],
        [
            "lan",
            "vpn",
            "tailscale",
            "loopback",
            "public",
            "nat_suspect",
            "unknown"
        ]
    );
    assert_wire_names!(
        [
            RouteActionAvailability::Available,
            RouteActionAvailability::DisabledUntrustedChannel,
            RouteActionAvailability::DisabledNatOrPublic,
            RouteActionAvailability::DisabledUnsupported,
        ],
        [
            "available",
            "disabled_untrusted_channel",
            "disabled_nat_or_public",
            "disabled_unsupported",
        ]
    );
    assert_wire_names!(
        [
            RouteArtifactKind::Powershell,
            RouteArtifactKind::StaticRoutesXml,
            RouteArtifactKind::ManualSteps,
            RouteArtifactKind::RemovalPowershell,
        ],
        [
            "powershell",
            "static_routes_xml",
            "manual_steps",
            "removal_powershell"
        ]
    );
    assert_wire_names!(
        [
            CredentialChannelClassification::TrustedSameHost,
            CredentialChannelClassification::TrustedHttpsAdmin,
            CredentialChannelClassification::LocalCliDirectAddRoute,
            CredentialChannelClassification::UntrustedRemotePlainTcp,
            CredentialChannelClassification::UntrustedPlainHttpNetwork,
        ],
        [
            "trusted_same_host",
            "trusted_https_admin",
            "local_cli_direct_add_route",
            "untrusted_remote_plain_tcp",
            "untrusted_plain_http_network",
        ]
    );
    assert_wire_names!(
        [
            AdsStatusOverall::Healthy,
            AdsStatusOverall::Degraded,
            AdsStatusOverall::NotReady,
            AdsStatusOverall::Faulted,
            AdsStatusOverall::Disabled,
            AdsStatusOverall::Unknown,
        ],
        [
            "healthy",
            "degraded",
            "not_ready",
            "faulted",
            "disabled",
            "unknown"
        ]
    );
    assert_wire_names!(
        [
            AdsConnectionStatusState::Connected,
            AdsConnectionStatusState::Reconnecting,
            AdsConnectionStatusState::NotReady,
            AdsConnectionStatusState::Faulted,
            AdsConnectionStatusState::Stale,
            AdsConnectionStatusState::Disabled,
            AdsConnectionStatusState::Unknown,
        ],
        [
            "connected",
            "reconnecting",
            "not_ready",
            "faulted",
            "stale",
            "disabled",
            "unknown",
        ]
    );
    assert_wire_names!(
        [
            ProductionReadinessState::Ready,
            ProductionReadinessState::NeedsRecheck,
            ProductionReadinessState::NotReady,
        ],
        ["ready", "needs_recheck", "not_ready"]
    );
    assert_wire_names!(
        [
            ProductionReadinessReason::MissingEvidence,
            ProductionReadinessReason::MissingRuntimeStatus,
            ProductionReadinessReason::EvidenceExpired,
            ProductionReadinessReason::RuntimeClockWarning,
            ProductionReadinessReason::DeployedAdsConfigMissing,
            ProductionReadinessReason::DeployedAdsConfigMismatch,
            ProductionReadinessReason::RuntimeAdsStatusChanged,
            ProductionReadinessReason::RuntimeAdsFaulted,
            ProductionReadinessReason::RuntimeAdsDegraded,
        ],
        [
            "missing_evidence",
            "missing_runtime_status",
            "evidence_expired",
            "runtime_clock_warning",
            "deployed_ads_config_missing",
            "deployed_ads_config_mismatch",
            "runtime_ads_status_changed",
            "runtime_ads_faulted",
            "runtime_ads_degraded",
        ]
    );
}

#[test]
fn diagnostic_maps_and_action_builders_serialize_deterministically() {
    let action = NextAction::new(NextActionKind::Deploy)
        .with_param("zeta", 2)
        .with_param("alpha", 1);

    assert_eq!(
        serde_json::to_string(&action).expect("action JSON"),
        r#"{"kind":"deploy","params":{"alpha":1,"zeta":2}}"#
    );
    assert_eq!(NextAction::default(), NextAction::new(NextActionKind::None));
}
