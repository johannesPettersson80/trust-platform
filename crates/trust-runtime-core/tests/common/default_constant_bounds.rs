//! Validated zero-body defaults must pay runtime construction charges separately.
use super::*;
use trust_runtime_core::bytecode::{
    compute_type_offsets_for_entries, ConstEntry, ConstructionRoot, InitializationPhase, RefEntry,
    RefLocation, StorageDeclaration, StorageOwner, StorageRole, TypeData, TypeEntry, TypeKind,
    VarMetaEntry,
};
use trust_runtime_core::error::StableErrorCode;
use trust_runtime_core::value::Value;
use trust_runtime_core::vm::{PreparationLimits, PreparedModule};

pub(super) fn owned_default() -> BytecodeModule {
    let mut module = value_default_module();
    let SectionData::TypeTable(types) = module.section_mut(SectionId::TypeTable).unwrap() else {
        unreachable!()
    };
    let ty = types.entries.len() as u32;
    types.entries.push(TypeEntry {
        kind: TypeKind::Primitive,
        name_idx: None,
        data: TypeData::Primitive {
            prim_id: 25,
            max_length: 8192,
        },
    });
    types.offsets = compute_type_offsets_for_entries(&types.entries).unwrap();
    let SectionData::ConstPool(constants) = module.section_mut(SectionId::ConstPool).unwrap()
    else {
        unreachable!()
    };
    let constant = constants.entries.len() as u32;
    constants.entries.push(ConstEntry {
        type_id: ty,
        payload: "A"
            .repeat(8192)
            .encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect(),
    });
    let SectionData::StringTable(strings) = module.section_mut(SectionId::StringTable).unwrap()
    else {
        unreachable!()
    };
    let name = strings.entries.len() as u32;
    strings.entries.push("payload".into());
    let SectionData::RefTable(refs) = module.section_mut(SectionId::RefTable).unwrap() else {
        unreachable!()
    };
    let reference = refs.entries.len() as u32;
    refs.entries.push(RefEntry {
        location: RefLocation::Global,
        owner_id: 0,
        offset: 0,
        segments: vec![],
    });
    let SectionData::VarMeta(meta) = module.section_mut(SectionId::VarMeta).unwrap() else {
        unreachable!()
    };
    meta.entries[0].type_id = ty;
    meta.entries.push(VarMetaEntry {
        name_idx: name,
        type_id: ty,
        ref_idx: reference,
        retain: 0,
        init_const_idx: Some(constant),
    });
    let SectionData::StorageLayout(layout) = module.section_mut(SectionId::StorageLayout).unwrap()
    else {
        unreachable!()
    };
    layout.entries.push(StorageDeclaration {
        owner: StorageOwner::Global,
        role: StorageRole::Variable,
        retain: 0,
        flags: 0,
        owner_pou_id: None,
        name_idx: name,
        type_id: Some(ty),
        slot: 0,
        ref_idx: Some(reference),
        default_const_idx: Some(constant),
        construction_nodes: 1,
        related_declaration_idx: None,
        source_name_idx: None,
    });
    let SectionData::ConstructionRoots(roots) =
        module.section_mut(SectionId::ConstructionRoots).unwrap()
    else {
        unreachable!()
    };
    roots.entries.push(ConstructionRoot {
        declaration_idx: 0,
        binding_ref_idx: Some(reference),
        instance_owner_id: None,
        parent_root_idx: None,
        template_pou_id: None,
        flags: 0,
    });
    let SectionData::Initializers(initializers) =
        module.section_mut(SectionId::Initializers).unwrap()
    else {
        unreachable!()
    };
    initializers.entries[0].declaration_idx = Some(0);
    initializers.entries[0].phase = InitializationPhase::Resource;
    module
}

#[test]
fn admitted_owned_constant_default_cannot_bypass_runtime_construction_bytes() {
    let module = owned_default();
    module
        .validate()
        .expect("empty default body and owned constant are valid STBC 2.0");
    let bytes = module.encode().unwrap();
    let mut limits = PreparationLimits {
        max_call_depth: 4,
        max_construction_bytes: 4096,
        ..PreparationLimits::default()
    };
    let prepared = PreparedModule::from_bytes(&bytes, limits)
        .expect("preparation has its own sufficient allowance");
    let error = match prepared.instantiate(0) {
        Err(error) => error,
        Ok(_) => panic!("owned default escaped its construction allowance"),
    };
    assert_eq!(
        error.stable_code(),
        StableErrorCode::RuntimeExecutionTimeout
    );
    limits.max_construction_bytes = 32768;
    let prepared = PreparedModule::from_bytes(&bytes, limits).unwrap();
    let state = prepared
        .instantiate(0)
        .expect("sufficient runtime allowance admits the same default");
    assert_eq!(
        state.storage().get_global("payload"),
        Some(&Value::WString("A".repeat(8192)))
    );
}

#[cfg(feature = "std")]
#[test]
fn rejected_owned_default_replacement_preserves_the_installed_controller() {
    use std::sync::Arc;
    use trust_runtime_core::vm::LogicalExecutionServices;
    let bytes = owned_default().encode().unwrap();
    let limits = PreparationLimits {
        max_call_depth: 4,
        max_construction_bytes: 32768,
        ..PreparationLimits::default()
    };
    let prepared = Arc::new(PreparedModule::from_bytes(&bytes, limits).unwrap());
    let mut installed = prepared.instantiate_owned(0).unwrap();
    installed
        .write_global("payload", Value::WString("kept".into()))
        .unwrap();
    installed
        .execute_cycle(trust_runtime_core::value::Duration::from_millis(10))
        .unwrap();
    let candidate = Arc::new(
        PreparedModule::from_bytes(
            &bytes,
            PreparationLimits {
                max_construction_bytes: 4096,
                ..limits
            },
        )
        .unwrap(),
    );
    let error = match installed.prepare_replacement(candidate, 0, LogicalExecutionServices, None) {
        Err(error) => error,
        Ok(_) => panic!("replacement escaped its owned-default allowance"),
    };
    assert_eq!(
        error.stable_code(),
        StableErrorCode::RuntimeExecutionTimeout
    );
    assert_eq!(
        installed.storage().get_global("payload"),
        Some(&Value::WString("kept".into()))
    );
    assert_eq!(
        installed.current_time(),
        trust_runtime_core::value::Duration::from_millis(10)
    );
    assert_eq!(installed.fault(), None);
}
