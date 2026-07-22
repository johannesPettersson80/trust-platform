//! Git helper utilities for product CLI workflows.

use std::path::Path;
use std::process::Command;

fn git_available() -> bool {
    Command::new("git")
        .arg("--version")
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

pub(crate) fn git_init(root: &Path) -> anyhow::Result<()> {
    if has_valid_git_marker(root)? {
        return Ok(());
    }
    if !git_available() {
        anyhow::bail!("git not found");
    }
    let status = Command::new("git").arg("init").current_dir(root).status()?;
    if !status.success() {
        anyhow::bail!("git init failed");
    }
    Ok(())
}

fn has_valid_git_marker(root: &Path) -> anyhow::Result<bool> {
    let marker = root.join(".git");
    let metadata = match std::fs::symlink_metadata(&marker) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(error.into()),
    };
    if metadata.is_dir() {
        return Ok(true);
    }
    if metadata.file_type().is_symlink() {
        if marker.is_dir() {
            return Ok(true);
        }
        anyhow::bail!(
            "invalid .git marker at {}: dangling or non-directory symlink",
            marker.display()
        );
    }
    if !metadata.is_file() {
        anyhow::bail!(
            "invalid .git marker at {}: unsupported file type",
            marker.display()
        );
    }

    let text = std::fs::read_to_string(&marker)?;
    let mut lines = text.lines();
    let Some(target) = lines
        .next()
        .and_then(|line| line.strip_prefix("gitdir:"))
        .map(str::trim)
        .filter(|target| !target.is_empty())
    else {
        anyhow::bail!(
            "invalid .git marker at {}: expected 'gitdir: <path>'",
            marker.display()
        );
    };
    if lines.next().is_some() {
        anyhow::bail!(
            "invalid .git marker at {}: unexpected extra content",
            marker.display()
        );
    }
    let target = Path::new(target);
    let target = if target.is_absolute() {
        target.to_path_buf()
    } else {
        root.join(target)
    };
    if !target.is_dir() {
        anyhow::bail!(
            "invalid .git marker at {}: gitdir target is not a directory: {}",
            marker.display(),
            target.display()
        );
    }
    Ok(true)
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    use super::*;

    static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(1);

    fn temp_dir(name: &str) -> PathBuf {
        let sequence = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "trust-git-{name}-{}-{sequence}",
            std::process::id()
        ));
        std::fs::create_dir(&path).expect("create git helper test directory");
        path
    }

    #[test]
    fn git_init_rejects_an_invalid_git_file_marker() {
        let root = temp_dir("invalid-marker");
        std::fs::write(root.join(".git"), "not a gitdir marker\n")
            .expect("write invalid .git marker");

        let error = git_init(&root).expect_err("invalid .git file must not report success");

        assert!(
            error.to_string().contains("invalid .git marker"),
            "unexpected error: {error:#}"
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn git_init_accepts_a_valid_worktree_gitdir_marker() {
        let root = temp_dir("worktree-marker");
        let git_dir = root.join("worktree-metadata");
        std::fs::create_dir(&git_dir).expect("create worktree metadata directory");
        std::fs::write(root.join(".git"), "gitdir: worktree-metadata\n")
            .expect("write worktree .git marker");

        git_init(&root).expect("valid worktree marker is already initialized");

        let _ = std::fs::remove_dir_all(root);
    }
}
