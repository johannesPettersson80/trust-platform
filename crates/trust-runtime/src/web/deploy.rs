//! Bundle deploy helpers for the web UI.

#![allow(missing_docs)]

use std::fs;
use std::path::{Component, Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use serde::Deserialize;
use serde_json::json;
use sha2::{Digest, Sha256};
use trust_runtime_services::ports::ResourcePort;
use trust_runtime_services::resource::replies::{hung_refusal, resource_not_running};

use crate::config::{validate_io_toml_text, validate_runtime_toml_text};
use crate::deployment::program_store::{self, Payload, PayloadFile};
use crate::error::RuntimeError;
use crate::native::PROGRAM_IMAGE_FILE as IMAGE_FILE;
use crate::scheduler::ResourceState;

#[derive(Debug, Deserialize)]
pub struct DeployRequest {
    pub runtime_toml: Option<String>,
    pub io_toml: Option<String>,
    /// The program image `program.tpi`, base64 (specification 44 §8.8).
    pub program_tpi_b64: Option<String>,
    /// The bytecode engine's `program.stbc`, which this runtime refuses (specification 44 §15).
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

/// The reply to a deployment that arrives while the resource is not Stopped, or `None` when it is
/// Stopped and the deployment may go on (specification 34 §11.2, decision Q11; specification 11
/// §6.8.2).
///
/// A deployment installs a program, so it follows the rule of `program.install`: the reply is that
/// request's refusal for the same state, a hung scan thread first, with its `error` message and
/// its stable `error_code` (`resource_not_running`) as the members of the web reply.
pub fn refusal_unless_stopped<H: ResourcePort>(host: &H) -> Option<serde_json::Value> {
    let refusal = if host.is_hung() {
        hung_refusal(0)
    } else {
        let resource = host.resource_state();
        if matches!(resource, ResourceState::Stopped) {
            return None;
        }
        resource_not_running(0, resource)
    };
    Some(json!({
        "ok": false,
        "error": refusal.error,
        "error_code": refusal.error_code,
    }))
}

/// Deploys a request into the project folder `bundle_root`. The request's signature policy is the
/// installed one (spec 11). The program is admitted before anything is written, and the program
/// store commits the deployment: a project that is a store's `current` link gets a new bundle, and
/// any other project folder is updated in place (see `program_store`).
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
    let payload = deploy_payload(&request)?;
    let deployed = program_store::deploy(bundle_root, &payload, None)?;
    Ok(DeployResult {
        written: deployed.written,
        restart: request.restart,
    })
}

/// The files a deploy request brings, in the order the store writes them: `runtime.toml`,
/// `io.toml`, `program.tpi` (decoded from base64), then the sources under `src/`. A request that
/// brings the bytecode engine's `program.stbc` is refused, as a project that holds it is
/// (specification 44 §15).
fn deploy_payload(request: &DeployRequest) -> Result<Payload, RuntimeError> {
    if request.program_stbc_b64.is_some() {
        return Err(RuntimeError::ControlError(
            "program_stbc_b64 holds the bytecode engine's program.stbc, which this runtime does not run: rebuild the project and deploy its program.tpi as program_tpi_b64"
                .into(),
        ));
    }
    let mut files = Vec::new();
    if let Some(runtime_toml) = &request.runtime_toml {
        files.push(payload_file(
            "runtime.toml",
            runtime_toml.as_bytes().to_vec(),
        ));
    }
    if let Some(io_toml) = &request.io_toml {
        files.push(payload_file("io.toml", io_toml.as_bytes().to_vec()));
    }
    if let Some(program_b64) = &request.program_tpi_b64 {
        let bytes = STANDARD.decode(program_b64.trim()).map_err(|err| {
            RuntimeError::ControlError(format!("decode {IMAGE_FILE}: {err}").into())
        })?;
        files.push(payload_file(IMAGE_FILE, bytes));
    }
    for source in request.sources.iter().flatten() {
        let rel = normalize_source_path_for_bundle(&source.path).ok_or_else(|| {
            RuntimeError::ControlError(format!("invalid source path: {}", source.path).into())
        })?;
        let rel_text = rel.to_string_lossy().replace('\\', "/");
        files.push(payload_file(
            &format!("src/{rel_text}"),
            source.content.as_bytes().to_vec(),
        ));
    }
    Ok(Payload { files })
}

fn payload_file(path: &str, bytes: Vec<u8>) -> PayloadFile {
    PayloadFile {
        path: path.to_string(),
        bytes,
    }
}

fn preflight_deploy(bundle_root: &Path, request: &DeployRequest) -> Result<(), RuntimeError> {
    let existing = bundle_root.join("runtime.toml");
    let installed_text =
        if existing.is_file() {
            Some(fs::read_to_string(&existing).map_err(|err| {
                RuntimeError::InvalidConfig(format!("runtime.toml: {err}").into())
            })?)
        } else {
            None
        };
    let runtime_text = request
        .runtime_toml
        .clone()
        .or_else(|| installed_text.clone())
        .ok_or_else(|| {
            RuntimeError::InvalidConfig("deploy preflight requires runtime.toml".into())
        })?;
    validate_runtime_toml_text(&runtime_text)?;
    // The signature policy in force is the installed configuration's, so a deployment cannot
    // lift it, or name its own keyring, with the runtime.toml it carries (spec 11, deployment
    // signature policy). Only a project with nothing installed takes the deployment's policy.
    let policy_text = installed_text.as_deref().unwrap_or(&runtime_text);
    verify_signature_policy(bundle_root, policy_text, request)?;

    let io_text = if let Some(text) = request.io_toml.as_deref() {
        Some(text.to_string())
    } else {
        let existing = bundle_root.join("io.toml");
        if existing.is_file() {
            Some(
                fs::read_to_string(&existing)
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
    let text = fs::read_to_string(path).map_err(|_| {
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
        "program_tpi_b64",
        request.program_tpi_b64.as_deref(),
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

/// Rolls the program store at `root` back: `current` and `previous` change bundles, one rename
/// per link (see `program_store::rollback`).
pub fn apply_rollback(root: &Path) -> Result<RollbackResult, RuntimeError> {
    let rolled = program_store::rollback(root)?;
    Ok(RollbackResult {
        current: rolled.current,
        previous: rolled.previous,
    })
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
