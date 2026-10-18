//! EtherCAT (specification 11, "comm.discover" and "Sources"): the SubDevices on the adapter that
//! the scope names, or on the adapter of the project's EtherCAT driver. The simulated bus (`mock`)
//! answers without hardware. The bus is scanned only inside a runtime, on the computer with the
//! adapter: `comm.discover` and the search check that before they ask. Identification maps the
//! process data in PRE-OP and never brings the bus to OP, so no output changes.

use std::path::Path;

use serde_json::json;
use smol_str::SmolStr;

use crate::error::RuntimeError;

use super::super::model::{DeviceKey, EthercatIdentity, Identity, ProtocolFind, SourceId};
use super::super::runner::SourceSink;
use super::find::{find, number, text, FindSink};
use super::report::{CandidateSink, Deadline, DiscoverCandidate};
use super::scope::DiscoverScope;
use super::SourceContext;

/// The adapter of an EtherCAT driver that names none: the simulated bus.
pub(crate) const MOCK_ETHERCAT_ADAPTER: &str = "mock";

/// The marker in `RuntimeFacts::owned_adapters` that this runtime has running EtherCAT drivers whose
/// adapters could not be determined (specification 11, "Safety"): a search then scans no EtherCAT
/// adapter and reports a warning saying why. No network adapter is named this.
pub(crate) const UNKNOWN_ADAPTER_OWNER: &str = "*";

/// One SubDevice that an identification read (specification 11, "Identity"): the fields a find
/// carries. The source's own shape, so a test can supply literal values.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct EthercatIdentifiedSubDevice {
    pub(super) slot: u16,
    pub(super) configured_address: u16,
    pub(super) alias: u16,
    pub(super) model: String,
    pub(super) channels: u16,
    pub(super) vendor_id: u32,
    pub(super) product_id: u32,
    pub(super) revision: u32,
    pub(super) serial: u32,
}

/// The SubDevices of one adapter and the bus's mapped process data lengths (specification 11,
/// "Identity" and "Sources").
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct EthercatIdentification {
    pub(super) subdevices: Vec<EthercatIdentifiedSubDevice>,
    pub(super) input_bytes: usize,
    pub(super) output_bytes: usize,
}

/// The seams of the EtherCAT source (specification 11, "Sources"): the adapters of this computer
/// and one adapter's identification. A test replaces both.
pub(super) struct EthercatSeams<'a> {
    /// The adapters of this computer, up and not loopback, that a search scans when the request
    /// names none.
    pub(super) local_adapters: &'a dyn Fn() -> Vec<SmolStr>,
    /// The SubDevices of one adapter, from its parsed parameters.
    pub(super) identify: &'a dyn Fn(&toml::Value) -> Result<EthercatIdentification, RuntimeError>,
}

impl Default for EthercatSeams<'_> {
    fn default() -> Self {
        Self {
            local_adapters: &local_ethercat_adapters,
            identify: &identify_ethercat,
        }
    }
}

/// What an EtherCAT search does with one adapter (specification 11, "Sources" and "Safety").
#[derive(Debug, PartialEq, Eq)]
pub(super) enum AdapterPlan {
    /// Scan the adapter.
    Scan(SmolStr),
    /// Do not touch it: a running EtherCAT driver of this runtime owns it.
    Refused(SmolStr),
}

/// Why a search does not scan an adapter: a running EtherCAT driver of this runtime owns it
/// (specification 11, "Safety").
fn refused_warning(adapter: &str) -> String {
    format!(
        "EtherCAT adapter '{adapter}' is owned by a running EtherCAT driver; its live view is reported instead."
    )
}

/// Why a search scans no EtherCAT adapter: the adapters of this runtime's running EtherCAT drivers
/// could not be determined, so no bus can be ruled out (specification 11, "Safety").
fn unknown_owner_warning() -> String {
    "EtherCAT adapter ownership could not be determined for this runtime's running EtherCAT drivers; no EtherCAT adapter is scanned."
        .to_string()
}

/// Scans the adapter of `scope`, or of the EtherCAT driver in the `io.toml` of `project_root`.
pub(crate) fn discover(
    scope: &DiscoverScope,
    project_root: Option<&Path>,
    deadline: Deadline,
    sink: &mut dyn CandidateSink,
) -> Result<(), String> {
    discover_with(
        scope,
        project_root,
        deadline,
        sink,
        &EthercatSeams::default(),
    )
}

fn discover_with(
    scope: &DiscoverScope,
    project_root: Option<&Path>,
    deadline: Deadline,
    sink: &mut dyn CandidateSink,
    seams: &EthercatSeams<'_>,
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
    match (seams.identify)(&params) {
        Ok(identification) => {
            let adapter = ethercat_adapter(&params);
            let source = if adapter.eq_ignore_ascii_case(MOCK_ETHERCAT_ADAPTER) {
                "ethercat_mock"
            } else {
                "ethercat_bus"
            };
            let input_bytes = identification.input_bytes;
            let output_bytes = identification.output_bytes;
            for subdevice in identification.subdevices {
                sink.found(DiscoverCandidate {
                    id: format!("ethercat:slot:{}", subdevice.slot),
                    label: format!("{} (slot {})", subdevice.model, subdevice.slot),
                    source,
                    confidence: "confirmed",
                    params: json!({
                        "adapter": adapter.as_str(),
                        "model": subdevice.model,
                        "slot": subdevice.slot,
                        "channels": subdevice.channels,
                        "configured_address": subdevice.configured_address,
                        "alias": subdevice.alias,
                        "vendor_id": subdevice.vendor_id,
                        "product_id": subdevice.product_id,
                        "revision": subdevice.revision,
                        "serial": subdevice.serial,
                        "input_bytes": input_bytes,
                        "output_bytes": output_bytes,
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
    project_ethercat_params(project_root)
}

/// The parameters of the EtherCAT driver in the `io.toml` of `project_root`, when it has one.
fn project_ethercat_params(project_root: Option<&Path>) -> Result<Option<toml::Value>, String> {
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
        .find(|driver| is_ethercat_driver(driver.name.as_str()))
        .map(|driver| driver.params))
}

/// Whether a configured driver name is this source's protocol (specification 11, "comm.discover"):
/// its name trimmed, in lower case, with `-` as `_`.
pub(crate) fn is_ethercat_driver(name: &str) -> bool {
    name.trim().to_ascii_lowercase().replace('-', "_") == "ethercat"
}

/// The adapter that EtherCAT parameters name (specification 11, "comm.discover"): their `adapter`,
/// or the simulated bus when they name none.
fn ethercat_adapter(params: &toml::Value) -> SmolStr {
    params
        .get("adapter")
        .and_then(toml::Value::as_str)
        .map(str::trim)
        .filter(|adapter| !adapter.is_empty())
        .map_or_else(|| SmolStr::new(MOCK_ETHERCAT_ADAPTER), SmolStr::new)
}

/// The SubDevices of one adapter, read by the production host seam. The identification never
/// exchanges process data and reports the simulated bus as simulated.
fn identify_ethercat(params: &toml::Value) -> Result<EthercatIdentification, RuntimeError> {
    let discovery = crate::io::discover_ethercat_modules(params)?;
    Ok(EthercatIdentification {
        subdevices: discovery
            .subdevices
            .into_iter()
            .map(|subdevice| EthercatIdentifiedSubDevice {
                slot: subdevice.slot,
                configured_address: subdevice.configured_address,
                alias: subdevice.alias_address,
                model: subdevice.model,
                channels: subdevice.channels,
                vendor_id: subdevice.vendor_id,
                product_id: subdevice.product_id,
                revision: subdevice.revision,
                serial: subdevice.serial,
            })
            .collect(),
        input_bytes: discovery.input_bytes,
        output_bytes: discovery.output_bytes,
    })
}

/// The adapters of this computer that a search scans when the request names none (specification 11,
/// "Sources"): every network interface that is up and not loopback.
#[cfg(all(feature = "ethercat-wire", unix))]
fn local_ethercat_adapters() -> Vec<SmolStr> {
    pnet_datalink::interfaces()
        .into_iter()
        .filter(|interface| interface.is_up() && !interface.is_loopback())
        .map(|interface| SmolStr::new(interface.name))
        .collect()
}

/// Without the EtherCAT wire transport no adapter can be opened, so no adapter is scanned.
#[cfg(not(all(feature = "ethercat-wire", unix)))]
fn local_ethercat_adapters() -> Vec<SmolStr> {
    Vec::new()
}

/// EtherCAT in a search: each requested adapter, or the adapter of the project's EtherCAT driver,
/// or the adapters of this computer when the request names none.
pub(super) fn search(context: &SourceContext, sink: &SourceSink) {
    let seams = EthercatSeams::default();
    let project_root = context
        .runtime
        .as_ref()
        .and_then(|runtime| runtime.project_root.as_deref());
    let plans = match adapter_plans(context, project_root, &seams) {
        Ok(plans) => plans,
        Err(error) => {
            sink.warn(error);
            return;
        }
    };
    for plan in plans {
        if sink.ended() {
            return;
        }
        match plan {
            AdapterPlan::Refused(adapter) => sink.warn(refused_warning(&adapter)),
            AdapterPlan::Scan(adapter) => {
                let scope = DiscoverScope {
                    adapter: Some(adapter.to_string()),
                    ..DiscoverScope::default()
                };
                let mut found = FindSink::new(sink, Some(adapter.as_str()), ethercat_find);
                if let Err(error) =
                    discover_with(&scope, project_root, found.deadline(), &mut found, &seams)
                {
                    sink.warn(error);
                }
            }
        }
    }
}

/// The adapters a search scans (specification 11, "Sources"): every requested adapter, the project
/// driver's, or this computer's own when the request names none.
///
/// An adapter that a running EtherCAT driver of this runtime owns is refused with a warning naming
/// it and is never scanned (specification 11, "Safety"). When those owners cannot be determined,
/// no EtherCAT adapter is scanned at all and a warning says why.
fn adapter_plans(
    context: &SourceContext,
    project_root: Option<&Path>,
    seams: &EthercatSeams<'_>,
) -> Result<Vec<AdapterPlan>, String> {
    let owned = context
        .runtime
        .as_ref()
        .map(|runtime| runtime.owned_adapters.as_slice())
        .unwrap_or_default();
    if owned.iter().any(|owned| owned == UNKNOWN_ADAPTER_OWNER) {
        return Err(unknown_owner_warning());
    }
    let mut plans = Vec::new();
    for requested in &context.ethercat_adapters {
        let adapters: Vec<SmolStr> = match requested {
            Some(adapter) => vec![SmolStr::new(adapter.as_str())],
            None => match project_ethercat_params(project_root)? {
                Some(params) => vec![ethercat_adapter(&params)],
                None => (seams.local_adapters)(),
            },
        };
        for adapter in adapters {
            if owned
                .iter()
                .any(|owned| owned.eq_ignore_ascii_case(adapter.as_str()))
            {
                plans.push(AdapterPlan::Refused(adapter));
            } else {
                plans.push(AdapterPlan::Scan(adapter));
            }
        }
    }
    Ok(plans)
}

fn ethercat_find(candidate: DiscoverCandidate) -> ProtocolFind {
    let adapter =
        text(&candidate.params, "adapter").unwrap_or_else(|| MOCK_ETHERCAT_ADAPTER.to_string());
    let slot = number(&candidate.params, "slot");
    let identity = Identity::Ethercat(EthercatIdentity {
        adapter: adapter.clone(),
        slot,
        configured_address: number(&candidate.params, "configured_address"),
        alias: number(&candidate.params, "alias"),
        model: text(&candidate.params, "model").unwrap_or_default(),
        channels: number(&candidate.params, "channels"),
        vendor_id: number(&candidate.params, "vendor_id"),
        product_id: number(&candidate.params, "product_id"),
        revision: number(&candidate.params, "revision"),
        serial: number(&candidate.params, "serial"),
        input_bytes: number(&candidate.params, "input_bytes"),
        output_bytes: number(&candidate.params, "output_bytes"),
    });
    let simulated = adapter.eq_ignore_ascii_case(MOCK_ETHERCAT_ADAPTER);
    find(SourceId::Ethercat, candidate, identity)
        .simulated(simulated)
        .keyed(
            format!("{adapter}:{slot}"),
            vec![DeviceKey::Local(format!("ethercat:{adapter}:{slot}"))],
        )
}

#[cfg(test)]
#[path = "ethercat_tests.rs"]
mod ethercat_tests;
