/// Evaluation context shared across expression and statement execution.
pub struct EvalContext<'a> {
    pub storage: &'a mut VariableStorage,
    pub registry: &'a TypeRegistry,
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
    pub pause_requested: bool,
    pub execution_deadline: Option<std::time::Instant>,
}

/// Parameter declaration for POUs.
#[derive(Debug, Clone)]
pub struct Param {
    pub name: SmolStr,
    pub type_id: TypeId,
    pub direction: ParamDirection,
    pub address: Option<IoAddress>,
    pub default: Option<expr::Expr>,
}

/// Variable declaration with optional initializer.
#[derive(Debug, Clone)]
pub struct VarDef {
    pub name: SmolStr,
    pub type_id: TypeId,
    pub initializer: Option<expr::Expr>,
    pub retain: crate::RetainPolicy,
    pub static_storage: bool,
    pub external: bool,
    pub constant: bool,
    pub address: Option<IoAddress>,
}

/// Function definition (used by tests and runtime).
#[derive(Debug, Clone)]
pub struct FunctionDef {
    pub name: SmolStr,
    pub return_type: TypeId,
    pub params: Vec<Param>,
    pub locals: Vec<VarDef>,
    pub static_locals: Vec<VarDef>,
    pub using: Vec<SmolStr>,
    pub body: Vec<stmt::Stmt>,
}

/// Base type for a function block.
#[derive(Debug, Clone)]
pub enum FunctionBlockBase {
    FunctionBlock(SmolStr),
    Class(SmolStr),
}

/// Function block definition (used by tests and runtime).
#[derive(Debug, Clone)]
pub struct FunctionBlockDef {
    pub name: SmolStr,
    pub base: Option<FunctionBlockBase>,
    pub params: Vec<Param>,
    pub vars: Vec<VarDef>,
    pub temps: Vec<VarDef>,
    pub using: Vec<SmolStr>,
    pub methods: Vec<MethodDef>,
    pub body: Vec<stmt::Stmt>,
}

/// Method definition for classes and function blocks.
#[derive(Debug, Clone)]
pub struct MethodDef {
    pub name: SmolStr,
    pub return_type: Option<TypeId>,
    pub params: Vec<Param>,
    pub locals: Vec<VarDef>,
    pub static_locals: Vec<VarDef>,
    pub using: Vec<SmolStr>,
    pub body: Vec<stmt::Stmt>,
}

/// Class definition (used by tests and runtime).
#[derive(Debug, Clone)]
pub struct ClassDef {
    pub name: SmolStr,
    pub base: Option<SmolStr>,
    pub vars: Vec<VarDef>,
    pub using: Vec<SmolStr>,
    pub methods: Vec<MethodDef>,
}

/// Interface definition (used for metadata and bytecode emission).
#[derive(Debug, Clone)]
pub struct InterfaceDef {
    pub name: SmolStr,
    pub base: Option<SmolStr>,
    pub using: Vec<SmolStr>,
    pub methods: Vec<MethodDef>,
}

/// Call argument value.
#[derive(Debug, Clone)]
pub enum ArgValue {
    Expr(expr::Expr),
    Target(expr::LValue),
}

/// Named call argument.
#[derive(Debug, Clone)]
pub struct CallArg {
    pub name: Option<SmolStr>,
    pub value: ArgValue,
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
    FunctionBlock,
}
