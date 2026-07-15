//! Beckhoff ADS command handlers.

use std::path::{Path, PathBuf};
#[cfg(feature = "ads-wire")]
use std::str::FromStr;

use anyhow::{bail, Context};
use serde::Serialize;
use serde_json::json;
use trust_ads_core::SymbolSnapshot;
#[cfg(feature = "ads-wire")]
use trust_ads_core::{AdsDataTypeDescriptor, IecDataType};
#[cfg(feature = "ads-wire")]
use trust_ads_core::{SymbolDescriptor, SymbolFlag};
#[cfg(feature = "ads-wire")]
use trust_runtime::ads::onboarding::GuardedWriteProbe;
#[cfg(feature = "ads-wire")]
use trust_runtime::ads::onboarding::{
    add_route_with_channel_policy, apply_symbol_import, build_symbol_import_response,
    derive_runtime_identity_from_source, directed_broadcast_targets_from_candidates,
    discover_targets, interface_directed_targets, resolve_os_source_ip,
    run_doctor as run_onboarding_doctor, runtime_address_candidates_from_interfaces,
    ActiveDeviceStrategy, AdsOnboardingWire, AdsRsOnboardingWire, DiscoveryRequest,
    DoctorCancellation, DoctorOptions, IdentityRequest, RouteAddRequest, RouteCredentials,
    SymbolImportApplyRequest, SymbolImportRequest,
};
use trust_runtime::ads::onboarding::{
    build_route_plan, build_route_remove_artifact, classify_local_address, RoutePlanRequest,
    RoutePlanRole,
};
use trust_runtime::ads::{
    diagnostics::{
        CredentialChannelClassification, LocalIdentity, LocalNetworkClassification, RouteArtifact,
        RouteArtifactKind, TargetIdentity,
    },
    generate_ads_interface, parse_ads_toml, validate_ads_interface_offline, AdsClientConfig,
};
#[cfg(feature = "ads-wire")]
use trust_runtime_core::value::Value;

use crate::cli::{AdsAction, AdsRouteArtifactFormat, AdsServerAction};
use crate::style;

#[derive(Debug, Serialize)]
struct AdsImportReport {
    config_path: PathBuf,
    snapshot_paths: Vec<PathBuf>,
    output_path: PathBuf,
    point_count: usize,
    bytes_written: usize,
    changed: bool,
}

#[derive(Debug, Serialize)]
struct AdsValidateReport {
    config_path: PathBuf,
    snapshot_paths: Vec<PathBuf>,
    generated_path: PathBuf,
    mode: &'static str,
    connection_count: usize,
    symbol_count: usize,
    point_count: usize,
    generated_bytes: usize,
}

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

#[derive(Debug, Serialize)]
struct AdsRouteScriptReport {
    route_name: String,
    automatic_route: trust_runtime::ads::diagnostics::RouteActionAvailability,
    artifact: RouteArtifact,
}

#[cfg(feature = "ads-wire")]
#[derive(Debug, Serialize)]
struct AdsAddRouteReport {
    route_name: String,
    target_ip: String,
    target_net_id: String,
    local_ip: String,
    local_net_id: String,
    channel: CredentialChannelClassification,
    status: &'static str,
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

pub fn run_ads(action: AdsAction) -> anyhow::Result<()> {
    match action {
        AdsAction::Discover {
            target,
            target_net_id,
            ams_port,
            no_broadcast,
            json,
        } => run_discover(target, target_net_id, ams_port, no_broadcast, json),
        AdsAction::Doctor {
            target,
            target_net_id,
            ams_port,
            write_symbol,
            write_type,
            write_value,
            json,
        } => run_doctor(
            target,
            target_net_id,
            ams_port,
            write_symbol,
            write_type,
            write_value,
            json,
        ),
        AdsAction::Browse {
            config,
            connection,
            json,
        } => run_browse(config, connection, json),
        AdsAction::RouteScript {
            route_name,
            target,
            target_net_id,
            ams_port,
            local_ip,
            local_net_id,
            format,
            json,
        } => run_route_script(
            route_name,
            target,
            target_net_id,
            ams_port,
            local_ip,
            local_net_id,
            format,
            json,
        ),
        AdsAction::AddRoute {
            route_name,
            target,
            target_net_id,
            ams_port,
            local_ip,
            local_net_id,
            username,
            password_stdin,
            json,
        } => run_add_route(
            route_name,
            target,
            target_net_id,
            ams_port,
            local_ip,
            local_net_id,
            username,
            password_stdin,
            json,
        ),
        AdsAction::RouteRemove { route_name, json } => run_route_remove(route_name, json),
        AdsAction::ImportSymbols {
            target,
            target_net_id,
            ams_port,
            connection,
            name_prefix,
            include_patterns,
            out,
            snapshot_out,
            existing_snapshots,
            generated,
            force,
            dry_run,
            json,
        } => run_import_symbols(
            target,
            target_net_id,
            ams_port,
            connection,
            name_prefix,
            include_patterns,
            out,
            snapshot_out,
            existing_snapshots,
            generated,
            force,
            dry_run,
            json,
        ),
        AdsAction::Import {
            config,
            snapshots,
            output,
            force,
            json,
        } => run_import(config, snapshots, output, force, json),
        AdsAction::Validate {
            offline,
            live,
            config,
            snapshots,
            generated,
            json,
        } => run_validate(offline, live, config, snapshots, generated, json),
        AdsAction::Server { action } => run_ads_server(action),
    }
}

fn run_ads_server(action: AdsServerAction) -> anyhow::Result<()> {
    match action {
        AdsServerAction::Status {
            project,
            endpoint,
            token,
            json,
        } => run_server_control(project, endpoint, token, "ads.server.status", None, json),
        AdsServerAction::Symbols {
            project,
            endpoint,
            token,
            json,
        } => run_server_control(project, endpoint, token, "ads.server.symbols", None, json),
        AdsServerAction::Doctor {
            project,
            endpoint,
            token,
            external_kind,
            external_name,
            json,
        } => {
            let params = server_doctor_params(external_kind, external_name)?;
            run_server_control(
                project,
                endpoint,
                token,
                "ads.server.doctor",
                Some(params),
                json,
            )
        }
        AdsServerAction::RouteScript {
            route_name,
            server_ip,
            server_net_id,
            ads_port,
            format,
            json,
        } => run_server_route_script(route_name, server_ip, server_net_id, ads_port, format, json),
    }
}

fn run_server_control(
    project: Option<PathBuf>,
    endpoint: Option<String>,
    token: Option<String>,
    kind: &'static str,
    params: Option<serde_json::Value>,
    json_output: bool,
) -> anyhow::Result<()> {
    let target = crate::ctl::resolve_control_target(project, endpoint, token)?;
    let request = json!({
        "id": 1,
        "type": kind,
        "auth": target.auth_token,
        "params": params.unwrap_or_else(|| json!({})),
    });
    let response = crate::ctl::send_control_request_value(&target.endpoint, &request)?;
    if json_output {
        println!("{}", serde_json::to_string_pretty(&response)?);
        return Ok(());
    }
    let ok = response
        .get("ok")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false);
    if !ok {
        let error = response
            .get("error")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("ADS server control request failed");
        bail!("{error}");
    }
    let result = response
        .get("result")
        .cloned()
        .unwrap_or(serde_json::Value::Null);
    println!("{}", render_server_control_result(kind, &result));
    Ok(())
}

fn server_doctor_params(
    external_kind: Option<String>,
    external_name: Option<String>,
) -> anyhow::Result<serde_json::Value> {
    match (external_kind, external_name) {
        (None, None) => Ok(json!({})),
        (Some(kind), Some(name)) => Ok(json!({
            "external_client": {
                "kind": kind,
                "name": name,
                "timestamp_ms": unix_now_ms(),
            }
        })),
        _ => bail!("ADS server doctor external proof requires --external-kind and --external-name together"),
    }
}

fn run_server_route_script(
    route_name: String,
    server_ip: String,
    server_net_id: String,
    ads_port: u16,
    format: AdsRouteArtifactFormat,
    json_output: bool,
) -> anyhow::Result<()> {
    let plan = build_route_plan(RoutePlanRequest {
        role: RoutePlanRole::Server,
        route_name: route_name.clone(),
        target: target_identity(server_ip.clone(), server_net_id.clone(), ads_port),
        local: local_identity(server_ip, server_net_id),
        channel: CredentialChannelClassification::LocalCliDirectAddRoute,
    });
    let kind = artifact_kind_for_format(format);
    let artifact = plan
        .artifacts
        .iter()
        .find(|artifact| artifact.kind == kind)
        .cloned()
        .with_context(|| format!("ADS server route artifact {kind:?} was not generated"))?;
    let report = AdsRouteScriptReport {
        route_name,
        automatic_route: plan.automatic_route,
        artifact,
    };
    emit_report(&report, json_output, render_route_script_report)
}

fn run_discover(
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
        let directed_targets = if target.is_none() {
            interface_directed_targets(candidates.iter().map(|candidate| candidate.ip.as_str()))
        } else {
            Vec::new()
        };
        let broadcast_targets = if no_broadcast {
            Vec::new()
        } else {
            directed_broadcast_targets_from_candidates(&candidates)
        };
        let request = DiscoveryRequest {
            target,
            directed_targets,
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

fn run_doctor(
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
        let source_ip = resolve_os_source_ip(target.as_str()).map_err(anyhow::Error::new)?;
        let nic = candidates
            .iter()
            .find(|candidate| candidate.ip == source_ip)
            .and_then(|candidate| candidate.nic.clone());
        let identity_request = IdentityRequest {
            target_ip: target.clone(),
            local_net_id_override: None,
        };
        let local = derive_runtime_identity_from_source(
            &identity_request,
            source_ip,
            None,
            nic,
            candidates,
        )
        .map_err(anyhow::Error::new)?;
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

fn run_browse(config_path: PathBuf, connection: Option<String>, json: bool) -> anyhow::Result<()> {
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
                trust_runtime::ads::HostAdsClient::new(connection_config.route.clone());
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

#[allow(clippy::too_many_arguments)]
fn run_route_script(
    route_name: String,
    target_ip: String,
    target_net_id: String,
    ams_port: u16,
    local_ip: String,
    local_net_id: String,
    format: AdsRouteArtifactFormat,
    json: bool,
) -> anyhow::Result<()> {
    let plan = build_route_plan(RoutePlanRequest {
        role: RoutePlanRole::Client,
        route_name: route_name.clone(),
        target: target_identity(target_ip, target_net_id, ams_port),
        local: local_identity(local_ip, local_net_id),
        channel: CredentialChannelClassification::LocalCliDirectAddRoute,
    });
    let kind = artifact_kind_for_format(format);
    let artifact = plan
        .artifacts
        .iter()
        .find(|artifact| artifact.kind == kind)
        .cloned()
        .with_context(|| format!("ADS route artifact {kind:?} was not generated"))?;
    let report = AdsRouteScriptReport {
        route_name,
        automatic_route: plan.automatic_route,
        artifact,
    };
    emit_report(&report, json, render_route_script_report)
}

#[allow(clippy::too_many_arguments)]
fn run_add_route(
    route_name: String,
    target_ip: String,
    target_net_id: String,
    ams_port: u16,
    local_ip: String,
    local_net_id: String,
    username: String,
    password_stdin: bool,
    json: bool,
) -> anyhow::Result<()> {
    if !password_stdin {
        bail!(
            "ADS add-route requires --password-stdin; password argv is intentionally unsupported"
        );
    }

    #[cfg(not(feature = "ads-wire"))]
    {
        let _ = (
            route_name,
            target_ip,
            target_net_id,
            ams_port,
            local_ip,
            local_net_id,
            username,
            json,
        );
        bail!("ADS add-route requires trust-runtime built with feature 'ads-wire'");
    }

    #[cfg(feature = "ads-wire")]
    {
        use trust_runtime::ads::onboarding::AdsRsOnboardingWire;

        let mut password = String::new();
        {
            use std::io::Read as _;
            std::io::stdin()
                .read_to_string(&mut password)
                .context("failed to read ADS route password from stdin")?;
        }
        let password = password.trim_end_matches(&['\r', '\n'][..]).to_string();
        if password.is_empty() {
            bail!("ADS add-route received an empty password on stdin");
        }

        let target = target_identity(target_ip, target_net_id, ams_port);
        let local = local_identity(local_ip, local_net_id);
        let request = RouteAddRequest {
            route_name: route_name.clone(),
            target: target.clone(),
            local: local.clone(),
            credentials: RouteCredentials { username, password },
        };
        let channel = CredentialChannelClassification::LocalCliDirectAddRoute;
        let mut wire = AdsRsOnboardingWire::default();
        add_route_with_channel_policy(&mut wire, &request, channel).map_err(anyhow::Error::new)?;
        let report = AdsAddRouteReport {
            route_name,
            target_ip: target.ip,
            target_net_id: target.ams_net_id,
            local_ip: local.chosen_ip,
            local_net_id: local.ams_net_id,
            channel,
            status: "added",
        };
        emit_report(&report, json, render_add_route_report)
    }
}

fn run_route_remove(route_name: String, json: bool) -> anyhow::Result<()> {
    let artifact = build_route_remove_artifact(route_name.as_str());
    emit_report(&artifact, json, render_route_remove_artifact)
}

#[allow(clippy::too_many_arguments)]
fn run_import_symbols(
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
        let source_ip =
            resolve_os_source_ip(target_identity.ip.as_str()).map_err(anyhow::Error::new)?;
        let nic = candidates
            .iter()
            .find(|candidate| candidate.ip == source_ip)
            .and_then(|candidate| candidate.nic.clone());
        let identity_request = IdentityRequest {
            target_ip: target_identity.ip.clone(),
            local_net_id_override: None,
        };
        let local = derive_runtime_identity_from_source(
            &identity_request,
            source_ip,
            None,
            nic,
            candidates,
        )
        .map_err(anyhow::Error::new)?;

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

fn run_import(
    config_path: PathBuf,
    snapshot_paths: Vec<PathBuf>,
    output_path: PathBuf,
    force: bool,
    json: bool,
) -> anyhow::Result<()> {
    let config = load_config(&config_path)?;
    let snapshots = load_snapshots(&snapshot_paths)?;
    let generated = generate_ads_interface(&config, &snapshots)?;
    let changed = write_generated_output(&output_path, generated.source.as_str(), force)?;
    let report = AdsImportReport {
        config_path,
        snapshot_paths,
        output_path,
        point_count: generated.point_count,
        bytes_written: generated.source.len(),
        changed,
    };
    emit_report(&report, json, render_import_report)
}

fn run_validate(
    offline: bool,
    live: bool,
    config_path: PathBuf,
    snapshot_paths: Vec<PathBuf>,
    generated_path: PathBuf,
    json: bool,
) -> anyhow::Result<()> {
    if !offline && !live {
        bail!("ADS validate requires --offline or --live");
    }
    if live {
        return run_validate_live(config_path, snapshot_paths, generated_path, json);
    }
    if snapshot_paths.is_empty() {
        bail!("ADS validate --offline requires at least one --snapshot");
    }
    let config = load_config(&config_path)?;
    let snapshots = load_snapshots(&snapshot_paths)?;
    let generated_source = read_generated_source(&generated_path)?;
    let report = validate_ads_interface_offline(&config, &snapshots, generated_source.as_str())?;
    let symbol_count = snapshots
        .iter()
        .map(|snapshot| snapshot.symbols.len())
        .sum::<usize>();
    let report = AdsValidateReport {
        config_path,
        snapshot_paths,
        generated_path,
        mode: "offline",
        connection_count: snapshots.len(),
        symbol_count,
        point_count: report.point_count,
        generated_bytes: report.generated_bytes,
    };
    emit_report(&report, json, render_validate_report)
}

fn run_validate_live(
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
fn load_live_snapshots(config: &AdsClientConfig) -> anyhow::Result<Vec<SymbolSnapshot>> {
    let mut snapshots = Vec::with_capacity(config.connections.len());
    for connection in &config.connections {
        let mut transport = trust_runtime::ads::HostAdsClient::new(connection.route.clone());
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

fn read_generated_source(path: &Path) -> anyhow::Result<String> {
    std::fs::read_to_string(path).with_context(|| format!("failed to read {}", path.display()))
}

fn load_config(path: &Path) -> anyhow::Result<AdsClientConfig> {
    let text = std::fs::read_to_string(path)
        .with_context(|| format!("failed to read ADS config {}", path.display()))?;
    parse_ads_toml(text.as_str()).map_err(anyhow::Error::new)
}

fn load_snapshots(paths: &[PathBuf]) -> anyhow::Result<Vec<SymbolSnapshot>> {
    if paths.is_empty() {
        bail!("at least one ADS symbol snapshot is required");
    }
    load_optional_snapshots(paths)
}

fn load_optional_snapshots(paths: &[PathBuf]) -> anyhow::Result<Vec<SymbolSnapshot>> {
    paths
        .iter()
        .map(|path| {
            let text = std::fs::read_to_string(path)
                .with_context(|| format!("failed to read ADS snapshot {}", path.display()))?;
            let mut snapshot: SymbolSnapshot = serde_json::from_str(text.as_str())
                .with_context(|| format!("failed to parse ADS snapshot {}", path.display()))?;
            snapshot.canonicalize();
            Ok(snapshot)
        })
        .collect()
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

fn write_generated_output(path: &Path, source: &str, force: bool) -> anyhow::Result<bool> {
    if let Ok(existing) = std::fs::read_to_string(path) {
        if existing == source {
            return Ok(false);
        }
        if !force {
            bail!(
                "generated ADS output '{}' already exists with different content; rerun with --force to overwrite",
                path.display()
            );
        }
    }
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    std::fs::write(path, source).with_context(|| format!("failed to write {}", path.display()))?;
    Ok(true)
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

fn emit_report<T>(
    report: &T,
    json: bool,
    render_human: impl FnOnce(&T) -> String,
) -> anyhow::Result<()>
where
    T: Serialize,
{
    if json {
        println!("{}", serde_json::to_string_pretty(report)?);
    } else {
        print!("{}", render_human(report));
    }
    Ok(())
}

fn render_import_report(report: &AdsImportReport) -> String {
    let status = if report.changed { "Wrote" } else { "Verified" };
    format!(
        "{}\nADS point(s): {}\nGenerated bytes: {}\n",
        style::success(format!("{status} {}", report.output_path.display())),
        report.point_count,
        report.bytes_written
    )
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

fn render_validate_report(report: &AdsValidateReport) -> String {
    let source = if report.mode == "live" {
        "live ADS symbol table"
    } else {
        "cached ADS snapshot(s)"
    };
    format!(
        "{}\nMode: {}\nADS connection(s): {}\nCompatible symbol(s): {}\nADS point(s): {}\nGenerated bytes: {}\n",
        style::success(format!(
            "Validated {} against {source}",
            report.generated_path.display(),
        )),
        report.mode,
        report.connection_count,
        report.symbol_count,
        report.point_count,
        report.generated_bytes
    )
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
        output.push_str(&format!(
            "- {name}: {} ({}) port {} via {:?}\n",
            target.ip, target.ams_net_id, target.ams_port, result.source
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

fn render_route_script_report(report: &AdsRouteScriptReport) -> String {
    let heading = if let Some(filename) = report.artifact.filename.as_ref() {
        style::success(format!("Generated ADS route artifact {filename}"))
    } else {
        style::success("Generated ADS route artifact")
    };
    format!("{heading}\n{}", report.artifact.content)
}

fn render_route_remove_artifact(artifact: &RouteArtifact) -> String {
    artifact.content.clone()
}

fn render_server_control_result(kind: &str, result: &serde_json::Value) -> String {
    match kind {
        "ads.server.status" => {
            let overall = result
                .pointer("/status/overall")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("unknown");
            let summary = result
                .pointer("/status/summary")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("ADS server status returned.");
            let identity = result
                .pointer("/identity/ams_net_id")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("unconfigured");
            let exposed = result
                .get("exposed_count")
                .and_then(serde_json::Value::as_u64)
                .unwrap_or(0);
            let writable = result
                .get("writable_count")
                .and_then(serde_json::Value::as_u64)
                .unwrap_or(0);
            format!(
                "{}\nIdentity: {}\nExposed symbols: {}\nWritable symbols: {}\n{}\n",
                style::success(format!("ADS server status: {overall}")),
                identity,
                exposed,
                writable,
                summary
            )
        }
        "ads.server.symbols" => {
            let route_name = result
                .get("route_name")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("ads-server");
            let symbol_count = result
                .get("symbols")
                .and_then(serde_json::Value::as_array)
                .map_or(0, Vec::len);
            format!(
                "{}\nSymbol table: {}\nSymbol(s): {}\n",
                style::success("ADS server symbols"),
                route_name,
                symbol_count
            )
        }
        "ads.server.doctor" => {
            let overall = result
                .get("overall")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("unknown");
            let summary = result
                .get("summary")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("ADS server doctor completed.");
            format!(
                "{}\n{}\n",
                style::success(format!("ADS server doctor: {overall}")),
                summary
            )
        }
        _ => format!(
            "{}\n",
            serde_json::to_string_pretty(result).unwrap_or_default()
        ),
    }
}

#[cfg(feature = "ads-wire")]
fn render_add_route_report(report: &AdsAddRouteReport) -> String {
    format!(
        "{}\nRoute: {}\nTarget: {} ({})\nRuntime host: {} ({})\n",
        style::success("Added ADS route"),
        report.route_name,
        report.target_ip,
        report.target_net_id,
        report.local_ip,
        report.local_net_id
    )
}

fn unix_now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .min(u128::from(u64::MAX)) as u64
}

fn artifact_kind_for_format(format: AdsRouteArtifactFormat) -> RouteArtifactKind {
    match format {
        AdsRouteArtifactFormat::Powershell => RouteArtifactKind::Powershell,
        AdsRouteArtifactFormat::Staticroutes => RouteArtifactKind::StaticRoutesXml,
        AdsRouteArtifactFormat::Gui => RouteArtifactKind::ManualSteps,
        AdsRouteArtifactFormat::RemovalPowershell => RouteArtifactKind::RemovalPowershell,
    }
}

fn target_identity(ip: String, ams_net_id: String, ams_port: u16) -> TargetIdentity {
    TargetIdentity {
        name: None,
        ip,
        ams_net_id,
        ams_port,
        tc_version: None,
    }
}

fn local_identity(chosen_ip: String, ams_net_id: String) -> LocalIdentity {
    let classification = classify_local_address(chosen_ip.as_str(), None);
    LocalIdentity {
        host_name: None,
        chosen_ip,
        ams_net_id,
        nic: None,
        candidates: Vec::new(),
        classification: if matches!(classification, LocalNetworkClassification::Unknown) {
            LocalNetworkClassification::Lan
        } else {
            classification
        },
    }
}

#[cfg(test)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use trust_ads_core::{AdsDataTypeDescriptor, IecDataType, SymbolDescriptor, SymbolFlag};

    use super::*;

    #[test]
    fn ads_import_writes_and_validates_generated_source_offline() {
        let root = unique_temp_dir("ads-import");
        let config = root.join("ads.toml");
        let snapshot = root.join("ads/snapshots/line1.symbols.json");
        let output = root.join("src/generated/ads_generated.st");
        write_fixture(&config, &snapshot);

        run_ads(AdsAction::Import {
            config: config.clone(),
            snapshots: vec![snapshot.clone()],
            output: output.clone(),
            force: false,
            json: true,
        })
        .expect("import");
        assert!(std::fs::read_to_string(&output)
            .expect("generated source")
            .contains("line1_temp : REAL;"));

        run_ads(AdsAction::Validate {
            offline: true,
            live: false,
            config,
            snapshots: vec![snapshot],
            generated: output,
            json: true,
        })
        .expect("offline validate");

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn ads_import_refuses_to_overwrite_changed_source_without_force() {
        let root = unique_temp_dir("ads-import-overwrite");
        let config = root.join("ads.toml");
        let snapshot = root.join("ads/snapshots/line1.symbols.json");
        let output = root.join("src/generated/ads_generated.st");
        write_fixture(&config, &snapshot);
        std::fs::create_dir_all(output.parent().expect("output parent")).expect("mkdir");
        std::fs::write(&output, "manually edited\n").expect("write changed output");

        let error = run_ads(AdsAction::Import {
            config,
            snapshots: vec![snapshot],
            output,
            force: false,
            json: false,
        })
        .expect_err("changed output requires force");

        assert!(error.to_string().contains("--force"));
        let _ = std::fs::remove_dir_all(root);
    }

    fn write_fixture(config_path: &Path, snapshot_path: &Path) {
        if let Some(parent) = config_path.parent() {
            std::fs::create_dir_all(parent).expect("config parent");
        }
        if let Some(parent) = snapshot_path.parent() {
            std::fs::create_dir_all(parent).expect("snapshot parent");
        }
        std::fs::write(
            config_path,
            r#"
[[connections]]
name = "line1"
target_net_id = "5.23.91.12.1.1"
host = "192.168.10.5"
transport = "plain"
insecure_transport = true

[[connections.points]]
symbol = "MAIN.Temperature"
var = "line1_temp"
type = "REAL"
"#,
        )
        .expect("write config");
        let snapshot = SymbolSnapshot::new(
            "line1",
            vec![SymbolDescriptor::new(
                "MAIN.Temperature",
                AdsDataTypeDescriptor::scalar("REAL", IecDataType::Real),
                0x4020,
                0,
                4,
            )
            .with_flag(SymbolFlag::Read)],
        );
        std::fs::write(
            snapshot_path,
            snapshot.to_deterministic_json().expect("snapshot json"),
        )
        .expect("write snapshot");
    }

    fn unique_temp_dir(prefix: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        std::env::temp_dir().join(format!("trust-runtime-{prefix}-{nanos}"))
    }
}
