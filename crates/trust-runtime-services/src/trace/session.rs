//! A recording on demand on any runtime (specification 48 RU-59, "Every runtime"; specification 11
//! §6.8.1, "Cycle traces"): the slot that the control link reads, the identity a trace names, what a
//! recording takes from the recorded program, and a recording from its start to its served file.
//!
//! The runtime keeps the slot where its control link and its scans both reach it, and gives what
//! only it knows: the device's identity and platform, the UTC time, and the memory it can give. An
//! ended trace is served part by part from its records ([`ServedTrace`]), and everything a recording
//! holds is reserved when it starts, so that a chip never allocates what it may not have (RU-59,
//! "Memory").

use alloc::boxed::Box;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use smol_str::SmolStr;
use trust_runtime_core::deployment::sha256;
use trust_runtime_core::io_image::PreparedIoBinding;
use trust_runtime_core::native::image::{Image, SectionId, StmtRecord, SymbolRecord, TaskRecord};
use trust_runtime_core::native::memory::type_name;
use trust_runtime_core::trace::{CycleFault, ImageTables, Located};
use trust_runtime_core::value::Duration;

use super::requests::{RecordRequest, TraceName, TraceRefusal, TraceStarted, TraceState, Until};
use super::{
    address_size, trace_area, trace_bit, Budget, Goal, Identity, Recorder, ReserveError,
    ServedTrace, Sizes,
};

/// The traces the control link reads: the recording that runs, the last that ended, and the flight
/// recorder's trace of the last fault, each served part by part. A trace the slot keeps is boxed,
/// so that the slot itself stays a few words: a chip's runner holds it from its boot, whether or not
/// anything records (specification 34 MEM-12).
#[derive(Debug, Default)]
pub struct TraceSlot {
    last_id: u64,
    running: Option<(u64, u64)>,
    ended: Option<(u64, Box<ServedTrace>)>,
    /// The flight recorder's trace of the last fault, which stays until the next fault.
    fault: Option<Box<ServedTrace>>,
}

impl TraceSlot {
    /// The state of the trace `name` (`trace.read`): a recording's, recording, ended, or unknown
    /// for a trace the slot no longer holds, such as one a later recording replaced; or the flight
    /// recorder's trace of the last fault, none before a fault has given one.
    #[must_use]
    pub fn state(&self, name: TraceName) -> TraceState {
        let TraceName::Id(id) = name else {
            return self
                .fault
                .as_deref()
                .map_or(TraceState::None, ServedTrace::state);
        };
        if let Some((running, cycles)) = self.running
            && running == id
        {
            return TraceState::Recording { cycles };
        }
        match &self.ended {
            Some((ended, served)) if *ended == id => served.state(),
            _ => TraceState::Unknown,
        }
    }

    /// Copies the bytes of the ended trace `name`'s file from `offset` into `out`, as many as `out`
    /// holds, and gives how many it copied; `None` for a trace it does not hold or that has not
    /// ended, and for an offset past the file's end.
    #[must_use]
    pub fn part(&self, name: TraceName, offset: usize, out: &mut [u8]) -> Option<usize> {
        let served = match name {
            TraceName::Id(id) => &*self.ended.as_ref().filter(|(ended, _)| *ended == id)?.1,
            TraceName::Fault => self.fault.as_deref()?,
        };
        (offset <= served.len()).then(|| served.part(offset, out))
    }

    /// Keeps `trace`, the flight recorder's trace of a fault, in place of the last fault's: it stays
    /// readable until the next fault, whatever recordings start (specification 11 §6.8.1, "Cycle
    /// traces").
    pub fn keep_fault(&mut self, trace: ServedTrace) {
        self.fault = Some(Box::new(trace));
    }

    /// A recording starts: its identity, which replaces the last trace's (specification 11,
    /// "Cycle traces").
    fn begin(&mut self) -> u64 {
        self.last_id += 1;
        self.running = Some((self.last_id, 0));
        self.ended = None;
        self.last_id
    }
}

/// What a trace's identity says of the program and the device (Appendix C, C.3).
#[derive(Debug, Clone)]
pub struct SessionIdentity {
    image_sha256: [u8; 32],
    sources_hash: [u8; 32],
    target: u32,
    runtime_id: SmolStr,
    platform: SmolStr,
    resource: SmolStr,
    task: Option<SmolStr>,
    started_utc_ms: i64,
}

impl SessionIdentity {
    /// The identity of a trace of the program of `image` on `device`, of `resource`, counting the
    /// task `task` by name when it counts one, begun at the UTC time `started_utc_ms`.
    #[must_use]
    pub fn of(
        image: &Image<'_>,
        device: &Device<'_>,
        resource: SmolStr,
        task: Option<SmolStr>,
        started_utc_ms: i64,
    ) -> Self {
        let header = image.header();
        Self {
            image_sha256: sha256(image.bytes()),
            sources_hash: header.source_hash,
            target: header.target,
            runtime_id: SmolStr::new(device.runtime_id),
            platform: SmolStr::new(device.platform),
            resource,
            task,
            started_utc_ms,
        }
    }

    /// The UTC time, in milliseconds, when the trace began.
    #[must_use]
    pub const fn started_utc_ms(&self) -> i64 {
        self.started_utc_ms
    }

    /// The identity as a trace's file writes it, begun at `started_utc_ms`.
    #[must_use]
    pub fn identity(&self, started_utc_ms: i64) -> Identity<'_> {
        Identity {
            image_sha256: self.image_sha256,
            sources_hash: self.sources_hash,
            target: self.target,
            runtime_version: env!("CARGO_PKG_VERSION"),
            runtime_id: &self.runtime_id,
            platform: &self.platform,
            resource: &self.resource,
            task: self.task.as_deref(),
            started_utc_ms,
        }
    }
}

/// The device a trace is recorded on, as `status` names it: its runtime ID and its platform.
#[derive(Debug, Clone)]
pub struct Device<'a> {
    /// The device's identity (`runtime_id`).
    pub runtime_id: &'a str,
    /// The device's platform, such as `linux-x86_64` or `esp32c6`.
    pub platform: &'a str,
}

/// What a recording takes from the program that runs: its image, its located variables' bindings
/// and the names the debugger gives them, its resource and the cycle interval its scans run at.
#[derive(Clone, Copy)]
pub struct Program<'a> {
    /// The program's image.
    pub image: &'a Image<'a>,
    /// The located variables' prepared bindings, in the order of the image's I/O binding table.
    pub bindings: &'a [PreparedIoBinding],
    /// Each binding's variable as the debugger names it.
    pub binding_names: &'a [Option<SmolStr>],
    /// The resource's name.
    pub resource: &'a SmolStr,
    /// The interval the resource's scans run at.
    pub cycle_interval: Duration,
}

impl Program<'_> {
    /// What a recording of the program reserves its memory for: its areas, its tasks, the cycle
    /// interval its scans run at, and the interval of the task a goal counts, when it counts one.
    #[must_use]
    pub fn sizes(&self, task_interval: Option<Duration>) -> Sizes {
        let image = self.image;
        Sizes {
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
            cycle_interval: self.cycle_interval,
            task_interval,
        }
    }

    /// The task named `name`, compared without regard to case: its index in the image's task table
    /// and its interval.
    #[must_use]
    pub fn task(&self, name: &str) -> Option<(u32, Duration)> {
        let image = self.image;
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
    #[must_use]
    pub fn located(&self) -> Vec<Located> {
        let image = self.image;
        let symbols = image.table::<SymbolRecord>().ok();
        self.bindings
            .iter()
            .zip(self.binding_names)
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

    /// A copy of the recorded image's sections that name its memory (Appendix C, C.2), reserved
    /// first: `None` when the memory cannot be given. A trace keeps its own copy, since a program
    /// installed after the recording would make the image's own sections another program's.
    #[must_use]
    pub fn tables(&self) -> Option<ImageTables> {
        let section = |id| -> Option<Vec<u8>> {
            let bytes = self.image.section_bytes(id);
            let mut copy = Vec::new();
            copy.try_reserve_exact(bytes.len()).ok()?;
            copy.extend_from_slice(bytes);
            Some(copy)
        };
        Some(ImageTables {
            areas: section(SectionId::Areas)?,
            tasks: section(SectionId::Tasks)?,
            iobind: section(SectionId::IoBind)?,
            symbols: section(SectionId::Symbols)?,
            types: section(SectionId::Types)?,
            strings: section(SectionId::Strings)?,
        })
    }

    /// The bytes of the recorded image's sections that name its memory, which a trace copies.
    #[must_use]
    pub fn tables_len(&self) -> usize {
        [
            SectionId::Areas,
            SectionId::Tasks,
            SectionId::IoBind,
            SectionId::Symbols,
            SectionId::Types,
            SectionId::Strings,
        ]
        .into_iter()
        .map(|id| self.image.section_bytes(id).len())
        .fold(0, usize::saturating_add)
    }

    /// The fault record a trace keeps of a faulted scan ([`cycle_fault`]).
    #[must_use]
    pub fn cycle_fault(&self, code: u32, statement: u32) -> CycleFault {
        cycle_fault(self.image, code, statement)
    }
}

/// A recording on demand that the scans fill (`trace.record`).
pub struct TraceSession {
    id: u64,
    recorder: Recorder,
    identity: SessionIdentity,
    located: Vec<Located>,
    /// The copy of the recorded image's tables, which the served trace takes when it ends.
    tables: Option<ImageTables>,
    /// The trace's first cycle has begun: its start state is taken at the cycle's first access.
    pub start_pending: bool,
}

impl TraceSession {
    /// Starts a recording at the next cycle (`trace.record`): its goal from `request`, its memory
    /// reserved now within `budget`, between two scans, and its identity begun at `now_utc_ms`.
    /// The slot then holds it as running, in place of the last trace. `free` is the memory the
    /// runtime has, where it knows it, which a refusal names.
    ///
    /// # Errors
    ///
    /// [`TraceRefusal::UnknownTask`] for a task the program lacks, and
    /// [`TraceRefusal::InsufficientMemory`] for memory the runtime cannot give.
    pub fn start(
        request: &RecordRequest,
        program: &Program<'_>,
        device: &Device<'_>,
        budget: Budget,
        free: Option<usize>,
        now_utc_ms: i64,
        slot: &mut TraceSlot,
    ) -> Result<(Self, TraceStarted), TraceRefusal> {
        let task = match &request.task {
            None => None,
            Some(name) => Some(
                program
                    .task(name)
                    .ok_or_else(|| TraceRefusal::UnknownTask(name.clone()))?,
            ),
        };
        let goal = match (request.until, task) {
            (Until::Cycles(runs), Some((task, _))) => Goal::TaskRuns { task, runs },
            (Until::Cycles(count), None) => Goal::Cycles(count),
            (Until::Duration(duration), _) => Goal::Duration(duration),
        };
        let sizes = program.sizes(task.map(|(_, interval)| interval));
        let tables_len = program.tables_len();
        let refused = |needed: usize| TraceRefusal::InsufficientMemory {
            needed: needed.saturating_add(tables_len),
            free,
        };
        let recorder = Recorder::arm(goal, sizes, budget)
            .map_err(|ReserveError::InsufficientMemory { needed }| refused(needed))?;
        let tables = program
            .tables()
            .ok_or_else(|| refused(recorder.reserved_bytes()))?;
        let reserved_bytes = recorder.reserved_bytes().saturating_add(tables_len);
        let id = slot.begin();
        let session = Self {
            id,
            recorder,
            identity: SessionIdentity::of(
                program.image,
                device,
                program.resource.clone(),
                request.task.as_deref().map(SmolStr::new),
                now_utc_ms,
            ),
            located: program.located(),
            tables: Some(tables),
            start_pending: false,
        };
        Ok((
            session,
            TraceStarted {
                trace: id,
                reserved_bytes,
            },
        ))
    }

    /// The recording's identity in the slot.
    #[must_use]
    pub const fn id(&self) -> u64 {
        self.id
    }

    /// The recording the scans fill.
    pub fn recorder(&mut self) -> &mut Recorder {
        &mut self.recorder
    }

    /// The recording the scans fill and whether its start state is still due, together, for a
    /// scan that takes the start state into the recording and then marks it taken.
    pub fn recorder_and_start(&mut self) -> (&mut Recorder, &mut bool) {
        (&mut self.recorder, &mut self.start_pending)
    }

    /// After a scan: the recording's progress into `slot`, or once it has ended its trace, served
    /// part by part, each variable named by `name` as the debugger names it (specification 48
    /// Appendix C, C.6). It answers whether the recording has ended, so that the runtime drops the
    /// session.
    pub fn publish(
        &mut self,
        slot: &mut TraceSlot,
        name: &mut dyn FnMut(&str) -> Option<String>,
    ) -> bool {
        if self.recorder.end().is_none() {
            slot.running = Some((self.id, self.recorder.cycles()));
            return false;
        }
        let identity = self.identity.identity(self.identity.started_utc_ms);
        let tables = self.tables.take().unwrap_or_default();
        let Some(served) = self.recorder.served(&identity, &self.located, tables, name) else {
            return false;
        };
        slot.running = None;
        slot.ended = Some((self.id, Box::new(served)));
        true
    }
}

#[cfg(test)]
mod tests;

/// The fault record a trace keeps of a faulted scan of the program of `image` (Appendix C, C.6):
/// the code `code` and the statement `statement` of the kernel's record of the fault the program
/// raised (specification 44 §8.6), with the POU that holds the statement, and zeros for a fault from
/// outside the program, such as a deadline's, of which the kernel keeps no record (code 0).
#[must_use]
pub fn cycle_fault(image: &Image<'_>, code: u32, statement: u32) -> CycleFault {
    if code == 0 {
        return CycleFault {
            code: 0,
            pou: 0,
            statement: 0,
        };
    }
    let pou = image
        .table::<StmtRecord>()
        .ok()
        .and_then(|statements| statements.get(statement))
        .map_or(0, |record| u32::from(record.pou));
    CycleFault {
        code,
        pou,
        statement,
    }
}
