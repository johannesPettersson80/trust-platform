#[path = "common/bytecode_helpers.rs"]
mod bytecode_helpers;

use trust_runtime_core::bytecode::*;

// Unique, reverse-ordered names and interleaved lookup requests expose insertion
// shifts and per-use table scans. References reach their independent format limit.
fn indexed_module(pou_count: usize, instruction_count: usize) -> BytecodeModule {
    let mut module = bytecode_helpers::base_module();
    let mut strings = bytecode_helpers::base_string_table();
    let mut pous = PouIndex::default();
    let mut refs = RefTable::default();
    let mut vars = VarMeta::default();
    let mut debug = DebugMap::default();
    let mut code = Vec::new();
    let slots = BYTECODE_MAX_REFERENCES / pou_count;
    for position in 0..pou_count {
        let id = (position + 1) as u32;
        let name_idx = if position == 0 {
            2
        } else {
            let at = strings.entries.len() as u32;
            strings
                .entries
                .push(format!("Function{:05}", pou_count - position).into());
            at
        };
        let symbol_idx = strings.entries.len() as u32;
        strings.entries.push("Function00001".into());
        // The call has no arguments; its conservative result is discarded.
        let start = code.len() as u32;
        code.push(0x09);
        code.extend_from_slice(&NATIVE_CALL_KIND_FUNCTION.to_le_bytes());
        code.extend_from_slice(&symbol_idx.to_le_bytes());
        code.extend_from_slice(&0u32.to_le_bytes());
        code.push(0x12);
        let count =
            instruction_count / pou_count + usize::from(position < instruction_count % pou_count);
        assert!(count >= 3);
        code.extend(std::iter::repeat_n(0x00, count - 3));
        code.push(0x06);
        let local_ref_start = refs.entries.len() as u32;
        for slot in 0..slots {
            let name_idx = strings.entries.len() as u32;
            strings
                .entries
                .push(format!("@local/{id}/{slot}/value").into());
            vars.entries.push(VarMetaEntry {
                name_idx,
                type_id: 0,
                ref_idx: refs.entries.len() as u32,
                retain: 0,
                init_const_idx: None,
            });
            refs.entries.push(RefEntry {
                location: RefLocation::Local,
                owner_id: id,
                offset: slot as u32,
                segments: vec![],
            });
            debug.entries.push(DebugEntry {
                pou_id: id,
                code_offset: start,
                file_idx: 0,
                line: slot as u32 + 1,
                column: 1,
                kind: 0,
            });
        }
        pous.entries.push(PouEntry {
            id,
            name_idx,
            kind: if position == 0 {
                PouKind::Program
            } else {
                PouKind::Function
            },
            code_offset: start,
            code_length: code.len() as u32 - start,
            local_ref_start,
            local_ref_count: slots as u32,
            return_type_id: None,
            owner_pou_id: None,
            params: vec![],
            class_meta: None,
        });
    }
    for section in &mut module.sections {
        section.data = match section.id {
            id if id == SectionId::StringTable.as_raw() => {
                SectionData::StringTable(core::mem::take(&mut strings))
            }
            id if id == SectionId::PouIndex.as_raw() => {
                SectionData::PouIndex(core::mem::take(&mut pous))
            }
            id if id == SectionId::RefTable.as_raw() => {
                SectionData::RefTable(core::mem::take(&mut refs))
            }
            id if id == SectionId::PouBodies.as_raw() => {
                SectionData::PouBodies(core::mem::take(&mut code))
            }
            _ => continue,
        };
    }
    for (id, data) in [
        (SectionId::VarMeta, SectionData::VarMeta(vars)),
        (SectionId::DebugMap, SectionData::DebugMap(debug)),
        (
            SectionId::DebugStringTable,
            SectionData::DebugStringTable(StringTable {
                entries: vec!["scale.st".into()],
            }),
        ),
    ] {
        module.sections.push(Section {
            id: id.as_raw(),
            flags: 0,
            data,
        });
    }
    module
}

#[test]
fn default_budget_admits_maximum_instruction_and_reference_fixture() {
    let module = indexed_module(4096, BYTECODE_MAX_INSTRUCTIONS);
    let stats = module
        .validated()
        .expect("maximum instructions/references with indexed metadata")
        .stats();
    assert!(stats.work < ValidationLimits::default().max_work);
    assert!(stats.peak_scratch_bytes < ValidationLimits::default().max_scratch_bytes);
}

#[test]
fn increasing_pou_density_does_not_multiply_metadata_scan_work() {
    // Same instruction/ref/debug totals, eight times as many POUs.
    let sparse = indexed_module(512, 131_072)
        .validated()
        .unwrap()
        .stats()
        .work;
    let dense = indexed_module(4096, 131_072)
        .validated()
        .unwrap()
        .stats()
        .work;
    assert!(dense < sparse * 2, "sparse={sparse}, dense={dense}");
}

#[test]
fn compound_local_range_errors_follow_documented_partition_precedence() {
    let mut module = indexed_module(4096, 16_384);
    if let Some(SectionData::RefTable(refs)) = module.section_mut(SectionId::RefTable) {
        refs.entries[1].owner_id = 999_999; // first range mixes owners
    }
    if let Some(SectionData::PouIndex(pous)) = module.section_mut(SectionId::PouIndex) {
        pous.entries[1].local_ref_start = 1; // later range overlaps it
    }
    assert_eq!(
        module.validate().unwrap_err(),
        BytecodeError::from(RejectionReason::PouLocalRefRangesOverlap)
    );
    if let Some(SectionData::PouIndex(pous)) = module.section_mut(SectionId::PouIndex) {
        pous.entries.last_mut().unwrap().local_ref_start = u32::MAX;
    }
    assert_eq!(
        module.validate().unwrap_err(),
        BytecodeError::from(RejectionReason::PouLocalRefRangeOverflow)
    );
}

#[test]
fn diagnostic_length_does_not_replace_the_actual_rejection_with_a_budget_error() {
    let mut module = bytecode_helpers::base_module();
    let peak = module.validated().unwrap().stats().peak_scratch_bytes;
    let name_idx = if let Some(SectionData::StringTable(strings)) =
        module.section_mut(SectionId::StringTable)
    {
        let at = strings.entries.len() as u32;
        strings.entries.push("z".repeat(4096).into());
        at
    } else {
        panic!("strings");
    };
    if let Some(SectionData::ResourceMeta(resources)) = module.section_mut(SectionId::ResourceMeta)
    {
        resources.resources[0].tasks[0].program_name_idx = vec![name_idx];
    }
    let error = module
        .validated_with_limits(ValidationLimits {
            max_scratch_bytes: peak,
            ..ValidationLimits::default()
        })
        .unwrap_err();
    match error {
        BytecodeError::InvalidSection(message) => {
            assert!(message.starts_with("task references unknown program '"))
        }
        other => panic!("lost original rejection: {other:?}"),
    }
}

// A source-free artifact whose one declaration is an array of `elements` BOOL values.
// Every construction table is present; only the declaration has content.
fn source_free_array_declaration(elements: i64) -> BytecodeModule {
    let mut module = bytecode_helpers::base_module();
    module.version = BytecodeVersion::SOURCE_FREE;
    module.flags = HEADER_FLAG_CRC32;
    let Some(SectionData::TypeTable(types)) = module.section_mut(SectionId::TypeTable) else {
        panic!("types");
    };
    types.entries.push(TypeEntry {
        kind: TypeKind::Array,
        name_idx: None,
        data: TypeData::Array {
            elem_type_id: 0,
            dims: vec![(0, elements - 1)],
        },
    });
    types.offsets = compute_type_offsets_for_entries(&types.entries).unwrap();
    let declaration = StorageDeclaration {
        owner: StorageOwner::Global,
        role: StorageRole::Variable,
        retain: 0,
        flags: 0,
        owner_pou_id: None,
        name_idx: 2,
        type_id: Some(1),
        slot: 0,
        ref_idx: None,
        default_const_idx: None,
        construction_nodes: 0,
        related_declaration_idx: None,
        source_name_idx: None,
    };
    module.sections.extend([
        Section {
            id: SectionId::StorageLayout.as_raw(),
            flags: 0,
            data: SectionData::StorageLayout(StorageLayout {
                entries: vec![declaration],
            }),
        },
        Section {
            id: SectionId::ConstructionRoots.as_raw(),
            flags: 0,
            data: SectionData::ConstructionRoots(ConstructionRoots::default()),
        },
        Section {
            id: SectionId::Initializers.as_raw(),
            flags: 0,
            data: SectionData::Initializers(InitializerIndex::default()),
        },
        Section {
            id: SectionId::AccessBindings.as_raw(),
            flags: 0,
            data: SectionData::AccessBindings(AccessBindings::default()),
        },
    ]);
    module
}

#[test]
fn construction_node_demand_admits_one_million_and_refuses_one_more() {
    // An array demands the root node plus one node per element: 1 + 999,999 nodes.
    assert_eq!(
        construction_demands(
            source_free_array_declaration(999_999).view(),
            ValidationLimits::default()
        ),
        Ok(vec![1_000_000])
    );
    // Sites: validate/construction/demand.rs:213-217 (`add`, reached from `include`)
    // and the per-declaration check at demand.rs:203. Both return
    // ConstructionDemandOverflow; `add` reports 1 + 1,000,000 nodes first.
    assert_eq!(
        construction_demands(
            source_free_array_declaration(1_000_000).view(),
            ValidationLimits::default()
        ),
        Err(BytecodeError::from(
            RejectionReason::ConstructionDemandOverflow
        ))
    );
}

#[test]
fn construction_record_limit_refuses_the_next_declaration_at_validation() {
    // Site: validate/construction.rs:58 (sections), reached before any declaration is
    // indexed. The count is one past the 65,536 limit.
    let mut module = source_free_array_declaration(1);
    let Some(SectionData::StorageLayout(layout)) = module.section_mut(SectionId::StorageLayout)
    else {
        panic!("layout");
    };
    let declaration = layout.entries[0].clone();
    layout.entries = vec![declaration; BYTECODE_MAX_CONSTRUCTION_RECORDS + 1];
    assert_eq!(
        module
            .validated_source_free(ValidationLimits::default())
            .unwrap_err(),
        BytecodeError::from(RejectionReason::InvalidConstructionRecord)
    );
}
