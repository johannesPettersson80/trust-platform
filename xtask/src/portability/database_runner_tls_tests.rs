//! Execute the owning TLS provisioning function with a restrictive host umask.
use std::{
    fs,
    os::unix::fs::{MetadataExt, PermissionsExt},
    path::PathBuf,
    process::Command,
};

struct Fixture(PathBuf);

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn missing_generated_database_input_cannot_use_stale_child_settings() {
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let fixture = Fixture(std::env::temp_dir().join(format!(
        "trust-database-environment-{}-{nonce}",
        std::process::id()
    )));
    let mut command = Command::new("bash");
    command
        .arg("scripts/openot_real_database_gate.sh")
        .current_dir(PathBuf::from(env!("CARGO_MANIFEST_DIR")).parent().unwrap())
        .env("TRUST_OPENOT_EVIDENCE_DIR", &fixture.0);
    for name in super::generated_names().chain(
        super::PRODUCTS
            .into_iter()
            .map(|product| format!("TRUST_TEST_OPENOT_{product}_CA")),
    ) {
        command.env(name, "stale-parent-secret");
    }
    super::apply_settings(
        &mut command,
        super::decode("TRUST_TEST_OPENOT_MYSQL_URL=generated-secret\n").unwrap(),
    );
    let output = command.output().unwrap();
    assert_eq!(output.status.code(), Some(2));
    let error = String::from_utf8(output.stderr).unwrap();
    assert!(error.contains("TRUST_TEST_OPENOT_POSTGRES_URL"));
    assert!(!error.contains("TRUST_TEST_OPENOT_MYSQL_URL"));
    assert!(!error.contains("stale-parent-secret"));
    assert!(!error.contains("generated-secret"));
}

#[test]
fn postgres_tls_mount_is_traversable_with_private_keys_under_umask_077() {
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let fixture = Fixture(
        std::env::temp_dir().join(format!("trust-database-tls-{}-{nonce}", std::process::id())),
    );
    let root = &fixture.0;
    fs::create_dir_all(root.join("tls")).unwrap();
    fs::create_dir(root.join("bin")).unwrap();
    for directory in [root.clone(), root.join("tls")] {
        fs::set_permissions(directory, fs::Permissions::from_mode(0o700)).unwrap();
    }
    for name in ["server.crt", "server.key", "ca.key"] {
        let file = root.join("tls").join(name);
        fs::write(&file, "native permission fixture\n").unwrap();
        fs::set_permissions(file, fs::Permissions::from_mode(0o600)).unwrap();
    }
    for (name, body) in [
        (
            "docker",
            "#!/bin/sh\ncase \"$*\" in *'id -u postgres') id -u;; *'id -g postgres') id -g;; *) exit 17;; esac\n",
        ),
        // Run real chmod/chown as the file owner, without requesting privileges.
        ("sudo", "#!/bin/sh\nexec \"$@\"\n"),
    ] {
        let file = root.join("bin").join(name);
        fs::write(&file, body).unwrap();
        fs::set_permissions(file, fs::Permissions::from_mode(0o755)).unwrap();
    }
    let script = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../scripts/openot_database_runner_prepare.sh"
    ));
    let start = script.find("prepare_postgres_tls() {\n").unwrap();
    let end = start + script[start..].find("\n}\n").unwrap() + 3;
    let function = &script[start..end];
    let mut paths = vec![root.join("bin")];
    paths.extend(std::env::split_paths(&std::env::var_os("PATH").unwrap()));
    let output = Command::new("bash")
        .arg("-c")
        .arg(format!(
            "set -euo pipefail\numask 077\n{function}\nprepare_postgres_tls fixture \"$state_dir/postgres-tls\"\nprepare_postgres_tls fixture \"$state_dir/timescale-tls\"\nid -u\nid -g\n"
        ))
        .env("state_dir", root)
        .env(
            "ownership_label",
            "com.trust.openot.runner=trust-openot-native",
        )
        .env("PATH", std::env::join_paths(paths).unwrap())
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "TLS fixture setup failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    // A shared TMPDIR may have a setgid directory; service IDs come from Docker.
    let identity = String::from_utf8(output.stdout).unwrap();
    let ids: Vec<u32> = identity.lines().map(|line| line.parse().unwrap()).collect();
    assert_eq!(ids.len(), 2);
    for product in ["postgres-tls", "timescale-tls"] {
        let directory = root.join(product);
        assert_eq!(fs::metadata(&directory).unwrap().mode() & 0o777, 0o755);
        for (name, mode) in [("server.crt", 0o644), ("server.key", 0o600)] {
            let metadata = fs::metadata(directory.join(name)).unwrap();
            assert_eq!(metadata.mode() & 0o777, mode);
            assert_eq!(metadata.uid(), ids[0]);
            assert_eq!(metadata.gid(), ids[1]);
        }
    }
    for directory in [root.clone(), root.join("tls")] {
        assert_eq!(fs::metadata(directory).unwrap().mode() & 0o777, 0o700);
    }
    assert_eq!(
        fs::metadata(root.join("tls/ca.key")).unwrap().mode() & 0o777,
        0o600
    );
}
