use std::net::IpAddr;

use serde_json::json;

use super::{parse_ipv4_cidr, sanitize_id, DiscoverCandidate, DiscoverScope};
use crate::ads::onboarding::{
    directed_broadcast_target, directed_broadcast_targets_from_candidates, discover_targets,
    runtime_address_candidates_from_interfaces, DiscoveryRequest, DiscoveryResult, DiscoverySource,
};

const DEFAULT_ADS_PORT: u16 = 851;

#[cfg(feature = "ads-wire")]
pub(super) fn discover_ads(
    scope: &DiscoverScope,
    warnings: &mut Vec<String>,
) -> Result<Vec<DiscoverCandidate>, String> {
    let request = discovery_request(scope, warnings)?;
    let mut wire = crate::ads::onboarding::AdsRsOnboardingWire::default();
    let results = discover_targets(&mut wire, &request)
        .map_err(|error| format!("ADS discovery failed: {error}"))?;
    Ok(discovery_candidates(results))
}

#[cfg(not(feature = "ads-wire"))]
pub(super) fn discover_ads(
    scope: &DiscoverScope,
    warnings: &mut Vec<String>,
) -> Result<Vec<DiscoverCandidate>, String> {
    let request = discovery_request(scope, warnings)?;
    if request.target_ams_net_id.is_none() || request.include_broadcast {
        warnings.push("ADS wire discovery needs a runtime built with the ads-wire feature.".into());
        return Ok(Vec::new());
    }
    let mut wire = crate::ads::onboarding::MockAdsOnboardingWire::default();
    let results = discover_targets(&mut wire, &request)
        .map_err(|error| format!("ADS discovery failed: {error}"))?;
    Ok(discovery_candidates(results))
}

fn discovery_request(
    scope: &DiscoverScope,
    warnings: &mut Vec<String>,
) -> Result<DiscoveryRequest, String> {
    let target = scope
        .host
        .as_deref()
        .map(str::trim)
        .filter(|host| !host.is_empty())
        .map(ToString::to_string);
    if let Some(host) = target.as_deref() {
        validate_host_without_port(host)?;
    }
    let target_ams_net_id = scope
        .target_ams_net_id
        .as_deref()
        .map(str::trim)
        .filter(|net_id| !net_id.is_empty())
        .map(ToString::to_string);
    if let Some(net_id) = target_ams_net_id.as_deref() {
        validate_ams_net_id(net_id)?;
    }
    if target.is_none() && target_ams_net_id.is_some() {
        return Err(
            "ADS manual discovery needs a Host or IP when target_ams_net_id is supplied."
                .to_string(),
        );
    }

    let include_broadcast = target.is_none();
    let mut request = DiscoveryRequest {
        target,
        target_ams_net_id,
        ams_port: Some(scope.ams_port.map_or(DEFAULT_ADS_PORT, |port| port.get())),
        target_name: None,
        include_broadcast,
        broadcast_targets: Vec::new(),
        timeout_ms: scope.timeout_ms,
    };
    if let Some(cidr) = scope.cidr.as_deref() {
        request.include_broadcast = true;
        request.broadcast_targets = vec![broadcast_target_for_cidr(cidr)?];
    } else if request.include_broadcast {
        let candidates = runtime_address_candidates_from_interfaces()
            .map_err(|error| format!("enumerate interfaces for ADS broadcast: {error}"))?;
        request.broadcast_targets = directed_broadcast_targets_from_candidates(&candidates);
    }
    if request.include_broadcast && request.broadcast_targets.is_empty() {
        warnings
            .push("No non-loopback IPv4 broadcast target was available for ADS discovery.".into());
    }
    Ok(request)
}

fn validate_host_without_port(host: &str) -> Result<(), String> {
    if host.parse::<IpAddr>().is_ok() {
        return Ok(());
    }
    let has_inline_port = host.rsplit_once(':').is_some_and(|(name, port)| {
        !name.is_empty() && !port.is_empty() && port.chars().all(|ch| ch.is_ascii_digit())
    });
    if has_inline_port {
        return Err(format!(
            "ADS discovery accepts a Host or IP only; remove the inline port from '{host}' and use the separate ADS port setting."
        ));
    }
    Ok(())
}

fn validate_ams_net_id(net_id: &str) -> Result<(), String> {
    let parts = net_id.split('.').collect::<Vec<_>>();
    if parts.len() != 6 || parts.iter().any(|part| part.parse::<u8>().is_err()) {
        return Err(
            "AMS Net ID must contain six numbers 0-255 separated by dots, for example 192.168.10.20.1.1."
                .to_string(),
        );
    }
    Ok(())
}

fn discovery_candidates(results: Vec<DiscoveryResult>) -> Vec<DiscoverCandidate> {
    results
        .into_iter()
        .map(|result| {
            let (source, confidence, warnings) = match result.source {
                DiscoverySource::Manual => (
                    "manual",
                    "declared",
                    vec!["Target identity was declared manually and has not been verified.".into()],
                ),
                DiscoverySource::LocalRouter => ("ads_local_router", "observed", Vec::new()),
                DiscoverySource::DirectedIdentify => ("ads_identify", "observed", Vec::new()),
                DiscoverySource::DirectedBroadcast => ("ads_broadcast", "observed", Vec::new()),
            };
            let target = result.target;
            let label = target
                .name
                .as_deref()
                .filter(|name| !name.trim().is_empty())
                .map_or_else(
                    || format!("TwinCAT {}", target.ams_net_id),
                    |name| format!("{name} · {}", target.ams_net_id),
                );
            DiscoverCandidate {
                id: format!("ads:{}", sanitize_id(target.ams_net_id.as_str())),
                label,
                source,
                confidence,
                params: json!({
                    "ams_net_id": target.ams_net_id,
                    "host": target.ip,
                    "name": target.name,
                    "ams_port": target.ams_port,
                    "tc_version": target.tc_version,
                }),
                warnings,
            }
        })
        .collect()
}

fn broadcast_target_for_cidr(cidr: &str) -> Result<String, String> {
    let (ip, prefix) = parse_ipv4_cidr(cidr)?;
    directed_broadcast_target(&ip.to_string(), prefix)
        .ok_or_else(|| format!("could not compute directed broadcast target for {cidr}"))
}
