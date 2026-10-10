//! Hosted metadata, clock and profiling adapters for the shared call implementation.
use crate::{
    error::RuntimeError,
    memory::InstanceId,
    stdlib::StandardLibrary,
    value::{DateTimeProfile, DateTimeValue, Duration, Value, ValueRef},
    Runtime,
};
use trust_runtime_core::vm::hosted::call::context::{
    BuiltinParam, BuiltinParamDirection, CallContext, RegisterCallOpKind, RegisterValueOpKind,
    VmEdgeInput,
};
use trust_runtime_core::vm::hosted::{dispatch::VmPouStackResult, module::VmModule, VmFrame};

impl CallContext for Runtime {
    fn profile(&self) -> &DateTimeProfile {
        &self.profile
    }
    fn current_time(&self) -> Duration {
        self.current_time
    }
    fn stdlib(&self) -> &StandardLibrary {
        &self.stdlib
    }
    fn current_dt(&mut self) -> Result<DateTimeValue, RuntimeError> {
        match crate::stdlib::time::runtime_clock_value("CURRENT_DT", self.current_time)? {
            Value::Dt(value) => Ok(value),
            _ => Err(RuntimeError::TypeMismatch),
        }
    }
    fn builtin_params(&self, key: &str) -> Option<Vec<BuiltinParam>> {
        self.function_blocks().get(key).map(|fb| {
            fb.params
                .iter()
                .map(|p| BuiltinParam {
                    name: p.name.clone(),
                    direction: match p.direction {
                        trust_hir::symbols::ParamDirection::In => BuiltinParamDirection::In,
                        trust_hir::symbols::ParamDirection::Out => BuiltinParamDirection::Out,
                        trust_hir::symbols::ParamDirection::InOut => BuiltinParamDirection::InOut,
                    },
                })
                .collect()
        })
    }
    fn edge_inputs(&self, owner: &str) -> Vec<VmEdgeInput> {
        self.edge_inputs
            .get(owner)
            .into_iter()
            .flatten()
            .map(|p| VmEdgeInput {
                name: p.name.clone(),
                phase_name: crate::program_model::edge_phase_storage_name(owner, &p.name),
                rising: matches!(p.qualifier, trust_hir::symbols::EdgeQualifier::Rising),
            })
            .collect()
    }
    fn suspend_frame(&mut self, _frame: &mut VmFrame) -> Result<(), RuntimeError> {
        Ok(())
    }
    fn resume_frame(&mut self, _frame: &mut VmFrame) -> Result<(), RuntimeError> {
        Ok(())
    }
    fn record_call_op(&mut self, kind: RegisterCallOpKind) {
        self.vm_register_profile.record_call_op(kind);
    }
    fn record_value_op(&mut self, kind: RegisterValueOpKind) {
        self.vm_register_profile.record_value_op(kind);
    }
    fn check_native_write(
        &self,
        _frame: Option<&VmFrame>,
        _reference: &ValueRef,
        _value: &Value,
    ) -> Result<(), RuntimeError> {
        Ok(())
    }
    fn check_builtin_call(&self, _instance: InstanceId) -> Result<(), RuntimeError> {
        Ok(())
    }
    fn try_execute_optimized(
        &mut self,
        module: &VmModule,
        pou_id: u32,
        instance: Option<InstanceId>,
        initial_locals: Option<&[Value]>,
        capture_return: bool,
        depth: u32,
        entry: super::budget::ExecutionEntry,
    ) -> Result<Option<VmPouStackResult>, RuntimeError> {
        super::register_ir::try_execute_pou_with_register_ir_with_locals(
            self,
            module,
            pou_id,
            instance,
            initial_locals,
            capture_return,
            depth,
            entry,
        )
        .map(|result| {
            result.map(|r| VmPouStackResult {
                return_value: r.return_value,
                locals: r.locals,
            })
        })
    }
}
