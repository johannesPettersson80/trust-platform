//! Response withholding for the causal worker-handoff fixture only.

use std::net::{SocketAddr, TcpListener};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration as StdDuration;

use super::modbus_support::{handle_modbus_request_before_response, ModbusTestState};

/// A real peer that reads the request but cannot reply until the test releases it.
pub struct GatedModbusServer {
    pub addr: SocketAddr,
    received: std::sync::mpsc::Receiver<()>,
    release: Option<std::sync::mpsc::Sender<()>>,
    worker: Option<thread::JoinHandle<()>>,
}

impl GatedModbusServer {
    pub fn wait_for_request(&self, timeout: StdDuration) -> Result<(), String> {
        self.received
            .recv_timeout(timeout)
            .map_err(|error| format!("peer did not receive request: {error}"))
    }
}

impl Drop for GatedModbusServer {
    fn drop(&mut self) {
        if let Some(release) = self.release.take() {
            let _ = release.send(());
        }
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

pub fn start_gated_modbus_server(state: Arc<Mutex<ModbusTestState>>) -> GatedModbusServer {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind gated modbus server");
    let addr = listener.local_addr().unwrap();
    listener.set_nonblocking(true).unwrap();
    let (received_tx, received) = std::sync::mpsc::channel();
    let (release, release_rx) = std::sync::mpsc::channel();
    let worker = thread::spawn(move || {
        let deadline = std::time::Instant::now() + StdDuration::from_secs(5);
        let mut stream = loop {
            match listener.accept() {
                Ok((stream, _)) => break stream,
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    if std::time::Instant::now() >= deadline {
                        return;
                    }
                    thread::sleep(StdDuration::from_millis(1));
                }
                Err(_) => return,
            }
        };
        stream.set_nonblocking(false).unwrap();
        stream
            .set_read_timeout(Some(StdDuration::from_secs(2)))
            .unwrap();
        stream
            .set_write_timeout(Some(StdDuration::from_secs(2)))
            .unwrap();
        let _ = handle_modbus_request_before_response(&mut stream, &state, || {
            received_tx.send(()).map_err(|_| ())?;
            release_rx.recv().map_err(|_| ())
        });
    });
    GatedModbusServer {
        addr,
        received,
        release: Some(release),
        worker: Some(worker),
    }
}
