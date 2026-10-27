//! `ads.import_symbols.remove`: drop imported ADS points from a project and rebuild its ST.
//!
//! The request ports the panel's `offlineAdsRemoveTag` transaction (`NC/offlineComm.ts:335-423`):
//! rewrite `ads.toml`, regenerate the generated ST from the rewritten configuration and every
//! cached snapshot, and write both. The same function backs `trust-runtime ads remove-symbols`.

use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::ads::onboarding::import::remove::{
    regenerate_ads_interface, remove_symbol_imports, AdsTagDevice,
};

use super::super::{ControlResponse, ControlState};
use super::import_symbols::load_project_snapshots;

/// The removal answer shared by the control request and `trust-runtime ads remove-symbols`.
#[derive(Debug, Serialize)]
struct SymbolImportRemovalReport {
    schema_version: u32,
    applied: bool,
    matched: bool,
    removed_count: usize,
    restart_required: bool,
    files_written: Vec<String>,
    message: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ImportSymbolsRemoveControlParams {
    #[serde(default)]
    host: Option<String>,
    #[serde(default)]
    target_net_id: Option<String>,
    ams_port: u16,
    paths: Vec<String>,
}

pub(in crate::control) fn handle_ads_import_symbols_remove(
    id: u64,
    params: Option<Value>,
    state: &ControlState,
) -> ControlResponse {
    let Some(project_root) = state.project_root.as_ref() else {
        return ControlResponse::error(
            id,
            "ads.import_symbols.remove requires a runtime project_root".to_string(),
        );
    };
    let params = match params {
        Some(params) => params,
        None => return ControlResponse::error(id, "missing params".into()),
    };
    match remove_symbols_from_project(project_root, params) {
        Ok(value) => ControlResponse::ok(id, value),
        Err(error) => ControlResponse::error(id, error),
    }
}

/// Remove the matching imported points from a project's `ads.toml` and regenerate its ST.
///
/// Nothing is written when no point matched, or when there is no cached snapshot to regenerate
/// from. The answer names the files it wrote as project-relative paths.
pub(crate) fn remove_symbols_from_project(
    project_root: &Path,
    params: Value,
) -> Result<Value, String> {
    let params: ImportSymbolsRemoveControlParams =
        serde_json::from_value(params).map_err(|error| format!("invalid params: {error}"))?;
    let device = AdsTagDevice {
        host: params.host,
        target_net_id: params.target_net_id,
        ams_port: params.ams_port,
    };
    let ads_toml_path = project_root.join("ads.toml");
    let generated_path = project_root.join("src/generated/ads_generated.st");
    let before = fs::read_to_string(&ads_toml_path)
        .map_err(|error| format!("failed to read {}: {error}", ads_toml_path.display()))?;
    let removal = remove_symbol_imports(before.as_str(), &device, &params.paths);
    if removal.removed_count == 0 {
        return report_value(SymbolImportRemovalReport {
            schema_version: 1,
            applied: true,
            matched: false,
            removed_count: 0,
            restart_required: false,
            files_written: Vec::new(),
            message: "The ADS tag was already absent from ads.toml.".to_string(),
        });
    }
    let snapshots = load_project_snapshots(project_root)?;
    if snapshots.is_empty() {
        return report_value(SymbolImportRemovalReport {
            schema_version: 1,
            applied: false,
            matched: true,
            removed_count: 0,
            restart_required: false,
            files_written: Vec::new(),
            message:
                "Could not regenerate ADS ST because no cached ADS symbol snapshots were found."
                    .to_string(),
        });
    }
    let generated = regenerate_ads_interface(removal.text.as_str(), &snapshots)?;
    write_removal_file(&ads_toml_path, removal.text.as_str())?;
    write_removal_file(&generated_path, generated.as_str())?;
    report_value(SymbolImportRemovalReport {
        schema_version: 1,
        applied: true,
        matched: true,
        removed_count: removal.removed_count,
        restart_required: true,
        files_written: vec![
            "ads.toml".to_string(),
            "src/generated/ads_generated.st".to_string(),
        ],
        message: format!(
            "Removed {} ADS {}.",
            removal.removed_count,
            if removal.removed_count == 1 {
                "tag"
            } else {
                "tags"
            }
        ),
    })
}

fn report_value(report: SymbolImportRemovalReport) -> Result<Value, String> {
    serde_json::to_value(report)
        .map_err(|error| format!("ADS tag removal serialization failed: {error}"))
}

fn write_removal_file(path: &Path, text: &str) -> Result<(), String> {
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent)
            .map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
    }
    fs::write(path, text).map_err(|error| format!("failed to write {}: {error}", path.display()))
}
