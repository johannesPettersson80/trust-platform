use trust_hir::types::TypeRegistry;
use trust_runtime::debug::DebugRuntimeContext;
use trust_runtime::eval::EvalContext;
use trust_runtime::memory::VariableStorage;
use trust_runtime::value::{DateTimeProfile, Duration};

pub fn make_context<'a>(
    storage: &'a mut VariableStorage,
    registry: &'a TypeRegistry,
) -> EvalContext<'a> {
    EvalContext {
        storage,
        registry,
        initializer_catalog: None,
        profile: DateTimeProfile::default(),
        now: Duration::ZERO,
        debug: None,
        call_depth: 0,
        functions: None,
        stdlib: None,
        function_blocks: None,
        classes: None,
        using: None,
        access: None,
        current_instance: None,
        return_name: None,
        loop_depth: 0,
        execution_deadline: None,
    }
}

#[allow(dead_code)]
pub fn make_debug_context<'a>(
    storage: &'a mut VariableStorage,
    registry: &'a TypeRegistry,
) -> DebugRuntimeContext<'a> {
    DebugRuntimeContext {
        storage,
        registry,
        stdlib: None,
        profile: DateTimeProfile::default(),
        current_instance: None,
        now: Duration::ZERO,
    }
}
