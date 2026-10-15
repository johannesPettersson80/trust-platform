//! Control server transport (TCP/Unix).

use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::sync::Arc;
use std::thread;

use crate::error::RuntimeError;

use super::{prepare_request_line, ControlEndpoint, ControlState};

pub(crate) fn spawn_control_server(
    endpoint: &ControlEndpoint,
    state: Arc<ControlState>,
) -> Result<(), RuntimeError> {
    match endpoint {
        ControlEndpoint::Tcp(addr) => {
            let listener = TcpListener::bind(addr)
                .map_err(|err| RuntimeError::ControlError(format!("bind {addr}: {err}").into()))?;
            let state = state.clone();
            thread::spawn(move || {
                for stream in listener.incoming().map_while(Result::ok) {
                    let client = stream.peer_addr().map(|addr| addr.to_string()).ok();
                    let state = state.clone();
                    thread::spawn(move || handle_client(stream, state, client));
                }
            });
        }
        #[cfg(unix)]
        ControlEndpoint::Unix(path) => {
            let listener = bind_owner_only_socket(path)?;
            // Defence in depth: the umask already created the socket owner-only.
            set_unix_permissions(path)?;
            let state = state.clone();
            thread::spawn(move || {
                for stream in listener.incoming().map_while(Result::ok) {
                    let state = state.clone();
                    thread::spawn(move || handle_unix_client(stream, state));
                }
            });
        }
    }
    Ok(())
}

fn handle_client(stream: std::net::TcpStream, state: Arc<ControlState>, client: Option<String>) {
    let reader = match stream.try_clone() {
        Ok(clone) => BufReader::new(clone),
        Err(_) => return,
    };
    let mut writer = stream;
    for line in reader.lines().map_while(Result::ok) {
        if let Some(response) = prepare_request_line(&line, &state, client.as_deref()) {
            let _ = writeln!(writer, "{}", response.response());
            response.complete(&state);
        }
    }
}

#[cfg(unix)]
fn handle_unix_client(stream: std::os::unix::net::UnixStream, state: Arc<ControlState>) {
    let reader = match stream.try_clone() {
        Ok(clone) => BufReader::new(clone),
        Err(_) => return,
    };
    let mut writer = stream;
    for line in reader.lines().map_while(Result::ok) {
        if let Some(response) = prepare_request_line(&line, &state, Some("unix")) {
            let _ = writeln!(writer, "{}", response.response());
            response.complete(&state);
        }
    }
}

/// The process umask is process-wide, so the binds that narrow it run one at a time.
#[cfg(unix)]
static UMASK_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// Binds the control socket so that it is owner-only from the moment it exists.
///
/// Spec 11 says a generated Unix socket is "created with restrictive permissions (0600)"
/// (section 6.8.3), and that a local Unix control socket without a configured token "may use
/// Admin authority because the socket is local and created with mode 0600" (section 6.9.5).
/// Binding first and narrowing the mode afterwards would leave a window in which other local
/// users can connect with that Admin authority.
/// The umask is therefore narrowed only around `bind` and restored right after it, even when
/// the bind fails. A path that holds anything other than a socket is refused, not removed.
#[cfg(unix)]
fn bind_owner_only_socket(
    path: &std::path::Path,
) -> Result<std::os::unix::net::UnixListener, RuntimeError> {
    use std::os::unix::fs::FileTypeExt;

    match std::fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_socket() => {
            std::fs::remove_file(path).map_err(|err| {
                RuntimeError::ControlError(
                    format!("remove stale socket {}: {err}", path.display()).into(),
                )
            })?;
        }
        Ok(_) => {
            return Err(RuntimeError::ControlError(
                format!(
                    "control socket path {} exists and is not a socket; refusing to replace it",
                    path.display()
                )
                .into(),
            ));
        }
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
        Err(err) => {
            return Err(RuntimeError::ControlError(
                format!("inspect {}: {err}", path.display()).into(),
            ));
        }
    }
    // Owner-only umask: a socket created under 0177 gets mode 0777 & !0177 = 0600.
    let _umask_guard = UMASK_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let previous = rustix::process::umask(rustix::fs::Mode::from_raw_mode(0o177));
    let bound = std::os::unix::net::UnixListener::bind(path);
    rustix::process::umask(previous);
    bound.map_err(|err| RuntimeError::ControlError(format!("bind {path:?}: {err}").into()))
}

#[cfg(unix)]
fn set_unix_permissions(path: &std::path::Path) -> Result<(), RuntimeError> {
    use std::os::unix::fs::PermissionsExt;
    let mut perms = std::fs::metadata(path)
        .map_err(|err| RuntimeError::ControlError(format!("socket metadata: {err}").into()))?
        .permissions();
    perms.set_mode(0o600);
    std::fs::set_permissions(path, perms)
        .map_err(|err| RuntimeError::ControlError(format!("socket chmod: {err}").into()))
}

#[cfg(all(test, unix))]
mod tests;
