//! Author the board probe without changing the saved A4 application artifacts.
use std::{error::Error, path::PathBuf};
use trust_runtime::{
    bytecode::BytecodeVersion,
    harness::{CompileSession, SourceFile},
};

fn main() -> Result<(), Box<dyn Error>> {
    let directory =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/portability/f401");
    let source = std::fs::read_to_string(directory.join("gpio.st"))?;
    let module = CompileSession::from_sources(vec![SourceFile::with_path(
        "portability/f401/gpio.st",
        source,
    )])
    .build_bytecode_module_for_version(BytecodeVersion::SOURCE_FREE)?;
    let bytes = module.encode()?;
    let decoded = trust_runtime_core::bytecode::BytecodeModule::decode(&bytes)?;
    decoded.validated_source_free(Default::default())?;
    std::fs::write(directory.join("gpio.stbc"), bytes)?;
    std::fs::write(
        directory.join("gpio.disassembly.txt"),
        decoded.disassemble()?,
    )?;
    Ok(())
}
