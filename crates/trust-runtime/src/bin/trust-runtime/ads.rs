//! Beckhoff ADS command handlers.

use std::path::{Path, PathBuf};

use crate::cli::{AdsAction, AdsRouteArtifactFormat, AdsServerAction};
use crate::style;
use anyhow::{bail, Context};
use serde::Serialize;
use serde_json::json;
use trust_ads_core::SymbolSnapshot;
#[cfg(feature = "ads-wire")]
use trust_runtime::ads::onboarding::{
    add_route_with_channel_policy, RouteAddRequest, RouteCredentials,
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

#[path = "ads/host.rs"]
mod host;

use host::{run_browse, run_discover, run_doctor, run_import_symbols, run_validate_live};

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
