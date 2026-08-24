use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};
use trust_ads_core::{PointAccess, SymbolFlag, SymbolSnapshot};

use crate::ads::diagnostics::{
    CredentialChannelClassification, LocalIdentity, RoutePlan, TargetIdentity,
};
use crate::ads::onboarding::{
    apply_symbol_import, build_route_plan, build_symbol_import_response,
    upload_failure_implies_missing_return_route, OnboardingWireError, RoutePlanRequest,
    RoutePlanRole, SymbolImportApplyRequest, SymbolImportRequest,
};

use super::super::{ControlResponse, ControlState};

mod artifacts;

use artifacts::{
    display_path, preflight_generated_output, read_optional_project_file, write_project_file,
    ImportArtifactPaths,
};

pub(in crate::control) fn handle_ads_import_symbols(
    id: u64,
    params: Option<serde_json::Value>,
    _state: &ControlState,
) -> ControlResponse {
    let params: ImportSymbolsControlParams = match params {
        Some(value) => match serde_json::from_value(value) {
            Ok(parsed) => parsed,
            Err(error) => return ControlResponse::error(id, format!("invalid params: {error}")),
        },
        None => return ControlResponse::error(id, "missing params".into()),
    };

    let request = SymbolImportRequest {
        connection_name: params.connection_name.clone(),
        symbols: params.symbols.clone(),
        include_patterns: params.include_patterns,
        name_prefix: params.name_prefix,
    };
    let symbols = if let Some(mut snapshot) = params.snapshot {
        snapshot.canonicalize();
        if snapshot.route_name != request.connection_name {
            return ControlResponse::error(
                id,
                format!(
                    "snapshot route '{}' does not match import connection '{}'",
                    snapshot.route_name, request.connection_name
                ),
            );
        }
        snapshot.symbols
    } else {
        let Some(target) = params.target else {
            return ControlResponse::error(
                id,
                "ads.import_symbols requires either a cached snapshot or a live target".to_string(),
            );
        };
        match upload_live_symbols(&target) {
            Ok(symbols) => symbols,
            Err(error) => return ControlResponse::error(id, error.to_string()),
        }
    };

    let response = build_symbol_import_response(&request, symbols);
    match serde_json::to_value(response) {
        Ok(value) => ControlResponse::ok(id, value),
        Err(error) => ControlResponse::error(
            id,
            format!("ADS import-symbols serialization failed: {error}"),
        ),
    }
}

pub(in crate::control) fn handle_ads_import_symbols_apply(
    id: u64,
    params: Option<serde_json::Value>,
    state: &ControlState,
) -> ControlResponse {
    let params: ImportSymbolsApplyControlParams = match params {
        Some(value) => match serde_json::from_value(value) {
            Ok(parsed) => parsed,
            Err(error) => return ControlResponse::error(id, format!("invalid params: {error}")),
        },
        None => return ControlResponse::error(id, "missing params".into()),
    };
    let Some(project_root) = state.project_root.as_ref() else {
        return ControlResponse::error(
            id,
            "ads.import_symbols.apply requires a runtime project_root".to_string(),
        );
    };
    match apply_import_to_project(project_root, params) {
        Ok(report) => match serde_json::to_value(report) {
            Ok(value) => ControlResponse::ok(id, value),
            Err(error) => ControlResponse::error(
                id,
                format!("ADS import-symbols apply serialization failed: {error}"),
            ),
        },
        Err(error) => ControlResponse::error(id, error),
    }
}

#[cfg(feature = "ads-wire")]
fn upload_live_symbols(
    target: &TargetIdentity,
) -> Result<Vec<trust_ads_core::SymbolDescriptor>, OnboardingWireError> {
    let mut wire = crate::ads::onboarding::AdsRsOnboardingWire::default();
    crate::ads::onboarding::AdsOnboardingWire::upload_symbols(&mut wire, target)
}

#[cfg(not(feature = "ads-wire"))]
fn upload_live_symbols(
    _target: &TargetIdentity,
) -> Result<Vec<trust_ads_core::SymbolDescriptor>, OnboardingWireError> {
    Err(OnboardingWireError::new(
        crate::ads::onboarding::OnboardingWireErrorKind::UnsupportedOperation,
        "ADS live symbol import needs an ads-wire build or a cached snapshot",
    ))
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ImportSymbolsControlParams {
    connection_name: String,
    #[serde(default)]
    symbols: Vec<String>,
    #[serde(default)]
    include_patterns: Vec<String>,
    #[serde(default)]
    name_prefix: Option<String>,
    #[serde(default)]
    target: Option<TargetIdentity>,
    #[serde(default)]
    snapshot: Option<SymbolSnapshot>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ImportSymbolsApplyControlParams {
    connection_name: String,
    #[serde(default)]
    symbols: Vec<SelectedSymbol>,
    #[serde(default)]
    include_patterns: Vec<String>,
    #[serde(default)]
    name_prefix: Option<String>,
    #[serde(default)]
    target: Option<TargetIdentity>,
    #[serde(default)]
    local: Option<LocalIdentity>,
    #[serde(default)]
    credential_channel: Option<CredentialChannelClassification>,
    #[serde(default)]
    snapshot: Option<SymbolSnapshot>,
    #[serde(default)]
    write_acknowledged: bool,
    #[serde(default)]
    ads_toml_path: Option<String>,
    #[serde(default)]
    snapshot_path: Option<String>,
    #[serde(default)]
    generated_path: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum SelectedSymbol {
    Name(String),
    Object {
        #[serde(default)]
        path: Option<String>,
        #[serde(default)]
        name: Option<String>,
    },
}

#[derive(Debug, Serialize)]
struct ImportSymbolsApplyReport {
    schema_version: u32,
    applied: bool,
    lifecycle_effect: &'static str,
    connection_name: String,
    selected_count: usize,
    candidate_count: usize,
    ads_toml_path: String,
    snapshot_path: String,
    generated_path: String,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    route: Option<serde_json::Value>,
}

fn apply_import_to_project(
    project_root: &Path,
    params: ImportSymbolsApplyControlParams,
) -> Result<ImportSymbolsApplyReport, String> {
    let artifact_paths = ImportArtifactPaths::resolve(project_root, &params)?;
    let target = params
        .target
        .clone()
        .ok_or_else(|| "ads.import_symbols.apply requires target identity".to_string())?;
    let local = match params.local {
        Some(local) => local,
        None => derive_local_identity_for_apply(&target)?,
    };
    let channel = params
        .credential_channel
        .unwrap_or(CredentialChannelClassification::TrustedSameHost);
    let route_plan =
        build_import_route_plan(params.connection_name.as_str(), &target, &local, channel);
    let symbols = selected_symbol_names(params.symbols)?;
    if symbols.is_empty() && params.include_patterns.is_empty() {
        return Err(
            "ads.import_symbols.apply requires symbols[] or include_patterns[] for selection"
                .to_string(),
        );
    }
    let mut snapshot = if let Some(mut snapshot) = params.snapshot {
        snapshot.canonicalize();
        if snapshot.route_name != params.connection_name {
            return Err(format!(
                "snapshot route '{}' does not match import connection '{}'",
                snapshot.route_name, params.connection_name
            ));
        }
        snapshot
    } else {
        let symbols = match upload_live_symbols(&target) {
            Ok(symbols) => symbols,
            Err(error) if upload_failure_implies_missing_return_route(&error) => {
                return Ok(route_missing_apply_report(
                    params.connection_name,
                    symbols.len(),
                    error.to_string(),
                    route_plan,
                    &artifact_paths,
                ));
            }
            Err(error) => return Err(format!("ADS symbol upload failed: {error}")),
        };
        SymbolSnapshot::new(params.connection_name.clone(), symbols)
    };
    snapshot.canonicalize();

    let request = SymbolImportRequest {
        connection_name: params.connection_name.clone(),
        symbols,
        include_patterns: params.include_patterns,
        name_prefix: params.name_prefix,
    };
    let mut response = build_symbol_import_response(&request, snapshot.symbols.clone());
    for candidate in response
        .candidates
        .iter_mut()
        .filter(|candidate| candidate.selected)
    {
        let readable = candidate.descriptor.flags.contains(&SymbolFlag::Read);
        let writable = candidate.descriptor.flags.contains(&SymbolFlag::Write);
        candidate.access = import_access(readable, writable, params.write_acknowledged);
    }
    let candidate_count = response.candidates.len();
    let existing_ads_toml = read_optional_project_file(&artifact_paths.ads_toml)?;
    let existing_snapshots = load_project_snapshots(project_root)?;
    preflight_generated_output(
        &artifact_paths.generated,
        existing_ads_toml.as_deref(),
        &existing_snapshots,
    )?;
    let artifacts = apply_symbol_import(
        existing_ads_toml.as_deref(),
        SymbolImportApplyRequest {
            response,
            target,
            local,
            existing_snapshots,
            write_acknowledged: params.write_acknowledged,
        },
    )
    .map_err(|error| error.to_string())?;
    let imported_snapshot = artifacts
        .snapshots
        .iter()
        .find(|snapshot| snapshot.route_name == params.connection_name)
        .ok_or_else(|| {
            format!(
                "imported ADS snapshot for connection '{}' was not generated",
                params.connection_name
            )
        })?;
    let snapshot_json = imported_snapshot
        .to_deterministic_json()
        .map_err(|error| format!("serialize ADS snapshot: {error}"))?;
    write_project_file(&artifact_paths.ads_toml, artifacts.ads_toml.as_str())?;
    write_project_file(&artifact_paths.snapshot, snapshot_json.as_str())?;
    write_project_file(&artifact_paths.generated, artifacts.generated_st.as_str())?;
    Ok(ImportSymbolsApplyReport {
        schema_version: 1,
        applied: true,
        lifecycle_effect: "restart_required",
        connection_name: params.connection_name,
        selected_count: artifacts.selected_count,
        candidate_count,
        ads_toml_path: display_path(&artifact_paths.ads_toml),
        snapshot_path: display_path(&artifact_paths.snapshot),
        generated_path: display_path(&artifact_paths.generated),
        message:
            "Selected ADS symbols were written to ads.toml, cached snapshot, and generated ST."
                .to_string(),
        route: None,
    })
}

fn build_import_route_plan(
    connection_name: &str,
    target: &TargetIdentity,
    local: &LocalIdentity,
    channel: CredentialChannelClassification,
) -> RoutePlan {
    build_route_plan(RoutePlanRequest {
        role: RoutePlanRole::Client,
        route_name: connection_name.to_string(),
        target: target.clone(),
        local: local.clone(),
        channel,
    })
}

fn route_missing_apply_report(
    connection_name: String,
    selected_count: usize,
    detail: String,
    route_plan: RoutePlan,
    paths: &ImportArtifactPaths,
) -> ImportSymbolsApplyReport {
    ImportSymbolsApplyReport {
        schema_version: 1,
        applied: false,
        lifecycle_effect: "blocked",
        connection_name,
        selected_count,
        candidate_count: 0,
        ads_toml_path: display_path(&paths.ads_toml),
        snapshot_path: display_path(&paths.snapshot),
        generated_path: display_path(&paths.generated),
        message: "ADS route is not ready; create or fix the route before adding tags.".to_string(),
        route: Some(serde_json::json!({
            "status": "missing",
            "detail": detail,
            "action": "ads.route_plan",
            "route_plan": route_plan,
        })),
    }
}

fn selected_symbol_names(symbols: Vec<SelectedSymbol>) -> Result<Vec<String>, String> {
    let mut names = Vec::new();
    for symbol in symbols {
        let value = match symbol {
            SelectedSymbol::Name(value) => value,
            SelectedSymbol::Object { path, name } => path.or(name).unwrap_or_default(),
        };
        let trimmed = value.trim();
        if trimmed.is_empty() {
            return Err("symbols[] entries must include a non-empty path or name".to_string());
        }
        names.push(trimmed.to_string());
    }
    names.sort();
    names.dedup();
    Ok(names)
}

fn import_access(readable: bool, writable: bool, write_acknowledged: bool) -> PointAccess {
    if !write_acknowledged || !writable {
        PointAccess::Read
    } else if readable {
        PointAccess::ReadWrite
    } else {
        PointAccess::Write
    }
}

fn load_project_snapshots(project_root: &Path) -> Result<Vec<SymbolSnapshot>, String> {
    let root = project_root.join("ads").join("snapshots");
    let entries = match fs::read_dir(&root) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(format!("failed to read {}: {error}", root.display())),
    };
    let mut snapshots = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|error| {
            format!(
                "failed to read snapshot entry in {}: {error}",
                root.display()
            )
        })?;
        let path = entry.path();
        if !path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.ends_with(".symbols.json"))
        {
            continue;
        }
        let text = fs::read_to_string(&path)
            .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
        let mut snapshot: SymbolSnapshot = serde_json::from_str(text.as_str())
            .map_err(|error| format!("failed to parse {}: {error}", path.display()))?;
        snapshot.canonicalize();
        snapshots.push(snapshot);
    }
    Ok(snapshots)
}

#[cfg(feature = "ads-wire")]
fn derive_local_identity_for_apply(target: &TargetIdentity) -> Result<LocalIdentity, String> {
    use std::net::{SocketAddr, ToSocketAddrs};

    use crate::ads::onboarding::{
        derive_runtime_identity_from_source, resolve_os_source_ip,
        runtime_address_candidates_from_interfaces, IdentityRequest,
    };

    let resolved_ip = if target.ip.parse::<std::net::IpAddr>().is_ok() {
        target.ip.clone()
    } else {
        (target.ip.as_str(), 48898)
            .to_socket_addrs()
            .map_err(|error| format!("resolve ADS target '{}': {error}", target.ip))?
            .map(|addr: SocketAddr| addr.ip().to_string())
            .next()
            .ok_or_else(|| format!("resolve ADS target '{}': no address found", target.ip))?
    };
    let candidates = runtime_address_candidates_from_interfaces()
        .map_err(|error| format!("enumerate local interfaces for ADS import: {error}"))?;
    let source_ip = resolve_os_source_ip(resolved_ip.as_str())
        .map_err(|error| format!("resolve local ADS source IP: {error}"))?;
    let nic = candidates
        .iter()
        .find(|candidate| candidate.ip == source_ip)
        .and_then(|candidate| candidate.nic.clone());
    derive_runtime_identity_from_source(
        &IdentityRequest {
            target_ip: resolved_ip,
            local_net_id_override: None,
        },
        source_ip,
        None,
        nic,
        candidates,
    )
    .map_err(|error| error.to_string())
}

#[cfg(not(feature = "ads-wire"))]
fn derive_local_identity_for_apply(_target: &TargetIdentity) -> Result<LocalIdentity, String> {
    Err("ads.import_symbols.apply needs local identity when trust-runtime is built without ads-wire"
        .to_string())
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    use serde_json::{json, Value};
    use trust_ads_core::{AdsDataTypeDescriptor, IecDataType, SymbolDescriptor, SymbolFlag};

    use super::*;

    #[test]
    fn import_symbols_apply_writes_exact_selected_symbols_to_project_files() {
        let root = temp_dir("ads-import-apply");
        let snapshot = SymbolSnapshot::new(
            "line1",
            vec![
                real_symbol("MAIN.Temperature"),
                real_symbol("GVL.Setpoint"),
                real_symbol("GVL.SetpointShadow"),
            ],
        );

        let report = apply_import_to_project(
            root.as_path(),
            serde_json::from_value(json!({
                "connection_name": "line1",
                "symbols": ["GVL.Setpoint"],
                "include_patterns": ["MAIN.*"],
                "name_prefix": "line1_",
                "target": target("5.23.91.12.1.1", "192.168.10.5"),
                "local": local("192.168.10.20.1.1"),
                "snapshot": snapshot
            }))
            .expect("params"),
        )
        .expect("apply import");

        assert!(report.applied);
        assert_eq!(report.selected_count, 1);
        assert_eq!(report.lifecycle_effect, "restart_required");
        let ads_toml = fs::read_to_string(root.join("ads.toml")).expect("read ads.toml");
        assert!(ads_toml.contains("symbol = \"GVL.Setpoint\""), "{ads_toml}");
        assert!(
            !ads_toml.contains("MAIN.Temperature"),
            "exact symbols[] must not be broadened by include_patterns: {ads_toml}"
        );
        assert!(
            !ads_toml.contains("GVL.SetpointShadow"),
            "exact symbols[] must not select similarly named symbols: {ads_toml}"
        );
        let generated =
            fs::read_to_string(root.join("src/generated/ads_generated.st")).expect("generated ST");
        assert!(generated.contains("line1_gvl_setpoint : REAL;"));
        assert!(!generated.contains("line1_main_temperature"));
        assert!(root.join("ads/snapshots/line1.symbols.json").is_file());
    }

    #[test]
    fn import_symbols_apply_acknowledged_writable_symbols_as_read_write() {
        let root = temp_dir("ads-import-apply-writable");
        let snapshot = SymbolSnapshot::new(
            "line1",
            vec![real_symbol("GVL.Setpoint").with_flag(SymbolFlag::Write)],
        );

        let report = apply_import_to_project(
            root.as_path(),
            serde_json::from_value(json!({
                "connection_name": "line1",
                "symbols": ["GVL.Setpoint"],
                "target": target("5.23.91.12.1.1", "192.168.10.5"),
                "local": local("192.168.10.20.1.1"),
                "snapshot": snapshot,
                "write_acknowledged": true
            }))
            .expect("params"),
        )
        .expect("apply import");

        assert!(report.applied);
        let ads_toml = fs::read_to_string(root.join("ads.toml")).expect("read ads.toml");
        assert!(ads_toml.contains("symbol = \"GVL.Setpoint\""), "{ads_toml}");
        assert!(ads_toml.contains("access = \"read_write\""), "{ads_toml}");
    }

    #[test]
    fn import_symbols_apply_rejects_absolute_output_path() {
        let root = temp_dir("ads-import-path-root");
        let outside = temp_dir("ads-import-path-outside").join("escaped.st");

        let error = apply_import_to_project(
            root.as_path(),
            import_params(json!({
                "generated_path": outside,
            })),
        )
        .expect_err("absolute output path must be rejected");

        assert!(error.contains("relative"), "{error}");
        assert!(!outside.exists(), "import wrote outside the project root");
    }

    #[test]
    fn import_symbols_apply_rejects_parent_path_traversal() {
        let root = temp_dir("ads-import-parent-path");

        let error = apply_import_to_project(
            root.as_path(),
            import_params(json!({
                "snapshot_path": "ads/../../escaped.symbols.json",
            })),
        )
        .expect_err("parent traversal must be rejected");

        assert!(error.contains("project root"), "{error}");
        assert!(!root.join("escaped.symbols.json").exists());
    }

    #[cfg(unix)]
    #[test]
    fn import_symbols_apply_rejects_symlink_path_escape() {
        use std::os::unix::fs::symlink;

        let root = temp_dir("ads-import-symlink-root");
        let outside = temp_dir("ads-import-symlink-outside");
        symlink(&outside, root.join("linked")).expect("create project symlink");

        let error = apply_import_to_project(
            root.as_path(),
            import_params(json!({
                "generated_path": "linked/escaped.st",
            })),
        )
        .expect_err("symlink output path must be rejected");

        assert!(error.contains("symbolic link"), "{error}");
        assert!(!outside.join("escaped.st").exists());
    }

    #[test]
    fn import_symbols_apply_rejects_connection_name_as_snapshot_path() {
        let root = temp_dir("ads-import-connection-path");
        let escaped_stem = format!(
            "{}-escaped",
            root.file_name()
                .and_then(|name| name.to_str())
                .expect("temporary root name")
        );
        let connection_name = format!("../../../{escaped_stem}");

        let error = apply_import_to_project(
            root.as_path(),
            import_params(json!({
                "connection_name": connection_name,
                "snapshot": SymbolSnapshot::new(
                    format!("../../../{escaped_stem}"),
                    vec![real_symbol("MAIN.Temperature")],
                ),
            })),
        )
        .expect_err("connection name path syntax must be rejected");

        assert!(error.contains("connection_name"), "{error}");
        assert!(!root
            .parent()
            .unwrap_or(root.as_path())
            .join(format!("{escaped_stem}.symbols.json"))
            .exists());
    }

    #[test]
    fn import_symbols_apply_rejects_colliding_artifact_paths() {
        let root = temp_dir("ads-import-colliding-paths");

        let error = apply_import_to_project(
            root.as_path(),
            import_params(json!({
                "ads_toml_path": "ads/snapshots/import.symbols.json",
                "snapshot_path": "ads/snapshots/import.symbols.json",
                "generated_path": "ads/snapshots/import.symbols.json",
            })),
        )
        .expect_err("colliding artifacts must be rejected");

        assert!(error.contains("distinct"), "{error}");
        assert!(!root.join("ads/snapshots/import.symbols.json").exists());
    }

    #[test]
    fn import_symbols_apply_preserves_different_generated_source() {
        let root = temp_dir("ads-import-generated-overwrite");
        let generated = root.join("src/generated/ads_generated.st");
        fs::create_dir_all(generated.parent().expect("generated parent"))
            .expect("create generated parent");
        fs::write(&generated, "operator-authored source\n").expect("write existing source");

        let error = apply_import_to_project(root.as_path(), import_params(json!({})))
            .expect_err("different generated source must not be overwritten");

        assert!(error.contains("generated ST"), "{error}");
        assert_eq!(
            fs::read_to_string(&generated).expect("read preserved source"),
            "operator-authored source\n"
        );
        assert!(!root.join("ads.toml").exists());
        assert!(!root.join("ads/snapshots/line1.symbols.json").exists());
    }

    #[test]
    fn import_symbols_apply_accepts_identical_generated_source() {
        let root = temp_dir("ads-import-generated-identical");
        apply_import_to_project(root.as_path(), import_params(json!({}))).expect("initial import");
        let expected = fs::read_to_string(root.join("src/generated/ads_generated.st"))
            .expect("read initial generated source");

        apply_import_to_project(root.as_path(), import_params(json!({})))
            .expect("byte-identical generated source is safe");

        assert_eq!(
            fs::read_to_string(root.join("src/generated/ads_generated.st"))
                .expect("read repeated generated source"),
            expected
        );
    }

    #[test]
    fn import_symbols_apply_updates_valid_current_generated_source() {
        let root = temp_dir("ads-import-generated-update");
        apply_import_to_project(
            root.as_path(),
            import_params(json!({
                "snapshot_path": "ads/snapshots/custom-line1.symbols.json",
            })),
        )
        .expect("initial import with discoverable custom snapshot name");

        let report = apply_import_to_project(
            root.as_path(),
            import_params(json!({
                "connection_name": "line2",
                "symbols": ["MAIN.Pressure"],
                "snapshot": SymbolSnapshot::new(
                    "line2",
                    vec![real_symbol("MAIN.Pressure")],
                ),
            })),
        )
        .expect("follow-up import may update valid generated source");

        assert_eq!(report.connection_name, "line2");
        let generated = fs::read_to_string(root.join("src/generated/ads_generated.st"))
            .expect("read updated generated source");
        assert!(generated.contains("main_temperature : REAL;"));
        assert!(generated.contains("main_pressure : REAL;"));
    }

    #[test]
    fn import_symbols_apply_rejects_undiscoverable_snapshot_path() {
        let root = temp_dir("ads-import-undiscoverable-snapshot");

        let error = apply_import_to_project(
            root.as_path(),
            import_params(json!({
                "snapshot_path": "cache/line1.symbols.json",
            })),
        )
        .expect_err("snapshot outside canonical cache must be rejected");

        assert!(error.contains("ads/snapshots"), "{error}");
        assert!(!root.join("cache/line1.symbols.json").exists());
        assert!(!root.join("ads.toml").exists());
    }

    #[test]
    fn import_symbols_apply_acknowledged_write_only_symbol_as_write() {
        let root = temp_dir("ads-import-write-only");
        let write_only = SymbolDescriptor::new(
            "GVL.Command",
            AdsDataTypeDescriptor::scalar("REAL", IecDataType::Real),
            0x4020,
            0,
            4,
        )
        .with_flag(SymbolFlag::Write);

        apply_import_to_project(
            root.as_path(),
            import_params(json!({
                "symbols": ["GVL.Command"],
                "snapshot": SymbolSnapshot::new("line1", vec![write_only]),
                "write_acknowledged": true,
            })),
        )
        .expect("acknowledged write-only symbol import");

        let ads_toml = fs::read_to_string(root.join("ads.toml")).expect("read ads.toml");
        assert!(ads_toml.contains("access = \"write\""), "{ads_toml}");
        assert!(!ads_toml.contains("access = \"read_write\""), "{ads_toml}");
    }

    #[test]
    fn import_symbols_apply_route_missing_report_carries_route_plan() {
        let root = temp_dir("ads-import-route-missing");
        let target = target("100.67.6.217.1.1", "192.168.77.11");
        let local = local("192.168.77.10.1.1");
        let route_plan = build_import_route_plan(
            "line1",
            &target,
            &local,
            CredentialChannelClassification::TrustedSameHost,
        );

        let report = route_missing_apply_report(
            "line1".to_string(),
            2,
            "NoSymbols: upload ADS symbol table: receiving reply (route set?): timed out"
                .to_string(),
            route_plan,
            &ImportArtifactPaths::resolve(root.as_path(), &import_params(json!({})))
                .expect("artifact paths"),
        );
        let value = serde_json::to_value(report).expect("report json");

        assert_eq!(
            value.pointer("/applied").and_then(Value::as_bool),
            Some(false)
        );
        assert_eq!(
            value.pointer("/lifecycle_effect").and_then(Value::as_str),
            Some("blocked")
        );
        assert_eq!(
            value.pointer("/route/status").and_then(Value::as_str),
            Some("missing")
        );
        assert_eq!(
            value.pointer("/route/action").and_then(Value::as_str),
            Some("ads.route_plan")
        );
        assert_eq!(
            value
                .pointer("/route/route_plan/target/ams_net_id")
                .and_then(Value::as_str),
            Some("100.67.6.217.1.1")
        );
        assert!(value
            .pointer("/route/detail")
            .and_then(Value::as_str)
            .is_some_and(|detail| detail.contains("timed out")));
    }

    fn real_symbol(name: &str) -> SymbolDescriptor {
        SymbolDescriptor::new(
            name,
            AdsDataTypeDescriptor::scalar("REAL", IecDataType::Real),
            0x4020,
            0,
            4,
        )
        .with_flag(SymbolFlag::Read)
    }

    fn import_params(overrides: Value) -> ImportSymbolsApplyControlParams {
        let mut value = json!({
            "connection_name": "line1",
            "symbols": ["MAIN.Temperature"],
            "target": target("5.23.91.12.1.1", "192.168.10.5"),
            "local": local("192.168.10.20.1.1"),
            "snapshot": SymbolSnapshot::new(
                "line1",
                vec![real_symbol("MAIN.Temperature")],
            ),
        });
        let object = value.as_object_mut().expect("base params object");
        for (key, value) in overrides.as_object().expect("override params object") {
            object.insert(key.clone(), value.clone());
        }
        serde_json::from_value(value).expect("import params")
    }

    fn target(net_id: &str, ip: &str) -> TargetIdentity {
        TargetIdentity {
            name: Some("CX".to_string()),
            ip: ip.to_string(),
            ams_net_id: net_id.to_string(),
            ams_port: 851,
            tc_version: Some("3.1.4024".to_string()),
        }
    }

    fn local(net_id: &str) -> LocalIdentity {
        LocalIdentity {
            host_name: Some("line-controller".to_string()),
            chosen_ip: "192.168.10.20".to_string(),
            ams_net_id: net_id.to_string(),
            nic: Some("eth0".to_string()),
            candidates: Vec::new(),
            classification: crate::ads::diagnostics::LocalNetworkClassification::Lan,
        }
    }

    fn temp_dir(name: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("trust-{name}-{stamp}"));
        fs::create_dir_all(&path).expect("create temp dir");
        path
    }
}
