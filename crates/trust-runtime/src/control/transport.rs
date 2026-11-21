//! Control server transport (TCP/Unix).

use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::sync::Arc;
use std::thread;

use crate::error::RuntimeError;

use super::{prepare_request_line, ControlEndpoint, ControlState};

/// A served endpoint: the endpoint that is bound and, on Unix-like systems, the socket file of a Unix
/// endpoint, which the control server removes when it is dropped.
pub(crate) struct Served {
    pub(crate) endpoint: ControlEndpoint,
    #[cfg(unix)]
    pub(crate) socket_file: Option<SocketFile>,
}

/// Binds `endpoint` and starts serving it. The returned endpoint is the one that is bound: a TCP
/// endpoint with port 0 comes back with the port that the operating system chose.
pub(crate) fn spawn_control_server(
    endpoint: &ControlEndpoint,
    state: Arc<ControlState>,
) -> Result<Served, RuntimeError> {
    match endpoint {
        ControlEndpoint::Tcp(addr) => {
            let listener = TcpListener::bind(addr)
                .map_err(|err| RuntimeError::ControlError(format!("bind {addr}: {err}").into()))?;
            let bound = listener.local_addr().map_err(|err| {
                RuntimeError::ControlError(format!("bound address of {addr}: {err}").into())
            })?;
            let state = state.clone();
            thread::spawn(move || {
                for stream in listener.incoming().map_while(Result::ok) {
                    let client = stream.peer_addr().map(|addr| addr.to_string()).ok();
                    let state = state.clone();
                    thread::spawn(move || handle_client(stream, state, client));
                }
            });
            Ok(Served {
                endpoint: ControlEndpoint::Tcp(bound),
                #[cfg(unix)]
                socket_file: None,
            })
        }
        #[cfg(unix)]
        ControlEndpoint::Unix(path) => {
            let (listener, socket_file) = bind_owner_only_socket(path)?;
            let state = state.clone();
            thread::spawn(move || {
                for stream in listener.incoming().map_while(Result::ok) {
                    let state = state.clone();
                    thread::spawn(move || handle_unix_client(stream, state));
                }
            });
            Ok(Served {
                endpoint: ControlEndpoint::Unix(path.clone()),
                socket_file: Some(socket_file),
            })
        }
    }
}

/// Writes `line` and its newline in one write. A newline written on its own is held by the TCP
/// stack until the peer acknowledges the line (Nagle's algorithm meeting delayed acknowledgements):
/// 41 to 44 ms a request on Linux, measured on loopback on 4 November 2026 (specification 11,
/// "Runtime control request, job, and state-surface contract").
pub(crate) fn write_line(writer: &mut impl Write, line: &str) -> std::io::Result<()> {
    writer.write_all(format!("{line}\n").as_bytes())
}

fn handle_client(stream: std::net::TcpStream, state: Arc<ControlState>, client: Option<String>) {
    let reader = match stream.try_clone() {
        Ok(clone) => BufReader::new(clone),
        Err(_) => return,
    };
    let mut writer = stream;
    for line in reader.lines().map_while(Result::ok) {
        if let Some(response) = prepare_request_line(&line, &state, client.as_deref()) {
            let _ = write_line(&mut writer, response.response());
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
            let _ = write_line(&mut writer, response.response());
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
/// never changed. A path that holds anything other than a socket is refused, not removed. The
/// socket's file comes back with the listener and is removed when it is dropped.
#[cfg(unix)]
fn bind_owner_only_socket(
    path: &std::path::Path,
) -> Result<(std::os::unix::net::UnixListener, SocketFile), RuntimeError> {
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
    // From here on a failure removes the socket file this call created: `file` does when it drops.
    let file = match SocketFile::bound_at(path) {
        Ok(file) => file,
        Err(err) => {
            let _ = std::fs::remove_file(path);
            return Err(failed("inspect", err));
        }
    };
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))
        .map_err(|err| failed("restrict", err))?;
    rustix::net::listen(&socket, 128).map_err(|err| failed("listen on", err.into()))?;
    Ok((std::os::unix::net::UnixListener::from(socket), file))
}

/// The socket file that a control server bound. Dropping it removes the file while the path still
/// holds that socket, so a runtime that ends cleanly leaves no socket behind (spec 11, section
/// 6.8.0a), and a socket that another server has bound at the path since is kept. The socket is
/// known by its device and inode: its listener, which stays open until the process ends, keeps that
/// inode in use, so no other file at the path can have it while this file is held.
#[cfg(unix)]
#[derive(Debug)]
pub(crate) struct SocketFile {
    path: std::path::PathBuf,
    device: u64,
    inode: u64,
}

#[cfg(unix)]
impl SocketFile {
    /// The socket that `bind` has just created at `path`.
    fn bound_at(path: &std::path::Path) -> std::io::Result<Self> {
        use std::os::unix::fs::MetadataExt;

        let metadata = std::fs::symlink_metadata(path)?;
        Ok(Self {
            path: path.to_path_buf(),
            device: metadata.dev(),
            inode: metadata.ino(),
        })
    }
}

#[cfg(unix)]
impl Drop for SocketFile {
    fn drop(&mut self) {
        use std::os::unix::fs::{FileTypeExt, MetadataExt};

        if let Ok(metadata) = std::fs::symlink_metadata(&self.path)
            && metadata.file_type().is_socket()
            && metadata.dev() == self.device
            && metadata.ino() == self.inode
        {
            let _ = std::fs::remove_file(&self.path);
        }
    }
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

#[cfg(test)]
#[path = "transport/write_line_tests.rs"]
mod write_line_tests;
