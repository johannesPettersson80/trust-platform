//! A compiled program's variables by the names a test gives them, and its process images by
//! address (specification 44 §14.2; specification 11 §6.8.1, "Stepped scans"): what the
//! conformance runner and a stepped scan write before a scan and read after it.

use alloc::format;
use alloc::vec;
use core::fmt;

use super::image::{Image, SymbolRecord, NONE, SYMBOL_PROGRAM};
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
