use super::*;

fn target(ip: &str, ams_net_id: &str, ams_port: u16) -> TargetIdentity {
    TargetIdentity {
        name: None,
        ip: ip.to_string(),
        ams_net_id: ams_net_id.to_string(),
        ams_port,
        tc_version: None,
    }
}

fn wire_error(detail: &str) -> OnboardingWireError {
    OnboardingWireError::new(OnboardingWireErrorKind::WrongPlcPort, detail)
}

#[test]
fn udp_wire_deduplicates_by_ads_endpoint_not_host_ip() {
    let mut results = vec![target("192.168.10.5", "1.2.3.4.5.6", 851)];

    push_unique_target_identity(&mut results, target("192.168.10.5", "1.2.3.4.5.7", 851));
    assert_eq!(results.len(), 2, "distinct same-host runtime was lost");

    push_unique_target_identity(&mut results, target("192.168.10.99", "1.2.3.4.5.7", 851));
    assert_eq!(results.len(), 2, "same ADS endpoint was duplicated");

    push_unique_target_identity(&mut results, target("192.168.10.5", "1.2.3.4.5.7", 852));
    assert_eq!(results.len(), 3, "distinct AMS port was lost");
}

#[test]
fn guarded_probe_write_failure_attempts_and_reports_restore_failure() {
    let mut restore_calls = 0;
    let error = require_probe_write(Err(wire_error("probe write failed")), || {
        restore_calls += 1;
        Err(wire_error("restore write failed"))
    })
    .expect_err("probe write must fail");

    assert_eq!(restore_calls, 1);
    assert!(error.detail.contains("probe write failed"));
    assert!(error.detail.contains("restore write failed"));
}

#[test]
fn guarded_probe_read_error_attempts_and_reports_restore_failure() {
    let mut restore_calls = 0;
    let error = require_expected_probe_readback(
        Err(wire_error("probe read-back failed")),
        &Value::Real(2.0),
        "MAIN.Setpoint",
        || {
            restore_calls += 1;
            Err(wire_error("restore verification failed"))
        },
    )
    .expect_err("probe read-back must fail");

    assert_eq!(restore_calls, 1);
    assert!(error.detail.contains("probe read-back failed"));
    assert!(error.detail.contains("restore verification failed"));
}

#[test]
fn guarded_probe_mismatch_reports_restore_failure() {
    let error = require_expected_probe_readback(
        Ok(Value::Real(3.0)),
        &Value::Real(2.0),
        "MAIN.Setpoint",
        || Err(wire_error("restore verification failed")),
    )
    .expect_err("probe mismatch must fail");

    assert!(error.detail.contains("read-back mismatch"));
    assert!(error.detail.contains("restore verification failed"));
}

#[test]
fn guarded_probe_matching_readback_defers_normal_restore() {
    let mut restore_calls = 0;
    require_expected_probe_readback(
        Ok(Value::Real(2.0)),
        &Value::Real(2.0),
        "MAIN.Setpoint",
        || {
            restore_calls += 1;
            Ok(())
        },
    )
    .expect("matching probe read-back");

    assert_eq!(restore_calls, 0);
}

#[test]
fn sumup_read_projection_rejects_incomplete_or_untrusted_results() {
    let target = target("192.168.10.5", "1.2.3.4.5.6", 851);
    let mut wire = AdsRsOnboardingWire::new();
    let unknown_handle = wire
        .sumup_read(&target, &[99])
        .expect_err("an unknown handle must fail before wire values exist");
    assert!(unknown_handle.detail.contains("not resolved"));

    let handle = AdsResolvedHandle {
        point_name: "MAIN.Temperature".to_string(),
        address: AdsPointAddress::Symbol("MAIN.Temperature".to_string()),
        data_type: AdsDataTypeDescriptor::scalar("REAL", IecDataType::Real),
        handle: 42,
    };
    let good = AdsReadResult {
        point_name: handle.point_name.clone(),
        value: Some(Value::Real(12.5)),
        quality: trust_ads_core::PointQuality::good(1),
    };
    let cases = [
        ("missing result", Vec::new()),
        ("duplicate result", vec![good.clone(), good.clone()]),
        (
            "descriptor mismatch",
            vec![AdsReadResult {
                value: Some(Value::Bool(true)),
                ..good.clone()
            }],
        ),
        (
            "non-good quality",
            vec![AdsReadResult {
                quality: trust_ads_core::PointQuality::error(1, "read failed"),
                ..good.clone()
            }],
        ),
        (
            "missing value",
            vec![AdsReadResult {
                value: None,
                ..good.clone()
            }],
        ),
        (
            "unknown returned point",
            vec![AdsReadResult {
                point_name: "MAIN.Unknown".to_string(),
                ..good
            }],
        ),
    ];

    for (case, results) in cases {
        assert!(
            project_sumup_payloads(std::slice::from_ref(&handle), results).is_err(),
            "{case} manufactured a wire value"
        );
    }
}
