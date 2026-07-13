use super::*;
use budget::test_support::*;

fn probe_result(responded: bool) -> Result<LocalRuntimeProbeResult, String> {
    Ok(if responded {
        LocalRuntimeProbeResult::Responded
    } else {
        LocalRuntimeProbeResult::NoResponse
    })
}

#[test]
fn configured_runtime_target_is_probed_instead_of_claiming_router_source() {
    let candidates = vec![windows_runtime::ConfiguredRuntime {
        name: "UmRT_Default".to_string(),
        ams_net_id: "10.20.30.40.1.1".to_string(),
    }];
    let mut probes = Vec::new();

    let report = responding_runtime_targets(
        "127.0.0.1",
        &candidates,
        "10.20.30.41.1.1",
        |net_id, port, _ports_remaining| {
            probes.push((net_id.to_string(), port));
            probe_result(net_id == "10.20.30.40.1.1" && port == 501)
        },
    )
    .expect("configured runtime probe");

    assert_eq!(report.identities.len(), 1);
    assert_eq!(report.identities[0].name.as_deref(), Some("UmRT_Default"));
    assert_eq!(report.identities[0].ams_net_id, "10.20.30.40.1.1");
    assert_eq!(report.identities[0].preferred_ams_port, Some(501));
    assert_eq!(report.identities[0].responding_ads_ports, vec![501]);
    assert!(report.warnings.is_empty());
    assert!(probes.contains(&("10.20.30.40.1.1".to_string(), 501)));
    assert!(probes.iter().any(|(net_id, _)| net_id == "10.20.30.41.1.1"));
    assert!(!probes.contains(&("10.20.30.41.1.1".to_string(), 10_000)));
}

#[test]
fn source_collision_probes_the_documented_second_byte_increment() {
    let configured = vec![windows_runtime::ConfiguredRuntime {
        name: "UmRT_Default".to_string(),
        ams_net_id: "199.4.1.1.1.1".to_string(),
    }];

    let report = responding_runtime_targets(
        "127.0.0.1",
        &configured,
        "199.4.1.1.1.1",
        |net_id, port, _ports_remaining| {
            probe_result(
                (net_id == "199.4.1.1.1.1" && port == 851)
                    || (net_id == "199.5.1.1.1.1" && port == 301),
            )
        },
    )
    .expect("collision probe");

    assert_eq!(
        report
            .identities
            .iter()
            .map(|target| target.ams_net_id.as_str())
            .collect::<Vec<_>>(),
        vec!["199.4.1.1.1.1", "199.5.1.1.1.1"]
    );
    assert!(report.identities[1]
        .name
        .as_deref()
        .is_some_and(|name| name.starts_with("Local ADS runtime")));
    assert!(report
        .warnings
        .iter()
        .any(|warning| warning.contains("collide")));
}

#[test]
fn duplicate_configured_ids_return_only_native_proven_effective_ids() {
    let configured = vec![
        windows_runtime::ConfiguredRuntime {
            name: "UmRT_Default".to_string(),
            ams_net_id: "10.20.30.40.1.1".to_string(),
        },
        windows_runtime::ConfiguredRuntime {
            name: "UmRT_Test".to_string(),
            ams_net_id: "10.20.30.40.1.1".to_string(),
        },
    ];

    let report = responding_runtime_targets(
        "127.0.0.1",
        &configured,
        "10.20.30.41.1.1",
        |net_id, port, _ports_remaining| {
            probe_result(
                (net_id == "10.20.30.40.1.1" && port == 301)
                    || (net_id == "10.21.30.40.1.1" && port == 501),
            )
        },
    )
    .expect("duplicate configured IDs");

    assert_eq!(
        report
            .identities
            .iter()
            .map(|target| target.ams_net_id.as_str())
            .collect::<Vec<_>>(),
        vec!["10.20.30.40.1.1", "10.21.30.40.1.1"]
    );
    assert!(report
        .identities
        .iter()
        .all(|target| target.name.as_deref() != Some("UmRT_Default")
            && target.name.as_deref() != Some("UmRT_Test")));
}

#[test]
fn unresolved_collision_warns_and_never_returns_unproven_increment() {
    let configured = vec![
        windows_runtime::ConfiguredRuntime {
            name: "UmRT_Default".to_string(),
            ams_net_id: "10.20.30.40.1.1".to_string(),
        },
        windows_runtime::ConfiguredRuntime {
            name: "UmRT_Test".to_string(),
            ams_net_id: "10.20.30.40.1.1".to_string(),
        },
    ];
    let mut probes = Vec::new();

    let report = responding_runtime_targets(
        "127.0.0.1",
        &configured,
        "10.20.30.41.1.1",
        |net_id, port, _ports_remaining| {
            probes.push((net_id.to_string(), port));
            probe_result(net_id == "10.20.30.40.1.1" && port == 301)
        },
    )
    .expect("unresolved collision probe");

    assert_eq!(report.identities.len(), 1);
    assert_eq!(report.identities[0].ams_net_id, "10.20.30.40.1.1");
    assert!(report
        .identities
        .iter()
        .all(|target| target.ams_net_id != "10.21.30.40.1.1"));
    assert!(report
        .warnings
        .iter()
        .any(|warning| warning.contains("Verified 1 of 2")));
    assert!(report
        .warnings
        .iter()
        .any(|warning| warning
            .contains("1 instance(s) could not be matched to a responding AMS Net ID")));
    assert!(probes.iter().any(|(net_id, _)| net_id == "10.36.30.40.1.1"));
    assert!(!probes.iter().any(|(net_id, _)| net_id == "10.37.30.40.1.1"));
}

#[test]
fn slow_source_probe_cannot_starve_a_valid_configured_runtime() {
    let configured = vec![windows_runtime::ConfiguredRuntime {
        name: "UmRT_Default".to_string(),
        ams_net_id: "10.20.30.40.1.1".to_string(),
    }];
    let mut probes = Vec::new();

    let report = responding_runtime_targets(
        "127.0.0.1",
        &configured,
        "10.20.30.41.1.1",
        |net_id, port, _ports_remaining| {
            probes.push((net_id.to_string(), port));
            if net_id == "10.20.30.40.1.1" && port == 851 {
                return Ok(LocalRuntimeProbeResult::Responded);
            }
            if net_id == "10.20.30.41.1.1" {
                return Ok(LocalRuntimeProbeResult::DeadlineReached);
            }
            Ok(LocalRuntimeProbeResult::NoResponse)
        },
    )
    .expect("configured runtime survives slow source fallback");

    assert!(report.deadline_reached);
    assert_eq!(report.identities.len(), 1);
    assert_eq!(report.identities[0].ams_net_id, "10.20.30.40.1.1");
    assert_eq!(report.identities[0].preferred_ams_port, Some(851));
    assert_eq!(
        probes.first(),
        Some(&("10.20.30.40.1.1".to_string(), 10_000))
    );
    assert!(probes.iter().any(|(net_id, _)| net_id == "10.20.30.41.1.1"));
}

#[test]
fn native_0x507_continues_to_plc_851_for_the_configured_laptop_identity() {
    let clock = FakeClock::new();
    let mut probe = ScriptedNativeProbe::new(&clock)
        .reply(
            "10.20.30.40.1.1",
            10_000,
            error_reply(
                0x507,
                "ADS router port is not registered",
                ScriptedDelay::Fixed(Duration::ZERO),
            ),
        )
        .reply(
            "10.20.30.40.1.1",
            851,
            running_reply(ScriptedDelay::Fixed(Duration::ZERO)),
        );

    let report = scan_with_script(
        &clock,
        &mut probe,
        Duration::from_millis(900),
        &[configured_runtime()],
        "10.20.30.41.1.1",
    )
    .expect("0x507 is an absent logical port, not a terminal router failure");

    assert_eq!(report.identities.len(), 1);
    assert_eq!(report.identities[0].preferred_ams_port, Some(851));
    assert_eq!(report.identities[0].ams_net_id, "10.20.30.40.1.1");
    assert_eq!(
        probe
            .calls
            .iter()
            .take(2)
            .map(|call| (call.net_id.as_str(), call.port))
            .collect::<Vec<_>>(),
        vec![("10.20.30.40.1.1", 10_000), ("10.20.30.40.1.1", 851)]
    );
}

#[test]
fn fair_native_budget_reaches_301_and_501_and_never_overstates_remaining_time() {
    let clock = FakeClock::new();
    let timeout = Duration::from_millis(900);
    let mut probe =
        ScriptedNativeProbe::new(&clock).with_set_timeout_delay(Duration::from_millis(1));
    for port in [10_000, 851, 852, 853, 854, 301] {
        probe = probe.reply(
            "10.20.30.40.1.1",
            port,
            error_reply(
                0x745,
                "remote terminal did not answer",
                ScriptedDelay::AppliedTimeout,
            ),
        );
    }
    probe = probe.reply(
        "10.20.30.40.1.1",
        501,
        running_reply(ScriptedDelay::Fixed(Duration::ZERO)),
    );

    let report = scan_with_script(
        &clock,
        &mut probe,
        timeout,
        &[configured_runtime()],
        "10.20.30.41.1.1",
    )
    .expect("fair local scan");

    assert_eq!(report.identities.len(), 1);
    assert_eq!(report.identities[0].preferred_ams_port, Some(501));
    let configured_ports = probe
        .calls
        .iter()
        .filter(|call| call.net_id == "10.20.30.40.1.1")
        .map(|call| call.port)
        .collect::<Vec<_>>();
    assert_eq!(configured_ports, LOCAL_RUNTIME_PROBE_PORTS);
    for call in &probe.calls {
        assert!(
            call.started_after < timeout,
            "probe started after deadline: {call:?}"
        );
        assert!(
            call.timeout <= timeout.saturating_sub(call.started_after),
            "applied timeout exceeded remaining budget: {call:?}"
        );
    }
}

#[test]
fn global_native_budget_reaches_a_later_collision_adjusted_runtime() {
    let clock = FakeClock::new();
    let timeout = Duration::from_millis(900);
    let mut probe = ScriptedNativeProbe::new(&clock);
    for (net_id, ports) in [
        ("10.20.30.40.1.1", LOCAL_RUNTIME_PROBE_PORTS.as_slice()),
        ("10.20.30.41.1.1", LOCAL_USER_SERVICE_PORTS.as_slice()),
        ("10.21.30.40.1.1", LOCAL_RUNTIME_PROBE_PORTS.as_slice()),
    ] {
        for port in ports {
            probe = probe.reply(
                net_id,
                *port,
                error_reply(
                    0x745,
                    "candidate did not answer",
                    ScriptedDelay::AppliedTimeout,
                ),
            );
        }
    }
    probe = probe.reply(
        "10.22.30.40.1.1",
        501,
        running_reply(ScriptedDelay::Fixed(Duration::ZERO)),
    );

    let report = scan_with_script(
        &clock,
        &mut probe,
        timeout,
        &[configured_runtime()],
        "10.20.30.41.1.1",
    )
    .expect("global fairness must reserve time for later collision candidates");

    let adjusted = report
        .identities
        .iter()
        .find(|identity| identity.ams_net_id == "10.22.30.40.1.1")
        .expect("second collision-adjusted identity was natively proven");
    assert_eq!(adjusted.responding_ads_ports, vec![501]);
    assert!(probe
        .calls
        .iter()
        .any(|call| call.net_id == "10.22.30.40.1.1" && call.port == 501));
    assert!(probe.calls.iter().all(|call| call.started_after < timeout));
}

#[test]
fn system_identity_port_is_never_fabricated_as_a_user_service() {
    let configured = vec![configured_runtime()];

    let report = responding_runtime_targets(
        "127.0.0.1",
        &configured,
        "10.20.30.41.1.1",
        |net_id, port, _ports_remaining| {
            probe_result(net_id == "10.20.30.40.1.1" && port == 10_000)
        },
    )
    .expect("identity-only response is non-fatal discovery evidence");

    assert_eq!(report.identities.len(), 1);
    assert_eq!(report.identities[0].preferred_ams_port, None);
    assert!(report.identities[0].responding_ads_ports.is_empty());
    assert!(report.warnings.iter().any(|warning| {
        warning.contains("system port 10000") && warning.contains("no connectable user ADS service")
    }));
}

#[test]
fn every_responding_user_port_is_preserved_for_one_ams_identity() {
    let configured = vec![configured_runtime()];

    let report = responding_runtime_targets(
        "127.0.0.1",
        &configured,
        "10.20.30.41.1.1",
        |net_id, port, _ports_remaining| {
            probe_result(net_id == "10.20.30.40.1.1" && matches!(port, 851 | 301 | 501))
        },
    )
    .expect("multi-port response");

    assert_eq!(
        report.identities[0].responding_ads_ports,
        vec![851, 301, 501]
    );
    assert_eq!(report.identities[0].preferred_ams_port, Some(851));
    assert!(!report.identities[0].responding_ads_ports.contains(&10_000));
}

#[test]
fn late_native_success_is_excluded_end_to_end_and_stops_new_calls() {
    let clock = FakeClock::new();
    let mut probe = ScriptedNativeProbe::new(&clock).reply(
        "10.20.30.40.1.1",
        10_000,
        running_reply(ScriptedDelay::Fixed(Duration::from_millis(901))),
    );

    let report = scan_with_script(
        &clock,
        &mut probe,
        Duration::from_millis(900),
        &[configured_runtime()],
        "10.20.30.41.1.1",
    )
    .expect("late reply becomes a bounded incomplete scan");

    assert!(report.identities.is_empty());
    assert!(report.deadline_reached);
    assert_eq!(probe.calls.len(), 1);

    let clock = FakeClock::new();
    let mut expired_during_setup =
        ScriptedNativeProbe::new(&clock).with_set_timeout_delay(Duration::from_millis(901));
    let report = scan_with_script(
        &clock,
        &mut expired_during_setup,
        Duration::from_millis(900),
        &[configured_runtime()],
        "10.20.30.41.1.1",
    )
    .expect("expired timeout application prevents a native read-state call");
    assert!(report.deadline_reached);
    assert!(expired_during_setup.calls.is_empty());
}

#[test]
fn fatal_native_error_preserves_prior_proof_but_fails_without_any_proof() {
    let clock = FakeClock::new();
    let fatal = error_reply(
        0x50A,
        "local ADS router is not active",
        ScriptedDelay::Fixed(Duration::ZERO),
    );
    let mut after_proof = ScriptedNativeProbe::new(&clock)
        .reply(
            "10.20.30.40.1.1",
            851,
            running_reply(ScriptedDelay::Fixed(Duration::ZERO)),
        )
        .reply("10.20.30.41.1.1", 851, fatal.clone());
    let report = scan_with_script(
        &clock,
        &mut after_proof,
        Duration::from_millis(900),
        &[configured_runtime()],
        "10.20.30.41.1.1",
    )
    .expect("verified target survives later native failure");
    assert_eq!(report.identities.len(), 1);
    assert!(report
        .warnings
        .iter()
        .any(|warning| warning.contains("Previously verified devices are still shown")));

    let clock = FakeClock::new();
    let mut before_proof = ScriptedNativeProbe::new(&clock).reply("10.20.30.40.1.1", 10_000, fatal);
    let error = scan_with_script(
        &clock,
        &mut before_proof,
        Duration::from_millis(900),
        &[configured_runtime()],
        "10.20.30.41.1.1",
    )
    .expect_err("fatal native failure before proof must remain terminal");
    assert!(error.contains("router is not active"), "{error}");
}

#[test]
fn local_runtime_probe_budget_uses_remaining_time_and_refuses_short_window() {
    let clock = FakeClock::new();
    let started = clock.now();
    let budget = LocalRuntimeProbeBudget::new_at(&clock, started, Duration::from_millis(900));

    assert_eq!(
        budget.remaining_probe_timeout_at(started, 7),
        Some(Duration::from_millis(112))
    );
    assert_eq!(
        budget.remaining_probe_timeout_at(started + Duration::from_millis(800), 1),
        Some(Duration::from_millis(50))
    );
    assert_eq!(
        budget.remaining_probe_timeout_at(started + Duration::from_millis(898), 1),
        Some(Duration::from_millis(1))
    );
    assert_eq!(
        budget.remaining_probe_timeout_at(started + Duration::from_millis(899), 1),
        Some(Duration::from_millis(1))
    );
    assert_eq!(
        budget.remaining_probe_timeout_at(started + Duration::from_millis(900), 1),
        None
    );
    assert!(budget.completion_is_within_deadline_at(started + Duration::from_millis(900)));
    assert!(!budget.completion_is_within_deadline_at(started + Duration::from_millis(901)));

    let mut live_budget = LocalRuntimeProbeBudget::new(&clock, Duration::from_secs(1));
    assert_eq!(
        live_budget.remaining_probe_timeout(1),
        Some(Duration::from_millis(250))
    );
    live_budget.record_probe_started();
    assert_eq!(live_budget.probes_started(), 1);
    assert!(live_budget.completion_is_within_deadline());
}

#[test]
fn large_native_probe_plan_still_starts_with_a_bounded_slice() {
    let bases = (0..64)
        .map(|index| format!("10.20.30.{index}.1.1"))
        .collect::<Vec<_>>();
    let slots = planned_runtime_probe_slots(&bases, "10.20.31.1.1.1");
    assert!(slots > 64 * LOCAL_RUNTIME_PROBE_PORTS.len());

    let clock = FakeClock::new();
    let started = clock.now();
    let budget = LocalRuntimeProbeBudget::new_at(&clock, started, Duration::from_millis(900));
    assert_eq!(
        budget.remaining_probe_timeout_at(started, slots),
        Some(Duration::from_millis(1))
    );
}

#[test]
fn native_success_completed_after_deadline_is_rejected() {
    let result = classify_native_runtime_probe(
        Ok(trust_ads_windows::AdsDeviceState {
            ads_state: 5,
            device_state: 0,
        }),
        false,
    )
    .expect("late success is a deadline outcome");

    assert_eq!(result, LocalRuntimeProbeResult::DeadlineReached);
}

#[test]
fn native_probe_error_classification_separates_absence_reply_and_router_failure() {
    let native_error = |code, description| trust_ads_windows::AdsError::Call {
        operation: "AdsSyncReadStateReqEx",
        code,
        description,
    };

    for code in [0x006, 0x007, 0x015, 0x01B, 0x507, 0x745] {
        assert_eq!(
            classify_native_runtime_probe(
                Err(native_error(code, "candidate did not answer")),
                true
            ),
            Ok(LocalRuntimeProbeResult::NoResponse),
            "ADS error 0x{code:03X}"
        );
    }
    assert_eq!(
        classify_native_runtime_probe(
            Err(native_error(0x701, "target replied: service unsupported")),
            true
        ),
        Ok(LocalRuntimeProbeResult::Responded)
    );
    let fatal = classify_native_runtime_probe(
        Err(native_error(0x50A, "local ADS router is not active")),
        true,
    )
    .expect_err("router failure must not become device absence");
    assert!(fatal.contains("router is not active"), "{fatal}");
}

#[test]
fn collision_increment_is_overflow_safe() {
    assert_eq!(
        increment_second_octet("1.254.3.4.5.6", 1).as_deref(),
        Some("1.255.3.4.5.6")
    );
    assert!(increment_second_octet("1.255.3.4.5.6", 1).is_none());
}
