//! Stop event coordination for remote attach sessions.

use std::collections::HashMap;
use std::fs::File;
use std::io::BufWriter;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use crate::protocol::{AdsStateEventBody, Event, IoStateEventBody, MessageType, StoppedEventBody};

use super::protocol_io::write_protocol_log;
use super::remote::{RemoteEndpoint, RemoteSession, RemoteStop};
use super::StopGate;

const POLL_INTERVAL: Duration = Duration::from_millis(50);
const LIVE_STATE_POLL_INTERVAL: Duration = Duration::from_millis(150);
const POLL_REQUEST_TIMEOUT: Duration = Duration::from_millis(250);

pub struct RemoteStopPoller {
    stop: Arc<AtomicBool>,
    handle: JoinHandle<()>,
}

pub struct RemoteStopPollerConfig {
    pub endpoint: RemoteEndpoint,
    pub token: Option<String>,
    pub stop_gate: StopGate,
    pub pause_expected: Arc<AtomicBool>,
    pub writer: Arc<Mutex<BufWriter<std::io::Stdout>>>,
    pub logger: Option<Arc<Mutex<BufWriter<File>>>>,
    pub seq: Arc<AtomicU32>,
    pub breakpoints: Arc<Mutex<HashMap<u32, u64>>>,
    pub io_state: Arc<Mutex<Option<IoStateEventBody>>>,
    pub ads_state: Arc<Mutex<Option<AdsStateEventBody>>>,
}

impl RemoteStopPoller {
    pub fn spawn(config: RemoteStopPollerConfig) -> Self {
        let stop = Arc::new(AtomicBool::new(false));
        let stop_flag = Arc::clone(&stop);
        let handle = thread::spawn(move || {
            let mut session = match RemoteSession::connect_polling(
                config.endpoint,
                config.token,
                POLL_REQUEST_TIMEOUT,
            ) {
                Ok(session) => session,
                Err(_) => return,
            };
            let mut last_live_poll = Instant::now() - LIVE_STATE_POLL_INTERVAL;
            while !stop_flag.load(Ordering::Relaxed) {
                if let Ok(stops) = session.debug_stops() {
                    for stop in stops {
                        config.stop_gate.wait_clear();
                        if !should_emit_stop(&stop, &config.pause_expected, &config.breakpoints) {
                            continue;
                        }
                        if !emit_stop_event(&stop, &config.writer, &config.logger, &config.seq) {
                            return;
                        }
                    }
                }
                if last_live_poll.elapsed() >= LIVE_STATE_POLL_INTERVAL {
                    let live_events =
                        poll_live_state(&mut session, &config.io_state, &config.ads_state);
                    for event in live_events {
                        if !emit_live_state_event(
                            event,
                            &config.writer,
                            &config.logger,
                            &config.seq,
                        ) {
                            return;
                        }
                    }
                    last_live_poll = Instant::now();
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

#[derive(Debug, Clone, PartialEq, Eq)]
enum RemoteLiveStateEvent {
    Io(IoStateEventBody),
    Ads(AdsStateEventBody),
}

fn poll_live_state(
    session: &mut RemoteSession,
    io_cache: &Arc<Mutex<Option<IoStateEventBody>>>,
    ads_cache: &Arc<Mutex<Option<AdsStateEventBody>>>,
) -> Vec<RemoteLiveStateEvent> {
    let mut events = Vec::with_capacity(2);
    if let Ok(state) = session.io_state() {
        if replace_if_changed(io_cache, &state) {
            events.push(RemoteLiveStateEvent::Io(state));
        }
    }
    if let Ok(state) = session.ads_live_values() {
        if replace_if_changed(ads_cache, &state) {
            events.push(RemoteLiveStateEvent::Ads(state));
        }
    }
    events
}

fn replace_if_changed<T>(cache: &Arc<Mutex<Option<T>>>, state: &T) -> bool
where
    T: Clone + PartialEq,
{
    let Ok(mut cache) = cache.lock() else {
        return false;
    };
    if cache.as_ref() == Some(state) {
        return false;
    }
    *cache = Some(state.clone());
    true
}

fn emit_live_state_event(
    event: RemoteLiveStateEvent,
    writer: &Arc<Mutex<BufWriter<std::io::Stdout>>>,
    logger: &Option<Arc<Mutex<BufWriter<File>>>>,
    seq: &Arc<AtomicU32>,
) -> bool {
    let serialized = match event {
        RemoteLiveStateEvent::Io(body) => serialize_event("stIoState", body, seq),
        RemoteLiveStateEvent::Ads(body) => serialize_event("stAdsState", body, seq),
    };
    let Some(serialized) = serialized else {
        return true;
    };
    if let Some(logger) = logger {
        let _ = write_protocol_log(logger, "->", &serialized);
    }
    super::protocol_io::write_message_locked(writer, &serialized).is_ok()
}

fn serialize_event<T>(name: &str, body: T, seq: &Arc<AtomicU32>) -> Option<String>
where
    T: serde::Serialize,
{
    serde_json::to_string(&Event {
        seq: seq.fetch_add(1, Ordering::Relaxed),
        message_type: MessageType::Event,
        event: name.to_string(),
        body: Some(body),
    })
    .ok()
}

fn should_emit_stop(
    stop: &RemoteStop,
    pause_expected: &Arc<AtomicBool>,
    breakpoints: &Arc<Mutex<HashMap<u32, u64>>>,
) -> bool {
    match stop.reason.as_str() {
        "pause" | "entry" if !pause_expected.swap(false, Ordering::SeqCst) => return false,
        "breakpoint" | "step" => {
            pause_expected.store(false, Ordering::SeqCst);
        }
        _ => {}
    }
    if stop.reason == "breakpoint" {
        if let (Some(file_id), Some(generation)) = (stop.file_id, stop.breakpoint_generation) {
            if let Ok(guard) = breakpoints.lock() {
                if guard.get(&file_id).copied() != Some(generation) {
                    return false;
                }
            }
        }
    }
    true
}

fn emit_stop_event(
    stop: &RemoteStop,
    writer: &Arc<Mutex<BufWriter<std::io::Stdout>>>,
    logger: &Option<Arc<Mutex<BufWriter<File>>>>,
    seq: &Arc<AtomicU32>,
) -> bool {
    let thread_id = stop.thread_id.or(Some(1));
    let telemetry_body = serde_json::json!({
        "message": format!(
            "[trust-debug] stopped: reason={} thread_id={}\n",
            stop.reason,
            thread_id
                .map(|id| id.to_string())
                .unwrap_or_else(|| "<none>".to_string())
        )
    });
    let output_event = Event {
        seq: seq.fetch_add(1, Ordering::Relaxed),
        message_type: MessageType::Event,
        event: "trustDebugInternal".to_string(),
        body: Some(telemetry_body),
    };
    let stopped_body = StoppedEventBody {
        reason: stop.reason.clone(),
        thread_id,
        all_threads_stopped: Some(true),
    };
    let stopped_event = Event {
        seq: seq.fetch_add(1, Ordering::Relaxed),
        message_type: MessageType::Event,
        event: "stopped".to_string(),
        body: Some(stopped_body),
    };
    let output_serialized = match serde_json::to_string(&output_event) {
        Ok(serialized) => serialized,
        Err(_) => return true,
    };
    let serialized = match serde_json::to_string(&stopped_event) {
        Ok(serialized) => serialized,
        Err(_) => return true,
    };
    if let Some(logger) = logger {
        let _ = write_protocol_log(logger, "->", &output_serialized);
        let _ = write_protocol_log(logger, "->", &serialized);
    }
    if super::protocol_io::write_message_locked(writer, &output_serialized).is_err() {
        return false;
    }
    if super::protocol_io::write_message_locked(writer, &serialized).is_err() {
        return false;
    }
    true
}

#[cfg(test)]
mod tests {
    use std::io::{BufRead, BufReader, Write};
    use std::net::TcpListener;

    use super::*;

    #[test]
    fn one_persistent_remote_poller_session_reads_io_and_ads_without_duplicates() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind live-state server");
        let address = listener.local_addr().expect("live-state address");
        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("poller connection");
            let mut reader = BufReader::new(stream.try_clone().expect("clone poller stream"));
            let mut request_types = Vec::new();
            for _ in 0..4 {
                let mut line = String::new();
                reader.read_line(&mut line).expect("read poll request");
                let request: serde_json::Value =
                    serde_json::from_str(&line).expect("poll request JSON");
                assert_eq!(request["auth"], "viewer-token");
                let request_type = request["type"].as_str().expect("request type");
                request_types.push(request_type.to_string());
                let result = match request_type {
                    "io.read" => serde_json::json!({
                        "snapshot": {"scan": 12, "inputs": [], "outputs": [], "memory": []}
                    }),
                    "ads.live_values" => serde_json::json!({
                        "schemaVersion": 1,
                        "scan": 12,
                        "entries": []
                    }),
                    other => panic!("unexpected poll request {other}"),
                };
                writeln!(
                    stream,
                    "{}",
                    serde_json::json!({"id": request["id"], "ok": true, "result": result})
                )
                .expect("write poll response");
                stream.flush().expect("flush poll response");
            }
            request_types
        });
        let mut session = RemoteSession::connect_polling(
            RemoteEndpoint::Tcp(address),
            Some("viewer-token".to_string()),
            POLL_REQUEST_TIMEOUT,
        )
        .expect("connect poller");
        let io_cache = Arc::new(Mutex::new(None));
        let ads_cache = Arc::new(Mutex::new(None));

        let first = poll_live_state(&mut session, &io_cache, &ads_cache);
        let second = poll_live_state(&mut session, &io_cache, &ads_cache);

        assert!(matches!(
            first.as_slice(),
            [RemoteLiveStateEvent::Io(_), RemoteLiveStateEvent::Ads(_)]
        ));
        assert!(
            second.is_empty(),
            "unchanged scans must not emit duplicates"
        );
        drop(session);
        assert_eq!(
            server.join().expect("poll server"),
            ["io.read", "ads.live_values", "io.read", "ads.live_values"]
        );
    }
}
