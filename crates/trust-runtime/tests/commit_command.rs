use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static TEMP_DIR_COUNTER: AtomicU64 = AtomicU64::new(1);

fn unique_temp_dir(prefix: &str) -> PathBuf {
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

fn git_available() -> bool {
    Command::new("git")
        .arg("--version")
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

fn git(root: &Path, args: &[&str]) {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .output()
        .expect("run git");
    assert!(
        output.status.success(),
        "git {} failed.\nstdout:\n{}\nstderr:\n{}",
        args.join(" "),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn write_dirty_project(repo: &Path) -> PathBuf {
    git(repo, &["init"]);
    git(
        repo,
        &["config", "user.email", "trust-tests@example.invalid"],
    );
    git(repo, &["config", "user.name", "truST tests"]);
    let project = repo.join("plc-project");
    let src = project.join("src");
    std::fs::create_dir_all(&src).expect("create project src");
    let main_st = src.join("main.st");
    std::fs::write(
        &main_st,
        r#"
PROGRAM Main
VAR
    Counter : INT := 0;
END_VAR
Counter := Counter + 1;
END_PROGRAM
"#,
    )
    .expect("write project source");
    git(repo, &["add", "--", "plc-project"]);
    git(repo, &["commit", "-m", "Initial project"]);
    std::fs::write(
        &main_st,
        r#"
PROGRAM Main
VAR
    Counter : INT := 1;
END_VAR
Counter := Counter + 1;
END_PROGRAM
"#,
    )
    .expect("modify project source");
    project
}

fn trust_dev_bin() -> PathBuf {
    if let Some(path) = option_env!("CARGO_BIN_EXE_trust-dev") {
        return path.into();
    }
    if let Ok(path) = std::env::var("TRUST_DEV_BIN") {
        return path.into();
    }
    let exe = std::env::current_exe().expect("current test exe path");
    let debug_dir = exe
        .parent()
        .and_then(|deps| deps.parent())
        .expect("target debug dir");
    debug_dir.join(format!("trust-dev{}", std::env::consts::EXE_SUFFIX))
}

#[test]
fn trust_dev_commit_dry_run_reports_project_changes() {
    if !git_available() {
        eprintln!("skipping commit command test because git is not available");
        return;
    }

    let repo = unique_temp_dir("commit-dev");
    let project = write_dirty_project(&repo);
    let output = Command::new(trust_dev_bin())
        .args(["commit", "--project"])
        .arg(&project)
        .args(["--message", "Update PLC project", "--dry-run"])
        .output()
        .expect("run trust-dev commit");

    assert!(
        output.status.success(),
        "expected trust-dev commit dry-run success.\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Changes detected:"));
    assert!(stdout.contains("main.st"));

    let _ = std::fs::remove_dir_all(repo);
}

#[test]
fn trust_runtime_commit_alias_forwards_to_trust_dev_with_deprecation_warning() {
    if !git_available() {
        eprintln!("skipping commit command alias test because git is not available");
        return;
    }

    let repo = unique_temp_dir("commit-alias");
    let project = write_dirty_project(&repo);
    let output = Command::new(env!("CARGO_BIN_EXE_trust-runtime"))
        .env("TRUST_DEV_BIN", trust_dev_bin())
        .args(["commit", "--project"])
        .arg(&project)
        .args(["--message", "Update PLC project", "--dry-run"])
        .output()
        .expect("run trust-runtime commit alias");

    assert!(
        output.status.success(),
        "expected trust-runtime commit alias success.\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Changes detected:"));
    assert!(stdout.contains("main.st"));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("deprecated"));
    assert!(stderr.contains("trust-dev commit"));

    let _ = std::fs::remove_dir_all(repo);
}
