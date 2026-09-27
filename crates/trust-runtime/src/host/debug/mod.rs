//! Debugging and tracing support.

#![allow(missing_docs)]

mod breakpoints;
mod control;
pub mod dap;
mod hook;
mod resolve;
mod trace;
mod types;

pub(crate) use control::{
    numeric_control_value, resolve_control_parameter_path, ForcedVarTarget, PendingVarTarget,
};
pub use control::{
    ControlAction, ControlOutcome, DebugControl, DebugMode, IoRecordingResult, IoRecordingSpec,
    StepKind,
};
pub use dap::{DebugScope, DebugSource, DebugVariable, DebugVariableHandles, VariableHandle};
pub use hook::{DebugHook, DebugRuntimeContext, NoopDebugHook};
pub use resolve::{location_to_line_col, offset_to_line_col, resolve_breakpoint_location};
pub use types::{
    DebugBreakpoint, DebugLog, DebugSnapshot, DebugStop, DebugStopReason, HitCondition,
    LogFragment, RuntimeEvent, SourceLocation,
};

#[cfg(test)]
mod controller_binding_contract_tests {
    use super::control::resolve_control_parameter_path;
    use crate::memory::VariableStorage;
    use crate::value::{StructValue, Value};
    use indexmap::IndexMap;
    use smol_str::SmolStr;
    use std::sync::Arc;

    #[test]
    fn resolves_scalar_leaf_inside_nested_controller_struct() {
        let mut storage = VariableStorage::new();
        let controller = storage.create_instance("Controller");
        storage.set_instance_var(
            controller,
            "ParametersValue",
            Value::Struct(Arc::new(StructValue::from_untyped_parts(
                SmolStr::new("PID_PARAMETERS"),
                IndexMap::from([(SmolStr::new("ProportionalGain"), Value::LReal(2.5))]),
            ))),
        );
        let main = storage.create_instance("Main");
        storage.set_instance_var(main, "Controller", Value::Instance(controller));
        storage.set_global("Main", Value::Instance(main));

        let resolved = resolve_control_parameter_path(
            &storage,
            "Main.Controller.ParametersValue.ProportionalGain",
        )
        .expect("nested controller scalar leaf should resolve");
        assert_eq!(resolved.value, Value::LReal(2.5));
        assert!(storage.write_by_ref(resolved.reference, Value::LReal(3.5)));
        let observed = resolve_control_parameter_path(
            &storage,
            "Main.Controller.ParametersValue.ProportionalGain",
        )
        .expect("nested controller scalar leaf should remain resolvable");
        assert_eq!(observed.value, Value::LReal(3.5));
    }
}
