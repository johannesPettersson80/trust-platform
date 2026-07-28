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

    #[test]
    fn git_init_creates_repository_and_accepts_existing_git_directory() {
        let root = temp_dir("create-repository");

        git_init(&root).expect("git init should create repository metadata");
        assert!(root.join(".git").is_dir());
        git_init(&root).expect("existing .git directory should be idempotent");

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn git_init_rejects_invalid_worktree_marker_shapes() {
        for (name, marker) in [
            ("empty-target", "gitdir:   \n"),
            ("extra-content", "gitdir: metadata\nunexpected\n"),
            ("missing-target", "gitdir: missing-metadata\n"),
        ] {
            let root = temp_dir(name);
            std::fs::write(root.join(".git"), marker).expect("write invalid worktree marker");

            let error = git_init(&root).expect_err("invalid worktree marker must fail");
            assert!(
                error.to_string().contains("invalid .git marker"),
                "unexpected {name} error: {error:#}"
            );
            let _ = std::fs::remove_dir_all(root);
        }
    }

    #[cfg(unix)]
    #[test]
    fn git_init_accepts_directory_symlink_and_rejects_dangling_symlink() {
        use std::os::unix::fs::symlink;

        let valid_root = temp_dir("valid-symlink");
        let metadata = valid_root.join("git-metadata");
        std::fs::create_dir(&metadata).expect("create Git metadata directory");
        symlink(&metadata, valid_root.join(".git")).expect("create valid .git symlink");
        git_init(&valid_root).expect("directory .git symlink should be accepted");

        let dangling_root = temp_dir("dangling-symlink");
        symlink("missing-metadata", dangling_root.join(".git"))
            .expect("create dangling .git symlink");
        let error = git_init(&dangling_root).expect_err("dangling .git symlink must fail");
        assert!(error
            .to_string()
            .contains("dangling or non-directory symlink"));

        let _ = std::fs::remove_dir_all(valid_root);
        let _ = std::fs::remove_dir_all(dangling_root);
    }

    #[cfg(unix)]
    #[test]
    fn git_init_rejects_unsupported_git_filesystem_object() {
        use std::os::unix::net::UnixListener;

        let root = temp_dir("socket-marker");
        let sequence = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let short_socket = root
            .parent()
            .expect("test directory has a parent")
            .join(format!("tg-socket-{}-{sequence}", std::process::id()));
        let listener = UnixListener::bind(&short_socket).expect("create short Unix socket");
        std::fs::rename(&short_socket, root.join(".git")).expect("move socket to .git marker");

        let error = git_init(&root).expect_err("socket .git marker must fail");
        assert!(error.to_string().contains("unsupported file type"));

        drop(listener);
        let _ = std::fs::remove_dir_all(root);
    }
}
