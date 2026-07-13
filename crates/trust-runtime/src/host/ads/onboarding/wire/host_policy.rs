use crate::ads::diagnostics::TargetIdentity;
use crate::ads::onboarding::errors::{OnboardingWireError, OnboardingWireErrorKind};

use super::{AdsOnboardingWire, AdsRouteRequirement, AdsRouterProbe};

pub(super) fn probe_ads_router<W: AdsOnboardingWire>(
    wire: &mut W,
    target: &TargetIdentity,
) -> Result<AdsRouterProbe, OnboardingWireError> {
    if crate::ads::backend_host::target_uses_native_windows_router(&target.ip) {
        wire.read_state(target)?;
        Ok(AdsRouterProbe::NativeRouterRoundTrip)
    } else {
        wire.tcp_probe_48898(&target.ip)?;
        Ok(AdsRouterProbe::Tcp48898)
    }
}

pub(super) fn route_requirement(target_ip: &str) -> AdsRouteRequirement {
    if crate::ads::backend_host::target_uses_native_windows_router(target_ip) {
        AdsRouteRequirement::NativeLocalRouter
    } else {
        AdsRouteRequirement::ReciprocalRouteRequired
    }
}

pub(super) fn route_failure_kind(target_ip: &str) -> OnboardingWireErrorKind {
    match route_requirement(target_ip) {
        AdsRouteRequirement::ReciprocalRouteRequired => OnboardingWireErrorKind::RouteMissing,
        AdsRouteRequirement::NativeLocalRouter => OnboardingWireErrorKind::WrongPlcPort,
    }
}
