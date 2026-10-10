//! Immutable source-free preparation, distinct from decoded or validated bytecode.
use super::module::{invalid_bytecode, VmModule};
use crate::bytecode::{
    AccessBindings, BytecodeModule, ConstructionRoots, InitializerIndex, IoMap, ResourceMeta,
    SectionData, SectionId, StorageLayout, ValidationLimits,
};
use crate::error::RuntimeError;
use alloc::{collections::BTreeMap, vec::Vec};
mod admission;
mod budget;
mod indexes;
pub(crate) use budget::PreparationBudget;
pub use budget::PreparationUsage;

/// Explicit preparation limits for an allocator-backed execution profile.
#[derive(Debug, Clone, Copy)]
pub struct PreparationLimits {
    /// Maximum encoded artifact length accepted by the consumer.
    pub max_artifact_bytes: usize,
    /// Conservative logical preparation demand, separate from validation scratch.
    pub max_preparation_bytes: usize,
    /// Metadata visits, comparisons and expanded constant-node work.
    pub max_preparation_work: usize,
    /// Independent decoder-validation work and scratch limits.
    pub validation: ValidationLimits,
    /// Maximum cumulative typed construction nodes in one entry; retiring values does not refund it.
    pub max_construction_values: usize,
    /// Conservative cumulative scratch allocation charge per execution entry.
    pub max_construction_bytes: usize,
    /// Maximum combined input, output and marker image bytes.
    pub max_process_image_bytes: usize,
    /// Maximum shared instructions and construction operations per entry.
    pub max_work: usize,
    /// Maximum nested POU and default-recipe activations.
    pub max_call_depth: usize,
}

impl Default for PreparationLimits {
    fn default() -> Self {
        Self {
            max_artifact_bytes: crate::bytecode::BYTECODE_MAX_CONTAINER_BYTES,
            max_preparation_bytes: 256 * 1024 * 1024,
            max_preparation_work: 64 * 1024 * 1024,
            validation: ValidationLimits::default(),
            max_construction_values: 1_000_000,
            max_construction_bytes: 64 * 1024 * 1024,
            max_process_image_bytes: 16 * 1024 * 1024,
            max_work: super::VM_MAX_EXECUTED_INSTRUCTIONS,
            max_call_depth: super::VM_MAX_CALL_DEPTH,
        }
    }
}

/// A prepared STBC 2.0 application. Mutable execution state is created separately.
/// Fields are private so raw metadata cannot forge executable admission.
#[derive(Debug)]
pub struct PreparedModule {
    pub(crate) vm: VmModule,
    pub(crate) layout: StorageLayout,
    pub(crate) roots: ConstructionRoots,
    pub(crate) initializers: InitializerIndex,
    pub(crate) access: AccessBindings,
    pub(crate) resources: ResourceMeta,
    pub(crate) io_bindings: Vec<crate::io_image::PreparedIoBinding>,
    pub(crate) limits: PreparationLimits,
    pub(crate) requires_wall_clock: bool,
    preparation_usage: PreparationUsage,
    indexes: indexes::PreparedIndexes,
    pub(crate) stdlib: crate::stdlib::StandardLibrary,
    /// Canonical recipe context, type, member (None for type default), initializer id.
    pub(crate) method_owners: BTreeMap<u32, u32>,
    pub(crate) recipes: Vec<(u32, u32, Option<u32>, u32)>,
}

impl PreparedModule {
    /// Decode, fully validate and prepare an artifact without source or a compiler.
    pub fn from_bytes(bytes: &[u8], limits: PreparationLimits) -> Result<Self, RuntimeError> {
        if bytes.len() > limits.max_artifact_bytes {
            return Err(RuntimeError::PreparationLimit);
        }
        let (raw, decoded) = BytecodeModule::decode_with_limits(
            bytes,
            limits.max_preparation_bytes,
            limits.max_preparation_work,
        )
        .map_err(RuntimeError::from)?;
        Self::from_decoded_after_decode(
            &raw,
            limits,
            PreparationUsage {
                bytes: decoded.allocation_bytes,
                work: decoded.work,
            },
        )
    }

    /// Validate a decoded artifact before retaining immutable execution metadata.
    pub fn from_decoded(
        raw: &BytecodeModule,
        limits: PreparationLimits,
    ) -> Result<Self, RuntimeError> {
        Self::from_decoded_after_decode(raw, limits, PreparationUsage::default())
    }

    fn from_decoded_after_decode(
        raw: &BytecodeModule,
        limits: PreparationLimits,
        decoded: PreparationUsage,
    ) -> Result<Self, RuntimeError> {
        if limits.max_call_depth == 0
            || limits.max_call_depth > super::VM_MAX_CALL_DEPTH
            || limits.max_work == 0
            || limits.max_construction_values == 0
        {
            return Err(RuntimeError::ProfileUnsupported(
                "invalid preparation profile limits".into(),
            ));
        }
        // Use the one serializer's checked widths/alignment for struct-built inputs.
        // Drop transport scratch before materializing the execution representation.
        let mut preparation = PreparationBudget::new(limits);
        preparation.charge(decoded.bytes, decoded.work)?;
        // Serialization is independently bounded before its first buffer reservation.
        let encoded = raw
            .encode_with_limit(
                limits
                    .max_artifact_bytes
                    .min(preparation.remaining_bytes() / 4)
                    .min(preparation.remaining_work()),
            )
            .map_err(RuntimeError::from)?;
        let encoded_len = encoded.len();
        preparation.charge(
            encoded_len.checked_mul(4).ok_or(RuntimeError::Overflow)?,
            encoded_len,
        )?;
        drop(encoded);
        preparation.metadata(raw, encoded_len)?;
        let token = raw
            .validated_with_limits(limits.validation)
            .map_err(RuntimeError::from)?;
        match (
            token.section(SectionId::StorageLayout),
            token.section(SectionId::ConstructionRoots),
        ) {
            (
                Some(SectionData::StorageLayout(layout)),
                Some(SectionData::ConstructionRoots(roots)),
            ) => admission::check_construction_demand(layout, roots, limits)?,
            _ => return Err(invalid_bytecode("missing construction metadata")),
        }
        let mut vm = VmModule::from_source_free(&token, &mut preparation)?;
        for reference in &vm.refs {
            match reference {
                super::module::VmRef::Retain { .. } => {
                    return Err(super::VmTrap::UnsupportedRefLocation("retain").into_runtime_error())
                }
                super::module::VmRef::Io { .. } => {
                    return Err(super::VmTrap::UnsupportedRefLocation("io").into_runtime_error())
                }
                _ => {}
            }
        }
        vm.instruction_budget = limits.max_work;
        let layout = match token.section(SectionId::StorageLayout) {
            Some(SectionData::StorageLayout(value)) => value.clone(),
            _ => return Err(invalid_bytecode("missing STORAGE_LAYOUT")),
        };
        let roots = match token.section(SectionId::ConstructionRoots) {
            Some(SectionData::ConstructionRoots(value)) => value.clone(),
            _ => return Err(invalid_bytecode("missing CONSTRUCTION_ROOTS")),
        };
        let initializers = match token.section(SectionId::Initializers) {
            Some(SectionData::Initializers(value)) => value.clone(),
            _ => return Err(invalid_bytecode("missing INITIALIZERS")),
        };
        let access = match token.section(SectionId::AccessBindings) {
            Some(SectionData::AccessBindings(value)) => value.clone(),
            _ => AccessBindings::default(),
        };
        let resources = match token.section(SectionId::ResourceMeta) {
            Some(SectionData::ResourceMeta(value)) => value.clone(),
            _ => ResourceMeta::default(),
        };
        let io = match token.section(SectionId::IoMap) {
            Some(SectionData::IoMap(value)) => value.clone(),
            _ => IoMap::default(),
        };
        if resources.resources.len() != 1 {
            return Err(RuntimeError::ProfileUnsupported(
                "source-free execution requires exactly one resource".into(),
            ));
        }
        for resource in &resources.resources {
            let image_bytes = (resource.inputs_size as usize)
                .checked_add(resource.outputs_size as usize)
                .and_then(|size| size.checked_add(resource.memory_size as usize))
                .ok_or(RuntimeError::Overflow)?;
            if image_bytes > limits.max_process_image_bytes {
                return Err(RuntimeError::PreparationLimit);
            }
        }
        let io_bindings = io
            .bindings
            .iter()
            .map(|binding| {
                crate::io_image::PreparedIoBinding::from_tables(binding, &vm.types, &vm.strings)
            })
            .collect::<Result<Vec<_>, _>>()?;
        // Count the shared registration lists without allocating, then build once.
        let (registry_bytes, registry_work) = crate::stdlib::StandardLibrary::preparation_demand()?;
        preparation.charge(registry_bytes, registry_work)?;
        let stdlib = crate::stdlib::StandardLibrary::new();
        let requires_wall_clock =
            admission::check_imports(&vm, &initializers, &layout, &stdlib, &mut preparation)?;
        let method_owners = match token.section(SectionId::PouIndex) {
            Some(SectionData::PouIndex(index)) => index
                .entries
                .iter()
                .filter(|entry| entry.kind == crate::bytecode::PouKind::Method)
                .filter_map(|entry| entry.owner_pou_id.map(|owner| (entry.id, owner)))
                .collect(),
            _ => BTreeMap::new(),
        };
        let mut recipes = Vec::new();
        for (id, entry) in initializers.entries.iter().enumerate() {
            if let Some(type_id) = entry.recipe_type_id {
                recipes.push((
                    entry
                        .context_initializer_idx
                        .ok_or_else(|| invalid_bytecode("recipe has no context"))?,
                    type_id,
                    entry.recipe_member_idx,
                    u32::try_from(id).map_err(|_| RuntimeError::Overflow)?,
                ));
            }
        }
        recipes.sort_unstable_by_key(|entry| (entry.0, entry.1, entry.2));
        let indexes = indexes::PreparedIndexes::build(
            &vm,
            &layout,
            &roots,
            &initializers,
            &method_owners,
            &mut preparation,
        )?;
        let preparation_usage = preparation.usage();
        Ok(Self {
            preparation_usage,
            indexes,
            stdlib,
            vm,
            layout,
            roots,
            initializers,
            access,
            resources,
            io_bindings,
            limits,
            requires_wall_clock,
            method_owners,
            recipes,
        })
    }

    /// Conservative cumulative logical demand observed during preparation.
    pub fn preparation_usage(&self) -> PreparationUsage {
        self.preparation_usage
    }

    pub(crate) fn validate_services(
        &self,
        services: &dyn super::ExecutionServices,
    ) -> Result<(), RuntimeError> {
        if self.requires_wall_clock && !services.has_wall_clock() {
            return Err(RuntimeError::ProfileUnsupported(
                "CURRENT_DT requires an admitted platform wall clock".into(),
            ));
        }
        Ok(())
    }

    pub(crate) fn recipe(&self, context: u32, type_id: u32, member: Option<u32>) -> Option<u32> {
        self.recipes
            .binary_search_by_key(&(context, type_id, member), |entry| {
                (entry.0, entry.1, entry.2)
            })
            .ok()
            .map(|index| self.recipes[index].3)
    }
}
