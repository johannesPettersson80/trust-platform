use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(1);

fn temp_project(name: &str) -> PathBuf {
    let sequence = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let root = std::env::temp_dir().join(format!(
        "trust-hmi-{name}-{}-{sequence}",
        std::process::id()
    ));
    std::fs::create_dir_all(root.join("src")).expect("create HMI test project");
    root
}

fn snapshot_tree(root: &Path) -> Vec<(PathBuf, Option<Vec<u8>>)> {
    fn visit(root: &Path, current: &Path, entries: &mut Vec<(PathBuf, Option<Vec<u8>>)>) {
        let mut children = std::fs::read_dir(current)
            .expect("read snapshot directory")
            .map(|entry| entry.expect("read snapshot entry").path())
            .collect::<Vec<_>>();
        children.sort();

        for path in children {
            let relative = path
                .strip_prefix(root)
                .expect("snapshot entry under root")
                .to_path_buf();
            let file_type = std::fs::symlink_metadata(&path)
                .expect("read snapshot metadata")
                .file_type();
            if file_type.is_dir() {
                entries.push((relative, None));
                visit(root, &path, entries);
            } else if file_type.is_file() {
                entries.push((
                    relative,
                    Some(std::fs::read(&path).expect("read snapshot file")),
                ));
            } else {
                panic!(
                    "unexpected non-file HMI descriptor entry: {}",
                    path.display()
                );
            }
        }
    }

    let mut entries = Vec::new();
    if root.is_dir() {
        visit(root, root, &mut entries);
    }
    entries
}

#[test]
fn hmi_init_treats_glob_metacharacters_in_project_path_literally() {
    let project = temp_project("[literal]");
    std::fs::write(
        project.join("src/Main.st"),
        r#"PROGRAM Main
VAR
    Running : BOOL;
END_VAR
Running := TRUE;
END_PROGRAM
"#,
    )
    .expect("write HMI source");

    let output = Command::new(env!("CARGO_BIN_EXE_trust-runtime"))
        .args(["hmi", "--project"])
        .arg(&project)
        .arg("init")
        .output()
        .expect("run HMI init");

    assert!(
        output.status.success(),
        "HMI init failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(project.join("hmi/_config.toml").is_file());
    assert!(project.join("hmi/overview.toml").is_file());
    let _ = std::fs::remove_dir_all(project);
}

#[test]
fn hmi_init_compiles_resolved_local_dependency_sources() {
    let project = temp_project("local-dependency");
    let dependency = project.join("deps/math/src");
    std::fs::create_dir_all(&dependency).expect("create dependency sources");
    std::fs::write(
        project.join("trust-lsp.toml"),
        "[dependencies]\nMath = \"deps/math\"\n",
    )
    .expect("write dependency manifest");
    std::fs::write(
        dependency.join("Math.st"),
        r#"FUNCTION DepDouble : INT
VAR_INPUT
    Value : INT;
END_VAR
DepDouble := Value * 2;
END_FUNCTION
"#,
    )
    .expect("write dependency source");
    std::fs::write(
        project.join("src/Main.st"),
        r#"PROGRAM Main
VAR
    Input : INT := 2;
    Output : INT;
END_VAR
Output := DepDouble(Input);
END_PROGRAM
"#,
    )
    .expect("write project source");

    let output = Command::new(env!("CARGO_BIN_EXE_trust-runtime"))
        .args(["hmi", "--project"])
        .arg(&project)
        .arg("init")
        .output()
        .expect("run HMI init with dependency");

    assert!(
        output.status.success(),
        "HMI init failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(project.join("hmi/_config.toml").is_file());
    let _ = std::fs::remove_dir_all(project);
}

#[test]
fn hmi_without_project_preserves_bundle_detection_failure() {
    let project = temp_project("undetected-current-dir");
    std::fs::write(
        project.join("src/Main.st"),
        r#"PROGRAM Main
VAR
    Running : BOOL;
END_VAR
Running := TRUE;
END_PROGRAM
"#,
    )
    .expect("write HMI source");

    let output = Command::new(env!("CARGO_BIN_EXE_trust-runtime"))
        .args(["hmi", "init"])
        .current_dir(&project)
        .output()
        .expect("run HMI init without project selection");

    assert!(
        !output.status.success(),
        "HMI init must preserve bundle-detection failure\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("project folder not found"),
        "unexpected stderr:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        !project.join("hmi").exists(),
        "failed project detection must not create an HMI descriptor"
    );
    assert!(
        !String::from_utf8_lossy(&output.stdout).contains("Generated HMI scaffold"),
        "failed project detection must not print a success summary"
    );
    let _ = std::fs::remove_dir_all(project);
}

#[test]
fn hmi_source_and_compile_failures_leave_descriptor_unchanged() {
    for (name, source, expected_error) in [
        ("no-source", None, "no ST sources found under"),
        (
            "compile-error",
            Some(
                r#"PROGRAM Main
VAR
    Running : NOT_A_TYPE;
END_VAR
END_PROGRAM
"#,
            ),
            "error[E102]: cannot resolve type 'NOT_A_TYPE'",
        ),
    ] {
        let project = temp_project(name);
        if let Some(source) = source {
            std::fs::write(project.join("src/Main.st"), source).expect("write invalid HMI source");
        }
        std::fs::create_dir_all(project.join("hmi")).expect("create existing HMI descriptor");
        std::fs::write(project.join("hmi/custom.txt"), "operator-owned")
            .expect("write operator-owned HMI file");
        std::fs::create_dir_all(project.join("hmi/operator")).expect("create operator directory");
        std::fs::write(
            project.join("hmi/operator/note.txt"),
            "nested operator state",
        )
        .expect("write nested operator state");
        let descriptor_before = snapshot_tree(&project.join("hmi"));

        let output = Command::new(env!("CARGO_BIN_EXE_trust-runtime"))
            .args(["hmi", "--project"])
            .arg(&project)
            .arg("reset")
            .output()
            .expect("run failing HMI reset");

        assert!(
            !output.status.success(),
            "{name} must fail before HMI mutation\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(
            String::from_utf8_lossy(&output.stderr).contains(expected_error),
            "{name} returned the wrong error category\nstderr:\n{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            snapshot_tree(&project.join("hmi")),
            descriptor_before,
            "{name} changed the HMI descriptor before compilation succeeded"
        );
        assert!(
            std::fs::read_dir(&project)
                .expect("read project directory")
                .all(|entry| !entry
                    .expect("read project entry")
                    .file_name()
                    .to_string_lossy()
                    .starts_with("hmi.backup.")),
            "{name} must not create a backup before source compilation succeeds"
        );
        assert!(
            !String::from_utf8_lossy(&output.stdout).contains("Generated HMI scaffold"),
            "{name} must not print a success summary"
        );
        let _ = std::fs::remove_dir_all(project);
    }
}

#[test]
fn hmi_update_preserves_user_files_and_reset_creates_backup() {
    let project = temp_project("update-reset");
    std::fs::write(
        project.join("src/Main.st"),
        r#"PROGRAM Main
VAR_INPUT
    StartCommand : BOOL;
END_VAR
VAR_OUTPUT
    Running : BOOL;
END_VAR
Running := StartCommand;
END_PROGRAM
"#,
    )
    .expect("write HMI source");

    let init = Command::new(env!("CARGO_BIN_EXE_trust-runtime"))
        .args(["hmi", "--project"])
        .arg(&project)
        .arg("init")
        .output()
        .expect("run HMI init");
    assert!(
        init.status.success(),
        "HMI init failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&init.stdout),
        String::from_utf8_lossy(&init.stderr)
    );

    std::fs::write(project.join("hmi/custom.txt"), "operator-owned")
        .expect("write operator-owned HMI file");
    std::fs::write(
        project.join("hmi/overview.toml"),
        "title = \"Overview\"\n[[section]]\ntitle = \"Custom\"\nspan = 12\n",
    )
    .expect("write customized overview");
    std::fs::remove_file(project.join("hmi/control.toml"))
        .expect("remove generated control page before update");
    assert!(
        !project.join("hmi/control.toml").exists(),
        "control page removal must establish a real update delta"
    );

    let update = Command::new(env!("CARGO_BIN_EXE_trust-runtime"))
        .args(["hmi", "--project"])
        .arg(&project)
        .arg("update")
        .output()
        .expect("run HMI update");
    assert!(
        update.status.success(),
        "HMI update failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&update.stdout),
        String::from_utf8_lossy(&update.stderr)
    );
    assert_eq!(
        std::fs::read_to_string(project.join("hmi/custom.txt"))
            .expect("read operator-owned HMI file after update"),
        "operator-owned"
    );
    assert!(
        std::fs::read_to_string(project.join("hmi/overview.toml"))
            .expect("read overview after update")
            .contains("title = \"Custom\""),
        "update must preserve the customized overview"
    );
    assert!(
        project.join("hmi/control.toml").is_file(),
        "update must restore a missing generated control page"
    );

    let reset = Command::new(env!("CARGO_BIN_EXE_trust-runtime"))
        .args(["hmi", "--project"])
        .arg(&project)
        .arg("reset")
        .output()
        .expect("run HMI reset");
    assert!(
        reset.status.success(),
        "HMI reset failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&reset.stdout),
        String::from_utf8_lossy(&reset.stderr)
    );
    let backups = std::fs::read_dir(&project)
        .expect("read project directory")
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .is_some_and(|name| name.to_string_lossy().starts_with("hmi.backup."))
        })
        .collect::<Vec<_>>();
    assert_eq!(backups.len(), 1, "reset must create one backup snapshot");
    assert_eq!(
        std::fs::read_to_string(backups[0].join("custom.txt"))
            .expect("read backed-up operator-owned file"),
        "operator-owned"
    );
    assert!(
        std::fs::read_to_string(backups[0].join("overview.toml"))
            .expect("read backed-up overview")
            .contains("title = \"Custom\""),
        "reset backup must preserve the pre-reset customized overview"
    );
    assert!(
        !std::fs::read_to_string(project.join("hmi/overview.toml"))
            .expect("read regenerated overview")
            .contains("title = \"Custom\""),
        "reset must regenerate scaffold-owned overview content"
    );
    let _ = std::fs::remove_dir_all(project);
}

#[test]
fn hmi_init_applies_selected_style_and_prints_success_summary() {
    let project = temp_project("selected-style-success");
    std::fs::write(
        project.join("src/Main.st"),
        r#"PROGRAM Main
VAR
    Running : BOOL;
END_VAR
Running := TRUE;
END_PROGRAM
"#,
    )
    .expect("write HMI source");

    let output = Command::new(env!("CARGO_BIN_EXE_trust-runtime"))
        .args(["hmi", "--project"])
        .arg(&project)
        .args(["init", "--style", "classic"])
        .output()
        .expect("run HMI init with selected style");

    assert!(
        output.status.success(),
        "HMI init failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        std::fs::read_to_string(project.join("hmi/_config.toml"))
            .expect("read generated HMI config")
            .contains("style = \"classic\""),
        "selected style must reach the generated descriptor"
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains(&format!(
            "Generated HMI scaffold in {} (init)",
            project.join("hmi").display()
        )),
        "success output must identify the selected HMI path and mode\nstdout:\n{stdout}"
    );
    assert!(
        stdout.contains("Generated hmi/ with") && stdout.contains("mode  - init"),
        "success output must include the deterministic scaffold summary\nstdout:\n{stdout}"
    );
    let _ = std::fs::remove_dir_all(project);
}
