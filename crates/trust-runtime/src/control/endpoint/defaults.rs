//! The platform default control endpoint, the per-user folders that hold a fallback socket or a
//! generated token, and the generated control token (spec 11, section 6.8.0a).

use std::path::{Path, PathBuf};

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use rand::TryRng;
use sha2::{Digest, Sha256};

use crate::control::ControlEndpoint;
use crate::error::RuntimeError;

#[cfg(unix)]
use super::file::{ensure_private_directory, ENDPOINT_DIR};

/// The socket's file name inside the endpoint directory.
#[cfg(unix)]
const SOCKET_FILE: &str = "control.sock";
/// Hexadecimal digits of the project-path hash that name a fallback socket or a token file.
const HASH_HEX_DIGITS: usize = 16;
/// The longest socket path that fits `sockaddr_un`, counting the terminating NUL byte: 108 bytes
/// on Linux.
#[cfg(target_os = "linux")]
const SOCKET_PATH_CAPACITY: usize = 108;
/// The longest socket path that fits `sockaddr_un`, counting the terminating NUL byte: 104 bytes
/// on macOS and the BSDs.
#[cfg(all(unix, not(target_os = "linux")))]
const SOCKET_PATH_CAPACITY: usize = 104;
/// Random bytes in a control token, before base64url encoding.
const CONTROL_TOKEN_BYTES: usize = 32;

/// The endpoint that a client uses for a project whose runtime published no endpoint file (spec
/// 11, section 6.8.0a).
///
/// On Unix-like systems it is `<project>/.trust-runtime/control.sock`. When that path does not
/// fit a socket address, it is the fallback socket `trust-<h>.sock` in the per-user runtime
/// directory, where `<h>` is the first 16 hexadecimal digits of the SHA-256 of the canonical
/// project path. A runtime that could not create the project's endpoint folder also listens on the
/// fallback socket, so a client takes the fallback socket when only that socket exists. On Windows
/// it is `tcp://127.0.0.1:0`: the operating system chooses the port when the runtime binds.
///
/// # Errors
///
/// Returns an error when the project folder cannot be resolved to its canonical path, or when the
/// fallback socket is needed and the per-user runtime directory is not private or the socket path
/// does not fit.
pub fn platform_default_endpoint(project_root: &Path) -> Result<ControlEndpoint, RuntimeError> {
    #[cfg(unix)]
    {
        client_unix_default_in(&canonical_project_root(project_root)?, runtime_directory)
    }
    #[cfg(not(unix))]
    {
        let _ = project_root;
        Ok(unbound_loopback())
    }
}

/// The Unix default that a client uses for a canonical project folder, with the runtime directory
/// supplied: the project-local socket, unless only the fallback socket exists.
#[cfg(unix)]
pub(super) fn client_unix_default_in(
    project_root: &Path,
    runtime_dir: impl Fn() -> Result<PathBuf, RuntimeError>,
) -> Result<ControlEndpoint, RuntimeError> {
    let fallback_only = !is_socket(&project_socket(project_root))
        && runtime_dir()
            .map(|dir| is_socket(&fallback_socket(project_root, &dir)))
            .unwrap_or(false);
    unix_default_endpoint_in(project_root, !fallback_only, runtime_dir)
}

/// The endpoint that a runtime binds when `[runtime.control] endpoint` is omitted (spec 11,
/// section 6.8.0a). It is the platform default, except that a runtime which cannot create or
/// restrict the project's `.trust-runtime` folder binds the fallback socket in the per-user runtime
/// directory, so that a read-only or foreign project folder never stops the runtime.
///
/// # Errors
///
/// Returns an error when the project folder cannot be resolved to its canonical path, or when the
/// fallback socket is needed and cannot be placed safely.
pub(super) fn default_binding_endpoint(
    project_root: &Path,
) -> Result<ControlEndpoint, RuntimeError> {
    #[cfg(unix)]
    {
        let canonical = canonical_project_root(project_root)?;
        let local_ready = ensure_private_directory(&canonical.join(ENDPOINT_DIR)).is_ok();
        unix_default_endpoint_in(&canonical, local_ready, runtime_directory)
    }
    #[cfg(not(unix))]
    {
        let _ = project_root;
        Ok(unbound_loopback())
    }
}

/// `tcp://127.0.0.1:0`, the Windows default.
#[cfg(not(unix))]
fn unbound_loopback() -> ControlEndpoint {
    ControlEndpoint::Tcp(std::net::SocketAddr::new(
        std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST),
        0,
    ))
}

/// The project folder as the operating system names it, so that every client and the runtime
/// derive the same fallback socket and token file from it.
pub(crate) fn canonical_project_root(project_root: &Path) -> Result<PathBuf, RuntimeError> {
    std::fs::canonicalize(project_root).map_err(|error| {
        RuntimeError::InvalidBundle(
            format!("project folder {}: {error}", project_root.display()).into(),
        )
    })
}

/// The first 16 hexadecimal digits of the SHA-256 of a canonical project path, taken over the
/// path's bytes (its UTF-8 text on Windows). It names the fallback socket on Unix-like systems and
/// the per-user token file on Windows.
pub(crate) fn project_path_hash(canonical_project: &Path) -> String {
    let mut hasher = Sha256::new();
    hasher.update(path_bytes(canonical_project));
    let digest = hex::encode(hasher.finalize());
    digest[..HASH_HEX_DIGITS].to_string()
}

#[cfg(unix)]
fn path_bytes(path: &Path) -> Vec<u8> {
    use std::os::unix::ffi::OsStrExt;
    path.as_os_str().as_bytes().to_vec()
}

#[cfg(not(unix))]
fn path_bytes(path: &Path) -> Vec<u8> {
    path.to_string_lossy().into_owned().into_bytes()
}

/// The Unix default for a canonical project folder: the project-local socket when `local_usable`
/// is true and its path fits a socket address, else the fallback socket in the runtime directory
/// that `runtime_dir` supplies. The runtime directory is asked for only when the fallback is
/// needed, so a project-local socket never creates it.
///
/// # Errors
///
/// Returns an error when the runtime directory is unsafe or the fallback path does not fit.
#[cfg(unix)]
pub(super) fn unix_default_endpoint_in(
    project_root: &Path,
    local_usable: bool,
    runtime_dir: impl FnOnce() -> Result<PathBuf, RuntimeError>,
) -> Result<ControlEndpoint, RuntimeError> {
    let local = project_socket(project_root);
    if local_usable && fits_socket_path(&local) {
        return Ok(ControlEndpoint::Unix(local));
    }
    let fallback = fallback_socket(project_root, &runtime_dir()?);
    if !fits_socket_path(&fallback) {
        return Err(RuntimeError::ControlError(
            format!(
                "the fallback control socket {} does not fit a socket address ({SOCKET_PATH_CAPACITY} bytes); set [runtime.control] endpoint",
                fallback.display()
            )
            .into(),
        ));
    }
    Ok(ControlEndpoint::Unix(fallback))
}

/// `<project>/.trust-runtime/control.sock`.
#[cfg(unix)]
pub(super) fn project_socket(project_root: &Path) -> PathBuf {
    project_root.join(ENDPOINT_DIR).join(SOCKET_FILE)
}

/// `<runtime dir>/trust-<h>.sock`.
#[cfg(unix)]
pub(super) fn fallback_socket(project_root: &Path, runtime_dir: &Path) -> PathBuf {
    runtime_dir.join(format!("trust-{}.sock", project_path_hash(project_root)))
}

/// Whether `path` fits a socket address. The path needs room for its terminating NUL byte, hence
/// the strict comparison.
#[cfg(unix)]
pub(super) fn fits_socket_path(path: &Path) -> bool {
    use std::os::unix::ffi::OsStrExt;
    path.as_os_str().as_bytes().len() < SOCKET_PATH_CAPACITY
}

/// Whether a socket file exists at `path`.
#[cfg(unix)]
fn is_socket(path: &Path) -> bool {
    use std::os::unix::fs::FileTypeExt;
    std::fs::symlink_metadata(path).is_ok_and(|metadata| metadata.file_type().is_socket())
}

/// The per-user directory of a fallback socket for this process's `$XDG_RUNTIME_DIR`, system
/// temporary directory and effective user (see [`runtime_directory_in`]).
///
/// # Errors
///
/// Returns an error when no private runtime directory can be had.
#[cfg(unix)]
pub(super) fn runtime_directory() -> Result<PathBuf, RuntimeError> {
    runtime_directory_in(
        std::env::var_os("XDG_RUNTIME_DIR"),
        &std::env::temp_dir(),
        rustix::process::geteuid().as_raw(),
    )
}

/// The per-user directory of a fallback socket: `$XDG_RUNTIME_DIR` when it is set, not empty and
/// passes the checks of [`ensure_user_private_directory`], else `trust-runtime-<uid>` in the system
/// temporary directory, which must pass them too. A `$XDG_RUNTIME_DIR` that is open to the user's
/// group, as some systems leave it against the XDG Base Directory Specification's mode 0700, or
/// that belongs to another user, as after `su`, is not used, and the runtime does not stop for it.
///
/// # Errors
///
/// Returns an error when `trust-runtime-<uid>` cannot be created or fails a check.
#[cfg(unix)]
pub(super) fn runtime_directory_in(
    xdg_runtime_dir: Option<std::ffi::OsString>,
    temp_dir: &Path,
    uid: u32,
) -> Result<PathBuf, RuntimeError> {
    if let Some(dir) = xdg_runtime_dir.filter(|value| !value.is_empty()) {
        let dir = PathBuf::from(dir);
        if ensure_user_private_directory(&dir, uid).is_ok() {
            return Ok(dir);
        }
    }
    let dir = temp_dir.join(format!("trust-runtime-{uid}"));
    ensure_user_private_directory(&dir, uid)?;
    Ok(dir)
}

/// Creates `dir` with mode 0700 when it is missing, then checks that it is a directory and not a
/// symbolic link, that user `uid` owns it, and that no group or other permission bit is set.
#[cfg(unix)]
pub(super) fn ensure_user_private_directory(dir: &Path, uid: u32) -> Result<(), RuntimeError> {
    use std::os::unix::fs::{DirBuilderExt, MetadataExt};

    let refused = |reason: String| {
        RuntimeError::ControlError(
            format!(
                "runtime directory {} cannot hold a control socket: {reason}",
                dir.display()
            )
            .into(),
        )
    };
    match std::fs::DirBuilder::new().mode(0o700).create(dir) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
        Err(error) => return Err(refused(format!("create: {error}"))),
    }
    let metadata =
        std::fs::symlink_metadata(dir).map_err(|error| refused(format!("inspect: {error}")))?;
    if !metadata.file_type().is_dir() {
        return Err(refused("it is not a directory".to_string()));
    }
    if metadata.uid() != uid {
        return Err(refused(format!(
            "it belongs to user {}, not to user {uid}",
            metadata.uid()
        )));
    }
    if metadata.mode() & 0o077 != 0 {
        return Err(refused(format!(
            "other users can use it (mode {:o})",
            metadata.mode() & 0o777
        )));
    }
    Ok(())
}

/// The per-user folder of the token files that Windows runtimes generate:
/// `%LOCALAPPDATA%\trust\runtimes`. Windows keeps the local application data folder private to its
/// user.
///
/// # Errors
///
/// Returns an error when `LOCALAPPDATA` is not set.
#[cfg(windows)]
pub(super) fn per_user_token_directory() -> Result<PathBuf, RuntimeError> {
    let base = std::env::var_os("LOCALAPPDATA")
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            RuntimeError::ControlError(
                "LOCALAPPDATA is not set, so a generated control token has no per-user folder"
                    .into(),
            )
        })?;
    Ok(per_user_token_directory_in(Path::new(&base)))
}

/// `<local application data>\trust\runtimes`.
#[cfg_attr(not(windows), allow(dead_code))]
pub(super) fn per_user_token_directory_in(local_app_data: &Path) -> PathBuf {
    local_app_data.join("trust").join("runtimes")
}

/// A control token: 32 bytes from the operating system's secure random source, encoded as
/// unpadded base64url. It is never derived from a predictable value.
///
/// # Errors
///
/// Returns an error when the random source fails.
pub fn generate_control_token() -> Result<String, RuntimeError> {
    let mut rng = rand::rngs::SysRng;
    token_from_rng(&mut rng)
}

/// Encodes 32 bytes from `rng`. A failing source is an error: no fallback value is ever produced.
pub(super) fn token_from_rng<R: TryRng>(rng: &mut R) -> Result<String, RuntimeError> {
    let mut bytes = [0_u8; CONTROL_TOKEN_BYTES];
    rng.try_fill_bytes(&mut bytes).map_err(|error| {
        RuntimeError::ControlError(
            format!("operating-system secure random source failed: {error}").into(),
        )
    })?;
    Ok(URL_SAFE_NO_PAD.encode(bytes))
}
