use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use smol_str::SmolStr;
use trust_runtime::bundle_builder::build_program_stbc;
use trust_runtime::bundle_template::{build_io_config_auto, render_io_toml, render_runtime_toml};

static TEMP_DIR_COUNTER: AtomicU64 = AtomicU64::new(1);

fn unique_temp_dir(prefix: &str) -> std::path::PathBuf {
    for _ in 0..64 {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time before unix epoch")
            .as_nanos();
        let seq = TEMP_DIR_COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!(
            "trust-runtime-{prefix}-{}-{nanos}-{seq}",
            std::process::id()
        ));
        match std::fs::create_dir(&dir) {
            Ok(()) => return dir,
            Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(err) => panic!("create temp dir {}: {err}", dir.display()),
        }
    }
    panic!("failed to allocate unique temp dir for '{prefix}'")
}

fn make_runtime_toml_portable(runtime_toml: String) -> String {
    #[cfg(windows)]
    {
        return runtime_toml.replacen(
            "endpoint = \"unix:///tmp/trust-runtime.sock\"",
            "endpoint = \"tcp://127.0.0.1:0\"\nauth_token = \"trust-ci-token\"",
            1,
        );
    }
    #[cfg(not(windows))]
    runtime_toml
}

fn write_project_fixture(root: &std::path::Path, resource_name: &str) {
    std::fs::create_dir_all(root.join("src")).expect("create source directory");
    let runtime_toml =
        make_runtime_toml_portable(render_runtime_toml(&SmolStr::new(resource_name), 100));
    let io_template = build_io_config_auto("loopback").expect("build loopback io template");
    let io_toml = render_io_toml(&io_template);
    std::fs::write(root.join("runtime.toml"), runtime_toml).expect("write runtime.toml");
    std::fs::write(root.join("io.toml"), io_toml).expect("write io.toml");
    std::fs::write(
        root.join("src").join("main.st"),
        r#"
PROGRAM Main
VAR
    Counter : INT := 0;
END_VAR
Counter := Counter + 1;
END_PROGRAM
"#,
    )
    .expect("write main source");
    std::fs::write(
        root.join("src").join("config.st"),
        format!(
            "CONFIGURATION Config\nRESOURCE {resource_name} ON PLC\n    TASK MainTask (INTERVAL := T#100ms, PRIORITY := 1);\n    PROGRAM P1 WITH MainTask : Main;\nEND_RESOURCE\nEND_CONFIGURATION\n"
        ),
    )
    .expect("write config source");
    let report = build_program_stbc(root, None).expect("build program.stbc");
    assert!(report.program_path.is_file(), "program.stbc should exist");
}

fn run_registry_command(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_trust-runtime"))
        .args(args)
        .output()
        .expect("run trust-runtime registry command")
}

#[test]
fn registry_profile_json_matches_contract() {
    let output = run_registry_command(&["registry", "profile", "--json"]);
    assert!(
        output.status.success(),
        "expected profile success, stderr was:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let profile: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("parse profile json");
    assert_eq!(
        profile
            .get("api_version")
            .and_then(serde_json::Value::as_str)
            .expect("api_version"),
        "v1"
    );
    assert_eq!(
        profile
            .get("schema_version")
            .and_then(serde_json::Value::as_u64)
            .expect("schema_version"),
        1
    );
    let endpoints = profile
        .get("endpoints")
        .and_then(serde_json::Value::as_array)
        .expect("endpoints");
    assert!(endpoints.iter().any(|entry| {
        entry.get("path").and_then(serde_json::Value::as_str)
            == Some("/v1/packages/{name}/{version}")
    }));
    assert!(endpoints.iter().any(|entry| {
        entry.get("path").and_then(serde_json::Value::as_str)
            == Some("/v1/packages/{name}/{version}/verify")
    }));
    assert!(endpoints.iter().any(|entry| {
        entry.get("path").and_then(serde_json::Value::as_str) == Some("/v1/index")
    }));
    let package_fields = profile
        .get("metadata_model")
        .and_then(|model| model.get("package_fields"))
        .and_then(serde_json::Value::as_array)
        .expect("package fields");
    assert!(package_fields
        .iter()
        .any(|field| { field.as_str() == Some("package_sha256") }));
}

#[test]
fn registry_publish_download_verify_round_trip() {
    let project = unique_temp_dir("registry-publish-project");
    let registry = unique_temp_dir("registry-publish-root");
    let download = unique_temp_dir("registry-download-target");
    write_project_fixture(&project, "pkg_alpha");

    let init = Command::new(env!("CARGO_BIN_EXE_trust-runtime"))
        .arg("registry")
        .arg("init")
        .arg("--root")
        .arg(&registry)
        .arg("--visibility")
        .arg("public")
        .output()
        .expect("init registry");
    assert!(
        init.status.success(),
        "expected init success, stderr was:\n{}",
        String::from_utf8_lossy(&init.stderr)
    );

    let publish = Command::new(env!("CARGO_BIN_EXE_trust-runtime"))
        .arg("registry")
        .arg("publish")
        .arg("--registry")
        .arg(&registry)
        .arg("--project")
        .arg(&project)
        .arg("--version")
        .arg("1.0.0")
        .output()
        .expect("publish package");
    assert!(
        publish.status.success(),
        "expected publish success, stderr was:\n{}",
        String::from_utf8_lossy(&publish.stderr)
    );

    let verify = Command::new(env!("CARGO_BIN_EXE_trust-runtime"))
        .arg("registry")
        .arg("verify")
        .arg("--registry")
        .arg(&registry)
        .arg("--name")
        .arg("pkg_alpha")
        .arg("--version")
        .arg("1.0.0")
        .output()
        .expect("verify package");
    assert!(
        verify.status.success(),
        "expected verify success, stderr was:\n{}",
        String::from_utf8_lossy(&verify.stderr)
    );

    let list = Command::new(env!("CARGO_BIN_EXE_trust-runtime"))
        .arg("registry")
        .arg("list")
        .arg("--registry")
        .arg(&registry)
        .arg("--json")
        .output()
        .expect("list packages");
    assert!(
        list.status.success(),
        "expected list success, stderr was:\n{}",
        String::from_utf8_lossy(&list.stderr)
    );
    let packages: serde_json::Value =
        serde_json::from_slice(&list.stdout).expect("parse list payload");
    let package = packages
        .as_array()
        .and_then(|entries| {
            entries.iter().find(|entry| {
                entry.get("name").and_then(serde_json::Value::as_str) == Some("pkg_alpha")
                    && entry.get("version").and_then(serde_json::Value::as_str) == Some("1.0.0")
            })
        })
        .expect("package summary in list");
    let package_sha = package
        .get("package_sha256")
        .and_then(serde_json::Value::as_str)
        .expect("package sha");
    assert_eq!(package_sha.len(), 64);

    let download_result = Command::new(env!("CARGO_BIN_EXE_trust-runtime"))
        .arg("registry")
        .arg("download")
        .arg("--registry")
        .arg(&registry)
        .arg("--name")
        .arg("pkg_alpha")
        .arg("--version")
        .arg("1.0.0")
        .arg("--output")
        .arg(&download)
        .arg("--verify")
        .output()
        .expect("download package");
    assert!(
        download_result.status.success(),
        "expected download success, stderr was:\n{}",
        String::from_utf8_lossy(&download_result.stderr)
    );

    assert!(download.join("runtime.toml").is_file());
    assert!(download.join("io.toml").is_file());
    assert!(download.join("program.stbc").is_file());
    assert!(download.join("src").join("main.st").is_file());
    assert_eq!(
        std::fs::read(project.join("program.stbc")).expect("read source bytecode"),
        std::fs::read(download.join("program.stbc")).expect("read downloaded bytecode")
    );

    let _ = std::fs::remove_dir_all(project);
    let _ = std::fs::remove_dir_all(registry);
    let _ = std::fs::remove_dir_all(download);
}

#[test]
fn registry_private_access_control_requires_token() {
    let project = unique_temp_dir("registry-private-project");
    let registry = unique_temp_dir("registry-private-root");
    write_project_fixture(&project, "pkg_private");

    let init = Command::new(env!("CARGO_BIN_EXE_trust-runtime"))
        .arg("registry")
        .arg("init")
        .arg("--root")
        .arg(&registry)
        .arg("--visibility")
        .arg("private")
        .arg("--token")
        .arg("secret-token")
        .output()
        .expect("init private registry");
    assert!(
        init.status.success(),
        "expected private init success, stderr was:\n{}",
        String::from_utf8_lossy(&init.stderr)
    );

    let publish_without_token = Command::new(env!("CARGO_BIN_EXE_trust-runtime"))
        .arg("registry")
        .arg("publish")
        .arg("--registry")
        .arg(&registry)
        .arg("--project")
        .arg(&project)
        .arg("--version")
        .arg("2.0.0")
        .output()
        .expect("publish without token");
    assert!(
        !publish_without_token.status.success(),
        "expected publish failure without token"
    );
    let unauthorized = String::from_utf8_lossy(&publish_without_token.stderr);
    assert!(
        unauthorized.contains("unauthorized"),
        "expected unauthorized failure, stderr was:\n{unauthorized}"
    );

    let publish_with_token = Command::new(env!("CARGO_BIN_EXE_trust-runtime"))
        .arg("registry")
        .arg("publish")
        .arg("--registry")
        .arg(&registry)
        .arg("--project")
        .arg(&project)
        .arg("--version")
        .arg("2.0.0")
        .arg("--token")
        .arg("secret-token")
        .output()
        .expect("publish with token");
    assert!(
        publish_with_token.status.success(),
        "expected publish success with token, stderr was:\n{}",
        String::from_utf8_lossy(&publish_with_token.stderr)
    );

    let list_wrong_token = Command::new(env!("CARGO_BIN_EXE_trust-runtime"))
        .arg("registry")
        .arg("list")
        .arg("--registry")
        .arg(&registry)
        .arg("--token")
        .arg("wrong-token")
        .output()
        .expect("list with wrong token");
    assert!(
        !list_wrong_token.status.success(),
        "expected list failure with wrong token"
    );

    let verify_with_token = Command::new(env!("CARGO_BIN_EXE_trust-runtime"))
        .arg("registry")
        .arg("verify")
        .arg("--registry")
        .arg(&registry)
        .arg("--name")
        .arg("pkg_private")
        .arg("--version")
        .arg("2.0.0")
        .arg("--token")
        .arg("secret-token")
        .output()
        .expect("verify with token");
    assert!(
        verify_with_token.status.success(),
        "expected verify success with token, stderr was:\n{}",
        String::from_utf8_lossy(&verify_with_token.stderr)
    );

    let _ = std::fs::remove_dir_all(project);
    let _ = std::fs::remove_dir_all(registry);
}
