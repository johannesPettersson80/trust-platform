//! EtherCAT (specification 11, "comm.discover" and "Sources"): the SubDevices on the adapter that
//! the scope names, or on the adapter of the project's EtherCAT driver. The simulated bus (`mock`)
//! answers without hardware. The bus is scanned only inside a runtime, on the computer with the
//! adapter: `comm.discover` and the search check that before they ask.

use std::path::Path;

use serde_json::json;

use super::super::model::{DeviceKey, EthercatIdentity, Identity, ProtocolFind, SourceId};
use super::super::runner::SourceSink;
use super::find::{find, number, text, FindSink};
use super::report::{CandidateSink, Deadline, DiscoverCandidate};
use super::scope::DiscoverScope;
use super::SourceContext;

/// The adapter of an EtherCAT driver that names none: the simulated bus.
const MOCK_ETHERCAT_ADAPTER: &str = "mock";

/// Scans the adapter of `scope`, or of the EtherCAT driver in the `io.toml` of `project_root`.
pub(crate) fn discover(
    scope: &DiscoverScope,
    project_root: Option<&Path>,
    deadline: Deadline,
    sink: &mut dyn CandidateSink,
) -> Result<(), String> {
    if deadline.passed() {
        return Ok(());
    }
    let Some(params) = ethercat_discovery_params(scope, project_root)? else {
        sink.warn(
            "EtherCAT discovery needs scope.adapter/scope.host or a configured EtherCAT driver in the runtime project."
                .to_string(),
        );
        return Ok(());
    };
    match crate::io::discover_ethercat_modules(&params) {
        Ok(discovery) => {
            for module in discovery.modules {
                sink.found(DiscoverCandidate {
                    id: format!("ethercat:slot:{}", module.slot),
                    label: format!("{} (slot {})", module.model, module.slot),
                    source: "ethercat_bus",
                    confidence: "observed",
                    params: json!({
                        "adapter": params.get("adapter").and_then(toml::Value::as_str),
                        "model": module.model,
                        "slot": module.slot,
                        "channels": module.channels,
                    }),
                    warnings: Vec::new(),
                });
            }
        }
        Err(error) => sink.warn(format!("EtherCAT discovery did not complete: {error}")),
    }
    Ok(())
}

fn ethercat_discovery_params(
    scope: &DiscoverScope,
    project_root: Option<&Path>,
) -> Result<Option<toml::Value>, String> {
    if let Some(adapter) = scope
        .adapter
        .as_deref()
        .or(scope.host.as_deref())
        .map(str::trim)
        .filter(|adapter| !adapter.is_empty())
    {
        let mut table = toml::map::Map::new();
        table.insert(
            "adapter".to_string(),
            toml::Value::String(adapter.to_string()),
        );
        return Ok(Some(toml::Value::Table(table)));
    }
    let Some(root) = project_root else {
        return Ok(None);
    };
    let path = root.join("io.toml");
    if !path.is_file() {
        return Ok(None);
    }
    let config = crate::config::IoConfig::load(path)
        .map_err(|error| format!("failed to load io.toml for EtherCAT discovery: {error}"))?;
    Ok(config
        .drivers
        .into_iter()
        // The driver name as `comm.discover` reads a protocol: trimmed, in lower case, `-` as `_`.
        .find(|driver| driver.name.trim().to_ascii_lowercase().replace('-', "_") == "ethercat")
        .map(|driver| driver.params))
}

/// EtherCAT in a search: each requested adapter, or the adapter of the project's EtherCAT driver.
pub(super) fn search(context: &SourceContext, sink: &SourceSink) {
    let project_root = context
        .runtime
        .as_ref()
        .and_then(|runtime| runtime.project_root.as_deref());
    for adapter in &context.ethercat_adapters {
        if sink.ended() {
            return;
        }
        let scope = DiscoverScope {
            adapter: adapter.clone(),
            ..DiscoverScope::default()
        };
        let mut found = FindSink::new(sink, adapter.as_deref(), ethercat_find);
        if let Err(error) = discover(&scope, project_root, found.deadline(), &mut found) {
            sink.warn(error);
        }
    }
}

fn ethercat_find(candidate: DiscoverCandidate) -> ProtocolFind {
    let adapter =
        text(&candidate.params, "adapter").unwrap_or_else(|| MOCK_ETHERCAT_ADAPTER.to_string());
    let slot = number(&candidate.params, "slot");
    let identity = Identity::Ethercat(EthercatIdentity {
        adapter: adapter.clone(),
        slot,
        model: text(&candidate.params, "model").unwrap_or_default(),
        channels: number(&candidate.params, "channels"),
    });
    let simulated = adapter.eq_ignore_ascii_case(MOCK_ETHERCAT_ADAPTER);
    find(SourceId::Ethercat, candidate, identity)
        .simulated(simulated)
        .keyed(
            format!("{adapter}:{slot}"),
            vec![DeviceKey::Local(format!("ethercat:{adapter}:{slot}"))],
        )
}
