//! Tests of the simulator journey's connection (specification 43, AT-08): every exchange reaches
//! only this computer without looking a host name up, and ends at its one deadline, also when a
//! TLS read waits on several socket reads.

use std::io::{self, Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::thread;
use std::time::{Duration, Instant};

use tower_lsp::lsp_types::Url;

use super::{addresses, out_of_time, Bounded, SLICE};

fn url(text: &str) -> Url {
    Url::parse(text).expect("a web address")
}

fn address(text: &str) -> SocketAddr {
    text.parse().expect("a socket address")
}

/// Catches: `localhost` looked up through the system, which can wait without a bound, instead of
/// tried as 127.0.0.1 and then [::1]; and a host other than this computer reached.
#[test]
fn localhost_is_this_computers_two_addresses_and_no_name_is_looked_up() {
    assert_eq!(
        addresses(&url("http://localhost:8080")).expect("localhost"),
        [address("127.0.0.1:8080"), address("[::1]:8080")]
    );
    assert_eq!(
        addresses(&url("https://LOCALHOST")).expect("localhost, any case"),
        [address("127.0.0.1:443"), address("[::1]:443")]
    );
    assert_eq!(
        addresses(&url("http://[::1]:9")).expect("an IPv6 address"),
        [address("[::1]:9")]
    );
    assert_eq!(
        addresses(&url("http://127.0.0.1")).expect("an IPv4 address"),
        [address("127.0.0.1:80")]
    );
    let refused = addresses(&url("http://simulator.example:8080")).expect_err("a host name");
    assert_eq!(refused.kind(), io::ErrorKind::InvalidInput);
}

/// Reads as a TLS stream does: one read gives only a whole record of `size` bytes, however many
/// socket reads that takes, and gives up at the first socket read that would block, keeping the
/// bytes it has (OpenSSL's `SSL_read`).
struct Records<R> {
    inner: R,
    size: usize,
    record: Vec<u8>,
}

impl<R: Read> Read for Records<R> {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        while self.record.len() < self.size {
            let mut chunk = [0_u8; 64];
            let read = self.inner.read(&mut chunk)?;
            if read == 0 {
                return Ok(0);
            }
            self.record.extend_from_slice(&chunk[..read]);
        }
        let given = buffer.len().min(self.record.len());
        buffer[..given].copy_from_slice(&self.record[..given]);
        self.record.drain(..given);
        Ok(given)
    }
}

/// Catches: a deadline kept only per socket read, which a peer that sends a record a byte at a time,
/// each within the read's wait, outlasts for as long as it likes: one read of a 400-byte record at a
/// byte every 5 ms would take two seconds. It ends at the 300 ms deadline, as `WouldBlock`, which a
/// TLS stream takes for "try again" before its caller sees the deadline passed.
#[test]
fn a_read_made_of_many_socket_reads_ends_at_the_deadline() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("a listener");
    let port = listener.local_addr().expect("its address").port();
    thread::spawn(move || {
        let Ok((mut peer, _)) = listener.accept() else {
            return;
        };
        while peer.write_all(b"x").is_ok() {
            thread::sleep(Duration::from_millis(5));
        }
    });
    let tcp = TcpStream::connect(("127.0.0.1", port)).expect("a connection");
    let started = Instant::now();
    let bounded = Bounded::new(tcp, started + Duration::from_millis(300));
    let mut records = Records {
        inner: bounded,
        size: 400,
        record: Vec::new(),
    };
    let error = records
        .read(&mut [0_u8; 16])
        .expect_err("no whole record arrives in time");
    let took = started.elapsed();
    assert_eq!(error.kind(), io::ErrorKind::WouldBlock);
    assert!(
        took < Duration::from_millis(300) + SLICE * 3,
        "ended after {took:?}"
    );
    assert!(
        records.record.len() >= 2,
        "one read took several socket reads"
    );
}

/// Catches: a socket wait that ran out on Windows, which reports it as `TimedOut`, taken by a TLS
/// stream for a failure of the connection instead of "try again".
#[test]
fn a_wait_that_ran_out_is_would_block_on_every_system() {
    let timed_out: io::Result<()> = Err(io::ErrorKind::TimedOut.into());
    assert_eq!(
        out_of_time(timed_out).expect_err("out of time").kind(),
        io::ErrorKind::WouldBlock
    );
    let reset: io::Result<()> = Err(io::ErrorKind::ConnectionReset.into());
    assert_eq!(
        out_of_time(reset).expect_err("a failure").kind(),
        io::ErrorKind::ConnectionReset
    );
}
