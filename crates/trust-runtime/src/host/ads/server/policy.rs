//! ADS server client allowlist policy.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::sync::{Arc, Mutex, RwLock};

use serde::Serialize;
use smol_str::SmolStr;
use trust_ads_core::AmsNetId;
use trust_ads_server::{ClientId, ClientPolicy};

use super::contracts::{AdsServerClientConfig, AdsServerRuntimeConfig, AdsServerSourcePin};

const MAX_REFUSED_CLIENTS: usize = 32;

/// Runtime-owned ADS server client policy.
#[derive(Debug, Clone)]
pub struct AdsServerClientPolicy {
    clients: Arc<RwLock<Vec<AdsServerClientConfig>>>,
    refused_clients: Arc<Mutex<Vec<AdsServerRefusedClient>>>,
    allow_unpinned_clients: bool,
}

impl AdsServerClientPolicy {
    /// Creates a client policy from runtime config.
    #[must_use]
    pub fn new(config: &AdsServerRuntimeConfig) -> Self {
        Self {
            clients: Arc::new(RwLock::new(config.clients.clone())),
            refused_clients: Arc::new(Mutex::new(Vec::new())),
            allow_unpinned_clients: config.allow_unpinned_clients,
        }
    }

    /// Temporarily permits one client until the returned guard is dropped.
    #[must_use]
    pub fn permit_temporarily(
        &self,
        ams_net_id: AmsNetId,
        source_ip: impl Into<SmolStr>,
    ) -> TemporaryClientPermit {
        let client = AdsServerClientConfig {
            ams_net_id,
            source: AdsServerSourcePin::Ip(source_ip.into()),
        };
        if let Ok(mut clients) = self.clients.write() {
            clients.push(client.clone());
        }
        TemporaryClientPermit {
            policy: self.clone(),
            client: Some(client),
        }
    }

    /// Returns true when the client's AMS Net ID and source IP satisfy config.
    #[must_use]
    pub fn permits_client(&self, client: &ClientId) -> bool {
        let Ok(clients) = self.clients.read() else {
            self.record_refused(client, "policy_lock_failed");
            return false;
        };
        let permitted = clients.iter().any(|allowed| {
            allowed.ams_net_id == client.ams_net_id
                && source_matches(
                    &allowed.source,
                    client.source_ip.as_deref(),
                    self.allow_unpinned_clients,
                )
        });
        if permitted {
            return true;
        }
        let reason = refusal_reason(&clients, client);
        drop(clients);
        self.record_refused(client, reason);
        false
    }

    /// Returns recent refused client attempts for setup/status UX.
    #[must_use]
    pub fn recently_refused_clients(&self) -> Vec<AdsServerRefusedClient> {
        self.refused_clients
            .lock()
            .map_or_else(|_| Vec::new(), |clients| clients.clone())
    }

    fn record_refused(&self, client: &ClientId, reason: &'static str) {
        let now_ms = now_ms();
        let source_ip = client.source_ip.clone();
        let Ok(mut refused) = self.refused_clients.lock() else {
            return;
        };
        if let Some(entry) = refused.iter_mut().find(|entry| {
            entry.ams_net_id == client.ams_net_id.0
                && entry.source_ip == source_ip
                && entry.reason == reason
        }) {
            entry.count = entry.count.saturating_add(1);
            entry.last_seen_ms = now_ms;
            return;
        }
        if refused.len() >= MAX_REFUSED_CLIENTS {
            refused.remove(0);
        }
        refused.push(AdsServerRefusedClient {
            ams_net_id: client.ams_net_id.0.clone(),
            source_ip: source_ip.clone(),
            reason: reason.to_string(),
            last_seen_ms: now_ms,
            count: 1,
            suggested_client: AdsServerClientSuggestion {
                ams_net_id: client.ams_net_id.0.clone(),
                source_ip,
            },
        });
    }
}

/// One refused ADS server client attempt, suitable for setup/status surfaces.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AdsServerRefusedClient {
    /// Client AMS Net ID asserted in the AMS frame.
    pub ams_net_id: String,
    /// TCP source IP observed by the runtime host.
    pub source_ip: Option<String>,
    /// Machine-readable denial reason.
    pub reason: String,
    /// Last observed time in Unix milliseconds.
    pub last_seen_ms: u64,
    /// Number of coalesced matching attempts.
    pub count: u64,
    /// Prefilled config values for an operator-approved allowlist entry.
    pub suggested_client: AdsServerClientSuggestion,
}

/// Suggested allowlist entry derived from one refused client attempt.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AdsServerClientSuggestion {
    /// Client AMS Net ID to add.
    pub ams_net_id: String,
    /// Source IP pin to add when known.
    pub source_ip: Option<String>,
}

/// Scoped temporary ADS client policy entry.
#[derive(Debug)]
pub struct TemporaryClientPermit {
    policy: AdsServerClientPolicy,
    client: Option<AdsServerClientConfig>,
}

impl Drop for TemporaryClientPermit {
    fn drop(&mut self) {
        let Some(client) = self.client.take() else {
            return;
        };
        if let Ok(mut clients) = self.policy.clients.write() {
            if let Some(index) = clients.iter().position(|entry| entry == &client) {
                clients.remove(index);
            }
        }
    }
}

impl ClientPolicy for AdsServerClientPolicy {
    fn permits(&self, client: &ClientId) -> bool {
        self.permits_client(client)
    }
}

fn source_matches(
    pin: &AdsServerSourcePin,
    source_ip: Option<&str>,
    allow_unpinned_clients: bool,
) -> bool {
    match pin {
        AdsServerSourcePin::Unpinned => allow_unpinned_clients,
        AdsServerSourcePin::Ip(expected) => {
            let Some(source_ip) = source_ip else {
                return false;
            };
            let (Ok(expected), Ok(source)) =
                (expected.parse::<IpAddr>(), source_ip.parse::<IpAddr>())
            else {
                return false;
            };
            source == expected
        }
        AdsServerSourcePin::Cidr(cidr) => {
            let Some(source_ip) = source_ip else {
                return false;
            };
            ip_in_cidr(source_ip, cidr.as_str()).unwrap_or(false)
        }
    }
}

fn refusal_reason(clients: &[AdsServerClientConfig], client: &ClientId) -> &'static str {
    let matching_net_id = clients
        .iter()
        .any(|allowed| allowed.ams_net_id == client.ams_net_id);
    if !matching_net_id {
        return "ams_net_id_not_allowlisted";
    }
    if client.source_ip.is_none() {
        return "missing_source_ip";
    }
    "source_ip_not_allowed"
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .min(u128::from(u64::MAX)) as u64
}

fn ip_in_cidr(source_ip: &str, cidr: &str) -> Option<bool> {
    let source: IpAddr = source_ip.parse().ok()?;
    let (network, prefix) = cidr.split_once('/')?;
    let network: IpAddr = network.parse().ok()?;
    let prefix: u8 = prefix.parse().ok()?;
    match (source, network) {
        (IpAddr::V4(source), IpAddr::V4(network)) if prefix <= 32 => {
            Some(masked_v4(source, prefix) == masked_v4(network, prefix))
        }
        (IpAddr::V6(source), IpAddr::V6(network)) if prefix <= 128 => {
            Some(masked_v6(source, prefix) == masked_v6(network, prefix))
        }
        _ => None,
    }
}

fn masked_v4(addr: Ipv4Addr, prefix: u8) -> u32 {
    let value = u32::from(addr);
    if prefix == 0 {
        0
    } else {
        value & (!0_u32 << (32 - u32::from(prefix)))
    }
}

fn masked_v6(addr: Ipv6Addr, prefix: u8) -> u128 {
    let value = u128::from(addr);
    if prefix == 0 {
        0
    } else {
        value & (!0_u128 << (128 - u32::from(prefix)))
    }
}
