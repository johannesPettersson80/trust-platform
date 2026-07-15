use serde::{Deserialize, Serialize};
use std::net::{IpAddr, Ipv4Addr, SocketAddr, UdpSocket};

use crate::ads::diagnostics::{
    LocalAddressCandidate, LocalIdentity, LocalNetworkClassification, RouteActionAvailability,
};
use crate::ads::onboarding::errors::OnboardingError;

/// Request to derive this host's ADS identity toward a target.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IdentityRequest {
    /// Selected target IP address.
    pub target_ip: String,
    /// Optional Advanced override for the local AMS Net ID.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub local_net_id_override: Option<String>,
}

/// Candidate source address known to the runtime host.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeAddressCandidate {
    /// Candidate local IP address.
    pub ip: String,
    /// Network interface name, if known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nic: Option<String>,
    /// Prefix length for broadcast target calculation, if known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prefix_len: Option<u8>,
    /// Interface-reported directed broadcast address, if available.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub broadcast: Option<String>,
}

pub fn derive_default_ams_net_id(ip: &str) -> Option<String> {
    let octets: Vec<&str> = ip.split('.').collect();
    if octets.len() != 4 {
        return None;
    }
    if octets.iter().any(|octet| octet.parse::<u8>().is_err()) {
        return None;
    }
    Some(format!("{ip}.1.1"))
}

/// Resolves the OS-selected local source IP for traffic toward an ADS target.
pub fn resolve_os_source_ip(target_ip: &str) -> Result<String, OnboardingError> {
    let target: IpAddr = target_ip
        .parse()
        .map_err(|_| OnboardingError::new(format!("invalid target IP '{target_ip}'")))?;
    let bind_addr = if target.is_ipv4() {
        "0.0.0.0:0"
    } else {
        "[::]:0"
    };
    let socket = UdpSocket::bind(bind_addr)
        .map_err(|error| OnboardingError::new(format!("bind source probe socket: {error}")))?;
    socket
        .connect(SocketAddr::new(target, 48_898))
        .map_err(|error| OnboardingError::new(format!("connect source probe socket: {error}")))?;
    socket
        .local_addr()
        .map(|addr| addr.ip().to_string())
        .map_err(|error| OnboardingError::new(format!("read source probe address: {error}")))
}

/// Collects runtime-host interface addresses for identity and broadcast discovery.
pub fn runtime_address_candidates_from_interfaces(
) -> Result<Vec<RuntimeAddressCandidate>, OnboardingError> {
    let interfaces = if_addrs::get_if_addrs()
        .map_err(|error| OnboardingError::new(format!("enumerate network interfaces: {error}")))?;
    Ok(interfaces
        .into_iter()
        .filter(|interface| interface.is_oper_up())
        .map(|interface| match interface.addr {
            if_addrs::IfAddr::V4(addr) => RuntimeAddressCandidate {
                ip: addr.ip.to_string(),
                nic: Some(interface.name),
                prefix_len: Some(addr.prefixlen),
                broadcast: addr.broadcast.map(|value| value.to_string()),
            },
            if_addrs::IfAddr::V6(addr) => RuntimeAddressCandidate {
                ip: addr.ip.to_string(),
                nic: Some(interface.name),
                prefix_len: Some(addr.prefixlen),
                broadcast: None,
            },
        })
        .collect())
}

/// Builds the runtime local identity report from a selected source IP.
pub fn derive_runtime_identity_from_source(
    request: &IdentityRequest,
    chosen_ip: impl Into<String>,
    host_name: Option<String>,
    nic: Option<String>,
    candidates: Vec<RuntimeAddressCandidate>,
) -> Result<LocalIdentity, OnboardingError> {
    let chosen_ip = chosen_ip.into();
    let ams_net_id = match request.local_net_id_override.as_ref() {
        Some(override_id) if !override_id.trim().is_empty() => override_id.trim().to_string(),
        _ => derive_default_ams_net_id(&chosen_ip).ok_or_else(|| {
            OnboardingError::new(format!(
                "cannot derive default AMS Net ID from non-IPv4 source address '{chosen_ip}'"
            ))
        })?,
    };
    let classification = classify_identity_path(&chosen_ip, &request.target_ip, nic.as_deref());
    let mut candidate_rows = candidates
        .into_iter()
        .map(|candidate| LocalAddressCandidate {
            ams_net_id: derive_default_ams_net_id(&candidate.ip).unwrap_or_default(),
            classification: classify_local_address(&candidate.ip, candidate.nic.as_deref()),
            selected: candidate.ip == chosen_ip,
            ip: candidate.ip,
            nic: candidate.nic,
        })
        .collect::<Vec<_>>();

    if !candidate_rows
        .iter()
        .any(|candidate| candidate.ip == chosen_ip)
    {
        candidate_rows.push(LocalAddressCandidate {
            ip: chosen_ip.clone(),
            ams_net_id: ams_net_id.clone(),
            nic: nic.clone(),
            classification,
            selected: true,
        });
    }

    Ok(LocalIdentity {
        host_name,
        chosen_ip,
        ams_net_id,
        nic,
        candidates: candidate_rows,
        classification,
    })
}

/// Classifies one local address for route UX and warnings.
pub fn classify_local_address(ip: &str, nic: Option<&str>) -> LocalNetworkClassification {
    if is_tailscale_like(ip, nic) {
        return LocalNetworkClassification::Tailscale;
    }
    if is_vpn_like_nic(nic) {
        return LocalNetworkClassification::Vpn;
    }
    match ip.parse::<IpAddr>() {
        Ok(IpAddr::V4(addr)) if addr.is_loopback() => LocalNetworkClassification::Loopback,
        Ok(IpAddr::V4(addr)) if addr.is_private() => LocalNetworkClassification::Lan,
        Ok(IpAddr::V4(addr)) if is_ipv4_link_local(addr) => LocalNetworkClassification::Unknown,
        Ok(IpAddr::V4(_)) => LocalNetworkClassification::Public,
        Ok(IpAddr::V6(addr)) if addr.is_loopback() => LocalNetworkClassification::Loopback,
        Ok(IpAddr::V6(addr)) if is_ipv6_unique_local(addr) => LocalNetworkClassification::Vpn,
        Ok(IpAddr::V6(_)) => LocalNetworkClassification::Public,
        Err(_) => LocalNetworkClassification::Unknown,
    }
}

/// Returns route action availability for the selected identity.
pub fn auto_route_availability_for_identity(identity: &LocalIdentity) -> RouteActionAvailability {
    match identity.classification {
        LocalNetworkClassification::Public | LocalNetworkClassification::NatSuspect => {
            RouteActionAvailability::DisabledNatOrPublic
        }
        _ => RouteActionAvailability::Available,
    }
}

fn classify_identity_path(
    chosen_ip: &str,
    target_ip: &str,
    nic: Option<&str>,
) -> LocalNetworkClassification {
    let base = classify_local_address(chosen_ip, nic);
    if matches!(
        base,
        LocalNetworkClassification::Lan
            | LocalNetworkClassification::Vpn
            | LocalNetworkClassification::Tailscale
    ) && matches!(
        classify_local_address(target_ip, None),
        LocalNetworkClassification::Public
    ) {
        return LocalNetworkClassification::NatSuspect;
    }
    base
}

fn is_tailscale_like(ip: &str, nic: Option<&str>) -> bool {
    let nic_matches = nic
        .map(|name| name.to_ascii_lowercase().contains("tailscale"))
        .unwrap_or(false);
    if nic_matches {
        return true;
    }
    let Ok(IpAddr::V4(addr)) = ip.parse::<IpAddr>() else {
        return false;
    };
    let octets = addr.octets();
    octets[0] == 100 && (64..=127).contains(&octets[1])
}

fn is_vpn_like_nic(nic: Option<&str>) -> bool {
    let Some(name) = nic else {
        return false;
    };
    let name = name.to_ascii_lowercase();
    ["tun", "tap", "wg", "vpn", "utun", "ppp"]
        .iter()
        .any(|marker| name.contains(marker))
}

fn is_ipv4_link_local(addr: Ipv4Addr) -> bool {
    let [a, b, _, _] = addr.octets();
    a == 169 && b == 254
}

fn is_ipv6_unique_local(addr: std::net::Ipv6Addr) -> bool {
    (addr.segments()[0] & 0xfe00) == 0xfc00
}
