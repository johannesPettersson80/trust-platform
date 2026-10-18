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
        program_stbc_b64: Some(STANDARD.encode(admitted_program())),
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

/// The bytes of an empty program that the runtime prepares: a deployment admits its program first
/// (spec 34 §11.2 step 6), so a test program must be real bytecode.
fn admitted_program() -> Vec<u8> {
    crate::harness::CompileSession::from_source("PROGRAM Main\nEND_PROGRAM\n")
        .build_bytecode_bytes()
        .expect("the test program compiles")
}

fn signed_request(root: &Path, key_id: &str, secret: &str) -> DeployRequest {
    let mut request = DeployRequest {
        runtime_toml: Some(runtime_with_signed_policy()),
        io_toml: None,
        program_stbc_b64: Some(STANDARD.encode(admitted_program())),
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

/// Spec 34 §5.1.1: a deployed program comes with its own deployment record, and never keeps
/// the record of the program it replaces.
#[test]
fn a_deployed_program_brings_its_own_deployment_record() {
    use crate::deployment::DEPLOYMENT_RECORD_FILE;
    use trust_runtime_core::deployment::{sha256, DeploymentRecord};

    let mut root = std::env::temp_dir();
    root.push(format!("trust-deploy-record-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).unwrap();
    fs::write(
        root.join(DEPLOYMENT_RECORD_FILE),
        b"an older program's record",
    )
    .unwrap();
    const RUNTIME_TOML: &str = r#"
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
"#;
    let request = |program: &[u8]| DeployRequest {
        runtime_toml: Some(RUNTIME_TOML.to_string()),
        io_toml: None,
        program_stbc_b64: Some(STANDARD.encode(program)),
        sources: None,
        signature: None,
        restart: None,
    };
    let program = crate::harness::CompileSession::from_source("PROGRAM Main\nEND_PROGRAM\n")
        .build_bytecode_bytes()
        .unwrap();
    let result = apply_deploy(&root, request(&program)).unwrap();
    assert!(result.written.contains(&DEPLOYMENT_RECORD_FILE.to_string()));
    let record =
        DeploymentRecord::decode(&fs::read(root.join(DEPLOYMENT_RECORD_FILE)).unwrap()).unwrap();
    assert_eq!(record.application.program_sha256, sha256(&program));

    // A program the runtime cannot prepare is refused before anything is written, so the deployed
    // program and its record stay (spec 34 §11.2 step 6; D2).
    let refused = apply_deploy(&root, request(&[1, 2, 3]))
        .expect_err("a program that does not prepare is refused");
    assert!(
        refused.to_string().contains("does not prepare"),
        "unexpected refusal: {refused}"
    );
    assert_eq!(fs::read(root.join("program.stbc")).unwrap(), program);
    let record_after =
        DeploymentRecord::decode(&fs::read(root.join(DEPLOYMENT_RECORD_FILE)).unwrap()).unwrap();
    assert_eq!(record_after.application.program_sha256, sha256(&program));
    let _ = fs::remove_dir_all(root);
}

use super::*;

/// The project's installed configuration with the signed-deploy policy and the keyring of
/// `signed_request`, as a running project holds it before the next deployment.
fn install_signed_policy(root: &Path) {
    fs::write(root.join("runtime.toml"), runtime_with_signed_policy())
        .expect("install runtime.toml");
}

/// Spec 11, deployment signature policy: the policy in force is the installed one. Catches a
/// deployment that turns `require_signed` off with the `runtime.toml` it carries and so
/// installs an unsigned program.
#[test]
fn a_request_cannot_turn_off_require_signed() {
    let mut root = std::env::temp_dir();
    root.push(format!("trust-deploy-lift-policy-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).expect("create root");
    install_signed_policy(&root);
    let unsigned = DeployRequest {
        runtime_toml: Some(
            runtime_with_signed_policy().replace("require_signed = true", "require_signed = false"),
        ),
        io_toml: None,
        program_stbc_b64: Some(STANDARD.encode([4u8, 5, 6])),
        sources: None,
        signature: None,
        restart: None,
    };
    let error =
        apply_deploy(&root, unsigned).expect_err("the installed policy requires a signature");
    assert!(
        error.to_string().contains("signed deploy required"),
        "unexpected error: {error}"
    );
    assert!(
        !root.join("program.stbc").exists(),
        "a refused deployment writes nothing"
    );
    let installed = fs::read_to_string(root.join("runtime.toml")).expect("installed runtime.toml");
    assert!(
        installed.contains("require_signed = true"),
        "the installed policy stays"
    );
    let _ = fs::remove_dir_all(root);
}

/// Spec 11, deployment signature policy: the keyring is the installed policy's. Catches a
/// deployment that names a keyring it controls and signs with a key of its own.
#[test]
fn a_request_cannot_name_its_own_keyring() {
    let mut root = std::env::temp_dir();
    root.push(format!("trust-deploy-own-keyring-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).expect("create root");
    install_signed_policy(&root);
    // The installed keyring trusts "ci"; a file under src/ that an earlier deployment left trusts
    // "intruder".
    fs::create_dir_all(root.join("security")).expect("security dir");
    fs::write(
        root.join("security/deploy-keys.toml"),
        "[[keys]]\nid = \"ci\"\nsecret = \"super-secret\"\nenabled = true\nnot_after_unix = 4102444800\n",
    )
    .expect("write the installed keyring");
    fs::create_dir_all(root.join("src")).expect("src dir");
    fs::write(
        root.join("src/keys.toml"),
        "[[keys]]\nid = \"intruder\"\nsecret = \"own-secret\"\nenabled = true\nnot_after_unix = 4102444800\n",
    )
    .expect("write the other keyring");
    let mut request = DeployRequest {
        runtime_toml: Some(runtime_with_signed_policy().replace(
            "keyring_path = \"security/deploy-keys.toml\"",
            "keyring_path = \"src/keys.toml\"",
        )),
        io_toml: None,
        program_stbc_b64: Some(STANDARD.encode([7u8, 7, 7])),
        sources: None,
        signature: None,
        restart: None,
    };
    let payload_sha = deploy_payload_sha256(&request);
    request.signature = Some(DeploySignature {
        key_id: "intruder".to_string(),
        signature: deploy_signature_digest("own-secret", &payload_sha),
        payload_sha256: payload_sha,
    });
    assert!(
        apply_deploy(&root, request).is_err(),
        "a key the installed keyring does not hold must not sign a deployment"
    );
    assert!(
        !root.join("program.stbc").exists(),
        "a refused deployment writes nothing"
    );
    let _ = fs::remove_dir_all(root);
}

/// A resource that reports a fixed state, for the gate of a deployment.
struct FixedResource {
    state: ResourceState,
    hung: bool,
}

impl ResourcePort for FixedResource {
    fn resource_state(&self) -> ResourceState {
        self.state
    }

    fn last_error(&self) -> Option<RuntimeError> {
        None
    }

    fn is_hung(&self) -> bool {
        self.hung
    }

    fn pause_resource(&self) -> Result<(), crate::scheduler::CommandSendError> {
        Ok(())
    }

    fn resume_resource(&self) -> Result<(), crate::scheduler::CommandSendError> {
        Ok(())
    }

    fn debug_control_mode(&self) -> bool {
        false
    }

    fn debugger_pause(&self) {}

    fn debugger_continue(&self) {}

    fn update_pending_restart(&self, _update: impl FnOnce(&mut Option<crate::RestartMode>)) {}

    fn starts_when_stopped(&self) -> bool {
        true
    }
}

#[test]
fn a_deployment_is_refused_with_the_reply_of_program_install_unless_the_resource_is_stopped() {
    // Specification 34 §11.2 (decision Q11) and 11 §6.8.2: a deployment installs a program, so it
    // is accepted only while the resource is Stopped, and every other state is refused with the
    // message and the code that `program.install` gives for it. Wrong behaviour: a running
    // resource accepts the deployment, or the refusal is a text of the web interface alone.
    for (state, message) in [
        (
            ResourceState::Running,
            "resource is running and its scheduler does not accept commands",
        ),
        (
            ResourceState::Paused,
            "resource is paused and its scheduler does not accept commands",
        ),
        (
            ResourceState::Ready,
            "resource is ready and its scheduler does not accept commands",
        ),
        (
            ResourceState::Faulted,
            "resource is faulted; restart required",
        ),
    ] {
        let resource = FixedResource { state, hung: false };
        assert_eq!(
            refusal_unless_stopped(&resource),
            Some(json!({
                "ok": false,
                "error": message,
                "error_code": "resource_not_running",
            })),
            "{state:?}"
        );
    }
    let stopped = FixedResource {
        state: ResourceState::Stopped,
        hung: false,
    };
    assert_eq!(refusal_unless_stopped(&stopped), None);
}

#[test]
fn a_deployment_to_a_hung_resource_is_refused_before_its_state_is_read() {
    // Specification 11 §6.8.2: a hung scan thread is refused as `program.install` refuses it,
    // even when its state reads Stopped. Wrong behaviour: the state decides and the deployment
    // goes on while no scan thread can take the program.
    let hung = FixedResource {
        state: ResourceState::Stopped,
        hung: true,
    };
    assert_eq!(
        refusal_unless_stopped(&hung),
        Some(json!({
            "ok": false,
            "error": "resource scan thread did not return; the runtime process must be restarted",
            "error_code": "resource_not_running",
        }))
    );
}
