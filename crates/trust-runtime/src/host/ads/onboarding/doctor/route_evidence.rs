use crate::ads::diagnostics::{DoctorStep, DoctorStepId, LocalIdentity, TargetIdentity};

use super::super::errors::{
    reply_failure_implies_missing_return_route, OnboardingWireError, OnboardingWireErrorKind,
};
use super::super::wire::{
    AdsOnboardingWire, AdsRouteRequirement, AdsRouterProbe, NativeAmsSourceAddress,
};
use super::{failed_step, pass_step};

pub(super) struct RouteRoundTripEvidence {
    pub(super) step: DoctorStep,
    pub(super) read_state: Option<Result<String, OnboardingWireError>>,
}

pub(super) fn transport_step<W: AdsOnboardingWire>(
    wire: &mut W,
    target: &TargetIdentity,
) -> DoctorStep {
    match wire.probe_ads_router(target) {
        Ok(AdsRouterProbe::Tcp48898) => pass_step(
            DoctorStepId::Tcp48898,
            "ADS transport reachable",
            "The remote ADS router accepted a TCP 48898 connection.",
        )
        .with_evidence("target_ip", target.ip.clone())
        .with_evidence("probe_transport", "tcp_48898"),
        Ok(AdsRouterProbe::NativeRouterRoundTrip) => with_native_address_evidence(
            pass_step(
                DoctorStepId::Tcp48898,
                "ADS transport reachable",
                "The local Windows ADS router completed a native target read-state round trip.",
            )
            .with_evidence("target_ip", target.ip.clone())
            .with_evidence("probe_transport", "native_windows_router")
            .with_evidence("probe_operation", "read_state"),
            wire.native_source_address(),
            target,
        ),
        Err(error) => failed_step(DoctorStepId::Tcp48898, "ADS transport reachable", error),
    }
}

pub(super) fn route_round_trip_step<W: AdsOnboardingWire>(
    wire: &mut W,
    target: &TargetIdentity,
    local: &LocalIdentity,
) -> RouteRoundTripEvidence {
    let route_requirement = wire.route_requirement(&target.ip);
    match wire.check_route(target, local) {
        Ok(()) => route_read_state_evidence(wire, target, local, route_requirement),
        Err(error) => RouteRoundTripEvidence {
            step: failed_step(DoctorStepId::RoutePresent, "Route back to truST", error),
            read_state: None,
        },
    }
}

fn route_read_state_evidence<W: AdsOnboardingWire>(
    wire: &mut W,
    target: &TargetIdentity,
    local: &LocalIdentity,
    route_requirement: AdsRouteRequirement,
) -> RouteRoundTripEvidence {
    match wire.read_state(target) {
        Ok(state) => RouteRoundTripEvidence {
            step: with_route_address_evidence(
                passed_round_trip_step(target, local, route_requirement, &state),
                wire,
                target,
                route_requirement,
            ),
            read_state: Some(Ok(state)),
        },
        Err(error)
            if route_requirement == AdsRouteRequirement::ReciprocalRouteRequired
                && reply_failure_implies_missing_return_route(&error) =>
        {
            RouteRoundTripEvidence {
                step: failed_step(
                    DoctorStepId::RoutePresent,
                    "Route back to truST",
                    missing_route_round_trip_error(error),
                ),
                read_state: None,
            }
        }
        Err(error) => RouteRoundTripEvidence {
            step: with_route_address_evidence(
                passed_error_reply_step(target, local, route_requirement, &error),
                wire,
                target,
                route_requirement,
            ),
            read_state: Some(Err(error)),
        },
    }
}

fn with_route_address_evidence<W: AdsOnboardingWire>(
    step: DoctorStep,
    wire: &W,
    target: &TargetIdentity,
    route_requirement: AdsRouteRequirement,
) -> DoctorStep {
    if route_requirement != AdsRouteRequirement::NativeLocalRouter {
        return step;
    }
    with_native_address_evidence(step, wire.native_source_address(), target)
}

fn with_native_address_evidence(
    step: DoctorStep,
    source: Option<NativeAmsSourceAddress>,
    target: &TargetIdentity,
) -> DoctorStep {
    let Some(source) = source else {
        return step
            .with_evidence("source_ams_address_available", false)
            .with_evidence("target_ams_net_id", target.ams_net_id.clone())
            .with_evidence("target_ams_port", target.ams_port);
    };
    let distinct = source.differs_from_target(target);
    let step = if distinct {
        step
    } else {
        failed_step(
            step.id,
            step.title.clone(),
            OnboardingWireError::new(
                OnboardingWireErrorKind::WrongAmsNetId,
                format!(
                    "native source AMS address {}:{} is identical to target AMS address {}:{}",
                    source.ams_net_id, source.ams_port, target.ams_net_id, target.ams_port
                ),
            ),
        )
    };
    step.with_evidence("source_ams_address_available", true)
        .with_evidence("source_ams_net_id", source.ams_net_id)
        .with_evidence("source_ams_port", source.ams_port)
        .with_evidence("target_ams_net_id", target.ams_net_id.clone())
        .with_evidence("target_ams_port", target.ams_port)
        .with_evidence("source_target_addresses_distinct", distinct)
        .with_evidence("source_target_comparison", "full_ams_address")
}

fn passed_round_trip_step(
    target: &TargetIdentity,
    local: &LocalIdentity,
    route_requirement: AdsRouteRequirement,
    state: &str,
) -> DoctorStep {
    let (title, detail, route_mode) = match route_requirement {
        AdsRouteRequirement::ReciprocalRouteRequired => (
            "Route back to truST",
            "An ADS read-state reply returned to the runtime host AMS identity.",
            "reciprocal_route",
        ),
        AdsRouteRequirement::NativeLocalRouter => (
            "Local ADS router path",
            "The Windows native ADS router completed the target round trip; no self-route is required.",
            "native_local_no_self_route",
        ),
    };
    pass_step(DoctorStepId::RoutePresent, title, detail)
        .with_evidence("target_ip", target.ip.clone())
        .with_evidence("local_ip", local.chosen_ip.clone())
        .with_evidence("local_ams_net_id", local.ams_net_id.clone())
        .with_evidence("route_mode", route_mode)
        .with_evidence("route_probe", "read_state")
        .with_evidence("state", state)
}

fn passed_error_reply_step(
    target: &TargetIdentity,
    local: &LocalIdentity,
    route_requirement: AdsRouteRequirement,
    error: &OnboardingWireError,
) -> DoctorStep {
    let (title, detail, route_mode) = match route_requirement {
        AdsRouteRequirement::ReciprocalRouteRequired => (
            "Route back to truST",
            "An ADS error reply returned to the runtime host AMS identity.",
            "reciprocal_route",
        ),
        AdsRouteRequirement::NativeLocalRouter => (
            "Local ADS router path",
            "The native Windows router returned a target/service error; a self-route is not required.",
            "native_local_no_self_route",
        ),
    };
    pass_step(DoctorStepId::RoutePresent, title, detail)
        .with_evidence("target_ip", target.ip.clone())
        .with_evidence("local_ip", local.chosen_ip.clone())
        .with_evidence("local_ams_net_id", local.ams_net_id.clone())
        .with_evidence("route_mode", route_mode)
        .with_evidence("route_probe", "read_state_error_reply")
        .with_evidence("reply_error", error.detail.clone())
}

fn missing_route_round_trip_error(error: OnboardingWireError) -> OnboardingWireError {
    OnboardingWireError::new(
        OnboardingWireErrorKind::RouteMissing,
        format!(
            "TCP 48898 accepted the connection, but no ADS reply returned to the runtime host AMS identity: {}",
            error.detail
        ),
    )
}
