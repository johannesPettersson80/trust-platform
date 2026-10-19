//! The retained-value check of a program install on a computer (specification 11 §6.7.1, decision
//! G13; specification 34 §11.2).
//!
//! The check runs the new program's warm start without writing anything. A runtime is built from the
//! program with the project's configuration, and the retain file the project names is loaded into it,
//! as a warm start loads it. The retain file is only read. The candidate and its configuration live in
//! a scratch folder, which is removed afterwards, so a refused check leaves the project as it was.

use std::fs;
use std::path::Path;

use crate::bytecode::BytecodeModule;
use crate::config::RuntimeConfig;
use crate::deployment::{admitted_limits, write_record};
use crate::error::RuntimeError;
use crate::harness::CompileSession;
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
    write_record(scratch, program).map_err(|error| {
        RuntimeError::InvalidBundle(format!("write the deployment record: {error}").into())
    })?;
    let limits = admitted_limits(scratch, program)?;
    let config = RuntimeConfig::load(scratch.join("runtime.toml"))?;
    if config.retain_mode == RetainMode::None {
        return Ok(());
    }
    let module = BytecodeModule::decode(program).map_err(RuntimeError::from)?;
    let mut candidate = CompileSession::from_sources(Vec::new())
        .build_runtime_from_bytecode_with_limits(
            &module,
            Some(config.resource_name.as_str()),
            limits,
        )
        .map_err(|error| RuntimeError::ControlError(error.to_string().into()))?;
    let store = config.retain_path.map(|path| {
        let path = if path.is_relative() {
            project.join(path)
        } else {
            path
        };
        Box::new(FileRetainStore::new(path)) as Box<dyn RetainStore>
    });
    candidate.set_retain_store(store, Some(config.retain_save_interval));
    candidate.load_retain_store()
}
