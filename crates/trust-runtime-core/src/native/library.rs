//! The program library a compiled program links (specification 44 §4.6, §5.2): `trust-plc-lib` of
//! a release, built for one target, which an image's header names by a hash and a kernel admits
//! when it is the library of its own release (§10.2, step 2).

use sha2::{Digest, Sha256};

use super::image::Target;

/// The program library of this kernel's release, as a toolchain of the same release names it.
pub const PROGRAM_LIBRARY: &str = concat!("trust-plc-lib ", env!("CARGO_PKG_VERSION"));

/// The Rust target triple a module for `target` is compiled for (specification 44 §4.4).
#[must_use]
pub fn triple(target: Target) -> &'static str {
    match target {
        Target::Rv32imac => "riscv32imac-unknown-none-elf",
        Target::ThumbV8mMainHf => "thumbv8m.main-none-eabihf",
        Target::X86_64 => "x86_64-unknown-linux-gnu",
        Target::Aarch64 => "aarch64-unknown-linux-gnu",
    }
}

/// The header's hash of the program library `library`, `trust-plc-lib <version>`, built for
/// `target`: the first 16 bytes of the SHA-256 of `<library> <triple>` (specification 44 §5.2).
#[must_use]
pub fn library_hash(library: &str, target: Target) -> [u8; 16] {
    let mut hasher = Sha256::new();
    hasher.update(library.as_bytes());
    hasher.update(b" ");
    hasher.update(triple(target).as_bytes());
    let digest = hasher.finalize();
    let mut hash = [0; 16];
    hash.copy_from_slice(&digest[..16]);
    hash
}

#[cfg(test)]
#[path = "library/tests.rs"]
mod tests;
