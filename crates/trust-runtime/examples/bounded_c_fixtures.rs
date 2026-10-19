//! Generate the bounded-C corpus that `trust-runtime-core`'s `scan_allocation_zero` test
//! measures (spec 34 ARCH-05 and MEM-04; bounded operation slice C1-1). Run it on the builder,
//! from the workspace root, into the saved fixture directory:
//!
//! ```sh
//! cargo run --locked -p trust-runtime --example bounded_c_fixtures -- crates/trust-runtime/tests/fixtures/bounded-c
//! ```
//!
//! Each fixture is its own project folder, `<name>/trust-lsp.toml` and `<name>/src/main.st`.
//! The command writes `<name>/program.stbc` in the current source-free format (STBC 3.0) and
//! `<name>/program.disassembly.txt` beside them. Every source is compiled, and every failure
//! is reported, before the command fails; the artifacts are never written by hand.
use std::{error::Error, io, path::PathBuf};
use trust_runtime::{
    bytecode::BytecodeVersion,
    harness::{CompileSession, SourceFile},
};

/// The corpus. The test's fixture table names the same folders and rejects any other.
const SOURCES: [(&str, &str); 20] = [
    (
        "scalar",
        include_str!("../tests/fixtures/bounded-c/scalar/src/main.st"),
    ),
    // The board GPIO probe, unchanged, in the current format.
    (
        "gpio",
        include_str!("../tests/fixtures/portability/f401/gpio.st"),
    ),
    (
        "io",
        include_str!("../tests/fixtures/bounded-c/io/src/main.st"),
    ),
    (
        "calls",
        include_str!("../tests/fixtures/bounded-c/calls/src/main.st"),
    ),
    (
        "blocks",
        include_str!("../tests/fixtures/bounded-c/blocks/src/main.st"),
    ),
    (
        "methods",
        include_str!("../tests/fixtures/bounded-c/methods/src/main.st"),
    ),
    (
        "instances",
        include_str!("../tests/fixtures/bounded-c/instances/src/main.st"),
    ),
    (
        "arrays",
        include_str!("../tests/fixtures/bounded-c/arrays/src/main.st"),
    ),
    (
        "structures",
        include_str!("../tests/fixtures/bounded-c/structures/src/main.st"),
    ),
    (
        "references",
        include_str!("../tests/fixtures/bounded-c/references/src/main.st"),
    ),
    (
        "strings",
        include_str!("../tests/fixtures/bounded-c/strings/src/main.st"),
    ),
    (
        "initializers",
        include_str!("../tests/fixtures/bounded-c/initializers/src/main.st"),
    ),
    (
        "sizeof",
        include_str!("../tests/fixtures/bounded-c/sizeof/src/main.st"),
    ),
    (
        "retain",
        include_str!("../tests/fixtures/bounded-c/retain/src/main.st"),
    ),
    (
        "retained_blocks",
        include_str!("../tests/fixtures/bounded-c/retained_blocks/src/main.st"),
    ),
    (
        "retained_values",
        include_str!("../tests/fixtures/bounded-c/retained_values/src/main.st"),
    ),
    (
        "faults",
        include_str!("../tests/fixtures/bounded-c/faults/src/main.st"),
    ),
    (
        "copies",
        include_str!("../tests/fixtures/bounded-c/copies/src/main.st"),
    ),
    (
        "overlays",
        include_str!("../tests/fixtures/bounded-c/overlays/src/main.st"),
    ),
    (
        "unions",
        include_str!("../tests/fixtures/bounded-c/unions/src/main.st"),
    ),
];

fn main() -> Result<(), Box<dyn Error>> {
    let mut arguments = std::env::args_os().skip(1);
    let directory = PathBuf::from(arguments.next().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "expected the corpus directory, crates/trust-runtime/tests/fixtures/bounded-c",
        )
    })?);
    if arguments.next().is_some() {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "expected one directory").into());
    }
    let mut failures = Vec::new();
    for (name, source) in SOURCES {
        match artifact(name, source) {
            Ok((bytes, disassembly)) => {
                let fixture = directory.join(name);
                std::fs::create_dir_all(&fixture)?;
                std::fs::write(fixture.join("program.stbc"), &bytes)?;
                std::fs::write(fixture.join("program.disassembly.txt"), disassembly)?;
                println!("bounded-c,{name},{} bytes", bytes.len());
            }
            Err(error) => failures.push(format!("{name}: {error}")),
        }
    }
    if failures.is_empty() {
        Ok(())
    } else {
        Err(io::Error::other(failures.join("\n")).into())
    }
}

/// Compile one source, then decode and validate the bytes as the portable consumer does.
fn artifact(name: &str, source: &str) -> Result<(Vec<u8>, String), Box<dyn Error>> {
    let module = CompileSession::from_sources(vec![SourceFile::with_path(
        format!("bounded-c/{name}/src/main.st"),
        source,
    )])
    .build_bytecode_module_for_version(BytecodeVersion::SOURCE_FREE)?;
    let bytes = module.encode()?;
    let decoded = trust_runtime_core::bytecode::BytecodeModule::decode(&bytes)?;
    decoded.validated_source_free(Default::default())?;
    if decoded.version != BytecodeVersion::SOURCE_FREE {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "expected STBC 3.0").into());
    }
    Ok((bytes, decoded.disassemble()?))
}
