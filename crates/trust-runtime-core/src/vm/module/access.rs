//! Read-only metadata access for hosted adapters.
use super::*;

impl VmModule {
    /// The admitted bytecode version.
    pub fn version(&self) -> crate::bytecode::BytecodeVersion {
        self.version
    }
    /// Instruction bytes retained by the module.
    pub fn code(&self) -> &[u8] {
        &self.code
    }
    /// Immutable symbol and literal strings.
    pub fn strings(&self) -> &[SmolStr] {
        &self.strings
    }
    /// Declared type metadata.
    pub fn types(&self) -> &TypeTable {
        &self.types
    }
    /// Encoded storage-reference descriptors.
    pub fn refs(&self) -> &[VmRef] {
        &self.refs
    }
    /// Materialized constant values.
    pub fn consts(&self) -> &[Value] {
        &self.consts
    }
    /// POU metadata indexed by artifact identity.
    pub fn pou_by_id(&self) -> &HashMap<u32, VmPouEntry> {
        &self.pou_by_id
    }
    /// Read-only debug source and symbol metadata.
    pub fn debug_map(&self) -> &debug_map::VmDebugMap {
        &self.debug_map
    }
    /// Legacy hosted instruction limit.
    pub fn instruction_budget(&self) -> usize {
        self.instruction_budget
    }
    /// Canonical names mapped to program identities.
    pub fn program_ids(&self) -> &HashMap<SmolStr, u32> {
        &self.program_ids
    }
    /// Canonical names mapped to function identities.
    pub fn function_ids(&self) -> &HashMap<SmolStr, u32> {
        &self.function_ids
    }
    /// Canonical names mapped to function block identities.
    pub fn function_block_ids(&self) -> &HashMap<SmolStr, u32> {
        &self.function_block_ids
    }
    /// Canonical names mapped to class identities.
    pub fn class_ids(&self) -> &HashMap<SmolStr, u32> {
        &self.class_ids
    }
    /// Declared inheritance parent identities.
    pub fn parent_pou_ids(&self) -> &HashMap<u32, u32> {
        &self.parent_pou_ids
    }
    /// Declared interface types for each POU.
    pub fn interface_type_ids_by_pou(&self) -> &HashMap<u32, Vec<u32>> {
        &self.interface_type_ids_by_pou
    }
    /// Resolved method identities for each owning POU.
    pub fn method_table_by_owner(&self) -> &HashMap<u32, HashMap<SmolStr, u32>> {
        &self.method_table_by_owner
    }
    /// Set the execution limit for an explicitly legacy hosted module.
    #[cfg(feature = "hir")]
    pub fn set_legacy_instruction_budget(&mut self, limit: usize) {
        assert_eq!(
            self.version.major, 1,
            "legacy adapter cannot alter source-free metadata"
        );
        self.instruction_budget = limit;
    }
}
