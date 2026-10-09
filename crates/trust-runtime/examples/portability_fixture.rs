//! Regenerate the separately versioned portability artifact on the validation builder.

use std::{error::Error, path::PathBuf};
use trust_runtime::{
    bytecode::BytecodeVersion,
    harness::{CompileSession, SourceFile},
};

fn main() -> Result<(), Box<dyn Error>> {
    let fixture =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/portability/stbc-2.0");
    let source = std::fs::read_to_string(fixture.join("main.st"))?;
    let module =
        CompileSession::from_sources(vec![SourceFile::with_path("portability/main.st", source)])
            .build_bytecode_module_for_version(BytecodeVersion::SOURCE_FREE)?;
    let bytes = module.encode()?;
    let decoded = trust_runtime_core::bytecode::BytecodeModule::decode(&bytes)?;
    decoded.validated_source_free(Default::default())?;
    std::fs::write(fixture.join("program-v2.stbc"), bytes)?;
    std::fs::write(
        fixture.join("program-v2.disassembly.txt"),
        decoded.disassemble()?,
    )?;
    Ok(())
}
