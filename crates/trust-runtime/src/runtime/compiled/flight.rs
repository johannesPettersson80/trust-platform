//! The flight recorder on a computer (specification 48 RU-59, "The flight recorder"; specification
//! 11 §6.8.1, "Cycle traces"): armed when the resource starts, restarts or changes its program,
//! fed every cycle beside a recording on demand, and its fault's trace kept in the slot that the
//! control link reads until the next fault, served part by part as every runtime serves it.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use smol_str::SmolStr;
use trust_runtime_core::native::variables::CompiledVariables;
use trust_runtime_core::trace::{ChangeSource, ImageTables, Located};
use trust_runtime_services::events::RuntimeEvent;
use trust_runtime_services::trace::session::{Device, SessionIdentity};
use trust_runtime_services::trace::{Budget, FlightRecorder, ReserveError, ServedTrace};

use super::trace::{lock, trace_file_name, utc_ms, SharedTraces};
use super::CompiledRuntime;

/// The flight recorder a runtime keeps (`[runtime.trace] flight_recorder`): the cycles `n` of
/// which it keeps the last `n` to `2n`, the slot its fault's trace goes to, and the device's
/// identity as `status` gives it.
#[derive(Debug, Clone)]
pub struct FlightSettings {
    /// The cycles `n`; 0 keeps no flight recorder.
    pub cycles: u64,
    /// The slot that the control link reads the fault's trace from.
    pub traces: SharedTraces,
    /// The device's identity (`runtime_id`).
    pub runtime_id: SmolStr,
    /// The folder of the runtime's files (specification 11 §6.8.0a), where the fault's trace is
    /// written too, as `traces/fault-<UTC date and time>.trusttrace`; none for a runtime without
    /// one, such as a simulator.
    pub files: Option<PathBuf>,
}

/// The flight recorder of the program that runs (module docs).
pub(crate) struct FlightSession {
    pub(super) recorder: FlightRecorder,
    identity: SessionIdentity,
    located: Vec<Located>,
    tables: ImageTables,
    /// The first recording's first cycle has begun: its start state is taken at the cycle's first
    /// access, as a recording on demand's is.
    pub(super) start_pending: bool,
    /// A recording handed over at the end of the scan: the next one's start state is taken once
    /// the scan has returned, before any change between the scans.
    pub(super) handover_pending: bool,
    /// The debugger held the scan, which no replay could run: the flight recorder starts again
    /// once the scan has returned.
    pub(super) rearm_pending: bool,
}

impl CompiledRuntime {
    /// Keeps the flight recorder of `settings`, or none, and arms it now for the program installed
    /// (specification 48 RU-59). A restart and an online change arm it again.
    pub fn set_flight_recorder(&mut self, settings: Option<FlightSettings>) {
        self.flight_settings = settings.filter(|settings| settings.cycles > 0);
        self.arm_flight();
    }

    /// Arms the flight recorder of the settings for the program installed, its memory reserved
    /// now, between two scans. Memory the computer cannot give leaves the resource without a
    /// flight recorder, and a `runtime_trace_memory` event names the bytes needed (specification
    /// 11 §6.8.1). A fault's trace already kept stays until the next fault.
    pub(crate) fn arm_flight(&mut self) {
        self.flight = None;
        let Some(settings) = &self.flight_settings else {
            return;
        };
        let armed = {
            let image = self.image.image();
            let program = self.traced_program(&image);
            FlightRecorder::arm(settings.cycles, program.sizes(None), Budget::COMPUTER).and_then(
                |mut recorder| {
                    // The fault's trace keeps its own copy of the image's tables, as a recording
                    // on demand does.
                    let tables =
                        program
                            .tables()
                            .ok_or_else(|| ReserveError::InsufficientMemory {
                                needed: recorder
                                    .reserved_bytes()
                                    .saturating_add(program.tables_len()),
                            })?;
                    let now = utc_ms();
                    recorder.began(now);
                    Ok(FlightSession {
                        recorder,
                        identity: SessionIdentity::of(
                            &image,
                            &Device {
                                runtime_id: &settings.runtime_id,
                                platform: crate::runtime_id::platform(),
                            },
                            self.resource_name.clone(),
                            None,
                            now,
                        ),
                        located: program.located(),
                        tables,
                        start_pending: false,
                        handover_pending: false,
                        rearm_pending: false,
                    })
                },
            )
        };
        match armed {
            Ok(session) => self.flight = Some(session),
            Err(ReserveError::InsufficientMemory { needed }) => {
                if let Some(debug) = &self.debug {
                    debug.push_runtime_event(RuntimeEvent::TraceMemory {
                        needed,
                        free: None,
                        time: self.current_time,
                    });
                }
            }
        }
    }

    /// After a scan: the start state of a recording that took over at the scan's end, the memory
    /// and the forces as the scan left them, taken before any change between the scans; then the
    /// fault's trace when the scan faulted the flight recorder.
    pub(super) fn after_scan_flight(&mut self) {
        let Some(session) = &mut self.flight else {
            return;
        };
        if session.rearm_pending {
            self.arm_flight();
            return;
        }
        if session.handover_pending {
            session.handover_pending = false;
            session.recorder.began(utc_ms());
            let program = self.cycle.program();
            let recorder = session.recorder.recorder();
            for id in 1..=6 {
                if let Some(area) = recorder.start_area(id) {
                    program.read_area_bytes(id, 0, area);
                }
            }
            if let Some(debug) = &self.debug {
                debug.with_forces(&mut |vars, io| {
                    recorder.start_forces(ChangeSource::ControlLink, vars, io, program);
                });
            }
        }
        self.publish_flight();
    }

    /// Writes the fault's trace `trace` into `folder` as `fault-<UTC date and time>.trusttrace`;
    /// a file it cannot write is reported as the event `runtime_trace_file_unavailable`, and the
    /// slot keeps the trace all the same (specification 11 §6.8.1).
    fn write_fault_trace(&self, folder: &Path, trace: &ServedTrace) {
        let path = folder.join(trace_file_name("fault", utc_ms().div_euclid(1_000)));
        let written = std::fs::create_dir_all(folder).and_then(|()| {
            write_parts(
                &path,
                &mut |offset, out| trace.part(offset, out),
                WRITE_PART_BYTES,
            )
        });
        if let (Err(error), Some(debug)) = (written, &self.debug) {
            debug.push_runtime_event(RuntimeEvent::TraceFileUnavailable {
                path: path.display().to_string(),
                error: error.to_string(),
                time: self.current_time,
            });
        }
    }

    /// The resource faulted outside a scan, as a tool's method call can make it: the flight
    /// recorder's recording ends with the cycles before, as the fault's trace.
    pub(crate) fn fault_flight(&mut self) {
        if let Some(session) = &mut self.flight {
            session.recorder.fault();
        }
        self.publish_flight();
    }

    /// Once the flight recorder has ended at a fault: its trace into the slot, where it stays
    /// until the next fault, its variables named as the debugger names them, written between two
    /// scans. The flight recorder ends with it; a restart arms it again.
    fn publish_flight(&mut self) {
        let Some(session) = &mut self.flight else {
            return;
        };
        if !session.recorder.faulted() {
            return;
        }
        let program = self.cycle.program();
        let started = session
            .recorder
            .fault_began_utc_ms()
            .unwrap_or(session.identity.started_utc_ms());
        let identity = session.identity.identity(started);
        let served = session.recorder.fault_served(
            &identity,
            &session.located,
            std::mem::take(&mut session.tables),
            &mut |path| program.display_path(path),
        );
        let (traces, files) = self
            .flight_settings
            .as_ref()
            .map_or((None, None), |settings| {
                (Some(Arc::clone(&settings.traces)), settings.files.clone())
            });
        self.flight = None;
        let (Some(served), Some(traces)) = (served, traces) else {
            return;
        };
        if let Some(folder) = files {
            self.write_fault_trace(&folder.join("traces"), &served);
        }
        lock(&traces).keep_fault(served);
    }
}

/// The bytes of the fault's trace that [`write_parts`] writes at a time.
const WRITE_PART_BYTES: usize = 64 * 1024;

/// Writes to `path` the file whose bytes `part` copies from an offset into a buffer, as
/// [`ServedTrace::part`] does, `part_bytes` at a time, until it copies none.
fn write_parts(
    path: &Path,
    part: &mut dyn FnMut(usize, &mut [u8]) -> usize,
    part_bytes: usize,
) -> std::io::Result<()> {
    let mut file = std::io::BufWriter::new(std::fs::File::create(path)?);
    let mut buffer = vec![0; part_bytes];
    let mut offset = 0;
    loop {
        let count = part(offset, &mut buffer);
        if count == 0 {
            break;
        }
        file.write_all(&buffer[..count])?;
        offset += count;
    }
    file.flush()
}

#[cfg(test)]
mod tests;
