//! A compiled program in the kernel (specification 44 §8, §10): the image admitted against the
//! device, its areas in RAM with their starting memory, its tasks scheduled as specification 10 §4
//! schedules them, its variables bound to the process image, and the faults of its module turned
//! into the runtime's errors.
//!
//! The kernel never decodes the program: it reads the image's tables where they lie and calls the
//! module's entry points. A platform puts the module's code where its CPU executes it and hands the
//! kernel the entry points ([`ModuleCode`]).

use alloc::boxed::Box;
use alloc::sync::Arc;
use alloc::vec;
use alloc::vec::Vec;
use core::cell::Cell;

use smol_str::SmolStr;
use trust_plc_abi::kernel::{call_with_sink, Entry, InPlace};
use trust_plc_abi::{Cx, FaultCode, FaultRecord};

use super::image::{
    EntryKind, EntryRecord, Image, Shape, StmtRecord, SymbolRecord, BACKGROUND_PRIORITY,
    BIND_TIME_MILLIS, NONE, SYMBOL_PROGRAM,
};
use super::memory::{read_place, retained_place, symbol_place, write_by_symbol, write_place};
use super::retain;
use super::stop::{arm_bits, HeldScan, Stop};
use super::values::{AreaBytes, Types};
use crate::cycle::{sort_ready_tasks_by_priority, ReadyTask};
use crate::error::{ExecutionLimitKind, RuntimeError};
use crate::io_address::IoAddress;
use crate::io_image::{read_flat_image, write_flat_image, FlatImages, IoAddressKey};
use crate::retain::capture::RetainCapture;
use crate::task::{evaluate_task_readiness, TaskState};
use crate::value::{DateTimeError, Duration, Value};

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

pub use super::admission::{admit, Device, Refusal};

/// One area's memory: 8-byte words, so that the module's views are aligned.
pub(super) struct Memory {
    words: Vec<u64>,
    pub(super) len: usize,
}

impl Memory {
    pub(super) fn new(len: usize) -> Self {
        Self {
            words: vec![0; len.div_ceil(8).max(1)],
            len,
        }
    }

    pub(super) fn write(&mut self, offset: usize, data: &[u8]) -> bool {
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

    pub(super) fn clear(&mut self) {
        self.words.fill(0);
    }

    fn base(&mut self) -> *mut u8 {
        self.words.as_mut_ptr().cast::<u8>()
    }

    /// The area's bytes, copied.
    pub(super) fn bytes(&self) -> Vec<u8> {
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
pub(super) struct Binding {
    /// 1 inputs, 2 outputs, 3 markers.
    pub(super) image: u8,
    /// 0 a bit, else a width in bytes.
    pub(super) width: u32,
    pub(super) byte: u32,
    pub(super) bit: u8,
    /// The bound leaf's area and offset: the variable's, plus the leaf's offset within it.
    pub(super) area: u8,
    pub(super) offset: u32,
    /// The codec: [`BIND_RAW`] or [`BIND_TIME_MILLIS`].
    pub(super) codec: u32,
    /// Whether the address is hierarchical: then `byte` is the binding's slot in the hierarchical
    /// image and `bit` is 0 (specification 44 §5.6).
    pub(super) hierarchical: bool,
    /// For a variable of an enumeration, its declared values: the first and the count in the
    /// program's `enum_values`, and the enumeration's name, a string offset; a value that is none
    /// of them is refused at the sample.
    pub(super) declared: Option<(u32, u32, u32)>,
}

/// The bytes of a binding's slot in the hierarchical image: a long word, the widest value of an
/// address.
pub(super) const HIERARCHICAL_SLOT: usize = 8;

/// A task with the entry points of its programs.
pub(super) struct NativeTask {
    pub(super) name: SmolStr,
    pub(super) interval: Duration,
    pub(super) priority: u32,
    /// The `SINGLE` variable's area and offset.
    pub(super) single: Option<(u8, u32)>,
    pub(super) programs: Vec<usize>,
    pub(super) state: TaskState,
}

/// What a scan reports while it runs.
pub trait ScanHooks {
    /// Task `task` missed `missed` activations before this one.
    fn task_overrun(&mut self, _task: usize, _missed: u64) {}
    /// Task `task` starts: a due task, or the background task that runs the programs outside every
    /// task, after the due tasks.
    fn task_started(&mut self, _task: usize) {}
    /// Task `task` ended.
    fn task_finished(&mut self, _task: usize) {}
    /// The body of a program instance starts: entry `entry` of the image's entry table.
    fn program_started(&mut self, _entry: usize) {}
    /// The body of entry `entry` ended, also when it faulted.
    fn program_finished(&mut self, _entry: usize) {}
    /// A statement whose breakpoint bit is set stops, in a debug image (specification 44 §13.2):
    /// the debugger decides here and may hold the scan before the statement runs. The default
    /// runs on.
    ///
    /// # Errors
    ///
    /// The error that ends the scan, which the scan reports in place of a fault.
    fn on_stop(&mut self, _stop: &mut Stop<'_>) -> Result<(), RuntimeError> {
        Ok(())
    }
}

/// Scan hooks that report nothing.
pub struct NoHooks;

impl ScanHooks for NoHooks {}

/// Where a program's image lies: in memory a device maps for as long as the program runs, or in
/// memory the program shares with the readers of its state, as a computer keeps it (specification
/// 44 §8.8).
pub(super) enum ImageBytes<'i> {
    /// Where a device maps it.
    Mapped(&'i [u8]),
    /// The program's, shared.
    Kept(Arc<AdmittedImage>),
}

impl ImageBytes<'_> {
    pub(super) fn as_slice(&self) -> &[u8] {
        match self {
            Self::Mapped(bytes) => bytes,
            Self::Kept(image) => &image.bytes,
        }
    }
}

/// An image admitted on a device (§10.2) with the bytes it lies in, which the program built from it
/// keeps ([`NativeProgram::keeping`]) and shares with the readers of its state: a computer holds an
/// image in its memory, not where a device maps it (specification 44 §8.8).
pub struct AdmittedImage {
    bytes: Box<[u8]>,
    pub(super) shape: Shape,
}

impl AdmittedImage {
    /// Admits the image in `bytes` on `device`, as [`admit`] does, and keeps the bytes.
    ///
    /// # Errors
    ///
    /// The first [`Refusal`].
    pub fn admit(bytes: Box<[u8]>, device: &Device) -> Result<Self, Refusal> {
        let shape = admit(&bytes, device)?.shape();
        Ok(Self { bytes, shape })
    }

    /// The admitted image.
    #[must_use]
    pub fn image(&self) -> Image<'_> {
        Image::over(&self.bytes, &self.shape)
    }
}

/// An admitted program with its areas.
pub struct NativeProgram<'i, C> {
    /// Where the image lies.
    pub(super) bytes: ImageBytes<'i>,
    /// What the image's header and section table hold.
    pub(super) shape: Shape,
    pub(super) code: C,
    pub(super) areas: [Memory; 7],
    pub(super) entries: Vec<Entry>,
    pub(super) cold: Option<usize>,
    pub(super) warm: Option<usize>,
    pub(super) tasks: Vec<NativeTask>,
    /// Whether the tasks are live: the program scanned since it was built, or took over the
    /// schedule of a running program. Until then a start of the task timing registers the tasks
    /// (specification 10 §4.1.1, [`NativeProgram::reset_task_timing`]).
    pub(super) live: bool,
    pub(super) bindings: Vec<Binding>,
    pub(super) ready: Vec<ReadyTask>,
    pub(super) cx: Cx,
    pub(super) budget: u32,
    /// The recursive calls an activation may make (specification 44 §6.4, §8.5).
    pub(super) depth: u32,
    /// One bit per statement id: the statements that stop while `armed` (specification 44
    /// §13.2), as cells, which the module reads and a stop rewrites.
    pub(super) breakpoints: Vec<Cell<u8>>,
    /// Whether any statement stops.
    pub(super) armed: bool,
    /// The input image the drivers fill (`%I`).
    pub inputs: Vec<u8>,
    /// The output image the drivers drive (`%Q`).
    pub outputs: Vec<u8>,
    /// The values at the hierarchical addresses the program binds (`%IX1.2.3`, IEC §6.5.5.2), one
    /// slot of [`HIERARCHICAL_SLOT`] bytes per binding, which no driver fills; Live Values and a
    /// test write and read them by address (specification 44 §5.6).
    pub(super) hierarchical: Vec<u8>,
    /// Each hierarchical address with its slot's offset in `hierarchical`.
    pub(super) hierarchical_keys: Vec<(IoAddressKey, usize)>,
    /// The declared values of the enumerations of the bound variables, which their bindings name.
    pub(super) enum_values: Vec<i64>,
}

/// The address of the value of hierarchical `address` in its slot: its first byte, a bit at bit 0.
fn slot_address(address: &IoAddress) -> IoAddress {
    IoAddress {
        area: address.area,
        size: address.size,
        byte: 0,
        bit: 0,
        path: vec![0],
        wildcard: false,
    }
}

/// The index of area `id` in a program's memory, 1 to 7.
pub(super) fn slot(id: u8) -> Option<usize> {
    usize::from(id).checked_sub(1).filter(|index| *index < 7)
}

/// The index of the stack area in a program's memory: the texts of a failed assertion
/// (specification 44 §7.1).
const STACK: usize = trust_plc_abi::AREA_STACK as usize - 1;

/// Whether area `id` is a process image: `%I`, `%Q` or `%M`.
pub(super) fn process_image(id: u32) -> bool {
    (1..=3).contains(&id)
}

impl<C: ModuleCode> NativeProgram<'static, C> {
    /// The admitted image the program keeps, which the readers of its state share.
    #[must_use]
    pub fn kept_image(&self) -> Option<Arc<AdmittedImage>> {
        match &self.bytes {
            ImageBytes::Kept(image) => Some(Arc::clone(image)),
            ImageBytes::Mapped(_) => None,
        }
    }
}

impl<'i, C: ModuleCode> NativeProgram<'i, C> {
    /// The image the program runs.
    #[must_use]
    pub fn image(&self) -> Image<'_> {
        Image::over(self.bytes.as_slice(), &self.shape)
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

    /// The name of the program instance whose body each entry of the image's entry table runs, in
    /// the entries' order: its declared name, the holding program instance's for a function block
    /// instance that a task runs on its own, and empty for an entry that runs no body
    /// (specification 44 §5.4, §5.7). Program calls are recorded under these names
    /// (specification 11, runtime metrics).
    #[must_use]
    pub fn program_names(&self) -> Vec<SmolStr> {
        let image = self.image();
        let (Ok(entries), Ok(symbols)) =
            (image.table::<EntryRecord>(), image.table::<SymbolRecord>())
        else {
            return Vec::new();
        };
        entries
            .iter()
            .map(|entry| {
                if entry.kind != EntryKind::ProgramBody as u32 {
                    return SmolStr::default();
                }
                symbols
                    .iter()
                    .find(|symbol| {
                        symbol.flags & SYMBOL_PROGRAM != 0
                            && symbol.parent == NONE
                            && symbol.pou == entry.index
                    })
                    .and_then(|symbol| image.string(symbol.display))
                    .map(SmolStr::new)
                    .unwrap_or_default()
            })
            .collect()
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
    /// keeping the capacity `images` has. The output image is the one the drivers drive, with Live
    /// Values' writes and forces and the safe state over the program's outputs, as an engine's
    /// snapshot shows it (specification 44 §13.3).
    pub fn copy_images(&self, images: &mut FlatImages) {
        for (area, image) in [(0, &mut images.inputs), (2, &mut images.memory)] {
            image.resize(self.areas[area].len, 0);
            self.areas[area].read(0, image);
        }
        images.outputs.clone_from(&self.outputs);
    }

    /// Refills `capture` with the checkpoint of the retained variables as they are now: each
    /// variable of the retain map under its key, written from its area without building a value,
    /// so that a capture into a reserved payload allocates nothing (specification 44 §11;
    /// specification 34 MEM-04).
    ///
    /// # Errors
    ///
    /// [`RuntimeError::RetainStore`] for a value no checkpoint holds, such as a non-finite `REAL`;
    /// the capture then holds no payload.
    pub fn capture_retain(&self, capture: &mut RetainCapture) -> Result<(), RuntimeError> {
        capture.refill(|payload| {
            retain::fill(
                &self.image(),
                |id| slot(id).and_then(|index| self.areas.get(index)),
                payload,
            )
        })
    }

    /// Every task counts its next activation from `now` and its missed activations from zero
    /// (specification 10 §4.3). Before the program's first scan this registers the tasks: an event
    /// input (`SINGLE`) that is TRUE is taken as the last sample, so that the first scan sees no
    /// rising edge (specification 10 §4.1.1; specification 11 §6.2). Once the program has
    /// scanned, it resets the live task timing and clears the last samples, so that an event input
    /// held TRUE through a restart is a rising edge at the first scan after it (specification 10
    /// §4.1.1, §4.8).
    pub fn reset_task_timing(&mut self, now: Duration) {
        for index in 0..self.tasks.len() {
            let sample = !self.live && self.event_input(index);
            self.tasks[index].state = TaskState {
                last_single: sample,
                ..TaskState::new(now)
            };
        }
    }

    /// The level of the event input (`SINGLE`) of task `index` in the program's memory; FALSE for
    /// a task without one.
    #[inline]
    fn event_input(&self, index: usize) -> bool {
        match self.tasks[index].single {
            Some((area, offset)) => {
                let mut byte = [0u8];
                self.read(area, offset as usize, &mut byte);
                byte[0] != 0
            }
            None => false,
        }
    }

    /// Refuses an input or marker value that is no declared value of its variable's enumeration,
    /// before any variable takes a sampled value (specification 10 §2.5; specification 32 §5).
    ///
    /// # Errors
    ///
    /// [`RuntimeError::IoDriver`] naming the value and the enumeration.
    fn check_declared(&self) -> Result<(), RuntimeError> {
        for binding in &self.bindings {
            let Some((first, count, name)) = binding.declared else {
                continue;
            };
            if binding.image == 2 {
                continue;
            }
            let width = binding.width as usize;
            let mut bytes = [0u8; 8];
            if binding.hierarchical {
                let start = binding.byte as usize;
                if let Some(held) = self.hierarchical.get(start..start + width) {
                    bytes[..width].copy_from_slice(held);
                }
            } else if binding.image == 1 {
                let start = binding.byte as usize;
                if let Some(held) = self.inputs.get(start..start + width) {
                    bytes[..width].copy_from_slice(held);
                }
            } else {
                self.areas[2].read(binding.byte as usize, &mut bytes[..width]);
            }
            // The image holds the base's bits, signed or not, which the type table does not say.
            let unsigned = u64::from_le_bytes(bytes);
            let shift = 64 - u32::try_from(width * 8).unwrap_or(64).min(64);
            let signed = (unsigned.cast_signed() << shift) >> shift;
            let declared = self
                .enum_values
                .get(first as usize..(first + count) as usize)
                .unwrap_or(&[])
                .iter()
                .any(|value| *value == signed || *value == unsigned.cast_signed());
            if !declared {
                let image = self.image();
                return Err(RuntimeError::IoDriver(SmolStr::new(alloc::format!(
                    "process-image value {signed} is not declared by enum {}",
                    image.string(name).unwrap_or("")
                ))));
            }
        }
        Ok(())
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
            let Some(area) = slot(binding.area) else {
                continue;
            };
            if binding.width == 0 {
                let mut byte = [0u8];
                self.image_read(&binding, &mut byte);
                let value = (byte[0] >> binding.bit) & 1;
                self.areas[area].write(binding.offset as usize, &[value]);
            } else if binding.codec == BIND_TIME_MILLIS {
                // Whole milliseconds in, nanoseconds in the variable.
                let mut millis = [0u8; 4];
                self.image_read(&binding, &mut millis);
                let nanos = i64::from(u32::from_le_bytes(millis)) * 1_000_000;
                self.areas[area].write(binding.offset as usize, &nanos.to_le_bytes());
            } else {
                let mut bytes = [0u8; 8];
                let width = binding.width as usize;
                self.image_read(&binding, &mut bytes[..width]);
                self.areas[area].write(binding.offset as usize, &bytes[..width]);
            }
        }
    }

    /// Reads `out.len()` bytes of the image of `binding` from its byte: its process image, or its
    /// slot of the hierarchical image.
    fn image_read(&self, binding: &Binding, out: &mut [u8]) {
        let start = binding.byte as usize;
        if binding.hierarchical {
            if let Some(bytes) = self.hierarchical.get(start..start + out.len()) {
                out.copy_from_slice(bytes);
            }
        } else {
            self.areas[usize::from(binding.image - 1)].read(start, out);
        }
    }

    /// Writes `data` into the image of `binding` from its byte: its process image, or its slot of
    /// the hierarchical image.
    fn image_write(&mut self, binding: &Binding, data: &[u8]) {
        let start = binding.byte as usize;
        if binding.hierarchical {
            if let Some(bytes) = self.hierarchical.get_mut(start..start + data.len()) {
                bytes.copy_from_slice(data);
            }
        } else {
            self.areas[usize::from(binding.image - 1)].write(start, data);
        }
    }

    /// Copies the bound variable of `binding` out to its output or marker image.
    ///
    /// # Errors
    ///
    /// [`RuntimeError::Overflow`] for a `TIME` that whole unsigned 32-bit milliseconds cannot
    /// hold; the image keeps its bytes.
    fn publish_binding(&mut self, binding: Binding) -> Result<(), RuntimeError> {
        let Some(area) = slot(binding.area) else {
            return Ok(());
        };
        if binding.width == 0 {
            let mut value = [0u8];
            self.areas[area].read(binding.offset as usize, &mut value);
            let mut byte = [0u8];
            self.image_read(&binding, &mut byte);
            let mask = 1u8 << binding.bit;
            byte[0] = if value[0] != 0 {
                byte[0] | mask
            } else {
                byte[0] & !mask
            };
            self.image_write(&binding, &byte);
        } else if binding.codec == BIND_TIME_MILLIS {
            // Nanoseconds in the variable, whole milliseconds out, truncated toward zero.
            let mut nanos = [0u8; 8];
            self.areas[area].read(binding.offset as usize, &mut nanos);
            let nanos = i64::from_le_bytes(nanos);
            let millis = u64::try_from(nanos)
                .ok()
                .and_then(|nanos| u32::try_from(nanos / 1_000_000).ok())
                .ok_or(RuntimeError::Overflow)?;
            self.image_write(&binding, &millis.to_le_bytes());
        } else {
            let mut bytes = [0u8; 8];
            let width = binding.width as usize;
            self.areas[area].read(binding.offset as usize, &mut bytes[..width]);
            self.image_write(&binding, &bytes[..width]);
        }
        Ok(())
    }

    /// The slot in the hierarchical image of the hierarchical address `address`, which a variable
    /// binds (IEC §6.5.5.2; specification 44 §5.6).
    fn hierarchical_slot(&self, address: &IoAddress) -> Result<usize, RuntimeError> {
        let key = IoAddressKey::from(address);
        self.hierarchical_keys
            .iter()
            .find(|(held, _)| *held == key)
            .map(|(_, slot)| *slot)
            .ok_or_else(|| {
                RuntimeError::InvalidIoAddress(SmolStr::new(alloc::format!(
                    "no variable is bound at the hierarchical address {:?}",
                    address.path
                )))
            })
    }

    /// The value at the hierarchical address `address`, as its variable was last sampled from or
    /// published to it.
    ///
    /// # Errors
    ///
    /// [`RuntimeError::InvalidIoAddress`] for an address no variable binds.
    pub(crate) fn read_hierarchical(&self, address: &IoAddress) -> Result<Value, RuntimeError> {
        let slot = self.hierarchical_slot(address)?;
        let bytes = &self.hierarchical[slot..slot + HIERARCHICAL_SLOT];
        read_flat_image(bytes, &slot_address(address))
    }

    /// Writes `value` at the hierarchical address `address`, which the next scan samples into its
    /// variable.
    ///
    /// # Errors
    ///
    /// [`RuntimeError::InvalidIoAddress`] for an address no variable binds, and the error of a
    /// value of another size than the address's.
    pub(crate) fn write_hierarchical(
        &mut self,
        address: &IoAddress,
        value: Value,
    ) -> Result<(), RuntimeError> {
        let slot = self.hierarchical_slot(address)?;
        let mut bytes = self.hierarchical[slot..slot + HIERARCHICAL_SLOT].to_vec();
        write_flat_image(&mut bytes, &slot_address(address), value)?;
        self.hierarchical[slot..slot + HIERARCHICAL_SLOT]
            .copy_from_slice(&bytes[..HIERARCHICAL_SLOT]);
        Ok(())
    }

    /// Publishes the variables located at `%M` into the marker image, so that the first scan's
    /// sample finds their starting or retained values there (specification 11 §6.7). Every other
    /// byte of the image keeps its value.
    pub(super) fn publish_markers(&mut self) -> Result<(), RuntimeError> {
        for index in 0..self.bindings.len() {
            let binding = self.bindings[index];
            if binding.image == 3 {
                self.publish_binding(binding)?;
            }
        }
        Ok(())
    }

    /// Copies the bound variables out to the output and marker images after the tasks ran, and
    /// the output area to the output image.
    fn publish(&mut self) -> Result<(), RuntimeError> {
        // By index: a scan allocates nothing (specification 34, "The allocation count").
        for index in 0..self.bindings.len() {
            let binding = self.bindings[index];
            if binding.image != 1 {
                self.publish_binding(binding)?;
            }
        }
        // A safe-state write may have grown the output image past the program's outputs
        // (`write_flat_image`); the program's outputs are its first bytes.
        let mut outputs = core::mem::take(&mut self.outputs);
        let length = outputs.len().min(self.areas[1].len);
        self.areas[1].read(0, &mut outputs[..length]);
        self.outputs = outputs;
        Ok(())
    }

    /// Runs one scan at logical time `now`: the inputs sampled, every due task's programs by
    /// priority, the programs without a task, the outputs published (specification 10 §4;
    /// specification 34 CYCLE-01 to CYCLE-06).
    ///
    /// # Errors
    ///
    /// The fault of the program that faulted; the scan ends there and publishes nothing.
    pub fn scan(&mut self, now: Duration, hooks: &mut dyn ScanHooks) -> Result<(), RuntimeError> {
        self.check_declared()?;
        self.sample();
        let mut ready = core::mem::take(&mut self.ready);
        ready.clear();
        for index in 0..self.tasks.len() {
            if self.tasks[index].priority == BACKGROUND_PRIORITY {
                continue;
            }
            let single = self.event_input(index);
            let task = &mut self.tasks[index];
            let readiness = evaluate_task_readiness(&mut task.state, task.interval, single, now);
            if readiness.missed_intervals > 0 {
                hooks.task_overrun(index, readiness.missed_intervals);
            }
            if let Some(due_at) = readiness.due_at {
                ready.push(ReadyTask { index, due_at });
            }
        }
        // The tasks have sampled their inputs: they are live from now on.
        self.live = true;
        let tasks = &self.tasks;
        sort_ready_tasks_by_priority(&mut ready, |index| tasks[index].priority);
        let mut result = Ok(());
        'tasks: for entry in &ready {
            hooks.task_started(entry.index);
            for position in 0..self.tasks[entry.index].programs.len() {
                let program = self.tasks[entry.index].programs[position];
                let task = u32::try_from(entry.index).unwrap_or(u32::MAX);
                hooks.program_started(program);
                let called = self.call(program, task, now, Some(&mut *hooks));
                hooks.program_finished(program);
                if let Err(error) = called {
                    result = Err(error);
                    break 'tasks;
                }
            }
            hooks.task_finished(entry.index);
        }
        self.ready = ready;
        result?;
        for index in 0..self.tasks.len() {
            if self.tasks[index].priority != BACKGROUND_PRIORITY
                || self.tasks[index].programs.is_empty()
            {
                continue;
            }
            hooks.task_started(index);
            for position in 0..self.tasks[index].programs.len() {
                let program = self.tasks[index].programs[position];
                hooks.program_started(program);
                let called = self.call(program, u32::MAX, now, Some(&mut *hooks));
                hooks.program_finished(program);
                called?;
            }
            hooks.task_finished(index);
        }
        self.publish()
    }

    /// Calls entry `entry` for task `task` at logical time `now`; a statement that stops goes to
    /// `hooks` (specification 44 §13.2).
    pub(super) fn call(
        &mut self,
        entry: usize,
        task: u32,
        now: Duration,
        hooks: Option<&mut dyn ScanHooks>,
    ) -> Result<(), RuntimeError> {
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
        self.cx.depth = self.depth;
        self.cx.fault = FaultRecord::default();
        self.cx.breakpoints = self.breakpoints.as_ptr().cast::<u8>();
        self.cx.breakpoint_bytes = u32::try_from(self.breakpoints.len()).unwrap_or(0);
        self.cx.armed = u32::from(self.armed);
        let (code, held) = match hooks {
            Some(hooks) if self.armed => {
                let mut sink = HeldScan {
                    hooks,
                    image: Image::over(self.bytes.as_slice(), &self.shape),
                    bits: &self.breakpoints,
                    error: None,
                };
                let code = call_with_sink(function, &mut self.cx, &mut sink);
                (code, sink.error)
            }
            _ => (function.call(&mut self.cx), None),
        };
        // A stop may have armed or disarmed the statements for the rest of the scan.
        self.armed = self.cx.armed != 0;
        if let Some(error) = held {
            return Err(error);
        }
        if code == 0 {
            Ok(())
        } else if code == FaultCode::AssertionFailed as u32
            || code == FaultCode::AssertionNear as u32
        {
            // A test's failed assertion, worded as the runtime words it, its operands read
            // through the type table.
            let types = Types::of(&self.image())?;
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
            .image()
            .table::<EntryRecord>()
            .ok()
            .and_then(|entries| {
                entries.iter().position(|entry| {
                    entry.kind == EntryKind::ProgramBody as u32 && entry.index == instance
                })
            })
            .ok_or_else(|| RuntimeError::UndefinedProgram(alloc::format!("{instance}").into()))?;
        self.call(entry, u32::MAX, now, None)
    }

    /// The value of the variable whose canonical name is `canonical`, read through the symbol and
    /// type tables as the interpreter holds it (specification 44 §5.7, §5.8, §13.3); `None` for a
    /// name the image does not declare or a symbol that holds no value.
    pub fn value(&self, canonical: &str) -> Option<crate::value::Value> {
        let (index, _) = self.image().find_symbol(canonical)?;
        self.read_symbol(index).ok()
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
        let (place, type_index) = symbol_place(&self.image(), index)?;
        read_place(self, place, type_index)
    }

    /// Writes `value` into the variable of symbol `index` with the conversions of the type table: a
    /// number widened within its family, a text cut to its capacity, an enumeration of the same type
    /// and value, a structure member by member, where a member the value lacks keeps what it holds.
    /// The variable keeps its value when the write is refused.
    ///
    /// # Errors
    ///
    /// [`RuntimeError::TypeMismatch`] for a value the variable's type refuses or a symbol that holds
    /// no value, [`RuntimeError::SubrangeViolation`] for a number outside its subrange, and the
    /// errors of [`write_by_symbol`] for a variable in a `STRUCT OVERLAP`.
    pub fn write_symbol(
        &mut self,
        index: u32,
        value: &crate::value::Value,
    ) -> Result<(), RuntimeError> {
        write_by_symbol(self, index, value)
    }

    /// Writes `value` into the retained variable whose retain-map key is `label`, compared without
    /// regard to case (specification 44 §5.10), with the conversions of [`Self::write_symbol`]: a
    /// variable of an instance or an element of an array too, which no symbol of its own names.
    ///
    /// # Errors
    ///
    /// [`RuntimeError::UndefinedVariable`] for a key the retain map does not hold, and the errors
    /// of [`Self::write_symbol`].
    pub fn write_retained(
        &mut self,
        label: &str,
        value: &crate::value::Value,
    ) -> Result<(), RuntimeError> {
        let (place, type_index) = retained_place(&self.image(), label)?;
        write_place(self, place, type_index, value)
    }

    /// Arms the statements a scan of a debug image stops at: every one when `every` is set,
    /// otherwise those `wanted` takes; none stops when nothing is armed (specification 44 §13.2).
    pub fn arm(&mut self, every: bool, wanted: &dyn Fn(u32) -> bool) {
        self.armed = arm_bits(&self.breakpoints, every, wanted);
    }

    /// The number of the image's statements, the ids a breakpoint names.
    #[must_use]
    pub fn statement_count(&self) -> u32 {
        self.image()
            .table::<StmtRecord>()
            .map_or(0, |statements| statements.len())
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
        // A recursion beyond the depth is the bytecode engine's call beyond its call levels.
        Some(FaultCode::CallDepth) => RuntimeError::call_depth_exceeded(),
        Some(
            FaultCode::StackOverflow | FaultCode::MemoryProtection | FaultCode::ModuleInternal,
        )
        | None => RuntimeError::InvalidExecutionState,
    }
}

mod assertions;
mod online_change;
#[cfg(test)]
pub(crate) mod tests;
