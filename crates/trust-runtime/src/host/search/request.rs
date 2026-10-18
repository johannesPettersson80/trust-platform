//! The parameters of `comm.search.start`, `comm.search.status` and `comm.search.cancel`, and the
//! checks a search passes before any source runs (specification 11, "One search", "Requests").

use std::fmt;
use std::time::Duration;

use serde::de::DeserializeOwned;
use serde::Deserialize;
use serde_json::Value;

use super::model::{SearchOrigin, SourceId};
use super::sources::scope::ipv4_targets_for_cidr;

/// The deadline of a search that names none.
pub const DEFAULT_DEADLINE_MS: u64 = 10_000;
/// The longest deadline a search accepts.
pub const MAX_DEADLINE_MS: u64 = 60_000;
/// The most known addresses one search probes.
pub const MAX_HOSTS: usize = 256;

/// The parameters of `comm.search.start`. Every member is optional, and an unknown member is a
/// request error.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SearchRequest {
    /// Where the search runs; it must be where the search is started.
    #[serde(default)]
    pub origin: Option<SearchOrigin>,
    /// The sources to run; every source when absent.
    #[serde(default)]
    pub only: Option<Vec<SourceId>>,
    /// Known addresses and a subnet.
    #[serde(default)]
    pub targets: SearchTargets,
    /// The deadline, from 1 to 60,000 ms; 10,000 ms when absent.
    #[serde(default)]
    pub deadline_ms: Option<u64>,
    /// Runs the fieldbus sources.
    #[serde(default)]
    pub confirm_fieldbus_scan: bool,
    /// The buses that a confirmed fieldbus scan scans.
    #[serde(default)]
    pub fieldbus: Vec<FieldbusTarget>,
    /// The Modbus TCP unit IDs to probe.
    #[serde(default)]
    pub modbus: ModbusTargets,
    /// The ADS ports to probe, each from 1 to 65,535; the default set when absent
    /// (specification 11, "Communication discovery scope and symbol browsing").
    #[serde(default)]
    pub ads_ports: Option<Value>,
}

/// The Modbus TCP unit IDs a search probes (specification 11, "Requests").
#[derive(Debug, Default, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModbusTargets {
    /// The unit IDs to probe, each from 1 to 247 and without repeats; unit ID 1 when absent.
    #[serde(default)]
    pub unit_ids: Vec<u8>,
}

/// What a search probes besides what it finds by itself.
#[derive(Debug, Default, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SearchTargets {
    /// Known addresses: a host name or IP address with an optional port.
    #[serde(default)]
    pub hosts: Vec<String>,
    /// One IPv4 subnet of at most `/24`.
    #[serde(default)]
    pub cidr: Option<String>,
}

/// One bus of a confirmed fieldbus scan.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FieldbusTarget {
    /// The bus.
    pub bus: FieldbusBus,
    /// The adapter; without it, the adapter of the project's driver for that bus.
    #[serde(default)]
    pub adapter: Option<String>,
}

/// A fieldbus that a search can scan.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FieldbusBus {
    /// EtherCAT.
    Ethercat,
}

/// The parameters of `comm.search.status`.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StatusRequest {
    /// The search.
    pub search_id: String,
    /// The `cursor` of an earlier answer: only what changed after it is listed.
    #[serde(default)]
    pub since: Option<u64>,
}

/// The parameters of `comm.search.cancel`.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CancelRequest {
    /// The search.
    pub search_id: String,
}

/// Why a search request has no answer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SearchError {
    /// The request breaks a rule; nothing was started or changed.
    Invalid(String),
    /// No kept search has this ID.
    NotFound(String),
    /// Every kept search is still running.
    Busy(usize),
}

impl SearchError {
    /// The stable error code, for the errors that have one.
    #[must_use]
    pub fn code(&self) -> Option<&'static str> {
        match self {
            Self::Invalid(_) => None,
            Self::NotFound(_) => Some("search_not_found"),
            Self::Busy(_) => Some("search_busy"),
        }
    }
}

impl fmt::Display for SearchError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Invalid(message) => formatter.write_str(message),
            Self::NotFound(search_id) => write!(formatter, "unknown search '{search_id}'"),
            Self::Busy(running) => write!(
                formatter,
                "{running} searches are already running; cancel one or wait until one ends"
            ),
        }
    }
}

impl std::error::Error for SearchError {}

/// A request that passed its checks: what the sources read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ValidSearch {
    pub(crate) sources: Vec<SourceId>,
    pub(crate) hosts: Vec<String>,
    pub(crate) cidr: Option<String>,
    pub(crate) deadline: Duration,
    pub(crate) fieldbus_confirmed: bool,
    /// The EtherCAT adapters to scan; `None` is the adapter of the project's EtherCAT driver.
    pub(crate) ethercat_adapters: Vec<Option<String>>,
    /// The Modbus TCP unit IDs to probe; unit ID 1 when the request names none.
    pub(crate) modbus_unit_ids: Vec<u8>,
    /// The ADS ports to probe; the default set when the request names none.
    pub(crate) ads_ports: Vec<u16>,
}

impl SearchRequest {
    /// Reads `comm.search.start` parameters; absent parameters are a request with no members.
    ///
    /// # Errors
    /// A member is unknown or has the wrong type.
    pub fn from_params(params: Option<Value>) -> Result<Self, SearchError> {
        match params {
            None => Ok(Self::default()),
            Some(params) => decode(params),
        }
    }

    /// Checks the request for a search that runs at `origin`.
    pub(crate) fn validate(self, origin: SearchOrigin) -> Result<ValidSearch, SearchError> {
        if let Some(requested) = self.origin
            && requested != origin
        {
            return Err(SearchError::Invalid(
                match origin {
                    SearchOrigin::Runtime => {
                        "a search started in a runtime runs inside it: origin must be runtime"
                    }
                    SearchOrigin::ThisHost => {
                        "a search that this tool runs is on this computer: origin must be this_host"
                    }
                }
                .to_string(),
            ));
        }
        let sources = match self.only {
            None => SourceId::ALL.to_vec(),
            Some(only) if only.is_empty() => {
                return Err(SearchError::Invalid(
                    "only must name at least one source".to_string(),
                ));
            }
            Some(only) => SourceId::ALL
                .into_iter()
                .filter(|id| only.contains(id))
                .collect(),
        };
        let deadline_ms = self.deadline_ms.unwrap_or(DEFAULT_DEADLINE_MS);
        if !(1..=MAX_DEADLINE_MS).contains(&deadline_ms) {
            return Err(SearchError::Invalid(format!(
                "deadline_ms must be between 1 and {MAX_DEADLINE_MS}, not {deadline_ms}"
            )));
        }
        let hosts = known_addresses(self.targets.hosts)?;
        let cidr = match self.targets.cidr.as_deref().map(str::trim) {
            None => None,
            Some(cidr) => {
                ipv4_targets_for_cidr(cidr).map_err(SearchError::Invalid)?;
                Some(cidr.to_string())
            }
        };
        Ok(ValidSearch {
            sources,
            hosts,
            cidr,
            deadline: Duration::from_millis(deadline_ms),
            fieldbus_confirmed: self.confirm_fieldbus_scan,
            ethercat_adapters: ethercat_adapters(self.fieldbus)?,
            modbus_unit_ids: modbus_unit_ids(self.modbus.unit_ids)?,
            ads_ports: super::sources::ads_ports::checked_ports(self.ads_ports.as_ref())
                .map_err(SearchError::Invalid)?,
        })
    }
}

impl StatusRequest {
    /// Reads `comm.search.status` parameters.
    ///
    /// # Errors
    /// The parameters are absent, lack `search_id`, or have an unknown member.
    pub fn from_params(params: Option<Value>) -> Result<Self, SearchError> {
        decode(params.ok_or_else(missing_search_id)?)
    }
}

impl CancelRequest {
    /// Reads `comm.search.cancel` parameters.
    ///
    /// # Errors
    /// The parameters are absent, lack `search_id`, or have an unknown member.
    pub fn from_params(params: Option<Value>) -> Result<Self, SearchError> {
        decode(params.ok_or_else(missing_search_id)?)
    }
}

fn decode<T: DeserializeOwned>(params: Value) -> Result<T, SearchError> {
    serde_json::from_value(params).map_err(|error| SearchError::Invalid(error.to_string()))
}

fn missing_search_id() -> SearchError {
    SearchError::Invalid("missing field `search_id`".to_string())
}

/// Known addresses, trimmed and without repeats: a host name or IP address with an optional port
/// and nothing else.
fn known_addresses(hosts: Vec<String>) -> Result<Vec<String>, SearchError> {
    if hosts.len() > MAX_HOSTS {
        return Err(SearchError::Invalid(format!(
            "targets.hosts names {} addresses; a search probes at most {MAX_HOSTS}",
            hosts.len()
        )));
    }
    let mut checked: Vec<String> = Vec::with_capacity(hosts.len());
    for host in hosts {
        let host = host.trim();
        let plain = !host.is_empty()
            && !host.contains("://")
            && !host
                .chars()
                .any(|ch| ch.is_whitespace() || matches!(ch, '/' | '?' | '#' | '@'));
        if !plain {
            return Err(SearchError::Invalid(format!(
                "targets.hosts: '{host}' is not a host name or IP address with an optional port"
            )));
        }
        if !checked.iter().any(|known| known == host) {
            checked.push(host.to_string());
        }
    }
    Ok(checked)
}

/// The Modbus TCP unit IDs that a search probes, each from 1 to 247 and without repeats;
/// unit ID 1 when the request names none.
fn modbus_unit_ids(unit_ids: Vec<u8>) -> Result<Vec<u8>, SearchError> {
    let mut checked = Vec::with_capacity(unit_ids.len());
    for unit_id in unit_ids {
        if !(1..=247).contains(&unit_id) {
            return Err(SearchError::Invalid(format!(
                "modbus.unit_ids must each be between 1 and 247, not {unit_id}"
            )));
        }
        if !checked.contains(&unit_id) {
            checked.push(unit_id);
        }
    }
    if checked.is_empty() {
        checked.push(1);
    }
    Ok(checked)
}

/// The EtherCAT adapters that a confirmed scan scans, without repeats; the project's driver
/// adapter (`None`) when the request names none.
fn ethercat_adapters(fieldbus: Vec<FieldbusTarget>) -> Result<Vec<Option<String>>, SearchError> {
    let mut adapters = Vec::new();
    for target in fieldbus {
        let adapter = match target.adapter.as_deref().map(str::trim) {
            None => None,
            Some("") => {
                return Err(SearchError::Invalid(
                    "fieldbus: an adapter must not be empty".to_string(),
                ));
            }
            Some(adapter) => Some(adapter.to_string()),
        };
        match target.bus {
            FieldbusBus::Ethercat => {
                if !adapters.contains(&adapter) {
                    adapters.push(adapter);
                }
            }
        }
    }
    if adapters.is_empty() {
        adapters.push(None);
    }
    Ok(adapters)
}

#[cfg(test)]
#[path = "request_tests.rs"]
mod tests;
