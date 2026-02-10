use super::*;
use parking_lot::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

mod collector;
mod database;
mod helpers;
mod salsa_backend;

pub use salsa_backend::SalsaEventSnapshot;

pub(super) use helpers::{
    collect_program_instances, implements_clause_names, name_from_node, normalize_member_name,
    program_config_instance_and_type, qualified_name_parts, qualified_name_string,
    resolve_access_path_target, type_path_from_type_ref, var_block_is_constant,
    var_qualifier_from_block,
};

/// A file identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FileId(pub u32);

/// Input queries - these are the "leaves" that drive all computation.
pub trait SourceDatabase {
    /// Get the source text for a file.
    fn source_text(&self, file_id: FileId) -> Arc<String>;

    /// Set the source text for a file.
    fn set_source_text(&mut self, file_id: FileId, text: String);
}

/// Derived queries for semantic analysis.
pub trait SemanticDatabase: SourceDatabase {
    /// Get the symbol table for a file.
    fn file_symbols(&self, file_id: FileId) -> Arc<SymbolTable>;

    /// Resolve a name at a position.
    fn resolve_name(&self, file_id: FileId, name: &str) -> Option<SymbolId>;

    /// Get the type of the expression with the given expression ID.
    fn type_of(&self, file_id: FileId, expr_id: u32) -> TypeId;

    /// Find the expression ID that contains the given byte offset.
    fn expr_id_at_offset(&self, file_id: FileId, offset: u32) -> Option<u32>;

    /// Get all diagnostics for a file.
    fn diagnostics(&self, file_id: FileId) -> Arc<Vec<Diagnostic>>;

    /// Collect symbols + typecheck diagnostics in one pass.
    fn analyze(&self, file_id: FileId) -> Arc<FileAnalysis>;
}

/// The main database struct.
pub struct Database {
    sources: FxHashMap<FileId, Arc<String>>,
    salsa_state: Mutex<salsa_backend::SalsaState>,
    source_revision: AtomicU64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileAnalysis {
    pub symbols: Arc<SymbolTable>,
    pub diagnostics: Arc<Vec<Diagnostic>>,
}

impl Default for Database {
    fn default() -> Self {
        Self {
            sources: FxHashMap::default(),
            salsa_state: Mutex::new(salsa_backend::SalsaState::default()),
            source_revision: AtomicU64::new(1),
        }
    }
}

#[cfg(test)]
impl Database {
    pub(crate) fn new_with_salsa_observability() -> Self {
        Self {
            sources: FxHashMap::default(),
            salsa_state: Mutex::new(salsa_backend::SalsaState::with_event_observability(
                true, false,
            )),
            source_revision: AtomicU64::new(1),
        }
    }
}

impl std::fmt::Debug for Database {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let state = self.salsa_state.lock();
        f.debug_struct("Database")
            .field("sources", &self.sources.len())
            .field("salsa_sources", &state.sources.len())
            .field(
                "source_revision",
                &self.source_revision.load(Ordering::Relaxed),
            )
            .finish()
    }
}

pub(super) struct PendingType {
    pub(super) name: SmolStr,
    pub(super) range: TextRange,
    pub(super) scope_id: ScopeId,
}

#[derive(Clone)]
pub(super) struct ParamSignature {
    pub(super) name: SmolStr,
    pub(super) direction: ParamDirection,
    pub(super) type_id: TypeId,
}

#[derive(Clone)]
pub(super) struct MethodSignature {
    pub(super) name: SmolStr,
    pub(super) return_type: Option<TypeId>,
    pub(super) parameters: Vec<ParamSignature>,
    pub(super) visibility: Visibility,
    pub(super) range: TextRange,
}

#[derive(Clone)]
pub(super) struct PropertySignature {
    pub(super) name: SmolStr,
    pub(super) prop_type: TypeId,
    pub(super) has_get: bool,
    pub(super) has_set: bool,
    pub(super) visibility: Visibility,
    pub(super) range: TextRange,
}

pub(super) struct InterfaceMembers {
    pub(super) methods: FxHashMap<SmolStr, MethodSignature>,
    pub(super) properties: FxHashMap<SmolStr, PropertySignature>,
}

pub(super) struct InterfaceCheckContext<'a> {
    pub(super) owner_name: &'a str,
    pub(super) interface_name: &'a str,
    pub(super) interface_range: TextRange,
    pub(super) allow_missing: bool,
}
