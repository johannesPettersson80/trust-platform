//! The deployment record next to `program.stbc` (specification 34 §5.1.1, HOST-02).
//!
//! Every tool that writes a program into a project writes its record too, and a project's
//! record is admitted against the computer's descriptor when the project is loaded. The record
//! itself, its identities and its admission are the shared core's (`runtime_core::deployment`).

use std::io;
use std::path::{Path, PathBuf};

use trust_runtime_core::deployment::{
    admit, configuration_identity, DeploymentRecord, DeviceDescriptor,
};
use trust_runtime_core::vm::PreparationLimits;

use crate::error::RuntimeError;

#[path = "deployment/configuration.rs"]
mod configuration;

/// The record's file name, next to `program.stbc`.
pub const DEPLOYMENT_RECORD_FILE: &str = "deployment-record.bin";

/// What writing a program's record did.
#[derive(Debug, Clone)]
pub enum RecordWrite {
    /// The record was written to this path.
    Written(PathBuf),
    /// The runtime cannot prepare the program, or a value section 7 cannot carry was found, so
    /// it has no record; an older record was removed.
    Skipped(RuntimeError),
}

/// The configuration identity of a project: its `runtime.toml` and `io.toml`.
///
/// # Errors
///
/// A file exists but cannot be read.
pub fn project_configuration_identity(project: &Path) -> io::Result<[u8; 32]> {
    let runtime = read_optional(&project.join("runtime.toml"))?;
    let io = read_optional(&project.join("io.toml"))?;
    Ok(configuration_identity(runtime.as_deref(), io.as_deref()))
}

/// Write the record of `program`, the bytes of the project's `program.stbc`, requesting the
/// hosted default limits and filling the configuration section from the project's `runtime.toml`
/// and `io.toml` (specification 34 §5.1.1). When the runtime cannot prepare the program, or a
/// value the record cannot carry is found, an older record is removed instead, so that a project
/// never pairs a program with another program's record.
///
/// # Errors
///
/// The configuration files cannot be read, or the record cannot be written or removed.
pub fn write_record(project: &Path, program: &[u8]) -> io::Result<RecordWrite> {
    let identity = project_configuration_identity(project)?;
    let path = project.join(DEPLOYMENT_RECORD_FILE);
    let encoded = configuration::from_project(project)
        .and_then(|section| {
            DeploymentRecord::for_program(program, PreparationLimits::default(), identity).map(
                |mut record| {
                    record.configuration = section;
                    record
                },
            )
        })
        .and_then(|record| {
            record
                .encode()
                .map_err(RuntimeError::DeploymentRecordInvalid)
        });
    match encoded {
        Ok(bytes) => {
            std::fs::write(&path, bytes)?;
            Ok(RecordWrite::Written(path))
        }
        Err(reason) => {
            match std::fs::remove_file(&path) {
                Ok(()) => {}
                Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                Err(error) => return Err(error),
            }
            Ok(RecordWrite::Skipped(reason))
        }
    }
}

/// The limits to prepare a project's program with: those of its record, admitted against the
/// computer's descriptor, or the hosted defaults when the project has no record (HOST-02).
///
/// # Errors
///
/// The record cannot be read (`InvalidBundle`), is malformed (`DeploymentRecordInvalid`), or
/// the computer refuses it (`DeploymentRefused`, `ImportUnsupported`).
pub fn admitted_limits(project: &Path, program: &[u8]) -> Result<PreparationLimits, RuntimeError> {
    let bytes = match std::fs::read(project.join(DEPLOYMENT_RECORD_FILE)) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return Ok(PreparationLimits::default());
        }
        Err(error) => {
            return Err(RuntimeError::InvalidBundle(
                format!("failed to read {DEPLOYMENT_RECORD_FILE}: {error}").into(),
            ));
        }
    };
    let record = DeploymentRecord::decode(&bytes).map_err(RuntimeError::DeploymentRecordInvalid)?;
    admit(&record, program, &DeviceDescriptor::hosted())
}

fn read_optional(path: &Path) -> io::Result<Option<Vec<u8>>> {
    match std::fs::read(path) {
        Ok(bytes) => Ok(Some(bytes)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error),
    }
}
