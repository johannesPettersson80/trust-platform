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
    use std::process::Command;
    use std::time::{SystemTime, UNIX_EPOCH};

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
        std::fs::create_dir_all(&project).expect("create non-utf8 project");
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
