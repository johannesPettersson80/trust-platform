//! Test shell orchestration only; the real dependency policy runs in the batch.
use std::{fs, os::unix::fs::PermissionsExt, path::PathBuf, process::Command};

struct Fixture(PathBuf);
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn fixture() -> Fixture {
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root =
        std::env::temp_dir().join(format!("trust-lock-audits-{}-{nonce}", std::process::id()));
    fs::create_dir_all(root.join("scripts")).unwrap();
    fs::create_dir(root.join("bin")).unwrap();
    fs::write(
        root.join("scripts/supply_chain_gate.sh"),
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../scripts/supply_chain_gate.sh"
        )),
    )
    .unwrap();
    for (name, body) in [
        (
            "cargo",
            r#"#!/bin/sh
printf '%s\n' "$*" >> "$AUDIT_COMMAND_LOG"
case "$*" in *"$FAIL_FRAGMENT"*) if [ -n "$FAIL_FRAGMENT" ]; then exit 17; fi;; esac
printf '{}\n'
"#,
        ),
        (
            "python3",
            r#"#!/bin/sh
if [ "$1" = '-' ]; then
  printf '%s\n' --ignore RUSTSEC-2026-0110
else
  cat >/dev/null
fi
"#,
        ),
    ] {
        let path = root.join("bin").join(name);
        fs::write(&path, body).unwrap();
        fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
    }
    Fixture(root)
}

#[test]
fn both_lock_graphs_are_audited_and_any_graph_failure_fails_the_gate() {
    for failure in [
        "",
        "deny --locked check",
        "--manifest-path firmware/",
        "audit --json --file Cargo.lock",
        "audit --json --file firmware/",
    ] {
        let fixture = fixture();
        let log = fixture.0.join("commands.txt");
        let mut paths = vec![fixture.0.join("bin")];
        paths.extend(std::env::split_paths(&std::env::var_os("PATH").unwrap()));
        let result = Command::new("bash")
            .arg(fixture.0.join("scripts/supply_chain_gate.sh"))
            .current_dir(std::env::temp_dir())
            .env("PATH", std::env::join_paths(paths).unwrap())
            .env("AUDIT_COMMAND_LOG", &log)
            .env("FAIL_FRAGMENT", failure)
            .output()
            .unwrap();
        assert_eq!(
            result.status.success(),
            failure.is_empty(),
            "{failure}: {}",
            String::from_utf8_lossy(&result.stderr)
        );
        let commands = fs::read_to_string(log).unwrap();
        let commands: Vec<_> = commands.lines().collect();
        assert_eq!(commands.len(), 4, "every independent graph check must run");
        assert_eq!(
            commands[0],
            "deny --locked check advisories licenses bans sources"
        );
        assert!(commands[1].starts_with(
            "deny --locked --manifest-path firmware/trust-nucleo-f401re/Cargo.toml check --config "
        ));
        assert_eq!(
            commands[2],
            "audit --json --file Cargo.lock --ignore RUSTSEC-2026-0110"
        );
        assert_eq!(commands[3], "audit --json --file firmware/trust-nucleo-f401re/Cargo.lock --ignore RUSTSEC-2026-0110");
    }
}
