use crate::ads::diagnostics::TargetIdentity;
use crate::ads::onboarding::errors::{OnboardingWireError, OnboardingWireErrorKind};

use super::{ams_net_id_text, DEFAULT_ADS_PLC_PORT};

pub(super) fn local_router_identity(
    target_ip: &str,
) -> Result<TargetIdentity, OnboardingWireError> {
    let net_id = trust_tcads_native::local_net_id()
        .map_err(|error| native_router_error(target_ip, format!("query TcAdsDll.dll: {error}")))?;
    Ok(TargetIdentity {
        name: None,
        ip: target_ip.to_string(),
        ams_net_id: ams_net_id_text(&net_id),
        ams_port: DEFAULT_ADS_PLC_PORT,
        tc_version: None,
    })
}

fn native_router_error(target_ip: &str, detail: String) -> OnboardingWireError {
    OnboardingWireError::new(
        OnboardingWireErrorKind::UdpIdentifyBlocked,
        format!("Windows TwinCAT ADS API identity failed for {target_ip}: {detail}"),
    )
}
