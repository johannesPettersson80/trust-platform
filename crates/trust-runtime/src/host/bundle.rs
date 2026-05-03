//! Bundle discovery helpers.

use std::path::PathBuf;

use crate::config::load_system_io_config;
use crate::error::RuntimeError;

/// Locate a runtime bundle using the standard search order.
pub fn detect_bundle_path(bundle: Option<PathBuf>) -> Result<PathBuf, RuntimeError> {
    if let Some(bundle) = bundle {
        return Ok(bundle);
    }
    let cwd = std::env::current_dir().map_err(|err| {
        RuntimeError::InvalidBundle(format!("failed to read current dir: {err}").into())
    })?;
    let runtime_toml = cwd.join("runtime.toml");
    let io_toml = cwd.join("io.toml");
    if runtime_toml.is_file() && (io_toml.is_file() || load_system_io_config()?.is_some()) {
        return Ok(cwd);
    }
    let bundle_dir = cwd.join("project");
    if bundle_dir.is_dir() {
        return Ok(bundle_dir);
    }
    Err(RuntimeError::InvalidBundle(
        "project folder not found (run `trust-runtime` to launch setup, or pass --project <dir>)"
            .into(),
    ))
}
