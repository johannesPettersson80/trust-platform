//! Bundle deploy helpers for the web UI.

#![allow(missing_docs)]

use std::fs;
use std::path::{Component, Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use serde::Deserialize;
use sha2::{Digest, Sha256};

use crate::config::{validate_io_toml_text, validate_runtime_toml_text};
use crate::error::RuntimeError;

#[derive(Debug, Deserialize)]
pub struct DeployRequest {
    pub runtime_toml: Option<String>,
    pub io_toml: Option<String>,
    pub program_stbc_b64: Option<String>,
    pub sources: Option<Vec<DeploySource>>,
    pub signature: Option<DeploySignature>,
    pub restart: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DeploySource {
    pub path: String,
    pub content: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DeploySignature {
    pub key_id: String,
    pub payload_sha256: String,
    pub signature: String,
}

#[derive(Debug)]
pub struct DeployResult {
    pub written: Vec<String>,
    pub restart: Option<String>,
}

#[derive(Debug)]
pub struct RollbackResult {
    pub current: PathBuf,
    pub previous: PathBuf,
}

#[derive(Debug, Default, Deserialize)]
struct RuntimeDeployPolicyDoc {
    runtime: Option<RuntimeDeployPolicyRuntime>,
}

#[derive(Debug, Default, Deserialize)]
struct RuntimeDeployPolicyRuntime {
    deploy: Option<RuntimeDeployPolicy>,
}

#[derive(Debug, Default, Deserialize)]
struct RuntimeDeployPolicy {
    require_signed: Option<bool>,
    keyring_path: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
struct DeployKeyringFile {
    keys: Vec<DeployKeyEntry>,
}

#[derive(Debug, Clone, Deserialize)]
struct DeployKeyEntry {
    id: String,
    secret: String,
    enabled: Option<bool>,
    not_after_unix: Option<u64>,
    #[serde(alias = "not_after")]
    not_after: Option<u64>,
}

pub fn apply_deploy(
    bundle_root: &Path,
    request: DeployRequest,
) -> Result<DeployResult, RuntimeError> {
    if !bundle_root.is_dir() {
        return Err(RuntimeError::ControlError(
            format!("project folder not found: {}", bundle_root.display()).into(),
        ));
    }
    preflight_deploy(bundle_root, &request)?;
    let mut written = Vec::new();
    if let Some(runtime_toml) = request.runtime_toml {
        let path = bundle_root.join("runtime.toml");
        fs::write(&path, runtime_toml).map_err(|err| {
            RuntimeError::ControlError(format!("write runtime.toml: {err}").into())
        })?;
        written.push("runtime.toml".to_string());
    }
    if let Some(io_toml) = request.io_toml {
        let path = bundle_root.join("io.toml");
        fs::write(&path, io_toml)
            .map_err(|err| RuntimeError::ControlError(format!("write io.toml: {err}").into()))?;
        written.push("io.toml".to_string());
    }
    if let Some(program_b64) = request.program_stbc_b64 {
        let bytes = STANDARD.decode(program_b64.trim()).map_err(|err| {
            RuntimeError::ControlError(format!("decode program.stbc: {err}").into())
        })?;
        let path = bundle_root.join("program.stbc");
        fs::write(&path, bytes).map_err(|err| {
            RuntimeError::ControlError(format!("write program.stbc: {err}").into())
        })?;
        written.push("program.stbc".to_string());
    }
    if let Some(sources) = request.sources {
        let sources_root = bundle_root.join("src");
        for source in sources {
            let rel = normalize_source_path_for_bundle(&source.path).ok_or_else(|| {
                RuntimeError::ControlError(format!("invalid source path: {}", source.path).into())
            })?;
            let dest = sources_root.join(&rel);
            if let Some(parent) = dest.parent() {
                fs::create_dir_all(parent).map_err(|err| {
                    RuntimeError::ControlError(format!("create src dir: {err}").into())
                })?;
            }
            fs::write(&dest, source.content).map_err(|err| {
                RuntimeError::ControlError(format!("write source {}: {err}", dest.display()).into())
            })?;
            let rel_text = rel.to_string_lossy().replace('\\', "/");
            written.push(format!("src/{rel_text}"));
        }
    }
    if written.is_empty() {
        return Err(RuntimeError::ControlError(
            "no deploy payload provided".into(),
        ));
    }
    Ok(DeployResult {
        written,
        restart: request.restart,
    })
}

fn preflight_deploy(bundle_root: &Path, request: &DeployRequest) -> Result<(), RuntimeError> {
    let runtime_text = if let Some(text) = request.runtime_toml.as_deref() {
        Some(text.to_string())
    } else {
        let existing = bundle_root.join("runtime.toml");
        if existing.is_file() {
            Some(std::fs::read_to_string(&existing).map_err(|err| {
                RuntimeError::InvalidConfig(format!("runtime.toml: {err}").into())
            })?)
        } else {
            None
        }
    };
    let runtime_text = runtime_text.ok_or_else(|| {
        RuntimeError::InvalidConfig("deploy preflight requires runtime.toml".into())
    })?;
    validate_runtime_toml_text(&runtime_text)?;
    verify_signature_policy(bundle_root, &runtime_text, request)?;

    let io_text = if let Some(text) = request.io_toml.as_deref() {
        Some(text.to_string())
    } else {
        let existing = bundle_root.join("io.toml");
        if existing.is_file() {
            Some(
                std::fs::read_to_string(&existing)
                    .map_err(|err| RuntimeError::InvalidConfig(format!("io.toml: {err}").into()))?,
            )
        } else {
            None
        }
    };
    if let Some(io_text) = io_text {
        validate_io_toml_text(&io_text)?;
    }
    Ok(())
}

fn verify_signature_policy(
    bundle_root: &Path,
    runtime_text: &str,
    request: &DeployRequest,
) -> Result<(), RuntimeError> {
    let policy = parse_runtime_deploy_policy(runtime_text)?;
    if !policy.require_signed.unwrap_or(false) {
        return Ok(());
    }
    let signature = request.signature.as_ref().ok_or_else(|| {
        RuntimeError::ControlError("signed deploy required by runtime.deploy.require_signed".into())
    })?;
    let payload_sha = deploy_payload_sha256(request);
    if signature.payload_sha256.trim().to_ascii_lowercase() != payload_sha {
        return Err(RuntimeError::ControlError(
            "deploy payload signature mismatch".into(),
        ));
    }
    let keyring_rel = policy
        .keyring_path
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("security/deploy-keys.toml");
    let keyring_path = if Path::new(keyring_rel).is_absolute() {
        PathBuf::from(keyring_rel)
    } else {
        bundle_root.join(keyring_rel)
    };
    let key = load_deploy_key(&keyring_path, signature.key_id.trim())?;
    let not_after = key.not_after_unix.or(key.not_after);
    if let Some(not_after) = not_after
        && not_after < now_secs()
    {
        return Err(RuntimeError::ControlError(
            "deploy signing key expired".into(),
        ));
    }
    let expected = deploy_signature_digest(key.secret.trim(), &payload_sha);
    if signature.signature.trim().to_ascii_lowercase() != expected {
        return Err(RuntimeError::ControlError(
            "deploy signature invalid".into(),
        ));
    }
    Ok(())
}

fn parse_runtime_deploy_policy(runtime_text: &str) -> Result<RuntimeDeployPolicy, RuntimeError> {
    let raw: RuntimeDeployPolicyDoc = toml::from_str(runtime_text).map_err(|err| {
        RuntimeError::InvalidConfig(format!("runtime.toml deploy policy: {err}").into())
    })?;
    Ok(raw
        .runtime
        .and_then(|runtime| runtime.deploy)
        .unwrap_or_default())
}

fn load_deploy_key(path: &Path, key_id: &str) -> Result<DeployKeyEntry, RuntimeError> {
    if key_id.trim().is_empty() {
        return Err(RuntimeError::ControlError("deploy key_id required".into()));
    }
    let text = std::fs::read_to_string(path).map_err(|_| {
        RuntimeError::ControlError(format!("deploy keyring not found: {}", path.display()).into())
    })?;
    let file: DeployKeyringFile = toml::from_str(&text)
        .map_err(|_| RuntimeError::ControlError("invalid deploy keyring".into()))?;
    file.keys
        .into_iter()
        .find(|entry| entry.id == key_id && entry.enabled.unwrap_or(true))
        .ok_or_else(|| RuntimeError::ControlError("unknown deploy signing key".into()))
}

fn deploy_payload_sha256(request: &DeployRequest) -> String {
    let mut hasher = Sha256::new();
    hash_field(&mut hasher, "runtime_toml", request.runtime_toml.as_deref());
    hash_field(&mut hasher, "io_toml", request.io_toml.as_deref());
    hash_field(
        &mut hasher,
        "program_stbc_b64",
        request.program_stbc_b64.as_deref(),
    );

    let mut sources = request.sources.clone().unwrap_or_default();
    sources.sort_by(|a, b| a.path.cmp(&b.path));
    hasher.update("sources".as_bytes());
    hasher.update([0u8]);
    for source in sources {
        hasher.update(source.path.as_bytes());
        hasher.update([0u8]);
        hasher.update(source.content.as_bytes());
        hasher.update([0u8]);
    }
    hex_string(&hasher.finalize())
}

fn hash_field(hasher: &mut Sha256, key: &str, value: Option<&str>) {
    hasher.update(key.as_bytes());
    hasher.update([0u8]);
    if let Some(value) = value {
        hasher.update(value.as_bytes());
    }
    hasher.update([0u8]);
}

fn deploy_signature_digest(secret: &str, payload_sha256: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(secret.as_bytes());
    hasher.update([0u8]);
    hasher.update(payload_sha256.as_bytes());
    hex_string(&hasher.finalize())
}

fn hex_string(bytes: &[u8]) -> String {
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push_str(&format!("{byte:02x}"));
    }
    output
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

pub fn apply_rollback(root: &Path) -> Result<RollbackResult, RuntimeError> {
    let current_link = root.join("current");
    let previous_link = root.join("previous");
    let current_target = read_link_target(&current_link).ok_or_else(|| {
        RuntimeError::ControlError(
            format!("no current project link at {}", current_link.display()).into(),
        )
    })?;
    let previous_target = read_link_target(&previous_link).ok_or_else(|| {
        RuntimeError::ControlError(
            format!("no previous project link at {}", previous_link.display()).into(),
        )
    })?;
    update_symlink(&current_link, &previous_target)?;
    update_symlink(&previous_link, &current_target)?;
    Ok(RollbackResult {
        current: previous_target,
        previous: current_target,
    })
}

fn read_link_target(path: &Path) -> Option<PathBuf> {
    std::fs::read_link(path).ok()
}

fn update_symlink(link: &Path, target: &Path) -> Result<(), RuntimeError> {
    if link.exists() {
        std::fs::remove_file(link).map_err(|err| {
            RuntimeError::ControlError(format!("remove link {}: {err}", link.display()).into())
        })?;
    }
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(target, link).map_err(|err| {
            RuntimeError::ControlError(format!("symlink {}: {err}", link.display()).into())
        })?;
    }
    #[cfg(windows)]
    {
        std::os::windows::fs::symlink_dir(target, link).map_err(|err| {
            RuntimeError::ControlError(format!("symlink {}: {err}", link.display()).into())
        })?;
    }
    Ok(())
}

fn sanitize_relative_path(path: &str) -> Option<PathBuf> {
    let path = Path::new(path);
    let mut clean = PathBuf::new();
    for comp in path.components() {
        match comp {
            Component::Normal(value) => clean.push(value),
            Component::CurDir => {}
            _ => return None,
        }
    }
    if clean.as_os_str().is_empty() {
        None
    } else {
        Some(clean)
    }
}

fn normalize_source_path_for_bundle(path: &str) -> Option<PathBuf> {
    let rel = sanitize_relative_path(path)?;
    // Accept both `main.st` and `src/main.st` payload paths, but always write
    // under `<project>/src/<relative>`.
    if let Ok(stripped) = rel.strip_prefix("src") {
        if stripped.as_os_str().is_empty() {
            return None;
        }
        return Some(stripped.to_path_buf());
    }
    Some(rel)
}

#[cfg(test)]
mod tests;
