//! A complete program of several files, with types, a library, interfaces, their implementations,
//! a program and a configuration, compiles into a program image for this computer (specification
//! 44 §4.1).
//!
//! The test needs the toolchain of specification 44 §4.6 at `TRUST_TOOLCHAIN`.

use trust_runtime::harness::SourceFile;
use trust_runtime::native::{build, Options, Target, Toolchain};

#[test]
fn complete_program_compiles_without_errors() {
    let sources = vec![
        SourceFile::with_path(
            "types.st",
            include_str!("fixtures/complete_program/types.st"),
        ),
        SourceFile::with_path("lib.st", include_str!("fixtures/complete_program/lib.st")),
        SourceFile::with_path("api.st", include_str!("fixtures/complete_program/api.st")),
        SourceFile::with_path("impl.st", include_str!("fixtures/complete_program/impl.st")),
        SourceFile::with_path("main.st", include_str!("fixtures/complete_program/main.st")),
        SourceFile::with_path(
            "config.st",
            include_str!("fixtures/complete_program/config.st"),
        ),
    ];
    let toolchain = Toolchain::find().expect("the toolchain of specification 44 §4.6");
    let target = Target::this_cpu().expect("this computer's CPU runs program images");
    let work = std::env::temp_dir().join(format!("trust-complete-program-{}", std::process::id()));
    let built = build(
        &sources,
        &Options {
            target,
            hooks: true,
        },
        &toolchain,
        &work,
    );
    let _ = std::fs::remove_dir_all(&work);
    if let Err(err) = built {
        panic!("complete program compile failed:\n{err}");
    }
}
