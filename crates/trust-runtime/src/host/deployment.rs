//! The deployment record next to the project's program image (specification 34 §5.1.1, HOST-02;
//! specification 44 §10.1).
//!
//! Every tool that writes a program into a project writes its record too. A computer admits the
//! project's image by the record beside it when it loads the project, as a device admits a
//! container (specification 44 §10.2, step 1; specification 34 HOST-02), and reads its
//! `runtime.toml` and `io.toml` at each start, which the record does not bind on a computer. The
//! record itself, its identities and its admission are the shared core's
//! (`runtime_core::deployment`).

use std::io;
use std::path::Path;

use trust_runtime_core::deployment::{configuration_identity, DeploymentRecord};

use crate::error::RuntimeError;

#[path = "deployment/configuration.rs"]
mod configuration;

#[path = "program_store.rs"]
pub mod program_store;

/// The record's file name, next to the project's program image.
pub const DEPLOYMENT_RECORD_FILE: &str = "deployment-record.bin";

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

/// Admits the program image `image` of `project` by the deployment record beside it, when the build
/// wrote one: the record must read, name this image and ask for nothing this computer does not run
/// (specification 44 §10.2, step 1; specification 34 HOST-02). A project without a record is
/// admitted by the image's own checks.
///
/// # Errors
///
/// [`RuntimeError::DeploymentRecordInvalid`] for a record that does not read,
/// [`RuntimeError::DeploymentRefused`] for one that names another program or asks for more, and
/// [`RuntimeError::InvalidBundle`] for one that cannot be read.
pub fn admit_project_image(project: &Path, image: &[u8]) -> Result<(), RuntimeError> {
    let bytes = match std::fs::read(project.join(DEPLOYMENT_RECORD_FILE)) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(error) => {
            return Err(RuntimeError::InvalidBundle(
                format!("failed to read {DEPLOYMENT_RECORD_FILE}: {error}").into(),
            ));
        }
    };
    let record = DeploymentRecord::decode(&bytes).map_err(RuntimeError::DeploymentRecordInvalid)?;
    trust_runtime_core::deployment::admit(
        &record,
        image,
        &trust_runtime_core::deployment::DeviceDescriptor::hosted(),
    )
}

/// The deployment record of the compiled program image `image` of `project` (specification 44
/// §10.1): the record the image's own tables give, with the configuration identity and the
/// configuration section of the project's `runtime.toml` and `io.toml`, encoded.
///
/// # Errors
///
/// The configuration files cannot be read or carry a value section 7 cannot hold, the image's
/// tables do not read, or the record cannot be encoded.
pub fn image_record(project: &Path, image: &[u8]) -> Result<Vec<u8>, RuntimeError> {
    let identity = project_configuration_identity(project).map_err(|error| {
        RuntimeError::InvalidBundle(format!("failed to read the configuration: {error}").into())
    })?;
    let mut record = DeploymentRecord::for_image(image, identity)?;
    record.configuration = configuration::from_project(project)?;
    record
        .encode()
        .map_err(RuntimeError::DeploymentRecordInvalid)
}

fn read_optional(path: &Path) -> io::Result<Option<Vec<u8>>> {
    match std::fs::read(path) {
        Ok(bytes) => Ok(Some(bytes)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error),
    }
}
