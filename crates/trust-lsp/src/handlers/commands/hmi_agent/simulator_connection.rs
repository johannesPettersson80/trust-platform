//! The connection of a simulator journey to the simulator's HMI web server (specification 43,
//! AT-08): TCP for an `http` address, and TLS over TCP with the certificates the system trusts
//! (`native-tls`) for `https`, as the extension's transport reached the simulator. It holds no rule
//! of the journey.
//!
//! A journey reaches only this computer, so no host name is looked up: `localhost` is tried as
//! 127.0.0.1, then as [::1]. Connecting, the TLS handshake and every socket read and write are
//! bounded by one deadline per exchange: TLS reads and writes through [`Bounded`], so that one TLS
//! read made of several socket reads ends at that deadline too.

use std::fmt::Display;
use std::io::{self, Read, Write};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, TcpStream};
use std::time::{Duration, Instant};

use tower_lsp::lsp_types::Url;

/// The longest one socket read waits before the deadline is checked again.
const SLICE: Duration = Duration::from_millis(100);

/// The least time a socket read or write waits: none waits forever.
const LEAST: Duration = Duration::from_millis(1);

/// A connection to the simulator: TCP for `http`, TLS over TCP for `https`, its socket reads and
/// writes bounded by a deadline.
pub(super) trait Connection: Read + Write + Send {
    /// Lets every socket read and write end by `deadline`.
    fn until(&mut self, deadline: Instant);
}

impl Connection for Bounded {
    fn until(&mut self, deadline: Instant) {
        self.deadline = deadline;
    }
}

impl Connection for native_tls::TlsStream<Bounded> {
    fn until(&mut self, deadline: Instant) {
        self.get_mut().deadline = deadline;
    }
}

/// A TCP connection whose socket reads and writes end by a deadline. A read waits at most a slice,
/// and none after the deadline; a read or write that runs out of time is `WouldBlock`, which a TLS
/// stream takes for "try again" and keeps its state, and its caller then checks the deadline. So a
/// TLS read that waits on several socket reads, one record arriving a few bytes at a time, ends at
/// the deadline too.
pub(super) struct Bounded {
    tcp: TcpStream,
    deadline: Instant,
}

impl Bounded {
    /// `tcp`, its reads and writes ending by `deadline`.
    pub(super) fn new(tcp: TcpStream, deadline: Instant) -> Self {
        Self { tcp, deadline }
    }

    /// The time left before the deadline.
    fn left(&self) -> Duration {
        self.deadline.saturating_duration_since(Instant::now())
    }
}

impl Read for Bounded {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        let left = self.left();
        if left.is_zero() {
            return Err(io::ErrorKind::WouldBlock.into());
        }
        self.tcp
            .set_read_timeout(Some(left.min(SLICE).max(LEAST)))?;
        out_of_time(self.tcp.read(buffer))
    }
}

impl Write for Bounded {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.tcp.set_write_timeout(Some(self.left().max(LEAST)))?;
        out_of_time(self.tcp.write(bytes))
    }

    fn flush(&mut self) -> io::Result<()> {
        self.tcp.flush()
    }
}

/// A socket wait that ran out as `WouldBlock`: Unix reports it so, Windows as `TimedOut`.
fn out_of_time<T>(result: io::Result<T>) -> io::Result<T> {
    result.map_err(|error| match error.kind() {
        io::ErrorKind::TimedOut => io::ErrorKind::WouldBlock.into(),
        _ => error,
    })
}

/// The time left before `deadline`; none left is a timeout.
pub(super) fn time_left(deadline: Instant) -> io::Result<Duration> {
    let left = deadline.saturating_duration_since(Instant::now());
    if left.is_zero() {
        return Err(io::ErrorKind::TimedOut.into());
    }
    Ok(left)
}

/// The addresses of a host of this computer: an IP address as it is, and `localhost` as 127.0.0.1,
/// then [::1]. Any other host is refused, since a journey reaches only this computer.
fn addresses(url: &Url) -> io::Result<Vec<SocketAddr>> {
    let port = url
        .port_or_known_default()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "no port"))?;
    let host = url.host_str().unwrap_or_default();
    let host = host.trim_start_matches('[').trim_end_matches(']');
    let ips: Vec<IpAddr> = if host.eq_ignore_ascii_case("localhost") {
        vec![Ipv4Addr::LOCALHOST.into(), Ipv6Addr::LOCALHOST.into()]
    } else {
        let ip = host.parse::<IpAddr>().map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "a host other than this computer",
            )
        })?;
        vec![ip]
    };
    Ok(ips
        .into_iter()
        .map(|ip| SocketAddr::new(ip, port))
        .collect())
}

/// Connects to the host of `url` before `deadline`, its reads and writes ending by `deadline`: TCP
/// for `http`, TLS for `https`. A refused TLS handshake is an error that names TLS.
pub(super) fn connect(url: &Url, deadline: Instant) -> io::Result<Box<dyn Connection>> {
    let mut failure = io::Error::new(io::ErrorKind::NotFound, "no address");
    for address in addresses(url)? {
        let wait = time_left(deadline)?;
        let tcp = match TcpStream::connect_timeout(&address, wait) {
            Ok(tcp) => tcp,
            Err(error) => {
                failure = error;
                continue;
            }
        };
        let bounded = Bounded::new(tcp, deadline);
        return match url.scheme() {
            "http" => Ok(Box::new(bounded)),
            "https" => secure(url, bounded, deadline),
            other => Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("{other} is neither http nor https"),
            )),
        };
    }
    Err(failure)
}

/// The TLS handshake over `tcp`, continued slice by slice until `deadline`.
fn secure(url: &Url, tcp: Bounded, deadline: Instant) -> io::Result<Box<dyn Connection>> {
    let tls = |error: &dyn Display| io::Error::other(format!("TLS: {error}"));
    let connector = native_tls::TlsConnector::new().map_err(|error| tls(&error))?;
    let host = url.host_str().unwrap_or_default();
    let domain = host.trim_start_matches('[').trim_end_matches(']');
    let mut handshake = connector.connect(domain, tcp);
    loop {
        match handshake {
            Ok(stream) => return Ok(Box::new(stream)),
            Err(native_tls::HandshakeError::Failure(error)) => return Err(tls(&error)),
            Err(native_tls::HandshakeError::WouldBlock(middle)) => {
                time_left(deadline)?;
                handshake = middle.handshake();
            }
        }
    }
}

#[cfg(test)]
#[path = "simulator_connection_tests.rs"]
mod tests;
