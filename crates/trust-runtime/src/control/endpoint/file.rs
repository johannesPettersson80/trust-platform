//! The endpoint file `<project>/.trust-runtime/control.json` (spec 11, section 6.8.0a).
//!
//! A token that the runtime generated is kept beside the endpoint: in the owner-only endpoint file
//! on Unix-like systems, and in a per-user token file outside the project folder on Windows (see
//! the `token_file` module).

use std::fmt;
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::control::ControlEndpoint;
use crate::error::RuntimeError;

// The endpoint file's place and its reader are the clients' (spec 11, section 6.8.0a): every client
// reads the file the runtime writes here through the same code.
pub(super) use trust_runtime_client::project_files::{
    endpoint_file_path, read_endpoint_file, EndpointFileError, ENDPOINT_FILE_SCHEMA_VERSION,
};
pub use trust_runtime_client::project_files::{EndpointFile, ENDPOINT_DIR, ENDPOINT_FILE};

use super::defaults::runtime_files_folder;
use super::token_file::{
    read_token_file, remove_token_file_of, token_file_path, write_token_file, TokenFile,
};

/// Where the token that a runtime generated is kept: in the endpoint file (`None`), or in the
/// per-user token file in this folder (Windows).
///
/// # Errors
///
/// Returns an error on Windows when the per-user folder is unknown.
pub(super) fn token_directory() -> Result<Option<PathBuf>, RuntimeError> {
    #[cfg(windows)]
    {
        super::defaults::per_user_token_directory().map(Some)
    }
    #[cfg(not(windows))]
    {
        Ok(None)
    }
}

/// The endpoint file that a running runtime published, and its token file on Windows. Dropping it
/// removes them, but only while they still name this process: a clean stop leaves nothing behind,
/// and files that a newer runtime has written are never removed.
pub struct PublishedEndpoint {
    project_root: PathBuf,
    pid: u32,
    token_file: Option<PathBuf>,
}

impl fmt::Debug for PublishedEndpoint {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PublishedEndpoint")
            .field("project_root", &self.project_root)
            .field("pid", &self.pid)
            .field("token_file", &self.token_file)
            .finish()
    }
}

impl PublishedEndpoint {
    /// Writes the endpoint file for `endpoint`, the endpoint that the runtime has bound.
    /// `generated_token` is the token the runtime generated for this start, if it generated one.
    /// On Windows the token goes into the per-user token file and the endpoint file carries none.
    ///
    /// # Errors
    ///
    /// Returns an error when the private directory, the token file or the endpoint file cannot be
    /// written. Nothing is left behind then.
    pub fn publish(
        project_root: &Path,
        endpoint: &ControlEndpoint,
        generated_token: Option<&str>,
    ) -> Result<Self, RuntimeError> {
        let token_dir = match generated_token {
            Some(_) => token_directory()?,
            None => None,
        };
        Self::publish_with(
            project_root,
            endpoint,
            generated_token,
            token_dir.as_deref(),
        )
    }

    /// [`Self::publish`] with the token home given: `Some(dir)` keeps a generated token in the
    /// per-user token file in `dir`, `None` keeps it in the endpoint file.
    pub(super) fn publish_with(
        project_root: &Path,
        endpoint: &ControlEndpoint,
        generated_token: Option<&str>,
        token_dir: Option<&Path>,
    ) -> Result<Self, RuntimeError> {
        let pid = std::process::id();
        let endpoint_text = endpoint.to_string();
        let mut token_file = None;
        let mut file_token = generated_token.map(str::to_string);
        if let (Some(dir), Some(token)) = (token_dir, generated_token) {
            let path = token_file_path(dir, project_root)?;
            write_token_file(
                &path,
                &TokenFile::new(endpoint_text.clone(), pid, token.to_string()),
            )?;
            token_file = Some(path);
            file_token = None;
        }
        let file = EndpointFile {
            schema_version: ENDPOINT_FILE_SCHEMA_VERSION,
            endpoint: endpoint_text,
            pid,
            auth_token: file_token,
        };
        if let Err(error) = write_endpoint_file(project_root, &file) {
            if let Some(path) = &token_file {
                remove_token_file_of(path, pid);
            }
            return Err(error);
        }
        Ok(Self {
            project_root: project_root.to_path_buf(),
            pid,
            token_file,
        })
    }
}

impl Drop for PublishedEndpoint {
    fn drop(&mut self) {
        remove_endpoint_file_of(&self.project_root, self.pid);
        if let Some(path) = &self.token_file {
            remove_token_file_of(path, self.pid);
        }
    }
}

/// Removes the project's endpoint file, and its token file on Windows, when they name process
/// `pid`. A process that stops another runtime without a clean stop, such as the config UI that
/// kills its child, calls this so that no endpoint file outlives its runtime. A file that cannot be
/// read, or that names another process, is kept. Returns whether the endpoint file was removed.
pub fn remove_endpoint_file_of(project_root: &Path, pid: u32) -> bool {
    let removed = match read_endpoint_file(project_root) {
        Ok(Some(file)) if file.pid == pid => {
            fs::remove_file(endpoint_file_path(project_root)).is_ok()
        }
        _ => false,
    };
    if let Ok(Some(dir)) = token_directory()
        && let Ok(path) = token_file_path(&dir, project_root)
    {
        remove_token_file_of(&path, pid);
    }
    removed
}

/// The token that a runtime generated and that `config.set` withdrew, kept so that a request
/// refused afterwards can put it back (spec 11, section 6.8.0a).
pub struct WithdrawnToken {
    place: WithdrawnPlace,
}

enum WithdrawnPlace {
    /// The endpoint file held the token; this is the file as it was.
    EndpointFile {
        project_root: PathBuf,
        file: EndpointFile,
    },
    /// The per-user token file held the token; this is the token file as it was.
    TokenFile { path: PathBuf, file: TokenFile },
}

impl fmt::Debug for WithdrawnToken {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let place = match &self.place {
            WithdrawnPlace::EndpointFile { project_root, .. } => {
                format!("endpoint file of {}", project_root.display())
            }
            WithdrawnPlace::TokenFile { path, .. } => format!("token file {}", path.display()),
        };
        formatter
            .debug_struct("WithdrawnToken")
            .field("place", &place)
            .finish()
    }
}

impl WithdrawnToken {
    /// Writes the withdrawn token back where it was.
    ///
    /// # Errors
    ///
    /// Returns an error when the file cannot be written.
    pub fn restore(self) -> Result<(), RuntimeError> {
        match self.place {
            WithdrawnPlace::EndpointFile { project_root, file } => {
                write_endpoint_file(&project_root, &file)
            }
            WithdrawnPlace::TokenFile { path, file } => write_token_file(&path, &file),
        }
    }
}

/// Withdraws the token that this process generated, after `config.set` replaced or cleared
/// `control.auth_token` (spec 11, section 6.8.0a): the endpoint file is rewritten without
/// `auth_token`, or the per-user token file is removed. Only files that name this process are
/// touched. A file that is missing, cannot be read or parsed, carries no token or names another
/// process belongs to no runtime of this process, so it is left alone and is no error.
///
/// Returns what was withdrawn, so that it can be restored, or `None` when nothing was.
///
/// # Errors
///
/// Returns an error when a file of this process cannot be rewritten or removed.
pub fn withdraw_auth_token(project_root: &Path) -> Result<Option<WithdrawnToken>, RuntimeError> {
    let token_dir = token_directory().ok().flatten();
    withdraw_auth_token_with(project_root, token_dir.as_deref())
}

/// [`withdraw_auth_token`] with the token home given, as for [`PublishedEndpoint::publish_with`].
pub(super) fn withdraw_auth_token_with(
    project_root: &Path,
    token_dir: Option<&Path>,
) -> Result<Option<WithdrawnToken>, RuntimeError> {
    let pid = std::process::id();
    let Ok(Some(file)) = read_endpoint_file(project_root) else {
        return Ok(None);
    };
    if file.pid != pid {
        return Ok(None);
    }
    if file.auth_token.is_some() {
        let mut without = file.clone();
        without.auth_token = None;
        write_endpoint_file(project_root, &without)?;
        return Ok(Some(WithdrawnToken {
            place: WithdrawnPlace::EndpointFile {
                project_root: project_root.to_path_buf(),
                file,
            },
        }));
    }
    let Some(dir) = token_dir else {
        return Ok(None);
    };
    let path = token_file_path(dir, project_root)?;
    let Some(token) = read_token_file(&path).filter(|token| token.pid == pid) else {
        return Ok(None);
    };
    fs::remove_file(&path)
        .map_err(|error| invalid(format!("remove {}: {error}", path.display())))?;
    Ok(Some(WithdrawnToken {
        place: WithdrawnPlace::TokenFile { path, file: token },
    }))
}

/// The token that the endpoint file offers a client: the file's own `auth_token`, or on Windows
/// the per-user token file's token when that file names the same process and endpoint.
pub(super) fn published_token(
    file: &EndpointFile,
    project_root: &Path,
    token_dir: Option<&Path>,
) -> Option<String> {
    let Some(dir) = token_dir else {
        return file.auth_token.clone();
    };
    let path = token_file_path(dir, project_root).ok()?;
    read_token_file(&path)
        .filter(|token| token.pid == file.pid && token.endpoint == file.endpoint)
        .map(TokenFile::into_token)
}

/// Creates `dir` when it is missing and restricts it to its owner (mode 0700 on Unix-like
/// systems).
pub(super) fn ensure_private_directory(dir: &Path) -> Result<(), RuntimeError> {
    fs::create_dir_all(dir)
        .map_err(|error| invalid(format!("create {}: {error}", dir.display())))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(dir, fs::Permissions::from_mode(0o700))
            .map_err(|error| invalid(format!("restrict {}: {error}", dir.display())))?;
    }
    Ok(())
}

/// Writes the endpoint file: a temporary file in the same directory is written completely and
/// then renamed over the old file, so a reader never sees half of it. On Unix-like systems the
/// temporary file is created with mode 0600, so it is never readable by other users.
pub(super) fn write_endpoint_file(
    project_root: &Path,
    file: &EndpointFile,
) -> Result<(), RuntimeError> {
    let dir = runtime_files_folder(project_root).join(ENDPOINT_DIR);
    ensure_private_directory(&dir)?;
    let text = serde_json::to_string(file)
        .map_err(|error| invalid(format!("encode endpoint file: {error}")))?;
    write_atomically(&dir.join(ENDPOINT_FILE), text.as_bytes())
}

/// Writes `bytes` to `target` through a temporary file in the same directory that is renamed over
/// `target`. The temporary file is owner-only on Unix-like systems and is removed on failure.
pub fn write_atomically(target: &Path, bytes: &[u8]) -> Result<(), RuntimeError> {
    let dir = target.parent().unwrap_or_else(|| Path::new("."));
    let name = target
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_nanos())
        .unwrap_or_default();
    let temp = dir.join(format!(".{name}.tmp-{}-{nanos}", std::process::id()));
    if let Err(error) = write_temp_then_rename(&temp, target, bytes) {
        let _ = fs::remove_file(&temp);
        return Err(invalid(format!("write {}: {error}", target.display())));
    }
    Ok(())
}

fn write_temp_then_rename(temp: &Path, target: &Path, bytes: &[u8]) -> io::Result<()> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(temp)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    drop(file);
    fs::rename(temp, target)
}

pub(super) fn invalid(message: String) -> RuntimeError {
    RuntimeError::ControlError(message.into())
}
