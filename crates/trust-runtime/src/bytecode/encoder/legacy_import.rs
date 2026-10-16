//! Hosted 1.1 construction context: original instructions remain authoritative.

use super::*;
use crate::bytecode::{PouIndex, PouKind, RefLocation, RefSegment, TypeData, VarMeta};
use crate::memory::{FrameId, MemoryLocation};
use crate::value::{ref_indices_from_iter, RefSegment as ValueSegment};

mod bindings;

pub(super) struct LegacySeed {
    index: PouIndex,
}

fn invalid(message: &str) -> BytecodeError {
    BytecodeError::InvalidSection(format!("legacy construction context: {message}").into())
}

#[cfg(test)]
pub(crate) fn build_module_from_legacy_context(
    supplied: &BytecodeModule,
    input: &crate::harness::LoweredApplication,
) -> Result<BytecodeModule, BytecodeError> {
    build_legacy_with_type_projection(supplied, input).map(|(module, _)| module)
}

pub(crate) fn build_legacy_with_type_projection(
    supplied: &BytecodeModule,
    input: &crate::harness::LoweredApplication,
) -> Result<(BytecodeModule, Vec<Option<TypeId>>), BytecodeError> {
    supplied.validate()?;
    if supplied.version != BytecodeVersion::new(1, 1) {
        return Err(invalid("expected STBC 1.1"));
    }
    if input.builder.storage_abi != crate::value::StorageAbi::LegacyTemporal32 {
        return Err(invalid("expected legacy temporal storage ABI"));
    }
    let mut encoder = BytecodeEncoder::from_view((&input.builder).into());
    encoder.authoring = Some(input);
    encoder.seed_legacy(supplied)?;
    encoder.collect_decl_types()?;
    encoder.reserve_source_globals()?;
    encoder.collect_instance_templates()?;
    encoder.construct_persistent_roots()?;
    encoder.collect_source_io()?;
    encoder.collect_source_configuration()?;
    // This common pass builds declaration scopes/signatures only: emit_pou_body
    // returns no instructions while a legacy seed is installed.
    let (generated, _, _) = encoder.build_pou_index_and_bodies()?;
    let index = encoder.reconcile_legacy_index(generated)?;
    encoder.collect_declaration_actions(&index)?;
    let mut bodies = match supplied.section(SectionId::PouBodies) {
        Some(SectionData::PouBodies(bytes)) => bytes.clone(),
        _ => return Err(invalid("missing ordinary bodies")),
    };
    encoder.emit_initializer_bodies(&mut bodies)?;
    let generated_meta = encoder.build_source_var_meta()?;
    encoder.check_legacy_bindings(supplied, &generated_meta)?;
    let resource = encoder.build_resource_meta()?;
    if supplied.section(SectionId::ResourceMeta) != Some(&SectionData::ResourceMeta(resource)) {
        return Err(invalid("resource configuration differs"));
    }
    let io = encoder.reconcile_legacy_io(supplied)?;
    let mut artifact_types = vec![None; encoder.types.len()];
    for (&ty, &index) in &encoder.type_map {
        *artifact_types
            .get_mut(index as usize)
            .ok_or_else(|| invalid("type projection index out of bounds"))? = Some(ty);
    }
    let mut sections = supplied.sections.clone();
    let types = TypeTable {
        offsets: compute_type_offsets_for_entries(&encoder.types)?,
        entries: encoder.types,
    };
    replace(
        &mut sections,
        SectionId::StringTable,
        SectionData::StringTable(encoder.strings.into_table()),
    );
    replace(
        &mut sections,
        SectionId::TypeTable,
        SectionData::TypeTable(types),
    );
    replace(
        &mut sections,
        SectionId::ConstPool,
        SectionData::ConstPool(ConstPool {
            entries: encoder.const_pool,
        }),
    );
    replace(
        &mut sections,
        SectionId::RefTable,
        SectionData::RefTable(RefTable {
            entries: encoder.ref_entries,
        }),
    );
    replace(&mut sections, SectionId::IoMap, SectionData::IoMap(io));
    replace(
        &mut sections,
        SectionId::PouIndex,
        SectionData::PouIndex(index),
    );
    replace(
        &mut sections,
        SectionId::PouBodies,
        SectionData::PouBodies(bodies),
    );
    let mut metadata = match supplied.section(SectionId::VarMeta) {
        Some(SectionData::VarMeta(meta)) => meta.clone(),
        _ => VarMeta::default(),
    };
    for entry in generated_meta.entries {
        if !metadata
            .entries
            .iter()
            .any(|old| old.ref_idx == entry.ref_idx)
        {
            metadata.entries.push(entry);
        }
    }
    replace(
        &mut sections,
        SectionId::VarMeta,
        SectionData::VarMeta(metadata),
    );
    let result = finish_module(
        sections,
        true,
        encoder.construction,
        BytecodeVersion::SOURCE_FREE,
        crate::bytecode::HEADER_FLAG_CRC32 | crate::bytecode::HEADER_FLAG_LEGACY_TEMPORAL32,
    )?;
    result.validate()?;
    Ok((result, artifact_types))
}

fn replace(sections: &mut Vec<Section>, id: SectionId, data: SectionData) {
    if let Some(section) = sections
        .iter_mut()
        .find(|section| section.id == id.as_raw())
    {
        section.data = data;
    } else {
        sections.push(Section {
            id: id.as_raw(),
            flags: 0,
            data,
        });
    }
}

impl BytecodeEncoder<'_> {
    fn seed_legacy(&mut self, supplied: &BytecodeModule) -> Result<(), BytecodeError> {
        let strings = match supplied.section(SectionId::StringTable) {
            Some(SectionData::StringTable(table)) => table.clone(),
            _ => return Err(invalid("missing strings")),
        };
        self.strings.entries = strings.entries;
        for (index, text) in self.strings.entries.iter().enumerate() {
            self.strings
                .index
                .entry(text.clone())
                .or_insert(u32::try_from(index).map_err(|_| invalid("string identity exhausted"))?);
        }
        self.types = match supplied.section(SectionId::TypeTable) {
            Some(SectionData::TypeTable(table)) => table.entries.clone(),
            _ => return Err(invalid("missing types")),
        };
        self.const_pool = match supplied.section(SectionId::ConstPool) {
            Some(SectionData::ConstPool(table)) => table.entries.clone(),
            _ => return Err(invalid("missing constants")),
        };
        self.ref_entries = match supplied.section(SectionId::RefTable) {
            Some(SectionData::RefTable(table)) => table.entries.clone(),
            _ => return Err(invalid("missing references")),
        };
        let index = match supplied.section(SectionId::PouIndex) {
            Some(SectionData::PouIndex(index)) => index.clone(),
            _ => return Err(invalid("missing POUs")),
        };
        self.seed_pou_ids(&index)?;
        for (id, reference) in self.ref_entries.iter().enumerate() {
            let location = match reference.location {
                RefLocation::Global => MemoryLocation::Global,
                RefLocation::Local => {
                    self.next_local_frame_id = self.next_local_frame_id.max(
                        reference
                            .owner_id
                            .checked_add(1)
                            .ok_or_else(|| invalid("frame identity exhausted"))?,
                    );
                    MemoryLocation::Local(FrameId(reference.owner_id))
                }
                RefLocation::Instance => MemoryLocation::Instance(InstanceId(reference.owner_id)),
                RefLocation::Retain => MemoryLocation::Retain,
                RefLocation::Io => MemoryLocation::Io(match reference.owner_id {
                    0 => crate::memory::IoArea::Input,
                    1 => crate::memory::IoArea::Output,
                    2 => crate::memory::IoArea::Memory,
                    _ => return Err(invalid("invalid I/O owner")),
                }),
                _ => return Err(invalid("non-legacy reference domain")),
            };
            let path = reference
                .segments
                .iter()
                .map(|segment| match segment {
                    RefSegment::Field { name_idx } => Ok(ValueSegment::Field(
                        self.strings
                            .entries
                            .get(*name_idx as usize)
                            .ok_or_else(|| invalid("field name missing"))?
                            .clone(),
                    )),
                    RefSegment::Index(indices) => Ok(ValueSegment::Index(ref_indices_from_iter(
                        indices.iter().copied(),
                    ))),
                })
                .collect::<Result<_, BytecodeError>>()?;
            self.ref_map
                .entry(ValueRef {
                    location,
                    offset: reference.offset as usize,
                    path,
                })
                .or_insert(u32::try_from(id).map_err(|_| invalid("reference identity exhausted"))?);
        }
        // Seed named roots first, then recursively bind anonymous component types.
        let named = self
            .types
            .iter()
            .enumerate()
            .filter_map(|(id, entry)| {
                let name = self.strings.entries.get(entry.name_idx? as usize)?;
                Some((self.runtime.registry().lookup(name)?, id as u32))
            })
            .collect::<Vec<_>>();
        for (ty, id) in named {
            self.bind_legacy_type(ty, id)?;
        }
        self.seed_legacy_declaration_types(supplied, &index)?;
        self.legacy = Some(LegacySeed { index });
        let mappings = self.type_map.clone();
        for (ty, id) in mappings {
            let source = self
                .runtime
                .registry()
                .get(ty)
                .cloned()
                .ok_or_else(|| invalid("context type missing"))?;
            let expected = self.encode_type_entry(ty, &source)?;
            let mut legacy_expected = expected.clone();
            if matches!(
                legacy_expected.kind,
                crate::bytecode::TypeKind::OverlappingStruct
                    | crate::bytecode::TypeKind::LocatedStruct
            ) {
                legacy_expected.kind = crate::bytecode::TypeKind::Struct;
                if let TypeData::Struct { fields } = &mut legacy_expected.data {
                    for field in fields {
                        field.bit_offset = None;
                    }
                }
            }
            if legacy_expected != self.types[id as usize] {
                return Err(invalid("type definition differs"));
            }
            // 1.1 did not encode relative locations. Its explicit, checked source
            // context supplies them without replacing any supplied instructions.
            self.types[id as usize] = expected;
        }
        Ok(())
    }

    fn seed_pou_ids(&mut self, index: &PouIndex) -> Result<(), BytecodeError> {
        let mut ids = PouIdMap {
            next_id: 0,
            programs: HashMap::new(),
            program_instances: HashMap::new(),
            function_blocks: HashMap::new(),
            functions: HashMap::new(),
            classes: HashMap::new(),
            methods: HashMap::new(),
        };
        let input = self.authoring.ok_or_else(|| invalid("context missing"))?;
        let configuration = input
            .configuration
            .as_ref()
            .ok_or_else(|| invalid("context is not prepared"))?;
        for entry in &index.entries {
            ids.next_id = ids.next_id.max(
                entry
                    .id
                    .checked_add(1)
                    .ok_or_else(|| invalid("POU identity exhausted"))?,
            );
            let name = self
                .strings
                .entries
                .get(entry.name_idx as usize)
                .ok_or_else(|| invalid("POU name missing"))?;
            let key = normalize_name(name);
            let duplicate = match entry.kind {
                PouKind::Program => {
                    let binding = configuration
                        .programs
                        .iter()
                        .find(|program| program.name.eq_ignore_ascii_case(name))
                        .ok_or_else(|| invalid("program instance has no context binding"))?;
                    let instance_key = normalize_name(&binding.name);
                    if ids
                        .program_instances
                        .insert(instance_key, entry.id)
                        .is_some()
                    {
                        true
                    } else {
                        ids.programs
                            .entry(normalize_name(&binding.type_name))
                            .or_insert(entry.id);
                        false
                    }
                }
                PouKind::FunctionBlock => ids.function_blocks.insert(key, entry.id).is_some(),
                PouKind::Function => ids.functions.insert(key, entry.id).is_some(),
                PouKind::Class => ids.classes.insert(key, entry.id).is_some(),
                PouKind::Method => {
                    let owner = index
                        .entries
                        .iter()
                        .find(|owner| Some(owner.id) == entry.owner_pou_id)
                        .ok_or_else(|| invalid("method owner missing"))?;
                    let owner_name = self
                        .strings
                        .entries
                        .get(owner.name_idx as usize)
                        .ok_or_else(|| invalid("owner name missing"))?;
                    ids.methods
                        .insert(MethodKey::new(owner_name, name), entry.id)
                        .is_some()
                }
            };
            if duplicate {
                return Err(invalid("ambiguous POU binding"));
            }
        }
        for program in &configuration.programs {
            if !ids
                .program_instances
                .contains_key(&normalize_name(&program.name))
            {
                return Err(invalid("configured program instance has no supplied POU"));
            }
        }
        self.pou_ids = ids;
        Ok(())
    }

    fn bind_legacy_type(&mut self, ty: TypeId, id: u32) -> Result<(), BytecodeError> {
        if let Some(previous) = self.type_map.get(&ty) {
            return if *previous == id {
                Ok(())
            } else {
                Err(invalid("ambiguous type identity"))
            };
        }
        let source = self
            .runtime
            .registry()
            .get(ty)
            .cloned()
            .ok_or_else(|| invalid("unknown context type"))?;
        let wire = self
            .types
            .get(id as usize)
            .cloned()
            .ok_or_else(|| invalid("unknown artifact type"))?;
        self.type_map.insert(ty, id);
        use trust_hir::Type;
        match (source, wire.data) {
            (Type::Array { element, .. }, TypeData::Array { elem_type_id, .. }) => {
                self.bind_legacy_type(element, elem_type_id)?
            }
            (Type::Struct { fields, .. }, TypeData::Struct { fields: target }) => {
                if fields.len() != target.len() {
                    return Err(invalid("field count differs"));
                }
                for (field, target) in fields.iter().zip(target) {
                    self.bind_legacy_type(field.type_id, target.type_id)?;
                }
            }
            (Type::Union { variants, .. }, TypeData::Union { fields }) => {
                if variants.len() != fields.len() {
                    return Err(invalid("variant count differs"));
                }
                for (variant, field) in variants.iter().zip(fields) {
                    self.bind_legacy_type(variant.type_id, field.type_id)?;
                }
            }
            (Type::Alias { target, .. }, TypeData::Alias { target_type_id })
            | (Type::Reference { target }, TypeData::Reference { target_type_id })
            | (Type::Pointer { target }, TypeData::Reference { target_type_id }) => {
                self.bind_legacy_type(target, target_type_id)?
            }
            (Type::Enum { base, .. }, TypeData::Enum { base_type_id, .. })
            | (
                Type::Enum { base, .. },
                TypeData::Alias {
                    target_type_id: base_type_id,
                },
            )
            | (Type::Subrange { base, .. }, TypeData::Subrange { base_type_id, .. }) => {
                self.bind_legacy_type(base, base_type_id)?
            }
            _ => {}
        }
        Ok(())
    }

    pub(super) fn legacy_local_scope(
        &self,
        pou: u32,
        names: Vec<SmolStr>,
    ) -> Result<LocalScope, BytecodeError> {
        let entry = self
            .legacy
            .as_ref()
            .and_then(|seed| seed.index.entries.iter().find(|entry| entry.id == pou))
            .ok_or_else(|| invalid("local owner missing"))?;
        if names.len() > entry.local_ref_count as usize {
            return Err(invalid("frame is smaller than declared locals"));
        }
        let mut locals = HashMap::new();
        for (offset, name) in names.into_iter().enumerate() {
            let id = entry
                .local_ref_start
                .checked_add(offset as u32)
                .ok_or_else(|| invalid("local range overflow"))?;
            let reference = self
                .ref_entries
                .get(id as usize)
                .ok_or_else(|| invalid("local reference missing"))?;
            if reference.location != RefLocation::Local
                || reference.offset != offset as u32
                || !reference.segments.is_empty()
            {
                return Err(invalid("declared local slot differs"));
            }
            if locals
                .insert(
                    normalize_name(&name),
                    ValueRef {
                        location: MemoryLocation::Local(FrameId(reference.owner_id)),
                        offset,
                        path: Default::default(),
                    },
                )
                .is_some()
            {
                return Err(invalid("duplicate local name"));
            }
        }
        Ok(LocalScope {
            locals,
            local_ref_start: entry.local_ref_start,
            local_ref_count: entry.local_ref_count,
            for_temp_pairs: Vec::new(),
        })
    }

    fn reconcile_legacy_index(&self, generated: PouIndex) -> Result<PouIndex, BytecodeError> {
        let mut supplied = self
            .legacy
            .as_ref()
            .ok_or_else(|| invalid("seed missing"))?
            .index
            .clone();
        let generated_non_programs = generated
            .entries
            .iter()
            .filter(|entry| entry.kind != PouKind::Program)
            .count();
        let supplied_non_programs = supplied
            .entries
            .iter()
            .filter(|entry| entry.kind != PouKind::Program)
            .count();
        if generated_non_programs != supplied_non_programs {
            return Err(invalid("non-program POU set differs"));
        }
        let config = self
            .authoring
            .and_then(|input| input.configuration.as_ref())
            .ok_or_else(|| invalid("configuration missing"))?;
        let mut program_types = Vec::<&str>::new();
        for program in &config.programs {
            if !program_types
                .iter()
                .any(|name| name.eq_ignore_ascii_case(&program.type_name))
            {
                program_types.push(&program.type_name);
            }
        }
        let generated_programs = generated
            .entries
            .iter()
            .filter(|entry| entry.kind == PouKind::Program)
            .collect::<Vec<_>>();
        if generated_programs.len() != program_types.len()
            || generated_programs.iter().any(|entry| {
                self.strings
                    .entries
                    .get(entry.name_idx as usize)
                    .is_none_or(|name| {
                        !program_types
                            .iter()
                            .any(|type_name| name.eq_ignore_ascii_case(type_name))
                    })
            })
        {
            return Err(invalid("program type POU set differs"));
        }
        for old in &mut supplied.entries {
            let new = if old.kind == PouKind::Program {
                let instance_name = &self.strings.entries[old.name_idx as usize];
                let configured = config
                    .programs
                    .iter()
                    .find(|program| program.name.eq_ignore_ascii_case(instance_name))
                    .ok_or_else(|| invalid("program instance has no context binding"))?;
                generated
                    .entries
                    .iter()
                    .find(|entry| {
                        entry.kind == PouKind::Program
                            && self
                                .strings
                                .entries
                                .get(entry.name_idx as usize)
                                .is_some_and(|name| {
                                    name.eq_ignore_ascii_case(&configured.type_name)
                                })
                    })
                    .ok_or_else(|| invalid("program type context missing"))?
            } else {
                generated
                    .entries
                    .iter()
                    .find(|entry| entry.id == old.id)
                    .ok_or_else(|| invalid("POU context missing"))?
            };
            if old.local_ref_count != new.local_ref_count {
                return Err(invalid("POU frame differs"));
            }
            let mut comparable = new.clone();
            comparable.id = old.id;
            comparable.name_idx = old.name_idx; // Configured program instance vs declaration type.
            comparable.local_ref_start = old.local_ref_start;
            comparable.local_ref_count = old.local_ref_count;
            comparable.code_offset = old.code_offset;
            comparable.code_length = old.code_length;
            if old.kind == PouKind::FunctionBlock
                && old.params.is_empty()
                && self.is_stdlib_fb(&self.strings.entries[old.name_idx as usize])
            {
                old.params = new.params.clone();
            }
            // Equivalent default payloads can be appended during signature construction;
            // the supplied constant indices remain authoritative.
            if old.params.len() != comparable.params.len() {
                return Err(invalid("parameter count differs"));
            }
            for (original, candidate) in old.params.iter().zip(&mut comparable.params) {
                if original.default_const_idx != candidate.default_const_idx {
                    let original_value = original
                        .default_const_idx
                        .and_then(|id| self.const_pool.get(id as usize));
                    let candidate_value = candidate
                        .default_const_idx
                        .and_then(|id| self.const_pool.get(id as usize));
                    // 2.0 represents ordinary defaults as actions rather than constants.
                    // Legacy constants remain authoritative and mark that parameter present.
                    if candidate.default_const_idx.is_some() && original_value != candidate_value {
                        return Err(invalid("parameter default differs"));
                    }
                    candidate.default_const_idx = original.default_const_idx;
                }
            }
            if *old != comparable {
                return Err(invalid("POU signature or frame differs"));
            }
        }
        Ok(supplied)
    }

    // The sole I/O enrichment is the existing 2.0 bounded STRING codec rule.
    // Addresses, target references, ordering and destination types remain supplied.
    fn reconcile_legacy_io(
        &mut self,
        supplied: &BytecodeModule,
    ) -> Result<crate::bytecode::IoMap, BytecodeError> {
        let generated = self.build_io_map()?;
        let Some(SectionData::IoMap(original)) = supplied.section(SectionId::IoMap) else {
            return Err(invalid("missing I/O map"));
        };
        if original.bindings.len() != generated.bindings.len() {
            return Err(invalid("I/O binding count differs"));
        }
        let mut result = original.clone();
        for (old, new) in result.bindings.iter_mut().zip(generated.bindings) {
            if old.address_str_idx != new.address_str_idx || old.ref_idx != new.ref_idx {
                return Err(invalid("I/O address or destination differs"));
            }
            if old.type_id == new.type_id {
                continue;
            }
            let old_type = old.type_id.and_then(|id| self.types.get(id as usize));
            let new_type = new.type_id.and_then(|id| self.types.get(id as usize));
            let address =
                crate::io::IoAddress::parse(&self.strings.entries[old.address_str_idx as usize])
                    .map_err(|_| invalid("invalid I/O address"))?;
            match (
                old_type.map(|entry| &entry.data),
                new_type.map(|entry| &entry.data),
            ) {
                (
                    Some(TypeData::Primitive {
                        prim_id: 24,
                        max_length: 0,
                    }),
                    Some(TypeData::Primitive {
                        prim_id: 24,
                        max_length,
                    }),
                ) if *max_length != 0 => {
                    // STRING capacity owns the effective byte window, including
                    // the legacy %QB base spelling. Retain the supplied address.
                    let codec = trust_runtime_core::io_image::IoValueCodec {
                        string_capacity: Some(*max_length),
                        ..Default::default()
                    };
                    codec
                        .address(address)
                        .map_err(|_| invalid("I/O codec differs"))?;
                    old.type_id = new.type_id
                }
                _ => return Err(invalid("I/O codec differs")),
            }
        }
        Ok(result)
    }

    fn check_legacy_bindings(
        &self,
        supplied: &BytecodeModule,
        generated: &VarMeta,
    ) -> Result<(), BytecodeError> {
        let Some(SectionData::VarMeta(original)) = supplied.section(SectionId::VarMeta) else {
            return Ok(());
        };
        for old in &original.entries {
            let name = self
                .strings
                .entries
                .get(old.name_idx as usize)
                .ok_or_else(|| invalid("binding name missing"))?;
            let reference = self
                .ref_entries
                .get(old.ref_idx as usize)
                .ok_or_else(|| invalid("binding reference missing"))?;
            match reference.location {
                RefLocation::Global => {
                    let binding = self
                        .construction
                        .bindings
                        .global(name)
                        .ok_or_else(|| invalid("global name missing"))?;
                    if self.ref_map.get(binding) != Some(&old.ref_idx) {
                        return Err(invalid("global slot differs"));
                    }
                }
                RefLocation::Local => {
                    if !self
                        .local_var_meta
                        .iter()
                        .any(|local| local.ref_idx == old.ref_idx && local.name == *name)
                    {
                        return Err(invalid("local name or slot differs"));
                    }
                }
                RefLocation::Instance => {
                    let input = self.authoring.ok_or_else(|| invalid("context missing"))?;
                    let config = input
                        .configuration
                        .as_ref()
                        .ok_or_else(|| invalid("configuration missing"))?;
                    let (program, field) = config
                        .programs
                        .iter()
                        .find_map(|program| {
                            name.strip_prefix(program.name.as_str())
                                .and_then(|tail| tail.strip_prefix('.'))
                                .map(|field| (program, field))
                        })
                        .ok_or_else(|| invalid("instance binding name missing"))?;
                    let root = self
                        .construction
                        .bindings
                        .global(&program.name)
                        .ok_or_else(|| invalid("program root missing"))?;
                    let instance = self
                        .construction
                        .bindings
                        .values_to_instances
                        .get(root)
                        .copied()
                        .ok_or_else(|| invalid("program instance missing"))?;
                    let binding = self
                        .construction
                        .bindings
                        .instance(instance, &field.into())
                        .ok_or_else(|| invalid("member missing"))?;
                    if self.ref_map.get(binding) != Some(&old.ref_idx) {
                        return Err(invalid("instance owner or slot differs"));
                    }
                }
                _ => return Err(invalid("unexpected declaration reference domain")),
            }
            let Some(new) = generated
                .entries
                .iter()
                .find(|entry| entry.ref_idx == old.ref_idx)
            else {
                return Err(invalid("declared binding has no construction context"));
            };
            if old.type_id != new.type_id || old.retain != new.retain {
                return Err(invalid("binding type or retention differs"));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
