//! Write the separate current-format board probe into the batch's artifact directory.
use std::{error::Error, io, path::PathBuf};
use trust_runtime::{
    bytecode::BytecodeVersion,
    harness::{CompileSession, SourceFile},
    value::StorageAbi,
};

fn main() -> Result<(), Box<dyn Error>> {
    let mut arguments = std::env::args_os().skip(1);
    let directory = PathBuf::from(arguments.next().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "expected an explicit artifact directory",
        )
    })?);
    if arguments.next().is_some() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "expected one artifact directory",
        )
        .into());
    }
    let source = include_str!("../tests/fixtures/portability/stbc-3.0/gpio-temporal.st");
    let module = CompileSession::from_sources(vec![SourceFile::with_path(
        "portability/stbc-3.0/gpio-temporal.st",
        source,
    )])
    .build_bytecode_module_for_version(BytecodeVersion::SOURCE_FREE)?;
    let bytes = module.encode()?;
    let decoded = trust_runtime_core::bytecode::BytecodeModule::decode(&bytes)?;
    decoded.validated_source_free(Default::default())?;
    if decoded.version != BytecodeVersion::SOURCE_FREE
        || decoded.storage_abi()? != StorageAbi::UniformTemporal64
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "expected current uniform-temporal artifact",
        )
        .into());
    }
    std::fs::create_dir_all(&directory)?;
    std::fs::write(directory.join("gpio-temporal-v3.stbc"), bytes)?;
    std::fs::write(
        directory.join("gpio-temporal-v3.disassembly.txt"),
        decoded.disassemble()?,
    )?;
    Ok(())
}
