use std::path::PathBuf;
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
