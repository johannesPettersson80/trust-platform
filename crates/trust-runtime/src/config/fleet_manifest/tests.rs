//! The fleet manifest checks and the control token reading. The manifest tests moved here with
//! the code from the `fleet` command; the token test pins specification 25, section 6.4.

use std::time::{SystemTime, UNIX_EPOCH};

use super::*;

#[test]
fn load_manifest_rejects_unsafe_or_duplicate_runtime_names() {
    let root = temp_dir("fleet-invalid-names");
    let manifest_path = root.join(FLEET_MANIFEST_FILE);
    fs::write(
        &manifest_path,
        r#"
[[runtime]]
name = "../escape"
path = "first"
control_endpoint = "tcp://127.0.0.1:19920"
web_port = 19921
"#,
    )
    .expect("write unsafe manifest");
    let unsafe_error = load_manifest(&manifest_path).expect_err("unsafe name must fail");
    assert!(
        unsafe_error.to_string().contains("runtime name"),
        "unexpected error: {unsafe_error:#}"
    );

    fs::write(
        &manifest_path,
        r#"
[[runtime]]
name = "cell"
path = "first"
control_endpoint = "tcp://127.0.0.1:19920"
web_port = 19921

[[runtime]]
name = "cell"
path = "second"
control_endpoint = "tcp://127.0.0.1:19922"
web_port = 19923
"#,
    )
    .expect("write duplicate manifest");
    let duplicate_error = load_manifest(&manifest_path).expect_err("duplicate name must fail");
    assert!(
        duplicate_error
            .to_string()
            .contains("duplicate fleet runtime name 'cell'"),
        "unexpected error: {duplicate_error:#}"
    );

    fs::write(
        &manifest_path,
        r#"
[[runtime]]
name = " cell "
path = "cell"
control_endpoint = "tcp://127.0.0.1:19920"
web_port = 19921
"#,
    )
    .expect("write whitespace name manifest");
    let whitespace_error = load_manifest(&manifest_path).expect_err("whitespace name must fail");
    assert!(
        whitespace_error
            .to_string()
            .contains("leading or trailing whitespace"),
        "unexpected error: {whitespace_error:#}"
    );
    fs::remove_dir_all(root).ok();
}

#[test]
fn load_manifest_rejects_invalid_endpoints_and_port_collisions() {
    let root = temp_dir("fleet-invalid-ports");
    let manifest_path = root.join(FLEET_MANIFEST_FILE);
    fs::write(
        &manifest_path,
        r#"
[[runtime]]
name = "first"
path = "first"
control_endpoint = "not-an-endpoint"
web_port = 19931
"#,
    )
    .expect("write invalid endpoint manifest");
    let endpoint_error =
        load_manifest(&manifest_path).expect_err("invalid control endpoint must fail");
    assert!(
        endpoint_error.to_string().contains("control endpoint"),
        "unexpected error: {endpoint_error:#}"
    );

    fs::write(
        &manifest_path,
        r#"
[[runtime]]
name = "first"
path = "first"
control_endpoint = "tcp://127.0.0.1:19930"
web_port = 19931

[[runtime]]
name = "second"
path = "second"
control_endpoint = "tcp://127.0.0.1:19932"
web_port = 19930
"#,
    )
    .expect("write colliding manifest");
    let collision_error =
        load_manifest(&manifest_path).expect_err("cross-role port collision must fail");
    assert!(
        collision_error
            .to_string()
            .contains("fleet TCP port 19930 is used more than once"),
        "unexpected error: {collision_error:#}"
    );

    fs::write(
        &manifest_path,
        r#"
[[runtime]]
name = "first"
path = "first"
control_endpoint = "tcp://127.0.0.1:19930"
web_port = 0
"#,
    )
    .expect("write zero web port manifest");
    let zero_port_error = load_manifest(&manifest_path).expect_err("zero web port must fail");
    assert!(
        zero_port_error
            .to_string()
            .contains("fleet TCP ports must be concrete, not 0"),
        "unexpected error: {zero_port_error:#}"
    );
    fs::remove_dir_all(root).ok();
}

#[test]
fn managed_runtime_project_paths_resolve_relative_and_preserve_absolute_paths() {
    let root = Path::new("fleet-root");
    assert_eq!(
        runtime_project_path(root, "cell"),
        root.join("cell"),
        "relative manifest paths resolve beneath the fleet root"
    );

    let absolute = std::env::current_dir()
        .expect("current directory")
        .join("external-managed-runtime");
    assert_eq!(
        runtime_project_path(root, absolute.to_string_lossy().as_ref()),
        absolute,
        "absolute hand-edited manifest paths remain absolute"
    );
}

/// The sections of a valid `runtime.toml` after `[runtime.control]`, with a mesh token that is not
/// the control token.
const RUNTIME_SECTIONS: &str = r#"
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

[runtime.mesh]
enabled = false
listen = "0.0.0.0:5200"
tls = false
auth_token = "mesh-secret"
publish = []
subscribe = {}
"#;

const BUNDLE_SECTIONS: &str = r#"
[bundle]
version = 1

[resource]
name = "cell"
cycle_interval_ms = 100
"#;

#[test]
fn managed_token_only_from_root_runtime_control() {
    let root = temp_dir("fleet-managed-token");
    let read = |text: String| {
        fs::write(root.join("runtime.toml"), text).expect("write runtime.toml");
        read_control_auth_token(&root).expect("a valid runtime.toml")
    };

    // The `[runtime.control]` table supplies the token, not the mesh table's key of that name.
    let table = format!(
        "{BUNDLE_SECTIONS}\n[runtime.control]\nendpoint = \"unix:///tmp/trust-fleet-token.sock\"\n\
         auth_token = \" managed-secret \" # local runtime token\n{RUNTIME_SECTIONS}"
    );
    assert_eq!(read(table).as_deref(), Some("managed-secret"));

    // The dotted form at the root is the same table.
    let dotted = format!(
        "runtime.control.endpoint = \"unix:///tmp/trust-fleet-token.sock\"\n\
         runtime.control.auth_token = \"managed-dotted-secret\" # local runtime token\n\
         {BUNDLE_SECTIONS}{RUNTIME_SECTIONS}"
    );
    assert_eq!(read(dotted).as_deref(), Some("managed-dotted-secret"));

    // An `auth_token` below another table never supplies runtime-control authority.
    let other_table = format!(
        "{BUNDLE_SECTIONS}\n[runtime.control]\nendpoint = \"unix:///tmp/trust-fleet-token.sock\"\n\
         {RUNTIME_SECTIONS}"
    );
    assert_eq!(read(other_table), None);
    fs::remove_dir_all(root).ok();
}

fn temp_dir(prefix: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "{prefix}-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock after epoch")
            .as_nanos()
    ));
    fs::create_dir_all(&root).expect("create temp dir");
    root
}
