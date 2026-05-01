//! Git commit helper for PLC projects.

use std::io::IsTerminal;
use std::path::{Path, PathBuf};

use crate::git::{git_available, git_output, git_repo_root};
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
        git_output(&repo_root, &["add", "--", project_rel.to_str().unwrap()])?;
        git_output(&repo_root, &["commit", "-m", commit_message.trim()])?;
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
    let output = git_output(
        repo_root,
        &["status", "--porcelain", "--", project_rel.to_str().unwrap()],
    )?;
    let mut entries = Vec::new();
    for line in output.lines() {
        if line.trim().is_empty() {
            continue;
        }
        let mut path = line.get(3..).unwrap_or("").trim().to_string();
        if let Some((_, new_path)) = path.split_once(" -> ") {
            path = new_path.trim().to_string();
        }
        entries.push(StatusEntry { path });
    }
    Ok(entries)
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
