//! Declaration and initialization records for STBC 2.0.

use super::*;

/// Owner of a declared storage slot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum StorageOwner {
    /// Resource-global storage, including function statics.
    Global = 0,
    /// A slot in an instance template, resolved against the current instance.
    Instance = 1,
    /// A slot in a POU invocation frame.
    Frame = 2,
}

impl StorageOwner {
    /// Decode the closed wire discriminant.
    pub fn from_raw(value: u8) -> Option<Self> {
        match value {
            0 => Some(Self::Global),
            1 => Some(Self::Instance),
            2 => Some(Self::Frame),
            _ => None,
        }
    }
}

/// Declaration role; initialization and ownership are not inferred from a name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum StorageRole {
    /// An ordinary declared value.
    Variable = 0,
    /// A construction-only program instance bound in global storage.
    ProgramRoot = 1,
    /// A function/method result slot, defaulted only when NULL.
    Return = 2,
    /// A parameter slot whose supplied value is preserved.
    Parameter = 3,
    /// Once-initialized persistent local storage.
    Static = 4,
    /// An alias to existing storage; no local slot is allocated.
    External = 5,
    /// Untyped compiler-generated tail slots, initialized to NULL.
    Scratch = 6,
    /// Per-instance previous raw value for an edge-qualified BOOL input.
    EdgePhase = 7,
    /// Reserved internal FB slot, initialized by its native import on first use.
    NativeState = 8,
}

impl StorageRole {
    /// Decode the closed wire discriminant.
    pub fn from_raw(value: u8) -> Option<Self> {
        match value {
            0 => Some(Self::Variable),
            1 => Some(Self::ProgramRoot),
            2 => Some(Self::Return),
            3 => Some(Self::Parameter),
            4 => Some(Self::Static),
            5 => Some(Self::External),
            6 => Some(Self::Scratch),
            7 => Some(Self::EdgePhase),
            8 => Some(Self::NativeState),
            _ => None,
        }
    }
}

/// Complete declaration templates; declaration identity is the vector index.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StorageLayout {
    /// Declarations in construction/declaration order.
    pub entries: Vec<StorageDeclaration>,
}

/// One fixed-width 40-byte storage declaration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StorageDeclaration {
    /// Storage ownership domain.
    pub owner: StorageOwner,
    /// Declaration behavior.
    pub role: StorageRole,
    /// Existing VAR_META retention policy (0 through 3).
    pub retain: u8,
    /// Constant/input/output/in-out or edge-phase direction bits; other bits reserved.
    pub flags: u8,
    /// Declaring POU, absent only for ordinary resource globals.
    pub owner_pou_id: Option<u32>,
    /// Storage name in STRING_TABLE.
    pub name_idx: u32,
    /// Declared type; absent only for a program root or compiler scratch bank.
    pub type_id: Option<u32>,
    /// Logical slot within the owner, not a byte offset.
    pub slot: u32,
    /// Optional base REF_TABLE binding; template members are relative.
    pub ref_idx: Option<u32>,
    /// Declaration constant, distinct from current mutable storage contents.
    pub default_const_idx: Option<u32>,
    /// Logical construction demand, independently checked against the type/template.
    pub construction_nodes: u32,
    /// Associated input declaration for edge phase; absent for other roles.
    pub related_declaration_idx: Option<u32>,
    /// Original lexical variable name for static storage; absent for other roles.
    pub source_name_idx: Option<u32>,
}

/// Persistent construction roots; future call instances use declaration templates.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ConstructionRoots {
    /// Parent-before-child root records.
    pub entries: Vec<ConstructionRoot>,
}

/// One fixed-width 24-byte root binding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConstructionRoot {
    /// STORAGE_LAYOUT declaration index.
    pub declaration_idx: u32,
    /// Existing reference binding, absent only for an inheritance parent.
    pub binding_ref_idx: Option<u32>,
    /// Logical artifact instance identity, never a required live host instance.
    pub instance_owner_id: Option<u32>,
    /// Earlier construction root owning this nested/inherited instance.
    pub parent_root_idx: Option<u32>,
    /// POU whose instance template is constructed, if this is an instance.
    pub template_pou_id: Option<u32>,
    /// Bit zero denotes an inheritance parent; other bits are reserved.
    pub flags: u32,
}

/// Execution boundary at which an initializer is evaluated.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum InitializationPhase {
    /// Resource startup before any task is enabled.
    Resource = 0,
    /// Construction of each owning instance.
    Instance = 1,
    /// Each POU invocation, after parameters have been bound.
    Frame = 2,
    /// Static initialization, selected by its ordinary or after-restart trigger.
    Static = 3,
    /// Return-slot default when no caller-provided value is present.
    Return = 4,
    /// Ordered per-target VAR_CONFIG startup action.
    Configuration = 5,
    /// Default for an omitted parameter, before local initialization.
    Parameter = 6,
    /// A typed value default evaluated by DEFAULT_VALUE, without callee invocation.
    ValueDefault = 7,
}

impl InitializationPhase {
    /// Decode the closed wire discriminant.
    pub fn from_raw(value: u8) -> Option<Self> {
        match value {
            0 => Some(Self::Resource),
            1 => Some(Self::Instance),
            2 => Some(Self::Frame),
            3 => Some(Self::Static),
            4 => Some(Self::Return),
            5 => Some(Self::Configuration),
            6 => Some(Self::Parameter),
            7 => Some(Self::ValueDefault),
            _ => None,
        }
    }
}

/// Lifetime of an initializer's once-state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum InitializationOnce {
    /// No separate static once-state.
    None = 0,
    /// A function static belongs to the module generation.
    Module = 1,
    /// A method static belongs to the current instance and declaring method.
    Instance = 2,
}

impl InitializationOnce {
    /// Decode the closed wire discriminant.
    pub fn from_raw(value: u8) -> Option<Self> {
        match value {
            0 => Some(Self::None),
            1 => Some(Self::Module),
            2 => Some(Self::Instance),
            _ => None,
        }
    }
}

/// Ordered operation within a declaration's initialization sequence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum InitializationStage {
    /// Typed defaults and nested construction.
    Default = 0,
    /// Explicit initializer or configuration override.
    Explicit = 1,
}

impl InitializationStage {
    /// Decode the closed wire discriminant.
    pub fn from_raw(value: u8) -> Option<Self> {
        match value {
            0 => Some(Self::Default),
            1 => Some(Self::Explicit),
            _ => None,
        }
    }
}

/// Lifecycle context for static initialization.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum InitializationTrigger {
    /// Ordinary owner construction or invocation lifecycle.
    Ordinary = 0,
    /// First function invocation after restart removed its module-static slots.
    AfterRestart = 1,
}

impl InitializationTrigger {
    /// Decode the closed wire discriminant.
    pub fn from_raw(value: u8) -> Option<Self> {
        match value {
            0 => Some(Self::Ordinary),
            1 => Some(Self::AfterRestart),
            _ => None,
        }
    }
}

/// Meaning of a configuration action's target index.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum InitializationTarget {
    /// Declaration action; no separate target index.
    Declaration = 0,
    /// Index in REF_TABLE, optionally followed by partial access.
    Reference = 1,
    /// Index in STRING_TABLE containing a fully specified direct I/O address.
    DirectIo = 2,
}

impl InitializationTarget {
    /// Decode the closed wire discriminant.
    pub fn from_raw(value: u8) -> Option<Self> {
        match value {
            0 => Some(Self::Declaration),
            1 => Some(Self::Reference),
            2 => Some(Self::DirectIo),
            _ => None,
        }
    }
}

/// Whether a body participates in lifecycle scheduling or supplies a callable default.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum InitializerBodyKind {
    /// Scheduled declaration action or explicit DEFAULT_VALUE entry point.
    Action = 0,
    /// Default expression for an exact type, including aliases.
    TypeDefault = 1,
    /// Default expression for one declared struct member or union variant.
    MemberDefault = 2,
}

impl InitializerBodyKind {
    /// Decode the closed wire discriminant.
    pub fn from_raw(value: u8) -> Option<Self> {
        match value {
            0 => Some(Self::Action),
            1 => Some(Self::TypeDefault),
            2 => Some(Self::MemberDefault),
            _ => None,
        }
    }
}

/// Executable initializer index; code remains ordinary STBC in POU_BODIES.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct InitializerIndex {
    /// Initializer identity is the vector index.
    pub entries: Vec<InitializerEntry>,
}

/// One fixed-width 60-byte initializer descriptor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InitializerEntry {
    /// STORAGE_LAYOUT declaration; absent for direct-I/O actions and value-default bodies.
    pub declaration_idx: Option<u32>,
    /// Original invocation/instance POU, absent for resource globals.
    pub owner_pou_id: Option<u32>,
    /// Typed temporary base reference with location InitializerResult.
    pub result_ref_idx: u32,
    /// Absolute byte offset in POU_BODIES.
    pub code_offset: u32,
    /// Length of the independently bounded code range.
    pub code_length: u32,
    /// Earlier frame slots visible while evaluating this declaration.
    pub visible_local_count: u32,
    /// Earlier static declarations visible while evaluating this declaration.
    pub visible_static_count: u32,
    /// When this body is evaluated.
    pub phase: InitializationPhase,
    /// Owner of static initialization state.
    pub once: InitializationOnce,
    /// Default and explicit actions retain producer-specified execution order.
    pub stage: InitializationStage,
    /// Selects the initialization context; recipes inherit their root trigger.
    pub trigger: InitializationTrigger,
    /// REF_TABLE or direct-address STRING_TABLE index, selected by target_kind.
    pub target_idx: Option<u32>,
    /// Partial access: none, bit, byte, word or double-word (0 through 4).
    pub partial_kind: u8,
    /// Target table selected by target_idx.
    pub target_kind: InitializationTarget,
    /// Reserved and required to be zero.
    pub target_reserved: [u8; 2],
    /// Selected partial unit; zero when no partial access is requested.
    pub partial_index: u32,
    /// Canonical action defining recipe context; absent on canonical actions.
    pub context_initializer_idx: Option<u32>,
    /// Exact type whose default or member default this recipe supplies.
    pub recipe_type_id: Option<u32>,
    /// Zero-based STRUCT member or UNION variant, present only for member-default recipes.
    pub recipe_member_idx: Option<u32>,
    /// Separates callable defaults from lifecycle scheduling.
    pub body_kind: InitializerBodyKind,
    /// Reserved and required to be zero.
    pub recipe_reserved: [u8; 3],
}

/// Named access aliases retained for source-free host access/debug surfaces.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AccessBindings {
    /// Aliases in source declaration order.
    pub entries: Vec<AccessBindingEntry>,
}

/// One fixed-width 20-byte alias descriptor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccessBindingEntry {
    /// Public alias name in STRING_TABLE.
    pub name_idx: u32,
    /// Exposed value type, including any partial selection.
    pub type_id: u32,
    /// Underlying REF_TABLE binding.
    pub ref_idx: u32,
    /// None, bit, byte, word or double-word (0 through 4).
    pub partial_kind: u8,
    /// Bit zero permits writes through the alias; other bits are reserved.
    pub flags: u8,
    /// Reserved and required to be zero.
    pub reserved: u16,
    /// Selected partial unit; zero for whole-value aliases.
    pub partial_index: u32,
}

impl InitializerEntry {
    /// Immutable recipe visibility identity. Instance defaults see a slot prefix;
    /// explicit passes see their declaration-role group. Runtime values and once
    /// state remain owned by the invoking action, never by this shared context.
    pub fn recipe_context_key(&self, layout: &StorageLayout) -> Option<[u32; 11]> {
        let declaration = match self.declaration_idx {
            Some(id) => Some(layout.entries.get(id as usize)?),
            None => None,
        };
        let (role, frontier) = match declaration {
            Some(declaration) if declaration.owner == StorageOwner::Instance => (
                declaration.role as u32 + 1,
                if self.stage == InitializationStage::Default {
                    declaration.slot
                } else {
                    u32::MAX
                },
            ),
            _ => (0, 0),
        };
        Some([
            u32::from(self.owner_pou_id.is_some()),
            self.owner_pou_id.unwrap_or(0),
            self.phase as u32,
            self.stage as u32,
            self.trigger as u32,
            self.once as u32,
            self.visible_local_count,
            self.visible_static_count,
            role,
            frontier,
            self.target_kind as u32,
        ])
    }
}

/// Partial-access wire metadata shared by authoring and portable validation.
#[derive(Clone, Copy)]
pub struct PartialAccessSpec {
    /// INITIALIZERS/ACCESS_BINDINGS discriminant.
    pub kind: u8,
    /// Selected bit width.
    pub width: u32,
    /// TYPE_TABLE primitive identifier of the result.
    pub result_primitive: u16,
}

impl PartialAccessSpec {
    const SPECS: [Self; 4] = [
        Self {
            kind: 1,
            width: 1,
            result_primitive: 1,
        },
        Self {
            kind: 2,
            width: 8,
            result_primitive: 2,
        },
        Self {
            kind: 3,
            width: 16,
            result_primitive: 3,
        },
        Self {
            kind: 4,
            width: 32,
            result_primitive: 4,
        },
    ];

    /// Decode a nonempty partial access; zero represents no selection.
    pub const fn from_raw(kind: u8) -> Option<Self> {
        if kind >= 1 && kind <= 4 {
            Some(Self::SPECS[(kind - 1) as usize])
        } else {
            None
        }
    }

    /// Map the runtime selection to the wire representation and element index.
    pub const fn from_access(access: crate::value::PartialAccess) -> (Self, u32) {
        use crate::value::PartialAccess;
        let (kind, index) = match access {
            PartialAccess::Bit(index) => (0, index),
            PartialAccess::Byte(index) => (1, index),
            PartialAccess::Word(index) => (2, index),
            PartialAccess::DWord(index) => (3, index),
        };
        (Self::SPECS[kind], index as u32)
    }
}

impl StorageDeclaration {
    /// Whether a warm restart preserves this declaration (RETAIN or PERSISTENT).
    pub fn is_retained(&self) -> bool {
        matches!(self.retain, 1 | 3)
    }
    /// Whether this record owns a physical slot rather than an external alias.
    pub fn owns_storage(&self) -> bool {
        self.role != StorageRole::External
    }
    /// Whether writes after initialization are forbidden.
    pub fn is_constant(&self) -> bool {
        self.flags & 1 != 0
    }
    /// Whether an edge-phase record represents a rising edge.
    pub fn is_rising_edge(&self) -> bool {
        self.flags & 16 != 0
    }
}
impl InitializerEntry {
    /// Whether this record commits a declaration/configuration action.
    pub fn is_action(&self) -> bool {
        self.body_kind == InitializerBodyKind::Action
    }
}
impl ConstructionRoot {
    /// Whether this root is the inherited parent of another instance.
    pub fn is_inheritance_parent(&self) -> bool {
        self.flags & 1 != 0
    }
}
impl AccessBindingEntry {
    /// Whether engineering writes are permitted through this alias.
    pub fn is_writable(&self) -> bool {
        self.flags & 1 != 0
    }
}
