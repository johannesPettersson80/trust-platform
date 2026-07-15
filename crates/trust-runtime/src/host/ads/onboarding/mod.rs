//! ADS onboarding control-plane engine.

pub mod discover;
pub mod doctor;
pub mod errors;
pub mod identity;
pub mod import;
pub mod route;
pub mod wire;

pub use discover::{
    directed_broadcast_target, directed_broadcast_targets_from_candidates, discover_targets,
    DiscoveryRequest, DiscoveryResult, DiscoverySource,
};
pub use doctor::{
    default_step_timeouts, run_doctor, ActiveAdsDeviceSnapshot, ActiveDeviceStrategy,
    DoctorCancellation, DoctorJob, DoctorJobProgress, DoctorJobState, DoctorOptions,
    DoctorStepTimeout, REQUIRED_DOCTOR_STEPS,
};
pub use errors::{
    upload_failure_implies_missing_return_route, OnboardingError, OnboardingWireError,
    OnboardingWireErrorKind,
};
pub use identity::{
    auto_route_availability_for_identity, classify_local_address, derive_default_ams_net_id,
    derive_runtime_identity_from_source, resolve_os_source_ip,
    runtime_address_candidates_from_interfaces, IdentityRequest, RuntimeAddressCandidate,
};
pub use import::{
    apply_symbol_import, build_symbol_import_response, SymbolImportApplyRequest,
    SymbolImportArtifacts, SymbolImportCandidate, SymbolImportGroup, SymbolImportRequest,
    SymbolImportResponse,
};
pub use route::{
    add_route_with_channel_policy, build_route_plan, build_route_remove_artifact, RouteAddRequest,
    RouteCredentials, RoutePlanRequest, RoutePlanRole, RouteRemoveRequest,
};
pub use wire::{
    AdsOnboardingWire, GuardedWriteProbe, MockAdsOnboardingScenario, MockAdsOnboardingWire,
};

#[cfg(feature = "ads-wire")]
pub use wire::AdsRsOnboardingWire;

#[cfg(test)]
mod tests;
