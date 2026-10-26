//! A tool's call of a method of a live function block or class instance of a compiled program on a
//! computer, between scans (specification 44 §13.3; specification 39 §21.2): the kernel makes the
//! call ([`NativeProgram::call_method`]), and a fault of the method faults the resource as a
//! faulted scan does.
//!
//! [`NativeProgram::call_method`]: trust_runtime_core::native::program::NativeProgram::call_method

use smol_str::SmolStr;
use trust_runtime_core::native::tool::MethodCallError;

use super::CompiledRuntime;
use crate::error::RuntimeError;
use crate::value::Value;
use crate::watchdog::FaultDecision;

impl CompiledRuntime {
    /// Calls method `method` of interface `interface` on the function block or class instance at
    /// `instance_path` (`Main.Pumps[2].Feed`), between scans at the program's logical time, with
    /// `inputs` by their names and an allowance of `allowance` statements and loop passes
    /// (specification 44 §13.3): the method a call through the interface reaches on the instance
    /// runs, the instance's body does not, and an input not given takes its declared initial value.
    /// Returns the method's result, `Value::Null` for a method without one, and the statements and
    /// loop passes it used.
    ///
    /// # Errors
    ///
    /// [`RuntimeError::ResourceFaulted`] while a fault is latched. The kernel's refusal of the call,
    /// which leaves the program as it was: an allowance of 0 or one the method uses up, a path that
    /// names nothing, an instance whose type does not implement the interface, a method the
    /// interface does not have or that takes another parameter than an input (`a host method call
    /// accepts only input parameters`), an input the method does not have, and a value its input
    /// refuses. The fault of the method, after the resource faulted as at a faulted scan: the fault
    /// latched and the fault policy's output action applied.
    pub fn call_method(
        &mut self,
        instance_path: &str,
        interface: &str,
        method: &str,
        inputs: &[(SmolStr, Value)],
        allowance: u32,
    ) -> Result<(Value, u32), RuntimeError> {
        if self.faulted() {
            return Err(RuntimeError::ResourceFaulted);
        }
        let now = self.current_time;
        match self.cycle.program_mut().call_method(
            instance_path,
            interface,
            method,
            inputs,
            allowance,
            now,
        ) {
            Ok(called) => Ok(called),
            Err(MethodCallError::Refused(error)) => Err(error),
            Err(MethodCallError::Fault(error)) => {
                Err(self.apply_fault(error, FaultDecision::from_fault_policy(self.fault_policy)))
            }
        }
    }
}
