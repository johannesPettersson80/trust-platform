//! The sources of a search and where it runs (specification 11, "Sources"). `catalog` decides
//! which requested sources run and why the others are skipped; `adapters` runs each source over
//! today's per-protocol searches of `comm.discover`, until V7 slice C1b moves those searches into
//! this folder.

mod adapters;
mod catalog;

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
}

/// What one source reads: the checked request's targets and where the search runs.
#[derive(Debug, Clone)]
pub(crate) struct SourceContext {
    pub(crate) hosts: Vec<String>,
    pub(crate) cidr: Option<String>,
    /// The EtherCAT adapters to scan; `None` is the adapter of the project's EtherCAT driver.
    pub(crate) ethercat_adapters: Vec<Option<String>>,
    /// The runtime the search runs in; `None` on the tool's computer.
    pub(crate) runtime: Option<RuntimeFacts>,
}
