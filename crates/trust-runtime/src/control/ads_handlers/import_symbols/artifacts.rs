use std::fs;
use std::path::{Component, Path, PathBuf};

use trust_ads_core::SymbolSnapshot;

use super::ImportSymbolsApplyControlParams;

pub(super) struct ImportArtifactPaths {
    pub(super) ads_toml: PathBuf,
    pub(super) snapshot: PathBuf,
    pub(super) generated: PathBuf,
}

impl ImportArtifactPaths {
    pub(super) fn resolve(
        project_root: &Path,
        params: &ImportSymbolsApplyControlParams,
    ) -> Result<Self, String> {
        let default_snapshot = default_snapshot_relative_path(params.connection_name.as_str())?;
        let ads_toml = project_artifact_path(
            project_root,
            params
                .ads_toml_path
                .as_deref()
                .unwrap_or(Path::new("ads.toml")),
            "ads_toml_path",
        )?;
        let snapshot = project_artifact_path(
            project_root,
            params.snapshot_path.as_deref().unwrap_or(&default_snapshot),
            "snapshot_path",
        )?;
        validate_snapshot_path(project_root, &snapshot)?;
        let generated = project_artifact_path(
            project_root,
            params
                .generated_path
                .as_deref()
                .unwrap_or(Path::new("src/generated/ads_generated.st")),
            "generated_path",
        )?;
        if same_artifact_path(&ads_toml, &snapshot)
            || same_artifact_path(&ads_toml, &generated)
            || same_artifact_path(&snapshot, &generated)
        {
            return Err(
                "ads_toml_path, snapshot_path, and generated_path must be distinct".to_string(),
            );
        }
        Ok(Self {
            ads_toml,
            snapshot,
            generated,
        })
    }
}

fn validate_snapshot_path(project_root: &Path, snapshot: &Path) -> Result<(), String> {
    let relative = snapshot.strip_prefix(project_root).map_err(|_| {
        "snapshot_path must be a direct *.symbols.json file under ads/snapshots".to_string()
    })?;
    let parts = relative
        .components()
        .filter_map(|component| match component {
            Component::Normal(value) => Some(value),
            _ => None,
        })
        .collect::<Vec<_>>();
    let valid = parts.len() == 3
        && parts[0] == "ads"
        && parts[1] == "snapshots"
        && parts[2].to_str().is_some_and(|name| {
            name.len() > ".symbols.json".len() && name.ends_with(".symbols.json")
        });
    if !valid {
        return Err(
            "snapshot_path must be a direct *.symbols.json file under ads/snapshots".to_string(),
        );
    }
    Ok(())
}

fn project_artifact_path(
    project_root: &Path,
    requested: &Path,
    field: &str,
) -> Result<PathBuf, String> {
    let text = requested
        .to_str()
        .ok_or_else(|| format!("{field} must be valid UTF-8"))?;
    if text.trim().is_empty() {
        return Err(format!("{field} must be a non-empty relative path"));
    }
    if requested.is_absolute() || requested.has_root() {
        return Err(format!(
            "{field} must be a relative path inside the project root"
        ));
    }
    if text.contains('\\')
        || text
            .as_bytes()
            .get(1)
            .is_some_and(|value| *value == b':' && text.as_bytes()[0].is_ascii_alphabetic())
    {
        return Err(format!(
            "{field} must not contain a platform path prefix or backslash"
        ));
    }

    let mut relative = PathBuf::new();
    for component in requested.components() {
        match component {
            Component::Normal(value) => relative.push(value),
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err(format!("{field} must stay inside the project root"));
            }
        }
    }
    if relative.as_os_str().is_empty() {
        return Err(format!("{field} must be a non-empty relative path"));
    }

    reject_symlink_path(project_root, &relative, field)?;
    Ok(project_root.join(relative))
}

fn reject_symlink_path(project_root: &Path, relative: &Path, field: &str) -> Result<(), String> {
    let mut current = project_root.to_path_buf();
    for component in relative.components() {
        let Component::Normal(value) = component else {
            continue;
        };
        current.push(value);
        match fs::symlink_metadata(&current) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return Err(format!(
                    "{field} must not traverse a symbolic link: {}",
                    current.display()
                ));
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => break,
            Err(error) => {
                return Err(format!(
                    "failed to inspect {field} component {}: {error}",
                    current.display()
                ));
            }
        }
    }
    Ok(())
}

fn same_artifact_path(left: &Path, right: &Path) -> bool {
    left.to_string_lossy()
        .eq_ignore_ascii_case(right.to_string_lossy().as_ref())
}

fn default_snapshot_relative_path(connection_name: &str) -> Result<PathBuf, String> {
    let unsafe_name = connection_name.trim().is_empty()
        || matches!(connection_name, "." | "..")
        || connection_name.chars().any(|ch| {
            ch.is_control() || matches!(ch, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*')
        });
    if unsafe_name {
        return Err(
            "connection_name must be a safe single filename component for the default snapshot path"
                .to_string(),
        );
    }
    Ok(Path::new("ads")
        .join("snapshots")
        .join(format!("{connection_name}.symbols.json")))
}

pub(super) fn read_optional_project_file(path: &Path) -> Result<Option<String>, String> {
    match fs::read_to_string(path) {
        Ok(text) => Ok(Some(text)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(format!("failed to read {}: {error}", path.display())),
    }
}

pub(super) fn preflight_generated_output(
    path: &Path,
    current_ads_toml: Option<&str>,
    current_snapshots: &[SymbolSnapshot],
) -> Result<(), String> {
    match fs::read_to_string(path) {
        Ok(existing) => {
            let current_ads_toml = current_ads_toml.ok_or_else(|| {
                format!(
                    "generated ST destination {} exists without current ads.toml authority",
                    path.display()
                )
            })?;
            let config = crate::ads::parse_ads_toml(current_ads_toml).map_err(|error| {
                format!(
                    "generated ST destination {} cannot be verified because current ads.toml is invalid: {error}",
                    path.display()
                )
            })?;
            crate::ads::validate_ads_interface_offline(&config, current_snapshots, &existing)
                .map_err(|error| {
                    format!(
                        "generated ST destination {} does not match current ads.toml and snapshots; refusing to overwrite it: {error}",
                        path.display()
                    )
                })?;
            Ok(())
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!(
            "failed to read generated ST destination {}: {error}",
            path.display()
        )),
    }
}

pub(super) fn write_project_file(path: &Path, text: &str) -> Result<(), String> {
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent)
            .map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
    }
    fs::write(path, text).map_err(|error| format!("failed to write {}: {error}", path.display()))
}

pub(super) fn display_path(path: &Path) -> String {
    path.to_string_lossy().to_string()
}
