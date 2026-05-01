//! Compatibility forwarding for workbench commands moving to `trust-dev`.

use std::ffi::OsString;
use std::path::PathBuf;
use std::process::Command;

use anyhow::Context;

pub(crate) fn run_trust_dev_with_warning(
    command_name: &str,
    args: Vec<OsString>,
) -> anyhow::Result<()> {
    eprintln!(
        "{}",
        crate::style::warning(format!(
            "Warning: `trust-runtime {command_name}` is deprecated. Use `trust-dev {command_name}` instead."
        ))
    );

    let trust_dev = resolve_trust_dev_binary();
    let status = Command::new(&trust_dev)
        .args(args)
        .status()
        .with_context(|| {
            format!(
                "run {}. Install `trust-dev` beside `trust-runtime` or put it on PATH.",
                trust_dev.display()
            )
        })?;
    if status.success() {
        return Ok(());
    }
    if let Some(code) = status.code() {
        std::process::exit(code);
    }
    anyhow::bail!("{} terminated by signal", trust_dev.display());
}

fn resolve_trust_dev_binary() -> PathBuf {
    if let Some(path) = std::env::var_os("TRUST_DEV_BIN") {
        return PathBuf::from(path);
    }
    let file_name = format!("trust-dev{}", std::env::consts::EXE_SUFFIX);
    if let Ok(current_exe) = std::env::current_exe() {
        if let Some(parent) = current_exe.parent() {
            let sibling = parent.join(&file_name);
            if sibling.exists() {
                return sibling;
            }
        }
    }
    PathBuf::from(file_name)
}
