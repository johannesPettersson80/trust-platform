//! The authoring frontend must retain startup expressions without running them.

use super::lower_application;
use crate::harness::SourceFile;
use crate::program_model::Expr;

#[test]
fn lowering_retains_faulting_runtime_initializer_without_materializing_storage() {
    let sources = [SourceFile::new(
        r#"
VAR_GLOBAL
    initial : LREAL := LREAL#8.0 / LREAL#0.0;
END_VAR
PROGRAM Main
END_PROGRAM
"#,
    )];
    let lowered = lower_application(&sources, false).expect("lowering does not run startup");
    let initial = lowered
        .globals
        .iter()
        .find(|item| item.name == "initial")
        .expect("original global declaration");
    assert!(initial.initializer.is_some());
    assert!(lowered.builder.storage().get_global("initial").is_none());
    assert!(lowered.builder.programs().is_empty());
    let error = match lowered.materialize(&[]) {
        Ok(_) => panic!("legacy startup must still reject division by zero"),
        Err(error) => error.to_string().to_ascii_lowercase(),
    };
    assert!(error.contains("zero"), "{error}");
}

#[test]
fn lowering_preserves_program_template_identity_and_ordered_instance_overrides() {
    let sources = [SourceFile::new(
        r#"
PROGRAM Main
VAR
    setting : INT;
END_VAR
END_PROGRAM
CONFIGURATION Conf
PROGRAM First : Main;
PROGRAM Second : Main;
VAR_CONFIG
    First.setting : INT := INT#11;
    Second.setting : INT := INT#22;
    First.setting : INT := INT#33;
END_VAR
END_CONFIGURATION
"#,
    )];
    let lowered = lower_application(&sources, false).expect("lower declarations");
    assert_eq!(lowered.program_defs["MAIN"].name, "Main");
    let config = lowered.configuration.as_ref().expect("configuration");
    assert_eq!(config.programs[0].name, "First");
    assert_eq!(config.programs[0].type_name, "Main");
    assert_eq!(config.programs[1].name, "Second");
    assert_eq!(config.programs[1].type_name, "Main");
    assert_eq!(config.config_inits.len(), 3);
    for (action, expected) in config.config_inits.iter().zip([11, 22, 33]) {
        let Some(Expr::Literal(value)) = &action.initializer else {
            panic!("initializer expression must survive lowering");
        };
        assert_eq!(value, &crate::value::Value::Int(expected));
    }
    assert!(lowered.builder.programs().is_empty());
}

#[test]
fn direct_startup_action_extents_are_declared_and_enforced() {
    use crate::bytecode::{SectionData, SectionId};
    let sources = [SourceFile::new("PROGRAM Main END_PROGRAM")];
    let mut input = lower_application(&sources, false)
        .unwrap()
        .prepare_authoring(&[])
        .unwrap();
    // The raw format supports direct actions; the current source grammar accepts
    // variable paths only, so construct the lowered action explicitly here.
    input
        .configuration
        .as_mut()
        .unwrap()
        .config_inits
        .push(crate::harness::ConfigInit {
            display_name: None,
            path: crate::harness::AccessPath::Direct {
                address: crate::io::IoAddress::parse("%MW4096").unwrap(),
                text: "%MW4096".into(),
            },
            address: None,
            type_id: trust_hir::TypeId::WORD,
            initializer: Some(Expr::Literal(crate::value::Value::Word(17))),
        });
    let mut module = crate::bytecode::build_module_from_declarations(&input).unwrap();
    let Some(SectionData::ResourceMeta(resource)) = module.section_mut(SectionId::ResourceMeta)
    else {
        unreachable!()
    };
    assert_eq!(resource.resources[0].memory_size, 4098);
    resource.resources[0].memory_size = 4097;
    assert!(
        module.validate().is_err(),
        "direct startup action must fit its declared image"
    );
}

#[test]
fn lowered_partial_configuration_preserves_selection_and_rejects_overflow() {
    use crate::bytecode::{InitializationPhase, SectionData, SectionId};
    use crate::harness::{AccessPart, AccessPath, ConfigInit};
    use crate::value::{PartialAccess, Value};
    let sources = [SourceFile::new("PROGRAM Main VAR bits : WORD; END_VAR END_PROGRAM CONFIGURATION Conf PROGRAM P1 : Main; END_CONFIGURATION")];
    let mut input = lower_application(&sources, false)
        .unwrap()
        .prepare_authoring(&[])
        .unwrap();
    // The wire/lowered model supports partial configuration targets; source
    // VAR_CONFIG admission intentionally accepts symbolic variable paths only.
    input
        .configuration
        .as_mut()
        .unwrap()
        .config_inits
        .push(ConfigInit {
            display_name: None,
            path: AccessPath::Parts(vec![
                AccessPart::Name("P1".into()),
                AccessPart::Name("bits".into()),
                AccessPart::Partial(PartialAccess::Byte(1)),
            ]),
            address: None,
            type_id: trust_hir::TypeId::BYTE,
            initializer: Some(Expr::Literal(Value::Byte(7))),
        });
    let module = crate::bytecode::build_module_from_declarations(&input).unwrap();
    let bytes = module.encode().unwrap();
    let mut decoded = trust_runtime_core::bytecode::BytecodeModule::decode(&bytes).unwrap();
    decoded.validated_source_free(Default::default()).unwrap();
    let Some(SectionData::Initializers(index)) = decoded.section_mut(SectionId::Initializers)
    else {
        unreachable!()
    };
    let action = index
        .entries
        .iter_mut()
        .find(|entry| entry.phase == InitializationPhase::Configuration)
        .unwrap();
    assert_eq!((action.partial_kind, action.partial_index), (2, 1));
    action.partial_index = 2;
    assert!(
        decoded.validate().is_err(),
        "a third byte cannot fit in WORD storage"
    );
    let AccessPath::Parts(parts) = &mut input.configuration.as_mut().unwrap().config_inits[0].path
    else {
        unreachable!()
    };
    parts[2] = AccessPart::Partial(PartialAccess::Byte(2));
    assert!(
        crate::bytecode::build_module_from_declarations(&input).is_err(),
        "producer must reject the same out-of-range selection"
    );
}

#[test]
fn encoder_rejects_forced_access_map_binding_that_shadows_global_name() {
    use crate::value::Value;
    let source = r#"
VAR_GLOBAL
    A1 : INT := INT#111;
END_VAR

PROGRAM Main
VAR
    source : INT := INT#7;
    observed : INT := INT#0;
END_VAR
A1 := INT#55;
observed := A1;
END_PROGRAM

CONFIGURATION Conf
PROGRAM P1 : Main;
END_CONFIGURATION
"#;

    let mut runtime = lower_application(&[SourceFile::new(source)], false)
        .expect("source-level global without VAR_ACCESS collision should compile")
        .materialize_legacy(&[])
        .expect("materialize construction-only declarations");
    let p1_instance = match runtime.storage().get_global("P1") {
        Some(Value::Instance(id)) => *id,
        other => panic!("expected P1 global to hold program instance id, got {other:?}"),
    };
    let source_ref = runtime
        .storage()
        .ref_for_instance_recursive(p1_instance, "source")
        .expect("P1.source ref");
    runtime.access_map_mut().bind("A1".into(), source_ref, None);

    let err = crate::bytecode::build_legacy_module_from_builder(&runtime, &[source], &["file_0"])
        .expect_err("forced VAR_ACCESS/global name shadowing must fail during bytecode encoding");
    let message = err.to_string();
    assert!(
        message.contains("VAR_ACCESS") && message.contains("A1") && message.contains("global"),
        "expected VAR_ACCESS/global collision diagnostic, got {message}"
    );
}
