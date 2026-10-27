//! The process image of a running session, sent as it changes (specification 13, "I/O state";
//! specification 25 §4.4): the adapter reads the runtime's I/O on its own connection and sends
//! `stIoState` whenever the image differs from the one it sent last, at most every 150 ms, so that
//! Live Values and Control Studio follow the scans without asking for each one.

use std::fs::File;
use std::io::BufWriter;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use crate::protocol::{Event, IoStateEventBody, MessageType};

use super::protocol_io::write_protocol_log;
use super::remote::{RemoteEndpoint, RemoteSession};

/// How often the image is read: as often as the adapter sent it before compiled programs, so a
/// trend of a 100 ms task still shows most of its scans.
const POLL_INTERVAL: Duration = Duration::from_millis(150);

pub struct RemoteIoPoller {
    stop: Arc<AtomicBool>,
    handle: JoinHandle<()>,
}

pub struct RemoteIoPollerConfig {
    pub endpoint: RemoteEndpoint,
    pub token: Option<String>,
    pub writer: Arc<Mutex<BufWriter<std::io::Stdout>>>,
    pub logger: Option<Arc<Mutex<BufWriter<File>>>>,
    pub seq: Arc<AtomicU32>,
    /// The image the adapter sent last, which the `stIoState` request answers from too.
    pub last_io_state: Arc<Mutex<Option<IoStateEventBody>>>,
}

impl RemoteIoPoller {
    pub fn spawn(config: RemoteIoPollerConfig) -> Self {
        let stop = Arc::new(AtomicBool::new(false));
        let stop_flag = Arc::clone(&stop);
        let handle = thread::spawn(move || {
            let mut session =
                match RemoteSession::connect(config.endpoint.clone(), config.token.clone()) {
                    Ok(session) => session,
                    Err(_) => return,
                };
            while !stop_flag.load(Ordering::Relaxed) {
                if let Ok(body) = session.io_state()
                    && changed(&config.last_io_state, &body)
                    && !emit(&body, &config)
                {
                    return;
                }
                thread::sleep(POLL_INTERVAL);
            }
        });
        Self { stop, handle }
    }

    pub fn stop(self) {
        self.stop.store(true, Ordering::Relaxed);
        let _ = self.handle.join();
    }
}

/// Whether `body` differs from the image sent last; if it does it becomes the image sent last. A
/// paused program, whose scan and values stay as they are, sends nothing.
pub(super) fn changed(last: &Mutex<Option<IoStateEventBody>>, body: &IoStateEventBody) -> bool {
    let Ok(mut last) = last.lock() else {
        return false;
    };
    if last.as_ref() == Some(body) {
        return false;
    }
    *last = Some(body.clone());
    true
}

/// Writes `body` to the client as `stIoState`, its values as Live Values shows them; `false` when
/// the client is gone.
fn emit(body: &IoStateEventBody, config: &RemoteIoPollerConfig) -> bool {
    let event = Event {
        seq: config.seq.fetch_add(1, Ordering::Relaxed),
        message_type: MessageType::Event,
        event: "stIoState".to_string(),
        body: Some(super::io_values::display_io_state(body)),
    };
    let Ok(serialized) = serde_json::to_string(&event) else {
        return true;
    };
    if let Some(logger) = &config.logger {
        let _ = write_protocol_log(logger, "->", &serialized);
    }
    super::protocol_io::write_message_locked(&config.writer, &serialized).is_ok()
}

#[cfg(test)]
mod tests {
    use super::changed;
    use crate::protocol::IoStateEventBody;
    use std::sync::Mutex;

    fn image(scan: u64) -> IoStateEventBody {
        IoStateEventBody {
            scan: Some(scan),
            inputs: Vec::new(),
            outputs: Vec::new(),
            memory: Vec::new(),
        }
    }

    /// Specification 25 §4.4: the client is sent each new image once and nothing while the image
    /// stays as it is. Catches an adapter that sends the same image again at every read, or one
    /// that keeps the first image and never sends a newer scan.
    #[test]
    fn a_new_image_is_sent_once_and_an_unchanged_one_not_again() {
        let last = Mutex::new(None);
        assert!(changed(&last, &image(7)), "the first image");
        assert!(!changed(&last, &image(7)), "the same image again");
        assert!(changed(&last, &image(8)), "the next scan");
        assert_eq!(last.lock().unwrap().as_ref(), Some(&image(8)));
    }
}
