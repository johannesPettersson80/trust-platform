use serde_json::json;

use crate::ads::diagnostics::{
    AdsConnectionStatusState, DoctorReport, DoctorSkipReason, DoctorStep, DoctorStepId,
    DoctorStepStatus, NextAction, NextActionKind,
};

use super::{pass_step, step_title, ActiveAdsDeviceSnapshot, DoctorOptions, REQUIRED_DOCTOR_STEPS};

pub(super) fn read_only_report(
    options: &DoctorOptions,
    active: &ActiveAdsDeviceSnapshot,
) -> DoctorReport {
    let degraded_points = active.degraded_points();
    let mut steps = Vec::with_capacity(REQUIRED_DOCTOR_STEPS.len());
    for step_id in REQUIRED_DOCTOR_STEPS {
        let step = match step_id {
            DoctorStepId::LocalIdentity => {
                if let Some(local) = active.local.as_ref().or(options.local_identity.as_ref()) {
                    pass_step(
                        *step_id,
                        "truST local identity",
                        format!("Using live runtime identity {}.", local.ams_net_id),
                    )
                    .with_evidence("local_ip", local.chosen_ip.clone())
                    .with_evidence("local_ams_net_id", local.ams_net_id.clone())
                } else {
                    active_skip(*step_id, "truST local identity", &active.connection_name)
                }
            }
            DoctorStepId::ReadState => active_read_state_step(active),
            DoctorStepId::SumupRead => active_sumup_step(active, degraded_points),
            DoctorStepId::SymbolVersion => {
                let mut step = active_skip(*step_id, "Symbol version", &active.connection_name);
                if let Some(version) = active.symbol_version {
                    step = DoctorStep::new(
                        *step_id,
                        "Symbol version",
                        DoctorStepStatus::Pass,
                        format!("Live worker reports symbol version {version}."),
                    )
                    .with_evidence("symbol_version", json!(version));
                }
                step
            }
            _ => active_skip(*step_id, step_title(*step_id), &active.connection_name),
        };
        steps.push(step);
    }

    let local = active
        .local
        .clone()
        .or_else(|| options.local_identity.clone());
    let mut report = DoctorReport::new(options.ran_from, options.transport)
        .with_target(active.target.clone())
        .with_steps(steps)
        .with_summary(format!(
            "Active ADS device '{}': using live runtime status; no duplicate AMS connection opened.",
            active.connection_name
        ));
    report.writes_enabled = options.writes_enabled;
    if let Some(local) = local {
        report = report.with_local(local);
    }
    report
}

pub(super) fn requires_pause_report(
    options: &DoctorOptions,
    active: &ActiveAdsDeviceSnapshot,
) -> DoctorReport {
    let steps = REQUIRED_DOCTOR_STEPS
        .iter()
        .map(|step_id| {
            active_skip(*step_id, step_title(*step_id), &active.connection_name)
                .with_remediation("Pause this ADS device before running the full doctor.")
                .with_next_action(NextAction::new(NextActionKind::RerunDoctor))
        })
        .collect();
    let mut report = DoctorReport::new(options.ran_from, options.transport)
        .with_target(active.target.clone())
        .with_steps(steps)
        .with_summary(format!(
            "Full doctor requires an explicit pause for active ADS device '{}'.",
            active.connection_name
        ));
    report.writes_enabled = options.writes_enabled;
    if let Some(local) = active
        .local
        .clone()
        .or_else(|| options.local_identity.clone())
    {
        report = report.with_local(local);
    }
    report
}

fn active_read_state_step(active: &ActiveAdsDeviceSnapshot) -> DoctorStep {
    match active.state {
        AdsConnectionStatusState::Connected => DoctorStep::new(
            DoctorStepId::ReadState,
            "PLC runtime state",
            DoctorStepStatus::Pass,
            "Live ADS worker is connected.",
        )
        .with_evidence("connection_state", "connected"),
        AdsConnectionStatusState::Reconnecting
        | AdsConnectionStatusState::NotReady
        | AdsConnectionStatusState::Stale => DoctorStep::new(
            DoctorStepId::ReadState,
            "PLC runtime state",
            DoctorStepStatus::Warn,
            format!("Live ADS worker is {:?}.", active.state),
        )
        .with_evidence("connection_state", format!("{:?}", active.state)),
        AdsConnectionStatusState::Faulted => {
            let mut step = DoctorStep::new(
                DoctorStepId::ReadState,
                "PLC runtime state",
                DoctorStepStatus::Fail,
                "Live ADS worker is faulted.",
            )
            .with_evidence("connection_state", "faulted");
            step.remediation = "Inspect ADS device status before rerunning the doctor.".to_string();
            step.next_action = NextAction::new(NextActionKind::RerunDoctor);
            step
        }
        AdsConnectionStatusState::Disabled | AdsConnectionStatusState::Unknown => DoctorStep::new(
            DoctorStepId::ReadState,
            "PLC runtime state",
            DoctorStepStatus::Warn,
            format!("Live ADS worker state is {:?}.", active.state),
        )
        .with_evidence("connection_state", format!("{:?}", active.state)),
    }
}

fn active_sumup_step(active: &ActiveAdsDeviceSnapshot, degraded_points: usize) -> DoctorStep {
    let point_count = active.point_statuses.len();
    let status = if degraded_points == 0 {
        DoctorStepStatus::Pass
    } else {
        DoctorStepStatus::Warn
    };
    DoctorStep::new(
        DoctorStepId::SumupRead,
        "Batch read",
        status,
        format!("{point_count} live point(s), {degraded_points} degraded."),
    )
    .with_evidence("point_count", json!(point_count))
    .with_evidence("degraded_points", json!(degraded_points))
    .with_evidence(
        "last_good_value_ms",
        active
            .last_good_value_ms()
            .map_or(json!(null), |value| json!(value)),
    )
}

fn active_skip(id: DoctorStepId, title: impl Into<String>, connection_name: &str) -> DoctorStep {
    DoctorStep::skipped(
        id,
        title,
        DoctorSkipReason::ActiveDevice,
        format!(
            "Skipped direct ADS probe because '{connection_name}' is already connected by the runtime."
        ),
    )
}
