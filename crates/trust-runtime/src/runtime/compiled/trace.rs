//! A cycle trace on a computer (specification 48 RU-59; specification 11 §6.8.1, "Cycle traces"):
//! the recording that the scan thread fills between the phases of each scan, and the traces that the
//! control link reads from the slot it shares with the scan thread.

use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::{SystemTime, UNIX_EPOCH};

use smol_str::SmolStr;
use trust_plc_abi::FaultRecord;
use trust_runtime_core::deployment::sha256;
use trust_runtime_core::io_image::PreparedIoBinding;
use trust_runtime_core::native::image::{Image, SectionId, StmtRecord, SymbolRecord, TaskRecord};
use trust_runtime_core::native::memory::type_name;
use trust_runtime_core::trace::{
    readings, ChangeSource, CycleFault, ImageTables, Located, TraceEnd,
};
use trust_runtime_core::value::Duration;
use trust_runtime_services::method_call::MethodCall;
use trust_runtime_services::trace::requests::{
    RecordRequest, TraceName, TraceRefusal, TraceStarted, TraceState, Until,
};
use trust_runtime_services::trace::{
    address_size, trace_area, trace_bit, Budget, Goal, Identity, Recorder, ReserveError, Sizes,
};

use super::CompiledRuntime;

/// The traces the control link reads: the recording that runs, and the last that ended.
#[derive(Debug, Default)]
pub struct TraceSlot {
    last_id: u64,
    running: Option<(u64, u64)>,
    ended: Option<EndedTrace>,
}

#[derive(Debug)]
struct EndedTrace {
    id: u64,
    end: TraceEnd,
    cycles: u64,
    file: Vec<u8>,
    sha256: [u8; 32],
}

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

impl TraceSlot {
    /// The state of the trace `name` (`trace.read`). The flight recorder's trace comes with the
    /// flight recorder: until then no fault has given one.
    #[must_use]
    pub fn state(&self, name: TraceName) -> TraceState {
        let TraceName::Id(id) = name else {
            return TraceState::None;
        };
        if let Some((running, cycles)) = self.running
            && running == id
        {
            return TraceState::Recording { cycles };
        }
        match &self.ended {
            Some(ended) if ended.id == id => TraceState::Ended {
                reason: ended.end,
                cycles: ended.cycles,
                bytes: ended.file.len(),
                sha256: ended.sha256,
            },
            _ => TraceState::Unknown,
        }
    }

    /// Copies the ended trace `name`'s file from `offset` into `out`, as many bytes as `out` holds
    /// and the file has, and gives how many.
    #[must_use]
    pub fn part(&self, name: TraceName, offset: usize, out: &mut [u8]) -> Option<usize> {
        let TraceName::Id(id) = name else {
            return None;
        };
        let ended = self.ended.as_ref().filter(|ended| ended.id == id)?;
        let rest = ended.file.get(offset..)?;
        let count = rest.len().min(out.len());
        out[..count].copy_from_slice(&rest[..count]);
        Some(count)
    }
}

/// A recording that the scan thread fills (`trace.record`).
pub(crate) struct TraceSession {
    id: u64,
    pub(super) recorder: Recorder,
    traces: SharedTraces,
    identity: SessionIdentity,
    located: Vec<Located>,
    tables: ImageTables,
    /// The trace's first cycle has begun: its start state is taken at the cycle's first access.
    pub(super) start_pending: bool,
}

struct SessionIdentity {
    image_sha256: [u8; 32],
    sources_hash: [u8; 32],
    target: u32,
    runtime_id: SmolStr,
    platform: &'static str,
    resource: SmolStr,
    task: Option<SmolStr>,
    started_utc_ms: i64,
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
        if let Some(session) = &self.trace {
            return Err(TraceRefusal::Busy {
                running: session.id,
            });
        }
        let image = self.image.image();
        let task = match &request.task {
            None => None,
            Some(name) => Some(
                task_by_name(&image, name)
                    .ok_or_else(|| TraceRefusal::UnknownTask(name.clone()))?,
            ),
        };
        let goal = match (request.until, task) {
            (Until::Cycles(runs), Some((task, _))) => Goal::TaskRuns { task, runs },
            (Until::Cycles(count), None) => Goal::Cycles(count),
            (Until::Duration(duration), _) => Goal::Duration(duration),
        };
        let sizes = Sizes {
            areas: core::array::from_fn(|index| {
                let id = u8::try_from(index + 1).unwrap_or(0);
                image
                    .area_length(id)
                    .and_then(|len| usize::try_from(len).ok())
                    .unwrap_or(0)
            }),
            tasks: image
                .table::<TaskRecord>()
                .map_or(1, |tasks| tasks.len() as usize),
            cycle_interval: self.cycle.program().cycle_interval(),
            task_interval: task.map(|(_, interval)| interval),
        };
        let recorder = Recorder::arm(goal, sizes, Budget::COMPUTER).map_err(
            |ReserveError::InsufficientMemory { needed }| TraceRefusal::InsufficientMemory {
                needed,
                free: None,
            },
        )?;
        let reserved_bytes = recorder.reserved_bytes();
        // A started recording replaces the last one's trace (specification 11, "Cycle traces").
        let id = {
            let mut slot = lock(traces);
            slot.last_id += 1;
            slot.running = Some((slot.last_id, 0));
            slot.ended = None;
            slot.last_id
        };
        let header = image.header();
        self.trace = Some(TraceSession {
            id,
            recorder,
            traces: Arc::clone(traces),
            identity: SessionIdentity {
                image_sha256: sha256(image.bytes()),
                sources_hash: header.source_hash,
                target: header.target,
                runtime_id: runtime_id.clone(),
                platform,
                resource: self.resource_name.clone(),
                task: request.task.as_deref().map(SmolStr::new),
                started_utc_ms: utc_ms(),
            },
            located: located(&image, &self.bindings, &self.binding_names),
            tables: tables(&image),
            start_pending: false,
        });
        Ok(TraceStarted {
            trace: id,
            reserved_bytes,
        })
    }

    /// Ends the recording `id` at once (`trace.stop`) and gives its state.
    pub(crate) fn stop_trace(&mut self, id: u64, traces: &SharedTraces) -> TraceState {
        if self.trace.as_ref().is_some_and(|session| session.id == id) {
            self.end_trace(TraceEnd::Stopped);
        }
        lock(traces).state(TraceName::Id(id))
    }

    /// Ends the running recording, if one runs, because of `end`: the resource stopped, restarted
    /// or changed its program, or a recording was stopped.
    pub(crate) fn end_trace(&mut self, end: TraceEnd) {
        if let Some(session) = &mut self.trace {
            session.recorder.finish(end);
            // A scan that did not finish, such as one that panicked, left the readings armed.
            readings::disarm();
        }
        self.publish_trace();
    }

    /// A method call that a tool made between two scans (`method.call`), which a running recording
    /// keeps as its next cycle's change (specification 48 RU-59): the instance by its symbol's
    /// display name, or as the request named it when no symbol names it, such as an element.
    pub(crate) fn record_method_call(&mut self, call: &MethodCall) {
        let Some(session) = &mut self.trace else {
            return;
        };
        let image = self.image.image();
        let instance = image
            .find_symbol(&call.instance.to_ascii_uppercase())
            .and_then(|(_, symbol)| image.string(symbol.display))
            .unwrap_or(&call.instance);
        session
            .recorder
            .method_called(ChangeSource::ControlLink, instance, call);
    }

    /// After a scan: the recording's progress for the control link, or once it has ended its file,
    /// which the scan thread builds between two scans.
    pub(super) fn publish_trace(&mut self) {
        let Some(session) = &self.trace else {
            return;
        };
        let identity = &session.identity;
        let file = session.recorder.file(
            &Identity {
                image_sha256: identity.image_sha256,
                sources_hash: identity.sources_hash,
                target: identity.target,
                runtime_version: env!("CARGO_PKG_VERSION"),
                runtime_id: &identity.runtime_id,
                platform: identity.platform,
                resource: &identity.resource,
                task: identity.task.as_deref(),
                started_utc_ms: identity.started_utc_ms,
            },
            &session.located,
            &session.tables,
        );
        // A recording has its file once it has ended, and only then.
        let (Some(file), Some(end)) = (file, session.recorder.end()) else {
            lock(&session.traces).running = Some((session.id, session.recorder.cycles()));
            return;
        };
        let (id, cycles, traces) = (
            session.id,
            session.recorder.cycles(),
            Arc::clone(&session.traces),
        );
        // The session ends, and the recorder's memory with it.
        self.trace = None;
        let mut slot = lock(&traces);
        slot.running = None;
        slot.ended = Some(EndedTrace {
            id,
            end,
            cycles,
            sha256: sha256(&file),
            file,
        });
    }
}

/// The fault record a trace keeps of a faulted scan (Appendix C, C.6): the code and statement of
/// the kernel's record of the fault the program raised (specification 44 §8.6), with the POU that
/// holds the statement, and zeros for a fault from outside the program, such as a deadline's, of
/// which the kernel keeps no record.
pub(super) fn cycle_fault(image: &Image<'_>, record: &FaultRecord) -> CycleFault {
    if record.code == 0 {
        return CycleFault {
            code: 0,
            pou: 0,
            statement: 0,
        };
    }
    let pou = image
        .table::<StmtRecord>()
        .ok()
        .and_then(|statements| statements.get(record.stmt))
        .map_or(0, |statement| u32::from(statement.pou));
    CycleFault {
        code: record.code,
        pou,
        statement: record.stmt,
    }
}

/// The task named `name`, compared without regard to case: its index in the image's task table and
/// its interval.
fn task_by_name(image: &Image<'_>, name: &str) -> Option<(u32, Duration)> {
    let tasks = image.table::<TaskRecord>().ok()?;
    tasks.iter().zip(0_u32..).find_map(|(task, index)| {
        let named = image.string(task.name)?;
        named.eq_ignore_ascii_case(name).then(|| {
            let interval = i64::try_from(task.interval_ns).unwrap_or(i64::MAX);
            (index, Duration::from_nanos(interval))
        })
    })
}

/// The recorded image's located variables (Appendix C, C.4), from its prepared bindings.
fn located(
    image: &Image<'_>,
    bindings: &[PreparedIoBinding],
    names: &[Option<SmolStr>],
) -> Vec<Located> {
    let symbols = image.table::<SymbolRecord>().ok();
    bindings
        .iter()
        .zip(names)
        .map(|(binding, name)| {
            let address = &binding.address;
            let type_index = symbols
                .as_ref()
                .and_then(|symbols| symbols.get(binding.symbol))
                .map(|symbol| symbol.type_index);
            Located {
                path: name.as_deref().unwrap_or_default().to_string(),
                type_name: type_index
                    .map(|index| type_name(image, index).to_string())
                    .unwrap_or_default(),
                area: trace_area(address.area),
                bit: trace_bit(address),
                offset: address.byte,
                size: address_size(address.size),
            }
        })
        .collect()
}

/// The recorded image's sections that name its memory (Appendix C, C.2).
fn tables(image: &Image<'_>) -> ImageTables {
    ImageTables {
        areas: image.section_bytes(SectionId::Areas).to_vec(),
        tasks: image.section_bytes(SectionId::Tasks).to_vec(),
        iobind: image.section_bytes(SectionId::IoBind).to_vec(),
        symbols: image.section_bytes(SectionId::Symbols).to_vec(),
        types: image.section_bytes(SectionId::Types).to_vec(),
        strings: image.section_bytes(SectionId::Strings).to_vec(),
    }
}

fn utc_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .and_then(|since| i64::try_from(since.as_millis()).ok())
        .unwrap_or(0)
}
