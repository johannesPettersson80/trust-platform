//! Regenerate the separately versioned portability artifact on the validation builder.

use std::{error::Error, path::PathBuf};
use trust_runtime::{
    bytecode::BytecodeVersion,
    harness::{CompileSession, SourceFile},
};

fn main() -> Result<(), Box<dyn Error>> {
    let fixture =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/portability/stbc-2.0");
    for (source_name, artifact_name) in [("main.st", "program-v2"), ("numeric.st", "numeric-v2")] {
        let source = std::fs::read_to_string(fixture.join(source_name))?;
        let source_path = format!("portability/{source_name}");
        let module = CompileSession::from_sources(vec![SourceFile::with_path(source_path, source)])
            .build_bytecode_module_for_version(BytecodeVersion::SOURCE_FREE)?;
        let bytes = module.encode()?;
        let decoded = trust_runtime_core::bytecode::BytecodeModule::decode(&bytes)?;
        decoded.validated_source_free(Default::default())?;
        std::fs::write(fixture.join(format!("{artifact_name}.stbc")), bytes)?;
        std::fs::write(
            fixture.join(format!("{artifact_name}.disassembly.txt")),
            decoded.disassemble()?,
        )?;
    }
    Ok(())
}
