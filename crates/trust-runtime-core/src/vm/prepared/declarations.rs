//! The storage declarations and the initializers of a prepared program, and what a scan reads of
//! them (specification 34 MEM-14, the program image in place).
//!
//! A scan reads of a declaration its type, its owning POU, its name, its slot, its default constant,
//! its owner, its role and its flags, when the general path finds a member, writes an own scalar or
//! checks a write through a reference, and when a call gives a frame variable its default. A call
//! reads of an initializer what decides whether it writes a frame variable's default directly or
//! whether a static is already initialized. So a program whose image a chip maps keeps its records
//! where they lie in the image, the chip's flash, and only those fields on the heap, one compact
//! record for each declaration and each initializer; only building an array, a structure, a text or
//! an instance reads the whole records. A record that preparation changed, a timer's duration member
//! that takes the native duration slot, is kept as preparation holds it. A program prepared from
//! bytes, which it does not keep, holds its records decoded, each as its compact record and its
//! other fields, in the bytes the record takes. Either way a scan reaches a compact record by
//! reference.

use alloc::vec::Vec;

use super::records::{Records, Split};
use super::PreparedModule;
use crate::bytecode::{
    initializer_entry, section_payload, storage_declaration, InitializationOnce,
    InitializationPhase, InitializationStage, InitializationTarget, InitializationTrigger,
    InitializerBodyKind, InitializerEntry, SectionId, StorageDeclaration, StorageOwner,
    StorageRole,
};
use crate::error::RuntimeError;

/// What a scan reads of a storage declaration (module documentation), with the names and meanings
/// that [`StorageDeclaration`] gives them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ScanDeclaration {
    /// The declared type, absent for a program root or a compiler scratch bank.
    pub(crate) type_id: Option<u32>,
    /// The declaring POU, absent only for ordinary resource globals.
    pub(crate) owner_pou_id: Option<u32>,
    /// The declaration's name in the string table.
    pub(crate) name_idx: u32,
    /// The logical slot within the owner.
    pub(crate) slot: u32,
    /// The declaration constant, [`NONE`] when there is none.
    default_const: u32,
    /// The storage ownership domain.
    pub(crate) owner: StorageOwner,
    /// The declaration's behaviour.
    pub(crate) role: StorageRole,
    /// The direction and qualifier bits.
    flags: u8,
}

crate::memory::figure::holds_no_heap!(ScanDeclaration);

/// An absent index in a compact record. Validation admits only indexes inside their tables, which
/// hold fewer records than this.
const NONE: u32 = u32::MAX;

/// The fields of a storage declaration that no scan reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct DeclarationRest {
    retain: u8,
    ref_idx: Option<u32>,
    construction_nodes: u32,
    related_declaration_idx: Option<u32>,
    source_name_idx: Option<u32>,
}

crate::memory::figure::holds_no_heap!(DeclarationRest);

impl Split for StorageDeclaration {
    type Scan = ScanDeclaration;
    type Rest = DeclarationRest;

    fn split(self) -> (ScanDeclaration, DeclarationRest) {
        let scan = Self::scan_part(&self);
        // Every field is named, so that a field added to the record is split too.
        let StorageDeclaration {
            owner: _,
            role: _,
            retain,
            flags: _,
            owner_pou_id: _,
            name_idx: _,
            type_id: _,
            slot: _,
            ref_idx,
            default_const_idx: _,
            construction_nodes,
            related_declaration_idx,
            source_name_idx,
        } = self;
        let rest = DeclarationRest {
            retain,
            ref_idx,
            construction_nodes,
            related_declaration_idx,
            source_name_idx,
        };
        (scan, rest)
    }

    fn scan_part(declaration: &Self) -> ScanDeclaration {
        ScanDeclaration {
            type_id: declaration.type_id,
            owner_pou_id: declaration.owner_pou_id,
            name_idx: declaration.name_idx,
            slot: declaration.slot,
            default_const: declaration.default_const_idx.unwrap_or(NONE),
            owner: declaration.owner,
            role: declaration.role,
            flags: declaration.flags,
        }
    }

    fn join(scan: &ScanDeclaration, rest: &DeclarationRest) -> Self {
        StorageDeclaration {
            owner: scan.owner,
            role: scan.role,
            retain: rest.retain,
            flags: scan.flags,
            owner_pou_id: scan.owner_pou_id,
            name_idx: scan.name_idx,
            type_id: scan.type_id,
            slot: scan.slot,
            ref_idx: rest.ref_idx,
            default_const_idx: scan.default_const_idx(),
            construction_nodes: rest.construction_nodes,
            related_declaration_idx: rest.related_declaration_idx,
            source_name_idx: rest.source_name_idx,
        }
    }
}

impl ScanDeclaration {
    /// The declaration constant, as [`StorageDeclaration::default_const_idx`] holds it.
    pub(crate) fn default_const_idx(&self) -> Option<u32> {
        (self.default_const != NONE).then_some(self.default_const)
    }

    /// Whether the declaration owns a physical slot rather than an external alias, as
    /// [`StorageDeclaration::owns_storage`] says.
    pub(crate) fn owns_storage(&self) -> bool {
        self.role != StorageRole::External
    }

    /// Whether writes after initialization are forbidden, as [`StorageDeclaration::is_constant`]
    /// says.
    pub(crate) fn is_constant(&self) -> bool {
        self.flags & 1 != 0
    }
}

/// What a call reads of an initializer when it builds a frame (module documentation), with the
/// names and meanings that [`InitializerEntry`] gives them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ScanInitializer {
    /// The declaration the initializer writes, [`NONE`] when it writes none.
    declaration: u32,
    /// The typed temporary base reference of the result.
    pub(crate) result_ref_idx: u32,
    /// The stage of the action.
    pub(crate) stage: InitializationStage,
    /// The owner of the static initialization state.
    pub(crate) once: InitializationOnce,
    /// Whether the initializer is an action or a recipe.
    pub(crate) body_kind: InitializerBodyKind,
    /// [`HAS_BODY`] and [`HAS_CONTEXT_OR_RECIPE`].
    bits: u8,
}

crate::memory::figure::holds_no_heap!(ScanInitializer);

/// The initializer has code to run.
const HAS_BODY: u8 = 1;
/// The initializer has a recipe context, a recipe type or a recipe member.
const HAS_CONTEXT_OR_RECIPE: u8 = 2;

/// The fields of an initializer that no call reads when it builds a frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct InitializerRest {
    owner_pou_id: Option<u32>,
    code_offset: u32,
    code_length: u32,
    visible_local_count: u32,
    visible_static_count: u32,
    phase: InitializationPhase,
    trigger: InitializationTrigger,
    target_idx: Option<u32>,
    partial_kind: u8,
    target_kind: InitializationTarget,
    target_reserved: [u8; 2],
    partial_index: u32,
    context_initializer_idx: Option<u32>,
    recipe_type_id: Option<u32>,
    recipe_member_idx: Option<u32>,
    recipe_reserved: [u8; 3],
}

crate::memory::figure::holds_no_heap!(InitializerRest);

impl Split for InitializerEntry {
    type Scan = ScanInitializer;
    type Rest = InitializerRest;

    fn split(self) -> (ScanInitializer, InitializerRest) {
        let scan = Self::scan_part(&self);
        // Every field is named, so that a field added to the record is split too.
        let InitializerEntry {
            declaration_idx: _,
            owner_pou_id,
            result_ref_idx: _,
            code_offset,
            code_length,
            visible_local_count,
            visible_static_count,
            phase,
            once: _,
            stage: _,
            trigger,
            target_idx,
            partial_kind,
            target_kind,
            target_reserved,
            partial_index,
            context_initializer_idx,
            recipe_type_id,
            recipe_member_idx,
            body_kind: _,
            recipe_reserved,
        } = self;
        let rest = InitializerRest {
            owner_pou_id,
            code_offset,
            code_length,
            visible_local_count,
            visible_static_count,
            phase,
            trigger,
            target_idx,
            partial_kind,
            target_kind,
            target_reserved,
            partial_index,
            context_initializer_idx,
            recipe_type_id,
            recipe_member_idx,
            recipe_reserved,
        };
        (scan, rest)
    }

    fn scan_part(initializer: &Self) -> ScanInitializer {
        let mut bits = 0;
        if initializer.code_length != 0 {
            bits |= HAS_BODY;
        }
        if initializer.context_initializer_idx.is_some()
            || initializer.recipe_type_id.is_some()
            || initializer.recipe_member_idx.is_some()
        {
            bits |= HAS_CONTEXT_OR_RECIPE;
        }
        ScanInitializer {
            declaration: initializer.declaration_idx.unwrap_or(NONE),
            result_ref_idx: initializer.result_ref_idx,
            stage: initializer.stage,
            once: initializer.once,
            body_kind: initializer.body_kind,
            bits,
        }
    }

    fn join(scan: &ScanInitializer, rest: &InitializerRest) -> Self {
        InitializerEntry {
            declaration_idx: scan.declaration_idx(),
            owner_pou_id: rest.owner_pou_id,
            result_ref_idx: scan.result_ref_idx,
            code_offset: rest.code_offset,
            code_length: rest.code_length,
            visible_local_count: rest.visible_local_count,
            visible_static_count: rest.visible_static_count,
            phase: rest.phase,
            once: scan.once,
            stage: scan.stage,
            trigger: rest.trigger,
            target_idx: rest.target_idx,
            partial_kind: rest.partial_kind,
            target_kind: rest.target_kind,
            target_reserved: rest.target_reserved,
            partial_index: rest.partial_index,
            context_initializer_idx: rest.context_initializer_idx,
            recipe_type_id: rest.recipe_type_id,
            recipe_member_idx: rest.recipe_member_idx,
            body_kind: scan.body_kind,
            recipe_reserved: rest.recipe_reserved,
        }
    }
}

impl ScanInitializer {
    /// The declaration the initializer writes, as [`InitializerEntry::declaration_idx`] holds it.
    pub(crate) fn declaration_idx(&self) -> Option<u32> {
        (self.declaration != NONE).then_some(self.declaration)
    }

    /// Whether the initializer has code to run, a nonzero [`InitializerEntry::code_length`].
    pub(crate) fn has_body(&self) -> bool {
        self.bits & HAS_BODY != 0
    }

    /// Whether the initializer has a recipe context, a recipe type or a recipe member.
    pub(crate) fn has_context_or_recipe(&self) -> bool {
        self.bits & HAS_CONTEXT_OR_RECIPE != 0
    }
}

/// The bytes of a STORAGE_LAYOUT record and of an INITIALIZER_INDEX record (specification 12
/// §11.5).
const DECLARATION_BYTES: usize = 40;
const INITIALIZER_BYTES: usize = 60;

/// The table of storage declarations and the table of initializers of a prepared program.
pub(super) type DeclarationTable = Records<StorageDeclaration>;
pub(super) type InitializerTable = Records<InitializerEntry>;

/// The tables of `declarations` and `initializers`, as preparation holds them: read where their
/// records lie in `image`, a program's mapped image, with what a scan reads of each record kept on
/// the heap, or decoded when the prepared program keeps no image.
///
/// # Errors
///
/// When the image does not hold the records, or the heap has no room for the tables.
pub(super) fn tables(
    image: Option<&[u8]>,
    declarations: Vec<StorageDeclaration>,
    initializers: Vec<InitializerEntry>,
) -> Result<(DeclarationTable, InitializerTable), RuntimeError> {
    let Some(image) = image else {
        return Ok((
            Records::decoded(declarations),
            Records::decoded(initializers),
        ));
    };
    // Each section's payload starts with the number of its records (specification 12 §11.5).
    let first = |id: SectionId| {
        section_payload(image, id)
            .map(|payload| payload.start + 4)
            .ok_or_else(|| {
                super::super::module::invalid_bytecode(smol_str::SmolStr::new_static(
                    "a section outside the image",
                ))
            })
    };
    let storage = Records::in_image::<DECLARATION_BYTES>(
        image,
        first(SectionId::StorageLayout)?,
        &declarations,
        storage_declaration,
    )?;
    let initializers = Records::in_image::<INITIALIZER_BYTES>(
        image,
        first(SectionId::Initializers)?,
        &initializers,
        initializer_entry,
    )?;
    Ok((storage, initializers))
}

impl PreparedModule {
    /// Admitted storage declaration `index`, for read-only tooling: decoded, or read where it lies
    /// in the program's mapped image (specification 34 MEM-14).
    #[must_use]
    pub fn storage_declaration(&self, index: usize) -> Option<StorageDeclaration> {
        self.storage
            .get::<DECLARATION_BYTES>(self.image, index, storage_declaration)
    }

    /// Storage declaration `index` of a validated program, which has it.
    ///
    /// # Panics
    ///
    /// When the program has no declaration `index`, as indexing its table would.
    pub(crate) fn storage_declaration_at(&self, index: usize) -> StorageDeclaration {
        self.storage_declaration(index)
            .expect("a declaration index that validation admitted")
    }

    /// The number of storage declarations.
    pub(crate) fn declaration_count(&self) -> usize {
        self.storage.len()
    }

    /// What a scan reads of storage declaration `index`.
    #[inline]
    pub(crate) fn scan_declaration(&self, index: usize) -> Option<&ScanDeclaration> {
        self.storage.scan(index)
    }

    /// What a scan reads of storage declaration `index` of a validated program, which has it.
    ///
    /// # Panics
    ///
    /// When the program has no declaration `index`, as indexing its table would.
    pub(crate) fn scan_declaration_at(&self, index: usize) -> &ScanDeclaration {
        self.scan_declaration(index)
            .expect("a declaration index that validation admitted")
    }

    /// What a call reads of initializer `index`.
    #[inline]
    pub(crate) fn scan_initializer(&self, index: usize) -> Option<&ScanInitializer> {
        self.initializers.scan(index)
    }

    /// Initializer `index`: decoded, or read where it lies in the program's mapped image.
    pub(crate) fn initializer(&self, index: usize) -> Option<InitializerEntry> {
        self.initializers
            .get::<INITIALIZER_BYTES>(self.image, index, initializer_entry)
    }

    /// Initializer `index` of a validated program, which has it.
    ///
    /// # Panics
    ///
    /// When the program has no initializer `index`, as indexing its table would.
    pub(crate) fn initializer_at(&self, index: usize) -> InitializerEntry {
        self.initializer(index)
            .expect("an initializer index that validation admitted")
    }

    /// The number of initializers.
    pub(crate) fn initializer_count(&self) -> usize {
        self.initializers.len()
    }

    /// Every initializer, in order.
    pub(crate) fn initializer_entries(&self) -> impl Iterator<Item = InitializerEntry> + '_ {
        (0..self.initializer_count()).filter_map(|index| self.initializer(index))
    }

    /// The storage layout as preparation held it, for a test that rebuilds an index from it.
    #[cfg(test)]
    pub(crate) fn storage_layout(&self) -> crate::bytecode::StorageLayout {
        crate::bytecode::StorageLayout {
            entries: (0..self.declaration_count())
                .filter_map(|index| self.storage_declaration(index))
                .collect(),
        }
    }

    /// The initializer index as preparation held it, for a test that rebuilds an index from it.
    #[cfg(test)]
    pub(crate) fn initializer_index(&self) -> crate::bytecode::InitializerIndex {
        crate::bytecode::InitializerIndex {
            entries: self.initializer_entries().collect(),
        }
    }

    /// Replace every storage declaration with `declarations`, as a test that forges a prepared
    /// program does.
    #[cfg(test)]
    pub(crate) fn set_declarations(&mut self, declarations: Vec<StorageDeclaration>) {
        self.storage.replace(declarations);
    }

    /// Replace storage declaration `index`, as a test that forges a prepared program does.
    #[cfg(test)]
    pub(crate) fn set_declaration(&mut self, index: usize, declaration: StorageDeclaration) {
        self.storage.set(index, declaration);
    }

    /// Replace initializer `index`, as a test that forges a prepared program does.
    #[cfg(test)]
    pub(crate) fn set_initializer(&mut self, index: usize, initializer: InitializerEntry) {
        self.initializers.set(index, initializer);
    }
}

#[cfg(test)]
mod tests;
