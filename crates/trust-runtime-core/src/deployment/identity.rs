//! The identities of a deployment record (specification 34 §5.1.1).

use sha2::{Digest, Sha256};

/// The SHA-256 of `bytes` (FIPS 180-4).
#[must_use]
pub fn sha256(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

/// The SHA-256 of the bytes of `parts` one after the other (FIPS 180-4): the digest of their
/// concatenation, worked out part by part without joining them. A reply that a chip holds in
/// parts of 4,096 bytes is hashed this way, because its heap has no block for the whole reply
/// (specification 34, PORT-05).
#[must_use]
pub fn sha256_parts<'a>(parts: impl IntoIterator<Item = &'a [u8]>) -> [u8; 32] {
    let mut hasher = Sha256::new();
    for part in parts {
        hasher.update(part);
    }
    hasher.finalize().into()
}

/// The SHA-256 of bytes that arrive in pieces over time (FIPS 180-4): the digest of their
/// concatenation, worked out piece by piece. A chip works out the SHA-256 of an upload that it writes
/// into its program store this way, part by part as each part is written, because it holds no more
/// of the upload than one part (specification 34 §5.1).
#[derive(Clone, Default)]
pub struct Sha256Stream(Sha256);

impl Sha256Stream {
    /// The SHA-256 of no bytes yet.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds `bytes` after the bytes added before.
    pub fn update(&mut self, bytes: &[u8]) {
        self.0.update(bytes);
    }

    /// The SHA-256 of every byte added, in the order added.
    #[must_use]
    pub fn finish(self) -> [u8; 32] {
        self.0.finalize().into()
    }
}

impl core::fmt::Debug for Sha256Stream {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str("Sha256Stream")
    }
}

/// The configuration identity: SHA-256 over the project's `runtime.toml` and then its
/// `io.toml`, each as its file name, a zero byte, and either a byte 1, its length as eight
/// little-endian bytes and its bytes, or a byte 0 when the project has no such file.
#[must_use]
pub fn configuration_identity(runtime_toml: Option<&[u8]>, io_toml: Option<&[u8]>) -> [u8; 32] {
    let mut hasher = Sha256::new();
    for (name, content) in [("runtime.toml", runtime_toml), ("io.toml", io_toml)] {
        hasher.update(name.as_bytes());
        hasher.update([0u8]);
        match content {
            Some(bytes) => {
                hasher.update([1u8]);
                hasher.update(super::wide(bytes.len()).to_le_bytes());
                hasher.update(bytes);
            }
            None => hasher.update([0u8]),
        }
    }
    hasher.finalize().into()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The digest of `hex`, 64 hexadecimal digits.
    fn digest(hex: &str) -> [u8; 32] {
        let mut bytes = [0u8; 32];
        for (index, byte) in bytes.iter_mut().enumerate() {
            *byte = u8::from_str_radix(&hex[index * 2..index * 2 + 2], 16).unwrap();
        }
        bytes
    }

    /// FIPS 180-4's example "abc" (Appendix B.1 of FIPS 180-2) fed as "a" and then "bc" has the
    /// digest of the whole message, and no bytes have the digest of the empty message. Catches a
    /// stream that restarts its state at each piece or pads each piece on its own.
    #[test]
    fn a_stream_of_pieces_has_the_digest_of_their_concatenation() {
        let mut stream = Sha256Stream::new();
        stream.update(b"a");
        stream.update(b"bc");
        assert_eq!(
            stream.finish(),
            digest("ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad")
        );
        assert_eq!(
            Sha256Stream::new().finish(),
            digest("e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855")
        );
    }
}
