//! The signature of a program image (specification 44 §5.11): the `SIGN` section, with its
//! algorithm, the signer's public key and the Ed25519ph signature over the SHA-512 of every byte
//! before the section, and a device's check of it against the keys it trusts (§10.2, step 4).

use alloc::boxed::Box;

use ed25519_dalek::{Signature, VerifyingKey};
use sha2::{Digest as _, Sha512};

use super::image::{Image, SectionId};

/// The bytes of `SIGN`: the algorithm (4), the public key (32) and the signature (64).
pub const SIGN_BYTES: usize = 100;

/// The algorithm of an image that no key signed: the build reserves `SIGN` so that a tool that
/// deploys signs it in place.
pub const UNSIGNED: u32 = 0;

/// Ed25519ph (RFC 8032 §5.1) with the context [`CONTEXT`]. Algorithm 1, the pure Ed25519 of
/// specification 44 0.20, is malformed.
pub const ED25519PH: u32 = 2;

/// The context of an image's signature, so that it is never taken for a signature over anything
/// else the same key signs (specification 44 §5.11).
pub const CONTEXT: &[u8] = b"truST program image";

/// The SHA-512 of the bytes an image's signature covers, every byte before `SIGN`, which a device
/// builds as an upload's parts arrive so that its check pays only for the signature's arithmetic
/// (specification 44 §5.11).
///
/// The hash's state lies in the heap, so that a value that holds a prehash stays small.
#[derive(Clone, Default)]
pub struct Prehash(Box<Sha512>);

impl Prehash {
    /// The hash of no bytes.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds `bytes`, the next bytes of the image.
    pub fn update(&mut self, bytes: &[u8]) {
        self.0.update(bytes);
    }
}

impl core::fmt::Debug for Prehash {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("Prehash")
    }
}

/// Two prehashes are equal when they hashed the same bytes so far.
impl PartialEq for Prehash {
    fn eq(&self, other: &Self) -> bool {
        (*self.0).clone().finalize() == (*other.0).clone().finalize()
    }
}

impl Eq for Prehash {}

/// Why a device refuses an image's signature (§10.2, step 4).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SignatureError {
    /// `SIGN` has another length or names an algorithm the kernel does not verify.
    Malformed,
    /// The signature does not verify over the image's bytes.
    Invalid,
    /// The device trusts keys, and the image is unsigned or signed by a key it does not trust.
    Untrusted,
}

/// Checks the signature of `image` against the keys `trusted` the device trusts (specification 44
/// §5.11): a malformed `SIGN` is refused on every device; a device that trusts no key checks nothing
/// more, because with no key to trust a signature proves nothing that the record's hash (§10.2,
/// step 1) does not; a device that trusts keys admits only an image signed by one of them whose
/// signature verifies over every byte before `SIGN`, and judges the key before it verifies.
///
/// # Errors
///
/// The [`SignatureError`] of a malformed section, a signer the device does not trust, or a
/// signature that does not verify.
pub fn check(image: &Image<'_>, trusted: &[[u8; 32]]) -> Result<(), SignatureError> {
    check_with(image, trusted, None)
}

/// [`check`] with `prehash`, the SHA-512 of every byte of `image` before `SIGN` that the caller
/// built as the bytes arrived, so that the check does not hash them again. A prehash of other bytes
/// fails the signature.
///
/// # Errors
///
/// As [`check`].
pub fn check_prehashed(
    image: &Image<'_>,
    trusted: &[[u8; 32]],
    prehash: Prehash,
) -> Result<(), SignatureError> {
    check_with(image, trusted, Some(prehash))
}

/// [`check`], hashing the signed bytes unless `prehash` holds their hash.
fn check_with(
    image: &Image<'_>,
    trusted: &[[u8; 32]],
    prehash: Option<Prehash>,
) -> Result<(), SignatureError> {
    let claim = claim(image)?;
    if trusted.is_empty() {
        return Ok(());
    }
    let Some(claim) = claim.filter(|claim| trusted.contains(&claim.key)) else {
        return Err(SignatureError::Untrusted);
    };
    let prehash = match prehash {
        Some(Prehash(hash)) => *hash,
        None => {
            let bytes = image
                .bytes()
                .get(..claim.start)
                .ok_or(SignatureError::Malformed)?;
            Sha512::new_with_prefix(bytes)
        }
    };
    VerifyingKey::from_bytes(&claim.key)
        .and_then(|key| {
            key.verify_prehashed_strict(
                prehash,
                Some(CONTEXT),
                &Signature::from_bytes(&claim.signature),
            )
        })
        .map_err(|_| SignatureError::Invalid)
}

/// What the `SIGN` of a signed image claims.
struct Claim {
    /// The signer's public key.
    key: [u8; 32],
    /// The signature.
    signature: [u8; 64],
    /// Where `SIGN` starts: the end of the signed bytes.
    start: usize,
}

/// What `SIGN` of `image` claims; `None` for an image without `SIGN` or with `SIGN` unsigned.
fn claim(image: &Image<'_>) -> Result<Option<Claim>, SignatureError> {
    let Some(section) = image.section(SectionId::Sign) else {
        return Ok(None);
    };
    let content = image.section_bytes(SectionId::Sign);
    if content.len() != SIGN_BYTES {
        return Err(SignatureError::Malformed);
    }
    let algorithm = u32::from_le_bytes([content[0], content[1], content[2], content[3]]);
    match algorithm {
        UNSIGNED => Ok(None),
        ED25519PH => {
            let mut key = [0u8; 32];
            key.copy_from_slice(&content[4..36]);
            let mut signature = [0u8; 64];
            signature.copy_from_slice(&content[36..SIGN_BYTES]);
            let start = usize::try_from(section.offset).map_err(|_| SignatureError::Malformed)?;
            Ok(Some(Claim {
                key,
                signature,
                start,
            }))
        }
        _ => Err(SignatureError::Malformed),
    }
}

#[cfg(test)]
#[path = "signature/tests.rs"]
mod tests;
