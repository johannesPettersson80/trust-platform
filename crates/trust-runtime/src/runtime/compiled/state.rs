//! The state of a compiled program as a scan left it, or as the debugger holds it, which a
//! computer's other threads read (specification 44 §8.8, §13.2, §13.3): a copy of the program's areas
//! with the image they belong to, and of a held scan's call stack, read through the image's symbol
//! and type tables. Nothing in it decodes a variable it is not asked for.

use std::sync::Arc;

use smol_str::SmolStr;
use trust_runtime_core::native::image::{
    Image, SymbolRecord, NONE, SYMBOL_CONSTANT, SYMBOL_INSTANCE, SYMBOL_PROGRAM, SYMBOL_RETAINED,
};
use trust_runtime_core::native::memory::{
    enum_values, pou_symbol, read_place, symbol_place, type_name, ProgramMemory, StopFrame,
};
use trust_runtime_core::native::program::{AdmittedImage, ModuleCode, NativeProgram};

use crate::error::RuntimeError;
use crate::value::{Duration, Value};

/// A variable of a compiled program as a computer's services list it (specification 44 §5.7,
/// §13.3): a program instance's own variable, or a global, found through its symbol.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompiledVariable {
    /// The index of the variable's symbol, through which its value is read.
    pub symbol: u32,
    /// The declared full name: `Vat.RegState` for a variable of the program instance `Vat`, the
    /// name of a global.
    pub name: SmolStr,
    /// The program instance whose own variable it is, by its declared name; none for a global and
    /// for a member of a structure or an instance, which `name` names in full.
    pub program: Option<SmolStr>,
    /// The variable's own declared name.
    pub member: SmolStr,
    /// The variable's IEC type name, as the type table names it (`INT`, `STRING[20]`, `TON`).
    pub type_name: SmolStr,
    /// Whether the variable is retained.
    pub retained: bool,
    /// Whether the variable is a constant.
    pub constant: bool,
    /// Whether the variable is an instance of a function block or a class, which holds no value
    /// of its own.
    pub instance: bool,
}

/// A copy of a compiled program's state at the end of a scan, or of a held scan.
#[derive(Clone)]
pub struct CompiledState {
    image: Arc<AdmittedImage>,
    /// The bytes of areas 1 to 7, in their order.
    areas: [Vec<u8>; 7],
    /// The held scan's call stack, innermost frame first, each with the bytes of its frame
    /// variables; empty between scans.
    frames: Vec<(StopFrame, Vec<u8>)>,
    now: Duration,
}

impl std::fmt::Debug for CompiledState {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("CompiledState")
            .field("now", &self.now)
            .finish_non_exhaustive()
    }
}

impl CompiledState {
    /// The state of `program` at the logical time `now`: a copy of each of its areas.
    ///
    /// # Errors
    ///
    /// [`RuntimeError::InvalidExecutionState`] for a program that does not keep its image, as a
    /// device's does.
    pub fn of<C: ModuleCode>(
        program: &NativeProgram<'static, C>,
        now: Duration,
    ) -> Result<Self, RuntimeError> {
        let image = program
            .kept_image()
            .ok_or(RuntimeError::InvalidExecutionState)?;
        Ok(Self::copied(image, program, Vec::new(), now))
    }

    /// The state of the scan the debugger holds, whose memory is `memory`, of the program of
    /// `image`: its areas and the frames of its call stack.
    #[must_use]
    pub fn held(image: Arc<AdmittedImage>, memory: &dyn ProgramMemory, now: Duration) -> Self {
        let frames = memory
            .frames()
            .into_iter()
            .enumerate()
            .map(|(index, frame)| {
                let size = if frame.locals {
                    pou_symbol(&image.image(), frame.pou)
                        .map_or(0, |(_, symbol)| symbol.size as usize)
                } else {
                    0
                };
                let mut bytes = vec![0; size];
                if !memory.read_frame(index, 0, &mut bytes) {
                    bytes.clear();
                }
                (frame, bytes)
            })
            .collect();
        Self::copied(image, memory, frames, now)
    }

    /// The copy of the areas of `memory`, of the program of `image`, with `frames`.
    fn copied(
        image: Arc<AdmittedImage>,
        memory: &dyn ProgramMemory,
        frames: Vec<(StopFrame, Vec<u8>)>,
        now: Duration,
    ) -> Self {
        let areas = std::array::from_fn(|index| {
            let id = u8::try_from(index + 1).unwrap_or(u8::MAX);
            let length = image
                .image()
                .area_length(id)
                .and_then(|length| usize::try_from(length).ok())
                .unwrap_or(0);
            let mut bytes = vec![0; length];
            if !memory.read_area(id, 0, &mut bytes) {
                bytes.clear();
            }
            bytes
        });
        Self {
            image,
            areas,
            frames,
            now,
        }
    }

    /// The logical time of the scan the state is of.
    #[must_use]
    pub fn now(&self) -> Duration {
        self.now
    }

    /// The program instances, by their declared names, in the symbol table's order.
    #[must_use]
    pub fn programs(&self) -> Vec<SmolStr> {
        let image = self.image();
        let Ok(symbols) = image.table::<SymbolRecord>() else {
            return Vec::new();
        };
        symbols
            .iter()
            .filter(|symbol| symbol.flags & SYMBOL_PROGRAM != 0)
            .filter_map(|symbol| image.string(symbol.display).map(SmolStr::new))
            .collect()
    }

    /// The variables of the program instances and the globals: each program instance's own
    /// variables, and then the globals, each by area (specification 44 §7.1) and in an area in their
    /// declared order, which their places keep (§7.2); not the members of structures and instances,
    /// nor program roots, POUs or frame variables.
    #[must_use]
    pub fn variables(&self) -> Vec<CompiledVariable> {
        self.listed(|parent| parent.is_none_or(|(_, record)| record.flags & SYMBOL_PROGRAM != 0))
    }

    /// The members of the instance or the structure whose symbol is `symbol`, each named in full
    /// (`Vat.Timer.Q`), as the symbol table's parent links join them (specification 44 §5.7), by
    /// area and in an area in their declared order; none for a variable without members. The
    /// private state of a library block has no symbol and is not among them.
    #[must_use]
    pub fn members(&self, symbol: u32) -> Vec<CompiledVariable> {
        self.listed(|parent| parent.is_some_and(|(index, _)| index == symbol))
    }

    /// The variables whose parent `belongs` takes, the parent's index and record or none for a
    /// global, in the order of their places.
    fn listed(
        &self,
        belongs: impl Fn(Option<(u32, &SymbolRecord)>) -> bool,
    ) -> Vec<CompiledVariable> {
        let image = self.image();
        let Ok(symbols) = image.table::<SymbolRecord>() else {
            return Vec::new();
        };
        let display = |symbol: &SymbolRecord| image.string(symbol.display).unwrap_or("");
        let mut variables: Vec<((u32, u8, u32), CompiledVariable)> = (0_u32..)
            .zip(symbols.iter())
            .filter(|(_, symbol)| {
                symbol.flags & SYMBOL_PROGRAM == 0 && (1..=6).contains(&symbol.area)
            })
            .filter_map(|(index, symbol)| {
                let parent = if symbol.parent == NONE {
                    None
                } else {
                    Some((symbol.parent, symbols.get(symbol.parent)?))
                };
                if !belongs(parent.as_ref().map(|(index, record)| (*index, record))) {
                    return None;
                }
                let program = parent
                    .filter(|(_, record)| record.flags & SYMBOL_PROGRAM != 0)
                    .map(|(_, record)| SmolStr::new(display(&record)));
                let name = SmolStr::new(display(&symbol));
                let member = SmolStr::new(name.rsplit('.').next().unwrap_or(""));
                let order = (
                    if symbol.parent == NONE {
                        u32::MAX
                    } else {
                        symbol.parent
                    },
                    symbol.area,
                    symbol.offset,
                );
                Some((
                    order,
                    CompiledVariable {
                        symbol: index,
                        name,
                        program,
                        member,
                        type_name: type_name(&image, symbol.type_index),
                        retained: symbol.flags & SYMBOL_RETAINED != 0,
                        constant: symbol.flags & SYMBOL_CONSTANT != 0,
                        instance: symbol.flags & SYMBOL_INSTANCE != 0,
                    },
                ))
            })
            .collect();
        variables.sort_by_key(|(order, _)| *order);
        variables
            .into_iter()
            .map(|(_, variable)| variable)
            .collect()
    }

    /// The value of the variable whose symbol is `symbol`: of an instance of a function block or a
    /// class its identity, the index of its symbol (specification 44 §13.3), as an interpreted
    /// program's instance value is its identity; none for a symbol that holds no value.
    #[must_use]
    pub fn value_of(&self, symbol: u32) -> Option<Value> {
        let image = self.image();
        let record = image.table::<SymbolRecord>().ok()?.get(symbol)?;
        if record.flags & SYMBOL_INSTANCE != 0 {
            return Some(Value::Instance(trust_runtime_core::memory::InstanceId(
                symbol,
            )));
        }
        let (place, type_index) = symbol_place(&image, symbol).ok()?;
        read_place(self, place, type_index).ok()
    }

    /// The names of the values of the variable's enumeration type, whose symbol is `symbol`, in their
    /// declared order; empty for a variable of another type.
    #[must_use]
    pub fn enum_values(&self, symbol: u32) -> Vec<SmolStr> {
        let image = self.image();
        image
            .table::<SymbolRecord>()
            .ok()
            .and_then(|symbols| symbols.get(symbol))
            .map(|record| enum_values(&image, record.type_index))
            .unwrap_or_default()
    }

    /// The value of the variable `name` names, as the test harness resolves a name: a global, a
    /// program's variable `Program.var`, or a unique unqualified name; `None` for a name that names
    /// no single variable or a variable that holds no value.
    #[must_use]
    pub fn value(&self, name: &str) -> Option<Value> {
        let symbol = self.image().find_variable(name).ok()?;
        self.value_of(symbol)
    }
}

impl ProgramMemory for CompiledState {
    fn image(&self) -> Image<'_> {
        self.image.image()
    }

    fn read_area(&self, id: u8, offset: usize, out: &mut [u8]) -> bool {
        let Some(area) = usize::from(id)
            .checked_sub(1)
            .and_then(|index| self.areas.get(index))
        else {
            return false;
        };
        match offset
            .checked_add(out.len())
            .and_then(|end| area.get(offset..end))
        {
            Some(bytes) => {
                out.copy_from_slice(bytes);
                true
            }
            None => false,
        }
    }

    /// The state is a copy for reading: a write reaches the program through the Live Values queue,
    /// never through the copy.
    fn write_area(&mut self, _id: u8, _offset: usize, _data: &[u8]) -> bool {
        false
    }

    fn frames(&self) -> Vec<StopFrame> {
        self.frames.iter().map(|(frame, _)| *frame).collect()
    }

    fn read_frame(&self, frame: usize, offset: usize, out: &mut [u8]) -> bool {
        let Some((_, bytes)) = self.frames.get(frame) else {
            return false;
        };
        match offset
            .checked_add(out.len())
            .and_then(|end| bytes.get(offset..end))
        {
            Some(bytes) => {
                out.copy_from_slice(bytes);
                true
            }
            None => false,
        }
    }

    fn write_image(
        &mut self,
        _address: &trust_runtime_core::io_address::IoAddress,
        _value: Value,
    ) -> Result<(), RuntimeError> {
        Err(RuntimeError::InvalidExecutionState)
    }
}
