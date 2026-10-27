//! The host HMI's typed, interface-constrained method entry (specification 39, §21.2). No HTTP
//! handler executes here: the call runs between scans through the kernel's method call of a
//! compiled program ([`CompiledRuntime::call_method`]); the HMI checks first that the instance
//! implements the interface and the interface declares the method.

use smol_str::SmolStr;
use trust_runtime_core::native::memory::{layout, Layout};
use trust_runtime_core::native::tool::tool_symbol;

use super::CompiledRuntime;
use crate::error::RuntimeError;
use crate::hmi::commands::write::path_text;
use crate::hmi::member_paths::{locate_member, MemberPath};
use crate::value::Value;

#[derive(Debug)]
pub(crate) enum InvocationError {
    /// Not called.
    Refused(String),
    /// The call started and failed; the resource has faulted.
    Fault(RuntimeError),
}

impl CompiledRuntime {
    /// Calls `method` of the instance at `path` with typed inputs, when its type implements
    /// `interface` and the interface declares the method; the instance's body does not run. The
    /// call draws on `budget` (statements and loop passes), which it reduces by the work it used.
    pub(crate) fn invoke_hmi_method(
        &mut self,
        path: &MemberPath,
        interface: &str,
        method: &str,
        inputs: &[(SmolStr, Value)],
        budget: &mut u32,
    ) -> Result<Value, InvocationError> {
        let capable = self.with_program_view(|program| {
            let at = locate_member(path, program)?;
            let image = program.program_image();
            let instance = matches!(layout(&image, at.type_index), Ok(Layout::Instance(_)));
            // An interface declares the method a tool calls when its tool symbol names it
            // (specification 44 §5.7, §13.3).
            Some(
                instance
                    && program.implements(&at, interface)
                    && image.find_symbol(&tool_symbol(interface, method)).is_some(),
            )
        });
        match capable {
            None => {
                return Err(InvocationError::Refused(
                    "equipment instance is unavailable".into(),
                ))
            }
            Some(false) => {
                return Err(InvocationError::Refused(
                    "HMI method is not declared by the required interface".into(),
                ))
            }
            Some(true) => {}
        }
        if *budget == 0 {
            return Err(InvocationError::Refused(
                "HMI method budget is exhausted".into(),
            ));
        }
        match self.call_method(&path_text(path), interface, method, inputs, *budget) {
            Ok((value, used)) => {
                *budget = budget.saturating_sub(used);
                Ok(value)
            }
            // A method that faulted has faulted the resource, as a faulted scan does.
            Err(error) if self.faulted() => Err(InvocationError::Fault(error)),
            Err(error) => Err(InvocationError::Refused(error.to_string())),
        }
    }

    /// Serves the HMI's method commands between scans, after a scan in which the program called
    /// `Hmi.Display` (specification 39, §21.2 item 2): one queued call, answered.
    ///
    /// # Errors
    ///
    /// The fault of a method that faulted. The resource has faulted then (the fault latched and
    /// the fault policy's output action applied), so the caller reports the error without
    /// applying it again.
    pub(crate) fn serve_hmi_methods(
        &mut self,
        exchange: &crate::hmi::exchange::HmiExchange,
    ) -> Result<(), RuntimeError> {
        exchange.methods.serve(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::harness::TestHarness;
    use crate::hmi::member_paths::member_value;

    const PROGRAM: &str = r#"
INTERFACE ICounter
METHOD Add : DINT
VAR_INPUT Amount : DINT; END_VAR
END_METHOD
END_INTERFACE
FUNCTION_BLOCK Counter IMPLEMENTS ICounter
VAR_OUTPUT Total : DINT; BodyCalls : DINT; END_VAR
BodyCalls := BodyCalls + 1;
METHOD PUBLIC Add : DINT
VAR_INPUT Amount : DINT; END_VAR
    Total := Total + Amount;
    Add := Total;
END_METHOD
METHOD PUBLIC HiddenFromCapability : DINT
    Total := 999;
    HiddenFromCapability := Total;
END_METHOD
END_FUNCTION_BLOCK
PROGRAM Main
VAR Items : ARRAY[1..2] OF Counter; END_VAR
END_PROGRAM
"#;

    fn value(harness: &TestHarness, path: &str) -> Option<Value> {
        let path = MemberPath::parse(path).unwrap();
        harness
            .runtime()
            .with_program_view(|program| member_value(&path, program))
    }

    #[test]
    fn typed_method_runs_on_selected_instance_without_executing_fb_body() {
        let mut harness = TestHarness::from_source(PROGRAM).unwrap();
        harness.cycle();
        let mut budget = 10000;
        let path = MemberPath::parse("Main.Items[2]").unwrap();
        let result = harness
            .runtime_mut()
            .invoke_hmi_method(
                &path,
                "ICounter",
                "Add",
                &[("Amount".into(), Value::DInt(3))],
                &mut budget,
            )
            .unwrap();
        assert_eq!(result, Value::DInt(3));
        for (path, expected) in [
            ("Main.Items[1].Total", 0),
            ("Main.Items[2].Total", 3),
            ("Main.Items[2].BodyCalls", 0),
        ] {
            assert_eq!(value(&harness, path), Some(Value::DInt(expected)));
        }
        assert!(budget < 10000);
    }

    #[test]
    fn methods_outside_capability_and_exhausted_budget_do_not_execute() {
        let mut harness = TestHarness::from_source(PROGRAM).unwrap();
        harness.cycle();
        let path = MemberPath::parse("Main.Items[1]").unwrap();
        let mut budget = 10000;
        assert!(harness
            .runtime_mut()
            .invoke_hmi_method(&path, "ICounter", "HiddenFromCapability", &[], &mut budget)
            .is_err());
        assert!(harness
            .runtime_mut()
            .invoke_hmi_method(&path, "IUnknown", "Add", &[], &mut budget)
            .is_err());
        budget = 0;
        assert!(harness
            .runtime_mut()
            .invoke_hmi_method(
                &path,
                "ICounter",
                "Add",
                &[("Amount".into(), Value::DInt(3))],
                &mut budget
            )
            .is_err());
        assert_eq!(value(&harness, "Main.Items[1].Total"), Some(Value::DInt(0)));
    }
}
