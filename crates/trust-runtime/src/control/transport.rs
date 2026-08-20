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
            if path.exists() {
                let _ = std::fs::remove_file(path);
            }
            let listener = std::os::unix::net::UnixListener::bind(path).map_err(|err| {
                RuntimeError::ControlError(format!("bind {path:?}: {err}").into())
            })?;
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
