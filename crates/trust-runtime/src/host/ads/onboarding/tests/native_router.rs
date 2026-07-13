use serde_json::json;

use super::*;

#[test]
fn doctor_does_not_report_route_present_when_ads_round_trip_times_out() {
    let mut wire = MockAdsOnboardingWire::new(MockAdsOnboardingScenario::TcpAcceptedNoAdsReply);
    let report = run_doctor(
        &mut wire,
        DoctorOptions::runtime_host("192.168.10.5", local_identity())
            .with_expected_target_ams_net_id("5.23.91.12.1.1"),
        &DoctorCancellation::new(),
    );

    assert_eq!(
        step(&report, DoctorStepId::Tcp48898).status,
        DoctorStepStatus::Pass,
        "a TCP accept only proves that port 48898 is reachable"
    );
    let route = step(&report, DoctorStepId::RoutePresent);
    assert_eq!(route.status, DoctorStepStatus::Fail);
    assert_eq!(route.next_action.kind, NextActionKind::AddRoute);
    assert!(route.remediation.contains("static ADS route"));
    assert_eq!(
        step(&report, DoctorStepId::ReadState).status,
        DoctorStepStatus::Skip,
        "the route failure must block duplicate downstream probes"
    );
}

#[test]
fn doctor_with_manual_target_identity_does_not_require_udp_identify() {
    let mut wire = NoUdpIdentifyWire::default();
    let cancellation = DoctorCancellation::new();

    let report = run_doctor(
        &mut wire,
        DoctorOptions::runtime_host("192.168.10.5", local_identity())
            .with_expected_target_ams_net_id("5.23.91.12.1.1"),
        &cancellation,
    );

    let identify = step(&report, DoctorStepId::UdpIdentify);
    assert_eq!(identify.status, DoctorStepStatus::Pass);
    assert_eq!(
        identify.evidence.get("target_source"),
        Some(&json!("manual"))
    );
    assert_eq!(
        step(&report, DoctorStepId::Tcp48898).status,
        DoctorStepStatus::Pass
    );
    assert_eq!(
        step(&report, DoctorStepId::SymbolUpload).status,
        DoctorStepStatus::Pass
    );
    assert_eq!(
        step(&report, DoctorStepId::SumupRead).status,
        DoctorStepStatus::Pass
    );
}

#[test]
fn doctor_uses_native_round_trip_instead_of_raw_tcp_for_local_windows_router() {
    let mut native_source = local_identity();
    native_source.chosen_ip = "127.0.0.1".to_string();
    native_source.ams_net_id = "10.20.30.41.1.1".to_string();
    let mut wire = NoUdpIdentifyWire {
        inner: MockAdsOnboardingWire::default(),
        native_router: true,
        native_route_timeout: false,
    };

    let report = run_doctor(
        &mut wire,
        DoctorOptions::runtime_host("127.0.0.1", native_source)
            .with_expected_target_ams_net_id("10.20.30.40.1.1"),
        &DoctorCancellation::new(),
    );

    let transport = step(&report, DoctorStepId::Tcp48898);
    assert_eq!(transport.status, DoctorStepStatus::Pass);
    assert_eq!(
        transport.evidence.get("probe_transport"),
        Some(&json!("native_windows_router"))
    );
    assert_eq!(
        transport.evidence.get("probe_operation"),
        Some(&json!("read_state"))
    );
    assert_eq!(
        step(&report, DoctorStepId::UdpIdentify)
            .evidence
            .get("target_ams_net_id"),
        Some(&json!("10.20.30.40.1.1"))
    );
    let route = step(&report, DoctorStepId::RoutePresent);
    assert_eq!(
        route.evidence.get("local_ams_net_id"),
        Some(&json!("10.20.30.41.1.1"))
    );
    assert_eq!(
        route.evidence.get("route_mode"),
        Some(&json!("native_local_no_self_route"))
    );
    assert_ne!(route.next_action.kind, NextActionKind::AddRoute);
}

#[test]
fn native_local_timeout_never_recommends_a_self_route() {
    let mut wire = NoUdpIdentifyWire {
        inner: MockAdsOnboardingWire::default(),
        native_router: true,
        native_route_timeout: true,
    };

    let report = run_doctor(
        &mut wire,
        DoctorOptions::runtime_host("127.0.0.1", local_identity())
            .with_expected_target_ams_net_id("10.20.30.40.1.1"),
        &DoctorCancellation::new(),
    );

    let route = step(&report, DoctorStepId::RoutePresent);
    assert_ne!(route.next_action.kind, NextActionKind::AddRoute);
    assert_eq!(
        route.evidence.get("route_mode"),
        Some(&json!("native_local_no_self_route"))
    );
    assert_eq!(
        step(&report, DoctorStepId::ReadState).next_action.kind,
        NextActionKind::PickTarget
    );
}

#[derive(Default)]
struct NoUdpIdentifyWire {
    inner: MockAdsOnboardingWire,
    native_router: bool,
    native_route_timeout: bool,
}

impl AdsOnboardingWire for NoUdpIdentifyWire {
    fn udp_identify(&mut self, _target_ip: &str) -> Result<TargetIdentity, OnboardingWireError> {
        panic!("manual-target doctor called udp_identify")
    }

    fn tcp_probe_48898(&mut self, target_ip: &str) -> Result<(), OnboardingWireError> {
        assert!(
            !self.native_router,
            "native local doctor used raw TCP 48898"
        );
        self.inner.tcp_probe_48898(target_ip)
    }

    fn probe_ads_router(
        &mut self,
        target: &TargetIdentity,
    ) -> Result<AdsRouterProbe, OnboardingWireError> {
        if self.native_router {
            self.inner.read_state(target)?;
            Ok(AdsRouterProbe::NativeRouterRoundTrip)
        } else {
            self.tcp_probe_48898(&target.ip)?;
            Ok(AdsRouterProbe::Tcp48898)
        }
    }

    fn route_requirement(&self, _target_ip: &str) -> AdsRouteRequirement {
        if self.native_router {
            AdsRouteRequirement::NativeLocalRouter
        } else {
            AdsRouteRequirement::ReciprocalRouteRequired
        }
    }

    fn check_route(
        &mut self,
        target: &TargetIdentity,
        local: &LocalIdentity,
    ) -> Result<(), OnboardingWireError> {
        self.inner.check_route(target, local)
    }

    fn verify_ams_target(&mut self, target: &TargetIdentity) -> Result<(), OnboardingWireError> {
        if self.native_router {
            Ok(())
        } else {
            self.inner.verify_ams_target(target)
        }
    }

    fn read_state(&mut self, target: &TargetIdentity) -> Result<String, OnboardingWireError> {
        if self.native_route_timeout {
            return Err(OnboardingWireError::new(
                OnboardingWireErrorKind::WrongPlcPort,
                "ADS client synchronous timeout",
            ));
        }
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
