use std::net::IpAddr;

use super::*;
use crate::ads::identity::is_canonical_ams_net_id;

pub(super) fn validate_manual_target(options: &DoctorOptions) -> Option<OnboardingWireError> {
    if options.target_ip.parse::<IpAddr>().is_err() {
        return Some(OnboardingWireError::new(
            OnboardingWireErrorKind::UdpIdentifyBlocked,
            format!("invalid ADS target IP '{}'", options.target_ip),
        ));
    }
    if options.ams_port == 0 {
        return Some(OnboardingWireError::new(
            OnboardingWireErrorKind::WrongPlcPort,
            "ADS target AMS port must be non-zero",
        ));
    }
    if let Some(expected) = options.expected_target_ams_net_id.as_deref() {
        if !is_canonical_ams_net_id(expected) {
            return Some(OnboardingWireError::new(
                OnboardingWireErrorKind::WrongAmsNetId,
                format!(
                    "invalid target AMS Net ID '{expected}'; expected six canonical decimal octets"
                ),
            ));
        }
    }
    None
}

pub(super) fn validate_active_target(
    options: &DoctorOptions,
    active: &ActiveAdsDeviceSnapshot,
) -> Option<OnboardingWireError> {
    let requested_ip = options.target_ip.parse::<IpAddr>().ok()?;
    let active_ip = active.target.ip.parse::<IpAddr>().ok();
    if active_ip != Some(requested_ip) {
        return Some(active_target_mismatch(
            options,
            active,
            OnboardingWireErrorKind::UdpIdentifyBlocked,
        ));
    }
    if active.target.ams_port != options.ams_port {
        return Some(active_target_mismatch(
            options,
            active,
            OnboardingWireErrorKind::WrongPlcPort,
        ));
    }
    if !is_canonical_ams_net_id(active.target.ams_net_id.as_str())
        || options
            .expected_target_ams_net_id
            .as_deref()
            .is_some_and(|expected| expected != active.target.ams_net_id)
    {
        return Some(active_target_mismatch(
            options,
            active,
            OnboardingWireErrorKind::WrongAmsNetId,
        ));
    }
    None
}

fn active_target_mismatch(
    options: &DoctorOptions,
    active: &ActiveAdsDeviceSnapshot,
    kind: OnboardingWireErrorKind,
) -> OnboardingWireError {
    OnboardingWireError::new(
        kind,
        format!(
            "active ADS device endpoint {}:{} ({}) does not match requested endpoint {}:{}{}",
            active.target.ip,
            active.target.ams_port,
            active.target.ams_net_id,
            options.target_ip,
            options.ams_port,
            options
                .expected_target_ams_net_id
                .as_deref()
                .map_or_else(String::new, |ams_net_id| format!(" ({ams_net_id})")),
        ),
    )
}

pub(super) fn rejected_target_report(
    options: DoctorOptions,
    active: Option<ActiveAdsDeviceSnapshot>,
    error: OnboardingWireError,
) -> DoctorReport {
    let mut steps = Vec::with_capacity(REQUIRED_DOCTOR_STEPS.len());
    steps.push(failed_step(
        DoctorStepId::UdpIdentify,
        step_title(DoctorStepId::UdpIdentify),
        error,
    ));
    steps.extend(REQUIRED_DOCTOR_STEPS[1..].iter().copied().map(blocked_step));

    let mut report = DoctorReport::new(options.ran_from, options.transport)
        .with_steps(steps)
        .with_summary("ADS Doctor target does not match a valid requested endpoint.");
    report.writes_enabled = options.writes_enabled;
    if let Some(active) = active {
        report = report.with_target(active.target);
        if let Some(local) = active.local.or(options.local_identity) {
            report = report.with_local(local);
        }
    } else if let Some(local) = options.local_identity {
        report = report.with_local(local);
    }
    report
}
