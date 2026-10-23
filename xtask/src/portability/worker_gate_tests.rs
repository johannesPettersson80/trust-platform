//! Pin the shared worker gate's locked frontend prerequisite, not browser semantics.
use std::{fs, os::unix::fs::PermissionsExt, path::PathBuf, process::Command};

struct Fixture(PathBuf);
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn worker_import_tests_require_the_locked_frontend_install_to_succeed() {
    for install_fails in [false, true] {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let fixture = Fixture(std::env::temp_dir().join(format!(
            "trust-worker-prerequisite-{}-{nonce}",
            std::process::id()
        )));
        fs::create_dir_all(fixture.0.join("scripts")).unwrap();
        let gate = fixture
            .0
            .join("scripts/check_browser_analysis_worker_recovery.sh");
        fs::write(
            &gate,
            include_str!("../../../scripts/check_browser_analysis_worker_recovery.sh"),
        )
        .unwrap();
        for path in [
            "docs/internal/prototypes/browser_analysis_wasm_spike/web/analysis-client.test.mjs",
            "docs/demo/lsp-position-resolver.test.mjs",
            "docs/demo/walkthrough-actions.test.mjs",
            "crates/trust-runtime/web/ide-frontend/src/monaco-bundle-imports.test.mjs",
            "docs/demo/monaco-bundle-artifact.test.mjs",
            "docs/demo/wasm-ready-race.test.mjs",
        ] {
            let path = fixture.0.join(path);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, "").unwrap();
        }
        let bin = fixture.0.join("bin");
        fs::create_dir(&bin).unwrap();
        for name in ["node", "npm"] {
            let path = bin.join(name);
            fs::write(
                &path,
                "#!/bin/sh\nprintf '%s %s\\n' \"$(basename \"$0\")\" \"$*\" >> \"$WORKER_COMMAND_LOG\"\nif [ \"$(basename \"$0\")\" = npm ] && [ \"$INSTALL_FAILS\" = 1 ]; then exit 17; fi\n",
            )
            .unwrap();
            fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
        }
        let log = fixture.0.join("commands.txt");
        let result = Command::new("/bin/bash")
            .arg(&gate)
            .env("PATH", format!("{}:/usr/bin:/bin", bin.display()))
            .env("WORKER_COMMAND_LOG", &log)
            .env("INSTALL_FAILS", if install_fails { "1" } else { "0" })
            .output()
            .unwrap();
        assert_eq!(result.status.success(), !install_fails);
        let commands = fs::read_to_string(log).unwrap();
        let rows: Vec<_> = commands.lines().collect();
        assert_eq!(
            rows[0],
            format!(
                "npm --prefix {} ci",
                fixture
                    .0
                    .join("crates/trust-runtime/web/ide-frontend")
                    .display()
            )
        );
        assert_eq!(rows.len(), if install_fails { 1 } else { 7 });
        assert!(rows[1..].iter().all(|row| row.starts_with("node --test ")));
    }
}
