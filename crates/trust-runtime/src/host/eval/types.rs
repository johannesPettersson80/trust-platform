/// Evaluation context shared across expression and statement execution.
pub(crate) struct EvalContext<'a> {
    pub storage: &'a mut VariableStorage,
    pub registry: &'a TypeRegistry,
    pub initializer_catalog: Option<&'a crate::program_model::InitializerCatalog>,
    pub profile: DateTimeProfile,
    pub now: Duration,
    pub debug: Option<&'a mut dyn crate::debug::DebugHook>,
    pub call_depth: u32,
    pub functions: Option<&'a IndexMap<SmolStr, FunctionDef>>,
    pub stdlib: Option<&'a StandardLibrary>,
    pub function_blocks: Option<&'a IndexMap<SmolStr, FunctionBlockDef>>,
    pub classes: Option<&'a IndexMap<SmolStr, ClassDef>>,
    pub using: Option<&'a [SmolStr]>,
    pub access: Option<&'a crate::memory::AccessMap>,
    pub current_instance: Option<InstanceId>,
    pub return_name: Option<SmolStr>,
    pub loop_depth: u32,
    pub execution_deadline: Option<std::time::Instant>,
}

#[derive(Debug, Clone)]
enum OutputBinding {
    Param {
        param: SmolStr,
        target: expr::LValue,
    },
    Value {
        target: expr::LValue,
        value: Value,
    },
}

struct PreparedBindings {
    should_execute: bool,
    param_values: Vec<(SmolStr, Value)>,
    out_targets: Vec<OutputBinding>,
}

#[derive(Debug, Clone, Copy)]
enum BindingMode {
    Function,
    FunctionBlock { instance_id: InstanceId },
}
