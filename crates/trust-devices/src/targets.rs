//! The runtimes that the panel asks (design §3.3, I1 and I3): the runtime of the project's
//! control endpoint, the simulator of the debug session, the runtimes that this computer manages
//! for the chosen project, and the endpoints added in the settings. Each endpoint is one target,
//! so no runtime is asked twice in one gather.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use trust_runtime::config::fleet_manifest::FleetManifestRuntime;
use trust_runtime_client::TargetProbe;

use crate::request::{HostFacts, RunMode, Token};

/// Where a target comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum TargetOrigin {
    /// The runtime of `trust.runtime.controlEndpoint`, in online mode.
    Project,
    /// The simulator of the debug session.
    Simulator,
    /// A runtime that this computer manages, from the chosen project's `fleet.toml`
    /// (specification 25 §5.11).
    Managed,
    /// An endpoint of `trust.runtime.fleetEndpoints`.
    Configured,
}

/// What names a target from one gather to the next.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TargetKey {
    /// Where the target comes from.
    pub origin: TargetOrigin,
    /// Its control endpoint.
    pub endpoint: String,
    /// A managed runtime's name.
    pub managed: Option<String>,
}

/// A runtime that a gather asks.
#[derive(Debug, Clone)]
pub struct Target {
    /// Where the target comes from.
    pub origin: TargetOrigin,
    /// Its control endpoint, trimmed.
    pub endpoint: String,
    /// Whether the endpoint is in use; a disabled endpoint is never contacted.
    pub enabled: bool,
    /// The token sent with each request. A managed runtime's token is read from its
    /// `runtime.toml` when it is asked.
    pub token: Option<Token>,
    /// The name that the user gave the endpoint.
    pub label: Option<String>,
    /// For a managed runtime, its fleet root and its entry there.
    pub managed: Option<ManagedTarget>,
    /// Whether to read the runtime's recent events: only for the selected runtime (decision O27).
    pub events: bool,
}

/// A managed runtime: its fleet root, which is the chosen project's folder, and its entry in that
/// folder's `fleet.toml`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManagedTarget {
    /// The fleet root.
    pub fleet_root: PathBuf,
    /// The runtime's entry in `fleet.toml`.
    pub runtime: FleetManifestRuntime,
}

impl Target {
    /// The target's key.
    #[must_use]
    pub fn key(&self) -> TargetKey {
        TargetKey {
            origin: self.origin,
            endpoint: self.endpoint.clone(),
            managed: self
                .managed
                .as_ref()
                .map(|managed| managed.runtime.name.clone()),
        }
    }

    fn at(origin: TargetOrigin, endpoint: &str, token: Option<Token>, host: &HostFacts) -> Self {
        Self {
            origin,
            endpoint: endpoint.to_string(),
            enabled: true,
            token,
            label: host.label(endpoint).map(str::to_string),
            managed: None,
            events: false,
        }
    }
}

/// The targets of one gather, in this order:
///
/// 1. In online mode, the runtime of the project's control endpoint. A blank or disabled endpoint
///    stays a target and is never contacted.
/// 2. The simulator, while a debug session exists and the extension set the simulator's endpoint.
/// 3. Each runtime of `managed`, the `fleet.toml` of the fleet root `fleet_root`.
/// 4. Each additional endpoint: trimmed, without blank and repeated ones, and without the
///    project's and the simulator's endpoints, in any mode, as before.
#[must_use]
pub fn panel_targets(
    host: &HostFacts,
    fleet_root: Option<&Path>,
    managed: &[FleetManifestRuntime],
) -> Vec<Target> {
    let settings = &host.settings;
    let primary = settings.control_endpoint.trim();
    let mut targets = Vec::new();
    if settings.mode == RunMode::Online {
        let mut project = Target::at(
            TargetOrigin::Project,
            primary,
            host.token(primary).cloned(),
            host,
        );
        project.enabled = settings.use_control_endpoint;
        targets.push(project);
    }
    let simulator = host
        .simulator
        .as_ref()
        .filter(|simulator| host.debug_session.is_some() && !simulator.endpoint.trim().is_empty());
    if let Some(simulator) = simulator {
        targets.push(Target::at(
            TargetOrigin::Simulator,
            simulator.endpoint.trim(),
            simulator.token.clone(),
            host,
        ));
    }
    if let Some(fleet_root) = fleet_root {
        for runtime in managed {
            let mut target = Target::at(
                TargetOrigin::Managed,
                runtime.control_endpoint.trim(),
                None,
                host,
            );
            target.managed = Some(ManagedTarget {
                fleet_root: fleet_root.to_path_buf(),
                runtime: runtime.clone(),
            });
            targets.push(target);
        }
    }
    let mut covered = BTreeSet::from([primary]);
    if let Some(simulator) = simulator {
        covered.insert(simulator.endpoint.trim());
    }
    for endpoint in &settings.additional_endpoints {
        let endpoint = endpoint.trim();
        if !endpoint.is_empty() && covered.insert(endpoint) {
            targets.push(Target::at(
                TargetOrigin::Configured,
                endpoint,
                host.token(endpoint).cloned(),
                host,
            ));
        }
    }
    targets
}

/// Which target serves the panel's live data (design §3.3 I1): the runtime of the project's
/// control endpoint when it answers `status`, otherwise the simulator when it answers `status`
/// with its token. A simulator that refuses the token or does not answer is not adopted, so a
/// debug session alone never makes the panel reachable.
#[must_use]
pub fn live_primary(
    project: Option<&TargetProbe>,
    simulator: Option<&TargetProbe>,
) -> Option<TargetOrigin> {
    if matches!(project, Some(TargetProbe::Reachable(_))) {
        Some(TargetOrigin::Project)
    } else if matches!(simulator, Some(TargetProbe::Reachable(_))) {
        Some(TargetOrigin::Simulator)
    } else {
        None
    }
}

#[cfg(test)]
mod tests;
