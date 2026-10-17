//! ADS (specification 11, "comm.discover" and "Sources"): TwinCAT systems that answer an ADS
//! identify, directed at a known address and at this computer, or broadcast to a subnet or to the
//! subnets of this computer's interfaces. `discover` runs one identify round for `comm.discover`
//! or for a search; `search` identifies each known address of a search once and repeats broadcast
//! rounds until the search ends. Without the `ads-wire` feature there is no ADS search.

use std::time::Duration;

#[cfg(feature = "ads-wire")]
use serde_json::json;

use super::super::model::{AdsIdentity, DeviceKey, Identity, ProtocolFind, SourceId};
use super::super::runner::SourceSink;
use super::find::{address_key, find, host_part, text, FindSink};
#[cfg(feature = "ads-wire")]
use super::report::sanitize_id;
use super::report::{CandidateSink, Deadline, DiscoverCandidate};
#[cfg(feature = "ads-wire")]
use super::scope::parse_ipv4_cidr;
use super::scope::DiscoverScope;
use super::SourceContext;

#[cfg(feature = "ads-wire")]
#[path = "ads_discovery_target.rs"]
mod ads_discovery_target;

/// The pause between two ADS broadcast rounds of a search.
const ADS_ROUND_PAUSE: Duration = Duration::from_millis(1_000);

/// One identify round: the known address of `scope` and this computer, or, without a known
/// address or with a subnet, a broadcast.
#[cfg(feature = "ads-wire")]
pub(crate) fn discover(
    scope: &DiscoverScope,
    deadline: Deadline,
    sink: &mut dyn CandidateSink,
) -> Result<(), String> {
    use crate::ads::onboarding::runtime_address_candidates_from_interfaces;
    use crate::ads::onboarding::{
        directed_broadcast_targets_from_candidates, discover_targets, interface_directed_targets,
        AdsRsOnboardingWire, DiscoveryRequest, DiscoverySource,
    };

    if deadline.passed() {
        return Ok(());
    }
    let mut request = DiscoveryRequest {
        target: ads_discovery_target::directed_target(scope.host.as_deref()),
        directed_targets: Vec::new(),
        target_ams_net_id: None,
        ams_port: Some(851),
        target_name: None,
        include_broadcast: scope.host.is_none(),
        broadcast_targets: Vec::new(),
        timeout_ms: scope.timeout_ms,
    };
    if let Some(cidr) = scope.cidr.as_deref() {
        request.include_broadcast = true;
        request.broadcast_targets = vec![broadcast_target_for_cidr(cidr)?];
    } else if request.include_broadcast {
        let candidates = runtime_address_candidates_from_interfaces()
            .map_err(|error| format!("enumerate interfaces for ADS broadcast: {error}"))?;
        request.directed_targets =
            interface_directed_targets(candidates.iter().map(|candidate| candidate.ip.as_str()));
        request.broadcast_targets = directed_broadcast_targets_from_candidates(&candidates);
    }
    let without_broadcast_target =
        request.include_broadcast && request.broadcast_targets.is_empty();

    let mut wire = AdsRsOnboardingWire::default();
    let results = discover_targets(&mut wire, &request)
        .map_err(|error| format!("ADS discovery failed: {error}"))?;
    // Reported once the round has answered: a round that fails reports only its error.
    if without_broadcast_target {
        sink.warn("No non-loopback IPv4 broadcast target was available for ADS discovery.".into());
    }
    for result in results {
        let source = match result.source {
            DiscoverySource::Manual => "manual",
            DiscoverySource::DirectedIdentify => "ads_identify",
            DiscoverySource::DirectedBroadcast => "ads_broadcast",
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
        sink.found(DiscoverCandidate {
            id: format!("ads:{}", sanitize_id(target.ams_net_id.as_str())),
            label,
            source,
            confidence: "observed",
            params: json!({
                "ams_net_id": target.ams_net_id,
                "host": target.ip,
                "name": target.name,
                "ams_port": target.ams_port,
                "tc_version": target.tc_version,
            }),
            warnings: Vec::new(),
        });
    }
    Ok(())
}

/// Without the `ads-wire` feature: a warning instead of a round.
#[cfg(not(feature = "ads-wire"))]
pub(crate) fn discover(
    _scope: &DiscoverScope,
    _deadline: Deadline,
    sink: &mut dyn CandidateSink,
) -> Result<(), String> {
    sink.warn("ADS discovery needs a runtime built with the ads-wire feature.".to_string());
    Ok(())
}

#[cfg(feature = "ads-wire")]
fn broadcast_target_for_cidr(cidr: &str) -> Result<String, String> {
    use crate::ads::onboarding::directed_broadcast_target;

    let (ip, prefix) = parse_ipv4_cidr(cidr)?;
    directed_broadcast_target(&ip.to_string(), prefix)
        .ok_or_else(|| format!("could not compute directed broadcast target for {cidr}"))
}

/// ADS in a search. Each known address is identified once. Without one, or with a subnet,
/// broadcast rounds listen until the search ends.
pub(super) fn search(context: &SourceContext, sink: &SourceSink) {
    for host in &context.hosts {
        if sink.ended() {
            return;
        }
        let host = host_part(host);
        let scope = DiscoverScope {
            host: Some(host.to_string()),
            ..DiscoverScope::default()
        };
        let mut found = FindSink::new(sink, Some(host), ads_find);
        if let Err(error) = discover(&scope, found.deadline(), &mut found) {
            sink.warn(format!("{host}: {error}"));
        }
    }
    if !context.hosts.is_empty() && context.cidr.is_none() {
        return;
    }
    let scope = DiscoverScope {
        cidr: context.cidr.clone(),
        ..DiscoverScope::default()
    };
    while !sink.ended() {
        let mut found = FindSink::new(sink, None, ads_find);
        if let Err(error) = discover(&scope, found.deadline(), &mut found) {
            sink.warn(error);
        }
        sink.wait(ADS_ROUND_PAUSE);
    }
}

fn ads_find(candidate: DiscoverCandidate) -> ProtocolFind {
    let ams_net_id = text(&candidate.params, "ams_net_id").unwrap_or_default();
    let mut keys = vec![DeviceKey::AmsNetId(ams_net_id.clone())];
    keys.extend(text(&candidate.params, "host").and_then(|host| address_key(&host)));
    let identity = Identity::Ads(AdsIdentity {
        ams_net_id: ams_net_id.clone(),
        computer_name: text(&candidate.params, "name"),
        twincat_version: text(&candidate.params, "tc_version"),
    });
    find(SourceId::Ads, candidate, identity).keyed(ams_net_id, keys)
}
