//! A cycle trace on a computer (specification 48 RU-59; specification 11 §6.8.1, "Cycle traces"):
//! the recording that the scan thread fills between the phases of each scan, and the traces that the
//! control link reads from the slot it shares with the scan thread. The recording, its identity and
//! the slot are those every runtime shares ([`session`]): a computer gives them the program that
//! runs, its device and the UTC time, and keeps the slot behind a lock, since its control link runs
//! on a thread of its own.

use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::{SystemTime, UNIX_EPOCH};

use smol_str::SmolStr;
use trust_plc_abi::FaultRecord;
use trust_runtime_core::io_address::IoAddress;
use trust_runtime_core::native::image::Image;
use trust_runtime_core::native::variables::CompiledVariables;
use trust_runtime_core::trace::{readings, ChangeRef, ChangeSource, CycleFault, TraceEnd};
use trust_runtime_core::value::{st_literal, Value};
use trust_runtime_services::method_call::MethodCall;
use trust_runtime_services::trace::requests::{
    RecordRequest, TraceName, TraceRefusal, TraceStarted, TraceState,
};
pub use trust_runtime_services::trace::session::TraceSlot;
use trust_runtime_services::trace::session::{self, Device, Program};
use trust_runtime_services::trace::{Budget, Recorder, Recordings};

use super::CompiledRuntime;
use crate::hmi::commands::write::path_text;
use crate::hmi::member_paths::MemberPath;

/// The slot the control link and the scan thread share.
pub type SharedTraces = Arc<Mutex<TraceSlot>>;

/// A new slot, which holds no trace.
#[must_use]
pub fn new_traces() -> SharedTraces {
    Arc::new(Mutex::new(TraceSlot::default()))
}

/// The slot, also after a thread that held it panicked: its fields are always whole.
pub fn lock(traces: &SharedTraces) -> MutexGuard<'_, TraceSlot> {
    traces.lock().unwrap_or_else(PoisonError::into_inner)
}

/// A recording that the scan thread fills (`trace.record`), and the slot it publishes to.
pub(crate) struct TraceSession {
    /// The recording, as every runtime keeps one.
    pub(super) shared: session::TraceSession,
    traces: SharedTraces,
}

impl CompiledRuntime {
    /// Starts a recording at the next cycle (`trace.record`): its goal from `request`, its memory
    /// reserved now, between two scans, for the cycle interval the scans run at. `device` is the
    /// runtime's identity and platform as `status` gives them.
    ///
    /// # Errors
    ///
    /// [`TraceRefusal::Busy`] while a recording runs, [`TraceRefusal::UnknownTask`] for a task the
    /// program lacks, and [`TraceRefusal::InsufficientMemory`] for memory the computer cannot give.
    pub(crate) fn start_trace(
        &mut self,
        request: &RecordRequest,
        device: (&SmolStr, &'static str),
        traces: &SharedTraces,
    ) -> Result<TraceStarted, TraceRefusal> {
        let (runtime_id, platform) = device;
        if let Some(trace) = &self.trace {
            return Err(TraceRefusal::Busy {
                running: trace.shared.id(),
            });
        }
        // A started recording replaces the last one's trace (specification 11, "Cycle traces").
        let (shared, started) = {
            let image = self.image.image();
            session::TraceSession::start(
                request,
                &self.traced_program(&image),
                &Device {
                    runtime_id,
                    platform,
                },
                Budget::COMPUTER,
                None,
                utc_ms(),
                &mut lock(traces),
            )?
        };
        self.trace = Some(TraceSession {
            shared,
            traces: Arc::clone(traces),
        });
        Ok(started)
    }

    /// The program that runs as a recording takes it: the image `image`, its located variables
    /// and the names the debugger gives them, the resource, and the cycle interval its scans run
    /// at.
    pub(super) fn traced_program<'a>(&'a self, image: &'a Image<'a>) -> Program<'a> {
        Program {
            image,
            bindings: &self.bindings,
            binding_names: &self.binding_names,
            resource: &self.resource_name,
            cycle_interval: self.cycle.program().cycle_interval(),
        }
    }

    /// Ends the recording `id` at once (`trace.stop`) and gives its state.
    pub(crate) fn stop_trace(&mut self, id: u64, traces: &SharedTraces) -> TraceState {
        if self
            .trace
            .as_ref()
            .is_some_and(|trace| trace.shared.id() == id)
        {
            self.end_trace(TraceEnd::Stopped);
        }
        lock(traces).state(TraceName::Id(id))
    }

    /// Ends the running recording, if one runs, because of `end`: the resource stopped, restarted
    /// or changed its program, or a recording was stopped.
    pub(crate) fn end_trace(&mut self, end: TraceEnd) {
        if let Some(trace) = &mut self.trace {
            trace.shared.recorder().finish(end);
            // A scan that did not finish, such as one that panicked, left the readings armed.
            readings::disarm();
        }
        self.publish_trace();
    }

    /// A method call that a tool made between two scans (`method.call`), which a running recording
    /// keeps as its next cycle's change (specification 48 RU-59); the trace's file names the
    /// instance as the debugger does.
    pub(crate) fn record_method_call(&mut self, call: &MethodCall) {
        recordings(&mut self.trace, &mut self.flight).each(|recorder| {
            recorder.method_called(ChangeSource::ControlLink, &call.instance, call);
        });
    }

    /// Whether a recording runs, on demand or the flight recorder's, which keeps the changes from
    /// outside (specification 48 RU-59): a writer names what it wrote for [`Self::record_written`]
    /// only then.
    pub(crate) fn records_changes(&self) -> bool {
        self.trace.is_some() || self.flight.is_some()
    }

    /// Writes that `source` made between two scans, such as a plant's feedback or the mesh's
    /// values, which a running recording keeps as its next cycle's changes in the order they were
    /// made (specification 48 RU-59): each write of `addresses` as the bytes it gave the process
    /// image, then each variable of `paths` as the bytes it then holds.
    pub(crate) fn record_written(
        &mut self,
        source: ChangeSource,
        addresses: &[(IoAddress, Value)],
        paths: &[&str],
    ) {
        let program = self.cycle.program();
        recordings(&mut self.trace, &mut self.flight).each(|recorder| {
            for (address, value) in addresses {
                recorder.address_written(source, address, value, program);
            }
            for path in paths {
                recorder.written(source, path, program);
            }
        });
    }

    /// The screens' writes made between two scans (specification 39 §18; specification 48
    /// RU-59): each a screen's change of the next cycle, as the bytes the variable then holds.
    pub(super) fn record_screen_writes(&mut self, written: &[MemberPath]) {
        let mut recordings = recordings(&mut self.trace, &mut self.flight);
        if recordings.is_empty() {
            return;
        }
        let program = self.cycle.program();
        for path in written {
            let path = path_text(path);
            recordings.each(|recorder| recorder.written(ChangeSource::Screen, &path, program));
        }
    }

    /// A screen's method call between two scans (specification 39 §21.2; specification 48 RU-59):
    /// a screen's change of the next cycle, each input as its Structured Text literal. The HMI
    /// passes only values that a literal spells; an input without one ends the recording
    /// `memory_full`, since a trace without that input would replay another call.
    pub(crate) fn record_hmi_method_call(
        &mut self,
        path: &MemberPath,
        interface: &str,
        method: &str,
        inputs: &[(SmolStr, Value)],
    ) {
        let mut recordings = recordings(&mut self.trace, &mut self.flight);
        if recordings.is_empty() {
            return;
        }
        let Some(literals) = inputs
            .iter()
            .map(|(_, value)| st_literal(value))
            .collect::<Option<Vec<_>>>()
        else {
            recordings.each(Recorder::change_lost);
            return;
        };
        let inputs: Vec<(&str, &str)> = inputs
            .iter()
            .zip(&literals)
            .map(|((name, _), literal)| (name.as_str(), literal.as_str()))
            .collect();
        let instance = path_text(path);
        recordings.each(|recorder| {
            recorder.change(
                ChangeSource::Screen,
                &ChangeRef::MethodCall {
                    instance: &instance,
                    interface,
                    method,
                    inputs: &inputs,
                },
            );
        });
    }

    /// After a scan: the recording's progress for the control link, or once it has ended its trace,
    /// which the scan thread builds between two scans and the slot serves part by part, each
    /// variable named as the debugger names it (specification 48 Appendix C, C.6).
    pub(super) fn publish_trace(&mut self) {
        let Some(trace) = &mut self.trace else {
            return;
        };
        let program = self.cycle.program();
        let ended = trace.shared.publish(&mut lock(&trace.traces), &mut |path| {
            program.display_path(path)
        });
        if ended {
            // The session ends, and what its recording held with it.
            self.trace = None;
        }
    }
}

/// The fault record a trace keeps of a faulted scan of the program of `image`: the kernel's record
/// `record` of the fault the program raised ([`session::cycle_fault`]).
pub(super) fn cycle_fault(image: &Image<'_>, record: &FaultRecord) -> CycleFault {
    session::cycle_fault(image, record.code, record.stmt)
}

/// The name of a trace's file written at the UTC time `utc_secs`: `<prefix>-<UTC date and
/// time>.trusttrace`, the date and time in the basic form of ISO 8601, as
/// `trace-20261124T013000Z.trusttrace` (specification 11 §6.8.1, "Cycle traces"): `trace` for a
/// recording that `ctl record` saves, `fault` for a fault's trace.
#[must_use]
pub fn trace_file_name(prefix: &str, utc_secs: i64) -> String {
    let at = time::OffsetDateTime::from_unix_timestamp(utc_secs)
        .unwrap_or(time::OffsetDateTime::UNIX_EPOCH);
    format!(
        "{prefix}-{:04}{:02}{:02}T{:02}{:02}{:02}Z.trusttrace",
        at.year(),
        u8::from(at.month()),
        at.day(),
        at.hour(),
        at.minute(),
        at.second()
    )
}

/// The recordings that a change made now feeds: the recording on demand of `trace` and the
/// flight recorder's current recording of `flight`, each when there is one.
pub(super) fn recordings<'a>(
    trace: &'a mut Option<TraceSession>,
    flight: &'a mut Option<super::flight::FlightSession>,
) -> Recordings<'a> {
    Recordings {
        on_demand: trace.as_mut().map(|trace| trace.shared.recorder()),
        flight: flight.as_mut().map(|session| session.recorder.recorder()),
    }
}

pub(super) fn utc_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .and_then(|since| i64::try_from(since.as_millis()).ok())
        .unwrap_or(0)
}

/// A recording of `runtime`'s next `cycles` cycles, started for a test, and the slot it publishes to.
#[cfg(test)]
pub(crate) fn record_cycles(runtime: &mut CompiledRuntime, cycles: u64) -> SharedTraces {
    let traces = new_traces();
    let request = RecordRequest {
        until: trust_runtime_services::trace::requests::Until::Cycles(cycles),
        task: None,
    };
    runtime
        .start_trace(&request, (&SmolStr::new("test"), "test"), &traces)
        .expect("the recording starts");
    traces
}

/// The trace that the recording `record_cycles` started has ended with, read back as a reader reads
/// its file.
#[cfg(test)]
pub(crate) fn ended_trace(traces: &SharedTraces) -> trust_runtime_core::trace::Trace {
    let slot = lock(traces);
    let TraceState::Ended { bytes, .. } = slot.state(TraceName::Id(1)) else {
        panic!("the recording has ended");
    };
    let mut file = vec![0; bytes];
    assert_eq!(slot.part(TraceName::Id(1), 0, &mut file), Some(bytes));
    trust_runtime_core::trace::decode(&file)
        .expect("a trace")
        .trace
}

#[cfg(test)]
mod tests;
