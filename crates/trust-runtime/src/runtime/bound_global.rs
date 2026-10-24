//! The global a protocol client's point reads into or writes from, and the program's globals at a
//! scan boundary as the client reaches them (specification 11's ADS and OPC UA clients;
//! specification 44 §13.3): a program on the bytecode engine by a storage reference, a compiled
//! program by the global's name, which its symbol table resolves at each access.

use smol_str::SmolStr;
use trust_runtime_core::native::variables::CompiledVariables;
use trust_runtime_core::vm::CycleAccess;

use crate::error::RuntimeError;
use crate::memory::VariableStorage;
use crate::value::{Value, ValueRef};

/// The global a protocol client's point is bound to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BoundGlobal {
    /// A global of a program on the bytecode engine, by its storage reference.
    Engine(ValueRef),
    /// A global of a compiled program, by its declared name.
    Compiled(SmolStr),
}

impl BoundGlobal {
    /// The storage reference of a global of the bytecode engine.
    ///
    /// # Errors
    ///
    /// [`RuntimeError::InvalidExecutionState`] for a compiled program's global, which no storage
    /// holds.
    pub fn reference(&self) -> Result<&ValueRef, RuntimeError> {
        match self {
            Self::Engine(reference) => Ok(reference),
            Self::Compiled(_) => Err(RuntimeError::InvalidExecutionState),
        }
    }

    /// The declared name of a compiled program's global.
    ///
    /// # Errors
    ///
    /// [`RuntimeError::InvalidExecutionState`] for a global of the bytecode engine.
    pub fn name(&self) -> Result<&str, RuntimeError> {
        match self {
            Self::Engine(_) => Err(RuntimeError::InvalidExecutionState),
            Self::Compiled(name) => Ok(name),
        }
    }
}

/// The program's globals at a scan boundary, as a protocol client reads and writes them.
pub(crate) trait GlobalPort {
    /// The value of the global `target`.
    fn read(&mut self, target: &BoundGlobal) -> Result<Value, RuntimeError>;

    /// Writes `value` to the global `target`.
    fn write(&mut self, target: &BoundGlobal, value: Value) -> Result<(), RuntimeError>;
}

/// The bytecode engine's checked cycle owner.
impl GlobalPort for CycleAccess<'_, '_> {
    fn read(&mut self, target: &BoundGlobal) -> Result<Value, RuntimeError> {
        self.read_reference(target.reference()?)
    }

    fn write(&mut self, target: &BoundGlobal, value: Value) -> Result<(), RuntimeError> {
        self.write_reference(target.reference()?.as_view(), value)
    }
}

/// A compiled program's variables at the boundaries of its scan (specification 44 §13.3).
impl GlobalPort for dyn CompiledVariables + '_ {
    fn read(&mut self, target: &BoundGlobal) -> Result<Value, RuntimeError> {
        self.read_named(target.name()?)
    }

    fn write(&mut self, target: &BoundGlobal, value: Value) -> Result<(), RuntimeError> {
        self.write_named(target.name()?, &value)
    }
}

/// The bytecode engine's storage outside a cycle.
impl GlobalPort for VariableStorage {
    fn read(&mut self, target: &BoundGlobal) -> Result<Value, RuntimeError> {
        self.read_by_ref(target.reference()?.clone())
            .cloned()
            .ok_or(RuntimeError::TypeMismatch)
    }

    fn write(&mut self, target: &BoundGlobal, value: Value) -> Result<(), RuntimeError> {
        self.write_by_ref_ref(target.reference()?, value)
            .then_some(())
            .ok_or(RuntimeError::TypeMismatch)
    }
}

/// A stand-in for a compiled program's globals in the protocol clients' tests.
#[cfg(test)]
pub(crate) mod test_support {
    use std::collections::BTreeMap;

    use trust_runtime_core::io_address::IoAddress;
    use trust_runtime_core::native::variables::CompiledVariables;

    use crate::error::RuntimeError;
    use crate::value::Value;

    /// The globals of a stand-in compiled program, by the names its symbol table would resolve; it
    /// holds no retained label, instance or process image.
    pub(crate) struct NamedGlobals(pub(crate) BTreeMap<&'static str, Value>);

    impl CompiledVariables for NamedGlobals {
        fn read_named(&self, name: &str) -> Result<Value, RuntimeError> {
            self.0
                .get(name)
                .cloned()
                .ok_or_else(|| RuntimeError::UndefinedVariable(name.into()))
        }

        fn write_named(&mut self, name: &str, value: &Value) -> Result<(), RuntimeError> {
            let slot = self
                .0
                .get_mut(name)
                .ok_or_else(|| RuntimeError::UndefinedVariable(name.into()))?;
            *slot = value.clone();
            Ok(())
        }

        fn write_retained(&mut self, label: &str, _value: &Value) -> Result<(), RuntimeError> {
            Err(RuntimeError::UndefinedVariable(label.into()))
        }

        fn write_member(
            &mut self,
            _instance: u32,
            member: &str,
            _value: &Value,
        ) -> Result<(), RuntimeError> {
            Err(RuntimeError::UndefinedField(member.into()))
        }

        fn write_image(&mut self, _address: &IoAddress, _value: Value) -> Result<(), RuntimeError> {
            Err(RuntimeError::InvalidExecutionState)
        }
    }
}
