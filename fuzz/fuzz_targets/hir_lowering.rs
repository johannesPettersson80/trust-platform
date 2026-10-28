#![no_main]

//! Front end and lowering fuzzing (specification 44 §6): arbitrary source goes through trust-hir and
//! the generator's lowering into the Rust of a module, which is not compiled. Every outcome is the
//! generated source or a refusal; none may panic.

use libfuzzer_sys::fuzz_target;
use trust_runtime::harness::SourceFile;
use trust_runtime::native::{generate_source, Options, Target};

const MAX_SOURCE_BYTES: usize = 16 * 1024;

fuzz_target!(|data: &[u8]| {
    let source = String::from_utf8_lossy(&data[..data.len().min(MAX_SOURCE_BYTES)]);
    let options = Options {
        target: Target::this_cpu().unwrap_or(Target::X86_64),
        hooks: true,
    };
    let _ = generate_source(&[SourceFile::new(source.into_owned())], &options);
});
