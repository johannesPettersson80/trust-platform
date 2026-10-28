//! A compiled program's values as a computer's services reach them (specification 44 §8.8, §13.3):
//! Control Studio, the simulated plants, the HMI, `Comm.Link` and the mesh read the scan's published
//! copy ([`CompiledState`]) or, at an access point, the program itself, and write the program only at
//! an access point, through the runtime's checks. A value is found by its path, or from an instance a
//! service found before, through the image's symbol and type tables; nothing is decoded that is not
//! asked for, and no copy of the program's storage is kept beside it.

use smol_str::SmolStr;
use trust_runtime_core::native::image::Image;
use trust_runtime_core::native::memory::{read_place, type_name};
use trust_runtime_core::native::reflect::{
    element_of, member_of, resolve_path, type_implements, type_is_a, walk_instances,
};
pub use trust_runtime_core::native::reflect::{Located, PathError, Walk};
use trust_runtime_core::native::variables::CompiledVariables;

use super::compiled::CompiledState;
use crate::error::RuntimeError;
use crate::value::Value;

/// What a service reads of a compiled program: the scan's published copy, or the program itself at
/// an access point.
pub trait ProgramRead {
    /// The image whose symbol and type tables the program's values are found through.
    fn program_image(&self) -> Image<'_>;

    /// The value at `at`.
    ///
    /// # Errors
    ///
    /// The error of a place that holds no value, an instance.
    fn read_at(&self, at: &Located) -> Result<Value, RuntimeError>;

    /// Where the variable, member or element `path` names lies (`Main.Pumps[2].Running`).
    ///
    /// # Errors
    ///
    /// The [`PathError`] that says why the path names nothing.
    fn locate(&self, path: &str) -> Result<Located, PathError> {
        resolve_path(&self.program_image(), path)
    }

    /// The value of the variable, member or element `path` names; `None` for a path that names
    /// nothing or a value that is none, an instance.
    fn value(&self, path: &str) -> Option<Value> {
        let at = self.locate(path).ok()?;
        self.read_at(&at).ok()
    }

    /// The member `name` of the value at `at`, compared without case.
    fn member(&self, at: &Located, name: &str) -> Option<Located> {
        member_of(&self.program_image(), at, name).ok().flatten()
    }

    /// The element `indices` of the array at `at`.
    fn element(&self, at: &Located, indices: &[i64]) -> Option<Located> {
        element_of(&self.program_image(), at, indices)
            .ok()
            .flatten()
    }

    /// The IEC name of the type of the value at `at` (`Comm.Link`, `INT`, `ARRAY[1..3] OF REAL`).
    fn type_name_of(&self, at: &Located) -> SmolStr {
        type_name(&self.program_image(), at.type_index)
    }

    /// Whether the instance at `at` is of the function block or class `name` or of one derived from
    /// it, compared without case.
    fn is_a(&self, at: &Located, name: &str) -> bool {
        type_is_a(&self.program_image(), at.type_index, name).unwrap_or(false)
    }

    /// Whether the instance at `at` implements the interface `name`, compared without case.
    fn implements(&self, at: &Located, name: &str) -> bool {
        type_implements(&self.program_image(), at.type_index, name).unwrap_or(false)
    }

    /// Visits every function block and class instance of the program where it is declared, with
    /// its path, in declaration order, as [`walk_instances`] walks them.
    fn instances(&self, visit: &mut dyn FnMut(&str, Located) -> Walk) {
        // A table that does not read has no instances to visit.
        let _ = walk_instances(&self.program_image(), visit);
    }
}

/// What a service writes at an access point: the program itself, through the checks of a Live
/// Values write (specification 44 §13.3).
pub trait ProgramWrite: ProgramRead {
    /// Writes `value` at `at`, converted to the place's type; the place keeps its value when the
    /// write is refused.
    ///
    /// # Errors
    ///
    /// The error of a value the place's type refuses.
    fn write_at(&mut self, at: &Located, value: &Value) -> Result<(), RuntimeError>;
}

impl ProgramRead for CompiledState {
    fn program_image(&self) -> Image<'_> {
        trust_runtime_core::native::memory::ProgramMemory::image(self)
    }

    fn read_at(&self, at: &Located) -> Result<Value, RuntimeError> {
        read_place(self, at.place, at.type_index)
    }
}

impl ProgramRead for dyn CompiledVariables + '_ {
    fn program_image(&self) -> Image<'_> {
        CompiledVariables::program_image(self)
    }

    fn read_at(&self, at: &Located) -> Result<Value, RuntimeError> {
        self.read_located(at)
    }
}

impl ProgramWrite for dyn CompiledVariables + '_ {
    fn write_at(&mut self, at: &Located, value: &Value) -> Result<(), RuntimeError> {
        self.write_located(at, value)
    }
}

/// The program at an access point, as a service writes it: wraps the access point's variables so
/// that a function taking `&mut dyn ProgramWrite` takes them (`&mut Access(variables)`).
pub struct Access<'a>(pub &'a mut dyn CompiledVariables);

impl ProgramRead for Access<'_> {
    fn program_image(&self) -> Image<'_> {
        CompiledVariables::program_image(&*self.0)
    }

    fn read_at(&self, at: &Located) -> Result<Value, RuntimeError> {
        self.0.read_located(at)
    }
}

impl ProgramWrite for Access<'_> {
    fn write_at(&mut self, at: &Located, value: &Value) -> Result<(), RuntimeError> {
        self.0.write_located(at, value)
    }
}

/// A program read where a function takes `&dyn ProgramRead`: the program as a scan left it, or any
/// other memory of a compiled program (`&View(program)`).
pub struct View<'a>(pub &'a dyn CompiledVariables);

impl ProgramRead for View<'_> {
    fn program_image(&self) -> Image<'_> {
        CompiledVariables::program_image(self.0)
    }

    fn read_at(&self, at: &Located) -> Result<Value, RuntimeError> {
        self.0.read_located(at)
    }
}
