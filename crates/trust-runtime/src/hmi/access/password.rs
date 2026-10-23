//! HMI passwords (specification 39, §12.8): kept only as a salted PBKDF2-HMAC-SHA256 hash,
//! `pbkdf2-sha256:<iterations>:<salt>:<hash>` with the salt and hash in unpadded URL-safe base64.

/// The fewest iterations a stored hash may use.
pub const MIN_ITERATIONS: u32 = 10_000;
/// The iterations a new hash uses (OWASP's figure for PBKDF2-HMAC-SHA256).
pub const DEFAULT_ITERATIONS: u32 = 600_000;

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use hmac::{Hmac, Mac};
use sha2::Sha256;

const PREFIX: &str = "pbkdf2-sha256";
/// Bytes of a derived hash.
const HASH_BYTES: usize = 32;
/// The fewest salt bytes a stored hash may use.
const MIN_SALT_BYTES: usize = 8;

/// A stored password hash.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PasswordHash {
    iterations: u32,
    salt: Vec<u8>,
    hash: Vec<u8>,
}

impl PasswordHash {
    /// Reads a stored hash.
    pub fn parse(text: &str) -> Result<Self, String> {
        let shape = || {
            format!("expected `{PREFIX}:<iterations>:<salt>:<hash>` (made by `trust-runtime hmi-password`)")
        };
        let mut parts = text.trim().split(':');
        let (Some(PREFIX), Some(iterations), Some(salt), Some(hash), None) = (
            parts.next(),
            parts.next(),
            parts.next(),
            parts.next(),
            parts.next(),
        ) else {
            return Err(shape());
        };
        let iterations: u32 = iterations.parse().map_err(|_| shape())?;
        if iterations < MIN_ITERATIONS {
            return Err(format!(
                "at least {MIN_ITERATIONS} iterations are needed, not {iterations}"
            ));
        }
        let salt = URL_SAFE_NO_PAD.decode(salt).map_err(|_| shape())?;
        let hash = URL_SAFE_NO_PAD.decode(hash).map_err(|_| shape())?;
        if salt.len() < MIN_SALT_BYTES || hash.len() != HASH_BYTES {
            return Err(shape());
        }
        Ok(Self {
            iterations,
            salt,
            hash,
        })
    }

    /// Whether a password matches, compared in constant time.
    pub fn verify(&self, password: &str) -> bool {
        let derived = pbkdf2_sha256(password.as_bytes(), &self.salt, self.iterations, HASH_BYTES);
        derived.len() == self.hash.len()
            && derived
                .iter()
                .zip(&self.hash)
                .fold(0_u8, |difference, (left, right)| {
                    difference | (left ^ right)
                })
                == 0
    }
}

/// A new stored hash of a password.
pub fn hash_password(password: &str, iterations: u32, salt: &[u8]) -> String {
    let hash = pbkdf2_sha256(password.as_bytes(), salt, iterations, HASH_BYTES);
    format!(
        "{PREFIX}:{iterations}:{}:{}",
        URL_SAFE_NO_PAD.encode(salt),
        URL_SAFE_NO_PAD.encode(hash)
    )
}

/// PBKDF2 with HMAC-SHA256 (RFC 8018, section 5.2).
pub fn pbkdf2_sha256(password: &[u8], salt: &[u8], iterations: u32, length: usize) -> Vec<u8> {
    let keyed = <Hmac<Sha256> as hmac::KeyInit>::new_from_slice(password)
        .expect("HMAC takes a key of any length");
    let mut derived = Vec::with_capacity(length);
    let mut block: u32 = 1;
    while derived.len() < length {
        let mut mac = keyed.clone();
        mac.update(salt);
        mac.update(&block.to_be_bytes());
        let mut chained = mac.finalize().into_bytes();
        let mut output = chained;
        for _ in 1..iterations {
            let mut mac = keyed.clone();
            mac.update(&chained);
            chained = mac.finalize().into_bytes();
            for (byte, next) in output.iter_mut().zip(chained.iter()) {
                *byte ^= next;
            }
        }
        derived.extend_from_slice(&output);
        block += 1;
    }
    derived.truncate(length);
    derived
}
