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

/// Binds the control socket so that no other local user can ever connect to it.
///
/// Spec 11 says a generated Unix socket is "created with restrictive permissions (0600)"
/// (section 6.8.3), and that a local Unix control socket without a configured token "may use
/// Admin authority because the socket is local and created with mode 0600" (section 6.9.5).
/// A socket that is bound but not yet listening refuses every connection, so the socket is
/// bound, restricted to its owner and only then made to listen: no connection can be made
/// while its mode is still the umask default. The process umask, which every thread shares, is
/// never changed. A path that holds anything other than a socket is refused, not removed.
#[cfg(unix)]
fn bind_owner_only_socket(
    path: &std::path::Path,
) -> Result<std::os::unix::net::UnixListener, RuntimeError> {
    use std::os::unix::fs::{FileTypeExt, PermissionsExt};

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
    let failed = |action: &str, err: std::io::Error| {
        RuntimeError::ControlError(format!("{action} {}: {err}", path.display()).into())
    };
    let socket = unix_stream_socket().map_err(|err| failed("create socket for", err.into()))?;
    let address =
        rustix::net::SocketAddrUnix::new(path).map_err(|err| failed("address", err.into()))?;
    rustix::net::bind(&socket, &address).map_err(|err| failed("bind", err.into()))?;
    // From here on a failure removes the socket file this call created.
    let listen = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))
        .map_err(|err| failed("restrict", err))
        .and_then(|()| {
            rustix::net::listen(&socket, 128).map_err(|err| failed("listen on", err.into()))
        });
    if let Err(err) = listen {
        let _ = std::fs::remove_file(path);
        return Err(err);
    }
    Ok(std::os::unix::net::UnixListener::from(socket))
}

/// A close-on-exec Unix stream socket, as `UnixListener::bind` creates. Apple targets have no
/// atomic close-on-exec socket flag, so the flag is set right after creation, as `std` does.
#[cfg(unix)]
fn unix_stream_socket() -> rustix::io::Result<std::os::fd::OwnedFd> {
    use rustix::net::{AddressFamily, SocketType};
    #[cfg(not(target_vendor = "apple"))]
    {
        rustix::net::socket_with(
            AddressFamily::UNIX,
            SocketType::STREAM,
            rustix::net::SocketFlags::CLOEXEC,
            None,
        )
    }
    #[cfg(target_vendor = "apple")]
    {
        let socket = rustix::net::socket(AddressFamily::UNIX, SocketType::STREAM, None)?;
        rustix::io::fcntl_setfd(&socket, rustix::io::FdFlags::CLOEXEC)?;
        Ok(socket)
    }
}

#[cfg(all(test, unix))]
mod tests;
