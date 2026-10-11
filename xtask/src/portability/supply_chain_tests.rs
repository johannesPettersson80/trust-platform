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
  if [ "$EMIT_EXCEPTION" = 1 ]; then
    printf '%s\n' --ignore RUSTSEC-2026-0110
  fi
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
    for exceptions in [true, false] {
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
            // Exercise the native system shell, including macOS's Bash 3.2.
            let result = Command::new("/bin/bash")
                .arg(fixture.0.join("scripts/supply_chain_gate.sh"))
                .current_dir(std::env::temp_dir())
                .env("PATH", std::env::join_paths(paths).unwrap())
                .env("AUDIT_COMMAND_LOG", &log)
                .env("FAIL_FRAGMENT", failure)
                .env("EMIT_EXCEPTION", if exceptions { "1" } else { "0" })
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
                "deny --locked --manifest-path firmware/trust-nucleo-f401re/Cargo.toml --config "
            ));
            assert!(commands[1].ends_with(" check advisories licenses bans sources"));
            let suffix = if exceptions {
                " --ignore RUSTSEC-2026-0110"
            } else {
                ""
            };
            assert_eq!(
                commands[2],
                format!("audit --json --file Cargo.lock{suffix}")
            );
            assert_eq!(
                commands[3],
                format!("audit --json --file firmware/trust-nucleo-f401re/Cargo.lock{suffix}")
            );
        }
    }
}

#[test]
fn cached_ci_tools_are_replaced_by_the_selected_versions() {
    for (workflow, step, expected) in [
        (
            include_str!("../../../.github/workflows/ci.yml"),
            "Install cargo-deny and cargo-audit",
            &[("cargo-deny", "0.20.2"), ("cargo-audit", "0.22.2")][..],
        ),
        (
            include_str!("../../../.github/workflows/ci.yml"),
            "Install wasm-pack",
            &[("wasm-pack", "0.15.0")][..],
        ),
        (
            include_str!("../../../.github/workflows/release.yml"),
            "Install wasm-pack",
            &[("wasm-pack", "0.15.0")][..],
        ),
    ] {
        let fixture = fixture();
        let cargo_bin = fixture.0.join(".cargo/bin");
        fs::create_dir_all(&cargo_bin).unwrap();
        let cargo = cargo_bin.join("cargo");
        fs::write(
            &cargo,
            r#"#!/bin/sh
if [ "$1" = install ]; then
  printf '%s\n' "$4" > "$HOME/$2-version"
elif [ "$2" = --version ]; then
  printf 'cargo-%s %s\n' "$1" "$(cat "$HOME/cargo-$1-version")"
else
  exit 19
fi
"#,
        )
        .unwrap();
        fs::set_permissions(&cargo, fs::Permissions::from_mode(0o755)).unwrap();
        let wasm_pack = cargo_bin.join("wasm-pack");
        fs::write(
            &wasm_pack,
            "#!/bin/sh\nprintf 'wasm-pack %s\\n' \"$(cat \"$HOME/wasm-pack-version\")\"\n",
        )
        .unwrap();
        fs::set_permissions(&wasm_pack, fs::Permissions::from_mode(0o755)).unwrap();
        for (tool, _) in expected {
            fs::write(fixture.0.join(format!("{tool}-version")), "0.0.1\n").unwrap();
        }
        let step_marker = format!("      - name: {step}\n");
        let step_body = workflow.split_once(&step_marker).unwrap().1;
        let run_body = step_body.split_once("        run: |\n").unwrap().1;
        let script = run_body
            .lines()
            .take_while(|line| line.starts_with("          ") || line.is_empty())
            .map(|line| line.strip_prefix("          ").unwrap_or(line))
            .collect::<Vec<_>>()
            .join("\n");
        let result = Command::new("/bin/bash")
            .args(["-ec", &script])
            .env("HOME", &fixture.0)
            .env("PATH", format!("{}:/usr/bin:/bin", cargo_bin.display()))
            .env("GITHUB_PATH", fixture.0.join("github-path"))
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{step}: {}",
            String::from_utf8_lossy(&result.stderr)
        );
        for (tool, version) in expected {
            assert_eq!(
                fs::read_to_string(fixture.0.join(format!("{tool}-version"))).unwrap(),
                format!("{version}\n"),
                "{step} must replace cached {tool}"
            );
        }
    }
}
