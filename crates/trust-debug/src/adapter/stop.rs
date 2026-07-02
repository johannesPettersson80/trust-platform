//! Stop event coordination (ordering + filtering).

use std::io::BufWriter;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{mpsc::Receiver, Arc, Mutex};
use std::thread::{self, JoinHandle};

use trust_runtime::debug::{DebugControl, DebugStop, DebugStopReason};

use crate::protocol::{Event, InvalidatedEventBody, MessageType, StoppedEventBody};

use super::protocol_io::write_protocol_log;
use super::StopGate;

/// Coordinates stop ordering + filtering.
pub struct StopCoordinator {
    stop_gate: StopGate,
    pause_expected: Arc<AtomicBool>,
    stop_control: DebugControl,
    writer: Arc<Mutex<BufWriter<std::io::Stdout>>>,
    logger: Option<Arc<Mutex<BufWriter<std::fs::File>>>>,
    seq: Arc<AtomicU32>,
}

impl StopCoordinator {
    pub fn new(
        stop_gate: StopGate,
        pause_expected: Arc<AtomicBool>,
        stop_control: DebugControl,
        writer: Arc<Mutex<BufWriter<std::io::Stdout>>>,
        logger: Option<Arc<Mutex<BufWriter<std::fs::File>>>>,
        seq: Arc<AtomicU32>,
    ) -> Self {
        Self {
            stop_gate,
            pause_expected,
            stop_control,
            writer,
            logger,
            seq,
        }
    }

    pub fn spawn(self, stop_rx: Receiver<DebugStop>) -> JoinHandle<()> {
        thread::spawn(move || {
            while let Ok(stop) = stop_rx.recv() {
                self.trace_stop("recv", &stop, None);
                self.stop_gate.wait_clear();
                if !self.should_emit_stop(&stop) {
                    continue;
                }
                if !self.emit_stop(stop) {
                    break;
                }
            }
        })
    }

    fn should_emit_stop(&self, stop: &DebugStop) -> bool {
        match stop.reason {
            DebugStopReason::Pause | DebugStopReason::Entry => {
                if !self.pause_expected.swap(false, Ordering::SeqCst) {
                    self.trace_stop(
                        "drop",
                        stop,
                        Some("pause/entry without pause_expected".to_string()),
                    );
                    return false;
                }
            }
            DebugStopReason::Breakpoint | DebugStopReason::Step => {
                self.pause_expected.store(false, Ordering::SeqCst);
            }
        }
        if matches!(stop.reason, DebugStopReason::Breakpoint) {
            let Some(location) = stop.location else {
                self.trace_stop(
                    "drop",
                    stop,
                    Some("breakpoint without location".to_string()),
                );
                return false;
            };
            let Some(generation) = stop.breakpoint_generation else {
                self.trace_stop(
                    "drop",
                    stop,
                    Some("breakpoint without generation".to_string()),
                );
                return false;
            };
            let current = self.stop_control.breakpoint_generation(location.file_id);
            if current != Some(generation) {
                self.trace_stop(
                    "drop",
                    stop,
                    Some(format!(
                        "breakpoint generation mismatch file_id={} current={:?} stop={}",
                        location.file_id, current, generation
                    )),
                );
                return false;
            }
        }
        self.trace_stop("emit", stop, None);
        true
    }

    fn emit_stop(&self, stop: DebugStop) -> bool {
        let reason = match stop.reason {
            DebugStopReason::Breakpoint => "breakpoint",
            DebugStopReason::Step => "step",
            DebugStopReason::Pause => "pause",
            DebugStopReason::Entry => "entry",
        };
        let thread_id = stop.thread_id.or(Some(1));
        let telemetry_body = serde_json::json!({
            "message": format!(
                "[trust-debug] stopped: reason={} thread_id={}\n",
                reason,
                thread_id
                    .map(|id| id.to_string())
                    .unwrap_or_else(|| "<none>".to_string())
            )
        });
        let output_event = Event {
            seq: self.seq.fetch_add(1, Ordering::Relaxed),
            message_type: MessageType::Event,
            event: "trustDebugInternal".to_string(),
            body: Some(telemetry_body),
        };
        let all_threads_stopped = self.stop_control.target_thread().is_none();
        let body = StoppedEventBody {
            reason: reason.to_string(),
            thread_id,
            all_threads_stopped: Some(all_threads_stopped),
        };
        let event = Event {
            seq: self.seq.fetch_add(1, Ordering::Relaxed),
            message_type: MessageType::Event,
            event: "stopped".to_string(),
            body: Some(body),
        };
        let output_serialized = match serde_json::to_string(&output_event) {
            Ok(serialized) => serialized,
            Err(_) => return true,
        };
        let serialized = match serde_json::to_string(&event) {
            Ok(serialized) => serialized,
            Err(_) => return true,
        };
        if let Some(logger) = &self.logger {
            let _ = write_protocol_log(logger, "->", &output_serialized);
            let _ = write_protocol_log(logger, "->", &serialized);
        }
        if super::protocol_io::write_message_locked(&self.writer, &output_serialized).is_err() {
            return false;
        }
        if super::protocol_io::write_message_locked(&self.writer, &serialized).is_err() {
            return false;
        }
        if self.stop_control.take_watch_changed() {
            let body = InvalidatedEventBody {
                areas: Some(vec!["watch".to_string()]),
                thread_id,
                stack_frame_id: None,
            };
            let event = Event {
                seq: self.seq.fetch_add(1, Ordering::Relaxed),
                message_type: MessageType::Event,
                event: "invalidated".to_string(),
                body: Some(body),
            };
            let serialized = match serde_json::to_string(&event) {
                Ok(serialized) => serialized,
                Err(_) => return true,
            };
            if let Some(logger) = &self.logger {
                let _ = write_protocol_log(logger, "->", &serialized);
            }
            if super::protocol_io::write_message_locked(&self.writer, &serialized).is_err() {
                return false;
            }
        }
        true
    }

    fn trace_stop(&self, action: &str, stop: &DebugStop, detail: Option<String>) {
        let reason = match stop.reason {
            DebugStopReason::Breakpoint => "breakpoint",
            DebugStopReason::Step => "step",
            DebugStopReason::Pause => "pause",
            DebugStopReason::Entry => "entry",
        };
        let location = stop
            .location
            .map(|loc| format!("{}:{}..{}", loc.file_id, loc.start, loc.end))
            .unwrap_or_else(|| "<none>".to_string());
        let detail = detail.unwrap_or_default();
        if let Some(logger) = &self.logger {
            let _ = write_protocol_log(
                logger,
                "##",
                &format!(
                    "[trust-debug][stop] action={} reason={} thread={:?} bp_gen={:?} location={} detail={}",
                    action, reason, stop.thread_id, stop.breakpoint_generation, location, detail
                ),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::BufWriter;
    use std::sync::Arc;

    use trust_runtime::debug::{DebugBreakpoint, SourceLocation};

    fn test_coordinator(control: DebugControl) -> StopCoordinator {
        StopCoordinator::new(
            StopGate::new(),
            Arc::new(AtomicBool::new(false)),
            control,
            Arc::new(Mutex::new(BufWriter::new(std::io::stdout()))),
            None,
            Arc::new(AtomicU32::new(1)),
        )
    }

    #[test]
    fn breakpoint_stop_is_emitted_without_pause_expected_when_generation_matches() {
        let control = DebugControl::new();
        let location = SourceLocation::new(0, 21, 42);
        control.set_breakpoints_for_file(0, vec![DebugBreakpoint::new(location)]);
        let generation = control
            .breakpoint_generation(0)
            .expect("breakpoint generation");
        let coordinator = test_coordinator(control);
        let stop = DebugStop {
            reason: DebugStopReason::Breakpoint,
            location: Some(location),
            thread_id: Some(1),
            breakpoint_generation: Some(generation),
        };
        assert!(coordinator.should_emit_stop(&stop));
    }

    #[test]
    fn breakpoint_stop_is_dropped_when_generation_mismatches() {
        let control = DebugControl::new();
        let location = SourceLocation::new(0, 21, 42);
        control.set_breakpoints_for_file(0, vec![DebugBreakpoint::new(location)]);
        let coordinator = test_coordinator(control);
        let stop = DebugStop {
            reason: DebugStopReason::Breakpoint,
            location: Some(location),
            thread_id: Some(1),
            breakpoint_generation: Some(999),
        };
        assert!(!coordinator.should_emit_stop(&stop));
    }
}
