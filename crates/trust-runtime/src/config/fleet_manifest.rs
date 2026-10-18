//! The runtimes that this computer manages under a fleet root (specification 25, sections 5.1,
//! 5.11 and 6.4): the fleet manifest `fleet.toml`, the control token in each runtime's
//! `runtime.toml`, and the runtime's status. The `trust-runtime fleet` commands and the Devices &
//! Connections panel engine read them here. Adding, starting and stopping a runtime stay with the
//! command line, which starts its own executable.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::Context;
use serde::{Deserialize, Serialize};
use toml_edit::{DocumentMut, Item};

use crate::control::ControlEndpoint;

mod status;

pub use status::{
    interrupted_stop_probe, managed_status, request_control, FleetRuntimeLifecycleResponse,
};

/// The fleet manifest's file name in a fleet root.
pub const FLEET_MANIFEST_FILE: &str = "fleet.toml";
/// The schema version of the fleet commands' JSON answers.
pub const FLEET_MANIFEST_SCHEMA_VERSION: u32 = 1;

/// The fleet manifest: the runtimes registered under a fleet root.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct FleetManifest {
    /// The registered runtimes.
    #[serde(default)]
    pub runtime: Vec<FleetManifestRuntime>,
}

/// One runtime of the fleet manifest.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct FleetManifestRuntime {
    /// The runtime's name.
    pub name: String,
    /// The runtime's project folder, relative to the fleet root or absolute.
    pub path: String,
    /// The runtime's control endpoint.
    pub control_endpoint: String,
    /// The runtime's web port.
    pub web_port: u16,
}

/// Reads and checks the fleet manifest at `path`; a missing file is an empty manifest.
///
/// # Errors
///
/// The file cannot be read or parsed, or its runtimes fail `validate_manifest`.
pub fn load_manifest(path: &Path) -> anyhow::Result<FleetManifest> {
    let manifest = match fs::read_to_string(path) {
        Ok(text) => toml::from_str(text.as_str())
            .with_context(|| format!("failed to parse {}", path.display())),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(FleetManifest::default()),
        Err(error) => Err(error).with_context(|| format!("failed to read {}", path.display())),
    }?;
    validate_manifest(&manifest)?;
    Ok(manifest)
}

/// Checks that every runtime has a valid unique name, a project path and a control endpoint,
/// and that no TCP port is used twice.
///
/// # Errors
///
/// The first runtime that breaks one of these rules.
pub fn validate_manifest(manifest: &FleetManifest) -> anyhow::Result<()> {
    let mut names = BTreeSet::new();
    let mut ports = BTreeSet::new();
    for runtime in &manifest.runtime {
        let validated_name = validate_runtime_name(runtime.name.as_str())?;
        if validated_name != runtime.name {
            anyhow::bail!(
                "fleet runtime name must not contain leading or trailing whitespace: {:?}",
                runtime.name
            );
        }
        if !names.insert(runtime.name.as_str()) {
            anyhow::bail!("duplicate fleet runtime name '{}'", runtime.name);
        }
        if runtime.path.trim().is_empty() {
            anyhow::bail!("fleet runtime '{}' has an empty project path", runtime.name);
        }

        let endpoint =
            ControlEndpoint::parse(runtime.control_endpoint.as_str()).map_err(|error| {
                anyhow::anyhow!(
                    "fleet runtime '{}' has invalid control endpoint '{}': {error}",
                    runtime.name,
                    runtime.control_endpoint
                )
            })?;
        match endpoint {
            ControlEndpoint::Tcp(address) => {
                validate_manifest_port(address.port(), &mut ports)?;
            }
            #[cfg(unix)]
            ControlEndpoint::Unix(_) => {}
        }
        validate_manifest_port(runtime.web_port, &mut ports)?;
    }
    Ok(())
}

fn validate_manifest_port(port: u16, ports: &mut BTreeSet<u16>) -> anyhow::Result<()> {
    if port == 0 {
        anyhow::bail!("fleet TCP ports must be concrete, not 0");
    }
    if !ports.insert(port) {
        anyhow::bail!("fleet TCP port {port} is used more than once");
    }
    Ok(())
}

/// Checks a runtime name: ASCII letters, digits, `-` and `_`, usable as a folder name.
///
/// # Errors
///
/// The name is empty, has leading or trailing whitespace, or has another character.
pub fn validate_runtime_name(name: &str) -> anyhow::Result<String> {
    let trimmed = name.trim();
    if trimmed != name {
        anyhow::bail!("runtime name must not contain leading or trailing whitespace: {name:?}");
    }
    if name.is_empty() {
        anyhow::bail!("runtime name must not be empty");
    }
    if !name
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_')
    {
        anyhow::bail!("runtime name may only contain ASCII letters, digits, '-' and '_'");
    }
    if name == "." || name == ".." {
        anyhow::bail!("runtime name must be a folder name, not '{name}'");
    }
    Ok(name.to_string())
}

/// Where the fleet commands keep a managed runtime's process ID and log.
#[derive(Debug, Clone)]
pub struct RuntimeLifecyclePaths {
    /// The folder of the lifecycle files, `<fleet root>/.trust-runtime`.
    pub dir: PathBuf,
    /// The advisory process ID file.
    pub pid: PathBuf,
    /// The log file.
    pub log: PathBuf,
}

/// The lifecycle files of the runtime `name` under `fleet_root`.
#[must_use]
pub fn lifecycle_paths(fleet_root: &Path, name: &str) -> RuntimeLifecyclePaths {
    let dir = fleet_root.join(".trust-runtime");
    RuntimeLifecyclePaths {
        dir: dir.clone(),
        pid: dir.join(format!("{name}.pid")),
        log: dir.join(format!("{name}.log")),
    }
}

/// A manifest project path: relative paths are below `fleet_root`, absolute ones stay as written.
#[must_use]
pub fn runtime_project_path(fleet_root: &Path, path: &str) -> PathBuf {
    let path = PathBuf::from(path);
    if path.is_absolute() {
        path
    } else {
        fleet_root.join(path)
    }
}

/// The control token of the runtime project at `project_path`: `runtime.control.auth_token` of
/// its `runtime.toml`, trimmed (specification 25, section 6.4). Only the root `runtime.control`
/// table supplies it, in table or dotted form; a key of the same name below another table does
/// not.
///
/// # Errors
///
/// The file cannot be read or parsed, the token is not a string, or the runtime configuration is
/// invalid.
pub fn read_control_auth_token(project_path: &Path) -> anyhow::Result<Option<String>> {
    let runtime_toml = project_path.join("runtime.toml");
    let text = fs::read_to_string(&runtime_toml)
        .with_context(|| format!("failed to read {}", runtime_toml.display()))?;
    let doc = text
        .parse::<DocumentMut>()
        .with_context(|| format!("failed to parse {}", runtime_toml.display()))?;
    let token = doc
        .get("runtime")
        .and_then(Item::as_table)
        .and_then(|runtime| runtime.get("control"))
        .and_then(Item::as_table)
        .and_then(|control| control.get("auth_token"))
        .and_then(Item::as_value);
    let token = match token {
        Some(value) => value
            .as_str()
            .map(|token| Some(token.to_owned()))
            .with_context(|| {
                format!(
                    "runtime.control.auth_token must be a string in {}",
                    runtime_toml.display()
                )
            }),
        None => Ok(None),
    }?;
    crate::config::validate_runtime_toml_text(text.as_str())
        .map_err(anyhow::Error::msg)
        .with_context(|| {
            format!(
                "invalid runtime configuration in {}",
                runtime_toml.display()
            )
        })?;
    Ok(token.map(|token| token.trim().to_owned()))
}

#[cfg(test)]
mod tests;
