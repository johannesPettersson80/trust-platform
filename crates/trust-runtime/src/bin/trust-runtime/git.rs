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
    if root.join(".git").exists() {
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
