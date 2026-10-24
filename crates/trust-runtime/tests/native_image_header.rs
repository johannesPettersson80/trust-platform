//! The program image's header names what it was built from (specification 44 §5.2): the hash of its
//! sources and the program library it links, which a kernel of the same release admits (§10.2).
//!
//! The test needs the toolchain of specification 44 §4.6 at `TRUST_TOOLCHAIN`.

use trust_runtime::harness::SourceFile;
use trust_runtime::native::{build, Options, Target, Toolchain};

/// A counter in a 10 ms task.
const COUNTER: &str = "
PROGRAM Counter
VAR
    Count : UDINT;
END_VAR
Count := Count + 1;
END_PROGRAM

CONFIGURATION Probe
RESOURCE Probe ON PLC
    TASK Fast (INTERVAL := T#10ms, PRIORITY := 1);
    PROGRAM Main WITH Fast : Counter;
END_RESOURCE
END_CONFIGURATION
";

/// Specification 44 §5.2, worked with Python's `hashlib`: the sources hash is the SHA-256 of each
/// source's label, a zero byte, its text and a zero byte, in the order of the labels:
/// `src/a.st\0A\0src/b.st\0B\0` for the sources given as `src/b.st` then `src/a.st`. Catches the
/// sources hashed in the order given, the labels left out, and texts run together.
#[test]
fn the_sources_hash_takes_each_label_and_text_in_label_order() {
    let sources = [
        SourceFile::with_path("src/b.st", "B"),
        SourceFile::with_path("src/a.st", "A"),
    ];
    assert_eq!(
        trust_runtime::native::sources_hash(&sources),
        [
            0xa0, 0x61, 0x19, 0xe9, 0xc7, 0x6c, 0xcb, 0x9d, 0x73, 0x48, 0xdc, 0xdb, 0x20, 0xa2,
            0x90, 0x2e, 0x3f, 0x8d, 0xbc, 0xcf, 0x68, 0x33, 0x30, 0x1f, 0xb6, 0x0d, 0xed, 0x7b,
            0xdb, 0x39, 0xc3, 0x28
        ]
    );
}

/// Specification 44 §5.2 and §10.2, step 2: an image built with this release's toolchain names
/// its sources' hash and this release's program library, which this release's kernel admits.
/// Catches a header left at zeros, which no kernel admits, and a library hash of another release.
#[test]
fn a_built_image_names_its_sources_and_this_releases_program_library() {
    let toolchain = Toolchain::from_env()
        .expect("TRUST_TOOLCHAIN names the toolchain of specification 44 §4.6");
    let sources = [SourceFile::with_path("src/counter.st", COUNTER)];
    let host = Target::this_cpu().expect("a computer target");
    let work = std::env::temp_dir().join(format!("trust-native-header-{}", std::process::id()));
    let built = build(
        &sources,
        &Options {
            target: host,
            hooks: true,
        },
        &toolchain,
        &work,
    )
    .expect("the counter builds");
    let _ = std::fs::remove_dir_all(&work);
    let image =
        trust_runtime_core::native::image::Image::parse(&built.image).expect("the image parses");
    assert_eq!(
        image.header().source_hash,
        trust_runtime::native::sources_hash(&sources)
    );
    assert_eq!(
        image.header().library,
        trust_runtime_core::native::library::library_hash(
            trust_runtime_core::native::library::PROGRAM_LIBRARY,
            host
        )
    );
}
