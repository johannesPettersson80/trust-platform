//! A compiled program's variables by the names a test gives them, and its process images by
//! address (specification 44 §14.2; specification 11 §6.8.1, "Stepped scans"): what the
//! conformance runner and a stepped scan write before a scan and read after it.

use alloc::format;
use alloc::vec;
use core::fmt;

use super::image::{Image, SymbolRecord, NONE, SYMBOL_PROGRAM};
use super::literal;
use super::memory::{
    member_place, read_place, retained_place, symbol_place, write_place, Place, ProgramMemory,
};
use super::program::{ModuleCode, NativeProgram};
use crate::error::RuntimeError;
use crate::io_address::IoAddress;
use crate::io_image::{read_flat_image, write_flat_image};
use crate::memory::IoArea;
use crate::value::Value;

/// Why a name reaches no single variable of a compiled program.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NameError {
    /// No variable has the name.
    Unknown,
    /// The name alone names a variable of several programs.
    Ambiguous,
}

impl fmt::Display for NameError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Unknown => "names no variable of the compiled program",
            Self::Ambiguous => "names a variable of several programs",
        })
    }
}

impl Image<'_> {
    /// The symbol of the variable `name` names, as the test harness resolves a name: a global by its
    /// name, a qualified path such as `Program.variable` by its full name, and a program variable by
    /// its name alone when exactly one program declares it. Names compare as canonical names, without
    /// regard to case (IEC 61131-3 §6.1.2).
    ///
    /// # Errors
    ///
    /// [`NameError::Unknown`] when no variable has the name, and [`NameError::Ambiguous`] when the
    /// name alone names a variable of several programs.
    pub fn find_variable(&self, name: &str) -> Result<u32, NameError> {
        let canonical = name.to_ascii_uppercase();
        if let Some((index, symbol)) = self.find_symbol(&canonical)
            && symbol.flags & SYMBOL_PROGRAM == 0
        {
            return Ok(index);
        }
        let symbols = self
            .table::<SymbolRecord>()
            .map_err(|_| NameError::Unknown)?;
        let mut found = None;
        for program in symbols
            .iter()
            .filter(|symbol| symbol.flags & SYMBOL_PROGRAM != 0 && symbol.parent == NONE)
        {
            let program_name = self.string(program.canonical).unwrap_or("");
            if let Some((index, _)) = self.find_symbol(&format!("{program_name}.{canonical}"))
                && found.replace(index).is_some()
            {
                return Err(NameError::Ambiguous);
            }
        }
        found.ok_or(NameError::Unknown)
    }

    /// The number of the program instance `name` names, which its root symbol carries (§5.7) and
    /// its body's entry is found by; `None` for a name that names no program instance. The name
    /// compares as a canonical name.
    #[must_use]
    pub fn find_program_instance(&self, name: &str) -> Option<u32> {
        let (_, symbol) = self.find_symbol(&name.to_ascii_uppercase())?;
        (symbol.flags & SYMBOL_PROGRAM != 0 && symbol.parent == NONE && symbol.pou != NONE)
            .then_some(symbol.pou)
    }
}

impl<C: ModuleCode> NativeProgram<'_, C> {
    /// The value at process-image `address`: in the input image the next scan samples, the output
    /// image the last scan published, or the marker area.
    ///
    /// # Errors
    ///
    /// The [`RuntimeError`] of an address that is no flat address of its image.
    pub fn read_address(&self, address: &IoAddress) -> Result<Value, RuntimeError> {
        match address.area {
            IoArea::Input => read_flat_image(&self.inputs, address),
            IoArea::Output => read_flat_image(&self.outputs, address),
            IoArea::Memory => read_flat_image(&self.markers(), address),
        }
    }

    /// Writes `value` at process-image `address`: into the input image the next scan samples, the
    /// output image, or the marker area.
    ///
    /// # Errors
    ///
    /// The [`RuntimeError`] of an address that is no flat address of its image, of a value of another
    /// size than the address's, or of a marker address outside the program's marker area.
    pub fn write_address(&mut self, address: &IoAddress, value: Value) -> Result<(), RuntimeError> {
        match address.area {
            IoArea::Input => write_flat_image(&mut self.inputs, address, value),
            IoArea::Output => write_flat_image(&mut self.outputs, address, value),
            IoArea::Memory => {
                let mut markers = self.markers();
                write_flat_image(&mut markers, address, value)?;
                if self.write(trust_plc_abi::AREA_MARKERS, 0, &markers) {
                    Ok(())
                } else {
                    Err(RuntimeError::IoAddress(
                        "the address lies outside the program's marker area".into(),
                    ))
                }
            }
        }
    }

    /// A copy of the marker area.
    fn markers(&self) -> alloc::vec::Vec<u8> {
        let mut markers = vec![0u8; self.area_length(trust_plc_abi::AREA_MARKERS)];
        self.read(trust_plc_abi::AREA_MARKERS, 0, &mut markers);
        markers
    }
}

/// A compiled program's variables and process images as the Live Values of a cycle boundary reach
/// them (specification 44 §13.3; specification 11, runtime control I/O and variable mutation
/// contract): a global or a qualified variable by its name, a retained variable by its retain-map
/// key, a variable of an instance by the instance's symbol, which is the instance's identity, and
/// the variable's name, and the process images by address. Each write converts the value as
/// [`NativeProgram::write_symbol`] does and leaves the variable as it was when it is refused. The
/// protocol clients of a computer read a variable by its name too, at the same boundaries.
pub trait CompiledVariables {
    /// The value of the global or the qualified variable `name`.
    ///
    /// # Errors
    ///
    /// [`RuntimeError::UndefinedVariable`] for a name that reaches no single variable, and the
    /// error of a variable that holds no value, an instance.
    fn read_named(&self, name: &str) -> Result<Value, RuntimeError>;

    /// Writes `value` to the global or the qualified variable `name`.
    ///
    /// # Errors
    ///
    /// [`RuntimeError::UndefinedVariable`] for a name that reaches no single variable, and the
    /// error of a value the variable's type refuses.
    fn write_named(&mut self, name: &str, value: &Value) -> Result<(), RuntimeError>;

    /// Writes `value` to the retained variable whose retain-map key is `label`.
    ///
    /// # Errors
    ///
    /// As [`NativeProgram::write_retained`].
    fn write_retained(&mut self, label: &str, value: &Value) -> Result<(), RuntimeError>;

    /// Writes `value` to the variable `member` of the instance whose symbol is `instance`.
    ///
    /// # Errors
    ///
    /// [`RuntimeError::UndefinedVariable`] for an instance symbol the program does not hold,
    /// [`RuntimeError::UndefinedField`] for a member the instance does not declare, and the error
    /// of a value the variable's type refuses.
    fn write_member(
        &mut self,
        instance: u32,
        member: &str,
        value: &Value,
    ) -> Result<(), RuntimeError>;

    /// Writes `value` at the process-image `address`.
    ///
    /// # Errors
    ///
    /// As [`NativeProgram::write_address`].
    fn write_image(&mut self, address: &IoAddress, value: Value) -> Result<(), RuntimeError>;
}

impl<C: ModuleCode> ProgramMemory for NativeProgram<'_, C> {
    fn image(&self) -> Image<'_> {
        NativeProgram::image(self)
    }

    fn read_area(&self, id: u8, offset: usize, out: &mut [u8]) -> bool {
        self.read(id, offset, out)
    }

    fn write_area(&mut self, id: u8, offset: usize, data: &[u8]) -> bool {
        self.write(id, offset, data)
    }

    fn write_image(&mut self, address: &IoAddress, value: Value) -> Result<(), RuntimeError> {
        self.write_address(address, value)
    }
}

/// Every program memory takes Live Values' writes the same way: between scans, and while a
/// debugger holds a scan (specification 44 §13.2, §13.3).
/// An integer that Live Values writes is an untyped literal, which takes the declared type of its
/// variable or of the variable bound at its address (specification 11, "An admitted integer is an
/// untyped literal"; specification 44 §13.3).
impl<M: ProgramMemory + ?Sized> CompiledVariables for M {
    fn read_named(&self, name: &str) -> Result<Value, RuntimeError> {
        let image = self.image();
        let index = image
            .find_variable(name)
            .map_err(|_| RuntimeError::UndefinedVariable(name.into()))?;
        let (place, type_index) = symbol_place(&image, index)?;
        read_place(self, place, type_index)
    }

    fn write_named(&mut self, name: &str, value: &Value) -> Result<(), RuntimeError> {
        let image = self.image();
        let index = image
            .find_variable(name)
            .map_err(|_| RuntimeError::UndefinedVariable(name.into()))?;
        let (place, type_index) = symbol_place(&image, index)?;
        write_admitted(self, place, type_index, value)
    }

    fn write_retained(&mut self, label: &str, value: &Value) -> Result<(), RuntimeError> {
        let (place, type_index) = retained_place(&self.image(), label)?;
        write_admitted(self, place, type_index, value)
    }

    fn write_member(
        &mut self,
        instance: u32,
        member: &str,
        value: &Value,
    ) -> Result<(), RuntimeError> {
        let (place, type_index) = member_place(&self.image(), instance, member)?;
        write_admitted(self, place, type_index, value)
    }

    fn write_image(&mut self, address: &IoAddress, value: Value) -> Result<(), RuntimeError> {
        let value = literal::for_address(&self.image(), address, value)?;
        ProgramMemory::write_image(self, address, value)
    }
}

/// Writes `value`, an integer literal typed for the variable first, at `place`.
fn write_admitted<M: ProgramMemory + ?Sized>(
    memory: &mut M,
    place: Place,
    type_index: u32,
    value: &Value,
) -> Result<(), RuntimeError> {
    match literal::for_variable(&memory.image(), type_index, value)? {
        Some(typed) => write_place(memory, place, type_index, &typed),
        None => write_place(memory, place, type_index, value),
    }
}

/// The boundaries of a compiled program's scan at which Live Values reach its variables
/// (specification 44 §13.3), as the engine's boundaries do (specification 13, Live Values mutation
/// lifetime).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompiledAccessPoint {
    /// The cycle start, before the program runs: the queued variable writes and the variable
    /// forces.
    CycleStart,
    /// After the drivers read the inputs, before the program samples them: the queued image writes
    /// and the forces of the input and marker images, so a forced input holds over the physical
    /// one.
    AfterInputs,
    /// After the scan, before the outputs reach the drivers and the snapshot: the forces of the
    /// output image.
    BeforeOutputs,
    /// A debugger's hold of the scan ends, before the held statement runs: the variable writes
    /// queued during the hold and the variable forces, as at a cycle start (specification 44
    /// §13.2).
    HoldEnd,
}
