//! Immutable source-free preparation, distinct from decoded or validated bytecode.
use super::module::{invalid_bytecode, VmModule};
use super::validated_source::{OwnedValidatedSource, ValidatedSource};
use crate::bytecode::{
    AccessBindings, BytecodeModule, ConstructionRoots, InitializerIndex, ResourceMeta, SectionData,
    SectionId, StorageLayout, ValidationLimits,
};
use crate::error::RuntimeError;
use alloc::{collections::BTreeMap, vec::Vec};
mod admission;
mod budget;
mod indexes;
mod metadata;
#[cfg(feature = "std")]
mod reference_names;
pub(crate) use budget::PreparationBudget;
pub use budget::PreparationUsage;

/// Explicit preparation limits for an allocator-backed execution profile.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
    /// Maximum work for one scan, engineering write or call-time initializer entry
    /// (spec 34 TIME-08).
    pub max_work: usize,
    /// Maximum work for one construction entry: instantiate, restart or candidate
    /// construction (spec 34 TIME-09).
    pub max_construction_work: usize,
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
            max_construction_work: 64 * 1024 * 1024,
            max_call_depth: super::VM_MAX_CALL_DEPTH,
        }
    }
}

/// A prepared source-free STBC application with immutable version-selected storage ABI.
/// Mutable execution state is created separately.
/// Fields are private so raw metadata cannot forge executable admission.
#[derive(Debug)]
pub struct PreparedModule {
    pub(crate) vm: VmModule,
    #[cfg(feature = "register-ir")]
    pub(crate) register_plans: super::register_ir::PreparedRegisterPlans,
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
    /// Preserve the admitted profile's limits when preparing a replacement.
    pub fn limits(&self) -> PreparationLimits {
        self.limits
    }

    /// Decode, fully validate and prepare an artifact without source or a compiler.
    pub fn from_bytes(bytes: &[u8], limits: PreparationLimits) -> Result<Self, RuntimeError> {
        let (raw, transport) = Self::decode_input(bytes, limits)?;
        Self::check_limits(limits)?;
        let mut preparation = PreparationBudget::new(limits);
        preparation.charge(transport.bytes, transport.work)?;
        let source =
            OwnedValidatedSource::new(raw, limits.validation).map_err(RuntimeError::from)?;
        Self::prepare_source(source, preparation, limits, bytes.len())
    }

    /// Hosted tooling may retain the immutable decoded artifact alongside its
    /// prepared engine. Decoding/validation happen once; the byte route still
    /// has no serializer dependency and the ordinary consumer drops the artifact.
    #[cfg(feature = "std")]
    pub fn from_bytes_with_artifact(
        bytes: &[u8],
        limits: PreparationLimits,
    ) -> Result<(Self, BytecodeModule), RuntimeError> {
        let (raw, transport) = Self::decode_input(bytes, limits)?;
        let prepared = Self::prepare_validated_metadata(&raw, limits, transport, bytes.len())?;
        Ok((prepared, raw))
    }

    fn decode_input(
        bytes: &[u8],
        limits: PreparationLimits,
    ) -> Result<(BytecodeModule, PreparationUsage), RuntimeError> {
        if bytes.len() > limits.max_artifact_bytes {
            return Err(RuntimeError::PreparationLimit);
        }
        let (raw, decoded) = BytecodeModule::decode_with_limits(
            bytes,
            limits.max_preparation_bytes,
            limits.max_preparation_work,
        )
        .map_err(RuntimeError::from)?;
        Ok((
            raw,
            PreparationUsage {
                bytes: decoded.allocation_bytes,
                work: decoded.work,
            },
        ))
    }

    /// Validate a decoded artifact before retaining immutable execution metadata.
    pub fn from_decoded(
        raw: &BytecodeModule,
        limits: PreparationLimits,
    ) -> Result<Self, RuntimeError> {
        Self::check_limits(limits)?;
        // Struct-built containers have not passed the byte decoder's framing,
        // widths and extent checks. Keep their independently bounded serializer.
        let encoded = raw
            .encode_with_limit(
                limits
                    .max_artifact_bytes
                    .min(limits.max_preparation_bytes / 4)
                    .min(limits.max_preparation_work),
            )
            .map_err(RuntimeError::from)?;
        let encoded_len = encoded.len();
        let usage = PreparationUsage {
            bytes: encoded_len.checked_mul(4).ok_or(RuntimeError::Overflow)?,
            work: encoded_len,
        };
        drop(encoded);
        Self::prepare_validated_metadata(raw, limits, usage, encoded_len)
    }

    fn check_limits(limits: PreparationLimits) -> Result<(), RuntimeError> {
        if limits.max_call_depth == 0
            || limits.max_call_depth > super::VM_MAX_CALL_DEPTH
            || limits.max_work == 0
            || limits.max_construction_work == 0
            || limits.max_construction_values == 0
        {
            return Err(RuntimeError::ProfileUnsupported(
                smol_str::SmolStr::new_static("invalid preparation profile limits"),
            ));
        }
        Ok(())
    }

    fn prepare_validated_metadata(
        raw: &BytecodeModule,
        limits: PreparationLimits,
        transport: PreparationUsage,
        encoded_len: usize,
    ) -> Result<Self, RuntimeError> {
        Self::check_limits(limits)?;
        // Byte inputs already passed framing and checked payload decoding.
        // Charge the actual route's cumulative transport demand, never a
        // synthetic re-encoding allocation which the byte route does not make.
        let mut preparation = PreparationBudget::new(limits);
        preparation.charge(transport.bytes, transport.work)?;
        // The bounded full validator owns malformed-artifact diagnostics.
        // Metadata accounting inspects admitted instructions; running that scan
        // first would replace precise wire errors with generic VM errors.
        let token = raw
            .validated_with_limits(limits.validation)
            .map_err(RuntimeError::from)?;
        Self::prepare_source(token, preparation, limits, encoded_len)
    }

    fn prepare_source(
        mut token: impl ValidatedSource,
        mut preparation: PreparationBudget,
        limits: PreparationLimits,
        encoded_len: usize,
    ) -> Result<Self, RuntimeError> {
        preparation.metadata(token.view(), encoded_len)?;
        match (
            token.section(SectionId::StorageLayout),
            token.section(SectionId::ConstructionRoots),
        ) {
            (
                Some(SectionData::StorageLayout(layout)),
                Some(SectionData::ConstructionRoots(roots)),
            ) => admission::check_construction_demand(layout, roots, limits)?,
            _ => {
                return Err(invalid_bytecode(smol_str::SmolStr::new_static(
                    "missing construction metadata",
                )))
            }
        }
        let mut vm = VmModule::from_source_free(&mut token, &mut preparation)?;
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
        let layout = match token.take_layout() {
            Some(value) => value,
            _ => {
                return Err(invalid_bytecode(smol_str::SmolStr::new_static(
                    "missing STORAGE_LAYOUT",
                )))
            }
        };
        let roots = match token.take_roots() {
            Some(value) => value,
            _ => {
                return Err(invalid_bytecode(smol_str::SmolStr::new_static(
                    "missing CONSTRUCTION_ROOTS",
                )))
            }
        };
        let initializers = match token.take_initializers() {
            Some(value) => value,
            _ => {
                return Err(invalid_bytecode(smol_str::SmolStr::new_static(
                    "missing INITIALIZERS",
                )))
            }
        };
        let access = token.take_access().unwrap_or_default();
        let resources = token.take_resources().unwrap_or_default();
        let io = match token.section(SectionId::IoMap) {
            Some(SectionData::IoMap(value)) => value.bindings.as_slice(),
            _ => &[],
        };
        if resources.resources.len() != 1 {
            return Err(RuntimeError::ProfileUnsupported(
                smol_str::SmolStr::new_static(
                    "source-free execution requires exactly one resource",
                ),
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
            .iter()
            .map(|binding| {
                crate::io_image::PreparedIoBinding::from_tables(binding, &vm.types, &vm.strings)
            })
            .collect::<Result<Vec<_>, _>>()?;
        // Count the shared registration lists without allocating, then build once.
        let (registry_bytes, registry_work) = crate::stdlib::StandardLibrary::preparation_demand();
        preparation.charge(registry_bytes, registry_work)?;
        let stdlib = crate::stdlib::StandardLibrary::new();
        let requires_wall_clock =
            admission::check_imports(&vm, &initializers, &layout, &stdlib, &mut preparation)?;
        let mut method_owners = BTreeMap::new();
        if let Some(SectionData::PouIndex(index)) = token.section(SectionId::PouIndex) {
            for entry in &index.entries {
                preparation.charge(0, 1)?;
                if entry.kind == crate::bytecode::PouKind::Method
                    && let Some(owner) = entry.owner_pou_id
                {
                    preparation
                        .charge(0, 12 * (method_owners.len().max(1).ilog2() as usize + 1))?;
                    method_owners.insert(entry.id, owner);
                }
            }
        }
        let mut recipes = Vec::new();
        for (id, entry) in initializers.entries.iter().enumerate() {
            if let Some(type_id) = entry.recipe_type_id {
                recipes.push((
                    entry.context_initializer_idx.ok_or_else(|| {
                        invalid_bytecode(smol_str::SmolStr::new_static("recipe has no context"))
                    })?,
                    type_id,
                    entry.recipe_member_idx,
                    u32::try_from(id).map_err(|_| RuntimeError::Overflow)?,
                ));
            }
        }
        indexes::sort::sort_by(&mut recipes, &mut preparation, &mut |left, right, _| {
            Ok((left.0, left.1, left.2).cmp(&(right.0, right.1, right.2)))
        })?;
        let indexes = indexes::PreparedIndexes::build(
            &vm,
            &layout,
            &roots,
            &initializers,
            &method_owners,
            &resources,
            &mut preparation,
        )?;
        #[cfg(feature = "register-ir")]
        let register_plans =
            super::register_ir::PreparedRegisterPlans::prepare(&vm, |bytes, work| {
                preparation.charge(bytes, work)
            })?;
        let preparation_usage = preparation.usage();
        Ok(Self {
            preparation_usage,
            indexes,
            stdlib,
            vm,
            #[cfg(feature = "register-ir")]
            register_plans,
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
                smol_str::SmolStr::new_static(
                    "CURRENT_DT requires an admitted platform wall clock",
                ),
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
