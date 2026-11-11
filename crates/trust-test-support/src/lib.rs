//! Support that the workspace's tests share; no product uses it.

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};

/// A path under the system's temporary directory that is this test's alone: `name`, after this
/// process's tag and a counter of this process (specification 22, "Native test and docs-capture
/// lifecycle portability"). Tests run at once in one process and in several, so a name made unique
/// by a clock stamp alone gave two of them one path. The unique part comes first, so a file name
/// keeps its extension. The path is not created.
#[must_use]
pub fn unique_temp_path(name: &str) -> PathBuf {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    std::env::temp_dir().join(format!(
        "{}-{}-{name}",
        process_tag(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ))
}

/// A path under the system's temporary directory for one name in this process: `name`, after this
/// process's tag. The same name gives the same path within the process, for a test that reopens what
/// it wrote, as across a restart, or that names one path twice; processes keep apart by their tags,
/// and the name must be the test's own. The path is not created.
#[must_use]
pub fn process_temp_path(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!("{}-{name}", process_tag()))
}

/// This process among the processes of every run: its id, and the microsecond of the second in which
/// it first asked for a path, so that a later process given the same id does not take a path that an
/// earlier run left behind.
fn process_tag() -> &'static str {
    static TAG: OnceLock<String> = OnceLock::new();
    TAG.get_or_init(|| {
        let micros = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |since| since.subsec_micros());
        format!("{}-{micros}", std::process::id())
    })
}

/// A loopback TCP port where nothing answers: a connection to it is refused at once on every system,
/// and while the value lives the system gives the port to no other socket, neither to a bind of
/// port 0 nor to a connection. The address of an endpoint where nothing answers, for the whole
/// test. An established connection holds the port, so that no socket listens there: a socket bound
/// to the port without listening would hold it too, but macOS answers no connection to one, which
/// then fails only at its own timeout (7.8 s for a plain connect in CI). On macOS a socket that
/// binds this very port with `SO_REUSEADDR` (as Rust's `TcpListener::bind` does) can still take it
/// from an established connection; a test gives such a port to no program. A port read from a
/// listener that the test then closes can be taken by any other socket before the test is done
/// with it.
#[derive(Debug)]
pub struct RefusedPort {
    _held: std::net::TcpStream,
    _peer: std::net::TcpStream,
    port: u16,
}

impl RefusedPort {
    /// The held port.
    #[must_use]
    pub fn port(&self) -> u16 {
        self.port
    }
}

/// Holds a free loopback port where nothing answers ([`RefusedPort`]): a socket bound to the port and
/// connected to a listener, which closes once it has taken the connection.
///
/// # Panics
///
/// The operating system gives no socket, no port or no connection.
#[must_use]
pub fn refused_loopback_port() -> RefusedPort {
    let listener = std::net::TcpListener::bind(std::net::SocketAddr::from(([127, 0, 0, 1], 0)))
        .expect("a loopback listener");
    let held = loopback_socket();
    let port = bound_port(&held);
    held.connect(
        &listener
            .local_addr()
            .expect("the listener's address")
            .into(),
    )
    .expect("connect the held port");
    let (peer, _) = listener.accept().expect("the held port's connection");
    drop(listener);
    RefusedPort {
        _held: held.into(),
        _peer: peer,
        port,
    }
}

/// A loopback TCP port that the test holds without listening on it, until a program that binds its
/// port late listens there ([`HeldPort::listen`]); no other socket can take it in the meantime.
/// Linux and Windows refuse a connection to it while it does not listen; macOS answers none, so that
/// a connection there fails only at its own timeout. An endpoint where nothing answers is a
/// [`RefusedPort`].
#[derive(Debug)]
pub struct HeldPort {
    socket: socket2::Socket,
    port: u16,
}

impl HeldPort {
    /// The held port.
    #[must_use]
    pub fn port(&self) -> u16 {
        self.port
    }

    /// Starts listening on the held port: a program that binds its port late, which no other
    /// socket could take in the meantime.
    ///
    /// # Panics
    ///
    /// The operating system refuses to listen on the port.
    #[must_use]
    pub fn listen(self) -> std::net::TcpListener {
        self.socket.listen(128).expect("listen on the held port");
        self.socket.into()
    }
}

/// Holds a free loopback port without listening on it ([`HeldPort`]).
///
/// # Panics
///
/// The operating system gives no socket or no port.
#[must_use]
pub fn held_loopback_port() -> HeldPort {
    let socket = loopback_socket();
    let port = bound_port(&socket);
    HeldPort { socket, port }
}

/// A TCP socket bound to a free loopback port.
fn loopback_socket() -> socket2::Socket {
    let socket = socket2::Socket::new(
        socket2::Domain::IPV4,
        socket2::Type::STREAM,
        Some(socket2::Protocol::TCP),
    )
    .expect("a TCP socket");
    socket
        .bind(&std::net::SocketAddr::from(([127, 0, 0, 1], 0)).into())
        .expect("bind a loopback port");
    socket
}

/// The port that `socket` is bound to.
fn bound_port(socket: &socket2::Socket) -> u16 {
    socket
        .local_addr()
        .ok()
        .and_then(|address| address.as_socket())
        .map(|address| address.port())
        .expect("the bound port")
}

/// How `std::io::Error` writes this operating system's "address in use" error, with which a bind of
/// a port that another socket holds fails.
#[cfg(target_os = "linux")]
const ADDRESS_IN_USE: &str = "(os error 98)";
#[cfg(windows)]
const ADDRESS_IN_USE: &str = "(os error 10048)";
#[cfg(not(any(target_os = "linux", windows)))]
const ADDRESS_IN_USE: &str = "(os error 48)";

/// Whether `line`, a line of a program's log, reports a bind that failed because another socket
/// holds the address. A test that chose a free port for a program, which another socket took before
/// the program bound it, may try again with another port, and only then.
#[must_use]
pub fn reports_a_taken_port(line: &str) -> bool {
    line.contains("bind") && is_a_taken_port_error(line)
}

/// Whether `error`, the text of an error that a library returned for a bind, is this system's
/// "address in use" error: another socket holds the port. A library words its own part of the
/// message (Zenoh: "Can not create a new TCP listener bound to …"), so only the system's error is
/// read. A test that chose a free port for a library's listener may try again with another port
/// when the bind failed so, and only then.
#[must_use]
pub fn is_a_taken_port_error(error: &str) -> bool {
    error.contains(ADDRESS_IN_USE)
}

#[cfg(test)]
mod tests {
    use std::net::{SocketAddr, TcpListener, TcpStream};
    use std::time::{Duration, Instant};

    use super::{
        held_loopback_port, is_a_taken_port_error, process_temp_path, refused_loopback_port,
        reports_a_taken_port, unique_temp_path,
    };

    /// A bind of a port that a listener holds fails with this system's "address in use" error, which
    /// `reports_a_taken_port` finds in a line that names the bind, and only there, and
    /// `is_a_taken_port_error` in a library's error whatever its own wording. Catches the error code
    /// of another system (a test would never try again) and a check that takes any failure for a
    /// taken port (a real failure would be tried again and hidden).
    #[test]
    fn a_bind_of_a_taken_port_is_reported_as_taken() {
        let holder = TcpListener::bind("127.0.0.1:0").expect("hold a port");
        let address = holder.local_addr().expect("the held address");
        let error = TcpListener::bind(address).expect_err("a second listener on the held port");

        let line = format!("Error: bind {address}: {error}");
        assert!(reports_a_taken_port(&line), "{line}");
        assert!(
            !reports_a_taken_port(&format!("Error: connect {address}: {error}")),
            "a line that names no bind"
        );
        assert!(
            !reports_a_taken_port("Error: bind 127.0.0.1:1: Permission denied (os error 13)"),
            "another bind failure"
        );
        let library = format!("Can not create a new TCP listener bound to tcp/{address}: {error}");
        assert!(is_a_taken_port_error(&library), "{library}");
        assert!(
            !is_a_taken_port_error(
                "Can not create a new TCP listener: Permission denied (os error 13)"
            ),
            "another bind failure"
        );
    }

    /// A refused port refuses a connection within 5 s on every system, and no other socket binds it
    /// while it is held. Catches a helper that listens (a connection is accepted and then waits for
    /// an answer), one that lets the port go (another socket can bind it and answer), and one that
    /// holds the port with a socket that does not listen, to which macOS answers nothing: a
    /// connection there failed only at its timeout, 7.8 s in CI (#441).
    #[test]
    fn a_refused_port_refuses_connections_at_once_and_stays_held() {
        let refused = refused_loopback_port();
        let address = SocketAddr::from(([127, 0, 0, 1], refused.port()));

        let started = Instant::now();
        assert!(
            TcpStream::connect_timeout(&address, Duration::from_secs(10)).is_err(),
            "a connection to the held port is refused"
        );
        assert!(
            started.elapsed() < Duration::from_secs(5),
            "refused at once, not at a timeout: {:?}",
            started.elapsed()
        );
        // A plain bind, without the `SO_REUSEADDR` that Rust's `TcpListener::bind` sets on Unix: macOS
        // lets such a bind take a port that only an established connection holds (#446's first run).
        let plain = socket2::Socket::new(
            socket2::Domain::IPV4,
            socket2::Type::STREAM,
            Some(socket2::Protocol::TCP),
        )
        .expect("a TCP socket");
        assert!(
            plain.bind(&address.into()).is_err(),
            "no other socket binds the held port"
        );
    }

    /// A held port that starts listening takes connections on that same port. Catches a `listen`
    /// that binds a new socket, on another port or on one that another socket took first.
    #[test]
    fn a_held_port_listens_when_asked() {
        let held = held_loopback_port();
        let port = held.port();
        let address = SocketAddr::from(([127, 0, 0, 1], port));
        let listener = held.listen();

        assert_eq!(
            listener
                .local_addr()
                .expect("the listener's address")
                .port(),
            port
        );
        TcpStream::connect_timeout(&address, Duration::from_secs(10))
            .expect("a connection to the port that listens now");
    }

    /// Two paths of one name in one process are two, both start with the process's id, and a file
    /// name keeps its extension. Catches a path that two tests of one name share, which the first to
    /// finish removed under the other, and a path that another process could take.
    #[test]
    fn two_paths_of_one_name_are_two() {
        let first = unique_temp_path("trust-test-support-log.jsonl");
        let second = unique_temp_path("trust-test-support-log.jsonl");
        assert_ne!(first, second);
        assert_eq!(
            first.extension().and_then(|ext| ext.to_str()),
            Some("jsonl")
        );
        assert_eq!(first.parent(), Some(std::env::temp_dir().as_path()));
        let file_name = first
            .file_name()
            .and_then(|name| name.to_str())
            .expect("a name");
        assert!(
            file_name.starts_with(&format!("{}-", std::process::id())),
            "{file_name}"
        );
    }

    /// One name gives one path within the process, and two names give two. Catches a test that names
    /// one path twice and finds two, and two tests of two names on one path.
    #[test]
    fn a_name_keeps_its_path_within_the_process() {
        assert_eq!(
            process_temp_path("trust-store.bin"),
            process_temp_path("trust-store.bin")
        );
        assert_ne!(
            process_temp_path("trust-store.bin"),
            process_temp_path("trust-other.bin")
        );
    }
}
