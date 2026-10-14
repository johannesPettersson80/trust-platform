//! Read-only installed metadata for platform and development adapters.

use super::PreparedModule;
use crate::bytecode::{ResourceEntry, StorageDeclaration, StorageOwner, TypeTable};
use alloc::vec::Vec;
use smol_str::SmolStr;

impl PreparedModule {
    /// Internal storage and SIZEOF ABI carried by this exact admitted artifact.
    pub fn storage_abi(&self) -> crate::value::StorageAbi {
        self.vm.storage_abi()
    }
    /// Admitted type of a reference-table destination, without a live storage borrow.
    pub fn reference_type(&self, index: u32) -> Option<u32> {
        self.vm.ref_type(index)
    }

    /// Inspect declared engineering aliases without exposing mutable admission data.
    pub fn access_bindings(&self) -> &[crate::bytecode::AccessBindingEntry] {
        &self.access.entries
    }
    /// Admitted logical process-image bindings and shared immutable wire codecs.
    pub fn io_bindings(&self) -> &[crate::io_image::PreparedIoBinding] {
        &self.io_bindings
    }
    /// Symbols retained by this installation, indexed by their artifact identity.
    pub fn strings(&self) -> &[SmolStr] {
        &self.vm.strings
    }

    /// Admitted executable types. No mutable type-table access is exposed.
    pub fn type_table(&self) -> &TypeTable {
        &self.vm.types
    }

    /// Admitted declaration identities, slots and qualifiers for read-only tooling.
    pub fn storage_declarations(&self) -> &[StorageDeclaration] {
        &self.layout.entries
    }

    /// Frame declarations selected through the preparation-time owner index.
    /// Looking up one invocation does not scan the application's whole layout.
    pub fn frame_declarations(&self, pou: u32) -> impl Iterator<Item = &StorageDeclaration> {
        self.declarations(StorageOwner::Frame, Some(pou))
            .iter()
            .filter_map(|id| self.layout.entries.get(*id as usize))
    }

    /// Instance declarations selected through the same preparation-time index.
    pub fn instance_declarations(&self, pou: u32) -> impl Iterator<Item = &StorageDeclaration> {
        self.declarations(StorageOwner::Instance, Some(pou))
            .iter()
            .filter_map(|id| self.layout.entries.get(*id as usize))
    }

    /// Resource and task descriptors for hosted scheduling and protocol metadata.
    pub fn resources(&self) -> &[ResourceEntry] {
        &self.resources.resources
    }

    /// Resolve an admitted POU identity for debugging and profiling presentation.
    pub fn pou_name(&self, id: u32) -> Option<&str> {
        self.vm.pou_name(id)
    }

    /// Read admitted POU identities and immutable frame/body descriptors.
    /// Iterator order is unspecified; presentation may sort by artifact id.
    pub fn pous(&self) -> impl Iterator<Item = (u32, &crate::vm::module::VmPouEntry)> {
        self.vm.pou_by_id.iter().map(|(id, entry)| (*id, entry))
    }

    /// Declared parameters of an admitted POU, in call binding order.
    pub fn pou_parameters(&self, id: u32) -> Option<&[crate::vm::module::VmParamMeta]> {
        self.vm.pou_params(id)
    }

    /// Admitted instance-construction roots; these contain artifact identities,
    /// never a license to cast those identities to mutable live storage ids.
    pub fn construction_roots(&self) -> &[crate::bytecode::ConstructionRoot] {
        &self.roots.entries
    }

    /// Whether an admitted POU has a function/method result slot.
    pub fn pou_has_return(&self, id: u32) -> bool {
        self.vm.pou_has_return_slot.contains(&id)
    }

    /// Parent POU for a derived function-block or class template.
    pub fn parent_pou(&self, id: u32) -> Option<u32> {
        self.vm.parent_pou_ids.get(&id).copied()
    }

    /// Interface type identities implemented by an admitted class-like POU.
    pub fn pou_interfaces(&self, id: u32) -> &[u32] {
        self.vm
            .interface_type_ids_by_pou
            .get(&id)
            .map_or(&[], Vec::as_slice)
    }

    /// Containing function-block/class POU for a method body.
    pub fn method_owner(&self, id: u32) -> Option<u32> {
        self.method_owners.get(&id).copied()
    }

    /// Closed POU category derived from the admitted execution indexes.
    pub fn pou_kind(&self, id: u32) -> Option<crate::bytecode::PouKind> {
        self.vm.pou_by_id.get(&id).map(|entry| entry.kind)
    }

    /// Read a prepared immutable constant for hosted tooling. Consumers cannot
    /// replace a constant or obtain mutable fields of admitted metadata.
    pub fn constant(&self, index: u32) -> Option<&crate::value::Value> {
        self.vm.consts.get(index as usize)
    }

    /// Source locations and symbols in a development composition.
    #[cfg(feature = "hir")]
    pub fn debug_map(&self) -> &crate::vm::debug_map::VmDebugMap {
        &self.vm.debug_map
    }
}
