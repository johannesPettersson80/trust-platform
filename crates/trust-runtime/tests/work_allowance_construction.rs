//! TIME-09 construction allowance: instantiation and candidate construction are each one entry
//! under `max_construction_work`, and a refused candidate leaves the installed application intact.
use trust_runtime::bytecode::BytecodeVersion;
use trust_runtime::harness::CompileSession;
use trust_runtime::Runtime;
use trust_runtime_core::error::{ExecutionLimitKind, RuntimeError};
use trust_runtime_core::value::Value;
use trust_runtime_core::vm::{PreparationLimits, PreparedModule};

fn artifact(source: &str) -> Vec<u8> {
    CompileSession::from_source(source)
        .build_bytecode_module_for_version(BytecodeVersion::SOURCE_FREE)
        .expect("accepted source")
        .encode()
        .unwrap()
}

fn scheduled(body: &str) -> String {
    format!(
        "{body}\nCONFIGURATION Conf RESOURCE Controller ON PLC TASK Periodic (INTERVAL := T#10ms, \
         PRIORITY := 0); PROGRAM Plant WITH Periodic : Main; END_RESOURCE END_CONFIGURATION"
    )
}

#[test]
fn instantiation_under_one_construction_unit_refuses_its_first_construct_node() {
    let bytes = artifact(&scheduled(
        "PROGRAM Main VAR count : INT := INT#3; END_VAR count := count + INT#1; END_PROGRAM",
    ));
    let prepared = PreparedModule::from_bytes(
        &bytes,
        PreparationLimits {
            max_construction_work: 1,
            ..Default::default()
        },
    )
    .expect("a positive construction allowance admits the artifact");

    // The program instance node takes the one unit. Its member declaration is the next CONSTRUCT
    // node, and it is refused before the instance is issued.
    let error = prepared
        .instantiate(0)
        .err()
        .expect("one unit cannot construct the program instance and its member");
    assert_eq!(
        error,
        RuntimeError::ExecutionLimit {
            kind: ExecutionLimitKind::Work,
            used: 1,
            requested: 1,
            limit: 1,
        }
    );
}

#[test]
fn candidate_construction_over_the_installed_allowance_leaves_the_installed_state_intact() {
    let limits = PreparationLimits {
        max_construction_work: 100,
        ..Default::default()
    };
    let installed = artifact(&scheduled(
        "VAR_GLOBAL big : ARRAY[0..9] OF INT; END_VAR PROGRAM Main END_PROGRAM",
    ));
    // Constructing 5,000 elements needs far more than the installed profile's 100 units.
    let candidate = artifact(&scheduled(
        "VAR_GLOBAL big : ARRAY[0..4999] OF INT; END_VAR PROGRAM Main END_PROGRAM",
    ));
    let mut runtime =
        Runtime::from_bytes(&installed, limits).expect("the installed artifact fits 100 units");

    let error = runtime
        .apply_bytecode_bytes(&candidate, None)
        .expect_err("the candidate exceeds the construction allowance");
    assert!(matches!(
        error,
        RuntimeError::ExecutionLimit {
            kind: ExecutionLimitKind::Work,
            limit: 100,
            ..
        }
    ));

    let Some(Value::Array(big)) = runtime.storage().get_global("big") else {
        panic!("the installed global big")
    };
    assert_eq!(big.elements().len(), 10);
    assert!(!runtime.faulted());
}
