//! The global a protocol client's point reads into or writes from, and the program's globals at a
//! scan boundary as the client reaches them (specification 11's ADS and OPC UA clients;
//! specification 44 §13.3): the global's name, which the program's symbol table resolves at each
//! access.

use smol_str::SmolStr;
use trust_runtime_core::native::variables::CompiledVariables;

use crate::error::RuntimeError;
use crate::value::Value;

/// The global a protocol client's point is bound to, by its declared name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundGlobal(pub SmolStr);

impl BoundGlobal {
    /// The global's declared name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.0
    }
}

/// The program's globals at a scan boundary, as a protocol client reads and writes them.
pub(crate) trait GlobalPort {
    /// The value of the global `target`.
    fn read(&mut self, target: &BoundGlobal) -> Result<Value, RuntimeError>;

    /// Writes `value` to the global `target`.
    fn write(&mut self, target: &BoundGlobal, value: Value) -> Result<(), RuntimeError>;
}

/// A compiled program's variables at the boundaries of its scan (specification 44 §13.3).
impl GlobalPort for dyn CompiledVariables + '_ {
    fn read(&mut self, target: &BoundGlobal) -> Result<Value, RuntimeError> {
        self.read_named(target.name())
    }

    fn write(&mut self, target: &BoundGlobal, value: Value) -> Result<(), RuntimeError> {
        self.write_named(target.name(), &value)
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
