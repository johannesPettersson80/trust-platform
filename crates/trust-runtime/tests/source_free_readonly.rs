//! Admitted wire input cannot bypass constant declaration protection.
use trust_runtime::bytecode::{BytecodeVersion, SectionData, SectionId, StorageOwner};
use trust_runtime::harness::CompileSession;
use trust_runtime_core::{
    error::RuntimeError,
    value::{Duration, Value},
    vm::{PreparationLimits, PreparedModule},
};

#[test]
fn forged_constant_global_rejects_direct_dynamic_and_native_output_stores() {
    for body in [
        "protected_value := 9;",
        "alias_ref := REF(protected_value); alias_ref^ := 9;",
        "result := Emit(output_value => protected_value);",
    ] {
        let source = format!(
            r#"
VAR_GLOBAL protected_value : INT := 7; END_VAR
FUNCTION Emit : INT
VAR_OUTPUT output_value : INT; END_VAR
output_value := 9;
Emit := 1;
END_FUNCTION
PROGRAM Main
VAR alias_ref : REF_TO INT; result : INT; END_VAR
{body}
END_PROGRAM
"#
        );
        let mut module = CompileSession::from_source(&source)
            .build_bytecode_module_for_version(BytecodeVersion::SOURCE_FREE)
            .unwrap();
        let Some(SectionData::StringTable(strings)) = module.section(SectionId::StringTable) else {
            panic!("strings")
        };
        let name = strings
            .entries
            .iter()
            .position(|name| name == "protected_value")
            .unwrap() as u32;
        let Some(SectionData::StorageLayout(layout)) = module.section_mut(SectionId::StorageLayout)
        else {
            panic!("layout")
        };
        let declaration = layout
            .entries
            .iter_mut()
            .find(|decl| decl.owner == StorageOwner::Global && decl.name_idx == name)
            .unwrap();
        declaration.flags |= 1;
        let prepared =
            PreparedModule::from_bytes(&module.encode().unwrap(), PreparationLimits::default())
                .expect("wire admission does not prove write permission");
        let mut state = prepared.instantiate(0).unwrap();
        assert_eq!(
            state.storage().get_global("protected_value"),
            Some(&Value::Int(7))
        );
        assert_eq!(
            state.execute_cycle(Duration::from_millis(10)),
            Err(RuntimeError::ConstantWrite),
            "{body}"
        );
        assert_eq!(
            state.storage().get_global("protected_value"),
            Some(&Value::Int(7)),
            "{body}"
        );
        assert_eq!(state.fault(), Some(&RuntimeError::ConstantWrite));
        assert_eq!(
            state.execute_cycle(Duration::from_millis(20)),
            Err(RuntimeError::ResourceFaulted)
        );
    }
}

#[test]
fn forged_constant_instance_member_is_initialized_but_never_overwritten() {
    let source =
        "PROGRAM Main VAR protected_value : INT := 7; END_VAR protected_value := 9; END_PROGRAM";
    let mut module = CompileSession::from_source(source)
        .build_bytecode_module_for_version(BytecodeVersion::SOURCE_FREE)
        .unwrap();
    let Some(SectionData::StringTable(strings)) = module.section(SectionId::StringTable) else {
        panic!("strings")
    };
    let name = strings
        .entries
        .iter()
        .position(|name| name == "protected_value")
        .unwrap() as u32;
    let Some(SectionData::StorageLayout(layout)) = module.section_mut(SectionId::StorageLayout)
    else {
        panic!("layout")
    };
    layout
        .entries
        .iter_mut()
        .find(|decl| decl.owner == StorageOwner::Instance && decl.name_idx == name)
        .unwrap()
        .flags |= 1;
    let prepared =
        PreparedModule::from_bytes(&module.encode().unwrap(), PreparationLimits::default())
            .unwrap();
    let mut state = prepared.instantiate(0).unwrap();
    let Some(Value::Instance(main)) = state.storage().get_global("Main") else {
        panic!("Main")
    };
    let main = *main;
    assert_eq!(
        state.execute_cycle(Duration::from_millis(10)),
        Err(RuntimeError::ConstantWrite)
    );
    assert_eq!(
        state.storage().get_instance_var(main, "protected_value"),
        Some(&Value::Int(7))
    );
    assert_eq!(state.fault(), Some(&RuntimeError::ConstantWrite));
}

#[test]
fn forged_constant_frame_local_faults_before_return_copyback() {
    let source = r#"
FUNCTION Work : INT
VAR protected_value : INT := 7; END_VAR
protected_value := 9;
Work := protected_value;
END_FUNCTION
PROGRAM Main
VAR result : INT; END_VAR
result := Work();
END_PROGRAM
"#;
    let mut module = CompileSession::from_source(source)
        .build_bytecode_module_for_version(BytecodeVersion::SOURCE_FREE)
        .unwrap();
    let Some(SectionData::StringTable(strings)) = module.section(SectionId::StringTable) else {
        panic!("strings")
    };
    let name = strings
        .entries
        .iter()
        .position(|name| name == "protected_value")
        .unwrap() as u32;
    let Some(SectionData::StorageLayout(layout)) = module.section_mut(SectionId::StorageLayout)
    else {
        panic!("layout")
    };
    layout
        .entries
        .iter_mut()
        .find(|decl| decl.owner == StorageOwner::Frame && decl.name_idx == name)
        .unwrap()
        .flags |= 1;
    let prepared =
        PreparedModule::from_bytes(&module.encode().unwrap(), PreparationLimits::default())
            .unwrap();
    let mut state = prepared.instantiate(0).unwrap();
    assert_eq!(
        state.execute_cycle(Duration::from_millis(10)),
        Err(RuntimeError::ConstantWrite)
    );
    let Some(Value::Instance(main)) = state.storage().get_global("Main") else {
        panic!("Main")
    };
    assert_eq!(
        state.storage().get_instance_var(*main, "result"),
        Some(&Value::Int(0))
    );
    assert_eq!(state.fault(), Some(&RuntimeError::ConstantWrite));
}

#[test]
fn forged_store_cannot_replace_an_untyped_program_root() {
    let source = "VAR_GLOBAL replacement_value : INT := 7; END_VAR PROGRAM Main replacement_value := 9; END_PROGRAM";
    let mut module = CompileSession::from_source(source)
        .build_bytecode_module_for_version(BytecodeVersion::SOURCE_FREE)
        .unwrap();
    let Some(SectionData::StringTable(strings)) = module.section(SectionId::StringTable) else {
        panic!("strings")
    };
    let Some(SectionData::StorageLayout(layout)) = module.section(SectionId::StorageLayout) else {
        panic!("layout")
    };
    let ordinary = layout
        .entries
        .iter()
        .find(|entry| strings.entries[entry.name_idx as usize] == "replacement_value")
        .unwrap()
        .ref_idx
        .unwrap();
    let root = layout
        .entries
        .iter()
        .find(|entry| entry.role == trust_runtime::bytecode::StorageRole::ProgramRoot)
        .unwrap()
        .ref_idx
        .unwrap();
    let mut encoded_store = vec![0x21];
    encoded_store.extend_from_slice(&ordinary.to_le_bytes());
    let Some(SectionData::PouBodies(code)) = module.section_mut(SectionId::PouBodies) else {
        panic!("code")
    };
    let stores: Vec<_> = code
        .windows(5)
        .enumerate()
        .filter(|(_, bytes)| *bytes == encoded_store.as_slice())
        .map(|(offset, _)| offset)
        .collect();
    assert_eq!(stores.len(), 1, "one ordinary assignment destination");
    code[stores[0] + 1..stores[0] + 5].copy_from_slice(&root.to_le_bytes());
    let prepared =
        PreparedModule::from_bytes(&module.encode().unwrap(), PreparationLimits::default())
            .expect("untyped root store is structurally admitted");
    let mut state = prepared.instantiate(0).unwrap();
    let original = state.storage().get_global("Main").unwrap().clone();
    assert_eq!(
        state.execute_cycle(Duration::from_millis(10)),
        Err(RuntimeError::ProgramRootReplacement)
    );
    assert_eq!(state.storage().get_global("Main"), Some(&original));
    assert_eq!(state.fault(), Some(&RuntimeError::ProgramRootReplacement));
}

#[test]
fn external_record_cannot_shadow_the_physical_constant_slot() {
    use trust_runtime::bytecode::StorageRole;
    let source = "VAR_GLOBAL protected_value : INT := 7; END_VAR PROGRAM Main protected_value := 9; END_PROGRAM";
    let mut module = CompileSession::from_source(source)
        .build_bytecode_module_for_version(BytecodeVersion::SOURCE_FREE)
        .unwrap();
    let Some(SectionData::StringTable(strings)) = module.section_mut(SectionId::StringTable) else {
        panic!("strings")
    };
    let physical_name = strings
        .entries
        .iter()
        .position(|name| name == "protected_value")
        .unwrap() as u32;
    let external_name = strings.entries.len() as u32;
    strings.entries.push("external_shadow".into());
    let Some(SectionData::StorageLayout(layout)) = module.section_mut(SectionId::StorageLayout)
    else {
        panic!("layout")
    };
    let physical = layout
        .entries
        .iter_mut()
        .find(|entry| entry.owner == StorageOwner::Global && entry.name_idx == physical_name)
        .unwrap();
    physical.flags |= 1;
    let mut external = physical.clone();
    external.role = StorageRole::External;
    external.flags = 0;
    external.name_idx = external_name;
    external.default_const_idx = None;
    external.construction_nodes = 0;
    // Keep the same slot/reference, which External may alias without owning it.
    // Prepending makes an incorrect first-owner/slot lookup select the alias.
    layout.entries.insert(0, external);
    for entry in &mut layout.entries {
        if let Some(index) = &mut entry.related_declaration_idx {
            *index += 1;
        }
    }
    let Some(SectionData::ConstructionRoots(roots)) =
        module.section_mut(SectionId::ConstructionRoots)
    else {
        panic!("roots")
    };
    for root in &mut roots.entries {
        root.declaration_idx += 1;
    }
    let Some(SectionData::Initializers(initializers)) = module.section_mut(SectionId::Initializers)
    else {
        panic!("initializers")
    };
    for initializer in &mut initializers.entries {
        if let Some(index) = &mut initializer.declaration_idx {
            *index += 1;
        }
    }
    let prepared =
        PreparedModule::from_bytes(&module.encode().unwrap(), PreparationLimits::default())
            .expect("External duplicate owner/slot is a valid non-owning binding");
    let mut state = prepared.instantiate(0).unwrap();
    assert_eq!(
        state.execute_cycle(Duration::from_millis(10)),
        Err(RuntimeError::ConstantWrite)
    );
    assert_eq!(
        state.storage().get_global("protected_value"),
        Some(&Value::Int(7))
    );
    assert_eq!(state.fault(), Some(&RuntimeError::ConstantWrite));
}
