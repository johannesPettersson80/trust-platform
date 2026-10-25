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
    /// Declared initial values of plain (non-constant) variables
    /// (docs/IEC_DEVIATIONS.md DEV-033). Never consulted by ordinary
    /// compile-time-constant evaluation; only declaration-size lowering
    /// builds the size-evaluation view that merges them in.
    declared_sizes: IndexMap<SmolStr, Value>,
    /// Last-segment index over the size view's merged values, built only by
    /// `size_evaluation_view` (IEC_DEVIATIONS 2026-10-25): a reference that
    /// fails exact lookup may still match one unique leaf table-wide.
    /// `None` marks an ambiguous leaf; ordinary const sets never build it.
    size_view_leaf_index: Option<std::collections::HashMap<SmolStr, Option<SmolStr>>>,
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

    pub(crate) fn insert_declared_size(&mut self, name: &str, value: Value) {
        self.declared_sizes.insert(Self::key(name), value);
    }

    /// Whether the size-evaluation view can resolve anything the strict set
    /// cannot: declared sizes exist, or namespace-qualified constants are
    /// present whose leaves the dotted-reference tolerance may match
    /// (IEC_DEVIATIONS 2026-10-25). Projects with neither keep the strict
    /// error without building a view.
    pub(crate) fn size_fallback_applicable(&self) -> bool {
        !self.declared_sizes.is_empty() || self.global_values.keys().any(|key| key.contains('.'))
    }

    pub(crate) fn absorb_declared_sizes(&mut self, declared: Self) {
        for (name, value) in declared.declared_sizes {
            self.declared_sizes.insert(name, value);
        }
    }

    /// The view declaration-size lowering evaluates against: true constants
    /// plus the declared initial values of plain variables, indexed by
    /// unique leaf for the flattened-library tolerance of dotted size
    /// references (IEC_DEVIATIONS 2026-10-25).
    pub(crate) fn size_evaluation_view(&self) -> Self {
        let mut view = self.clone();
        view.global_values.extend(self.declared_sizes.clone());
        let mut index: std::collections::HashMap<SmolStr, Option<SmolStr>> = Default::default();
        for key in view.global_values.keys() {
            let leaf = match key.rsplit_once('.') {
                Some((_, leaf)) => leaf,
                None => key.as_str(),
            };
            match index.entry(SmolStr::new(leaf)) {
                std::collections::hash_map::Entry::Vacant(slot) => {
                    slot.insert(Some(key.clone()));
                }
                std::collections::hash_map::Entry::Occupied(mut slot) => {
                    if slot.get().is_some() {
                        slot.insert(None);
                    }
                }
            }
        }
        view.size_view_leaf_index = Some(index);
        view
    }

    fn lookup(&self, name: &str, using: &[SmolStr]) -> Option<Value> {
        let key = Self::key(name);
        if self.local_reserved.contains(&key) {
            return self.local_values.get(&key).cloned();
        }
        if name.contains('.') {
            return self
                .global_values
                .get(&key)
                .cloned()
                .or_else(|| self.lookup_unique_leaf(&key));
        }
        for namespace in using {
            let qualified = Self::key(&format!("{namespace}.{name}"));
            if self.global_reserved.contains(&qualified) {
                return self.global_values.get(&qualified).cloned();
            }
        }
        self.global_values.get(&key).cloned()
    }

    /// Unique-leaf fallback for dotted size references in the size view
    /// (IEC_DEVIATIONS 2026-10-25): a reference naming a GVL level a
    /// library stub flattened away (`LogParameter.MAX_LOG_MSG_LEN` over
    /// namespace-level constants, or the fully qualified
    /// `WagoTypesLog.LogParameter.MAX_LOG_MSG_LEN`) resolves when exactly
    /// one entry carries the reference's last segment. Bare names never
    /// fall back — an unimported namespace's operand must stay unimported;
    /// ambiguous leaves stay unresolved.
    fn lookup_unique_leaf(&self, key: &SmolStr) -> Option<Value> {
        if !key.contains('.') {
            return None;
        }
        let index = self.size_view_leaf_index.as_ref()?;
        let leaf = match key.rsplit_once('.') {
            Some((_, leaf)) => leaf,
            None => key.as_str(),
        };
        let resolved = index.get(leaf)?;
        resolved
            .as_ref()
            .and_then(|full_key| self.global_values.get(full_key).cloned())
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
    /// The connections of the instance's inputs and outputs (IEC 61131-3 §6.8.2, Table 62 8a, 8b,
    /// 9a, 9b).
    pub(crate) connections: Vec<ProgramConnection>,
}

/// A connection of a program instance's input or output in its configuration (IEC 61131-3 §6.8.2,
/// Table 62 8a, 8b, 9a, 9b): `x1 := %IX1.1`, `x2 := w`, `out1 => w`.
#[derive(Debug, Clone)]
pub(crate) struct ProgramConnection {
    /// The program's input, connected with `:=`, or output, connected with `=>`.
    pub(crate) parameter: SmolStr,
    /// Whether the connection is of an output.
    pub(crate) output: bool,
    /// What the input reads or the output writes.
    pub(crate) end: ConnectionEnd,
}

/// The other end of a program connection.
#[derive(Debug, Clone)]
pub(crate) enum ConnectionEnd {
    /// A directly represented variable, with its text as written.
    Address { address: IoAddress, text: SmolStr },
    /// A global variable or an element of one, by its name, its members and constant indices; a
    /// name alone may also be a value of the parameter's enumeration.
    Global(Vec<AccessPart>),
    /// A constant.
    Constant(Expr),
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
    /// Original declaration/binding label for hosted I/O presentation only.
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
        /// The same place by the names of the program's symbols, as a compiled program reaches it.
        symbol: AccessSymbol,
    },
}

/// A place of the program by the names of its symbols (specification 44 §5.7): the variable the
/// path's names reach, and the path into that variable's value; `None` for a place within a
/// function block instance that an array or a structure holds, which no symbol names.
pub(crate) type AccessSymbol = Option<(SmolStr, Vec<crate::value::RefSegment>)>;

/// A `VAR_ACCESS` declaration as the test harness reads and writes it (specification 10 §4.1.2):
/// its name, the place it names by the program's symbols, its partial selection and whether it
/// takes writes.
#[derive(Debug, Clone)]
pub(crate) struct NamedAccess {
    pub(crate) name: SmolStr,
    pub(crate) symbol: AccessSymbol,
    pub(crate) partial: Option<crate::value::PartialAccess>,
    pub(crate) writable: bool,
}

#[derive(Clone)]
pub(crate) struct GlobalInit {
    pub(crate) name: SmolStr,
    pub(crate) type_id: TypeId,
    pub(crate) initializer: Option<Expr>,
    pub(crate) retain: crate::RetainPolicy,
    pub(crate) address: Option<SmolStr>,
    /// Declared in a `VAR_GLOBAL CONSTANT` block.
    pub(crate) constant: bool,
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
    /// Whether the POUs' statements are lowered. The compiled-program build reads only the
    /// declarations and generates the statements from the syntax tree (specification 44 §3).
    pub(crate) lower_bodies: bool,
}

pub(crate) struct LoweringInputs<'a> {
    pub(crate) profile: DateTimeProfile,
    pub(crate) file_id: u32,
    pub(crate) semantic_db: Option<&'a dyn SemanticDatabase>,
    pub(crate) semantic_file_id: Option<FileId>,
    pub(crate) statement_locations: &'a mut Vec<SourceLocation>,
    pub(crate) compile_time_consts: CompileTimeConsts,
    pub(crate) lower_bodies: bool,
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
            lower_bodies: true,
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
            lower_bodies: self.lower_bodies,
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
