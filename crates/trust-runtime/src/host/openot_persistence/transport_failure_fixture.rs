//! Real unavailable transport for adapter and service lifecycle tests.

use std::io::{self, Read};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc};
use std::thread::{self, JoinHandle};
use std::time::Duration;

pub(super) struct TransportFailureEndpoint {
    address: SocketAddr,
    first_request: Option<mpsc::Receiver<TcpStream>>,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<io::Result<()>>>,
}

impl TransportFailureEndpoint {
    pub(super) fn new() -> io::Result<Self> {
        let listener = TcpListener::bind("127.0.0.1:0")?;
        let address = listener.local_addr()?;
        listener.set_nonblocking(true)?;
        let stop = Arc::new(AtomicBool::new(false));
        let thread_stop = Arc::clone(&stop);
        let (sender, receiver) = mpsc::channel();
        let thread = thread::spawn(move || {
            let mut observed_first_request = false;
            while !thread_stop.load(Ordering::Acquire) {
                let mut stream = match listener.accept() {
                    Ok((stream, _)) => stream,
                    Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(1));
                        continue;
                    }
                    Err(error) => return Err(error),
                };
                // Accepted sockets inherit nonblocking mode on Windows.
                stream.set_nonblocking(false)?;
                if observed_first_request {
                    // Deliberately disconnect: this peer sends no HTTP/TLS response.
                    continue;
                }
                stream.set_read_timeout(Some(Duration::from_millis(50)))?;
                let mut byte = [0];
                while !thread_stop.load(Ordering::Acquire) {
                    match stream.read(&mut byte) {
                        Ok(0) => break,
                        Ok(_) => {
                            // A TCP accept alone precedes ureq's lazy TLS preparation.
                            // Transfer ownership after actual request/ClientHello bytes.
                            if sender.send(stream).is_err() {
                                return Ok(());
                            }
                            observed_first_request = true;
                            break;
                        }
                        Err(error)
                            if matches!(
                                error.kind(),
                                io::ErrorKind::WouldBlock
                                    | io::ErrorKind::TimedOut
                                    | io::ErrorKind::Interrupted
                            ) => {}
                        Err(error) => return Err(error),
                    }
                }
            }
            Ok(())
        });
        Ok(Self {
            address,
            first_request: Some(receiver),
            stop,
            thread: Some(thread),
        })
    }

    pub(super) fn address(&self) -> SocketAddr {
        self.address
    }

    pub(super) fn wait_for_request(&self) -> TcpStream {
        // Harness completion bound, separate from the unchanged retry assertion window.
        self.first_request
            .as_ref()
            .expect("live transport fixture")
            .recv_timeout(Duration::from_secs(10))
            .expect("client must send actual request bytes to the owned endpoint")
    }
}

impl Drop for TransportFailureEndpoint {
    fn drop(&mut self) {
        // Closing the receiver also drops any transferred but unobserved connection.
        self.first_request.take();
        self.stop.store(true, Ordering::Release);
        if let Some(thread) = self.thread.take() {
            let result = thread.join();
            if !thread::panicking() {
                result
                    .expect("join unavailable endpoint")
                    .expect("unavailable endpoint transport setup");
            }
        }
    }
}
