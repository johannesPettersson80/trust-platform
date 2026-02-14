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
            let rel = sanitize_relative_path(&source.path).ok_or_else(|| {
                RuntimeError::ControlError(format!("invalid source path: {}", source.path).into())
            })?;
            let dest = sources_root.join(rel);
            if let Some(parent) = dest.parent() {
                fs::create_dir_all(parent).map_err(|err| {
                    RuntimeError::ControlError(format!("create src dir: {err}").into())
                })?;
            }
            fs::write(&dest, source.content).map_err(|err| {
                RuntimeError::ControlError(format!("write source {}: {err}", dest.display()).into())
            })?;
            written.push(format!("src/{}", source.path));
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
    if let Some(not_after) = not_after {
        if not_after < now_secs() {
            return Err(RuntimeError::ControlError(
                "deploy signing key expired".into(),
            ));
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_rejects_parent() {
        assert!(sanitize_relative_path("../bad.st").is_none());
        assert!(sanitize_relative_path("/abs/bad.st").is_none());
    }

    #[test]
    fn sanitize_accepts_nested() {
        let path = sanitize_relative_path("lib/util.st").unwrap();
        assert_eq!(path, PathBuf::from("lib/util.st"));
    }

    #[test]
    fn apply_deploy_writes_files() {
        let mut root = std::env::temp_dir();
        root.push(format!("trust-deploy-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let request = DeployRequest {
            runtime_toml: Some(
                r#"
[bundle]
version = 1

[resource]
name = "main"
cycle_interval_ms = 100

[runtime.control]
endpoint = "unix:///tmp/trust-runtime.sock"
mode = "production"
debug_enabled = false

[runtime.log]
level = "info"

[runtime.retain]
mode = "none"
save_interval_ms = 1000

[runtime.watchdog]
enabled = false
timeout_ms = 5000
action = "halt"

[runtime.fault]
policy = "halt"
"#
                .to_string(),
            ),
            io_toml: None,
            program_stbc_b64: Some(STANDARD.encode([1u8, 2, 3])),
            sources: Some(vec![DeploySource {
                path: "main.st".to_string(),
                content: "PROGRAM Main\nEND_PROGRAM\n".to_string(),
            }]),
            signature: None,
            restart: None,
        };
        let result = apply_deploy(&root, request).unwrap();
        assert!(result.written.contains(&"runtime.toml".to_string()));
        assert!(root.join("program.stbc").exists());
        assert!(root.join("src/main.st").exists());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn apply_deploy_rejects_invalid_runtime_schema() {
        let mut root = std::env::temp_dir();
        root.push(format!("trust-deploy-invalid-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let request = DeployRequest {
            runtime_toml: Some(
                r#"
[bundle]
version = 1

[resource]
name = "main"
cycle_interval_ms = 0

[runtime.control]
endpoint = "unix:///tmp/trust-runtime.sock"

[runtime.log]
level = "info"

[runtime.retain]
mode = "none"
save_interval_ms = 1000

[runtime.watchdog]
enabled = false
timeout_ms = 5000
action = "halt"

[runtime.fault]
policy = "halt"
"#
                .to_string(),
            ),
            io_toml: None,
            program_stbc_b64: None,
            sources: None,
            signature: None,
            restart: None,
        };
        let err = apply_deploy(&root, request).expect_err("schema should fail");
        assert!(err
            .to_string()
            .contains("resource.cycle_interval_ms must be >= 1"));
        let _ = fs::remove_dir_all(root);
    }

    fn runtime_with_signed_policy() -> String {
        r#"
[bundle]
version = 1

[resource]
name = "main"
cycle_interval_ms = 100

[runtime.control]
endpoint = "unix:///tmp/trust-runtime.sock"
mode = "production"
debug_enabled = false

[runtime.deploy]
require_signed = true
keyring_path = "security/deploy-keys.toml"

[runtime.log]
level = "info"

[runtime.retain]
mode = "none"
save_interval_ms = 1000

[runtime.watchdog]
enabled = false
timeout_ms = 5000
action = "halt"

[runtime.fault]
policy = "halt"
"#
        .to_string()
    }

    fn signed_request(root: &Path, key_id: &str, secret: &str) -> DeployRequest {
        let mut request = DeployRequest {
            runtime_toml: Some(runtime_with_signed_policy()),
            io_toml: None,
            program_stbc_b64: Some(STANDARD.encode([9u8, 8, 7])),
            sources: Some(vec![DeploySource {
                path: "main.st".to_string(),
                content: "PROGRAM Main\nEND_PROGRAM\n".to_string(),
            }]),
            signature: None,
            restart: None,
        };
        let key_dir = root.join("security");
        fs::create_dir_all(&key_dir).expect("security dir");
        fs::write(
            key_dir.join("deploy-keys.toml"),
            format!(
                r#"
[[keys]]
id = "{key_id}"
secret = "{secret}"
enabled = true
not_after_unix = 4102444800
"#
            ),
        )
        .expect("write keyring");
        let payload_sha = deploy_payload_sha256(&request);
        let signature = deploy_signature_digest(secret, &payload_sha);
        request.signature = Some(DeploySignature {
            key_id: key_id.to_string(),
            payload_sha256: payload_sha,
            signature,
        });
        request
    }

    #[test]
    fn apply_deploy_accepts_valid_signature_policy() {
        let mut root = std::env::temp_dir();
        root.push(format!("trust-deploy-signed-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).expect("create root");
        let request = signed_request(&root, "ci", "super-secret");
        let result = apply_deploy(&root, request).expect("signed deploy should pass");
        assert!(result.written.contains(&"runtime.toml".to_string()));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn apply_deploy_rejects_tampered_payload_signature() {
        let mut root = std::env::temp_dir();
        root.push(format!(
            "trust-deploy-signed-tampered-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).expect("create root");
        let mut request = signed_request(&root, "ci", "super-secret");
        request.program_stbc_b64 = Some(STANDARD.encode([1u8, 1, 1]));
        let err = apply_deploy(&root, request).expect_err("tampered payload should fail");
        assert!(err.to_string().contains("signature mismatch"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn apply_deploy_rejects_unknown_or_expired_signing_keys() {
        let mut root = std::env::temp_dir();
        root.push(format!(
            "trust-deploy-signed-key-errors-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).expect("create root");
        let mut request = signed_request(&root, "ci", "super-secret");
        request.signature.as_mut().expect("signature").key_id = "unknown".to_string();
        let unknown = apply_deploy(&root, request).expect_err("unknown key should fail");
        assert!(unknown.to_string().contains("unknown deploy signing key"));

        let expired_request = signed_request(&root, "ci", "super-secret");
        fs::write(
            root.join("security/deploy-keys.toml"),
            r#"
[[keys]]
id = "ci"
secret = "super-secret"
enabled = true
not_after_unix = 100
"#,
        )
        .expect("write expired keyring");
        let expired = apply_deploy(&root, expired_request).expect_err("expired key should fail");
        assert!(expired.to_string().contains("deploy signing key expired"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn signature_errors_do_not_echo_key_secrets() {
        let mut root = std::env::temp_dir();
        root.push(format!(
            "trust-deploy-signed-secret-safety-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).expect("create root");

        let secret = "very-sensitive-secret-value";
        let mut request = signed_request(&root, "ci", secret);
        request.signature.as_mut().expect("signature").signature = "deadbeef".to_string();
        let err = apply_deploy(&root, request).expect_err("invalid signature should fail");
        let text = err.to_string();
        assert!(!text.contains(secret), "error leaked secret: {text}");
        let _ = fs::remove_dir_all(root);
    }
}
