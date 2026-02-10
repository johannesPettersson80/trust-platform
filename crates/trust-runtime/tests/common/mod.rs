use trust_hir::types::TypeRegistry;
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
        pause_requested: false,
        execution_deadline: None,
    }
}
