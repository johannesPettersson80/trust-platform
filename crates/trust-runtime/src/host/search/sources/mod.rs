//! The sources of a search and where it runs (specification 11, "Sources"). Each protocol has its
//! module: `discover` searches once, within a deadline and through a result sink, and answers a
//! `comm.discover` request or one target of a search; `search` is the protocol's source in the one
//! search. `scope` reads what a protocol search probes, `report` is what it reports, `probe` is
//! what a TCP probe proves, `find` turns candidates into a search's results, and `catalog`
//! decides which requested sources run and why the others are skipped.

pub(crate) mod ads;
pub(crate) mod ads_ports;
mod catalog;
pub(crate) mod ethercat;
mod find;
pub(crate) mod gpio;
mod mdns_services;
pub(crate) mod modbus_tcp;
pub(crate) mod mqtt;
pub(crate) mod opcua;
mod opcua_lds;
mod probe;
pub(crate) mod report;
pub(crate) mod scope;
mod serial;
pub(crate) mod trust_runtimes;
mod usb;

use std::path::PathBuf;
use std::sync::Arc;

use smol_str::SmolStr;

use crate::discovery::DiscoveryState;

use super::model::SearchOrigin;

pub(crate) use catalog::plan;

#[cfg(test)]
mod tests;

/// Where a search runs.
#[derive(Debug, Clone)]
pub enum SearchHost {
    /// On the tool's own computer: the command line or the language server.
    ThisHost,
    /// Inside a runtime, which reaches the networks and buses of its computer.
    Runtime(RuntimeFacts),
}

impl SearchHost {
    /// The search's origin.
    #[must_use]
    pub fn origin(&self) -> SearchOrigin {
        match self {
            Self::ThisHost => SearchOrigin::ThisHost,
            Self::Runtime(_) => SearchOrigin::Runtime,
        }
    }
}

/// What a runtime gives the searches that run inside it.
#[derive(Debug, Clone)]
pub struct RuntimeFacts {
    /// The runtime's project folder, whose `io.toml` names its EtherCAT driver.
    pub project_root: Option<PathBuf>,
    /// The runtime's own mDNS discovery (specification 11, §6.12).
    pub discovery: Arc<DiscoveryState>,
    /// Whether that discovery runs (`[runtime.discovery] enabled`).
    pub discovery_enabled: bool,
    /// The network interfaces it may use (`discovery.interfaces`); empty for every interface.
    pub discovery_interfaces: Vec<SmolStr>,
    /// The runtime's own stable runtime ID; a search skips the runtime itself by it
    /// (specification 11, "One search, device identity, and presence").
    pub runtime_id: SmolStr,
    /// The adapters of its running EtherCAT drivers (specification 11, "Safety"); a search never
    /// scans one of these.
    pub owned_adapters: Vec<SmolStr>,
}

/// What one source reads: the checked request's targets and where the search runs.
#[derive(Debug, Clone)]
pub(crate) struct SourceContext {
    pub(crate) hosts: Vec<String>,
    pub(crate) cidr: Option<String>,
    /// The EtherCAT adapters to scan; `None` is the adapter of the project's EtherCAT driver.
    pub(crate) ethercat_adapters: Vec<Option<String>>,
    /// The Modbus TCP unit IDs to probe; unit ID 1 when the request names none.
    pub(crate) modbus_unit_ids: Vec<u8>,
    /// The ADS ports to probe; the default set when the request names none.
    pub(crate) ads_ports: Vec<u16>,
    /// The runtime the search runs in; `None` on the tool's computer.
    pub(crate) runtime: Option<RuntimeFacts>,
}
