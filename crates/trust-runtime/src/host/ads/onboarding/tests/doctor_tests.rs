use super::*;

#[test]
fn required_doctor_steps_and_timeouts_match_spec() {
    assert_eq!(
        REQUIRED_DOCTOR_STEPS,
        &[
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
        ]
    );

    let timeouts = default_step_timeouts();
    assert!(timeouts.contains(&DoctorStepTimeout {
        step: DoctorStepId::UdpIdentify,
        timeout_ms: 3_000,
    }));
    assert!(timeouts.contains(&DoctorStepTimeout {
        step: DoctorStepId::Tcp48898,
        timeout_ms: 1_000,
    }));
    for step in [
        DoctorStepId::ReadState,
        DoctorStepId::SymbolUpload,
        DoctorStepId::HandleResolve,
        DoctorStepId::SumupRead,
        DoctorStepId::Notification,
        DoctorStepId::SymbolVersion,
    ] {
        assert!(timeouts.contains(&DoctorStepTimeout {
            step,
            timeout_ms: 5_000,
        }));
    }
}

#[test]
fn doctor_happy_path_runs_required_steps_and_skips_write_probe_by_default() {
    let mut wire = MockAdsOnboardingWire::default();
    let cancellation = DoctorCancellation::new();

    let report = run_doctor(
        &mut wire,
        DoctorOptions::runtime_host("192.168.10.5", local_identity())
            .with_expected_target_ams_net_id("5.23.91.12.1.1"),
        &cancellation,
    );

    assert_eq!(report.steps.len(), REQUIRED_DOCTOR_STEPS.len());
    assert_eq!(
        report.steps.iter().map(|step| step.id).collect::<Vec<_>>(),
        REQUIRED_DOCTOR_STEPS
    );
    let write = step(&report, DoctorStepId::WriteGuarded);
    assert_eq!(write.status, DoctorStepStatus::Skip);
    assert_eq!(write.skip_reason, Some(DoctorSkipReason::WritesDisabled));
    assert!(!write.blocks_production_ready);
    assert_eq!(report.overall, DoctorOverall::Partial);
    assert!(!report.production_ready);
}

#[test]
fn doctor_runs_guarded_write_probe_only_when_explicit() {
    let mut wire = MockAdsOnboardingWire::default();
    let cancellation = DoctorCancellation::new();

    let report = run_doctor(
        &mut wire,
        DoctorOptions::runtime_host("192.168.10.5", local_identity())
            .with_expected_target_ams_net_id("5.23.91.12.1.1")
            .with_write_probe(GuardedWriteProbe {
                symbol: "MAIN.Temperature".to_string(),
                data_type: AdsDataTypeDescriptor::scalar("REAL", IecDataType::Real),
                value: Value::Real(21.5),
            }),
        &cancellation,
    );

    let write = step(&report, DoctorStepId::WriteGuarded);
    assert_eq!(write.status, DoctorStepStatus::Pass);
    assert_eq!(
        write
            .evidence
            .get("symbol")
            .and_then(serde_json::Value::as_str),
        Some("MAIN.Temperature")
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
fn doctor_rejects_invalid_manual_endpoint_before_wire_activity() {
    for (target_ip, ams_net_id, ams_port) in [
        ("not-an-ip", "5.23.91.12.1.1", 851),
        ("192.168.10.5", "", 851),
        ("192.168.10.5", "5.23.91.12.1", 851),
        ("192.168.10.5", "5.23.91.12.1.999", 851),
        ("192.168.10.5", "05.23.91.12.1.1", 851),
        ("192.168.10.5", " 5.23.91.12.1.1", 851),
        ("192.168.10.5", "5.23.91.12.1.1", 0),
    ] {
        let mut options = DoctorOptions::runtime_host(target_ip, local_identity())
            .with_expected_target_ams_net_id(ams_net_id);
        options.ams_port = ams_port;

        let report = run_doctor(&mut NoCallWire, options, &DoctorCancellation::new());

        assert_eq!(report.overall, DoctorOverall::Fail);
        assert_eq!(report.steps.len(), REQUIRED_DOCTOR_STEPS.len());
        assert_eq!(report.steps[0].id, DoctorStepId::UdpIdentify);
        assert_eq!(report.steps[0].status, DoctorStepStatus::Fail);
        assert!(report.steps[1..]
            .iter()
            .all(|step| { step.skip_reason == Some(DoctorSkipReason::BlockedByPreviousStep) }));
    }
}

#[test]
fn doctor_rejects_blank_or_absent_selected_symbol_at_upload_boundary() {
    for selected_symbol in ["   ", "MAIN.DoesNotExist"] {
        let mut wire = MockAdsOnboardingWire::default();
        let report = run_doctor(
            &mut wire,
            DoctorOptions::runtime_host("192.168.10.5", local_identity())
                .with_expected_target_ams_net_id("5.23.91.12.1.1")
                .with_selected_symbol(selected_symbol),
            &DoctorCancellation::new(),
        );

        assert_eq!(report.overall, DoctorOverall::Fail);
        assert_eq!(
            step(&report, DoctorStepId::SymbolUpload).status,
            DoctorStepStatus::Fail
        );
        assert_eq!(
            step(&report, DoctorStepId::HandleResolve).skip_reason,
            Some(DoctorSkipReason::BlockedByPreviousStep)
        );
    }
}

#[test]
fn doctor_maps_named_failures_to_the_failed_step_and_blocks_dependents() {
    for (scenario, failed_step, next_action) in [
        (
            MockAdsOnboardingScenario::WrongIp,
            DoctorStepId::UdpIdentify,
            NextActionKind::PickTarget,
        ),
        (
            MockAdsOnboardingScenario::FirewallBlocked,
            DoctorStepId::Tcp48898,
            NextActionKind::FixLocalIp,
        ),
        (
            MockAdsOnboardingScenario::MissingRoute,
            DoctorStepId::RoutePresent,
            NextActionKind::AddRoute,
        ),
        (
            MockAdsOnboardingScenario::WrongAmsNetId,
            DoctorStepId::AmsTarget,
            NextActionKind::PickTarget,
        ),
        (
            MockAdsOnboardingScenario::WrongPlcPort,
            DoctorStepId::ReadState,
            NextActionKind::PickTarget,
        ),
        (
            MockAdsOnboardingScenario::SecureRequired,
            DoctorStepId::ReadState,
            NextActionKind::UseSecure,
        ),
        (
            MockAdsOnboardingScenario::EmptySymbols,
            DoctorStepId::SymbolUpload,
            NextActionKind::None,
        ),
        (
            MockAdsOnboardingScenario::NotificationFailure,
            DoctorStepId::Notification,
            NextActionKind::None,
        ),
    ] {
        let mut wire = MockAdsOnboardingWire::new(scenario);
        let mut options = DoctorOptions::runtime_host("192.168.10.5", local_identity());
        if scenario != MockAdsOnboardingScenario::WrongIp {
            options = options.with_expected_target_ams_net_id("5.23.91.12.1.1");
        }
        let report = run_doctor(&mut wire, options, &DoctorCancellation::new());
        let failed = step(&report, failed_step);

        assert_eq!(failed.status, DoctorStepStatus::Fail, "{scenario:?}");
        assert_eq!(failed.next_action.kind, next_action, "{scenario:?}");
        assert!(failed.blocks_production_ready, "{scenario:?}");
        assert_eq!(report.overall, DoctorOverall::Fail, "{scenario:?}");

        let failed_index = REQUIRED_DOCTOR_STEPS
            .iter()
            .position(|id| *id == failed_step)
            .expect("failed step is required");
        if let Some(next_id) = REQUIRED_DOCTOR_STEPS.get(failed_index + 1) {
            assert_eq!(
                step(&report, *next_id).skip_reason,
                Some(DoctorSkipReason::BlockedByPreviousStep),
                "{scenario:?}"
            );
        }
    }
}

#[test]
fn doctor_job_reports_progress_and_cancellation() {
    let mut job = DoctorJob::new();
    let mut wire = MockAdsOnboardingWire::default();
    let report = job.run(
        &mut wire,
        DoctorOptions::runtime_host("192.168.10.5", local_identity())
            .with_expected_target_ams_net_id("5.23.91.12.1.1"),
    );

    assert_eq!(job.state(), DoctorJobState::Complete);
    assert_eq!(job.progress().completed_steps, REQUIRED_DOCTOR_STEPS.len());
    assert_eq!(job.progress().current_step, None);
    assert_eq!(job.report(), Some(&report));

    let mut cancelled = DoctorJob::new();
    cancelled.cancel();
    let mut wire = MockAdsOnboardingWire::default();
    let report = cancelled.run(
        &mut wire,
        DoctorOptions::runtime_host("192.168.10.5", local_identity()),
    );

    assert_eq!(cancelled.state(), DoctorJobState::Cancelled);
    assert_eq!(
        report.steps[0].skip_reason,
        Some(DoctorSkipReason::Cancelled)
    );
    assert_eq!(
        report.steps[1].skip_reason,
        Some(DoctorSkipReason::BlockedByPreviousStep)
    );
}

#[test]
fn production_ready_requires_pass_evidence_and_live_deployed_status() {
    let mut wire = MockAdsOnboardingWire::default();
    let passing_options = DoctorOptions::runtime_host("192.168.10.5", local_identity())
        .with_expected_target_ams_net_id("5.23.91.12.1.1")
        .with_write_probe(test_write_probe());
    let report = run_doctor(
        &mut wire,
        passing_options.clone(),
        &DoctorCancellation::new(),
    );
    assert_eq!(report.overall, DoctorOverall::Pass);
    assert!(!report.production_ready);

    let mut wire = MockAdsOnboardingWire::default();
    let report = run_doctor(
        &mut wire,
        passing_options
            .clone()
            .with_production_evidence(production_evidence(), false),
        &DoctorCancellation::new(),
    );
    assert_eq!(report.overall, DoctorOverall::Pass);
    assert!(!report.production_ready);

    let mut wire = MockAdsOnboardingWire::default();
    let report = run_doctor(
        &mut wire,
        passing_options.with_production_evidence(production_evidence(), true),
        &DoctorCancellation::new(),
    );
    assert_eq!(report.overall, DoctorOverall::Pass);
    assert!(report.production_ready);
    assert!(report.evidence.is_some());
}

#[test]
fn active_device_read_only_uses_live_status_and_never_opens_wire_connection() {
    let active = active_device_snapshot();
    let mut wire = NoCallWire;

    let report = run_doctor(
        &mut wire,
        DoctorOptions::runtime_host("192.168.10.5", local_identity())
            .with_active_device(active, ActiveDeviceStrategy::ReadOnlyViaLiveStatus),
        &DoctorCancellation::new(),
    );

    assert_eq!(
        report.target.as_ref().map(|target| target.ip.as_str()),
        Some("192.168.10.5")
    );
    assert_eq!(
        step(&report, DoctorStepId::ReadState).status,
        DoctorStepStatus::Pass
    );
    assert_eq!(
        step(&report, DoctorStepId::SumupRead).status,
        DoctorStepStatus::Pass
    );
    assert_eq!(
        step(&report, DoctorStepId::Tcp48898).skip_reason,
        Some(DoctorSkipReason::ActiveDevice)
    );
    assert_eq!(report.overall, DoctorOverall::Partial);
}

#[test]
fn active_device_shortcut_requires_the_exact_requested_endpoint() {
    let cases = [
        ("192.168.10.99", "5.23.91.12.1.1", 851),
        ("192.168.10.5", "5.23.91.12.9.9", 851),
        ("192.168.10.5", "5.23.91.12.1.1", 852),
    ];

    for (target_ip, ams_net_id, ams_port) in cases {
        let mut options = DoctorOptions::runtime_host(target_ip, local_identity())
            .with_expected_target_ams_net_id(ams_net_id)
            .with_active_device(
                active_device_snapshot(),
                ActiveDeviceStrategy::ReadOnlyViaLiveStatus,
            );
        options.ams_port = ams_port;

        let report = run_doctor(&mut NoCallWire, options, &DoctorCancellation::new());

        assert_eq!(report.overall, DoctorOverall::Fail);
        assert_eq!(
            step(&report, DoctorStepId::UdpIdentify).status,
            DoctorStepStatus::Fail
        );
        assert!(report.summary.contains("does not match"));
    }

    let mut malformed_active = active_device_snapshot();
    malformed_active.target.ams_net_id = "not-an-ams-net-id".to_string();
    let report = run_doctor(
        &mut NoCallWire,
        DoctorOptions::runtime_host("192.168.10.5", local_identity()).with_active_device(
            malformed_active,
            ActiveDeviceStrategy::ReadOnlyViaLiveStatus,
        ),
        &DoctorCancellation::new(),
    );
    assert_eq!(report.overall, DoctorOverall::Fail);
    assert_eq!(
        step(&report, DoctorStepId::UdpIdentify).status,
        DoctorStepStatus::Fail
    );
}

#[test]
fn active_device_batch_read_requires_a_timestamped_good_point() {
    let mut snapshots = Vec::new();

    let mut empty = active_device_snapshot();
    empty.point_statuses.clear();
    snapshots.push(empty);

    let mut missing_timestamp = active_device_snapshot();
    missing_timestamp.point_statuses = vec![PointStatus {
        point_name: "line1_temp".to_string(),
        quality: PointQuality {
            state: trust_ads_core::QualityState::Good,
            last_update_ms: None,
            detail: None,
        },
    }];
    snapshots.push(missing_timestamp);

    let mut degraded = active_device_snapshot();
    degraded.point_statuses.push(PointStatus {
        point_name: "line1_pressure".to_string(),
        quality: PointQuality::stale("waiting for first sample"),
    });
    snapshots.push(degraded);

    for active in snapshots {
        let report = run_doctor(
            &mut NoCallWire,
            DoctorOptions::runtime_host("192.168.10.5", local_identity())
                .with_active_device(active, ActiveDeviceStrategy::ReadOnlyViaLiveStatus),
            &DoctorCancellation::new(),
        );

        assert_eq!(
            step(&report, DoctorStepId::SumupRead).status,
            DoctorStepStatus::Warn
        );
    }
}

#[test]
fn full_doctor_against_active_device_requires_explicit_pause() {
    let mut wire = NoCallWire;
    let report = run_doctor(
        &mut wire,
        DoctorOptions::runtime_host("192.168.10.5", local_identity()).with_active_device(
            active_device_snapshot(),
            ActiveDeviceStrategy::RequiresPause,
        ),
        &DoctorCancellation::new(),
    );

    assert!(report.summary.contains("requires an explicit pause"));
    assert!(report
        .steps
        .iter()
        .all(|step| step.skip_reason == Some(DoctorSkipReason::ActiveDevice)));
    assert!(report
        .steps
        .iter()
        .all(|step| step.next_action.kind == NextActionKind::RerunDoctor));
}

#[test]
fn mock_failure_scenarios_cover_named_field_failures() {
    assert_wire_failure(
        MockAdsOnboardingScenario::WrongIp,
        OnboardingWireErrorKind::UdpIdentifyBlocked,
        |wire, _target, _local| wire.udp_identify("192.168.10.5").map(drop),
    );
    assert_wire_failure(
        MockAdsOnboardingScenario::FirewallBlocked,
        OnboardingWireErrorKind::Tcp48898Blocked,
        |wire, target, _local| wire.tcp_probe_48898(&target.ip),
    );
    assert_wire_failure(
        MockAdsOnboardingScenario::MissingRoute,
        OnboardingWireErrorKind::RouteMissing,
        |wire, target, local| wire.check_route(target, local),
    );
    assert_wire_failure(
        MockAdsOnboardingScenario::WrongAmsNetId,
        OnboardingWireErrorKind::WrongAmsNetId,
        |wire, target, _local| wire.verify_ams_target(target),
    );
    assert_wire_failure(
        MockAdsOnboardingScenario::WrongPlcPort,
        OnboardingWireErrorKind::WrongPlcPort,
        |wire, target, _local| wire.read_state(target).map(drop),
    );
    assert_wire_failure(
        MockAdsOnboardingScenario::SecureRequired,
        OnboardingWireErrorKind::SecureRequired,
        |wire, target, _local| wire.read_state(target).map(drop),
    );
    assert_wire_failure(
        MockAdsOnboardingScenario::NotificationFailure,
        OnboardingWireErrorKind::NotificationFailure,
        |wire, target, _local| wire.subscribe_notification(target, "MAIN.Temperature"),
    );

    let mut empty = MockAdsOnboardingWire::new(MockAdsOnboardingScenario::EmptySymbols);
    let target = target_identity();
    let symbols = empty
        .upload_symbols(&target)
        .expect("symbol upload succeeds");
    assert!(symbols.is_empty());
    assert_eq!(
        OnboardingWireErrorKind::NoSymbols
            .classification()
            .next_action
            .kind,
        NextActionKind::None
    );
}
