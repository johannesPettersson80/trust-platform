//! Git commit helper for PLC projects.

use std::ffi::OsStr;
use std::io::IsTerminal;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::git::{git_available, git_repo_root};
use crate::prompt::{prompt_string, prompt_yes_no};

pub fn run_commit(
    project: Option<PathBuf>,
    message: Option<String>,
    dry_run: bool,
) -> anyhow::Result<()> {
    if !git_available() {
        anyhow::bail!("git not found; install git to use `trust-dev commit`");
    }

    let project_root = project.unwrap_or(std::env::current_dir()?);
    let repo_root = git_repo_root(&project_root).ok_or_else(|| {
        anyhow::anyhow!(
            "no git repository found in {} (run `git init` or `trust-runtime wizard`)",
            project_root.display()
        )
    })?;

    let project_rel = project_rel_path(&project_root, &repo_root)?;
    let collisions = pre_staged_collisions(&repo_root, &project_rel)?;
    if !collisions.is_empty() {
        anyhow::bail!(
            "pre-staged path intersects the selected commit scope: {}",
            collisions.join(", ")
        );
    }
    let status = git_status(&repo_root, &project_rel)?;
    if status.is_empty() {
        println!("No changes to commit.");
        return Ok(());
    }

    let summary = CommitSummary::from_status(&status);
    summary.print();

    if dry_run {
        return Ok(());
    }

    let commit_message = if let Some(message) = message {
        message
    } else {
        ensure_interactive()?;
        let default = summary.default_message();
        prompt_string("Commit message", &default)?
    };

    if commit_message.trim().is_empty() {
        anyhow::bail!("commit message cannot be empty");
    }

    let confirm = if std::io::stdin().is_terminal() {
        prompt_yes_no("Stage and commit these changes?", true)?
    } else {
        true
    };

    if confirm {
        let project_pathspec = project_pathspec(&project_rel);
        git_output_os(
            &repo_root,
            &[OsStr::new("add"), OsStr::new("--"), project_pathspec],
        )?;
        git_output_os(
            &repo_root,
            &[
                OsStr::new("commit"),
                OsStr::new("-m"),
                OsStr::new(commit_message.trim()),
                OsStr::new("--"),
                project_pathspec,
            ],
        )?;
        println!("Commit created.");
    } else {
        println!("Commit cancelled.");
    }

    Ok(())
}

fn ensure_interactive() -> anyhow::Result<()> {
    if !std::io::stdin().is_terminal() {
        anyhow::bail!("no TTY available; pass --message to commit non-interactively");
    }
    Ok(())
}

#[derive(Debug, Clone)]
struct StatusEntry {
    path: String,
}

fn git_status(repo_root: &Path, project_rel: &Path) -> anyhow::Result<Vec<StatusEntry>> {
    let output = git_output_os(
        repo_root,
        &[
            OsStr::new("status"),
            OsStr::new("--porcelain=v1"),
            OsStr::new("-z"),
            OsStr::new("--untracked-files=all"),
            OsStr::new("--"),
            project_pathspec(project_rel),
        ],
    )?;
    Ok(parse_porcelain_v1_z(&output))
}

fn pre_staged_collisions(repo_root: &Path, project_rel: &Path) -> anyhow::Result<Vec<String>> {
    let output = git_output_os(
        repo_root,
        &[
            OsStr::new("diff"),
            OsStr::new("--cached"),
            OsStr::new("--name-only"),
            OsStr::new("--no-renames"),
            OsStr::new("-z"),
        ],
    )?;
    let mut collisions = output
        .split(|byte| *byte == 0)
        .filter(|path| !path.is_empty())
        .filter_map(|path| {
            let valid_utf8 = std::str::from_utf8(path).ok();
            let display = display_git_path(path);
            if project_rel.as_os_str().is_empty()
                || valid_utf8.is_none()
                || Path::new(valid_utf8.expect("checked above")).starts_with(project_rel)
            {
                Some(display)
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    collisions.sort();
    collisions.dedup();
    Ok(collisions)
}

fn git_output_os(repo_root: &Path, args: &[&OsStr]) -> anyhow::Result<Vec<u8>> {
    let output = Command::new("git")
        .arg("-C")
        .arg(repo_root)
        .args(args)
        .output()?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("git {} failed: {}", format_git_args(args), stderr.trim());
    }
    Ok(output.stdout)
}

fn format_git_args(args: &[&OsStr]) -> String {
    args.iter()
        .map(|arg| arg.to_string_lossy())
        .collect::<Vec<_>>()
        .join(" ")
}

fn project_pathspec(project_rel: &Path) -> &OsStr {
    if project_rel.as_os_str().is_empty() {
        OsStr::new(".")
    } else {
        project_rel.as_os_str()
    }
}

fn parse_porcelain_v1_z(output: &[u8]) -> Vec<StatusEntry> {
    let mut entries = Vec::new();
    let mut records = output
        .split(|byte| *byte == 0)
        .filter(|record| !record.is_empty());

    while let Some(record) = records.next() {
        if record.len() < 4 {
            continue;
        }
        let status_x = record[0];
        let status_y = record[1];
        entries.push(StatusEntry {
            path: display_git_path(&record[3..]),
        });

        if matches!(status_x, b'R' | b'C') || matches!(status_y, b'R' | b'C') {
            let _ = records.next();
        }
    }

    entries
}

fn display_git_path(path: &[u8]) -> String {
    String::from_utf8_lossy(path).into_owned()
}

fn project_rel_path(project: &Path, repo_root: &Path) -> anyhow::Result<PathBuf> {
    let project = project
        .canonicalize()
        .unwrap_or_else(|_| project.to_path_buf());
    let repo_root = repo_root
        .canonicalize()
        .unwrap_or_else(|_| repo_root.to_path_buf());
    let rel = project
        .strip_prefix(&repo_root)
        .map(|path| path.to_path_buf())
        .unwrap_or_else(|_| project.clone());
    Ok(rel)
}

#[derive(Debug, Clone)]
struct CommitSummary {
    total: usize,
    st_files: Vec<String>,
    config_files: Vec<String>,
    other_files: Vec<String>,
}

impl CommitSummary {
    fn from_status(entries: &[StatusEntry]) -> Self {
        let mut st_files = Vec::new();
        let mut config_files = Vec::new();
        let mut other_files = Vec::new();
        for entry in entries {
            let path = entry.path.clone();
            if is_st_file(&path) {
                st_files.push(path);
            } else if is_config_file(&path) {
                config_files.push(path);
            } else {
                other_files.push(path);
            }
        }
        Self {
            total: entries.len(),
            st_files,
            config_files,
            other_files,
        }
    }

    fn default_message(&self) -> String {
        match (
            self.st_files.is_empty(),
            self.config_files.is_empty(),
            self.other_files.is_empty(),
        ) {
            (false, true, true) => format!("Update PLC program ({} files)", self.st_files.len()),
            (true, false, true) => "Update PLC configuration".to_string(),
            (false, false, _) => "Update PLC program + configuration".to_string(),
            _ => "Update PLC project".to_string(),
        }
    }

    fn print(&self) {
        println!("Changes detected: {} file(s)", self.total);
        if !self.config_files.is_empty() {
            println!("Config: {}", summarize_list(&self.config_files, 4));
        }
        if !self.st_files.is_empty() {
            println!("Sources: {}", summarize_list(&self.st_files, 6));
        }
        if !self.other_files.is_empty() {
            println!("Other: {}", summarize_list(&self.other_files, 4));
        }
    }
}

fn summarize_list(items: &[String], limit: usize) -> String {
    if items.len() <= limit {
        return items.join(", ");
    }
    let mut out = items[..limit].join(", ");
    out.push_str(&format!(", +{} more", items.len() - limit));
    out
}

fn is_st_file(path: &str) -> bool {
    let path = path.to_ascii_lowercase();
    path.ends_with(".st") || path.ends_with(".pou")
}

fn is_config_file(path: &str) -> bool {
    matches!(
        path,
        "runtime.toml" | "io.toml" | "program.stbc" | ".gitignore"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    use std::process::Command;
    use std::time::{SystemTime, UNIX_EPOCH};
    use verification_cases::{
        run_case_file, CaseExecution, CaseRecord, CaseResult, RunConfig, StateProbe, StateSnapshot,
    };

    const TRACE_TEST_ID: &str = "TEST_DEV_COMMIT_SCOPE_TRACE_001";
    const TRACE_CASE_FILE: &str = "verification/cases/plcopen_devtools/DEV_COMMIT_SCOPE_001.toml";
    const TRACE_CASE_DIGEST: &str =
        "sha256:abc30f653558fd4c40ff0f4e325482641a1c6eafb25d584b1b71ca56f433d2ea";

    fn unique_temp_dir(prefix: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time before unix epoch")
            .as_nanos();
        std::env::temp_dir().join(format!("trust-dev-{prefix}-{}-{nanos}", std::process::id()))
    }

    fn run_git(repo: &Path, args: &[&str]) -> String {
        let output = Command::new("git")
            .arg("-C")
            .arg(repo)
            .args(args)
            .output()
            .expect("run git");
        assert!(
            output.status.success(),
            "git {} failed\nstdout:\n{}\nstderr:\n{}",
            args.join(" "),
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8_lossy(&output.stdout).into_owned()
    }

    fn init_repo(repo: &Path) {
        std::fs::create_dir_all(repo).expect("create repo");
        run_git(repo, &["init"]);
        run_git(repo, &["config", "user.email", "dev@example.invalid"]);
        run_git(repo, &["config", "user.name", "trust-dev test"]);
        run_git(repo, &["config", "core.quotePath", "true"]);
    }

    fn seed_repo(repo: &Path) {
        std::fs::write(repo.join(".gitignore"), "target/\n").expect("write seed file");
        run_git(repo, &["add", ".gitignore"]);
        run_git(repo, &["commit", "-m", "Seed repository"]);
    }

    #[test]
    fn developer_commit_scope_trace_cases() {
        if !git_available() {
            return;
        }
        let workspace = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(Path::parent)
            .expect("trust-dev must be inside workspace/crates")
            .to_path_buf();
        let mut probe = CommitTraceProbe::default();
        let config = RunConfig::new(
            TRACE_TEST_ID,
            workspace.join(TRACE_CASE_FILE),
            TRACE_CASE_DIGEST,
        );
        let artifact = run_case_file(&config, &mut probe, run_commit_trace_case)
            .expect("commit-scope artifact must be written");
        let failures = artifact
            .cases
            .iter()
            .filter(|case| case.result != CaseResult::Passed)
            .map(|case| {
                format!(
                    "{}: {}",
                    case.id,
                    case.observed_error.as_deref().unwrap_or("not passed")
                )
            })
            .collect::<Vec<_>>();
        assert!(
            failures.is_empty(),
            "commit trace failures: {}",
            failures.join("; ")
        );
    }

    fn run_commit_trace_case(
        case: &CaseRecord,
        probe: &mut CommitTraceProbe,
    ) -> Result<CaseExecution, String> {
        let scenario = case
            .input
            .get("scenario")
            .and_then(toml::Value::as_str)
            .ok_or_else(|| format!("{} scenario must be a string", case.id))?;
        let failure = match scenario {
            "PRESTAGED_SCOPE_COLLISION" => validate_scoped_collisions(),
            "PRESTAGED_OUTSIDE_SCOPE" => validate_outside_scope_staging(),
            "DRY_RUN_OR_CANCEL" => validate_dry_run(),
            other => return Err(format!("unreviewed commit scenario {other}")),
        };
        probe.target = Some(serde_json::json!({
            "scenario": scenario,
            "passed": failure.is_none(),
        }));
        Ok(CaseExecution {
            result: if failure.is_none() {
                CaseResult::Passed
            } else {
                CaseResult::Failed
            },
            observed_error: failure,
            observed_status: Some("project_scope_checked".to_string()),
        })
    }

    fn validate_scoped_collisions() -> Option<String> {
        for root_scope in [false, true] {
            let repo = unique_temp_dir(if root_scope {
                "trace-root-collision"
            } else {
                "trace-project-collision"
            });
            init_repo(&repo);
            seed_repo(&repo);
            let project = if root_scope {
                repo.clone()
            } else {
                let project = repo.join("project");
                std::fs::create_dir_all(&project).expect("create project");
                project
            };
            let relative = if root_scope {
                "Main.st"
            } else {
                "project/Motor Ω.st"
            };
            std::fs::write(repo.join(relative), "PROGRAM Main\nEND_PROGRAM\n")
                .expect("write collision fixture");
            run_git(&repo, &["add", relative]);
            let before = run_git(&repo, &["status", "--porcelain=v1"]);
            let result = run_commit(
                Some(project),
                Some("must reject collision".to_string()),
                false,
            );
            let after = run_git(&repo, &["status", "--porcelain=v1"]);
            let commit_count = run_git(&repo, &["rev-list", "--count", "HEAD"]);
            let _ = std::fs::remove_dir_all(&repo);
            let message = result.as_ref().err().map(|error| format!("{error:#}"));
            if result.is_ok()
                || !message
                    .as_deref()
                    .is_some_and(|text| text.contains(relative))
                || before != after
                || commit_count.trim() != "1"
            {
                return Some(format!(
                    "scope collision root={root_scope} result={result:?} before={before:?} after={after:?} commits={commit_count:?}"
                ));
            }
        }

        let repo = unique_temp_dir("trace-rename-collision");
        init_repo(&repo);
        seed_repo(&repo);
        let project = repo.join("project");
        std::fs::create_dir_all(&project).expect("create project");
        std::fs::write(project.join("Old.st"), "PROGRAM Old\nEND_PROGRAM\n")
            .expect("write old source");
        run_git(&repo, &["add", "project/Old.st"]);
        run_git(&repo, &["commit", "-m", "Add old source"]);
        std::fs::rename(project.join("Old.st"), project.join("New.st")).expect("rename source");
        run_git(&repo, &["add", "-A", "project"]);
        let result = run_commit(
            Some(project),
            Some("must reject staged rename".to_string()),
            false,
        );
        let _ = std::fs::remove_dir_all(&repo);
        result
            .is_ok()
            .then(|| "staged rename was not rejected".to_string())
    }

    fn validate_outside_scope_staging() -> Option<String> {
        let repo = unique_temp_dir("trace-outside-scope");
        init_repo(&repo);
        seed_repo(&repo);
        let project = repo.join("project");
        std::fs::create_dir_all(&project).expect("create project");
        std::fs::write(project.join("Main.st"), "PROGRAM Main\nEND_PROGRAM\n")
            .expect("write project source");
        std::fs::write(repo.join("outside.txt"), "outside\n").expect("write outside");
        run_git(&repo, &["add", "outside.txt"]);
        let result = run_commit(
            Some(project),
            Some("Commit project only".to_string()),
            false,
        );
        let committed = run_git(&repo, &["show", "--name-only", "--format=", "HEAD"]);
        let staged = run_git(&repo, &["status", "--porcelain", "--", "outside.txt"]);
        let _ = std::fs::remove_dir_all(&repo);
        if let Err(error) = result {
            return Some(format!("scoped commit failed: {error:#}"));
        }
        (!committed.contains("project/Main.st")
            || committed.contains("outside.txt")
            || !staged.starts_with("A  outside.txt"))
        .then(|| format!("committed={committed:?}, staged={staged:?}"))
    }

    fn validate_dry_run() -> Option<String> {
        let repo = unique_temp_dir("trace-dry-run");
        init_repo(&repo);
        seed_repo(&repo);
        let project = repo.join("project");
        std::fs::create_dir_all(&project).expect("create project");
        std::fs::write(project.join("Main.st"), "PROGRAM Main\nEND_PROGRAM\n")
            .expect("write source");
        let before = run_git(&repo, &["status", "--porcelain=v1"]);
        let result = run_commit(Some(project), None, true);
        let after = run_git(&repo, &["status", "--porcelain=v1"]);
        let commits = run_git(&repo, &["rev-list", "--count", "HEAD"]);
        let _ = std::fs::remove_dir_all(&repo);
        if let Err(error) = result {
            return Some(format!("dry-run failed: {error:#}"));
        }
        (before != after || commits.trim() != "1")
            .then(|| format!("before={before:?}, after={after:?}, commits={commits:?}"))
    }

    #[derive(Default)]
    struct CommitTraceProbe {
        target: Option<serde_json::Value>,
        after: bool,
    }

    impl StateProbe for CommitTraceProbe {
        type Error = String;

        fn snapshot(&mut self) -> Result<StateSnapshot, Self::Error> {
            if !self.after {
                self.target = None;
            }
            self.after = !self.after;
            Ok(StateSnapshot {
                process_image_hash: None,
                retain_hash: None,
                target: self.target.clone(),
                siblings: BTreeMap::new(),
                diagnostics: Vec::new(),
            })
        }
    }

    #[test]
    fn commit_scopes_commit_to_project_path_without_sweeping_pre_staged_files() {
        if !git_available() {
            return;
        }
        let repo = unique_temp_dir("commit-scope");
        init_repo(&repo);
        let project = repo.join("project");
        std::fs::create_dir_all(&project).expect("create project");
        std::fs::write(project.join("Main.st"), "PROGRAM Main\nEND_PROGRAM\n")
            .expect("write project file");
        std::fs::write(repo.join("unrelated.txt"), "outside project\n")
            .expect("write unrelated file");
        run_git(&repo, &["add", "unrelated.txt"]);

        run_commit(Some(project), Some("Update PLC project".to_string()), false)
            .expect("commit project");

        let committed = run_git(&repo, &["show", "--name-only", "--format=", "HEAD"]);
        assert!(
            committed.contains("project/Main.st"),
            "project file should be committed: {committed}"
        );
        assert!(
            !committed.contains("unrelated.txt"),
            "pre-staged out-of-project file was swept into commit: {committed}"
        );

        let staged = run_git(&repo, &["status", "--porcelain", "--", "unrelated.txt"]);
        assert!(
            staged.starts_with("A  unrelated.txt"),
            "unrelated file should remain staged after scoped commit: {staged:?}"
        );

        let _ = std::fs::remove_dir_all(repo);
    }

    #[test]
    fn commit_rejects_pre_staged_path_inside_project_without_mutation() {
        if !git_available() {
            return;
        }
        let repo = unique_temp_dir("commit-staged-collision");
        init_repo(&repo);
        seed_repo(&repo);
        let project = repo.join("project");
        std::fs::create_dir_all(&project).expect("create project");
        std::fs::write(project.join("Main.st"), "PROGRAM Main\nEND_PROGRAM\n")
            .expect("write source");
        run_git(&repo, &["add", "project/Main.st"]);

        let result = run_commit(
            Some(project),
            Some("Must not absorb staged source".to_string()),
            false,
        );

        assert!(result.is_err(), "in-scope staged collision must abort");
        let message = format!("{:#}", result.expect_err("collision error"));
        assert!(
            message.contains("project/Main.st") && message.contains("pre-staged"),
            "collision diagnostic must name the path: {message}"
        );
        assert!(
            run_git(&repo, &["rev-list", "--count", "HEAD"]).trim() == "1",
            "collision must not create a commit"
        );
        let staged = run_git(&repo, &["status", "--porcelain", "--", "project/Main.st"]);
        assert!(
            staged.starts_with("A  project/Main.st"),
            "index changed: {staged:?}"
        );

        let _ = std::fs::remove_dir_all(repo);
    }

    #[test]
    fn repository_root_commit_rejects_any_pre_staged_path() {
        if !git_available() {
            return;
        }
        let repo = unique_temp_dir("commit-root-staged-collision");
        init_repo(&repo);
        seed_repo(&repo);
        std::fs::write(repo.join("Main.st"), "PROGRAM Main\nEND_PROGRAM\n").expect("write source");
        run_git(&repo, &["add", "Main.st"]);

        let result = run_commit(
            Some(repo.clone()),
            Some("Must not absorb root index".to_string()),
            false,
        );

        assert!(result.is_err(), "root staged collision must abort");
        let message = format!("{:#}", result.expect_err("collision error"));
        assert!(
            message.contains("Main.st") && message.contains("pre-staged"),
            "root collision diagnostic must name the path: {message}"
        );
        assert!(
            run_git(&repo, &["rev-list", "--count", "HEAD"]).trim() == "1",
            "collision must not create a commit"
        );

        let _ = std::fs::remove_dir_all(repo);
    }

    #[test]
    fn dry_run_with_pre_staged_collision_reports_without_mutation() {
        if !git_available() {
            return;
        }
        let repo = unique_temp_dir("commit-dry-run-collision");
        init_repo(&repo);
        seed_repo(&repo);
        let project = repo.join("project");
        std::fs::create_dir_all(&project).expect("create project");
        std::fs::write(project.join("Motor Ω.st"), "PROGRAM Main\nEND_PROGRAM\n")
            .expect("write source");
        run_git(&repo, &["add", "project/Motor Ω.st"]);
        let before = run_git(&repo, &["status", "--porcelain=v1"]);

        let result = run_commit(Some(project), None, true);

        assert!(result.is_err(), "dry-run must still report the collision");
        let after = run_git(&repo, &["status", "--porcelain=v1"]);
        assert_eq!(after, before, "dry-run collision mutated repository state");

        let _ = std::fs::remove_dir_all(repo);
    }

    #[test]
    fn git_status_decodes_quoted_porcelain_paths_for_summary() {
        if !git_available() {
            return;
        }
        let repo = unique_temp_dir("quoted-status");
        init_repo(&repo);
        let project = repo.join("project");
        std::fs::create_dir_all(&project).expect("create project");
        std::fs::write(project.join("Motor Ω.st"), "PROGRAM Main\nEND_PROGRAM\n")
            .expect("write unicode file");
        run_git(&repo, &["add", "project/Motor Ω.st"]);

        let project_rel = project_rel_path(&project, &repo).expect("relative project path");
        let entries = git_status(&repo, &project_rel).expect("git status");

        assert!(
            entries
                .iter()
                .any(|entry| entry.path == "project/Motor Ω.st"),
            "status entries should contain decoded display path, got {entries:?}"
        );

        let _ = std::fs::remove_dir_all(repo);
    }

    #[cfg(unix)]
    #[test]
    fn git_status_accepts_non_utf8_project_path_without_panic() {
        use std::ffi::OsString;
        use std::os::unix::ffi::OsStringExt;

        if !git_available() {
            return;
        }
        let repo = unique_temp_dir("non-utf8-status");
        init_repo(&repo);
        let project = repo.join(OsString::from_vec(b"project-\xFF".to_vec()));
        if let Err(error) = std::fs::create_dir_all(&project) {
            // macOS/APFS rejects invalid UTF-8 path bytes before Git can exercise
            // pathspec handling. Linux filesystems accept this fixture.
            if cfg!(target_os = "macos") && error.raw_os_error() == Some(92) {
                let _ = std::fs::remove_dir_all(repo);
                return;
            }
            panic!("create non-utf8 project: {error}");
        }
        std::fs::write(project.join("Main.st"), "PROGRAM Main\nEND_PROGRAM\n")
            .expect("write source");

        let project_rel = project_rel_path(&project, &repo).expect("relative project path");
        let result = std::panic::catch_unwind(|| git_status(&repo, &project_rel));
        assert!(
            result.is_ok(),
            "git_status must not unwrap non-UTF-8 project paths"
        );
        let entries = result
            .expect("no panic")
            .expect("git status should accept non-UTF-8 pathspecs");
        assert_eq!(entries.len(), 1);

        let _ = std::fs::remove_dir_all(repo);
    }
}
