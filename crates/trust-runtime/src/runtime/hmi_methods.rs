//! The host HMI's typed, interface-constrained method entry (specification 39, §21.2). No HTTP
//! handler executes here: the call runs between scans through the shared engine's method entry.
use super::Runtime;
use crate::{
    error::RuntimeError,
    hmi::member_paths::{resolve_member, MemberPath},
    value::Value,
};
use smol_str::SmolStr;

#[derive(Debug)]
pub(crate) enum InvocationError {
    /// Not called.
    Refused(String),
    /// The call started and failed.
    Fault(RuntimeError),
}

impl Runtime {
    /// Calls `method` of the instance at `path` with typed inputs, when its type implements
    /// `interface` and the interface declares the method; the instance's body does not run. The
    /// call draws on `budget` (work units), which it reduces by the work it used.
    pub(crate) fn invoke_hmi_method(
        &mut self,
        path: &MemberPath,
        interface: &str,
        method: &str,
        inputs: &[(SmolStr, Value)],
        budget: &mut usize,
    ) -> Result<Value, InvocationError> {
        let instance_id = match resolve_member(path, self.storage()) {
            Some(Value::Instance(id)) => *id,
            _ => {
                return Err(InvocationError::Refused(
                    "equipment instance is unavailable".into(),
                ))
            }
        };
        let instance = self
            .storage()
            .get_instance(instance_id)
            .ok_or_else(|| InvocationError::Refused("equipment instance is unavailable".into()))?;
        if !crate::hmi::commands::capability_checks::implements_in(
            self.function_blocks(),
            self.classes(),
            self.interfaces(),
            &instance.type_name,
            interface,
        ) || crate::hmi::commands::capability_checks::declared_method_in(
            self.interfaces(),
            interface,
            method,
        )
        .is_none()
        {
            return Err(InvocationError::Refused(
                "HMI method is not declared by the required interface".into(),
            ));
        }
        if *budget == 0 {
            return Err(InvocationError::Refused(
                "HMI method budget is exhausted".into(),
            ));
        }
        let (value, used) = self
            .state
            .execute_instance_method(instance_id, method, inputs, *budget)
            .map_err(InvocationError::Fault)?;
        *budget = budget.saturating_sub(used);
        Ok(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::harness::TestHarness;

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
            assert_eq!(
                resolve_member(
                    &MemberPath::parse(path).unwrap(),
                    harness.runtime().storage()
                ),
                Some(&Value::DInt(expected))
            );
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
        assert_eq!(
            resolve_member(
                &MemberPath::parse("Main.Items[1].Total").unwrap(),
                harness.runtime().storage()
            ),
            Some(&Value::DInt(0))
        );
    }
}
