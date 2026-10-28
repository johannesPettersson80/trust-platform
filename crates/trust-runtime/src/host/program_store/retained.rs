//! The retained-value check of a program install on a computer (specification 11 §6.7.1, decision
//! G13; specification 34 §11.2).
//!
//! The check compares the retain file the project names with the new program image's retain map, as
//! its warm start restores them (specification 44 §10.2, step 7), without writing anything. The
//! retain file is only read. The configuration lives in a scratch folder, which is removed
//! afterwards, so a refused check leaves the project as it was.

use std::fs;
use std::path::Path;

use crate::config::RuntimeConfig;
use crate::error::RuntimeError;
use crate::retain::{FileRetainStore, RetainStore};
use crate::watchdog::RetainMode;

use super::{scratch_path, stage_configuration, Payload};

/// Checks that the retained values of `project` can be restored into `program`, as its warm start
/// restores them (specification 11 §6.7.1). A project with `retain mode = "none"` has no retained
/// values to restore, so the check passes for it.
///
/// # Errors
///
/// The warm start's own error, which names the retained value and its reason, or the error of a
/// configuration or program that cannot be read or built.
pub fn check_retained(project: &Path, program: &[u8]) -> Result<(), RuntimeError> {
    let scratch = scratch_path(
        &std::env::temp_dir().join("trust-program-store"),
        "retain-check",
    );
    let result = check_in_scratch(&scratch, project, program);
    let _ = fs::remove_dir_all(&scratch);
    result
}

fn check_in_scratch(scratch: &Path, project: &Path, program: &[u8]) -> Result<(), RuntimeError> {
    stage_configuration(scratch, project, &Payload::default())?;
    // The image is checked against the retain file through its retain map (specification 44 §10.2,
    // step 7), without building the program.
    let config = RuntimeConfig::load(scratch.join("runtime.toml"))?;
    if config.retain_mode == RetainMode::None {
        return Ok(());
    }
    let Some(path) = config.retain_path else {
        return Ok(());
    };
    let path = if path.is_relative() {
        project.join(path)
    } else {
        path
    };
    let image = trust_runtime_core::native::image::Image::parse(program)
        .map_err(|error| RuntimeError::InvalidBundle(error.to_string().into()))?;
    let snapshot = FileRetainStore::new(path).load()?;
    trust_runtime_core::native::retain::check(&image, &snapshot.inner).map(drop)
}
