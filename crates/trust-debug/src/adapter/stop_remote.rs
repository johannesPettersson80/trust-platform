//! Stop event coordination for remote attach sessions.

use std::collections::HashMap;
use std::fs::File;
use std::io::BufWriter;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use crate::protocol::{Event, MessageType, OutputEventBody, Source, StoppedEventBody};

use super::protocol_io::write_protocol_log;
use super::remote::{client_source_path, RemoteEndpoint, RemoteLog, RemoteSession, RemoteStop};
use super::{CoordinateConverter, StopGate};

const POLL_INTERVAL: Duration = Duration::from_millis(50);

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
    /// The client's line and column base, for the positions of logpoint messages.
    pub(super) coordinate: CoordinateConverter,
    /// The folder the program's file labels resolve against, for a logpoint's source.
    pub label_root: Option<PathBuf>,
}

impl RemoteStopPoller {
    pub fn spawn(config: RemoteStopPollerConfig) -> Self {
        let stop = Arc::new(AtomicBool::new(false));
        let stop_flag = Arc::clone(&stop);
        let handle = thread::spawn(move || {
            let mut session =
                match RemoteSession::connect(config.endpoint.clone(), config.token.clone()) {
                    Ok(session) => session,
                    Err(_) => return,
                };
            while !stop_flag.load(Ordering::Relaxed) {
                if let Ok((stops, logs)) = session.debug_stops() {
                    // A logpoint's messages, which stop nothing (specification 11 §6.13.2).
                    for log in logs {
                        if !emit_log_event(&log, &config) {
                            return;
                        }
                    }
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
    if stop.reason == "breakpoint"
        && let (Some(file_id), Some(generation)) = (stop.file_id, stop.breakpoint_generation)
        && let Ok(guard) = breakpoints.lock()
        && guard.get(&file_id).copied() != Some(generation)
    {
        return false;
    }
    true
}

/// Writes the logpoint message `log` to the client as a `console` output event at the logpoint's
/// source and position; `false` when the client is gone.
fn emit_log_event(log: &RemoteLog, config: &RemoteStopPollerConfig) -> bool {
    let output = if log.text.ends_with('\n') {
        log.text.clone()
    } else {
        format!("{}\n", log.text)
    };
    let body = OutputEventBody {
        output,
        category: Some("console".to_string()),
        source: log.path.as_ref().map(|path| Source {
            name: None,
            path: Some(client_source_path(
                config.label_root.as_deref(),
                path.clone(),
            )),
            source_reference: None,
        }),
        line: log.line.map(|line| config.coordinate.to_client_line(line)),
        column: log
            .column
            .map(|column| config.coordinate.to_client_column(column)),
    };
    let event = Event {
        seq: config.seq.fetch_add(1, Ordering::Relaxed),
        message_type: MessageType::Event,
        event: "output".to_string(),
        body: Some(body),
    };
    let Ok(serialized) = serde_json::to_string(&event) else {
        return true;
    };
    if let Some(logger) = &config.logger {
        let _ = write_protocol_log(logger, "->", &serialized);
    }
    super::protocol_io::write_message_locked(&config.writer, &serialized).is_ok()
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
