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
        hmi: None,
        accept_broken_bindings: None,
    };
    let result = apply_deploy(&root, request).unwrap();
    assert!(result.written.contains(&"runtime.toml".to_string()));
    assert!(root.join("program.stbc").exists());
    assert!(root.join("src/main.st").exists());
    let _ = fs::remove_dir_all(root);
}

#[test]
fn apply_deploy_normalizes_src_prefixed_source_paths() {
    let mut root = std::env::temp_dir();
    root.push(format!("trust-deploy-src-prefix-{}", std::process::id()));
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
        program_stbc_b64: None,
        sources: Some(vec![DeploySource {
            path: "src/main.st".to_string(),
            content: "PROGRAM Main\nEND_PROGRAM\n".to_string(),
        }]),
        signature: None,
        restart: None,
        hmi: None,
        accept_broken_bindings: None,
    };
    let result = apply_deploy(&root, request).expect("deploy should pass");
    assert!(result.written.contains(&"runtime.toml".to_string()));
    assert!(result.written.contains(&"src/main.st".to_string()));
    assert!(root.join("src/main.st").exists());
    assert!(!root.join("src/src/main.st").exists());
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
        hmi: None,
        accept_broken_bindings: None,
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
        hmi: None,
        accept_broken_bindings: None,
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
use super::*;
