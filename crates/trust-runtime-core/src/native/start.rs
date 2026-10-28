//! Building and starting a compiled program in the kernel (specification 44 §8, §10.2, §11): the
//! program of an admitted image with its areas, bindings and tasks, its cold and warm restarts, and
//! the retained values a warm restart keeps or restores. It runs once per install or restart, so a
//! device keeps it out of the RAM it holds for the scan path (specification 44 §10.3).

use alloc::sync::Arc;
use alloc::vec;
use alloc::vec::Vec;
use core::cell::Cell;

use smol_str::SmolStr;
use trust_plc_abi::{Cx, KernelApi};

use super::admission::{Device, Refusal};
use super::image::{
    AreaRecord, EntryKind, EntryRecord, Image, ImageError, IoBindRecord, MemoryFigure, SectionId,
    Shape, StmtRecord, SymbolRecord, TaskRecord, AREA_HAS_INIT, BIND_OFFSET_SHIFT, NONE,
};
use super::program::{
    process_image, slot, AdmittedImage, Binding, ImageBytes, Memory, ModuleCode, NativeProgram,
    NativeTask, HIERARCHICAL_SLOT,
};
use super::retain;
use super::values::Types;
use crate::error::RuntimeError;
use crate::io_image::IoAddressKey;
use crate::retain::{RetainApplyEvent, RetainSnapshot};
use crate::task::TaskState;
use crate::value::Duration;

impl<C: ModuleCode> NativeProgram<'static, C> {
    /// The program of the admitted `image`, which it keeps, with its module at `code`, its areas
    /// allocated and zero, as a computer runs it (specification 44 §8.8). The program starts with
    /// [`Self::cold_start`] or [`Self::warm_start`].
    ///
    /// # Errors
    ///
    /// As [`Self::new`].
    pub fn keeping(
        image: Arc<AdmittedImage>,
        code: C,
        device: &Device,
        api: &'static KernelApi,
    ) -> Result<Self, Refusal> {
        let shape = image.shape;
        Self::built(ImageBytes::Kept(image), shape, code, device, api)
    }
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
        Self::built(
            ImageBytes::Mapped(image.bytes()),
            image.shape(),
            code,
            device,
            api,
        )
    }

    /// The program of the image in `bytes`, as `shape` describes it, with its module at `code`.
    fn built(
        bytes: ImageBytes<'i>,
        shape: Shape,
        code: C,
        device: &Device,
        api: &'static KernelApi,
    ) -> Result<Self, Refusal> {
        let image = Image::over(bytes.as_slice(), &shape);
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
        let types =
            Types::of(&image).map_err(|_| Refusal::Image(ImageError::Table(SectionId::Types)))?;
        let mut bindings = Vec::new();
        let mut hierarchical_keys = Vec::new();
        let mut enum_values = Vec::new();
        for binding in image.table::<IoBindRecord>().map_err(table_error)?.iter() {
            let Some(symbol) = symbols.get(binding.symbol) else {
                continue;
            };
            // A hierarchical address takes the next slot of the hierarchical image.
            let (byte, bit) = if binding.is_hierarchical() {
                let address = image
                    .binding_address(&binding)
                    .ok_or(Refusal::Image(ImageError::Table(SectionId::IoBind)))?;
                let slot = hierarchical_keys.len() * HIERARCHICAL_SLOT;
                hierarchical_keys.push((IoAddressKey::from(&address), slot));
                (u32::try_from(slot).unwrap_or(u32::MAX), 0)
            } else {
                (binding.byte, binding.bit)
            };
            bindings.push(Binding {
                image: binding.area,
                width: if binding.size == 0 {
                    0
                } else {
                    binding.width()
                },
                byte,
                bit,
                area: symbol.area,
                offset: symbol
                    .offset
                    .saturating_add(binding.flags >> BIND_OFFSET_SHIFT),
                codec: binding.flags & 0xFF,
                hierarchical: binding.is_hierarchical(),
                declared: declared_values(&types, &symbol, &binding, &mut enum_values),
                float: bound_float(&types, &symbol, &binding),
            });
        }
        let hierarchical = vec![0; hierarchical_keys.len() * HIERARCHICAL_SLOT];
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
        let depth = call_depth(device, &image.memory_figure());
        let statements = image.table::<StmtRecord>().map_err(table_error)?.len();
        let breakpoints = (0..statements.div_ceil(8)).map(|_| Cell::new(0)).collect();
        let mut program = Self {
            bytes,
            shape,
            code,
            areas,
            entries,
            cold,
            warm,
            tasks,
            live: false,
            bindings,
            ready: Vec::new(),
            cx: Cx::new(api),
            budget: device.max_statements,
            depth,
            breakpoints,
            armed: false,
            scanning: false,
            inputs,
            outputs,
            hierarchical,
            hierarchical_keys,
            enum_values,
        };
        // The process images take their starting memory once; a restart carries them over
        // (specification 11 §6.7, "Directly represented variables at restart").
        program
            .start_memory(process_image)
            .map_err(|_| Refusal::Image(ImageError::Table(SectionId::Areas)))?;
        Ok(program)
    }

    /// Copies the starting memory into the areas `starts` selects by their id (specification 44
    /// §6.7, §7.4).
    pub(super) fn start_memory(
        &mut self,
        starts: impl Fn(u32) -> bool,
    ) -> Result<(), RuntimeError> {
        // The image is read from its own fields, so that the areas can be written meanwhile.
        let image = Image::over(self.bytes.as_slice(), &self.shape);
        let init = image.section_bytes(SectionId::Init);
        let areas = image.table::<AreaRecord>().map_err(|error| {
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
    /// cold-start initialization runs, the task timing starts again at `now`
    /// ([`Self::reset_task_timing`]), and the variables located at `%M` are published into the
    /// marker image (specification 44 §6.7; IEC §6.5.6.2; specification 11 §6.7). The process
    /// images are carried over.
    ///
    /// # Errors
    ///
    /// The fault of the initialization.
    pub fn cold_start(&mut self, now: Duration) -> Result<(), RuntimeError> {
        self.start_memory(|id| !process_image(id))?;
        if let Some(entry) = self.cold {
            self.call(entry, u32::MAX, now, None)?;
        }
        // After the initialization, so that a registration samples the event inputs' starting
        // values.
        self.reset_task_timing(now);
        self.publish_markers()
    }

    /// A warm restart: every retained variable the retain map names keeps what it holds, and every
    /// other variable takes its starting memory, an instance or a reference in the retain area too,
    /// which is never retained (specification 10); the warm-start initialization runs, the task
    /// timing starts again at `now` ([`Self::reset_task_timing`]), and the variables located at
    /// `%M` are published into the marker image (IEC §6.5.6.1; specification 11 §6.7;
    /// specification 44 §6.7). The process images are carried over.
    ///
    /// # Errors
    ///
    /// The fault of the initialization.
    pub fn warm_start(&mut self, now: Duration) -> Result<(), RuntimeError> {
        let kept = retain::retained_bytes(&self.image(), |id| {
            slot(id).and_then(|index| self.areas.get(index))
        })?;
        self.start_memory(|id| !process_image(id))?;
        self.keep(&kept)?;
        if let Some(entry) = self.warm {
            self.call(entry, u32::MAX, now, None)?;
        }
        // After the initialization, so that a registration samples the event inputs' retained or
        // starting values.
        self.reset_task_timing(now);
        self.publish_markers()
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
        let current: Vec<(u8, Vec<u8>)> = [
            trust_plc_abi::AREA_GLOBALS,
            trust_plc_abi::AREA_RETAIN,
            trust_plc_abi::AREA_INSTANCES,
        ]
        .into_iter()
        .filter_map(|id| Some((id, self.areas.get(slot(id)?)?.bytes())))
        .collect();
        let (staged, events) = retain::stage(&self.image(), snapshot, |id| {
            current
                .iter()
                .find(|(area, _)| *area == id)
                .map(|(_, bytes)| bytes.as_slice())
        })?;
        self.keep(&staged)?;
        self.warm_start(now)?;
        Ok(events)
    }

    /// Writes the bytes of retained variables back where they lie.
    fn keep(&mut self, kept: &[retain::Kept]) -> Result<(), RuntimeError> {
        for variable in kept {
            let written = slot(variable.area)
                .and_then(|index| self.areas.get_mut(index))
                .is_some_and(|area| area.write(variable.offset, &variable.bytes));
            if !written {
                return Err(super::values::damaged("retain map"));
            }
        }
        Ok(())
    }

    /// The most bytes [`Self::capture_retain`] writes, which a capture reserves (specification 34
    /// MEM-09).
    ///
    /// # Errors
    ///
    /// The error of [`retain::payload_bound`].
    pub fn retain_payload_bound(&self) -> Result<usize, RuntimeError> {
        retain::payload_bound(&self.image())
    }
}

/// The declared values of the enumeration of the variable `symbol` binds whole, `binding` binding
/// it at a byte, a word, a double word or a long word: added to `values`, with the enumeration's
/// name; `None` for a variable of any other type, a leaf of an aggregate and a bit.
fn declared_values(
    types: &Types<'_>,
    symbol: &SymbolRecord,
    binding: &IoBindRecord,
    values: &mut Vec<i64>,
) -> Option<(u32, u32, u32)> {
    if binding.flags >> BIND_OFFSET_SHIFT != 0 || binding.size == 0 {
        return None;
    }
    let ty = types.get(symbol.type_index).ok()?;
    if ty.kind != super::image::type_kind::ENUM {
        return None;
    }
    let first = u32::try_from(values.len()).ok()?;
    values.extend(types.variants(&ty).ok()?.map(|(_, value)| value));
    let count = u32::try_from(values.len()).ok()?.checked_sub(first)?;
    Some((first, count, ty.name))
}

/// The bytes of the `REAL` (4) or `LREAL` (8) that `binding` binds, its leaf found through the
/// variable's arrays and structures by the leaf's offset; 0 for any other type. The sample refuses
/// such an input's bits when they are no finite number (specification 11, "Floating-point boundary
/// admission policy").
fn bound_float(types: &Types<'_>, symbol: &SymbolRecord, binding: &IoBindRecord) -> u8 {
    use super::image::type_kind as kind;
    let Ok(mut ty) = types.get(symbol.type_index) else {
        return 0;
    };
    let mut offset = binding.flags >> BIND_OFFSET_SHIFT;
    // A type nests a bounded number of levels; the bound only guards a damaged table.
    for _ in 0..64 {
        match ty.kind {
            kind::REAL if offset == 0 => return 4,
            kind::LREAL if offset == 0 => return 8,
            kind::ARRAY => {
                let Some(element) = types
                    .array(&ty)
                    .ok()
                    .and_then(|(element, _)| types.get(element).ok())
                    .filter(|element| element.size > 0)
                else {
                    return 0;
                };
                offset %= element.size;
                ty = element;
            }
            kind::STRUCT => {
                let Some((member, member_type)) =
                    types.members(&ty).ok().and_then(|mut members| {
                        members.find_map(|member| {
                            let member_type = types.get(member.type_index).ok()?;
                            (offset >= member.offset
                                && offset - member.offset < member_type.size.max(1))
                            .then_some((member, member_type))
                        })
                    })
                else {
                    return 0;
                };
                offset -= member.offset;
                ty = member_type;
            }
            _ => return 0,
        }
    }
    0
}

/// The recursive calls an activation of a program with memory figure `figure` may make on `device`:
/// the task stack beyond the stack bound, in call levels (specification 44 §8.5). A program without
/// a recursion makes no recursive call, and its depth is never taken.
fn call_depth(device: &Device, figure: &MemoryFigure) -> u32 {
    if figure.call_level == 0 {
        return u32::MAX;
    }
    let spare = device.task_stack.saturating_sub(figure.stack);
    u32::try_from(spare / figure.call_level).unwrap_or(u32::MAX)
}
