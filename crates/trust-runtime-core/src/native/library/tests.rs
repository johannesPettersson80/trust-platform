//! The program library's hash (specification 44 §5.2).

use super::*;

/// Specification 44 §5.2, worked with Python's `hashlib`: the first 16 bytes of the SHA-256 of
/// `trust-plc-lib 1.2.3 riscv32imac-unknown-none-elf` and of `trust-plc-lib 1.2.3
/// x86_64-unknown-linux-gnu`. Catches a hash of the library without its target (one hash for both
/// CPUs), a separator other than one space, and another digest or its other bytes.
#[test]
fn the_library_hash_names_the_library_and_its_target() {
    assert_eq!(
        library_hash("trust-plc-lib 1.2.3", Target::Rv32imac),
        [
            0xed, 0xbc, 0xfa, 0x88, 0xc3, 0xe1, 0x1c, 0xc1, 0xd4, 0xf8, 0x22, 0xe9, 0x47, 0xf5,
            0x77, 0xf7
        ]
    );
    assert_eq!(
        library_hash("trust-plc-lib 1.2.3", Target::X86_64),
        [
            0xb3, 0x98, 0xb5, 0xea, 0xe8, 0xdd, 0x23, 0xb4, 0xbb, 0x90, 0x99, 0xf2, 0xa2, 0x92,
            0x79, 0x02
        ]
    );
    assert_eq!(
        PROGRAM_LIBRARY,
        concat!("trust-plc-lib ", env!("CARGO_PKG_VERSION"))
    );
}
