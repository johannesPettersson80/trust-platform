//! Source-free production must not evaluate application initializers.

use trust_runtime::bytecode::{
    BytecodeVersion, InitializationPhase, SectionData, SectionId, StorageRole,
};
use trust_runtime::harness::CompileSession;

#[test]
fn source_free_authoring_preserves_a_faulting_initializer_without_running_it() {
    let source =
        "VAR_GLOBAL value : LREAL := LREAL#7.0 / LREAL#0.0; END_VAR PROGRAM Main END_PROGRAM";
    let session = CompileSession::from_source(source);
    assert!(
        session.build_runtime().is_err(),
        "legacy startup evaluates division by zero"
    );
    let module = session
        .build_bytecode_module_for_version(BytecodeVersion::SOURCE_FREE)
        .expect("source-free authoring retains executable startup code");
    assert_eq!(module.version, BytecodeVersion::SOURCE_FREE);
    let bytes = module.encode().unwrap();
    let portable = trust_runtime_core::bytecode::BytecodeModule::decode(&bytes).unwrap();
    portable.validated_source_free(Default::default()).unwrap();
    let Some(SectionData::Initializers(initializers)) = module.section(SectionId::Initializers)
    else {
        panic!("initializer index missing")
    };
    assert!(initializers
        .entries
        .iter()
        .any(|entry| entry.phase == InitializationPhase::Resource && entry.code_length != 0));
}

#[test]
fn explicit_version_selection_keeps_the_legacy_default() {
    let session = CompileSession::from_source(
        "PROGRAM Main VAR counter : INT; END_VAR counter := counter + 1; END_PROGRAM",
    );
    assert_eq!(
        session.build_bytecode_module().unwrap().version,
        BytecodeVersion::LEGACY
    );
    assert!(session
        .build_bytecode_module_for_version(BytecodeVersion::new(2, 1))
        .is_err());
    let module = session
        .build_bytecode_module_for_version(BytecodeVersion::SOURCE_FREE)
        .unwrap();
    let Some(SectionData::StorageLayout(layout)) = module.section(SectionId::StorageLayout) else {
        panic!("storage layout missing")
    };
    assert!(layout
        .entries
        .iter()
        .any(|entry| entry.role == StorageRole::ProgramRoot));
}

#[test]
fn array_and_member_defaults_are_compiled_as_value_recipes() {
    let source = "TYPE Pair : STRUCT first : INT := 3; second : INT := 4; END_STRUCT; END_TYPE PROGRAM Main VAR pair : Pair := (second := 9); items : ARRAY[1..3] OF INT := [2(7), 8]; END_VAR END_PROGRAM";
    let module = CompileSession::from_source(source)
        .build_bytecode_module_for_version(BytecodeVersion::SOURCE_FREE)
        .unwrap();
    let Some(SectionData::Initializers(initializers)) = module.section(SectionId::Initializers)
    else {
        panic!("initializer index missing")
    };
    assert!(initializers.entries.iter().any(
        |entry| entry.body_kind == trust_runtime::bytecode::InitializerBodyKind::MemberDefault
    ));
    let bytes = module.encode().unwrap();
    trust_runtime_core::bytecode::BytecodeModule::decode(&bytes)
        .unwrap()
        .validated_source_free(Default::default())
        .unwrap();
}

#[test]
fn explicit_local_initialization_does_not_execute_a_separate_type_default() {
    let source = "TYPE WithDefault : INT := 7; END_TYPE FUNCTION Next : INT VAR local : WithDefault := 9; END_VAR Next := local; END_FUNCTION PROGRAM Main END_PROGRAM";
    let module = CompileSession::from_source(source)
        .build_bytecode_module_for_version(BytecodeVersion::SOURCE_FREE)
        .unwrap();
    let Some(SectionData::StringTable(strings)) = module.section(SectionId::StringTable) else {
        unreachable!()
    };
    let Some(SectionData::StorageLayout(layout)) = module.section(SectionId::StorageLayout) else {
        unreachable!()
    };
    let declaration = layout
        .entries
        .iter()
        .position(|entry| strings.entries[entry.name_idx as usize].eq_ignore_ascii_case("local"))
        .unwrap() as u32;
    let Some(SectionData::Initializers(index)) = module.section(SectionId::Initializers) else {
        unreachable!()
    };
    let actions: Vec<_> = index
        .entries
        .iter()
        .filter(|entry| entry.declaration_idx == Some(declaration))
        .collect();
    assert_eq!(actions.len(), 2);
    assert_eq!(
        actions[0].stage,
        trust_runtime::bytecode::InitializationStage::Default
    );
    assert_eq!(
        actions[0].code_length, 0,
        "explicit local initialization bypasses an independent declared default"
    );
    assert!(actions[1].code_length > 0);
}

#[test]
fn function_statics_keep_distinct_initial_and_restart_contexts() {
    let source = "FUNCTION Next : INT VAR_INPUT input : INT; END_VAR VAR_STAT count : INT := 2; END_VAR Next := count + input; END_FUNCTION PROGRAM Main END_PROGRAM";
    let module = CompileSession::from_source(source)
        .build_bytecode_module_for_version(BytecodeVersion::SOURCE_FREE)
        .unwrap();
    let Some(SectionData::StorageLayout(layout)) = module.section(SectionId::StorageLayout) else {
        unreachable!()
    };
    let declaration = layout
        .entries
        .iter()
        .position(|entry| {
            entry.role == StorageRole::Static
                && entry.owner == trust_runtime::bytecode::StorageOwner::Global
        })
        .unwrap() as u32;
    let Some(SectionData::Initializers(index)) = module.section(SectionId::Initializers) else {
        unreachable!()
    };
    let actions: Vec<_> = index
        .entries
        .iter()
        .filter(|entry| entry.declaration_idx == Some(declaration))
        .collect();
    assert_eq!(actions.len(), 4);
    for entry in actions
        .iter()
        .filter(|entry| entry.trigger == trust_runtime::bytecode::InitializationTrigger::Ordinary)
    {
        assert_eq!(entry.visible_local_count, 0);
        assert_eq!(entry.visible_static_count, 0);
    }
    let restart = actions
        .iter()
        .find(|entry| {
            entry.trigger == trust_runtime::bytecode::InitializationTrigger::AfterRestart
                && entry.stage == trust_runtime::bytecode::InitializationStage::Default
        })
        .unwrap();
    assert!(
        restart.visible_local_count >= 2,
        "return slot and input are visible in the invocation context"
    );
    assert_eq!(
        restart.code_length, 0,
        "explicit restart initializer skips a separate default evaluation"
    );
}

#[test]
fn source_free_single_tasks_require_a_bool_global() {
    for globals in ["", "VAR_GLOBAL Trigger : INT; END_VAR"] {
        let source = format!("PROGRAM Main END_PROGRAM CONFIGURATION C {globals} TASK EventTask (SINGLE := Trigger, PRIORITY := 0); PROGRAM P WITH EventTask : Main; END_CONFIGURATION");
        assert!(CompileSession::from_source(source)
            .build_bytecode_module_for_version(BytecodeVersion::SOURCE_FREE)
            .is_err());
    }
    let source = "PROGRAM Main END_PROGRAM CONFIGURATION C VAR_GLOBAL Trigger : BOOL; END_VAR TASK EventTask (SINGLE := Trigger, PRIORITY := 0); PROGRAM P WITH EventTask : Main; END_CONFIGURATION";
    CompileSession::from_source(source)
        .build_bytecode_module_for_version(BytecodeVersion::SOURCE_FREE)
        .unwrap();
}

#[test]
fn source_free_globals_preserve_constant_qualifiers() {
    let source = "VAR_GLOBAL CONSTANT Limit : INT := 12; END_VAR PROGRAM Main END_PROGRAM";
    let module = CompileSession::from_source(source)
        .build_bytecode_module_for_version(BytecodeVersion::SOURCE_FREE)
        .unwrap();
    let Some(SectionData::StringTable(strings)) = module.section(SectionId::StringTable) else {
        unreachable!()
    };
    let Some(SectionData::StorageLayout(layout)) = module.section(SectionId::StorageLayout) else {
        unreachable!()
    };
    let limit = layout
        .entries
        .iter()
        .find(|entry| strings.entries[entry.name_idx as usize].eq_ignore_ascii_case("Limit"))
        .unwrap();
    assert_eq!(limit.flags & 1, 1);
}

#[test]
fn saved_two_point_zero_fixture_matches_source_and_portable_reader() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/portability/stbc-2.0");
    let source = std::fs::read_to_string(root.join("main.st")).unwrap();
    let source_file = trust_runtime::harness::SourceFile::with_path("portability/main.st", source);
    let module = CompileSession::from_sources(vec![source_file])
        .build_bytecode_module_for_version(BytecodeVersion::SOURCE_FREE)
        .unwrap();
    let saved = std::fs::read(root.join("program-v2.stbc"))
        .expect("generate the fixture before the consolidated test batch");
    assert_eq!(module.encode().unwrap(), saved);
    let portable = trust_runtime_core::bytecode::BytecodeModule::decode(&saved).unwrap();
    portable.validated_source_free(Default::default()).unwrap();
    let Some(SectionData::ResourceMeta(meta)) = portable.section(SectionId::ResourceMeta) else {
        unreachable!()
    };
    assert_eq!(meta.resources[0].tasks[0].interval_nanos, 25_000_000);
}

#[test]
fn source_free_io_bindings_cannot_exceed_the_advertised_image() {
    let source = "VAR_GLOBAL InputLong AT %IL5 : LINT; Text AT %QB20 : STRING[8]; END_VAR PROGRAM Main END_PROGRAM";
    let module = CompileSession::from_source(source)
        .build_bytecode_module_for_version(BytecodeVersion::SOURCE_FREE)
        .unwrap();
    let Some(SectionData::ResourceMeta(meta)) = module.section(SectionId::ResourceMeta) else {
        unreachable!()
    };
    assert_eq!(meta.resources[0].inputs_size, 13);
    assert_eq!(meta.resources[0].outputs_size, 28);
    for area in 0..2 {
        let mut bad = module.clone();
        let Some(SectionData::ResourceMeta(meta)) = bad.section_mut(SectionId::ResourceMeta) else {
            unreachable!()
        };
        if area == 0 {
            meta.resources[0].inputs_size = 12;
        } else {
            meta.resources[0].outputs_size = 27;
        }
        assert!(bad.validate().is_err());
    }
}

#[test]
fn source_free_frame_types_and_dense_slots_are_admission_invariants() {
    let source = "VAR_GLOBAL root : INT; boolean_type : BOOL; END_VAR FUNCTION Read : INT VAR local : INT; END_VAR Read := local; END_FUNCTION PROGRAM Main END_PROGRAM";
    let module = CompileSession::from_source(source)
        .build_bytecode_module_for_version(BytecodeVersion::SOURCE_FREE)
        .unwrap();
    let Some(SectionData::StorageLayout(layout)) = module.section(SectionId::StorageLayout) else {
        unreachable!()
    };
    let frame_ref = layout
        .entries
        .iter()
        .find(|entry| {
            entry.owner == trust_runtime::bytecode::StorageOwner::Frame
                && entry.role == StorageRole::Variable
        })
        .unwrap()
        .ref_idx
        .unwrap();
    let Some(SectionData::TypeTable(types)) = module.section(SectionId::TypeTable) else {
        unreachable!()
    };
    let boolean = types
        .entries
        .iter()
        .position(|ty| {
            matches!(
                ty.data,
                trust_runtime::bytecode::TypeData::Primitive { prim_id: 1, .. }
            )
        })
        .unwrap() as u32;
    let mut forged_type = module.clone();
    let Some(SectionData::VarMeta(meta)) = forged_type.section_mut(SectionId::VarMeta) else {
        unreachable!()
    };
    meta.entries
        .iter_mut()
        .find(|entry| entry.ref_idx == frame_ref)
        .unwrap()
        .type_id = boolean;
    assert!(
        forged_type.validate().is_err(),
        "frame signature and reference types must agree"
    );

    let mut sparse = module;
    let Some(SectionData::StorageLayout(layout)) = sparse.section_mut(SectionId::StorageLayout)
    else {
        unreachable!()
    };
    let global = layout
        .entries
        .iter_mut()
        .find(|entry| {
            entry.owner == trust_runtime::bytecode::StorageOwner::Global
                && entry.role == StorageRole::Variable
        })
        .unwrap();
    global.slot = u32::MAX;
    let reference = global.ref_idx.unwrap();
    let Some(SectionData::RefTable(refs)) = sparse.section_mut(SectionId::RefTable) else {
        unreachable!()
    };
    refs.entries[reference as usize].offset = u32::MAX;
    assert!(
        sparse.validate().is_err(),
        "one value cannot claim a sparse four-billion-slot extent"
    );
}

#[test]
fn source_free_fb_signature_lookup_uses_frame_shadowing_without_a_concrete_instance() {
    let source = r#"
FUNCTION_BLOCK GlobalDevice
VAR_INPUT value : INT; END_VAR
END_FUNCTION_BLOCK
FUNCTION_BLOCK LocalDevice
VAR_INPUT value : INT; END_VAR
END_FUNCTION_BLOCK
VAR_GLOBAL device : GlobalDevice; END_VAR
FUNCTION Invoke : INT
VAR device : LocalDevice; END_VAR
device(value := INT#1);
Invoke := INT#0;
END_FUNCTION
PROGRAM Main END_PROGRAM
"#;
    let mut module = CompileSession::from_source(source)
        .build_bytecode_module_for_version(BytecodeVersion::SOURCE_FREE)
        .unwrap();
    let Some(SectionData::StringTable(strings)) = module.section(SectionId::StringTable) else {
        unreachable!()
    };
    let names = strings.entries.clone();
    let Some(SectionData::PouIndex(index)) = module.section_mut(SectionId::PouIndex) else {
        unreachable!()
    };
    let local = index
        .entries
        .iter_mut()
        .find(|entry| names[entry.name_idx as usize].eq_ignore_ascii_case("LocalDevice"))
        .unwrap();
    let local_id = local.id;
    local
        .params
        .iter_mut()
        .find(|param| names[param.name_idx as usize].eq_ignore_ascii_case("value"))
        .unwrap()
        .direction = 2;
    let Some(SectionData::StorageLayout(layout)) = module.section_mut(SectionId::StorageLayout)
    else {
        unreachable!()
    };
    layout
        .entries
        .iter_mut()
        .find(|entry| {
            entry.owner_pou_id == Some(local_id)
                && names[entry.name_idx as usize].eq_ignore_ascii_case("value")
        })
        .unwrap()
        .flags = 8;
    assert!(module.validate().is_err(), "the local receiver now requires a writable argument; the same-named global cannot supply its signature");
}

#[test]
fn source_free_signatures_cover_rootless_and_inherited_fb_templates() {
    let source = r#"
FUNCTION_BLOCK Cell
VAR_INPUT value : INT; END_VAR
END_FUNCTION_BLOCK
FUNCTION_BLOCK Parent
VAR device : Cell; END_VAR
END_FUNCTION_BLOCK
FUNCTION_BLOCK Child EXTENDS Parent
device(value := INT#1);
END_FUNCTION_BLOCK
FUNCTION_BLOCK UnusedTemplate
VAR device : Cell; END_VAR
device(value := INT#2);
END_FUNCTION_BLOCK
PROGRAM Main
VAR first : Child; second : Child; END_VAR
first();
second();
END_PROGRAM
"#;
    let module = CompileSession::from_source(source)
        .build_bytecode_module_for_version(BytecodeVersion::SOURCE_FREE)
        .unwrap();
    let bytes = module.encode().unwrap();
    trust_runtime_core::bytecode::BytecodeModule::decode(&bytes)
        .unwrap()
        .validated_source_free(Default::default())
        .unwrap();
}

#[test]
fn source_free_disabled_result_recipe_is_not_evaluated_during_authoring() {
    let source = r#"
TYPE BrokenDefault : LREAL := LREAL#7.0 / LREAL#0.0; END_TYPE
FUNCTION Read : BrokenDefault
VAR_INPUT EN : BOOL := TRUE; END_VAR
Read := LREAL#1.0;
END_FUNCTION
PROGRAM Main
VAR result : LREAL; END_VAR
result := Read(EN := FALSE);
END_PROGRAM
"#;
    let module = CompileSession::from_source(source)
        .build_bytecode_module_for_version(BytecodeVersion::SOURCE_FREE)
        .unwrap();
    let Some(SectionData::Initializers(index)) = module.section(SectionId::Initializers) else {
        unreachable!()
    };
    assert!(index
        .entries
        .iter()
        .any(|entry| entry.phase == InitializationPhase::ValueDefault
            && entry.body_kind == trust_runtime::bytecode::InitializerBodyKind::Action));
}

#[test]
fn static_lexical_names_must_be_unique_even_when_backing_names_differ() {
    let source = "FUNCTION Next : INT VAR_STAT first : INT; second : INT; END_VAR Next := first; END_FUNCTION PROGRAM Main END_PROGRAM";
    let mut module = CompileSession::from_source(source)
        .build_bytecode_module_for_version(BytecodeVersion::SOURCE_FREE)
        .unwrap();
    let Some(SectionData::StorageLayout(layout)) = module.section_mut(SectionId::StorageLayout)
    else {
        unreachable!()
    };
    let statics: Vec<_> = layout
        .entries
        .iter()
        .enumerate()
        .filter(|(_, entry)| {
            entry.role == StorageRole::Static
                && entry.owner == trust_runtime::bytecode::StorageOwner::Global
        })
        .map(|(id, _)| id)
        .collect();
    let [first, second] = statics.as_slice() else {
        panic!("expected two function statics")
    };
    let lexical_name = layout.entries[*first].source_name_idx;
    layout.entries[*second].source_name_idx = lexical_name;
    assert!(module.validate().is_err());
}

#[test]
fn access_aliases_cannot_collide_with_resource_globals_in_saved_artifacts() {
    let source = r#"
VAR_GLOBAL other : INT; END_VAR
PROGRAM Main VAR value : INT; END_VAR END_PROGRAM
CONFIGURATION C
PROGRAM P : Main;
VAR_ACCESS exposed : P.value : INT READ_WRITE; END_VAR
END_CONFIGURATION
"#;
    let mut module = CompileSession::from_source(source)
        .build_bytecode_module_for_version(BytecodeVersion::SOURCE_FREE)
        .unwrap();
    let Some(SectionData::StringTable(strings)) = module.section(SectionId::StringTable) else {
        unreachable!()
    };
    let other = strings
        .entries
        .iter()
        .position(|name| name.eq_ignore_ascii_case("other"))
        .unwrap() as u32;
    let Some(SectionData::AccessBindings(aliases)) = module.section_mut(SectionId::AccessBindings)
    else {
        unreachable!()
    };
    aliases.entries[0].name_idx = other;
    assert!(module.validate().is_err());
}

#[test]
fn source_free_aliases_can_read_multiple_concrete_program_roots() {
    let source = r#"
PROGRAM First VAR value : INT := INT#1; END_VAR END_PROGRAM
PROGRAM Second VAR value : INT := INT#2; END_VAR END_PROGRAM
PROGRAM Reader VAR sum : INT; END_VAR sum := LeftValue + RightValue; END_PROGRAM
CONFIGURATION C
PROGRAM Left : First;
PROGRAM Right : Second;
PROGRAM ReadBoth : Reader;
VAR_ACCESS
    LeftValue : Left.value : INT READ_ONLY;
    RightValue : Right.value : INT READ_ONLY;
END_VAR
END_CONFIGURATION
"#;
    let module = CompileSession::from_source(source)
        .build_bytecode_module_for_version(BytecodeVersion::SOURCE_FREE)
        .unwrap();
    module.validate().unwrap();
}

#[test]
fn static_visibility_is_derived_from_the_owner_and_declaration_prefix() {
    let source = "FUNCTION Next : INT VAR_STAT first : INT := 1; second : INT := 2; END_VAR VAR local : INT; END_VAR Next := local + first + second; END_FUNCTION PROGRAM Main END_PROGRAM";
    let module = CompileSession::from_source(source)
        .build_bytecode_module_for_version(BytecodeVersion::SOURCE_FREE)
        .unwrap();
    let Some(SectionData::Initializers(index)) = module.section(SectionId::Initializers) else {
        unreachable!()
    };
    let selected: Vec<_> = index
        .entries
        .iter()
        .enumerate()
        .filter(|(_, entry)| {
            entry.phase == InitializationPhase::Frame
                || entry.trigger == trust_runtime::bytecode::InitializationTrigger::AfterRestart
        })
        .map(|(id, _)| id)
        .collect();
    assert!(!selected.is_empty());
    for id in selected {
        let mut bad = module.clone();
        let Some(SectionData::Initializers(index)) = bad.section_mut(SectionId::Initializers)
        else {
            unreachable!()
        };
        index.entries[id].visible_static_count = u32::MAX;
        assert!(
            bad.validate().is_err(),
            "an artifact cannot grant itself visibility of later static aliases"
        );
    }
}

#[test]
fn hosted_runtime_rejects_two_point_zero_without_replacing_legacy_execution() {
    use trust_runtime::error::StableErrorCode;
    use trust_runtime::value::{Duration, Value};

    let replacement = CompileSession::from_source("PROGRAM Replacement END_PROGRAM")
        .build_bytecode_module_for_version(BytecodeVersion::SOURCE_FREE)
        .unwrap();
    replacement.validate().unwrap();
    let bytes = replacement.encode().unwrap();
    for use_bytes in [false, true] {
        let session = CompileSession::from_source(
            "PROGRAM Main VAR count : INT; END_VAR count := count + 1; END_PROGRAM",
        );
        let mut runtime = session.build_runtime().unwrap();
        let legacy = session.build_bytecode_module().unwrap();
        runtime.apply_bytecode_module(&legacy, None).unwrap();
        runtime.set_current_time(Duration::from_millis(10));
        runtime.execute_cycle().unwrap();
        let Some(Value::Instance(instance)) = runtime.storage().get_global("Main") else {
            panic!("missing original program instance");
        };
        let instance = *instance;
        assert_eq!(
            runtime.storage().get_instance_var(instance, "count"),
            Some(&Value::Int(1))
        );
        let error = if use_bytes {
            runtime.apply_bytecode_bytes(&bytes, None).unwrap_err()
        } else {
            runtime
                .apply_bytecode_module(&replacement, None)
                .unwrap_err()
        };
        assert_eq!(error.stable_code(), StableErrorCode::VmBytecodeDecode);
        assert!(runtime.storage().get_global("Replacement").is_none());
        assert_eq!(
            runtime.storage().get_instance_var(instance, "count"),
            Some(&Value::Int(1))
        );
        runtime.set_current_time(Duration::from_millis(20));
        runtime.execute_cycle().unwrap();
        assert_eq!(
            runtime.storage().get_instance_var(instance, "count"),
            Some(&Value::Int(2))
        );
    }
}

#[test]
fn source_free_authoring_preserves_iec_constant_initializer_diagnostics() {
    let session = CompileSession::from_source(
        "VAR_GLOBAL input : INT; initial : INT := input; END_VAR PROGRAM Main END_PROGRAM",
    );
    for version in [BytecodeVersion::LEGACY, BytecodeVersion::SOURCE_FREE] {
        let error = session
            .build_bytecode_module_for_version(version)
            .unwrap_err()
            .to_string();
        assert!(error.contains("error[E202]"), "{error}");
        assert!(error.contains("mutable dependency"), "{error}");
    }
}

#[test]
fn nested_and_aliased_string_io_retains_each_leaf_capacity() {
    let source = "TYPE Label : STRING[3]; Packet : STRUCT first : Label; rest : ARRAY[1..2] OF Label; END_STRUCT; END_TYPE VAR_GLOBAL message AT %QB10 : Packet; END_VAR PROGRAM Main END_PROGRAM";
    let module = CompileSession::from_source(source)
        .build_bytecode_module_for_version(BytecodeVersion::SOURCE_FREE)
        .unwrap();
    let Some(SectionData::IoMap(io)) = module.section(SectionId::IoMap) else {
        unreachable!()
    };
    let Some(SectionData::TypeTable(types)) = module.section(SectionId::TypeTable) else {
        unreachable!()
    };
    assert_eq!(io.bindings.len(), 3);
    for binding in &io.bindings {
        assert!(matches!(
            types.entries[binding.type_id.unwrap() as usize].data,
            trust_runtime::bytecode::TypeData::Primitive {
                prim_id: 24,
                max_length: 3
            }
        ));
    }
    let Some(SectionData::ResourceMeta(meta)) = module.section(SectionId::ResourceMeta) else {
        unreachable!()
    };
    assert_eq!(meta.resources[0].outputs_size, 19);
    let portable =
        trust_runtime_core::bytecode::BytecodeModule::decode(&module.encode().unwrap()).unwrap();
    portable.validated_source_free(Default::default()).unwrap();
}

fn author_and_reload(source: &str) -> trust_runtime_core::bytecode::BytecodeModule {
    let module = CompileSession::from_source(source)
        .build_bytecode_module_for_version(BytecodeVersion::SOURCE_FREE)
        .expect("source-free authoring");
    let portable =
        trust_runtime_core::bytecode::BytecodeModule::decode(&module.encode().unwrap()).unwrap();
    portable.validated_source_free(Default::default()).unwrap();
    portable
}

#[test]
fn source_free_edges_have_distinct_previous_sample_slots() {
    let module = author_and_reload(
        "PROGRAM Main VAR_INPUT rising : BOOL R_EDGE; falling : BOOL F_EDGE; END_VAR END_PROGRAM",
    );
    let Some(SectionData::StorageLayout(layout)) = module.section(SectionId::StorageLayout) else {
        unreachable!()
    };
    let edges: Vec<_> = layout
        .entries
        .iter()
        .filter(|entry| entry.role == StorageRole::EdgePhase)
        .collect();
    assert_eq!(edges.len(), 2);
    assert_ne!(edges[0].slot, edges[1].slot);
    assert_ne!(
        edges[0].related_declaration_idx,
        edges[1].related_declaration_idx
    );
    assert!(edges
        .iter()
        .all(|entry| entry.related_declaration_idx.is_some()));
}

#[test]
fn source_free_retained_state_survives_metadata_round_trip() {
    let module =
        author_and_reload("PROGRAM Main VAR RETAIN saved : DINT := DINT#7; END_VAR END_PROGRAM");
    let Some(SectionData::StorageLayout(layout)) = module.section(SectionId::StorageLayout) else {
        unreachable!()
    };
    let Some(SectionData::StringTable(strings)) = module.section(SectionId::StringTable) else {
        unreachable!()
    };
    let saved = layout
        .entries
        .iter()
        .find(|entry| strings.entries[entry.name_idx as usize].eq_ignore_ascii_case("saved"))
        .unwrap();
    assert_ne!(saved.retain, 0);
    assert_eq!(saved.role, StorageRole::Variable);
}

#[test]
fn source_free_partial_alias_and_configuration_preserve_selection() {
    let module = author_and_reload(
        r#"
PROGRAM Main VAR bits : WORD; END_VAR END_PROGRAM
CONFIGURATION Conf PROGRAM P1 : Main;
VAR_ACCESS selected : P1.bits.%X3 : BOOL READ_WRITE; END_VAR
VAR_CONFIG P1.bits : WORD := WORD#7; END_VAR
END_CONFIGURATION
"#,
    );
    let Some(SectionData::AccessBindings(aliases)) = module.section(SectionId::AccessBindings)
    else {
        unreachable!()
    };
    assert_eq!(
        (
            aliases.entries[0].partial_kind,
            aliases.entries[0].partial_index
        ),
        (1, 3)
    );
    let Some(SectionData::Initializers(initializers)) = module.section(SectionId::Initializers)
    else {
        unreachable!()
    };
    let action = initializers
        .entries
        .iter()
        .find(|entry| entry.phase == InitializationPhase::Configuration)
        .unwrap();
    assert_eq!((action.partial_kind, action.partial_index), (0, 0));
}

#[test]
fn source_free_configuration_resolves_source_at_address() {
    let module = author_and_reload(
        r#"
PROGRAM Main VAR output AT %Q* : BOOL; END_VAR END_PROGRAM
CONFIGURATION Conf PROGRAM P1 : Main;
VAR_CONFIG P1.output AT %QX2.3 : BOOL := TRUE; END_VAR
END_CONFIGURATION
"#,
    );
    let Some(SectionData::Initializers(initializers)) = module.section(SectionId::Initializers)
    else {
        unreachable!()
    };
    assert!(initializers
        .entries
        .iter()
        .any(|entry| entry.phase == InitializationPhase::Configuration));
    let Some(SectionData::ResourceMeta(meta)) = module.section(SectionId::ResourceMeta) else {
        unreachable!()
    };
    assert!(meta.resources[0].outputs_size >= 3);
}

#[test]
fn source_free_class_templates_preserve_inheritance_and_methods() {
    let module = author_and_reload(
        r#"
CLASS BaseDevice VAR PUBLIC value : INT := INT#5; END_VAR
METHOD PUBLIC ReadValue : INT ReadValue := value; END_METHOD END_CLASS
FUNCTION_BLOCK Device EXTENDS BaseDevice END_FUNCTION_BLOCK
PROGRAM Main VAR device : Device; result : INT; END_VAR result := device.ReadValue(); END_PROGRAM
"#,
    );
    let Some(SectionData::PouIndex(index)) = module.section(SectionId::PouIndex) else {
        unreachable!()
    };
    let class = index
        .entries
        .iter()
        .find(|entry| entry.kind == trust_runtime::bytecode::PouKind::Class)
        .unwrap();
    assert!(index.entries.iter().any(|entry| entry
        .class_meta
        .as_ref()
        .is_some_and(|meta| meta.parent_pou_id == Some(class.id))));
}

#[test]
fn source_free_qualifiers_are_recognized_without_skipping_misspellings() {
    for prefix in ["", "Conf.", "Conf.Controller.", "Controller."] {
        author_and_reload(&format!("PROGRAM Main VAR value : INT; END_VAR END_PROGRAM CONFIGURATION Conf RESOURCE Controller ON PLC PROGRAM P1 : Main; VAR_ACCESS observed : {prefix}P1.value : INT READ_ONLY; END_VAR END_RESOURCE END_CONFIGURATION"));
    }
    let source = "VAR_GLOBAL value : INT; END_VAR PROGRAM Main END_PROGRAM CONFIGURATION Conf PROGRAM P1 : Main; VAR_ACCESS observed : Typo.value : INT READ_ONLY; END_VAR END_CONFIGURATION";
    assert!(CompileSession::from_source(source)
        .build_bytecode_module_for_version(BytecodeVersion::SOURCE_FREE)
        .is_err());
}

#[test]
fn source_free_templates_exclude_unused_standard_blocks() {
    let module = author_and_reload(
        "PROGRAM Main VAR delay : TON; END_VAR delay(IN := TRUE, PT := T#1ms); END_PROGRAM",
    );
    let Some(SectionData::PouIndex(index)) = module.section(SectionId::PouIndex) else {
        unreachable!()
    };
    let Some(SectionData::StringTable(strings)) = module.section(SectionId::StringTable) else {
        unreachable!()
    };
    let blocks: Vec<_> = index
        .entries
        .iter()
        .filter(|entry| entry.kind == trust_runtime::bytecode::PouKind::FunctionBlock)
        .map(|entry| strings.entries[entry.name_idx as usize].as_str())
        .collect();
    assert_eq!(blocks, ["TON"]);
}

#[test]
fn source_free_equivalent_actions_share_recipes_without_sharing_results() {
    use trust_runtime::bytecode::InitializerBodyKind;
    let module = author_and_reload("TYPE WithDefault : INT := INT#7; END_TYPE VAR_GLOBAL first : WithDefault; second : WithDefault; END_VAR PROGRAM Main END_PROGRAM");
    let Some(SectionData::Initializers(index)) = module.section(SectionId::Initializers) else {
        unreachable!()
    };
    let actions: Vec<_> = index
        .entries
        .iter()
        .filter(|entry| entry.phase == InitializationPhase::Resource)
        .collect();
    assert_eq!(actions.len(), 2);
    assert!(actions[0].context_initializer_idx.is_none());
    assert!(actions[1].context_initializer_idx.is_some());
    assert_ne!(actions[0].result_ref_idx, actions[1].result_ref_idx);
    assert_eq!(
        index
            .entries
            .iter()
            .filter(|entry| entry.body_kind == InitializerBodyKind::TypeDefault)
            .count(),
        1
    );
}

#[test]
fn portable_admission_rejects_a_forged_initializer_global_write() {
    use trust_runtime::bytecode::{BytecodeError, RejectionReason, StorageOwner};
    let mut module =
        author_and_reload("VAR_GLOBAL target : BOOL; END_VAR PROGRAM Main END_PROGRAM");
    let Some(SectionData::StorageLayout(layout)) = module.section(SectionId::StorageLayout) else {
        unreachable!()
    };
    let target = layout
        .entries
        .iter()
        .find(|entry| entry.owner == StorageOwner::Global && entry.role == StorageRole::Variable)
        .unwrap();
    let reference = target.ref_idx.unwrap();
    let ty = target.type_id.unwrap();
    let Some(SectionData::PouBodies(code)) = module.section_mut(SectionId::PouBodies) else {
        unreachable!()
    };
    let offset = code.len() as u32;
    code.push(0x22); // address of global, not the result staging slot
    code.extend_from_slice(&reference.to_le_bytes());
    code.push(trust_runtime::bytecode::opcodes::DEFAULT_TYPED);
    code.extend_from_slice(&ty.to_le_bytes());
    code.push(0x33);
    let Some(SectionData::Initializers(index)) = module.section_mut(SectionId::Initializers) else {
        unreachable!()
    };
    let action = index
        .entries
        .iter_mut()
        .find(|entry| entry.phase == InitializationPhase::Resource)
        .unwrap();
    action.code_offset = offset;
    action.code_length = 11;
    assert_eq!(
        module.validate(),
        Err(BytecodeError::from(
            RejectionReason::InitializerWriteOutsideStaging
        ))
    );
    // The fresh byte reader must enforce the same rule, not merely a hosted wrapper.
    let decoded =
        trust_runtime_core::bytecode::BytecodeModule::decode(&module.encode().unwrap()).unwrap();
    assert_eq!(decoded.validate(), module.validate());
}

#[test]
fn source_free_templates_include_expression_only_type_dependencies() {
    let module = author_and_reload("TYPE TimerRef : REF_TO TON; END_TYPE PROGRAM Main VAR bytes : DINT; END_VAR bytes := SIZEOF(TimerRef); END_PROGRAM");
    let Some(SectionData::PouIndex(index)) = module.section(SectionId::PouIndex) else {
        unreachable!()
    };
    let Some(SectionData::StringTable(strings)) = module.section(SectionId::StringTable) else {
        unreachable!()
    };
    assert!(index
        .entries
        .iter()
        .any(|entry| strings.entries[entry.name_idx as usize] == "TON"));
}

#[test]
fn portable_admission_rejects_invalid_canonical_action_links() {
    use trust_runtime::bytecode::{
        BytecodeError, InitializationStage, InitializationTrigger, RejectionReason,
    };
    let module = author_and_reload("TYPE WithDefault : INT := INT#7; END_TYPE VAR_GLOBAL first, second, third : WithDefault; END_VAR PROGRAM Main END_PROGRAM");
    let Some(SectionData::Initializers(index)) = module.section(SectionId::Initializers) else {
        unreachable!()
    };
    let ids: Vec<_> = index
        .entries
        .iter()
        .enumerate()
        .filter(|(_, entry)| entry.phase == InitializationPhase::Resource)
        .map(|(id, _)| id)
        .collect();
    assert_eq!(ids.len(), 3);
    for mutation in 0..7 {
        let mut bad = module.clone();
        let Some(SectionData::Initializers(index)) = bad.section_mut(SectionId::Initializers)
        else {
            unreachable!()
        };
        match mutation {
            0 => index.entries[ids[1]].context_initializer_idx = Some(ids[1] as u32),
            1 => index.entries[ids[0]].context_initializer_idx = Some(ids[2] as u32),
            2 => index.entries[ids[2]].context_initializer_idx = Some(ids[1] as u32),
            3 => index.entries[ids[1]].stage = InitializationStage::Explicit,
            4 => index.entries[ids[1]].trigger = InitializationTrigger::AfterRestart,
            5 => index.entries[ids[1]].visible_local_count = 1,
            _ => index.entries[ids[1]].visible_static_count = 1,
        }
        assert_eq!(
            bad.validate(),
            Err(BytecodeError::from(RejectionReason::InitializerVisibility)),
            "mutation {mutation}"
        );
    }
    let mut module = author_and_reload("TYPE WithDefault : INT := INT#7; END_TYPE PROGRAM Main VAR first, second : WithDefault; END_VAR END_PROGRAM");
    let Some(SectionData::Initializers(index)) = module.section_mut(SectionId::Initializers) else {
        unreachable!()
    };
    let ids: Vec<_> = index
        .entries
        .iter()
        .enumerate()
        .filter(|(_, entry)| {
            entry.phase == InitializationPhase::Instance
                && entry.stage == InitializationStage::Default
        })
        .map(|(id, _)| id)
        .collect();
    assert_eq!(ids.len(), 2);
    index.entries[ids[1]].context_initializer_idx = Some(ids[0] as u32);
    assert_eq!(
        module.validate(),
        Err(BytecodeError::from(RejectionReason::InitializerVisibility)),
        "different instance construction frontiers cannot share recipes"
    );
}
