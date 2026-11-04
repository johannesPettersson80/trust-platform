//! The connection of a simulator journey to the simulator's HMI web server (specification 43,
//! AT-08): TCP for an `http` address, and TLS over TCP with the certificates the system trusts
//! (`native-tls`) for `https`, as the extension's transport reached the simulator. Connecting and
//! each read are bounded in time. It holds no rule of the journey.

use std::fmt::Display;
use std::io::{self, Read, Write};
use std::net::TcpStream;
use std::time::{Duration, Instant};

use tower_lsp::lsp_types::Url;

/// A connection to the simulator: TCP for `http`, TLS over TCP for `https`.
pub(super) trait Connection: Read + Write + Send {
    /// The TCP connection under it.
    fn tcp(&self) -> &TcpStream;

    /// Lets the next read wait at most `wait`, and at least a millisecond (none waits forever).
    fn wait_at_most(&self, wait: Duration) -> io::Result<()> {
        let wait = wait.max(Duration::from_millis(1));
        self.tcp().set_read_timeout(Some(wait))
    }
}

impl Connection for TcpStream {
    fn tcp(&self) -> &TcpStream {
        self
    }
}

impl Connection for native_tls::TlsStream<TcpStream> {
    fn tcp(&self) -> &TcpStream {
        self.get_ref()
    }
}

/// The time left before `deadline`; none left is a timeout.
pub(super) fn time_left(deadline: Instant) -> io::Result<Duration> {
    let left = deadline.saturating_duration_since(Instant::now());
    if left.is_zero() {
        return Err(io::ErrorKind::TimedOut.into());
    }
    Ok(left)
}

/// Connects to the host of `url` before `deadline`: TCP for `http`, TLS for `https`. A refused TLS
/// handshake is an error that names TLS.
pub(super) fn connect(url: &Url, deadline: Instant) -> io::Result<Box<dyn Connection>> {
    let host = url.host_str().unwrap_or_default();
    let mut failure = io::Error::new(io::ErrorKind::NotFound, "no address");
    for address in url.socket_addrs(|| None)? {
        let wait = time_left(deadline)?;
        let tcp = match TcpStream::connect_timeout(&address, wait) {
            Ok(tcp) => tcp,
            Err(error) => {
                failure = error;
                continue;
            }
        };
        tcp.set_read_timeout(Some(wait))?;
        tcp.set_write_timeout(Some(wait))?;
        return match url.scheme() {
            "http" => Ok(Box::new(tcp)),
            "https" => {
                let tls = |error: &dyn Display| io::Error::other(format!("TLS: {error}"));
                let connector = native_tls::TlsConnector::new().map_err(|error| tls(&error))?;
                let domain = host.trim_start_matches('[').trim_end_matches(']');
                match connector.connect(domain, tcp) {
                    Ok(stream) => Ok(Box::new(stream)),
                    Err(native_tls::HandshakeError::Failure(error)) => Err(tls(&error)),
                    Err(native_tls::HandshakeError::WouldBlock(_)) => {
                        Err(io::ErrorKind::TimedOut.into())
                    }
                }
            }
            other => Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("{other} is neither http nor https"),
            )),
        };
    }
    Err(failure)
}
