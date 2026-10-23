//! A compiled program in the kernel (specification 44 §8, §10): the image admitted against the
//! device, its areas in RAM with their starting memory, its tasks scheduled as specification 10 §4
//! schedules them, its variables bound to the process image, and the faults of its module turned
//! into the runtime's errors.
//!
//! The kernel never decodes the program: it reads the image's tables where they lie and calls the
//! module's entry points. A platform puts the module's code where its CPU executes it and hands the
//! kernel the entry points ([`ModuleCode`]).

use alloc::vec;
use alloc::vec::Vec;
use core::fmt;

use smol_str::SmolStr;
use trust_plc_abi::kernel::{Entry, InPlace};
use trust_plc_abi::{Cx, FaultCode, FaultRecord, KernelApi, INTERFACE_VERSION};

use super::image::{
    AreaRecord, EntryKind, EntryRecord, Image, ImageError, IoBindRecord, SectionId, SymbolRecord,
    Target, TaskRecord, AREA_HAS_INIT, BACKGROUND_PRIORITY, NONE, SYMBOL_PROGRAM,
};
use super::retain;
use super::values::{damaged, read_value, AreaBytes, Types, Writer};
use crate::cycle::{sort_ready_tasks_by_priority, ReadyTask};
use crate::error::{ExecutionLimitKind, RuntimeError};
use crate::io_image::FlatImages;
use crate::retain::capture::RetainCapture;
use crate::retain::RetainSnapshot;
use crate::task::{evaluate_task_readiness, TaskState};
use crate::value::{DateTimeError, Duration};
use crate::vm::RetainApplyEvent;

/// Where a platform put an admitted module's code: the entry point at each offset of `CODE`
/// (specification 44 §10.3).
pub trait ModuleCode {
    /// The entry point whose first instruction is `offset` bytes into `CODE`; `None` when the
    /// platform cannot reach it.
    fn entry(&self, offset: u32) -> Option<Entry>;
}

/// The code of a device that executes the image where it lies.
impl ModuleCode for InPlace {
    fn entry(&self, offset: u32) -> Option<Entry> {
        InPlace::entry(self, offset)
    }
}

/// What a device offers a program (specification 44 §10.2, steps 2 and 6).
#[derive(Clone, Copy, Debug)]
pub struct Device {
    /// The device's CPU.
    pub target: Target,
    /// The bytes the device has for programs.
    pub program_memory: u64,
    /// The bytes of the task stack.
    pub task_stack: u64,
    /// The statements one activation may execute (specification 44 §8.4).
    pub max_statements: u32,
}

/// Why a device refuses an image; the first failed check names its reason (specification 44
/// §10.2).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal {
    /// The image is damaged or inconsistent.
    Image(ImageError),
    /// The image is for another CPU.
    Target {
        /// The image's target number.
        image: u32,
        /// The device's.
        device: Target,
    },
    /// The image was built against another kernel interface.
    Interface {
        /// The image's version.
        image: u16,
    },
    /// The program needs more memory than the device has.
    Memory {
        /// The program's memory figure.
        needs: u64,
        /// The device's memory for programs.
        has: u64,
    },
    /// The program's stack bound exceeds the task stack.
    Stack {
        /// The bound.
        needs: u64,
        /// The task stack.
        has: u64,
    },
    /// An entry point the platform cannot reach.
    Entry(u32),
}

impl fmt::Display for Refusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Image(error) => error.fmt(f),
            Self::Target { image, device } => write!(
                f,
                "the image is for target {image}, this device is {device:?}: rebuild the project for it"
            ),
            Self::Interface { image } => write!(
                f,
                "kernel interface {image} required, this runtime has {INTERFACE_VERSION}: rebuild the project"
            ),
            Self::Memory { needs, has } => write!(
                f,
                "insufficient memory: the program needs {needs} bytes, the device has {has}"
            ),
            Self::Stack { needs, has } => write!(
                f,
                "insufficient stack: the program needs {needs} bytes, the task stack has {has}"
            ),
            Self::Entry(offset) => write!(f, "the entry point at code offset {offset} cannot be reached"),
        }
    }
}

impl From<Refusal> for RuntimeError {
    fn from(refusal: Refusal) -> Self {
        RuntimeError::InvalidBundle(SmolStr::new(alloc::format!("{refusal}")))
    }
}

/// Admits the image in `bytes` on `device`: its header, its section table, its tables, its memory
/// figure and its stack bound (specification 44 §10.2, steps 2, 3, 5 and 6). The image is read where
/// it lies and nothing is allocated.
///
/// # Errors
///
/// The first [`Refusal`].
pub fn admit<'i>(bytes: &'i [u8], device: &Device) -> Result<Image<'i>, Refusal> {
    let image = Image::parse(bytes).map_err(Refusal::Image)?;
    let header = *image.header();
    if Target::from_raw(header.target) != Some(device.target) {
        return Err(Refusal::Target {
            image: header.target,
            device: device.target,
        });
    }
    if u32::from(header.interface_version) != INTERFACE_VERSION {
        return Err(Refusal::Interface {
            image: header.interface_version,
        });
    }
    image.check_tables().map_err(Refusal::Image)?;
    let figure = image.memory_figure();
    if figure.total > device.program_memory {
        return Err(Refusal::Memory {
            needs: figure.total,
            has: device.program_memory,
        });
    }
    if figure.stack > device.task_stack {
        return Err(Refusal::Stack {
            needs: figure.stack,
            has: device.task_stack,
        });
    }
    Ok(image)
}

/// One area's memory: 8-byte words, so that the module's views are aligned.
struct Memory {
    words: Vec<u64>,
    len: usize,
}

impl Memory {
    fn new(len: usize) -> Self {
        Self {
            words: vec![0; len.div_ceil(8).max(1)],
            len,
        }
    }

    fn write(&mut self, offset: usize, data: &[u8]) -> bool {
        if offset
            .checked_add(data.len())
            .is_none_or(|end| end > self.len)
        {
            return false;
        }
        for (index, byte) in data.iter().enumerate() {
            let at = offset + index;
            let shift = (at % 8) * 8;
            if let Some(word) = self.words.get_mut(at / 8) {
                *word = (*word & !(0xFF << shift)) | (u64::from(*byte) << shift);
            }
        }
        true
    }

    fn read(&self, offset: usize, out: &mut [u8]) -> bool {
        if offset
            .checked_add(out.len())
            .is_none_or(|end| end > self.len)
        {
            return false;
        }
        for (index, byte) in out.iter_mut().enumerate() {
            let at = offset + index;
            let word = self.words.get(at / 8).copied().unwrap_or(0);
            *byte = (word >> ((at % 8) * 8)).to_le_bytes()[0];
        }
        true
    }

    fn clear(&mut self) {
        self.words.fill(0);
    }

    fn base(&mut self) -> *mut u8 {
        self.words.as_mut_ptr().cast::<u8>()
    }

    /// The area's bytes, copied.
    fn bytes(&self) -> Vec<u8> {
        let mut bytes = vec![0; self.len];
        self.read(0, &mut bytes);
        bytes
    }
}

impl AreaBytes for Memory {
    fn len(&self) -> usize {
        self.len
    }

    fn read(&self, offset: usize, out: &mut [u8]) -> bool {
        Memory::read(self, offset, out)
    }
}

/// A variable bound to a process image address.
#[derive(Clone, Copy)]
struct Binding {
    /// 1 inputs, 2 outputs, 3 markers.
    image: u8,
    /// 0 a bit, else a width in bytes.
    width: u32,
    byte: u32,
    bit: u8,
    /// The variable's area and offset.
    area: u8,
    offset: u32,
}

/// A task with the entry points of its programs.
struct NativeTask {
    name: SmolStr,
    interval: Duration,
    priority: u32,
    /// The `SINGLE` variable's area and offset.
    single: Option<(u8, u32)>,
    programs: Vec<usize>,
    state: TaskState,
}

/// What a scan reports while it runs.
pub trait ScanHooks {
    /// Task `task` missed `missed` activations before this one.
    fn task_overrun(&mut self, _task: usize, _missed: u64) {}
    /// Task `task` starts.
    fn task_started(&mut self, _task: usize) {}
    /// Task `task` ended.
    fn task_finished(&mut self, _task: usize) {}
}

/// Scan hooks that report nothing.
pub struct NoHooks;

impl ScanHooks for NoHooks {}

/// An admitted program with its areas.
pub struct NativeProgram<'i, C> {
    image: Image<'i>,
    code: C,
    areas: [Memory; 7],
    entries: Vec<Entry>,
    cold: Option<usize>,
    warm: Option<usize>,
    tasks: Vec<NativeTask>,
    bindings: Vec<Binding>,
    ready: Vec<ReadyTask>,
    cx: Cx,
    budget: u32,
    /// The input image the drivers fill (`%I`).
    pub inputs: Vec<u8>,
    /// The output image the drivers drive (`%Q`).
    pub outputs: Vec<u8>,
}

/// The index of area `id` in a program's memory, 1 to 7.
fn slot(id: u8) -> Option<usize> {
    usize::from(id).checked_sub(1).filter(|index| *index < 7)
}

/// The index of the retain area in a program's memory.
const RETAIN: usize = trust_plc_abi::AREA_RETAIN as usize - 1;

/// The index of the stack area in a program's memory: the texts of a failed assertion
/// (specification 44 §7.1).
const STACK: usize = trust_plc_abi::AREA_STACK as usize - 1;

/// Whether area `id` is a process image: `%I`, `%Q` or `%M`.
fn process_image(id: u32) -> bool {
    (1..=3).contains(&id)
}

impl<'i, C: ModuleCode> NativeProgram<'i, C> {
    /// The program of the admitted `image` with its module at `code`, its areas allocated and
    /// zero. The program starts with [`Self::cold_start`] or [`Self::warm_start`].
    ///
    /// # Errors
    ///
    /// [`Refusal::Entry`] for an entry point the platform cannot reach; [`Refusal::Image`] for a
    /// table that does not read.
    pub fn new(
        image: Image<'i>,
        code: C,
        device: &Device,
        api: &'static KernelApi,
    ) -> Result<Self, Refusal> {
        let table_error = Refusal::Image;
        let entries_table = image.table::<EntryRecord>().map_err(table_error)?;
        let mut entries = Vec::with_capacity(entries_table.len() as usize);
        let mut cold = None;
        let mut warm = None;
        for (index, entry) in entries_table.iter().enumerate() {
            let Some(function) = code.entry(entry.code_offset) else {
                return Err(Refusal::Entry(entry.code_offset));
            };
            if entry.kind == EntryKind::ColdInit as u32 {
                cold = Some(index);
            } else if entry.kind == EntryKind::WarmInit as u32 {
                warm = Some(index);
            }
            entries.push(function);
        }
        let mut lengths = [0usize; 7];
        for area in image.table::<AreaRecord>().map_err(table_error)?.iter() {
            if let Some(index) = u8::try_from(area.id).ok().and_then(slot) {
                lengths[index] = usize::try_from(area.length).unwrap_or(0);
            }
        }
        let areas = lengths.map(Memory::new);
        let symbols = image.table::<SymbolRecord>().map_err(table_error)?;
        let mut bindings = Vec::new();
        for binding in image.table::<IoBindRecord>().map_err(table_error)?.iter() {
            let Some(symbol) = symbols.get(binding.symbol) else {
                continue;
            };
            bindings.push(Binding {
                image: binding.area,
                width: if binding.size == 0 {
                    0
                } else {
                    binding.width()
                },
                byte: binding.byte,
                bit: binding.bit,
                area: symbol.area,
                offset: symbol.offset,
            });
        }
        let mut tasks = Vec::new();
        for task in image.table::<TaskRecord>().map_err(table_error)?.iter() {
            let single = (task.single != NONE)
                .then(|| symbols.get(task.single))
                .flatten()
                .map(|symbol| (symbol.area, symbol.offset));
            tasks.push(NativeTask {
                name: SmolStr::new(image.string(task.name).unwrap_or("")),
                interval: Duration::from_nanos(i64::try_from(task.interval_ns).unwrap_or(i64::MAX)),
                priority: task.priority,
                single,
                programs: image
                    .task_programs(&task)
                    .filter_map(|entry| usize::try_from(entry).ok())
                    .collect(),
                state: TaskState::new(Duration::ZERO),
            });
        }
        let inputs = vec![0; lengths[0]];
        let outputs = vec![0; lengths[1]];
        let mut program = Self {
            image,
            code,
            areas,
            entries,
            cold,
            warm,
            tasks,
            bindings,
            ready: Vec::new(),
            cx: Cx::new(api),
            budget: device.max_statements,
            inputs,
            outputs,
        };
        // The process images take their starting memory once; a restart carries them over
        // (specification 11 §6.7, "Directly represented variables at restart").
        program
            .start_memory(process_image)
            .map_err(|_| Refusal::Image(ImageError::Table(SectionId::Areas)))?;
        Ok(program)
    }

    /// The image the program runs.
    pub fn image(&self) -> &Image<'i> {
        &self.image
    }

    /// Where the module's code is.
    pub fn code(&self) -> &C {
        &self.code
    }

    /// The tasks by name and priority, in the image's order; the programs without a task are the
    /// task of priority [`BACKGROUND_PRIORITY`].
    pub fn tasks(&self) -> impl Iterator<Item = (&str, u32)> {
        self.tasks
            .iter()
            .map(|task| (task.name.as_str(), task.priority))
    }

    /// The bytes of area `id`, 0 for an area the program does not have.
    pub fn area_length(&self, id: u8) -> usize {
        slot(id).map_or(0, |index| self.areas[index].len)
    }

    /// Reads `out.len()` bytes of area `id` from `offset`; `false` outside the area.
    pub fn read(&self, id: u8, offset: usize, out: &mut [u8]) -> bool {
        slot(id).is_some_and(|index| self.areas[index].read(offset, out))
    }

    /// Writes `data` into area `id` at `offset`; `false` outside the area.
    pub fn write(&mut self, id: u8, offset: usize, data: &[u8]) -> bool {
        slot(id).is_some_and(|index| self.areas[index].write(offset, data))
    }

    /// Copies the process images, `%I`, `%Q` and `%M`, into `images` as the last scan left them,
    /// keeping the capacity `images` has.
    pub fn copy_images(&self, images: &mut FlatImages) {
        for (area, image) in [
            (0, &mut images.inputs),
            (1, &mut images.outputs),
            (2, &mut images.memory),
        ] {
            image.resize(self.areas[area].len, 0);
            self.areas[area].read(0, image);
        }
    }

    /// Copies the starting memory into the areas `starts` selects by their id (specification 44
    /// §6.7, §7.4).
    fn start_memory(&mut self, starts: impl Fn(u32) -> bool) -> Result<(), RuntimeError> {
        let init = self.image.section_bytes(SectionId::Init);
        let areas = self.image.table::<AreaRecord>().map_err(|error| {
            RuntimeError::InvalidBundle(SmolStr::new(alloc::format!("{error}")))
        })?;
        for area in areas.iter() {
            let Some(index) = u8::try_from(area.id).ok().and_then(slot) else {
                continue;
            };
            if !starts(area.id) {
                continue;
            }
            let memory = &mut self.areas[index];
            memory.clear();
            if area.flags & AREA_HAS_INIT != 0 {
                let start = area.init_offset as usize;
                let bytes = init.get(start..start + memory.len).unwrap_or(&[]);
                memory.write(0, bytes);
            }
        }
        Ok(())
    }

    /// A cold restart: every variable takes its starting memory, the retained ones too, the
    /// cold-start initialization runs, and the variables located at `%M` are published into the
    /// marker image (specification 44 §6.7; IEC §6.5.6.2; specification 11 §6.7). The process
    /// images are carried over.
    ///
    /// # Errors
    ///
    /// The fault of the initialization.
    pub fn cold_start(&mut self, now: Duration) -> Result<(), RuntimeError> {
        self.start_memory(|id| !process_image(id))?;
        self.reset_task_timing(now);
        if let Some(entry) = self.cold {
            self.call(entry, u32::MAX, now)?;
        }
        self.publish_markers();
        Ok(())
    }

    /// A warm restart: every variable but the retained ones takes its starting memory, the
    /// retained variables keep what they hold but an instance or a reference, which is never
    /// retained (specification 10), the warm-start initialization runs, and the variables located at
    /// `%M` are published into the marker image (IEC §6.5.6.1; specification 11 §6.7). The process
    /// images are carried over.
    ///
    /// # Errors
    ///
    /// The fault of the initialization.
    pub fn warm_start(&mut self, now: Duration) -> Result<(), RuntimeError> {
        let retain = u32::from(trust_plc_abi::AREA_RETAIN);
        self.start_memory(|id| !process_image(id) && id != retain)?;
        // A retained instance or reference is never retained: it starts again.
        for (offset, starting) in retain::unretainable(&self.image)? {
            self.areas[RETAIN].write(offset, &starting);
        }
        self.reset_task_timing(now);
        if let Some(entry) = self.warm {
            self.call(entry, u32::MAX, now)?;
        }
        self.publish_markers();
        Ok(())
    }

    /// A warm restart from the checkpoint `snapshot`: the checkpoint is restored into the retained
    /// variables by name, each value checked against the variable's type before any of them
    /// changes, and then the warm restart runs (specification 44 §11; specification 11 §6.7). A
    /// retained variable the checkpoint does not name keeps what it holds, which after a power-on
    /// is its starting value. Returns the events of the restore.
    ///
    /// # Errors
    ///
    /// The refusal of a value, which changes nothing, or the fault of the initialization.
    pub fn warm_start_from(
        &mut self,
        now: Duration,
        snapshot: &RetainSnapshot,
    ) -> Result<Vec<RetainApplyEvent>, RuntimeError> {
        let current = self.areas[RETAIN].bytes();
        let (staged, events) = retain::stage(&self.image, snapshot, &current)?;
        self.areas[RETAIN].write(0, &staged);
        self.warm_start(now)?;
        Ok(events)
    }

    /// Refills `capture` with the checkpoint of the retained variables as they are now: each
    /// variable of the retain map under its key, written from the retain area without building a
    /// value, so that a capture into a reserved payload allocates nothing (specification 44 §11;
    /// specification 34 MEM-04).
    ///
    /// # Errors
    ///
    /// [`RuntimeError::RetainStore`] for a value no checkpoint holds, such as a non-finite `REAL`;
    /// the capture then holds no payload.
    pub fn capture_retain(&self, capture: &mut RetainCapture) -> Result<(), RuntimeError> {
        let area = &self.areas[RETAIN];
        capture.refill(|payload| retain::fill(&self.image, area, payload))
    }

    /// The most bytes [`Self::capture_retain`] writes, which a capture reserves (specification 34
    /// MEM-09).
    ///
    /// # Errors
    ///
    /// The error of [`retain::payload_bound`].
    pub fn retain_payload_bound(&self) -> Result<usize, RuntimeError> {
        retain::payload_bound(&self.image)
    }

    /// Every task counts its next activation from `now` (specification 10 §4).
    pub fn reset_task_timing(&mut self, now: Duration) {
        for task in &mut self.tasks {
            task.state = TaskState::new(now);
        }
    }

    /// Copies the bound variables in from the input and marker images before the tasks run.
    fn sample(&mut self) {
        let inputs = core::mem::take(&mut self.inputs);
        self.areas[0].write(0, &inputs);
        self.inputs = inputs;
        // By index: a scan allocates nothing (specification 34, "The allocation count").
        for index in 0..self.bindings.len() {
            let binding = self.bindings[index];
            if binding.image == 2 {
                continue;
            }
            let image = usize::from(binding.image - 1);
            let Some(area) = slot(binding.area) else {
                continue;
            };
            if binding.width == 0 {
                let mut byte = [0u8];
                self.areas[image].read(binding.byte as usize, &mut byte);
                let value = (byte[0] >> binding.bit) & 1;
                self.areas[area].write(binding.offset as usize, &[value]);
            } else {
                let mut bytes = [0u8; 8];
                let width = binding.width as usize;
                self.areas[image].read(binding.byte as usize, &mut bytes[..width]);
                self.areas[area].write(binding.offset as usize, &bytes[..width]);
            }
        }
    }

    /// Copies the bound variable of `binding` out to its output or marker image.
    fn publish_binding(&mut self, binding: Binding) {
        let image = usize::from(binding.image - 1);
        let Some(area) = slot(binding.area) else {
            return;
        };
        if binding.width == 0 {
            let mut value = [0u8];
            self.areas[area].read(binding.offset as usize, &mut value);
            let mut byte = [0u8];
            self.areas[image].read(binding.byte as usize, &mut byte);
            let mask = 1u8 << binding.bit;
            byte[0] = if value[0] != 0 {
                byte[0] | mask
            } else {
                byte[0] & !mask
            };
            self.areas[image].write(binding.byte as usize, &byte);
        } else {
            let mut bytes = [0u8; 8];
            let width = binding.width as usize;
            self.areas[area].read(binding.offset as usize, &mut bytes[..width]);
            self.areas[image].write(binding.byte as usize, &bytes[..width]);
        }
    }

    /// Publishes the variables located at `%M` into the marker image, so that the first scan's
    /// sample finds their starting or retained values there (specification 11 §6.7). Every other
    /// byte of the image keeps its value.
    fn publish_markers(&mut self) {
        for index in 0..self.bindings.len() {
            let binding = self.bindings[index];
            if binding.image == 3 {
                self.publish_binding(binding);
            }
        }
    }

    /// Copies the bound variables out to the output and marker images after the tasks ran, and
    /// the output area to the output image.
    fn publish(&mut self) {
        // By index: a scan allocates nothing (specification 34, "The allocation count").
        for index in 0..self.bindings.len() {
            let binding = self.bindings[index];
            if binding.image != 1 {
                self.publish_binding(binding);
            }
        }
        // A safe-state write may have grown the output image past the program's outputs
        // (`write_flat_image`); the program's outputs are its first bytes.
        let mut outputs = core::mem::take(&mut self.outputs);
        let length = outputs.len().min(self.areas[1].len);
        self.areas[1].read(0, &mut outputs[..length]);
        self.outputs = outputs;
    }

    /// Runs one scan at logical time `now`: the inputs sampled, every due task's programs by
    /// priority, the programs without a task, the outputs published (specification 10 §4;
    /// specification 34 CYCLE-01 to CYCLE-06).
    ///
    /// # Errors
    ///
    /// The fault of the program that faulted; the scan ends there and publishes nothing.
    pub fn scan(&mut self, now: Duration, hooks: &mut dyn ScanHooks) -> Result<(), RuntimeError> {
        self.sample();
        let mut ready = core::mem::take(&mut self.ready);
        ready.clear();
        for index in 0..self.tasks.len() {
            if self.tasks[index].priority == BACKGROUND_PRIORITY {
                continue;
            }
            let single = match self.tasks[index].single {
                Some((area, offset)) => {
                    let mut byte = [0u8];
                    self.read(area, offset as usize, &mut byte);
                    byte[0] != 0
                }
                None => false,
            };
            let task = &mut self.tasks[index];
            let readiness = evaluate_task_readiness(&mut task.state, task.interval, single, now);
            if readiness.missed_intervals > 0 {
                hooks.task_overrun(index, readiness.missed_intervals);
            }
            if let Some(due_at) = readiness.due_at {
                ready.push(ReadyTask { index, due_at });
            }
        }
        let tasks = &self.tasks;
        sort_ready_tasks_by_priority(&mut ready, |index| tasks[index].priority);
        let mut result = Ok(());
        'tasks: for entry in &ready {
            hooks.task_started(entry.index);
            for position in 0..self.tasks[entry.index].programs.len() {
                let program = self.tasks[entry.index].programs[position];
                let task = u32::try_from(entry.index).unwrap_or(u32::MAX);
                if let Err(error) = self.call(program, task, now) {
                    result = Err(error);
                    break 'tasks;
                }
            }
            hooks.task_finished(entry.index);
        }
        self.ready = ready;
        result?;
        for index in 0..self.tasks.len() {
            if self.tasks[index].priority != BACKGROUND_PRIORITY {
                continue;
            }
            for position in 0..self.tasks[index].programs.len() {
                let program = self.tasks[index].programs[position];
                self.call(program, u32::MAX, now)?;
            }
        }
        self.publish();
        Ok(())
    }

    /// Calls entry `entry` for task `task` at logical time `now`.
    fn call(&mut self, entry: usize, task: u32, now: Duration) -> Result<(), RuntimeError> {
        let Some(function) = self.entries.get(entry).copied() else {
            return Err(RuntimeError::InvalidExecutionState);
        };
        for (index, memory) in self.areas.iter_mut().enumerate() {
            let id = u8::try_from(index + 1).unwrap_or(0);
            let len = u32::try_from(memory.len).unwrap_or(u32::MAX);
            let base = memory.base();
            self.cx.set_area(id, base, len);
        }
        self.cx.task = task;
        self.cx.now_ns = now.as_nanos();
        self.cx.budget = self.budget;
        self.cx.fault = FaultRecord::default();
        let code = function.call(&mut self.cx);
        if code == 0 {
            Ok(())
        } else if code == FaultCode::AssertionFailed as u32
            || code == FaultCode::AssertionNear as u32
        {
            // A test's failed assertion, worded as the runtime words it, its operands read
            // through the type table.
            let types = Types::of(&self.image)?;
            Err(RuntimeError::AssertionFailed(
                assertions::message(&types, &self.cx.fault, code, &self.areas[STACK]).into(),
            ))
        } else {
            Err(fault_error(&self.cx.fault, code, self.budget))
        }
    }

    /// Runs the body of program instance `instance`, the program's number of its body's entry, once
    /// at logical time `now`, outside the task schedule: the inputs are not sampled and the outputs
    /// not published. This is a test program's run (specification 22; `trust-dev test`).
    ///
    /// # Errors
    ///
    /// The fault the body ended with; [`RuntimeError::UndefinedProgram`] for an instance the
    /// image has no body of.
    pub fn run_instance(&mut self, instance: u32, now: Duration) -> Result<(), RuntimeError> {
        let entry = self
            .image
            .table::<EntryRecord>()
            .ok()
            .and_then(|entries| {
                entries.iter().position(|entry| {
                    entry.kind == EntryKind::ProgramBody as u32 && entry.index == instance
                })
            })
            .ok_or_else(|| RuntimeError::UndefinedProgram(alloc::format!("{instance}").into()))?;
        self.call(entry, u32::MAX, now)
    }

    /// The value of the variable whose canonical name is `canonical`, read through the symbol and
    /// type tables as the interpreter holds it (specification 44 §5.7, §5.8, §13.3); `None` for a
    /// name the image does not declare or a symbol that holds no value.
    pub fn value(&self, canonical: &str) -> Option<crate::value::Value> {
        let (index, _) = self.image.find_symbol(canonical)?;
        self.read_symbol(index).ok()
    }

    /// The record of symbol `index` and the index of its area in the program's memory; a program
    /// instance holds no value of its own.
    fn variable(&self, index: u32) -> Result<(SymbolRecord, usize), RuntimeError> {
        let symbol = self
            .image
            .table::<SymbolRecord>()
            .ok()
            .and_then(|symbols| symbols.get(index))
            .ok_or_else(|| damaged("symbol table"))?;
        if symbol.flags & SYMBOL_PROGRAM != 0 {
            return Err(RuntimeError::TypeMismatch);
        }
        // A frame variable's symbol names the stack area, where no variable lies between calls.
        let area = slot(symbol.area)
            .filter(|area| *area != STACK)
            .ok_or(RuntimeError::TypeMismatch)?;
        Ok((symbol, area))
    }

    /// The value of the variable of symbol `index`, read through the type table as the interpreter
    /// holds it: an enumeration by its declared names, a structure by its declared type and member
    /// names (specification 44 §5.8).
    ///
    /// # Errors
    ///
    /// [`RuntimeError::TypeMismatch`] for a symbol that holds no value (a program, an instance, a
    /// frame variable) or bytes that are no value of the type, [`RuntimeError::InvalidBundle`] for a
    /// symbol or a type the image's tables do not hold.
    pub fn read_symbol(&self, index: u32) -> Result<crate::value::Value, RuntimeError> {
        let (symbol, area) = self.variable(index)?;
        let types = Types::of(&self.image)?;
        read_value(
            &types,
            &self.areas[area],
            symbol.offset as usize,
            symbol.type_index,
        )
    }

    /// Writes `value` into the variable of symbol `index` with the conversions of the type table: a
    /// number widened within its family, a text cut to its capacity, an enumeration of the same type
    /// and value, a structure member by member, where a member the value lacks keeps what it holds.
    /// The variable keeps its value when the write is refused.
    ///
    /// # Errors
    ///
    /// [`RuntimeError::TypeMismatch`] for a value the variable's type refuses or a symbol that holds
    /// no value, [`RuntimeError::SubrangeViolation`] for a number outside its subrange.
    pub fn write_symbol(
        &mut self,
        index: u32,
        value: &crate::value::Value,
    ) -> Result<(), RuntimeError> {
        let (symbol, area) = self.variable(index)?;
        let types = Types::of(&self.image)?;
        let offset = symbol.offset as usize;
        let mut current = vec![0u8; symbol.size as usize];
        if !self.areas[area].read(offset, &mut current) {
            return Err(damaged("symbol table"));
        }
        let mut staged = current.clone();
        Writer {
            types: &types,
            staged: &mut staged,
            starting: &current,
        }
        .write(symbol.type_index, 0, value, 0)?;
        self.areas[area].write(offset, &staged);
        Ok(())
    }

    /// The record of the last fault a module raised.
    pub fn last_fault(&self) -> FaultRecord {
        self.cx.fault
    }
}

/// The runtime error of the fault `code` with its record (specification 44 §8.6, Appendix A).
pub fn fault_error(record: &FaultRecord, code: u32, budget: u32) -> RuntimeError {
    let [a, b, c] = record.detail;
    match FaultCode::from_raw(code) {
        Some(FaultCode::DivisionByZero) => RuntimeError::DivisionByZero,
        Some(FaultCode::ModuloByZero) => RuntimeError::ModuloByZero,
        Some(FaultCode::Overflow) => RuntimeError::Overflow,
        Some(FaultCode::DateTimeRange) => RuntimeError::DateTimeRange(DateTimeError::OutOfRange),
        Some(FaultCode::IndexOutOfBounds) => RuntimeError::IndexOutOfBounds {
            index: a,
            lower: b,
            upper: c,
        },
        Some(FaultCode::SubrangeViolation) => RuntimeError::SubrangeViolation {
            value: i128::from(a),
            lower: i128::from(b),
            upper: i128::from(c),
        },
        Some(FaultCode::NullReference) => RuntimeError::NullReference,
        Some(FaultCode::ForStepZero) => RuntimeError::ForStepZero,
        Some(FaultCode::TypeMismatch) => RuntimeError::TypeMismatch,
        Some(FaultCode::StringCapacityExceeded) => RuntimeError::StringCapacityExceeded,
        Some(FaultCode::ReferenceLifetime) => RuntimeError::ReferenceLifetime,
        Some(FaultCode::NonFiniteValue) => RuntimeError::NonFiniteValue,
        Some(FaultCode::InfiniteLoop) => RuntimeError::ExecutionLimit {
            kind: ExecutionLimitKind::Work,
            used: budget as usize,
            requested: 1,
            limit: budget as usize,
        },
        Some(FaultCode::AssertionFailed | FaultCode::AssertionNear) => {
            RuntimeError::AssertionFailed("assertion failed".into())
        }
        Some(
            FaultCode::StackOverflow | FaultCode::MemoryProtection | FaultCode::ModuleInternal,
        )
        | None => RuntimeError::InvalidExecutionState,
    }
}

mod assertions;
#[cfg(test)]
pub(crate) mod tests;
