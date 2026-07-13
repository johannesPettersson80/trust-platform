//! Live host-facing ADS CLI operations.

use std::path::PathBuf;

use anyhow::bail;
#[cfg(feature = "ads-wire")]
use anyhow::Context;
#[cfg(feature = "ads-wire")]
use serde::Serialize;
#[cfg(feature = "ads-wire")]
use std::{path::Path, str::FromStr};
#[cfg(feature = "ads-wire")]
use trust_ads_core::{
    AdsDataTypeDescriptor, IecDataType, SymbolDescriptor, SymbolFlag, SymbolSnapshot,
};
#[cfg(feature = "ads-wire")]
use trust_runtime::ads::onboarding::{
    apply_symbol_import, build_symbol_import_response, derive_host_ads_identity,
    directed_broadcast_targets_from_candidates, discover_targets,
    run_doctor as run_onboarding_doctor, runtime_address_candidates_from_interfaces,
    ActiveDeviceStrategy, AdsOnboardingWire, AdsRsOnboardingWire, DiscoveryRequest,
    DoctorCancellation, DoctorOptions, GuardedWriteProbe, IdentityRequest,
    SymbolImportApplyRequest, SymbolImportRequest,
};
#[cfg(feature = "ads-wire")]
use trust_runtime::ads::AdsClientConfig;
#[cfg(feature = "ads-wire")]
use trust_runtime_core::value::Value;

#[cfg(feature = "ads-wire")]
use crate::style;

#[cfg(feature = "ads-wire")]
use super::{
    emit_report, load_config, load_optional_snapshots, read_generated_source,
    render_validate_report, target_identity, validate_ads_interface_offline,
    write_generated_output, AdsValidateReport,
};

#[cfg(feature = "ads-wire")]
#[derive(Debug, Serialize)]
struct AdsBrowseReport {
    config_path: PathBuf,
    connection_count: usize,
    symbol_count: usize,
    connections: Vec<AdsBrowseConnectionReport>,
}

#[cfg(feature = "ads-wire")]
#[derive(Debug, Serialize)]
struct AdsBrowseConnectionReport {
    name: String,
    symbol_count: usize,
    symbols: Vec<SymbolSnapshotSymbolReport>,
}

#[cfg(feature = "ads-wire")]
#[derive(Debug, Serialize)]
struct SymbolSnapshotSymbolReport {
    name: String,
    type_name: String,
    byte_size: u32,
    index_group: u32,
    index_offset: u32,
    flags: Vec<String>,
}

#[cfg(feature = "ads-wire")]
#[derive(Debug, Serialize)]
struct AdsImportSymbolsReport {
    ads_toml_path: PathBuf,
    snapshot_path: PathBuf,
    generated_path: PathBuf,
    connection_name: String,
    candidate_count: usize,
    selected_count: usize,
    ads_toml_bytes: usize,
    snapshot_bytes: usize,
    generated_bytes: usize,
    dry_run: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    previews: Vec<AdsGeneratedFilePreview>,
}

#[cfg(feature = "ads-wire")]
#[derive(Debug, Serialize)]
struct AdsGeneratedFilePreview {
    path: PathBuf,
    kind: &'static str,
    content: String,
    bytes: usize,
    exists: bool,
    changed: bool,
}

pub(super) fn run_discover(
    target: Option<String>,
    target_net_id: Option<String>,
    ams_port: u16,
    no_broadcast: bool,
    json: bool,
) -> anyhow::Result<()> {
    #[cfg(not(feature = "ads-wire"))]
    {
        let _ = (target, target_net_id, ams_port, no_broadcast, json);
        bail!("ADS discover requires trust-runtime built with feature 'ads-wire'");
    }

    #[cfg(feature = "ads-wire")]
    {
        let candidates =
            runtime_address_candidates_from_interfaces().map_err(anyhow::Error::new)?;
        let broadcast_targets = if no_broadcast {
            Vec::new()
        } else {
            directed_broadcast_targets_from_candidates(&candidates)
        };
        let request = DiscoveryRequest {
            target,
            target_ams_net_id: target_net_id,
            ams_port: Some(ams_port),
            target_name: None,
            include_broadcast: !no_broadcast,
            broadcast_targets,
            timeout_ms: None,
        };
        let mut wire = AdsRsOnboardingWire::default();
        let results = discover_targets(&mut wire, &request).map_err(anyhow::Error::new)?;
        emit_report(&results, json, |items| {
            render_discovery_results(items.as_slice())
        })
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn run_doctor(
    target: String,
    target_net_id: Option<String>,
    ams_port: u16,
    write_symbol: Option<String>,
    write_type: Option<String>,
    write_value: Option<String>,
    json: bool,
) -> anyhow::Result<()> {
    #[cfg(not(feature = "ads-wire"))]
    {
        let _ = (
            target,
            target_net_id,
            ams_port,
            write_symbol,
            write_type,
            write_value,
            json,
        );
        bail!("ADS doctor requires trust-runtime built with feature 'ads-wire'");
    }

    #[cfg(feature = "ads-wire")]
    {
        let write_probe = parse_write_probe(write_symbol, write_type, write_value)?;
        let candidates =
            runtime_address_candidates_from_interfaces().map_err(anyhow::Error::new)?;
        let identity_request = IdentityRequest {
            target_ip: target.clone(),
            local_net_id_override: None,
        };
        let local =
            derive_host_ads_identity(&identity_request, candidates).map_err(anyhow::Error::new)?;
        let mut options = DoctorOptions::runtime_host(target, local);
        options.ran_from = trust_runtime::ads::diagnostics::DoctorVantage::CliLocal;
        options.ams_port = ams_port;
        options.active_device_strategy = ActiveDeviceStrategy::ReadOnlyViaLiveStatus;
        if let Some(target_net_id) = target_net_id {
            options = options.with_expected_target_ams_net_id(target_net_id);
        }
        if let Some(write_probe) = write_probe {
            options = options.with_write_probe(write_probe);
        }

        let mut wire = AdsRsOnboardingWire::default();
        let report = run_onboarding_doctor(&mut wire, options, &DoctorCancellation::new());
        emit_report(&report, json, render_doctor_report)
    }
}

pub(super) fn run_browse(
    config_path: PathBuf,
    connection: Option<String>,
    json: bool,
) -> anyhow::Result<()> {
    #[cfg(not(feature = "ads-wire"))]
    {
        let _ = (config_path, connection, json);
        bail!("ADS browse requires trust-runtime built with feature 'ads-wire'");
    }

    #[cfg(feature = "ads-wire")]
    {
        let config = load_config(&config_path)?;
        let selected = selected_connections(&config, connection.as_deref())?;
        let mut connections = Vec::with_capacity(selected.len());
        for connection_config in selected {
            let mut transport =
                trust_runtime::ads::HostAdsTransport::new(connection_config.route.clone());
            transport.connect().with_context(|| {
                format!(
                    "failed to connect ADS connection '{}'",
                    connection_config.route.name
                )
            })?;
            let symbols = match transport.upload_symbol_table() {
                Ok(symbols) => symbols,
                Err(error) => {
                    let _ = transport.disconnect();
                    return Err(anyhow::Error::new(error).context(format!(
                        "failed to browse live ADS symbols for connection '{}'",
                        connection_config.route.name
                    )));
                }
            };
            transport.disconnect().with_context(|| {
                format!(
                    "failed to disconnect ADS connection '{}'",
                    connection_config.route.name
                )
            })?;
            connections.push(AdsBrowseConnectionReport {
                name: connection_config.route.name.clone(),
                symbol_count: symbols.len(),
                symbols: symbols.into_iter().map(symbol_report).collect(),
            });
        }
        let symbol_count = connections
            .iter()
            .map(|connection| connection.symbol_count)
            .sum::<usize>();
        let report = AdsBrowseReport {
            config_path,
            connection_count: connections.len(),
            symbol_count,
            connections,
        };
        emit_report(&report, json, render_browse_report)
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn run_import_symbols(
    target: String,
    target_net_id: Option<String>,
    ams_port: u16,
    connection_name: String,
    name_prefix: Option<String>,
    include_patterns: Vec<String>,
    out: PathBuf,
    snapshot_out: Option<PathBuf>,
    existing_snapshot_paths: Vec<PathBuf>,
    generated: PathBuf,
    force: bool,
    dry_run: bool,
    json: bool,
) -> anyhow::Result<()> {
    #[cfg(not(feature = "ads-wire"))]
    {
        let _ = (
            target,
            target_net_id,
            ams_port,
            connection_name,
            name_prefix,
            include_patterns,
            out,
            snapshot_out,
            existing_snapshot_paths,
            generated,
            force,
            dry_run,
            json,
        );
        bail!("ADS import-symbols requires trust-runtime built with feature 'ads-wire'");
    }

    #[cfg(feature = "ads-wire")]
    {
        let mut wire = AdsRsOnboardingWire::default();
        let mut target_identity = match target_net_id {
            Some(target_net_id) => target_identity(target.clone(), target_net_id, ams_port),
            None => {
                let mut identity = wire
                    .udp_identify(target.as_str())
                    .map_err(anyhow::Error::new)?;
                if identity.ip.is_empty() {
                    identity.ip = target.clone();
                }
                identity.ams_port = ams_port;
                identity
            }
        };
        target_identity.ams_port = ams_port;

        let candidates =
            runtime_address_candidates_from_interfaces().map_err(anyhow::Error::new)?;
        let identity_request = IdentityRequest {
            target_ip: target_identity.ip.clone(),
            local_net_id_override: None,
        };
        let local =
            derive_host_ads_identity(&identity_request, candidates).map_err(anyhow::Error::new)?;

        let symbols = wire
            .upload_symbols(&target_identity)
            .map_err(anyhow::Error::new)?;
        let response = build_symbol_import_response(
            &SymbolImportRequest {
                connection_name: connection_name.clone(),
                symbols: Vec::new(),
                include_patterns,
                name_prefix,
            },
            symbols,
        );
        let candidate_count = response.candidates.len();
        let existing_ads_toml = std::fs::read_to_string(&out).ok();
        let existing_snapshots = load_optional_snapshots(&existing_snapshot_paths)?;
        let artifacts = apply_symbol_import(
            existing_ads_toml.as_deref(),
            SymbolImportApplyRequest {
                response,
                target: target_identity,
                local,
                existing_snapshots,
                write_acknowledged: false,
            },
        )
        .map_err(anyhow::Error::new)?;
        let snapshot_path =
            snapshot_out.unwrap_or_else(|| default_snapshot_path(&out, connection_name.as_str()));
        let imported_snapshot = artifacts
            .snapshots
            .iter()
            .find(|snapshot| snapshot.route_name == connection_name)
            .with_context(|| {
                format!(
                    "imported ADS snapshot for connection '{connection_name}' was not generated"
                )
            })?;
        let snapshot_json = imported_snapshot.to_deterministic_json()?;
        let previews = vec![
            preview_generated_file(&out, "ads_toml", artifacts.ads_toml.clone()),
            preview_generated_file(&snapshot_path, "symbol_snapshot", snapshot_json.clone()),
            preview_generated_file(&generated, "generated_st", artifacts.generated_st.clone()),
        ];
        if !dry_run {
            write_text_file(&out, artifacts.ads_toml.as_str())?;
            write_text_file(&snapshot_path, snapshot_json.as_str())?;
            write_generated_output(&generated, artifacts.generated_st.as_str(), force)?;
        }

        let report = AdsImportSymbolsReport {
            ads_toml_path: out,
            snapshot_path,
            generated_path: generated,
            connection_name,
            candidate_count,
            selected_count: artifacts.selected_count,
            ads_toml_bytes: artifacts.ads_toml.len(),
            snapshot_bytes: snapshot_json.len(),
            generated_bytes: artifacts.generated_st.len(),
            dry_run,
            previews: if dry_run { previews } else { Vec::new() },
        };
        emit_report(&report, json, render_import_symbols_report)
    }
}

pub(super) fn run_validate_live(
    config_path: PathBuf,
    snapshot_paths: Vec<PathBuf>,
    generated_path: PathBuf,
    json: bool,
) -> anyhow::Result<()> {
    if !snapshot_paths.is_empty() {
        bail!("ADS validate --live reads live TwinCAT symbols; remove --snapshot");
    }

    #[cfg(not(feature = "ads-wire"))]
    {
        let _ = (config_path, generated_path, json);
        bail!("ADS validate --live requires trust-runtime built with feature 'ads-wire'");
    }

    #[cfg(feature = "ads-wire")]
    {
        let config = load_config(&config_path)?;
        let snapshots = load_live_snapshots(&config)?;
        let generated_source = read_generated_source(&generated_path)?;
        let report =
            validate_ads_interface_offline(&config, &snapshots, generated_source.as_str())?;
        let symbol_count = snapshots
            .iter()
            .map(|snapshot| snapshot.symbols.len())
            .sum::<usize>();
        let report = AdsValidateReport {
            config_path,
            snapshot_paths: Vec::new(),
            generated_path,
            mode: "live",
            connection_count: snapshots.len(),
            symbol_count,
            point_count: report.point_count,
            generated_bytes: report.generated_bytes,
        };
        emit_report(&report, json, render_validate_report)
    }
}

#[cfg(feature = "ads-wire")]
fn parse_write_probe(
    symbol: Option<String>,
    type_name: Option<String>,
    value: Option<String>,
) -> anyhow::Result<Option<GuardedWriteProbe>> {
    match (symbol, type_name, value) {
        (None, None, None) => Ok(None),
        (Some(symbol), Some(type_name), Some(value)) => {
            let (data_type, value) = parse_write_probe_value(type_name.as_str(), value.as_str())?;
            Ok(Some(GuardedWriteProbe {
                symbol,
                data_type,
                value,
            }))
        }
        _ => bail!(
            "ADS doctor write probe requires --write-symbol, --write-type, and --write-value together"
        ),
    }
}

#[cfg(feature = "ads-wire")]
fn parse_write_probe_value(
    type_name: &str,
    value: &str,
) -> anyhow::Result<(AdsDataTypeDescriptor, Value)> {
    let canonical = type_name.trim().to_ascii_uppercase();
    let parsed = match canonical.as_str() {
        "BOOL" => (
            IecDataType::Bool,
            Value::Bool(match value.trim().to_ascii_lowercase().as_str() {
                "true" | "1" => true,
                "false" | "0" => false,
                _ => bail!("BOOL write value must be true, false, 1, or 0"),
            }),
        ),
        "SINT" => (IecDataType::Sint, Value::SInt(parse_scalar(value, "SINT")?)),
        "INT" => (IecDataType::Int, Value::Int(parse_scalar(value, "INT")?)),
        "DINT" => (IecDataType::Dint, Value::DInt(parse_scalar(value, "DINT")?)),
        "LINT" => (IecDataType::Lint, Value::LInt(parse_scalar(value, "LINT")?)),
        "USINT" => (
            IecDataType::Usint,
            Value::USInt(parse_scalar(value, "USINT")?),
        ),
        "UINT" => (IecDataType::Uint, Value::UInt(parse_scalar(value, "UINT")?)),
        "UDINT" => (
            IecDataType::Udint,
            Value::UDInt(parse_scalar(value, "UDINT")?),
        ),
        "ULINT" => (
            IecDataType::Ulint,
            Value::ULInt(parse_scalar(value, "ULINT")?),
        ),
        "REAL" => (IecDataType::Real, Value::Real(parse_scalar(value, "REAL")?)),
        "LREAL" => (
            IecDataType::Lreal,
            Value::LReal(parse_scalar(value, "LREAL")?),
        ),
        "BYTE" => (IecDataType::Byte, Value::Byte(parse_scalar(value, "BYTE")?)),
        "WORD" => (IecDataType::Word, Value::Word(parse_scalar(value, "WORD")?)),
        "DWORD" => (
            IecDataType::Dword,
            Value::DWord(parse_scalar(value, "DWORD")?),
        ),
        "LWORD" => (
            IecDataType::Lword,
            Value::LWord(parse_scalar(value, "LWORD")?),
        ),
        _ => bail!(
            "ADS doctor write probe supports scalar BOOL, integer, REAL/LREAL, and BYTE/WORD/DWORD/LWORD types"
        ),
    };
    Ok((
        AdsDataTypeDescriptor::scalar(canonical.as_str(), parsed.0),
        parsed.1,
    ))
}

#[cfg(feature = "ads-wire")]
fn parse_scalar<T>(value: &str, type_name: &str) -> anyhow::Result<T>
where
    T: FromStr,
    T::Err: std::fmt::Display,
{
    value.trim().parse::<T>().map_err(|error| {
        anyhow::anyhow!(
            "invalid {type_name} write value '{}': {error}",
            value.trim()
        )
    })
}

#[cfg(feature = "ads-wire")]
fn load_live_snapshots(config: &AdsClientConfig) -> anyhow::Result<Vec<SymbolSnapshot>> {
    let mut snapshots = Vec::with_capacity(config.connections.len());
    for connection in &config.connections {
        let mut transport = trust_runtime::ads::HostAdsTransport::new(connection.route.clone());
        transport.connect().with_context(|| {
            format!(
                "failed to connect ADS connection '{}'",
                connection.route.name
            )
        })?;
        let symbols = match transport.upload_symbol_table() {
            Ok(symbols) => symbols,
            Err(error) => {
                let _ = transport.disconnect();
                return Err(anyhow::Error::new(error).context(format!(
                    "failed to upload live ADS symbols for connection '{}'",
                    connection.route.name
                )));
            }
        };
        transport.disconnect().with_context(|| {
            format!(
                "failed to disconnect ADS connection '{}'",
                connection.route.name
            )
        })?;
        snapshots.push(SymbolSnapshot::new(connection.route.name.clone(), symbols));
    }
    Ok(snapshots)
}

#[cfg(feature = "ads-wire")]
fn selected_connections<'a>(
    config: &'a AdsClientConfig,
    connection: Option<&str>,
) -> anyhow::Result<Vec<&'a trust_runtime::ads::AdsConnectionConfig>> {
    match connection {
        Some(name) => {
            let connection = config
                .connections
                .iter()
                .find(|candidate| candidate.route.name == name)
                .with_context(|| format!("ADS connection '{name}' is not defined in ads.toml"))?;
            Ok(vec![connection])
        }
        None => Ok(config.connections.iter().collect()),
    }
}

#[cfg(feature = "ads-wire")]
fn symbol_report(symbol: SymbolDescriptor) -> SymbolSnapshotSymbolReport {
    SymbolSnapshotSymbolReport {
        type_name: symbol_type_label(&symbol),
        byte_size: symbol.byte_size,
        index_group: symbol.index_group,
        index_offset: symbol.index_offset,
        flags: symbol.flags.into_iter().map(symbol_flag_label).collect(),
        name: symbol.name,
    }
}

#[cfg(feature = "ads-wire")]
fn symbol_type_label(symbol: &SymbolDescriptor) -> String {
    let mut label = symbol.data_type.source_name.clone();
    if !symbol.data_type.dimensions.is_empty() {
        let dimensions = symbol
            .data_type
            .dimensions
            .iter()
            .map(|dimension| format!("{}..{}", dimension.lower, dimension.upper))
            .collect::<Vec<_>>()
            .join(", ");
        label = format!("ARRAY[{dimensions}] OF {label}");
    }
    label
}

#[cfg(feature = "ads-wire")]
fn symbol_flag_label(flag: SymbolFlag) -> String {
    match flag {
        SymbolFlag::Read => "read",
        SymbolFlag::Write => "write",
        SymbolFlag::Persistent => "persistent",
        SymbolFlag::Retain => "retain",
    }
    .to_string()
}

#[cfg(feature = "ads-wire")]
fn write_text_file(path: &Path, source: &str) -> anyhow::Result<()> {
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    std::fs::write(path, source).with_context(|| format!("failed to write {}", path.display()))
}

#[cfg(feature = "ads-wire")]
fn preview_generated_file(
    path: &Path,
    kind: &'static str,
    content: String,
) -> AdsGeneratedFilePreview {
    let existing = std::fs::read_to_string(path).ok();
    let changed = existing.as_deref() != Some(content.as_str());
    AdsGeneratedFilePreview {
        path: path.to_path_buf(),
        kind,
        bytes: content.len(),
        exists: existing.is_some(),
        changed,
        content,
    }
}

#[cfg(feature = "ads-wire")]
fn default_snapshot_path(ads_toml_path: &Path, connection_name: &str) -> PathBuf {
    let root = ads_toml_path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    root.join("ads")
        .join("snapshots")
        .join(format!("{connection_name}.symbols.json"))
}

#[cfg(feature = "ads-wire")]
fn render_import_symbols_report(report: &AdsImportSymbolsReport) -> String {
    let action = if report.dry_run {
        "Previewed"
    } else {
        "Imported"
    };
    format!(
        "{}\nADS config: {}\nSnapshot: {}\nGenerated ST: {}\nCandidate symbol(s): {}\nSelected symbol(s): {}\n",
        style::success(format!(
            "{action} ADS symbols for connection '{}'",
            report.connection_name
        )),
        report.ads_toml_path.display(),
        report.snapshot_path.display(),
        report.generated_path.display(),
        report.candidate_count,
        report.selected_count
    )
}

#[cfg(feature = "ads-wire")]
fn render_browse_report(report: &AdsBrowseReport) -> String {
    let mut output = format!(
        "{}\nADS config: {}\nADS connection(s): {}\nADS symbol(s): {}\n",
        style::success("Browsed live ADS symbol table"),
        report.config_path.display(),
        report.connection_count,
        report.symbol_count
    );
    for connection in &report.connections {
        output.push_str(&format!(
            "\n[{}] {} symbol(s)\n",
            connection.name, connection.symbol_count
        ));
        for symbol in &connection.symbols {
            let flags = if symbol.flags.is_empty() {
                "-".to_string()
            } else {
                symbol.flags.join(",")
            };
            output.push_str(&format!(
                "  {} : {} ({} byte(s), IG 0x{:X}, IO 0x{:X}, {})\n",
                symbol.name,
                symbol.type_name,
                symbol.byte_size,
                symbol.index_group,
                symbol.index_offset,
                flags
            ));
        }
    }
    output
}

#[cfg(feature = "ads-wire")]
fn render_discovery_results(results: &[trust_runtime::ads::onboarding::DiscoveryResult]) -> String {
    if results.is_empty() {
        return format!("{}\n", style::warning("No ADS targets discovered."));
    }
    let mut output = format!("{}\n", style::success("Discovered ADS target(s):"));
    for result in results {
        let target = &result.target;
        let name = target.name.as_deref().unwrap_or("unnamed");
        let services = if target.responding_ads_ports.is_empty() {
            "identity only; no responding user ADS service".to_string()
        } else {
            format!(
                "responding ADS service(s) {}",
                target
                    .responding_ads_ports
                    .iter()
                    .map(u16::to_string)
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        };
        output.push_str(&format!(
            "- {name}: {} ({}) — {services} via {:?}\n",
            target.ip, target.ams_net_id, result.source
        ));
    }
    output
}

#[cfg(feature = "ads-wire")]
fn render_doctor_report(report: &trust_runtime::ads::diagnostics::DoctorReport) -> String {
    let mut output = format!("ADS doctor: {:?}\n{}\n", report.overall, report.summary);
    for step in &report.steps {
        output.push_str(&format!(
            "- {:?}: {:?} - {}\n",
            step.id, step.status, step.detail
        ));
        if !step.remediation.is_empty() {
            output.push_str(&format!("  remediation: {}\n", step.remediation));
        }
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(feature = "ads-wire")]
    #[test]
    fn discovery_text_reports_only_observed_ads_services() {
        use trust_runtime::ads::onboarding::{
            DiscoveryResult, DiscoverySource, ObservedAdsIdentity,
        };

        let output = render_discovery_results(&[
            DiscoveryResult {
                target: ObservedAdsIdentity {
                    name: Some("Controller".to_string()),
                    ip: "192.0.2.10".to_string(),
                    ams_net_id: "192.0.2.10.1.1".to_string(),
                    preferred_ams_port: Some(851),
                    responding_ads_ports: vec![851, 301],
                    tc_version: None,
                },
                source: DiscoverySource::LocalRouter,
            },
            DiscoveryResult {
                target: ObservedAdsIdentity {
                    name: Some("Identity-only controller".to_string()),
                    ip: "192.0.2.20".to_string(),
                    ams_net_id: "192.0.2.20.1.1".to_string(),
                    preferred_ams_port: None,
                    responding_ads_ports: Vec::new(),
                    tc_version: None,
                },
                source: DiscoverySource::DirectedIdentify,
            },
        ]);

        assert!(output.contains("responding ADS service(s) 851, 301"));
        assert!(output.contains("identity only; no responding user ADS service"));
        assert!(!output.contains("port 851 via"));
    }
}
