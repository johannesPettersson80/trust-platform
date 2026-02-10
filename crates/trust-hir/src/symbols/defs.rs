//! Symbol table and symbol definitions.
//!
//! This module provides the symbol table that tracks all declarations
//! in a Structured Text program.

use rustc_hash::FxHashMap;
use smol_str::SmolStr;
use text_size::TextRange;

use crate::db::FileId;
use crate::types::TypeId;

/// A unique identifier for a symbol.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SymbolId(pub u32);

impl SymbolId {
    /// The invalid/unknown symbol ID.
    pub const UNKNOWN: Self = Self(u32::MAX);
}

/// Origin information for an imported symbol.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SymbolOrigin {
    /// The file containing the original symbol.
    pub file_id: FileId,
    /// The symbol ID within the original file.
    pub symbol_id: SymbolId,
}

/// A USING directive attached to a scope.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UsingDirective {
    /// Fully qualified namespace path.
    pub path: Vec<SmolStr>,
    /// Source range of the directive.
    pub range: TextRange,
}

/// Resolution result for USING directives within a scope.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UsingResolution {
    /// No match.
    None,
    /// Exactly one matching symbol.
    Single(SymbolId),
    /// Multiple matching symbols (ambiguous).
    Ambiguous,
}

/// A unique identifier for a scope.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct ScopeId(pub u32);

impl ScopeId {
    /// The global scope ID.
    pub const GLOBAL: Self = Self(0);
}

/// The kind of a symbol.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SymbolKind {
    /// A program.
    Program,
    /// A configuration declaration.
    Configuration,
    /// A resource declaration.
    Resource,
    /// A task declaration.
    Task,
    /// A program instance declared in a CONFIGURATION/RESOURCE.
    ProgramInstance,
    /// A namespace.
    Namespace,
    /// A function.
    Function {
        /// Return type.
        return_type: TypeId,
        /// Parameter symbol IDs.
        parameters: Vec<SymbolId>,
    },
    /// A function block.
    FunctionBlock,
    /// A class.
    Class,
    /// A method.
    Method {
        /// Return type (None for void).
        return_type: Option<TypeId>,
        /// Parameter symbol IDs.
        parameters: Vec<SymbolId>,
    },
    /// A property.
    Property {
        /// Property type.
        prop_type: TypeId,
        /// Has getter.
        has_get: bool,
        /// Has setter.
        has_set: bool,
    },
    /// An interface.
    Interface,
    /// A variable.
    Variable {
        /// Variable qualifier.
        qualifier: VarQualifier,
    },
    /// A constant.
    Constant,
    /// A type definition.
    Type,
    /// An enum value.
    EnumValue {
        /// The numeric value.
        value: i64,
    },
    /// A parameter.
    Parameter {
        /// Parameter direction.
        direction: ParamDirection,
    },
}

/// Variable qualifier (where it was declared).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VarQualifier {
    /// Local variable (VAR).
    Local,
    /// Input variable (VAR_INPUT).
    Input,
    /// Output variable (VAR_OUTPUT).
    Output,
    /// In-out variable (VAR_IN_OUT).
    InOut,
    /// Temporary variable (VAR_TEMP).
    Temp,
    /// Global variable (VAR_GLOBAL).
    Global,
    /// External variable (VAR_EXTERNAL).
    External,
    /// Access variable (VAR_ACCESS).
    Access,
    /// Static variable (VAR_STAT).
    Static,
}

/// Parameter direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParamDirection {
    /// Input parameter.
    In,
    /// Output parameter.
    Out,
    /// In-out parameter.
    InOut,
}

/// Visibility of a symbol.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Visibility {
    /// Public (accessible from anywhere).
    #[default]
    Public,
    /// Private (accessible only within the POU).
    Private,
    /// Protected (accessible within the POU and derived POUs).
    Protected,
    /// Internal (accessible within the namespace).
    Internal,
}

/// Modifiers applied to symbols (classes, methods, function blocks).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SymbolModifiers {
    /// FINAL modifier (class/function block/method).
    pub is_final: bool,
    /// ABSTRACT modifier (class/function block/method).
    pub is_abstract: bool,
    /// OVERRIDE modifier (method).
    pub is_override: bool,
}

/// A symbol in the symbol table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Symbol {
    /// The symbol's unique ID.
    pub id: SymbolId,
    /// The symbol's name.
    pub name: SmolStr,
    /// The kind of symbol.
    pub kind: SymbolKind,
    /// The symbol's type.
    pub type_id: TypeId,
    /// Direct address binding (`AT %...`) if present.
    pub direct_address: Option<SmolStr>,
    /// The symbol's visibility.
    pub visibility: Visibility,
    /// Modifiers (FINAL/ABSTRACT/OVERRIDE) associated with the symbol.
    pub modifiers: SymbolModifiers,
    /// The source location of the declaration.
    pub range: TextRange,
    /// Origin of the symbol if imported from another file.
    pub origin: Option<SymbolOrigin>,
    /// The parent symbol (for nested declarations).
    pub parent: Option<SymbolId>,
    /// Documentation comment, if any.
    pub doc: Option<SmolStr>,
}

impl Symbol {
    /// Creates a new symbol.
    pub fn new(
        id: SymbolId,
        name: impl Into<SmolStr>,
        kind: SymbolKind,
        type_id: TypeId,
        range: TextRange,
    ) -> Self {
        Self {
            id,
            name: name.into(),
            kind,
            type_id,
            direct_address: None,
            visibility: Visibility::default(),
            modifiers: SymbolModifiers::default(),
            range,
            origin: None,
            parent: None,
            doc: None,
        }
    }

    /// Returns true if this is a callable symbol (function, method, function block).
    #[must_use]
    pub fn is_callable(&self) -> bool {
        matches!(
            self.kind,
            SymbolKind::Function { .. } | SymbolKind::Method { .. } | SymbolKind::FunctionBlock
        )
    }

    /// Returns true if this is a type symbol.
    #[must_use]
    pub fn is_type(&self) -> bool {
        matches!(
            self.kind,
            SymbolKind::Type
                | SymbolKind::FunctionBlock
                | SymbolKind::Class
                | SymbolKind::Interface
        )
    }
}

/// A scope in the symbol table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Scope {
    /// The scope's unique ID.
    pub id: ScopeId,
    /// Parent scope, if any.
    pub parent: Option<ScopeId>,
    /// The symbol that owns this scope (e.g., the function or program).
    pub owner: Option<SymbolId>,
    /// Symbols defined in this scope (normalized name -> SymbolId).
    pub symbols: FxHashMap<SmolStr, SymbolId>,
    /// The kind of scope.
    pub kind: ScopeKind,
    /// USING directives applicable in this scope.
    pub using_directives: Vec<UsingDirective>,
}

impl Scope {
    /// Creates a new scope.
    pub fn new(
        id: ScopeId,
        kind: ScopeKind,
        parent: Option<ScopeId>,
        owner: Option<SymbolId>,
    ) -> Self {
        Self {
            id,
            parent,
            owner,
            symbols: FxHashMap::default(),
            kind,
            using_directives: Vec::new(),
        }
    }

    /// Defines a symbol in this scope. Returns the previous symbol ID if there was a duplicate.
    pub fn define(&mut self, name: SmolStr, id: SymbolId) -> Option<SymbolId> {
        self.symbols.insert(normalize_name(&name), id)
    }

    /// Looks up a symbol by name in this scope only.
    #[must_use]
    pub fn lookup_local(&self, name: &str) -> Option<SymbolId> {
        self.symbols.get(&normalize_name(name)).copied()
    }

    /// Returns an iterator over the symbol IDs in this scope.
    pub fn symbol_ids(&self) -> impl Iterator<Item = &SymbolId> {
        self.symbols.values()
    }
}

/// The kind of scope.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScopeKind {
    /// Global scope.
    Global,
    /// Configuration scope.
    Configuration,
    /// Resource scope.
    Resource,
    /// Namespace scope.
    Namespace,
    /// Program scope.
    Program,
    /// Function scope.
    Function,
    /// Function block scope.
    FunctionBlock,
    /// Class scope.
    Class,
    /// Method scope.
    Method,
    /// Property scope.
    Property,
    /// Block scope (IF, FOR, etc.).
    Block,
}

pub(super) fn normalize_name(name: &str) -> SmolStr {
    SmolStr::new(name.to_ascii_uppercase())
}
