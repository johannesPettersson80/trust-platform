#[path = "common/bytecode_helpers.rs"]
mod bytecode_helpers;

use bytecode_helpers::base_module;
use trust_runtime::bytecode::{BytecodeError, BytecodeModule, BytecodeVersion};
use trust_runtime_core::error::{RuntimeError, StableErrorCode};
use trust_runtime_core::vm::{PreparationLimits, PreparedModule};

/// Spec 12 §11.1 (versioned compatibility window): source-free consumers execute 2.0 and 3.0 and reject 1.x without construction context. Spec 34 §11.3: source-free 1.1 loading remains rejected. The context-less 1.1 artifact is refused during preparation with the exact invalid-bytecode error below, before any VM module is built, so no prepared module (and no executable state) is installed.
#[test]
fn source_free_loader_rejects_a_context_less_1_1_artifact_and_installs_nothing() {
    let module = base_module();
    assert_eq!(module.version, BytecodeVersion::new(1, 1));
    let bytes = module.encode().expect("encode 1.1 artifact");

    let decoded = BytecodeModule::decode(&bytes).expect("1.1 is a supported legacy container");
    assert_eq!(decoded.version, BytecodeVersion::new(1, 1));

    let error = match PreparedModule::from_bytes(&bytes, PreparationLimits::default()) {
        Ok(_) => panic!("the source-free loader must not prepare a context-less 1.1 artifact"),
        Err(error) => error,
    };
    assert_eq!(
        error,
        RuntimeError::Bytecode {
            code: StableErrorCode::VmBytecodeDecode,
            detail: "missing construction metadata".into(),
        }
    );
}

/// Spec 12 §11.1 (versioned compatibility window): the source-free validator rejects a 1.1 artifact by its version alone, with the typed UnsupportedVersion error, before any construction check runs.
#[test]
fn source_free_validation_rejects_a_1_1_artifact_by_version_alone() {
    let bytes = base_module().encode().expect("encode 1.1 artifact");
    let decoded = BytecodeModule::decode(&bytes).expect("1.1 is a supported legacy container");

    let result = decoded.view().validated_source_free(Default::default());
    assert_eq!(
        result.err(),
        Some(BytecodeError::UnsupportedVersion { major: 1, minor: 1 })
    );
}
