use indexmap::IndexMap;
use smol_str::SmolStr;

use crate::debug::SourceLocation;
use crate::io::IoAddress;
use crate::memory::IoArea;
use crate::program_model::Expr;
use crate::program_model::VarDef;
use crate::task::ProgramDef;
use crate::value::{DateTimeProfile, Value};
use trust_hir::db::{FileId, SemanticDatabase};
use trust_hir::TypeId;

use super::super::types::CompileError;

#[derive(Clone, Default)]
pub(crate) struct CompileTimeConsts {
    global_values: IndexMap<SmolStr, Value>,
    global_reserved: indexmap::IndexSet<SmolStr>,
    local_values: IndexMap<SmolStr, Value>,
    local_reserved: indexmap::IndexSet<SmolStr>,
}

impl CompileTimeConsts {
    fn key(name: &str) -> SmolStr {
        SmolStr::new(name.to_ascii_uppercase())
    }

    pub(crate) fn reserve_global(&mut self, name: &str) {
        self.global_reserved.insert(Self::key(name));
    }

    pub(crate) fn reserve_local(&mut self, name: &str) {
        self.local_reserved.insert(Self::key(name));
    }

    pub(crate) fn insert_global(&mut self, name: &str, value: Value) {
        self.global_values.insert(Self::key(name), value);
    }

    pub(crate) fn insert_local(&mut self, name: &str, value: Value) {
        self.local_values.insert(Self::key(name), value);
    }

    fn lookup(&self, name: &str, using: &[SmolStr]) -> Option<Value> {
        let key = Self::key(name);
        if self.local_reserved.contains(&key) {
            return self.local_values.get(&key).cloned();
        }
        if name.contains('.') {
            return self.global_values.get(&key).cloned();
        }
        for namespace in using {
            let qualified = Self::key(&format!("{namespace}.{name}"));
            if self.global_reserved.contains(&qualified) {
                return self.global_values.get(&qualified).cloned();
            }
        }
        self.global_values.get(&key).cloned()
    }
}

pub(crate) struct LoweredProgram {
    pub(crate) program: ProgramDef,
    pub(crate) globals: Vec<GlobalInit>,
    pub(crate) access: Vec<AccessDecl>,
}

pub(crate) struct ProgramVars {
    pub(crate) globals: Vec<GlobalInit>,
    pub(crate) vars: Vec<VarDef>,
    pub(crate) temps: Vec<VarDef>,
}

pub(crate) struct ConfigModel {
    pub(crate) resource_name: Option<SmolStr>,
    pub(crate) globals: Vec<GlobalInit>,
    pub(crate) tasks: Vec<crate::task::TaskConfig>,
    pub(crate) programs: Vec<ProgramInstanceConfig>,
    pub(crate) using: Vec<SmolStr>,
    pub(crate) access: Vec<AccessDecl>,
    pub(crate) config_inits: Vec<ConfigInit>,
}

pub(crate) struct ProgramInstanceConfig {
    pub(crate) name: SmolStr,
    pub(crate) type_name: SmolStr,
    pub(crate) task: Option<SmolStr>,
    pub(crate) retain: Option<crate::RetainPolicy>,
    pub(crate) fb_tasks: Vec<FbTaskBinding>,
}

#[derive(Debug, Clone)]
pub(crate) struct FbTaskBinding {
    pub(crate) path: AccessPath,
    pub(crate) task: SmolStr,
}

#[derive(Debug, Clone)]
pub(crate) enum AccessPath {
    Direct { address: IoAddress, text: SmolStr },
    Parts(Vec<AccessPart>),
}

#[derive(Debug, Clone)]
pub(crate) enum AccessPart {
    Name(SmolStr),
    Index(Vec<i64>),
    Partial(crate::value::PartialAccess),
}

#[derive(Debug, Clone)]
pub(crate) struct AccessDecl {
    pub(crate) name: SmolStr,
    pub(crate) path: AccessPath,
    pub(crate) writable: bool,
}

#[derive(Debug, Clone)]
pub(crate) struct ConfigInit {
    pub(crate) path: AccessPath,
    pub(crate) address: Option<IoAddress>,
    pub(crate) type_id: TypeId,
    pub(crate) initializer: Option<Expr>,
}

#[derive(Debug, Clone)]
pub(crate) enum ResolvedAccess {
    Direct(IoAddress),
    Variable {
        reference: crate::value::ValueRef,
        partial: Option<crate::value::PartialAccess>,
    },
}

#[derive(Clone)]
pub(crate) struct GlobalInit {
    pub(crate) name: SmolStr,
    pub(crate) type_id: TypeId,
    pub(crate) initializer: Option<Expr>,
    pub(crate) retain: crate::RetainPolicy,
    pub(crate) address: Option<SmolStr>,
}

#[derive(Clone)]
pub(crate) struct WildcardRequirement {
    pub(crate) name: SmolStr,
    pub(crate) reference: crate::value::ValueRef,
    pub(crate) area: IoArea,
}

pub(crate) struct LoweringContext<'a> {
    pub(crate) registry: &'a mut trust_hir::types::TypeRegistry,
    pub(crate) profile: DateTimeProfile,
    pub(crate) using: Vec<SmolStr>,
    pub(crate) file_id: u32,
    pub(crate) semantic_db: Option<&'a dyn SemanticDatabase>,
    pub(crate) semantic_file_id: Option<FileId>,
    pub(crate) statement_locations: &'a mut Vec<SourceLocation>,
    pub(crate) compile_time_consts: CompileTimeConsts,
}

pub(crate) struct LoweringInputs<'a> {
    pub(crate) profile: DateTimeProfile,
    pub(crate) file_id: u32,
    pub(crate) semantic_db: Option<&'a dyn SemanticDatabase>,
    pub(crate) semantic_file_id: Option<FileId>,
    pub(crate) statement_locations: &'a mut Vec<SourceLocation>,
    pub(crate) compile_time_consts: CompileTimeConsts,
}

impl<'a> LoweringInputs<'a> {
    pub(crate) fn new(
        profile: DateTimeProfile,
        file_id: u32,
        semantic_db: Option<&'a dyn SemanticDatabase>,
        semantic_file_id: Option<FileId>,
        statement_locations: &'a mut Vec<SourceLocation>,
        compile_time_consts: CompileTimeConsts,
    ) -> Self {
        Self {
            profile,
            file_id,
            semantic_db,
            semantic_file_id,
            statement_locations,
            compile_time_consts,
        }
    }

    pub(crate) fn context<'b>(
        &'b mut self,
        registry: &'b mut trust_hir::types::TypeRegistry,
        using: Vec<SmolStr>,
    ) -> LoweringContext<'b> {
        LoweringContext {
            registry,
            profile: self.profile,
            using,
            file_id: self.file_id,
            semantic_db: self.semantic_db,
            semantic_file_id: self.semantic_file_id,
            statement_locations: &mut *self.statement_locations,
            compile_time_consts: self.compile_time_consts.clone(),
        }
    }
}

impl LoweringContext<'_> {
    pub(crate) fn lookup_compile_time_const(&self, name: &str) -> Option<Value> {
        self.compile_time_consts.lookup(name, &self.using)
    }

    fn lookup_compile_time_value(&self, name: &str) -> Option<Value> {
        self.lookup_compile_time_const(name).or_else(|| {
            match self.registry.resolve_enum_value_by_name(name) {
                trust_hir::symbols::EnumValueResolution::Resolved(value) => {
                    Some(Value::LInt(value))
                }
                trust_hir::symbols::EnumValueResolution::NotFound
                | trust_hir::symbols::EnumValueResolution::Ambiguous => None,
            }
        })
    }

    pub(crate) fn register_compile_time_const(&mut self, name: &str, value: Value) {
        self.compile_time_consts.insert_local(name, value);
    }

    pub(crate) fn register_global_compile_time_const(&mut self, name: &str, value: Value) {
        self.compile_time_consts.insert_global(name, value);
    }

    pub(crate) fn eval_compile_time_const_expr(&self, expr: &Expr) -> Result<Value, CompileError> {
        crate::helper_eval::eval_const_expr_with_resolver_and_registry(
            expr,
            &self.profile,
            self.registry,
            &|name| self.lookup_compile_time_value(name),
        )
        .map_err(|err| CompileError::new(err.to_string()))
    }

    pub(crate) fn eval_compile_time_const_initializer(
        &self,
        expr: &Expr,
        type_id: TypeId,
    ) -> Result<Value, CompileError> {
        let value = self.eval_compile_time_const_expr(expr)?;
        crate::harness::initializer::coerce_evaluated_initializer_value(
            value,
            type_id,
            self.registry,
            &self.profile,
        )
    }
}
