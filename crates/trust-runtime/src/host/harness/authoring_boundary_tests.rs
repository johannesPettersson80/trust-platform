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
    let error = match lowered.construct(&[]) {
        Ok(_) => panic!("startup must still reject division by zero"),
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
